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
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KeyMap {
    pub enabled: bool,
    pub zones: [MapZone; ZONES],
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            enabled: false,
            zones: [MapZone::default(); ZONES],
        }
    }
}

impl KeyMap {
    /// Noten efter kartan: `(not, anslag, kanal)`. Ingen aktiv zon = oförändrat.
    ///
    /// Anslaget kläms till 1–127: 0 betyder note-off i MIDI, och en nivåskalning som
    /// råkade ge 0 skulle tyst släppa noten i stället för att spela den svagt.
    pub fn apply(&self, note: u8, velocity: u8, channel: u8) -> (u8, u8, u8) {
        if !self.enabled {
            return (note, velocity, channel);
        }
        for zone in &self.zones {
            if !zone.active || note < zone.low || note > zone.high {
                continue;
            }
            let transposed = (note as i16 + zone.transpose as i16).clamp(0, 127) as u8;
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
        assert_eq!(map.apply(60, 100, 1), (60, 100, 1));
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
        assert_eq!(map.apply(52, 90, 3), (64, 90, 3));
        // Utanför zonen: oförändrat.
        assert_eq!(map.apply(60, 90, 3), (60, 90, 3));
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
        assert_eq!(map.apply(40, 80, 1), (40, 80, 4));
        assert_eq!(map.apply(70, 80, 1), (70, 80, 1));
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
        assert_eq!(map.apply(60, 100, 0).1, 50);
        // 1 * 0,5 rundas till 1 — aldrig 0, som hade varit note-off.
        assert_eq!(map.apply(60, 1, 0).1, 1);
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
        assert_eq!(map.apply(60, 64, 0).1, 90);
        // Fullt anslag ligger kvar på 127.
        assert_eq!(map.apply(60, 127, 0).1, 127);

        // Över 1 trycker ner de svaga slagen.
        soft.curve = 2.0;
        let hard = map_with(soft);
        assert_eq!(hard.apply(60, 64, 0).1, 32);
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
        assert_eq!(map.apply(60, 100, 0).0, 61);
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
        assert_eq!(map.apply(127, 100, 0).0, 127);
    }
}
