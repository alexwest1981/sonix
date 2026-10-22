//! Läser Abletons analysfiler (`.asd`) — deras egna onset-positioner.
//!
//! Ableton skriver en analysfil bredvid varje sampling. Filen är deras egen
//! serialisering: en namntabell med UTF-16-strängar och därefter datablock med
//! fält-id. Onset-arrayen ligger som ett **u32 antal** följt av så många **int32 i
//! sampel**, monotont stigande.
//!
//! Det finns ingen publik spec, så arrayen hittas med en regel som kan prövas: rätt
//! array är monotont stigande, börjar nära noll, har steg på minst 1 ms och slutar
//! inom filens längd. Den regeln håller på 3 567 av 3 567 filer i Abletons
//! Core Library (se `asd_onsets`-provet), och positionerna ligger på sextondelsnätet
//! vid filens egen BPM — vilket är vad man väntar sig av en transientdetektor.
//!
//! Nyttan är inte att kunna läsa deras filer i drift, utan att **mäta vår egen
//! detektor mot deras facit**: 6 138 samplingar med facit i Abletons egen produkt.

/// Onset-positioner i sampel, enligt Abletons analysfil.
///
/// Används av mätprovet i `onset` (facit för vår detektor), inte i drift.
#[cfg(test)]
pub fn parse_onsets(data: &[u8]) -> Option<Vec<usize>> {
    let n = data.len() / 4;
    if n < 3 {
        return None;
    }
    // Läses som i32 för att jämförelser ska kunna göras utan teckenstrul.
    let mut words = Vec::with_capacity(n);
    for i in 0..n {
        let b = &data[i * 4..i * 4 + 4];
        words.push(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    }
    let mut best: Option<Vec<usize>> = None;
    for i in 0..n.saturating_sub(2) {
        let count = words[i];
        if !(1..=20_000).contains(&count) {
            continue;
        }
        let count = count as usize;
        if i + 1 + count > n {
            continue;
        }
        let first = words[i + 1];
        if !(0..=20_000).contains(&first) {
            continue;
        }
        let seq = &words[i + 1..i + 1 + count];
        let mut ok = true;
        for k in 0..count.saturating_sub(1) {
            // i64: rådata innehåller godtyckliga bitmönster, och i32 kan svämma över.
            let step = seq[k + 1] as i64 - seq[k] as i64;
            // 1 ms … 2 s vid 44,1 kHz: snabbare är inte en transient, långsammare är en takt.
            if !(44..=88_200).contains(&step) {
                ok = false;
                break;
            }
        }
        if ok && best.as_ref().is_none_or(|b| count > b.len()) {
            best = Some(seq.iter().map(|v| *v as usize).collect());
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bygger en buffer som ser ut som Abletons: skräp, ett antal, sedan positionerna.
    fn fake_asd(prefix: &[i32], onsets: &[i32], suffix: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        let count = onsets.len() as i32;
        for v in prefix
            .iter()
            .chain(std::iter::once(&count))
            .chain(onsets.iter())
            .chain(suffix.iter())
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    #[test]
    fn the_onset_array_is_found_inside_other_numbers() {
        // Skräp som ser ut som tal, men inte som en stigande sekvens.
        let prefix = [0, 0, 1, 4_194_304, 1, 3, 7, 0, 0, 0];
        let onsets = [3, 7_581, 14_701, 22_358, 29_093, 37_523];
        let suffix = [0, 0, 123_456_789, 2];
        let data = fake_asd(&prefix, &onsets, &suffix);
        let got = parse_onsets(&data).expect("arrayen ska hittas");
        assert_eq!(got, onsets.iter().map(|v| *v as usize).collect::<Vec<_>>());
    }

    #[test]
    fn a_buffer_without_an_onset_array_gives_nothing() {
        assert!(parse_onsets(&[]).is_none());
        let flat: Vec<u8> = (0..64).flat_map(|_| 0i32.to_le_bytes()).collect();
        assert!(parse_onsets(&flat).is_none());
        // Fallande sekvens är inte en onset-array.
        let data = fake_asd(&[], &[9000, 4000, 1000], &[]);
        assert!(parse_onsets(&data).is_none());
    }

    #[test]
    fn steps_that_are_too_short_or_too_long_are_rejected() {
        // 10 sampel mellan två onsets är 0,2 ms — för snabbt för att vara en transient.
        assert!(parse_onsets(&fake_asd(&[], &[1_000, 1_010, 9_000], &[])).is_none());
        // 5 sekunder mellan två är inte heller ett anslag.
        assert!(parse_onsets(&fake_asd(&[], &[1_000, 221_000], &[])).is_none());
    }
}
