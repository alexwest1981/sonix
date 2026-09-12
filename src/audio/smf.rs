//! Standard MIDI File (SMF) — skriv och läs `.mid` utan externa beroenden.
//!
//! Varför egen kod: formatet är litet och väldefinierat (SMF 1.0), repot har
//! redan samma hållning för WAV, loudness och FLAC, och en import måste kunna
//! läsa filer som *andra* DAW:er skrivit — inklusive running status och
//! okända meta-events — utan att panika på dem.
//!
//! Modulen är ren data in och data ut (inga app- eller ljudberoenden), så hela
//! kodningen kan testas utan GUI.

/// Pulser per fjärdedelsnot. 480 är den vanligaste upplösningen och delbart
/// med både 16-delar (120), 32-delar (60) och tripletter (160).
pub const PPQ: u16 = 480;

/// Raka 16-delar i pulser.
pub const TICKS_PER_STEP_16TH: u32 = (PPQ as u32) / 4;

/// En not med absolut start och längd i pulser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiNote {
    pub start: u32,
    pub length: u32,
    /// MIDI-kanal 0–15 (9 = percussion enligt SMF-konventionen).
    pub channel: u8,
    /// Tonhöjd 0–127.
    pub key: u8,
    pub velocity: u8,
}

/// Ett spår i filen: namn + noter. Tomma spår skrivs ändå, så spårnumret i
/// filen motsvarar spåret i projektet.
#[derive(Clone, Debug, Default)]
pub struct MidiTrack {
    pub name: String,
    pub notes: Vec<MidiNote>,
}

/// Läser en variabel längdkodning (VLQ): 7 bitar per byte, högsta biten satt
/// betyder "fortsätter". Max fyra byte enligt specifikationen.
fn write_vlq(out: &mut Vec<u8>, mut value: u32) {
    let mut buf = [0u8; 4];
    let mut n = 0;
    loop {
        buf[n] = (value & 0x7f) as u8;
        value >>= 7;
        n += 1;
        if value == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        let last = i == 0;
        out.push(if last { buf[i] } else { buf[i] | 0x80 });
    }
}

fn read_vlq(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    let mut value: u32 = 0;
    for _ in 0..4 {
        let b = *bytes
            .get(*pos)
            .ok_or("filen tar slut mitt i en längdkodning")?;
        *pos += 1;
        value = (value << 7) | (b & 0x7f) as u32;
        if b & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("längdkodning längre än fyra byte".to_string())
}

fn write_track_chunk(out: &mut Vec<u8>, track: &MidiTrack) {
    let mut data: Vec<u8> = Vec::new();

    // Spårnamn (meta 0x03).
    if !track.name.is_empty() {
        data.push(0x00);
        data.push(0xff);
        data.push(0x03);
        write_vlq(&mut data, track.name.len() as u32);
        data.extend_from_slice(track.name.as_bytes());
    }

    // Händelser i tidsordning. Vid samma tick skrivs not-off före not-on, så
    // en ny ton på samma tangent inte kan klippas av föregående not-off.
    #[derive(Clone, Copy)]
    enum Ev {
        On(usize),
        Off(usize),
    }
    let mut events: Vec<(u32, u8, Ev)> = Vec::with_capacity(track.notes.len() * 2);
    for (i, n) in track.notes.iter().enumerate() {
        let key = n.key & 0x7f;
        events.push((n.start, key, Ev::On(i)));
        events.push((n.start + n.length.max(1), key, Ev::Off(i)));
    }
    events.sort_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| match (a.2, b.2) {
            (Ev::Off(_), Ev::On(_)) => std::cmp::Ordering::Less,
            (Ev::On(_), Ev::Off(_)) => std::cmp::Ordering::Greater,
            _ => a.1.cmp(&b.1),
        })
    });

    let mut last_tick = 0u32;
    for (tick, _, ev) in events {
        write_vlq(&mut data, tick.saturating_sub(last_tick));
        last_tick = tick;
        match ev {
            Ev::On(i) => {
                let n = &track.notes[i];
                data.push(0x90 | (n.channel & 0x0f));
                data.push(n.key & 0x7f);
                data.push(n.velocity.clamp(1, 127));
            }
            Ev::Off(i) => {
                let n = &track.notes[i];
                data.push(0x80 | (n.channel & 0x0f));
                data.push(n.key & 0x7f);
                data.push(0x40);
            }
        }
    }

    // End of track (FF 2F 00).
    write_vlq(&mut data, 0);
    data.push(0xff);
    data.push(0x2f);
    data.push(0x00);

    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&data);
}

/// Skriver en format 1-fil: första spåret är tempot, därefter ett spår per
/// `MidiTrack`, med ett enda tempo.
///
/// Enkel-tempovarianten. Appen skriver via `write_midi_with_tempo` (den går
/// genom tempokartan), men den här är den form de flesta anrop vill ha och
/// testerna använder den.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "appen skriver via write_midi_with_tempo; testerna använder denna"
    )
)]
pub fn write_midi(bpm: f32, tracks: &[MidiTrack]) -> Vec<u8> {
    write_midi_with_tempo(&[(0.0, bpm)], tracks)
}

/// Skriver en format 1-fil med tempobyten: varje punkt `(takt, BPM)` blir en
/// egen händelse i ledspåret, med avståndet i tick till nästa.
///
/// Punkterna kommer in som tal i stället för som en `TempoMap`, av samma skäl
/// som modulen inte har några andra beroenden: kodeken ska kunna läsas för sig
/// själv. Den som har en karta projicerar den hit — det är ett ställe.
///
/// Med en enda punkt blir filen **bitvis** identisk med den gamla skrivaren, och
/// det bevisas av ett test som jämför byte för byte. En tempokarta ska inte
/// ändra en enda fil som inte har några byten.
pub fn write_midi_with_tempo(points: &[(f64, f32)], tracks: &[MidiTrack]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // format 1
    out.extend_from_slice(&((tracks.len() + 1) as u16).to_be_bytes());
    out.extend_from_slice(&PPQ.to_be_bytes());

    // Ledspår med tempo och taktart. Första tempot först, därefter taktarten
    // (båda på takt 0), och sedan resten av tempopunkterna i stigande ordning —
    // så står de i en sequencer, och så blir en fil utan byten oförändrad.
    let mut conductor: Vec<u8> = Vec::new();
    let mut last_tick: u32 = 0;
    if let Some((_, bpm)) = points.first() {
        push_tempo_event(&mut conductor, 0, *bpm);
    }
    conductor.push(0x00);
    conductor.push(0xff);
    conductor.push(0x58);
    conductor.push(0x04);
    conductor.extend_from_slice(&[4, 2, 24, 8]); // 4/4, 24 klockor, 8 32-delar
    for (bar, bpm) in points.iter().skip(1) {
        let tick = (bar * PPQ as f64 * 4.0).round().max(0.0) as u32;
        push_tempo_event(&mut conductor, tick.saturating_sub(last_tick), *bpm);
        last_tick = tick;
    }
    conductor.push(0x00);
    conductor.push(0xff);
    conductor.push(0x2f);
    conductor.push(0x00);
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(conductor.len() as u32).to_be_bytes());
    out.extend_from_slice(&conductor);

    for track in tracks {
        write_track_chunk(&mut out, track);
    }
    out
}

/// Skriver en tempo-händelse (FF 51 03) med `delta` tick före sig.
///
/// Tempot klampar till 20–300 BPM, precis som den gamla skrivaren gjorde: en
/// fil med ett orimligt tempo ska bli en fil, inte ett fel.
fn push_tempo_event(out: &mut Vec<u8>, delta: u32, bpm: f32) {
    write_vlq(out, delta);
    out.push(0xff);
    out.push(0x51);
    out.push(0x03);
    let us_per_quarter = (60_000_000.0 / bpm.clamp(20.0, 300.0)).round() as u32;
    out.extend_from_slice(&us_per_quarter.to_be_bytes()[1..4]);
}

/// Ett läst spår.
#[derive(Clone, Debug, Default)]
pub struct ParsedTrack {
    pub name: Option<String>,
    pub notes: Vec<MidiNote>,
}

/// En läst fil.
#[derive(Clone, Debug)]
pub struct ParsedMidi {
    pub format: u16,
    pub ppq: u16,
    pub bpm: Option<f32>,
    /// Alla tempohändelser i filen som (takt, BPM), i den ordning de står.
    ///
    /// `bpm` är den första av dem — det är den som gäller från början. Med en
    /// tempokarta i filen räcker inte ett enda tal, och då behövs de här.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "läses av importen när den tar med tempobyten (8.2 steg 3)"
        )
    )]
    pub tempo_events: Vec<(f64, f32)>,
    pub tracks: Vec<ParsedTrack>,
}

impl ParsedMidi {
    /// Alla noter i filen, med spårnummer.
    pub fn notes_with_track(&self) -> Vec<(usize, &MidiNote)> {
        self.tracks
            .iter()
            .enumerate()
            .flat_map(|(i, t)| t.notes.iter().map(move |n| (i, n)))
            .collect()
    }
}

/// Läser en `.mid`-fil. Ger ett begripligt fel i stället för panik för allt som
/// inte är en SMF-fil — en import ska aldrig kunna fälla appen.
pub fn parse_midi(bytes: &[u8]) -> Result<ParsedMidi, String> {
    if bytes.len() < 14 || &bytes[0..4] != b"MThd" {
        return Err("inte en MIDI-fil (MThd saknas)".to_string());
    }
    let header_len = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if header_len < 6 {
        return Err("skadad MIDI-header".to_string());
    }
    let format = u16::from_be_bytes([bytes[8], bytes[9]]);
    if format > 2 {
        return Err(format!("okänd MIDI-formattyp {format}"));
    }
    let ntrks = u16::from_be_bytes([bytes[10], bytes[11]]) as usize;
    let division = u16::from_be_bytes([bytes[12], bytes[13]]);
    if division & 0x8000 != 0 {
        return Err("SMPTE-tidsbas (bildrutor) stöds inte".to_string());
    }
    let ppq = division.max(1);

    let mut pos = 8 + header_len;
    let mut tracks: Vec<ParsedTrack> = Vec::new();
    let mut bpm: Option<f32> = None;
    let mut tempo_events: Vec<(f64, f32)> = Vec::new();

    for _ in 0..ntrks {
        if pos + 8 > bytes.len() {
            break; // Färre spår än rubriken påstår: läs det som finns.
        }
        if &bytes[pos..pos + 4] != b"MTrk" {
            return Err("förväntade ett MTrk-block".to_string());
        }
        let len = u32::from_be_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        pos += 8;
        let end = (pos + len).min(bytes.len());

        let mut track = ParsedTrack::default();
        // Öppna noter per (kanal, tonhöjd) så att note-off kan paras ihop.
        let mut open: Vec<((u8, u8), (u32, u8))> = Vec::new();
        let mut tick: u32 = 0;
        let mut running: Option<u8> = None;

        while pos < end {
            let delta = read_vlq(&bytes[..end], &mut pos)?;
            tick = tick.saturating_add(delta);
            let mut status = *bytes.get(pos).ok_or("filen tar slut mitt i en händelse")?;
            if status < 0x80 {
                status = running.ok_or("händelse utan statusbyte")?;
            } else {
                pos += 1;
                if status < 0xf0 {
                    running = Some(status);
                } else {
                    running = None;
                }
            }

            match status & 0xf0 {
                0x80 | 0x90 => {
                    let key = *bytes.get(pos).ok_or("nothändelse utan tonhöjd")?;
                    let vel = *bytes.get(pos + 1).ok_or("nothändelse utan styrka")?;
                    pos += 2;
                    let channel = status & 0x0f;
                    let is_off = status & 0xf0 == 0x80 || vel == 0;
                    if is_off {
                        if let Some(idx) = open.iter().position(|(k, _)| *k == (channel, key)) {
                            let ((_, _), (start, start_vel)) = open.remove(idx);
                            track.notes.push(MidiNote {
                                start,
                                length: tick.saturating_sub(start),
                                channel,
                                key,
                                velocity: start_vel,
                            });
                        }
                    } else {
                        open.push(((channel, key), (tick, vel)));
                    }
                }
                0xa0 | 0xb0 | 0xe0 => pos += 2,
                0xc0 | 0xd0 => pos += 1,
                0xf0 => match status {
                    0xff => {
                        let kind = *bytes.get(pos).ok_or("meta-händelse utan typ")?;
                        pos += 1;
                        let len = read_vlq(&bytes[..end], &mut pos)? as usize;
                        let data = bytes.get(pos..pos + len).ok_or("meta-händelse utan data")?;
                        match kind {
                            0x03 => {
                                track.name = String::from_utf8(data.to_vec()).ok();
                            }
                            0x51 if len >= 3 => {
                                let us = ((data[0] as u32) << 16)
                                    | ((data[1] as u32) << 8)
                                    | data[2] as u32;
                                if us > 0 {
                                    let b = 60_000_000.0 / us as f32;
                                    // Första händelsen är filens begynnelsetempo;
                                    // senare händelser är byten och hör till kartan.
                                    if bpm.is_none() {
                                        bpm = Some(b);
                                    }
                                    tempo_events.push((tick as f64 / (ppq as f64 * 4.0), b));
                                }
                            }
                            0x2f => {
                                pos = end; // End of track: sluta läsa det här blocket.
                            }
                            _ => {}
                        }
                        pos += len.min(end.saturating_sub(pos));
                    }
                    0xf0 | 0xf7 => {
                        // SysEx: hoppa över, den påverkar inte noterna.
                        let len = read_vlq(&bytes[..end], &mut pos)? as usize;
                        pos = (pos + len).min(end);
                    }
                    _ => {
                        // Systemhändelser (t.ex. song position) — hoppa två byte.
                        pos = (pos + 2).min(end);
                    }
                },
                _ => return Err("okänd MIDI-händelse".to_string()),
            }
        }

        // Noter som aldrig fick en note-off stängs vid spårets slut, annars
        // tappas de (och en import hade varit tyst om det).
        for ((channel, key), (start, vel)) in open {
            track.notes.push(MidiNote {
                start,
                length: tick.saturating_sub(start).max(1),
                channel,
                key,
                velocity: vel,
            });
        }
        track.notes.sort_by_key(|n| n.start);
        tracks.push(track);
    }

    Ok(ParsedMidi {
        format,
        ppq,
        bpm,
        tempo_events,
        tracks,
    })
}

#[cfg(test)]
mod tests {
    /// Ett tempo ska ge EXAKT den gamla filen, byte för byte.
    ///
    /// Det här är testet som gör tempokartan ofarlig för alla filer som inte har
    /// några byten: ledspåret skrivs med samma bytes i samma ordning som förut,
    /// och skulle någon ändra ordningen fälls det här — inte av ett öra.
    #[test]
    fn one_tempo_writes_the_same_bytes_as_before() {
        for bpm in [60.0f32, 120.0, 128.0, 174.0] {
            let us = (60_000_000.0 / bpm).round() as u32;
            let mut conductor: Vec<u8> = vec![
                0x00,
                0xff,
                0x51,
                0x03,
                us.to_be_bytes()[1],
                us.to_be_bytes()[2],
                us.to_be_bytes()[3],
                0x00,
                0xff,
                0x58,
                0x04,
                4,
                2,
                24,
                8,
                0x00,
                0xff,
                0x2f,
                0x00,
            ];
            let mut expected: Vec<u8> = Vec::new();
            expected.extend_from_slice(b"MThd");
            expected.extend_from_slice(&6u32.to_be_bytes());
            expected.extend_from_slice(&1u16.to_be_bytes());
            expected.extend_from_slice(&1u16.to_be_bytes());
            expected.extend_from_slice(&PPQ.to_be_bytes());
            expected.extend_from_slice(b"MTrk");
            expected.extend_from_slice(&(conductor.len() as u32).to_be_bytes());
            expected.append(&mut conductor);
            assert_eq!(
                write_midi(bpm, &[]),
                expected,
                "{bpm} BPM ska ge exakt samma bytes som förut"
            );
        }
    }

    /// Ett tempobyte ska gå att läsa tillbaka på rätt takt.
    #[test]
    fn a_tempo_change_survives_the_round_trip() {
        let bytes = write_midi_with_tempo(&[(0.0, 120.0), (4.0, 90.0), (12.5, 174.0)], &[]);
        let parsed = parse_midi(&bytes).expect("filen ska gå att läsa");
        assert_eq!(
            parsed.tempo_events.len(),
            3,
            "alla tre punkterna ska finnas"
        );
        for ((bar, bpm), (want_bar, want_bpm)) in
            parsed
                .tempo_events
                .iter()
                .zip([(0.0, 120.0), (4.0, 90.0), (12.5, 174.0)])
        {
            assert!(
                (bar - want_bar).abs() < 1e-6,
                "takten blev {bar}, väntade {want_bar}"
            );
            assert!(
                (bpm - want_bpm).abs() < 0.01,
                "tempot blev {bpm}, väntade {want_bpm}"
            );
        }
        // Första händelsen är filens begynnelsetempo.
        let first = parsed.bpm.expect("begynnelsetempot ska finnas");
        assert!(
            (first - 120.0).abs() < 0.01,
            "begynnelsetempot blev {first}"
        );
    }

    /// Ett orimligt tempo ska bli en fil, inte ett fel — och klampar som förut.
    #[test]
    fn an_impossible_tempo_is_clamped_not_refused() {
        let bytes = write_midi_with_tempo(&[(0.0, 5000.0)], &[]);
        let parsed = parse_midi(&bytes).expect("filen ska gå att läsa");
        let bpm = parsed.bpm.expect("tempot ska finnas");
        assert!(
            (bpm - 300.0).abs() < 0.01,
            "5000 BPM skulle klamps till 300, blev {bpm}"
        );
    }

    use super::*;

    fn note(start: u32, key: u8) -> MidiNote {
        MidiNote {
            start,
            length: TICKS_PER_STEP_16TH,
            channel: 0,
            key,
            velocity: 100,
        }
    }

    #[test]
    fn vlq_matches_the_specification_examples() {
        // Exempelvärdena ur SMF-specifikationen.
        let cases = [
            (0u32, vec![0x00u8]),
            (0x40, vec![0x40]),
            (0x7f, vec![0x7f]),
            (0x80, vec![0x81, 0x00]),
            (0x2000, vec![0xc0, 0x00]),
            (0x3fff, vec![0xff, 0x7f]),
            (0x100000, vec![0xc0, 0x80, 0x00]),
            (0x0fffffff, vec![0xff, 0xff, 0xff, 0x7f]),
        ];
        for (value, expected) in cases {
            let mut out = Vec::new();
            write_vlq(&mut out, value);
            assert_eq!(out, expected, "fel kodning av {value:#x}");
            let mut pos = 0;
            assert_eq!(read_vlq(&out, &mut pos).unwrap(), value);
            assert_eq!(pos, out.len());
        }
    }

    #[test]
    fn round_trip_keeps_notes_and_tempo() {
        let tracks = vec![
            MidiTrack {
                name: "Trummor".to_string(),
                notes: vec![
                    note(0, 36),
                    note(TICKS_PER_STEP_16TH, 38),
                    note(TICKS_PER_STEP_16TH * 4, 36),
                ],
            },
            MidiTrack {
                name: "Bass".to_string(),
                notes: vec![
                    MidiNote {
                        channel: 1,
                        ..note(0, 40)
                    },
                    MidiNote {
                        channel: 1,
                        start: TICKS_PER_STEP_16TH * 8,
                        length: TICKS_PER_STEP_16TH * 2,
                        ..note(0, 43)
                    },
                ],
            },
        ];
        let bytes = write_midi(128.0, &tracks);
        let parsed = parse_midi(&bytes).expect("ska gå att läsa tillbaka");

        assert_eq!(parsed.ppq, PPQ);
        assert_eq!(parsed.format, 1);
        let bpm = parsed.bpm.expect("tempo ska finnas");
        assert!((bpm - 128.0).abs() < 0.01, "tempo blev {bpm}");

        // Ledspåret är först och innehåller inga noter.
        assert!(parsed.tracks[0].notes.is_empty());
        assert_eq!(parsed.tracks.len(), 3);
        assert_eq!(parsed.tracks[1].name.as_deref(), Some("Trummor"));
        assert_eq!(parsed.tracks[2].name.as_deref(), Some("Bass"));
        assert_eq!(parsed.tracks[1].notes, tracks[0].notes);
        assert_eq!(parsed.tracks[2].notes, tracks[1].notes);
    }

    #[test]
    fn overlapping_notes_on_the_same_key_keep_their_lengths() {
        // Två noter på samma tangent direkt efter varandra: not-off för den
        // första får inte klippa den andra. Skrivaren lägger not-off före
        // not-on vid samma tick, och läsaren parar ihop i tur och ordning.
        let tracks = vec![MidiTrack {
            name: "Lead".to_string(),
            notes: vec![
                MidiNote {
                    start: 0,
                    length: 240,
                    ..note(0, 60)
                },
                MidiNote {
                    start: 240,
                    length: 240,
                    ..note(0, 60)
                },
            ],
        }];
        let parsed = parse_midi(&write_midi(120.0, &tracks)).unwrap();
        let notes = &parsed.tracks[1].notes;
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].start, 0);
        assert_eq!(notes[0].length, 240);
        assert_eq!(notes[1].start, 240);
        assert_eq!(notes[1].length, 240);
    }

    #[test]
    fn a_foreign_file_with_running_status_and_odd_meta_is_read() {
        // Byggd för hand: running status (inga statusbyte på de två sista
        // noterna), ett okänt meta-event och ett text-event. Så här ser filer
        // från andra DAW:er ofta ut.
        let mut track: Vec<u8> = Vec::new();
        track.extend_from_slice(&[0x00, 0xff, 0x01, 0x03, b'a', b'b', b'c']); // text
        track.extend_from_slice(&[0x00, 0x90, 60, 100]); // note on
        track.extend_from_slice(&[0x81, 0x40, 60, 0]); // note off (running status): 0x81 0x40 = 192
                                                       // Running status: INGET statusbyte, bara delta + tonhöjd + styrka.
        track.extend_from_slice(&[0x00, 64, 90]);
        track.extend_from_slice(&[0x81, 0x40, 64, 0]); // note off, 192 tick senare
        track.extend_from_slice(&[0x00, 0xff, 0x7f, 0x02, 0xde, 0xad]); // okänt meta
        track.extend_from_slice(&[0x00, 0xff, 0x2f, 0x00]); // end of track

        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&0u16.to_be_bytes()); // format 0
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&96u16.to_be_bytes()); // annan PPQ
        file.extend_from_slice(b"MTrk");
        file.extend_from_slice(&(track.len() as u32).to_be_bytes());
        file.extend_from_slice(&track);

        let parsed = parse_midi(&file).expect("främmande fil ska läsas");
        assert_eq!(parsed.ppq, 96);
        assert_eq!(parsed.format, 0);
        let notes = &parsed.tracks[0].notes;
        assert_eq!(notes.len(), 2, "båda noterna ska hittas: {notes:?}");
        assert_eq!(notes[0].key, 60);
        assert_eq!(notes[0].start, 0);
        assert_eq!(notes[0].length, 96 * 2);
        assert_eq!(notes[1].key, 64);
        assert_eq!(notes[1].start, 96 * 2);
    }

    #[test]
    fn unclosed_notes_are_closed_at_the_end_of_the_track() {
        let mut track: Vec<u8> = Vec::new();
        track.extend_from_slice(&[0x00, 0x90, 60, 100]); // note on, ingen off
        track.extend_from_slice(&[0x83, 0x60, 0xff, 0x2f, 0x00]); // 480 tick, end
        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&0u16.to_be_bytes());
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&480u16.to_be_bytes());
        file.extend_from_slice(b"MTrk");
        file.extend_from_slice(&(track.len() as u32).to_be_bytes());
        file.extend_from_slice(&track);

        let parsed = parse_midi(&file).unwrap();
        assert_eq!(parsed.tracks[0].notes.len(), 1);
        assert_eq!(parsed.tracks[0].notes[0].length, 480);
    }

    #[test]
    fn broken_input_gives_an_error_instead_of_panic() {
        assert!(parse_midi(b"").is_err());
        assert!(parse_midi(b"inte en midi-fil alls").is_err());
        // Rätt rubrik men skräp i spåret.
        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&480u16.to_be_bytes());
        file.extend_from_slice(b"MTrk");
        file.extend_from_slice(&3u32.to_be_bytes());
        file.extend_from_slice(&[0x00, 0x90, 60]); // nothändelse utan styrka
        assert!(parse_midi(&file).is_err());
        // Trunkerad fil ska inte heller fälla något.
        let full = write_midi(
            120.0,
            &[MidiTrack {
                name: "x".into(),
                notes: vec![note(0, 60)],
            }],
        );
        for cut in 1..full.len() {
            let _ = parse_midi(&full[..cut]);
        }
    }

    #[test]
    fn smpte_division_is_rejected_with_a_clear_message() {
        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&0u16.to_be_bytes());
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&0xE250u16.to_be_bytes()); // SMPTE
        let err = parse_midi(&file).unwrap_err();
        assert!(err.contains("SMPTE"), "fel meddelande: {err}");
    }
}
