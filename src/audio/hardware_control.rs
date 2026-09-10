use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum ControlEvent {
    Play(bool),
    Stop,
    Record(bool),
    TrackVolume { track: usize, value: f32 },
    TrackMute { track: usize, mute: bool },
    BusVolume { bus: usize, value: f32 },
    VcaVolume { vca: usize, value: f32 },
    /// A note on/off from a real MIDI keyboard (`on == false` = note off).
    MidiNote { note: u8, velocity: u8, on: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum OscArg {
    Float(f32),
    Int(i32),
    Str(String),
}

/// Parses a single OSC message. Returns `None` for malformed data.
pub fn parse_osc_message(data: &[u8]) -> Option<(String, Vec<OscArg>)> {
    if data.starts_with(b"#bundle\0") {
        // Skip "#bundle\0" (8 bytes) + timetag (8 bytes), then read sized messages.
        let mut offset = 16usize;
        while offset + 4 <= data.len() {
            let len = i32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;
            if len == 0 || offset + len > data.len() {
                break;
            }
            if let Some(msg) = parse_osc_message(&data[offset..offset + len]) {
                return Some(msg);
            }
            offset += len;
            offset = (offset + 3) & !3;
        }
        return None;
    }
    let (address, mut offset) = read_osc_string(data, 0)?;
    let (tags, next) = read_osc_string(data, offset)?;
    offset = next;
    if !tags.starts_with(',') {
        return Some((address, Vec::new()));
    }
    let mut args = Vec::new();
    for tag in tags[1..].chars() {
        match tag {
            'f' => {
                if offset + 4 > data.len() {
                    return None;
                }
                args.push(OscArg::Float(f32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ])));
                offset += 4;
            }
            'i' => {
                if offset + 4 > data.len() {
                    return None;
                }
                args.push(OscArg::Int(i32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ])));
                offset += 4;
            }
            's' => {
                let (s, next) = read_osc_string(data, offset)?;
                args.push(OscArg::Str(s));
                offset = next;
            }
            _ => break,
        }
    }
    Some((address, args))
}

fn read_osc_string(data: &[u8], start: usize) -> Option<(String, usize)> {
    if start >= data.len() {
        return None;
    }
    let end = data[start..].iter().position(|&b| b == 0)? + start;
    let s = String::from_utf8_lossy(&data[start..end]).to_string();
    let padded = (end + 1 + 3) & !3;
    Some((s, padded))
}

/// Maps an OSC address + args to a control event. Unknown addresses return `None`.
pub fn osc_to_event(address: &str, args: &[OscArg]) -> Option<ControlEvent> {
    let num = |i: usize| -> f32 {
        match args.get(i) {
            Some(OscArg::Float(f)) => *f,
            Some(OscArg::Int(v)) => *v as f32,
            Some(OscArg::Str(s)) => s.parse().unwrap_or(0.0),
            None => 0.0,
        }
    };
    let as_bool = |i: usize| num(i) >= 0.5;

    let parts: Vec<&str> = address.trim_start_matches('/').split('/').collect();
    match parts.as_slice() {
        ["sonix", "transport", "play"] => Some(ControlEvent::Play(as_bool(0))),
        ["sonix", "transport", "stop"] => Some(ControlEvent::Stop),
        ["sonix", "transport", "record"] => Some(ControlEvent::Record(as_bool(0))),
        ["sonix", "track", n, "volume"] => n.parse::<usize>().ok().map(|t| ControlEvent::TrackVolume {
            track: t.saturating_sub(1),
            value: num(0),
        }),
        ["sonix", "track", n, "mute"] => n.parse::<usize>().ok().map(|t| ControlEvent::TrackMute {
            track: t.saturating_sub(1),
            mute: as_bool(0),
        }),
        ["sonix", "bus", name, "volume"] => {
            let bus = match *name {
                "vocal" => 0,
                "drum" => 1,
                "synth" => 2,
                "fx" => 3,
                _ => return None,
            };
            Some(ControlEvent::BusVolume { bus, value: num(0) })
        }
        ["sonix", "vca", n, "volume"] => n.parse::<usize>().ok().map(|v| ControlEvent::VcaVolume {
            vca: v.saturating_sub(1),
            value: num(0),
        }),
        _ => None,
    }
}

/// Real OSC/UDP receiver running on a background thread.
pub struct OscServer {
    pub bound_port: u16,
    pub event_count: Arc<AtomicUsize>,
    pub last_error: Arc<Mutex<Option<String>>>,
    running: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl OscServer {
    pub fn start(rx_port: u16, tx: Sender<ControlEvent>) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", rx_port))?;
        let bound_port = socket.local_addr().map(|a| a.port()).unwrap_or(rx_port);
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let running = Arc::new(AtomicBool::new(true));
        let event_count = Arc::new(AtomicUsize::new(0));
        let last_error = Arc::new(Mutex::new(None));

        let running_t = running.clone();
        let count_t = event_count.clone();
        let err_t = last_error.clone();
        let join = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while running_t.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buf) {
                    Ok((n, _addr)) => {
                        if let Some((addr, args)) = parse_osc_message(&buf[..n]) {
                            count_t.fetch_add(1, Ordering::Relaxed);
                            if let Some(ev) = osc_to_event(&addr, &args) {
                                let _ = tx.send(ev);
                            }
                        }
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(e) => {
                        if let Ok(mut slot) = err_t.lock() {
                            *slot = Some(e.to_string());
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            bound_port,
            event_count,
            last_error,
            running,
            join: Some(join),
        })
    }

    pub fn received(&self) -> usize {
        self.event_count.load(Ordering::Relaxed)
    }
}

impl Drop for OscServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Real Mackie Control Universal input over the ALSA sequencer.
pub struct McuInput {
    pub event_count: Arc<AtomicUsize>,
    pub devices: Arc<Mutex<Vec<String>>>,
    running: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl McuInput {
    /// Opens an ALSA sequencer input port. External controllers connect to
    /// "Sonix MCU In" (e.g. with `aconnect`).
    pub fn connect(tx: Sender<ControlEvent>) -> Result<Self, String> {
        use alsa::seq::{PortCap, PortType, Seq};

        let seq = Seq::open(None, Some(alsa::Direction::Capture), true)
            .map_err(|e| format!("Kunde inte öppna ALSA-sequencer: {}", e))?;
        seq.set_client_name(&std::ffi::CString::new("Sonix").unwrap())
            .map_err(|e| e.to_string())?;
        seq.create_simple_port(
            &std::ffi::CString::new("Sonix MCU In").unwrap(),
            PortCap::WRITE | PortCap::SUBS_WRITE,
            PortType::MIDI_GENERIC | PortType::APPLICATION,
        )
        .map_err(|e| format!("Kunde inte skapa MIDI-port: {}", e))?;

        let devices = Arc::new(Mutex::new(Self::list_devices(&seq)));
        let event_count = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let running_t = running.clone();
        let count_t = event_count.clone();
        let devices_t = devices.clone();
        let join = thread::spawn(move || {
            use alsa::seq::{EventType, EvCtrl, EvNote};
            let mut input = seq.input();
            let mut refresh = 0u32;
            while running_t.load(Ordering::Relaxed) {
                match input.event_input() {
                    Ok(ev) => {
                        count_t.fetch_add(1, Ordering::Relaxed);
                        match ev.get_type() {
                            EventType::Noteon | EventType::Noteoff => {
                                if let Some(n) = ev.get_data::<EvNote>() {
                                    if let Some(ctrl) = mcu_note_to_event(n, ev.get_type() == EventType::Noteon) {
                                        let _ = tx.send(ctrl);
                                    }
                                }
                            }
                            EventType::Controller | EventType::Pitchbend => {
                                if let Some(c) = ev.get_data::<EvCtrl>() {
                                    if let Some(ctrl) = mcu_cc_to_event(c, ev.get_type() == EventType::Pitchbend) {
                                        let _ = tx.send(ctrl);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => thread::sleep(Duration::from_millis(4)),
                }
                refresh += 1;
                if refresh >= 250 {
                    refresh = 0;
                    if let Ok(mut d) = devices_t.lock() {
                        *d = Self::list_devices(&seq);
                    }
                }
            }
        });

        Ok(Self {
            event_count,
            devices,
            running,
            join: Some(join),
        })
    }

    fn list_devices(seq: &alsa::seq::Seq) -> Vec<String> {
        use alsa::seq::{ClientIter, PortCap, PortIter};
        let own = seq.client_id().unwrap_or(-1);
        let mut out = Vec::new();
        for client in ClientIter::new(seq) {
            let cid = client.get_client();
            if cid == own {
                continue;
            }
            for port in PortIter::new(seq, cid) {
                let caps = port.get_capability();
                let can_read = caps.contains(PortCap::READ) || caps.contains(PortCap::SUBS_READ);
                if can_read {
                    out.push(format!("{}:{} {}", cid, port.get_port(), port.get_name().unwrap_or("?")));
                }
            }
        }
        out
    }

    pub fn received(&self) -> usize {
        self.event_count.load(Ordering::Relaxed)
    }

    pub fn device_list(&self) -> Vec<String> {
        self.devices.lock().map(|d| d.clone()).unwrap_or_default()
    }
}

impl Drop for McuInput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn mcu_note_to_event(n: alsa::seq::EvNote, is_on: bool) -> Option<ControlEvent> {
    match n.note {
        94 => Some(ControlEvent::Play(is_on)),
        93 if is_on => Some(ControlEvent::Stop),
        95 => Some(ControlEvent::Record(is_on)),
        16..=23 => Some(ControlEvent::TrackMute {
            track: (n.note - 16) as usize,
            mute: is_on,
        }),
        _ => None,
    }
}

fn mcu_cc_to_event(c: alsa::seq::EvCtrl, is_bend: bool) -> Option<ControlEvent> {
    if is_bend {
        // MCU motor faders: 14-bit pitch bend on channels 0..7 -> track volume.
        let value = (c.value.clamp(0, 16383) as f32) / 16383.0 * 1.25;
        Some(ControlEvent::TrackVolume {
            track: c.channel as usize,
            value,
        })
    } else {
        match c.param {
            16..=23 => Some(ControlEvent::VcaVolume {
                vca: (c.param - 16) as usize,
                value: (c.value.clamp(0, 127) as f32) / 127.0 * 1.25,
            }),
            _ => None,
        }
    }
}

/// Builds a `(channel, sender)` pair for wiring controller input into the app.
pub fn control_channel() -> (Sender<ControlEvent>, Receiver<ControlEvent>) {
    mpsc::channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osc(addr: &str, tags: &str, args: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(addr.as_bytes());
        v.push(0);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v.extend_from_slice(tags.as_bytes());
        v.push(0);
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v.extend_from_slice(args);
        v
    }

    #[test]
    fn parses_osc_float_message() {
        let data = osc("/sonix/track/3/volume", ",f", &0.75f32.to_be_bytes());
        let (addr, args) = parse_osc_message(&data).unwrap();
        assert_eq!(addr, "/sonix/track/3/volume");
        assert_eq!(args, vec![OscArg::Float(0.75)]);
        assert_eq!(
            osc_to_event(&addr, &args),
            Some(ControlEvent::TrackVolume { track: 2, value: 0.75 })
        );
    }

    #[test]
    fn parses_osc_int_and_maps_transport() {
        let data = osc("/sonix/transport/play", ",i", &1i32.to_be_bytes());
        let (addr, args) = parse_osc_message(&data).unwrap();
        assert_eq!(osc_to_event(&addr, &args), Some(ControlEvent::Play(true)));
    }

    #[test]
    fn parses_osc_bundle() {
        let inner = osc("/sonix/bus/synth/volume", ",f", &0.5f32.to_be_bytes());
        let mut bundle = Vec::new();
        bundle.extend_from_slice(b"#bundle\0");
        bundle.extend_from_slice(&[0u8; 8]);
        bundle.extend_from_slice(&(inner.len() as i32).to_be_bytes());
        bundle.extend_from_slice(&inner);
        let (addr, args) = parse_osc_message(&bundle).unwrap();
        assert_eq!(
            osc_to_event(&addr, &args),
            Some(ControlEvent::BusVolume { bus: 2, value: 0.5 })
        );
    }

    #[test]
    fn unknown_osc_address_is_ignored() {
        assert_eq!(osc_to_event("/foo/bar", &[]), None);
    }

    #[test]
    fn osc_server_receives_real_udp_packet() {
        use std::time::Instant;
        let (tx, rx) = control_channel();
        let server = OscServer::start(0, tx).expect("bind osc server");
        let target = format!("127.0.0.1:{}", server.bound_port);
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let pkt = osc("/sonix/transport/play", ",i", &1i32.to_be_bytes());
        sender.send_to(&pkt, &target).unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut got = None;
        while Instant::now() < deadline {
            if let Ok(ev) = rx.try_recv() {
                got = Some(ev);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(got, Some(ControlEvent::Play(true)));
        assert!(server.received() >= 1);
    }
}
