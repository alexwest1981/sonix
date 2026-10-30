//! **MIDI-karta**: FL:s Keyboard Splitter, Key Mapper och Level Scaler i en tabell.
//!
//! Alla tre gör samma sorts sak — en inkommande not får nya egenskaper beroende på var på
//! klaviaturet den ligger: en zon kan transponera (Key Mapper), byta kanal (Keyboard
//! Splitter) eller skala anslaget (Level Scaler). Att bygga tre separata lager för det hade
//! gett tre ställen som bestämmer samma sak; här finns en regel per zon och första träffen
//! vinner, precis som i en splitter.
//!
//! Kanalen är MIDI-kanal + 1 (1–16); `None` lämnar notens kanal orörd. Det gör zonen
//! användbar tillsammans med MPE: en zon kan skickas till en egen kanal och böjas för sig.
//!
//! Status: stabil — reglerna är rena och prövade; nya fält får ett prov.

/// Antalet zoner. Fyra räcker för en splitter (bas, komp, lead, solo) och håller panelen
/// läsbar; fler zoner är en annan sorts editor.
pub const ZONES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapZone {
    pub active: bool,
    /// Lägsta och högsta tangent i zonen (inklusive).
    pub low: u8,
    pub high: u8,
    /// Halvtoner som läggs på noten.
    pub transpose: i8,
    /// Ny kanal (1–16), eller `None` för att behålla notens.
    pub channel: Option<u8>,
    /// Anslagets multiplikator (1,0 = orört).
    pub velocity_scale: f32,
    /// Kurvan för anslaget: 1,0 = linjärt, under 1 = mjukare (svaga slag starkare),
    /// över 1 = hårdare. FL:s Level Scaler har samma ratt.
    #[serde(default = "linear_curve")]
    pub curve: f32,
    /// **Not → not** (FL:s VFX Key Mapper): kvantisera zonen till en skala. `None` = av,
    /// annars index i [`crate::audio::scale::SCALES`]. Tonarten kommer från projektet.
    #[serde(default)]
    pub snap_scale: Option<u8>,
}

fn linear_curve() -> f32 {
    1.0
}

impl Default for MapZone {
    fn default() -> Self {
        Self {
            active: false,
            low: 0,
            high: 127,
            transpose: 0,
            channel: None,
            velocity_scale: 1.0,
            curve: 1.0,
            snap_scale: None,
        }
    }
}

/// **Ackordlägen** — halvtoner ovanför den spelade tonen (grundtonen spelas alltid själv).
///
/// Läget multiplicerar en tangenttryckning till flera noter. Släppet räknar fram samma
/// intervall ur samma not, så ingen tabell över hållna noter behövs: `apply` är deterministisk.
pub const CHORD_MODES: [(&str, &[i8]); 8] = [
    ("Av", &[]),
    ("Oktav", &[12]),
    ("Kvint", &[7]),
    ("Power", &[7, 12]),
    ("Dur", &[4, 7]),
    ("Moll", &[3, 7]),
    ("Septima", &[3, 7, 10]),
    ("Sus4", &[5, 7]),
];

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KeyMap {
    pub enabled: bool,
    /// Index i `CHORD_MODES`. `#[serde(default)]` så att äldre projekt öppnas som förut.
    #[serde(default)]
    pub chord: u8,
    pub zones: [MapZone; ZONES],
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            enabled: false,
            chord: 0,
            zones: [MapZone::default(); ZONES],
        }
    }
}

impl KeyMap {
    /// Noten efter kartan: `(not, anslag, kanal)`. Ingen aktiv zon = oförändrat.
    ///
    /// Anslaget kläms till 1–127: 0 betyder note-off i MIDI, och en nivåskalning som
    /// råkade ge 0 skulle tyst släppa noten i stället för att spela den svagt.
    pub fn apply(&self, note: u8, velocity: u8, channel: u8, key_root: u8) -> (u8, u8, u8) {
        if !self.enabled {
            return (note, velocity, channel);
        }
        for zone in &self.zones {
            if !zone.active || note < zone.low || note > zone.high {
                continue;
            }
            let transposed = (note as i16 + zone.transpose as i16).clamp(0, 127) as u8;
            // Not → not: efter transponeringen flyttas tonen till närmaste ton i skalan.
            let transposed = match zone.snap_scale {
                Some(scale) => snap_to_scale(transposed, key_root, scale as usize),
                None => transposed,
            };
            // Kurvan läggs på det normaliserade anslaget, inte på 0–127: annars blir
            // 1,0-kurvan inte identitet.
            let shaped = (velocity as f32 / 127.0)
                .powf(zone.curve.clamp(0.2, 5.0))
                * 127.0;
            let scaled = (shaped * zone.velocity_scale.clamp(0.0, 2.0))
                .round()
                .clamp(1.0, 127.0) as u8;
            return (transposed, scaled, zone.channel.unwrap_or(channel));
        }
        (note, velocity, channel)
    }
}

impl KeyMap {
    /// Noterna en tangent ska spela: grundtonen (efter kartan) plus ackordets intervall.
    /// Kartan måste vara påslagen — annars spelar en tangent en not, som förut.
    pub fn chord_notes(&self, note: u8) -> Vec<u8> {
        let mut notes = vec![note];
        if !self.enabled {
            return notes;
        }
        let intervals = CHORD_MODES[(self.chord as usize).min(CHORD_MODES.len() - 1)].1;
        for &semi in intervals {
            let n = note as i16 + semi as i16;
            if (0..=127).contains(&n) {
                notes.push(n as u8);
            }
        }
        notes
    }
}

/// Närmaste ton i skalan, högst sex halvtoner bort (det är alltid närmast någon av de
/// tolv tonklasserna). Hittar ingen (trasigt skalindex) lämnas tonen orörd.
pub fn snap_to_scale(note: u8, root: u8, scale: usize) -> u8 {
    use crate::audio::scale::{in_scale, SCALES};
    if scale >= SCALES.len() {
        return note;
    }
    for distance in 0..=6i16 {
        for candidate in [note as i16 - distance, note as i16 + distance] {
            if (0..=127).contains(&candidate)
                && in_scale((candidate as u8) % 12, root % 12, scale)
            {
                return candidate as u8;
            }
        }
    }
    note
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with(zone: MapZone) -> KeyMap {
        let mut map = KeyMap {
            enabled: true,
            ..KeyMap::default()
        };
        map.zones[0] = zone;
        map
    }

    #[test]
    fn without_an_active_zone_the_note_passes_untouched() {
        let map = KeyMap::default();
        assert_eq!(map.apply(60, 100, 1, 0), (60, 100, 1));
    }

    #[test]
    fn a_zone_transposes_and_leaves_the_rest_alone() {
        let map = map_with(MapZone {
            active: true,
            low: 48,
            high: 59,
            transpose: 12,
            ..MapZone::default()
        });
        assert_eq!(map.apply(52, 90, 3, 0), (64, 90, 3));
        // Utanför zonen: oförändrat.
        assert_eq!(map.apply(60, 90, 3, 0), (60, 90, 3));
    }

    #[test]
    fn a_zone_can_move_the_note_to_another_channel() {
        let map = map_with(MapZone {
            active: true,
            low: 0,
            high: 63,
            channel: Some(4),
            ..MapZone::default()
        });
        assert_eq!(map.apply(40, 80, 1, 0), (40, 80, 4));
        assert_eq!(map.apply(70, 80, 1, 0), (70, 80, 1));
    }

    #[test]
    fn velocity_is_scaled_but_never_reaches_zero() {
        let map = map_with(MapZone {
            active: true,
            low: 0,
            high: 127,
            velocity_scale: 0.5,
            ..MapZone::default()
        });
        assert_eq!(map.apply(60, 100, 0, 0).1, 50);
        // 1 * 0,5 rundas till 1 — aldrig 0, som hade varit note-off.
        assert_eq!(map.apply(60, 1, 0, 0).1, 1);
    }

    #[test]
    fn the_curve_makes_soft_notes_harder_without_moving_the_loud_ones() {
        let mut soft = MapZone {
            active: true,
            low: 0,
            high: 127,
            curve: 0.5,
            ..MapZone::default()
        };
        let map = map_with(soft);
        // (64/127)^0,5 ≈ 0,71 → 90, alltså starkare än 64.
        assert_eq!(map.apply(60, 64, 0, 0).1, 90);
        // Fullt anslag ligger kvar på 127.
        assert_eq!(map.apply(60, 127, 0, 0).1, 127);

        // Över 1 trycker ner de svaga slagen.
        soft.curve = 2.0;
        let hard = map_with(soft);
        assert_eq!(hard.apply(60, 64, 0, 0).1, 32);
    }

    #[test]
    fn the_first_matching_zone_wins() {
        let mut map = KeyMap {
            enabled: true,
            ..KeyMap::default()
        };
        map.zones[0] = MapZone {
            active: true,
            low: 0,
            high: 127,
            transpose: 1,
            ..MapZone::default()
        };
        map.zones[1] = MapZone {
            active: true,
            low: 0,
            high: 127,
            transpose: 7,
            ..MapZone::default()
        };
        assert_eq!(map.apply(60, 100, 0, 0).0, 61);
    }

    #[test]
    fn transposing_past_the_keyboard_ends_is_clamped() {
        let map = map_with(MapZone {
            active: true,
            low: 120,
            high: 127,
            transpose: 24,
            ..MapZone::default()
        });
        assert_eq!(map.apply(127, 100, 0, 0).0, 127);
    }

    /// **Not → not:** en zon kan kvantisera till en skala — tonen som lämnar kartan är en
    /// annan än den som kom in, och en ton som redan ligger i skalan rörs inte.
    #[test]
    fn a_zone_can_snap_notes_to_a_scale() {
        let mut map = KeyMap::default();
        map.enabled = true;
        map.zones[0].active = true;
        map.zones[0].low = 0;
        map.zones[0].high = 127;
        map.zones[0].snap_scale = Some(0); // Dur

        assert_eq!(map.apply(61, 100, 1, 0).0, 60, "C# i C-dur blir C");
        assert_eq!(map.apply(66, 100, 1, 0).0, 65, "F# blir F");
        assert_eq!(map.apply(64, 100, 1, 0).0, 64, "E ligger redan i skalan");
        assert_eq!(map.apply(61, 100, 1, 1).0, 61, "C# är i C#-dur");
        assert_eq!(
            map.apply(61, 100, 1, 99).0,
            60,
            "en grundton utanför 0–11 tolkas mod 12"
        );
    }

    #[test]
    fn a_chord_mode_turns_one_key_into_several_notes() {
        let mut map = KeyMap::default();
        // Avstängd karta = en tangent, en not (som förut).
        assert_eq!(map.chord_notes(60), vec![60]);
        map.enabled = true;
        // Moll: grundton, liten ters, kvint.
        map.chord = 5;
        assert_eq!(map.chord_notes(60), vec![60, 63, 67]);
        // Samma not igen vid släpp — `chord_notes` är deterministisk, ingen tabell behövs.
        assert_eq!(map.chord_notes(60), vec![60, 63, 67]);
        // Översta tangenten får inga barn som hamnar utanför 0..127.
        assert_eq!(map.chord_notes(127), vec![127]);
        // Ett trasigt index ska inte panika.
        map.chord = 200;
        assert_eq!(map.chord_notes(60), vec![60, 65, 67]);
    }
}
