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
/// 103 av Abletons 219 grooves är **binära**; de läses av [`parse_agr_binary`].
pub fn parse_agr_bytes(data: &[u8], name: &str) -> Option<Groove> {
    if agr_looks_binary(data) {
        return parse_agr_binary(data, name);
    }
    parse_agr(&String::from_utf8_lossy(data), name)
}

/// Läser Abletons **binära** groove-format.
///
/// Formatet är Abletons egen serialisering och har ingen publik spec. En not är 25 byte:
/// en flaggbyte, sedan tre `f64` — `(anslag, tid, längd)` — där tiden är i **beats** och
/// längden för en sextondel är 0,0625. Noterna ligger i en array med 25 byte mellan varje
/// post och stigande tid, så arrayen hittas genom att leta efter den längsta följden av
/// poster som har samma längd och stigande tid.
///
/// Tolkningen är prövad mot filnamnen: mallar som heter "Swing MPC … 70" får förskjutningar
/// upp till 0,499 steg (maximal swing), medan "Swing Logic 16ths 63" får 0,000 — Logic-mallen
/// markerar swing i sitt eget `Grid` i stället. Det är vad man väntar sig om tiden är beats
/// och förskjutningen mäts mot sextondelsnätet.
pub fn parse_agr_binary(data: &[u8], name: &str) -> Option<Groove> {
    const STRIDE: usize = 25;
    const MIN_RECORDS: usize = 8;

    let f64_at = |at: usize| -> Option<f64> {
        let bytes = data.get(at..at + 8)?;
        Some(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    };
    // En post läses från sin flaggbyte: anslag (f64), tid (f64), längd (f64).
    let record = |start: usize| -> Option<(f64, f64, f64)> {
        let velocity = f64_at(start + 1)?;
        let time = f64_at(start + 9)?;
        let length = f64_at(start + 17)?;
        Some((velocity, time, length))
    };

    let mut best: Option<Vec<(f64, f64)>> = None;
    if data.len() < MIN_RECORDS * STRIDE {
        return None;
    }
    for start in 0..=(data.len() - MIN_RECORDS * STRIDE) {
        let mut notes: Vec<(f64, f64)> = Vec::with_capacity(MIN_RECORDS);
        let mut length: Option<f64> = None;
        let mut ok = true;
        for k in 0..MIN_RECORDS {
            let Some((velocity, time, len)) = record(start + k * STRIDE) else {
                ok = false;
                break;
            };
            if !(-0.5..=2.0).contains(&velocity) || !(0.0..512.0).contains(&time) {
                ok = false;
                break;
            }
            let expected = *length.get_or_insert(len);
            // Längden är gemensam för hela arrayen, och den ska vara ett notvärde.
            if (len - expected).abs() > 1e-12 || !(0.0..=8.0).contains(&len) || len == 0.0 {
                ok = false;
                break;
            }
            if notes.last().is_some_and(|(_, prev)| time < *prev - 1e-9) {
                ok = false;
                break;
            }
            notes.push((velocity, time));
        }
        if !ok {
            continue;
        }
        // Förläng med så många poster som fortsätter mönstret.
        let mut k = MIN_RECORDS;
        while let Some((velocity, time, len)) = record(start + k * STRIDE) {
            if (len - length.unwrap_or(len)).abs() > 1e-12
                || !(-0.5..=2.0).contains(&velocity)
                || !(0.0..512.0).contains(&time)
                || notes.last().is_some_and(|(_, prev)| time < *prev - 1e-9)
            {
                break;
            }
            notes.push((velocity, time));
            k += 1;
        }
        if best.as_ref().is_none_or(|b| notes.len() > b.len()) {
            best = Some(notes);
        }
    }

    let notes = best?;
    // Position i sextondelar = tid i beats × 4. Förskjutningen är avståndet till närmaste
    // sextondel, medelvärt per steg (en mall kan träffa samma steg flera gånger).
    let mut sum = [0.0f32; GROOVE_SLOTS];
    let mut count = [0u32; GROOVE_SLOTS];
    for (_, time) in &notes {
        let position = (*time as f32) * 4.0;
        let slot_pos = position.round();
        let slot = (slot_pos as i64).rem_euclid(GROOVE_SLOTS as i64) as usize;
        sum[slot] += position - slot_pos;
        count[slot] += 1;
    }
    let mut offsets = [0.0f32; GROOVE_SLOTS];
    for slot in 0..GROOVE_SLOTS {
        if count[slot] > 0 {
            offsets[slot] = (sum[slot] / count[slot] as f32).clamp(-0.5, 0.5);
        }
    }
    Some(Groove { name: name.to_string(), offsets })
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

/// En mall i poolen: namn, grooven och hur mycket av den som läggs på.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GrooveEntry {
    pub name: String,
    pub groove: Groove,
    pub amount: f32,
    pub enabled: bool,
}

impl GrooveEntry {
    pub fn new(groove: Groove, amount: f32) -> Self {
        Self { name: groove.name.clone(), groove, amount, enabled: true }
    }
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

    /// Lägger **alla aktiva mallar i poolen**, i listans ordning.
    ///
    /// Förskjutningarna summeras: två mallar som var för sig skjuter udda sextondelar
    /// framåt lägger ihop sin verkan, precis som om man staplade dem.
    pub fn apply_groove_pool(&mut self, pool: &[GrooveEntry]) -> usize {
        let mut total = 0;
        for entry in pool.iter().filter(|e| e.enabled) {
            total += self.apply_groove(&entry.groove, entry.amount);
        }
        total
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
    fn a_pool_stacks_its_templates_and_skips_the_disabled_ones() {
        let g = parse_agr(&swing_52(), "swing").unwrap();
        let mut take = Take::new();
        take.push(1.0, 60, 1.0);

        let mut off = GrooveEntry::new(g.clone(), 1.0);
        off.enabled = false;
        let pool = vec![GrooveEntry::new(g.clone(), 0.5), off, GrooveEntry::new(g, 1.0)];
        let moved = take.apply_groove_pool(&pool);
        assert!(moved >= 1, "minst en mall ska ha flyttat noten");
        // 0,04 × 0,5 + 0,04 × 1,0 = 0,06 steg. Den avstängda raden räknas inte.
        assert!((take.notes[0].pos - 1.06).abs() < 0.003, "blev {}", take.notes[0].pos);
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
    /// Bygger en binär `.agr` som Abletons: skräp, sedan 25-byte-poster.
    fn binary_agr(times: &[f64], length: f64) -> Vec<u8> {
        let mut out = vec![0xab, 0x1e, 0x56, 0x78, 3, 0x93, 0, 0, 0];
        out.extend_from_slice(b"\x06Groove");
        for (i, t) in times.iter().enumerate() {
            out.push(if i % 2 == 0 { 1 } else { 0x80 });
            out.extend_from_slice(&0.9f64.to_le_bytes());
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&length.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 12]);
        out
    }

    #[test]
    fn a_binary_agr_yields_the_same_offsets_as_the_xml_one() {
        // Samma swing som en XML-mall: varannan sextondel på 0,6224 beats i stället för 0,5.
        let times = [
            0.0, 0.6224, 1.0, 1.6224, 2.0, 2.6224, 3.0, 3.6224, 4.0, 4.6224,
        ];
        let data = binary_agr(&times, 0.0625);
        let g = parse_agr_binary(&data, "swing").expect("binär mall ska gå att läsa");
        assert!(g.offsets[0].abs() < 0.01, "steg 0 ligger på nätet: {}", g.offsets[0]);
        // 0,6224 × 4 = 2,4896 → steg 2 med +0,49.
        assert!(
            (g.offsets[2] - 0.49).abs() < 0.02,
            "steg 2 ska ligga 0,49 steg sent: {}",
            g.offsets[2]
        );
        assert_eq!(g.offsets[1], 0.0, "steg utan noter har ingen förskjutning");
    }

    #[test]
    fn a_binary_file_without_records_is_rejected() {
        assert!(parse_agr_binary(&[0u8; 400], "skräp").is_none());
        // Poster med fallande tid är ingen notlista.
        let falling = [9.0, 4.0, 3.0, 2.0, 1.0, 0.5, 0.4, 0.3, 0.2, 0.1];
        assert!(binary_agr(&falling, 0.0625).len() > 0);
        assert!(parse_agr_binary(&binary_agr(&falling, 0.0625), "fallande").is_none());
    }

    /// **Alla Abletons groove-mallar**, XML som binära, när katalogen är angiven.
    /// Namnen är facit: mallar som heter "Swing MPC … 70" ska ha stor förskjutning,
    /// "Swing Logic 16ths 63" ska ha noll (Logic markerar swing i sitt `Grid`).
    #[test]
    fn every_real_ableton_groove_parses_if_a_folder_is_available() {
        let Ok(root) = std::env::var("SONIX_AGR_DIR") else {
            return;
        };
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "agr") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(std::path::Path::new(&root), &mut files);
        files.sort();
        let total = files.len();
        let mut parsed = 0usize;
        let mut binary = 0usize;
        let mut mpc_depth = 0.0f32;
        let mut logic_depth = f32::NAN;
        for path in &files {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let Some(g) = parse_agr_bytes(&bytes, &name) else {
                // De fyra "Quantize …"-filerna är **kvantiseringsnät**, inte grooves: de har
                // tre noter och ingen förskjutning att läsa. De räknas inte som misslyckade.
                assert!(
                    name.starts_with("Quantize"),
                    "bara kvantiseringsnäten får vara oläsbara, inte {name}"
                );
                continue;
            };
            parsed += 1;
            if agr_looks_binary(&bytes) {
                binary += 1;
            }
            if name.contains("Swing MPC") {
                mpc_depth = mpc_depth.max(g.depth());
            }
            if name.contains("Swing Logic") {
                logic_depth = g.depth();
            }
        }
        println!("{parsed} av {total} mallar lästa ({binary} binära)");
        assert_eq!(parsed, total - 4, "alla grooves ska läsas; bara de fyra kvantiseringsnäten får falla");
        assert!(mpc_depth > 0.2, "MPC-swingen ska ha tydlig förskjutning: {mpc_depth:.3}");
        assert!(
            logic_depth.is_nan() || logic_depth < 0.01,
            "Logic-mallen markerar swing i Grid, inte i tiderna: {logic_depth:.3}"
        );
    }

}
