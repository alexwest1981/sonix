//! **MIDI-klaviatur in — en väg, `midir`, på alla plattformar** (Fas 7.1).
//!
//! Modulen hade två implementationer: ALSA-sequencern på Linux, `midir` utanför. Bara den
//! första användes i praktiken, och skillnaden var inte bara teknisk:
//!
//! - ALSA-vägen **skapade en port** ("Sonix MIDI In") som användaren själv måste koppla sin
//!   klaviatur till — `aconnect` eller en patchbay, och instruktionen stod i gränssnittet.
//! - `midir`-vägen **ansluter till varje in-port** som finns. Klaviaturen fungerar utan ett
//!   enda handgrepp.
//!
//! Den andra vägen låg färdig och väntade på Windows. Nu finns **en** väg och den används
//! överallt: på Linux är `midir` fortfarande ALSA-sequencern under huven, så inget byts ut mot
//! något sämre — det som försvinner är en andra implementation att hålla levande, `cfg`-grenar
//! som bara en plattform kunde pröva, och en instruktion som bara var sann för den ena.
//!
//! `control_events_from_midi` är ren och prövad: den tolkar **bytes**, inte plattformar.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

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

/// MIDI-klaviatur in över `midir`.
///
/// Varje in-port som hittas öppnas och kopplas till en egen anslutning, och varje note-on/off
/// skickas vidare som [`ControlEvent::MidiNote`] så att appen kan spela och spela in den.
/// Att inga portar finns är **inte** ett fel: klaviaturen kan kopplas in senare, och
/// `device_list` frågar efter den aktuella listan varje gång.
pub struct MidiKeyboardInput {
    pub event_count: Arc<AtomicUsize>,
    pub devices: Arc<Mutex<Vec<String>>>,
    /// Anslutningarna måste hållas vid liv så länge vi vill ta emot något: att släppa dem
    /// kopplar ner. Fältet **läses aldrig** — det finns för sin livstid, inte för sitt värde.
    /// Det ska stå uttryckligt, annars varnar bygget för död kod. Den varningen fångades av
    /// CI:s Windows-jobb när vägen bara kompilerades där: Linux såg den aldrig.
    #[allow(dead_code, reason = "bär anslutningarna för sin livstid; att släppa dem kopplar ner")]
    connections: Vec<midir::MidiInputConnection<()>>,
}

/// Tolkar en MIDI-messages bytes som appens kontrollhändelser.
///
/// Ren funktion utan plattformsberoenden. Noter och pitch bend blir händelser: CC,
/// aftertouch, programbyte och systemmeddelanden (0xF0 och uppåt, som har olika längd) lämnas.
/// `velocity == 0` på note-on betyder note-off — vanligt bland klaviaturer.
///
/// Pitch bend räknas om till halvtoner direkt: kanal 0 (MPE-mastern) ±2, medlemskanalerna
/// 1–15 ±48. Att göra det här i stället för i motorn håller `ControlEvent` ren och gör
/// omräkningen prövbar utan ljudenhet.
pub fn control_events_from_midi(bytes: &[u8]) -> Vec<ControlEvent> {
    let Some(&status) = bytes.first() else {
        return Vec::new();
    };
    // Systemmeddelanden (0xF0 och uppåt) bär ingen not och har olika längd.
    if status >= 0xF0 {
        return Vec::new();
    }
    let kind = status & 0xF0;
    let channel = status & 0x0F;
    if kind == 0xE0 {
        let (Some(&lsb), Some(&msb)) = (bytes.get(1), bytes.get(2)) else {
            return Vec::new();
        };
        // 14 bitar, mitten 8192 = ingen böjning.
        let raw = (((msb & 0x7F) as i32) << 7) | (lsb & 0x7F) as i32;
        let norm = (raw - 8192) as f32 / 8192.0;
        let range = if channel == 0 { 2.0 } else { 48.0 };
        return vec![ControlEvent::PitchBend {
            channel,
            semitones: norm * range,
        }];
    }
    if kind == 0xB0 {
        // **CC64 = sustainpedal.** Tröskeln 64 är MIDI-standard: under är upp.
        let (Some(&cc), Some(&value)) = (bytes.get(1), bytes.get(2)) else {
            return Vec::new();
        };
        if cc == 64 {
            return vec![ControlEvent::SustainPedal(value >= 64)];
        }
        return Vec::new();
    }
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
        channel,
    }]
}

/// Är porten en **slinga** i stället för ett instrument?
///
/// ALSA:s `Midi Through` ekar allt som skrivs till den rakt ut ur samma port igen. Appens egen
/// `--selftest` skickar sin testnot till **första** ut-porten, och på Linux är det Midi Through —
/// alltså hörde en pågående Sonix sin egen testnot och spelade den som ett ljud ur tomma luften.
/// Mätt 2026-10-27: `aplaymidi -p 14:0` kom ut ur `14:0` igen, och `14:0` var kopplad till appens
/// in-port (`aconnect -l`). En andra Sonix-instans (`Sonix Keys`) är samma sorts slinga.
pub fn is_loopback_port(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("midi through") || n.contains("sonix keys")
}

impl MidiKeyboardInput {
    /// Öppnar alla MIDI-in-portar som hittas och kopplar var och en till en egen anslutning.
    /// Varje anslutning får sin egen `MidiInput`, eftersom `connect` tar över instansen.
    ///
    /// Att inga portar finns är **inte** ett fel: klaviaturen kan kopplas in senare, och då
    /// hittas den av `device_list`. Bara ett fel som hindrar själva starten rapporteras.
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
            // En slinga är inget instrument: att lyssna på den gör att appen spelar upp noter
            // som ingen spelade (se `is_loopback_port`).
            if is_loopback_port(&name) {
                continue;
            }
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

        // Att ingen port kunde öppnas är ett fel värt att visa; att det inte fanns någon port
        // alls är det inte.
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

    /// Listan hämtas färsk varje gång: `midir` ser aktuella portar när en ny `MidiInput`
    /// skapas, så en klaviatur som kopplas in mitt i en session dyker upp utan omstart.
    /// Går det inte att fråga behålls den senaste listan.
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

fn port_names(input: &midir::MidiInput) -> Vec<String> {
    input
        .ports()
        .iter()
        .map(|p| input.port_name(p).unwrap_or_else(|_| "?".to_string()))
        .collect()
}

#[cfg(test)]
mod midir_parser_tests {
    use super::*;

    fn note(note: u8, velocity: u8, on: bool) -> ControlEvent {
        ControlEvent::MidiNote {
            note,
            velocity,
            on,
            channel: 0,
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
                vec![ControlEvent::MidiNote {
                    note: 48,
                    velocity: 90,
                    on: true,
                    channel: ch
                }],
                "kanal {ch} tappades"
            );
        }
    }

    #[test]
    fn the_master_bends_two_semitones_and_members_forty_eight() {
        // Mitten = ingen böjning.
        assert_eq!(
            control_events_from_midi(&[0xE0, 0, 64]),
            vec![ControlEvent::PitchBend {
                channel: 0,
                semitones: 0.0
            }]
        );
        // Full upp på mastern (kanal 0): +2 halvtoner.
        let up = control_events_from_midi(&[0xE0, 0x7F, 0x7F]);
        let ControlEvent::PitchBend { channel, semitones } = up[0] else {
            panic!("inte en pitch bend")
        };
        assert_eq!(channel, 0);
        assert!((semitones - 2.0).abs() < 0.01, "{semitones}");
        // Full upp på medlemskanal 1: +48.
        let up = control_events_from_midi(&[0xE1, 0x7F, 0x7F]);
        let ControlEvent::PitchBend { channel, semitones } = up[0] else {
            panic!("inte en pitch bend")
        };
        assert_eq!(channel, 1);
        assert!((semitones - 48.0).abs() < 0.1, "{semitones}");
        // Full ner ger minus.
        let down = control_events_from_midi(&[0xE2, 0x00, 0x00]);
        let ControlEvent::PitchBend { semitones, .. } = down[0] else {
            panic!("inte en pitch bend")
        };
        assert!(semitones < -47.9, "{semitones}");
    }

    /// **En slinga är inget instrument.** `Midi Through` ekar tillbaka allt som skrivs till den,
    /// och appens `--selftest` skickar sin testnot till första ut-porten — utan spärren spelade
    /// en pågående Sonix upp en not som ingen spelade (mätt 2026-10-27).
    #[test]
    fn loopback_ports_are_not_keyboards() {
        assert!(is_loopback_port("Midi Through Port-0"));
        assert!(is_loopback_port("14:Midi Through:Midi Through Port-0"));
        assert!(is_loopback_port("Sonix Keys:sonix-keys"), "en andra instans är samma slinga");
        assert!(!is_loopback_port("Arturia KeyStep 37"));
        assert!(!is_loopback_port("Roland Digital Piano"));
    }

    #[test]
    fn messages_that_are_not_notes_or_bends_are_ignored() {
        // CC, aftertouch, programbyte och systemmeddelanden.
        assert!(control_events_from_midi(&[0xB0, 7, 100]).is_empty());
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
