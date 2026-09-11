#[cfg(target_os = "linux")]
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
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
    running: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

#[cfg(not(target_os = "linux"))]
impl MidiKeyboardInput {
    /// På andra plattformar än Linux finns ingen ALSA-sequencer. Stubben svarar
    /// med ett begripligt fel i stället för att kratet inte ska gå att bygga —
    /// porten till `midir` är en egen uppgift i Fas 7.1.
    pub fn connect(_tx: Sender<ControlEvent>) -> Result<Self, String> {
        Err(
            "MIDI-klaviatur kräver ALSA-sequencern, som bara finns på Linux i den här versionen"
                .to_string(),
        )
    }

    pub fn received(&self) -> usize {
        0
    }

    pub fn device_list(&self) -> Vec<String> {
        Vec::new()
    }
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

impl Drop for MidiKeyboardInput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
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
