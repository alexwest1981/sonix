//! Tonarter: **en** tabell och **ett** index (Alex' kvittens 2026-09-12).
//!
//! **Varför modulen finns.** Arrangerarens tonartskontroll och piano roll hade
//! var sin lista över skalor, och de skrev över varandras index:
//!
//! - Arrangeraren: `["Dur", "Moll", "Dorian", "Blues", "Synthwave"]` — fem namn.
//! - Piano roll: `["Kromatisk", "Dur (Maj)", "Moll (Min)", "Harm. Moll", "Mel. Moll",
//!   "Dorian", "Mixolydian", "Pentatonisk", "Blues", "Synthwave"]` — tio.
//!
//! Att välja *Dorian* i arrangeraren satte alltså index 2 — som i piano rollen är
//! **Moll**. Kontrollen visade ett namn och gjorde något annat, och den som litade på
//! den (eller på AI-kontexten, som läste arrangerarens namn) fick fel svar.
//!
//! Dessutom hade grundtonslistorna **13** namn för 12 kromatiska steg
//! (`C C# D D# Eb E F F# G Ab A Bb B`): index 4 visade "Eb" men satte tonhöjd 4 = **E**.
//! Fyra av tolv val gav alltså en annan ton än den man klickade på. Listan här har tolv
//! namn, och `the_root_names_are_pitch_classes` håller den fast.
//!
//! Nu finns skalorna på ett ställe: samma tabell ger arrangerarens meny,
//! piano rollens meny, skalmarkeringen i rutnätet och AI-kontextens etikett.

/// Grundtonernas namn, index = tonhöjdsklass (0 = C).
///
/// Tolv namn, inte tretton: en blandning av kors och b är läsbarare än bara det ena,
/// men varje ton får **en** stavning och index är tonhöjdsklassen.
pub const ROOT_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// En skala: namnet som visas och avstånden i halvtoner från grundtonen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    pub name: &'static str,
    /// Halvtoner från grundtonen, stigande och med 0 först.
    pub intervals: &'static [u8],
}

/// Skalorna, i den ordning menyerna visar dem.
///
/// `Dur` är först: det är vad arrangerarens kontroll alltid har *sagt* (index 0), så
/// med en gemensam tabell betyder index 0 samma sak i båda menyerna.
pub const SCALES: [Scale; 11] = [
    Scale {
        name: "Dur",
        intervals: &[0, 2, 4, 5, 7, 9, 11],
    },
    Scale {
        name: "Moll",
        intervals: &[0, 2, 3, 5, 7, 8, 10],
    },
    Scale {
        name: "Harmonisk moll",
        intervals: &[0, 2, 3, 5, 7, 8, 11],
    },
    Scale {
        name: "Melodisk moll",
        intervals: &[0, 2, 3, 5, 7, 9, 11],
    },
    Scale {
        name: "Dorian",
        intervals: &[0, 2, 3, 5, 7, 9, 10],
    },
    Scale {
        name: "Mixolydian",
        intervals: &[0, 2, 4, 5, 7, 9, 10],
    },
    Scale {
        name: "Pentatonisk moll",
        intervals: &[0, 3, 5, 7, 10],
    },
    Scale {
        name: "Pentatonisk dur",
        intervals: &[0, 2, 4, 7, 9],
    },
    Scale {
        name: "Blues",
        intervals: &[0, 3, 5, 6, 7, 10],
    },
    // Friskisk med mollters: den ton som synthvågen lånar sin klang från.
    Scale {
        name: "Synthwave",
        intervals: &[0, 1, 3, 5, 7, 8, 10],
    },
    Scale {
        name: "Kromatisk",
        intervals: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    },
];

impl Scale {
    /// Är skalans ters en **mollters**?
    ///
    /// Regeln är mätbar ur intervallen: en mollters ligger tre halvtoner över
    /// grundtonen, en durters fyra. Kromatisk skala har båda, och är därför varken
    /// eller — den säger ingenting om dur och moll.
    pub fn is_minor(&self) -> bool {
        self.intervals.contains(&3) && !self.intervals.contains(&4)
    }
}

/// Skalan för ett index, klämt till tabellen (ett projektfält kan innehålla vad som
/// helst, och en skala som inte finns får inte bli en panik).
pub fn scale_at(index: usize) -> Scale {
    SCALES[index.min(SCALES.len() - 1)]
}

/// Grundtonens namn för en tonhöjdsklass.
pub fn root_name(root: u8) -> &'static str {
    ROOT_NAMES[(root % 12) as usize]
}

/// Etiketten som beskriver tonarten, t.ex. `"Eb Dur"` — samma sträng som
/// arrangeraren visar och AI-kontexten får.
pub fn key_label(root: u8, scale: usize) -> String {
    format!("{} {}", root_name(root), scale_at(scale).name)
}

/// Tonhöjdsklasserna i skalan, med grundtonen först.
pub fn scale_notes(root: u8, scale: usize) -> Vec<u8> {
    let root = root % 12;
    scale_at(scale)
        .intervals
        .iter()
        .map(|&i| (root + i) % 12)
        .collect()
}

/// Är tonen i skalan?
pub fn in_scale(pc: u8, root: u8, scale: usize) -> bool {
    scale_notes(root, scale).contains(&(pc % 12))
}

/// Närmaste ton i skalan, räknat i halvtoner (cirkulärt).
///
/// Lika långt upp som ned → **nedåt** vinner: den som låser sig till en skala ska
/// hamna på en ton som ligger under den man pekade på, inte över. Regeln är vald, inte
/// mätt, och står här för att den ska gå att ändra på ett ställe.
pub fn nearest_in_scale(pc: u8, root: u8, scale: usize) -> u8 {
    let pc = pc % 12;
    let notes = scale_notes(root, scale);
    if notes.contains(&pc) {
        return pc;
    }
    notes
        .iter()
        .copied()
        .min_by_key(|&n| {
            let up = (n + 12 - pc) % 12;
            let down = (pc + 12 - n) % 12;
            // Lika nära upp som ned → **nedåt** vinner, och det är den här nyckeln
            // som avgör: distansen först, sedan hur långt nedåt svaret ligger.
            (up.min(down), down)
        })
        .unwrap_or(pc)
}

/// Raden i piano rollen en klickad rad ska hamna på när skal-låset är på.
///
/// Rutnätet är `rows` rader från `base_midi`, och en rad motsvarar en tangent. Är
/// tangenten i skalan blir svaret samma rad; annars den **närmaste** raden i skalan,
/// och vid lika avstånd den lägre (samma regel som [`nearest_in_scale`]).
pub fn snap_row(row: usize, base_midi: u8, rows: usize, root: u8, scale: usize) -> usize {
    let pc = (base_midi as usize + row) % 12;
    let target = nearest_in_scale(pc as u8, root, scale) as usize;
    // Närmaste rad med den tonhöjdsklassen — sök utåt från den klickade raden.
    for distance in 0..=rows {
        if distance <= row {
            let below = row - distance;
            if (base_midi as usize + below) % 12 == target {
                return below;
            }
        }
        let above = row + distance;
        if above < rows && (base_midi as usize + above) % 12 == target {
            return above;
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grundtonslistan är tolv tonhöjdsklasser — och bär rätt namn.
    ///
    /// Testet finns för att en tretton-namnlista såg rätt ut men flyttade fyra av tolv
    /// val: index 4 visade "Eb" och satte E.
    #[test]
    fn the_root_names_are_pitch_classes() {
        assert_eq!(ROOT_NAMES.len(), 12);
        assert_eq!(ROOT_NAMES[0], "C");
        assert_eq!(ROOT_NAMES[3], "Eb", "3 = Eb, inte E");
        assert_eq!(ROOT_NAMES[4], "E");
        assert_eq!(ROOT_NAMES[5], "F");
        assert_eq!(ROOT_NAMES[8], "Ab", "8 = Ab, inte A");
        assert_eq!(ROOT_NAMES[10], "Bb", "10 = Bb, inte B");
        assert_eq!(ROOT_NAMES[11], "B");
        // Varje namn finns exakt en gång: två stavningar av samma ton vore två rader
        // som gör samma sak.
        let mut sorted = ROOT_NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 12);
    }

    /// Tabellen är meningsfull: stigande avstånd, börjar på 0, ryms i en oktav.
    #[test]
    fn every_scale_is_a_scale() {
        for (i, s) in SCALES.iter().enumerate() {
            assert_eq!(s.intervals[0], 0, "{} börjar inte på grundtonen", s.name);
            assert!(s.intervals.len() >= 5, "{} har för få toner", s.name);
            assert!(
                s.intervals.windows(2).all(|w| w[0] < w[1]),
                "{} är inte stigande eller har dubbletter: {:?}",
                s.name,
                s.intervals
            );
            assert!(
                s.intervals.iter().all(|&iv| iv < 12),
                "{} går utanför oktaven: {:?}",
                s.name,
                s.intervals
            );
            // Och alla skalor ger samma antal unika tonhöjdsklasser som intervall.
            let notes = scale_notes(0, i);
            let mut unique = notes.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), notes.len(), "{} ger dubbletter", s.name);
        }
    }

    /// Namnen är unika — annars kan en meny visa två rader som ser likadana ut men
    /// betyder olika saker (vilket var hela felet med de två listorna).
    #[test]
    fn the_scale_names_are_unique() {
        let mut names: Vec<&str> = SCALES.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SCALES.len());
    }

    /// Rätt toner: Dur och Moll från C, och samma skalor från Eb.
    #[test]
    fn the_scales_contain_the_notes_they_say() {
        // C-dur: C D E F G A B
        assert_eq!(scale_notes(0, 0), vec![0, 2, 4, 5, 7, 9, 11]);
        // C-moll: C D Eb F G Ab Bb
        assert_eq!(scale_notes(0, 1), vec![0, 2, 3, 5, 7, 8, 10]);
        // Eb-dur (grundton 3): Eb F G Ab Bb C D
        assert_eq!(scale_notes(3, 0), vec![3, 5, 7, 8, 10, 0, 2]);
        // C-blues: C Eb F F# G Bb
        assert_eq!(scale_notes(0, 8), vec![0, 3, 5, 6, 7, 10]);
        // Kromatisk rymmer allt — den låser ingenting.
        assert_eq!(scale_notes(0, SCALES.len() - 1).len(), 12);
        assert!(in_scale(6, 0, SCALES.len() - 1));
    }

    /// Dur och moll går att skilja på, och kromatisk är varken eller.
    #[test]
    fn minor_and_major_are_told_apart_by_the_third() {
        assert!(!scale_at(0).is_minor(), "Dur");
        assert!(scale_at(1).is_minor(), "Moll");
        assert!(scale_at(4).is_minor(), "Dorian har mollters");
        assert!(!scale_at(5).is_minor(), "Mixolydian har durters");
        assert!(scale_at(6).is_minor(), "Pentatonisk moll");
        assert!(!scale_at(7).is_minor(), "Pentatonisk dur");
        assert!(scale_at(8).is_minor(), "Blues");
        assert!(
            !scale_at(SCALES.len() - 1).is_minor(),
            "Kromatisk är varken dur eller moll"
        );
    }

    /// Ett index utanför tabellen får inte panika — ett projektfält kan vara vad som
    /// helst.
    #[test]
    fn an_index_outside_the_table_falls_back_to_the_last_scale() {
        let last = SCALES.len() - 1;
        assert_eq!(scale_at(999).name, SCALES[last].name);
        assert_eq!(scale_notes(0, 999), scale_notes(0, last));
    }

    /// Närmaste skalton: i skalan → sig själv, annars den närmaste.
    #[test]
    fn the_nearest_scale_note_is_nearest() {
        // C-dur: Eb (3) ligger ett halvt steg från både D (2) och E (4) → nedåt.
        assert_eq!(nearest_in_scale(3, 0, 0), 2, "Eb ska bli D");
        // Samma sak uppifrån: F# (6) ett halvt steg från F (5) och G (7) → F.
        assert_eq!(nearest_in_scale(6, 0, 0), 5, "F# ska bli F");
        // Ett helt steg ifrån: G# (8) i C-dur → G (7), inte A (9).
        assert_eq!(nearest_in_scale(8, 0, 0), 7);
        // Redan i skalan: oförändrad.
        assert_eq!(nearest_in_scale(7, 0, 0), 7);
        // Kromatisk: allt är i skalan.
        assert_eq!(nearest_in_scale(6, 0, SCALES.len() - 1), 6);
    }

    /// Låset flyttar en klickad rad till närmaste rad i skalan — och rör ingenting när
    /// raden redan är i skalan.
    #[test]
    fn the_scale_lock_moves_the_row_to_the_scale() {
        // 24 rader från C3 (midi 48 = C). Rad 1 = C# (utanför C-dur), närmast C.
        assert_eq!(snap_row(1, 48, 24, 0, 0), 0, "C# ska bli C");
        // Rad 3 = Eb (utanför C-dur), lika nära D som E → nedåt, rad 2.
        assert_eq!(snap_row(3, 48, 24, 0, 0), 2, "Eb ska bli D");
        // Rad 0 = C, som är i skalan: oförändrad.
        assert_eq!(snap_row(0, 48, 24, 0, 0), 0);
        // Kromatisk låser ingenting.
        let chromatic = SCALES.len() - 1;
        for row in 0..24 {
            assert_eq!(snap_row(row, 48, 24, 0, chromatic), row);
        }
        // Och låset kan aldrig peka utanför rutnätet.
        for row in 0..24 {
            assert!(snap_row(row, 48, 24, 3, 0) < 24);
        }
    }
}
