#[cfg(target_os = "linux")]
use std::ffi::CString;
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(target_os = "linux")]
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::thread::JoinHandle;
// Tråden och dess sömn hör till ALSA-läsaren: på andra plattformar finns ingen
// tråd, så importerna ska inte ligga där heller (annars varnar bygget).
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

use super::hardware_control::ControlEvent;

/// Maps a MIDI note number into a Piano Roll row (`0` = lowest visible key).
///
/// The visible grid only spans `rows` semitones, so notes outside the window
/// are transposed by whole octaves to the closest in-range row. This keeps the
/// played pitch class intact while staying inside the editor.
pub fn note_to_roll_offset(note: u8, base: u8, rows: usize) -> Option<usize> {
    if rows == 0 {
        return None;
    }
    let rows_i = rows as i32;
    let rel = note as i32 - base as i32;
    let pc = rel.rem_euclid(12);
    let mut best: Option<i32> = None;
    for k in 0..=(rows_i / 12 + 1) {
        let candidate = pc + 12 * k;
        if candidate >= 0 && candidate < rows_i {
            match best {
                Some(b) if (b - rel).abs() <= (candidate - rel).abs() => {}
                _ => best = Some(candidate),
            }
        }
    }
    match best {
        Some(c) => Some(c as usize),
        // Grid shorter than an octave: clamp the pitch class into range.
        None => Some(pc.min(rows_i - 1) as usize),
    }
}

/// Real MIDI keyboard input over the ALSA sequencer.
///
/// Opens an input port named "Sonix MIDI In". External keyboards connect to it
/// (e.g. with `aconnect`), and every note on/off is forwarded as a
/// [`ControlEvent::MidiNote`] so the app can play and record it.
pub struct MidiKeyboardInput {
    pub event_count: Arc<AtomicUsize>,
    pub devices: Arc<Mutex<Vec<String>>>,
    /// Stoppflaggan hör till lästråden, som bara finns i ALSA-vägen.
    #[cfg(target_os = "linux")]
    running: Arc<AtomicBool>,
    /// ALSA-vägen läser i en egen tråd …
    #[cfg(target_os = "linux")]
    join: Option<JoinHandle<()>>,
    /// … medan midir anropar tillbaka från sin egen kö. Anslutningarna måste
    /// hållas vid liv så länge vi vill ta emot något: att släppa dem kopplar ner.
    #[cfg(not(target_os = "linux"))]
    connections: Vec<midir::MidiInputConnection<()>>,
}

/// Tolkar en MIDI-messages bytes som appens kontrollhändelser.
///
/// Ren funktion utan plattformsberoenden, så att den kan prövas i CI på Linux
/// även om den bara anropas av midir-vägen. Bara noter blir händelser — samma
/// mappning som ALSA-läsaren gör (`velocity == 0` på note-on betyder note-off).
#[cfg_attr(
    target_os = "linux",
    allow(dead_code, reason = "anropas av midir-backenden; Linux-läsaren får färdigtolkade ALSA-händelser")
)]
pub fn control_events_from_midi(bytes: &[u8]) -> Vec<ControlEvent> {
    let Some(&status) = bytes.first() else {
        return Vec::new();
    };
    // Systemmeddelanden (0xF0 och uppåt) bär ingen not och har olika längd.
    if status >= 0xF0 {
        return Vec::new();
    }
    let kind = status & 0xF0;
    if kind != 0x80 && kind != 0x90 {
        return Vec::new();
    }
    let (Some(&note), Some(&velocity)) = (bytes.get(1), bytes.get(2)) else {
        return Vec::new();
    };
    // Note-on med velocity 0 är note-off (vanligt bland klaviaturer).
    let on = kind == 0x90 && velocity > 0;
    vec![ControlEvent::MidiNote {
        note: note & 0x7F,
        velocity: velocity & 0x7F,
        on,
    }]
}

#[cfg(not(target_os = "linux"))]
impl MidiKeyboardInput {
    /// Öppnar alla MIDI-in-portar midir hittar och kopplar var och en till en
    /// egen anslutning. Varje anslutning får sin egen `MidiInput`, eftersom
    /// `connect` tar över instansen.
    ///
    /// Att inga portar finns är **inte** ett fel: klaviaturen kan kopplas in
    /// senare, och då hittas den av `device_list`. Bara ett fel som hindrar
    /// själva starten rapporteras.
    pub fn connect(tx: Sender<ControlEvent>) -> Result<Self, String> {
        let event_count = Arc::new(AtomicUsize::new(0));
        let probe = midir::MidiInput::new("Sonix Keys")
            .map_err(|e| format!("Kunde inte starta MIDI-in: {}", e))?;
        let devices = Arc::new(Mutex::new(port_names(&probe)));
        let ports = probe.ports();
        drop(probe);

        let mut connections = Vec::new();
        let mut failures = Vec::new();
        for port in ports {
            let Ok(input) = midir::MidiInput::new("Sonix Keys") else {
                continue;
            };
            let name = input.port_name(&port).unwrap_or_else(|_| "?".to_string());
            let count_t = event_count.clone();
            let tx_t = tx.clone();
            match input.connect(
                &port,
                "sonix-keys",
                move |_when, bytes, _| {
                    for ev in control_events_from_midi(bytes) {
                        count_t.fetch_add(1, Ordering::Relaxed);
                        let _ = tx_t.send(ev);
                    }
                },
                (),
            ) {
                Ok(conn) => connections.push(conn),
                Err(e) => failures.push(format!("{}: {}", name, e)),
            }
        }

        // Att ingen port kunde öppnas är ett fel värt att visa; att det inte
        // fanns någon port alls är det inte.
        if connections.is_empty() && !failures.is_empty() {
            return Err(format!("Kunde inte öppna MIDI-in: {}", failures.join(", ")));
        }

        Ok(Self {
            event_count,
            devices,
            connections,
        })
    }

    pub fn received(&self) -> usize {
        self.event_count.load(Ordering::Relaxed)
    }

    /// Listan hämtas färsk varje gång: midir ser aktuella portar när en ny
    /// `MidiInput` skapas, så en klaviatur som kopplas in mitt i en session
    /// dyker upp utan omstart. Går det inte att fråga behålls den senaste listan.
    pub fn device_list(&self) -> Vec<String> {
        match midir::MidiInput::new("Sonix Keys") {
            Ok(input) => port_names(&input),
            Err(_) => self
                .devices
                .lock()
                .map(|d| d.clone())
                .unwrap_or_default(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn port_names(input: &midir::MidiInput) -> Vec<String> {
    input
        .ports()
        .iter()
        .map(|p| input.port_name(p).unwrap_or_else(|_| "?".to_string()))
        .collect()
}

#[cfg(target_os = "linux")]
impl MidiKeyboardInput {
    pub fn connect(tx: Sender<ControlEvent>) -> Result<Self, String> {
        use alsa::seq::{PortCap, PortType, Seq};

        let seq = Seq::open(None, Some(alsa::Direction::Capture), true)
            .map_err(|e| format!("Kunde inte öppna ALSA-sequencer: {}", e))?;
        seq.set_client_name(&CString::new("Sonix Keys").unwrap())
            .map_err(|e| e.to_string())?;
        seq.create_simple_port(
            &CString::new("Sonix MIDI In").unwrap(),
            PortCap::WRITE | PortCap::SUBS_WRITE,
            PortType::MIDI_GENERIC | PortType::APPLICATION,
        )
        .map_err(|e| format!("Kunde inte skapa MIDI-in-port: {}", e))?;

        let devices = Arc::new(Mutex::new(Self::list_devices(&seq)));
        let event_count = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let running_t = running.clone();
        let count_t = event_count.clone();
        let devices_t = devices.clone();
        let join = thread::spawn(move || {
            use alsa::seq::{EventType, EvNote};
            let mut input = seq.input();
            let mut refresh = 0u32;
            while running_t.load(Ordering::Relaxed) {
                match input.event_input() {
                    Ok(ev) => {
                        let etype = ev.get_type();
                        if (etype == EventType::Noteon || etype == EventType::Noteoff)
                            && let Some(n) = ev.get_data::<EvNote>()
                        {
                            let on = etype == EventType::Noteon && n.velocity > 0;
                            count_t.fetch_add(1, Ordering::Relaxed);
                            let _ = tx.send(ControlEvent::MidiNote {
                                note: n.note,
                                velocity: n.velocity,
                                on,
                            });
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
                    out.push(format!(
                        "{}:{} {}",
                        cid,
                        port.get_port(),
                        port.get_name().unwrap_or("?")
                    ));
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

#[cfg(target_os = "linux")]
impl Drop for MidiKeyboardInput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[cfg(test)]
mod midir_parser_tests {
    use super::*;

    fn note(note: u8, velocity: u8, on: bool) -> ControlEvent {
        ControlEvent::MidiNote {
            note,
            velocity,
            on,
        }
    }

    #[test]
    fn a_note_on_and_a_note_off_become_note_events() {
        assert_eq!(control_events_from_midi(&[0x90, 60, 100]), vec![note(60, 100, true)]);
        assert_eq!(control_events_from_midi(&[0x80, 60, 0]), vec![note(60, 0, false)]);
    }

    #[test]
    fn note_on_with_velocity_zero_counts_as_note_off() {
        // Vanligt bland klaviaturer: samma statusbyte, velocity 0 släpper noten.
        assert_eq!(control_events_from_midi(&[0x90, 64, 0]), vec![note(64, 0, false)]);
    }

    #[test]
    fn every_channel_is_read() {
        // Statusbytet bär kanalen i de fyra låga bitarna (0x9n = note-on, kanal n).
        for ch in 0..16u8 {
            assert_eq!(
                control_events_from_midi(&[0x90 | ch, 48, 90]),
                vec![note(48, 90, true)],
                "kanal {ch} tappades"
            );
        }
    }

    #[test]
    fn messages_that_are_not_notes_are_ignored() {
        // CC, pitch bend, aftertouch, programbyte och systemmeddelanden.
        assert!(control_events_from_midi(&[0xB0, 7, 100]).is_empty());
        assert!(control_events_from_midi(&[0xE0, 0, 64]).is_empty());
        assert!(control_events_from_midi(&[0xD0, 40]).is_empty());
        assert!(control_events_from_midi(&[0xC0, 5]).is_empty());
        assert!(control_events_from_midi(&[0xF8]).is_empty());
        assert!(control_events_from_midi(&[0xF0, 0x7E, 0x7F]).is_empty());
    }

    #[test]
    fn short_or_empty_messages_do_not_panic() {
        assert!(control_events_from_midi(&[]).is_empty());
        assert!(control_events_from_midi(&[0x90]).is_empty());
        assert!(control_events_from_midi(&[0x90, 60]).is_empty());
        assert!(control_events_from_midi(&[0x80, 60]).is_empty());
    }

    #[test]
    fn data_bytes_are_masked_to_seven_bits() {
        // Skräp i de höga bitarna ska inte läcka in i notnumret.
        assert_eq!(
            control_events_from_midi(&[0x90, 0xFF, 0xFF]),
            vec![note(0x7F, 0x7F, true)]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_notes_inside_grid_unchanged() {
        assert_eq!(note_to_roll_offset(48, 48, 24), Some(0));
        assert_eq!(note_to_roll_offset(60, 48, 24), Some(12));
        assert_eq!(note_to_roll_offset(71, 48, 24), Some(23));
    }

    #[test]
    fn transposes_notes_by_octaves_into_grid() {
        // C2 (36) is two octaves below the C3 base -> folded to the same pitch class.
        assert_eq!(note_to_roll_offset(36, 48, 24), Some(0));
        // C5 (72) is one semitone above the top (71) -> folded down an octave.
        assert_eq!(note_to_roll_offset(72, 48, 24), Some(12));
        // A5 (81) -> pitch class A, closest in-range row is A4 (offset 21).
        assert_eq!(note_to_roll_offset(81, 48, 24), Some(21));
    }

    #[test]
    fn short_grid_clamps_pitch_class() {
        assert_eq!(note_to_roll_offset(70, 48, 5), Some(4));
        assert_eq!(note_to_roll_offset(48, 48, 5), Some(0));
    }

    #[test]
    fn empty_grid_returns_none() {
        assert_eq!(note_to_roll_offset(60, 48, 0), None);
    }
}
