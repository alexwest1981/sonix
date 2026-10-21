//! Groove-mallar — Abletons `.agr`-format, applicerat på en tagning.
//!
//! Formatet är XML (läsbart, se `Core Library/Grooves` i en Live-installation): en groove är en
//! MIDI-clip vars notpositioner **är** förskjutningarna. En "Swing 16ths 52" har sina udda
//! sextondelar skjutna 4 % av ett steg, och det är precis vad filen innehåller.
//!
//! Vi läser bara noterna ur `<MidiNoteEvent Time="…">` och räknar om dem till en förskjutning per
//! steg. Det gör mallen oberoende av Abletons `<Grid>`-koder, som är otydliga utan dokumentation.

use crate::midi_take::Take;

/// Steg per takt som grooven arbetar på (fjärdedelar räknas som takter här).
pub const GROOVE_SLOTS: usize = 16;

/// En groove: förskjutning per sextondel, i **andel av ett steg** (0,04 = 4 %).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Groove {
    pub name: String,
    pub offsets: [f32; GROOVE_SLOTS],
}

impl Groove {
    /// Största förskjutningen i mallen, som andel av ett steg. Används för att visa styrkan.
    pub fn depth(&self) -> f32 {
        self.offsets.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    /// Förskjutningen för ett steg (0–15), i andel av ett steg.
    pub fn offset_for(&self, slot: usize) -> f32 {
        self.offsets[slot % GROOVE_SLOTS]
    }
}

/// Kandidater för ett steg, i takter (4/4): 1/32, 1/16-triol, 1/16, 1/8-triol, 1/8, 1/4.
const STEP_CANDIDATES: [f32; 6] = [0.0625, 1.0 / 12.0, 0.125, 1.0 / 6.0, 0.25, 0.5];

/// Sant om filen är en av Abletons **binära** groove-filer.
///
/// 103 av bibliotekets 219 mallar är binärt serialiserade (de äldre, bl.a. Logic- och
/// Percussion-mapparna) i stället för XML. Vi läser XML-varianten; den binära känns igen här så
/// gränssnittet kan säga *varför* en mall inte gick att läsa, i stället för att skylla på noterna.
pub fn agr_looks_binary(data: &[u8]) -> bool {
    let trimmed = data.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(0);
    !matches!(data.get(trimmed), Some(b'<'))
}

/// Läser en `.agr`-fil från disk. Abletons egna filer är inte alltid UTF-8 (Logic-mallarna
/// innehåller bytes utanför UTF-8), så avkodningen är förlåtande i stället för att kasta.
pub fn parse_agr_bytes(data: &[u8], name: &str) -> Option<Groove> {
    parse_agr(&String::from_utf8_lossy(data), name)
}

/// Läser en `.agr`-fil och räknar ut förskjutningen per steg.
///
/// Steget hittas genom att prova [`STEP_CANDIDATES`] och välja det som ligger närmast noterna —
/// hellre än att lita på Abletons `Grid`-kod, som skiljer sig mellan versioner.
pub fn parse_agr(text: &str, name: &str) -> Option<Groove> {
    let mut times: Vec<f32> = text
        .split("<MidiNoteEvent")
        .skip(1)
        .filter_map(|chunk| {
            let after = chunk.split("Time=\"").nth(1)?;
            after.split('"').next()?.parse::<f32>().ok()
        })
        .collect();
    if times.len() < 4 {
        return None;
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let first = times[0];

    // Välj det steg som ger minst sammanlagd avvikelse.
    let mut best = (f32::MAX, STEP_CANDIDATES[2]);
    for step in STEP_CANDIDATES {
        let err: f32 = times
            .iter()
            .enumerate()
            .map(|(i, t)| (t - first - i as f32 * step).abs())
            .sum();
        if err < best.0 {
            best = (err, step);
        }
    }
    let step = best.1;
    let slots_in_clip = (times.len() / GROOVE_SLOTS).max(1);

    // Medelvärde per slot: en groove upprepar sig varje takt, och Ableton skriver ut alla repor.
    let mut sums = [0.0f32; GROOVE_SLOTS];
    let mut counts = [0u32; GROOVE_SLOTS];
    for (i, t) in times.iter().enumerate() {
        let slot = i % GROOVE_SLOTS;
        let deviation = (t - first - i as f32 * step) / step;
        sums[slot] += deviation;
        counts[slot] += 1;
    }
    let mut offsets = [0.0f32; GROOVE_SLOTS];
    for i in 0..GROOVE_SLOTS {
        if counts[i] > 0 {
            // Klippet kan innehålla flera takter; medelvärdet över reporna är mallen.
            offsets[i] = sums[i] / counts[i] as f32 / slots_in_clip as f32;
        }
    }
    Some(Groove { name: name.to_string(), offsets })
}

impl Take {
    /// Lägger grooven på tagningen: varje not flyttas med mallens förskjutning för sitt steg,
    /// skalad med `amount` (0 = rak, 1 = full förskjutning).
    ///
    /// Tagningen är det lager både uppspelningen och exporten läser, så en groove som läggs här
    /// hörs och exporteras likadant — ingen andra väg att hålla i takt.
    pub fn apply_groove(&mut self, groove: &Groove, amount: f32) -> usize {
        let amount = amount.clamp(0.0, 1.0);
        if amount <= 0.0001 {
            return 0;
        }
        let mut moved = 0;
        for note in &mut self.notes {
            let slot = Self::slot(note);
            let shift = groove.offset_for(slot) * amount;
            if shift.abs() > 0.0001 {
                note.pos = (note.pos + shift).clamp(0.0, 15.99);
                moved += 1;
            }
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal `.agr` i Abletons format: sexton noter, udda sextondelar skjutna 4 %.
    fn swing_52() -> String {
        let mut xml = String::from("<Groove><Clip><Value><MidiClip><Notes><KeyTracks><KeyTrack><Notes>");
        for i in 0..16 {
            let t = i as f32 * 0.25 + if i % 2 == 1 { 0.01 } else { 0.0 };
            xml.push_str(&format!(
                "<MidiNoteEvent Time=\"{t}\" Duration=\"0.0625\" Velocity=\"127\" />"
            ));
        }
        xml.push_str("</Notes></KeyTrack></KeyTracks></Notes></MidiClip></Value></Clip></Groove>");
        xml
    }

    #[test]
    fn parsing_a_swing_agr_gives_four_percent_on_the_odd_steps() {
        let g = parse_agr(&swing_52(), "test").expect("mallen ska gå att läsa");
        // 0,01 takter av ett steg på 0,25 = 4 %.
        assert!((g.offset_for(1) - 0.04).abs() < 0.002, "udda steg: {}", g.offset_for(1));
        assert!(g.offset_for(0).abs() < 0.002, "jämna steg ska stå still");
        assert!((g.offset_for(3) - 0.04).abs() < 0.002);
        assert!((g.depth() - 0.04).abs() < 0.002);
    }

    #[test]
    fn applying_a_groove_moves_only_the_notes_it_should() {
        let g = parse_agr(&swing_52(), "test").unwrap();
        let mut take = Take::new();
        take.push(0.0, 60, 1.0); // jämnt steg
        take.push(1.0, 60, 1.0); // udda steg
        take.push(2.0, 60, 1.0); // jämnt steg
        assert_eq!(take.apply_groove(&g, 1.0), 1, "bara den udda noten flyttas");
        assert_eq!(take.notes[0].pos, 0.0);
        assert!((take.notes[1].pos - 1.04).abs() < 0.002, "blev {}", take.notes[1].pos);
        assert_eq!(take.notes[2].pos, 2.0);
    }

    #[test]
    fn amount_scales_and_zero_is_a_no_op() {
        let g = parse_agr(&swing_52(), "test").unwrap();
        let mut take = Take::new();
        take.push(1.0, 60, 1.0);
        assert_eq!(take.apply_groove(&g, 0.0), 0);
        assert_eq!(take.notes[0].pos, 1.0, "noll styrka ska lämna noten orörd");
        take.apply_groove(&g, 0.5);
        assert!((take.notes[0].pos - 1.02).abs() < 0.002, "halv styrka = halv förskjutning");
    }

    /// Mot en **riktig** Ableton-fil, när en sådan pekas ut. Hoppar över annars, så sviten
    /// fungerar på en maskin utan Live installerat.
    #[test]
    fn a_real_ableton_agr_parses_if_one_is_available() {
        let Ok(path) = std::env::var("SONIX_AGR") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("filen ska gå att läsa");
        let g = parse_agr_bytes(&bytes, "riktig").expect("Abletons egen .agr ska gå att läsa");
        assert!(g.depth() > 0.0, "en swing-mall utan förskjutning är ingen mall");
        assert!(g.depth() < 0.5, "förskjutningen är orimligt stor: {}", g.depth());
        println!("{}: djup {:.3} steg", path, g.depth());
    }

    #[test]
    fn a_file_without_notes_is_rejected_instead_of_panicking() {
        assert!(parse_agr("<Groove></Groove>", "tom").is_none());
        assert!(parse_agr("inte xml alls", "skräp").is_none());
    }
}
