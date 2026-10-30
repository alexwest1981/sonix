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
    /// **Note length**: the engine releases the note this many milliseconds after the
    /// note-on even if the key is still held. 0 = off. A key-up before that point sends
    /// its own note-off and the engine drops the pending one.
    #[serde(default)]
    pub length_ms: u32,
    /// **Echo**: the note is heard again this many milliseconds after the note-on. 0 = off.
    #[serde(default)]
    pub delay_ms: u32,
    /// How many times the note is heard with echo on: 2 = one echo, 3 = two. 0 or 1 = the
    /// note alone. The echo count is `echo_repeats - 1`, see [`KeyMap::echo_offsets`].
    #[serde(default)]
    pub echo_repeats: u8,
    /// **Random velocity** per hit, as a fraction of the full 0–127 range: 0 = off,
    /// 0.5 = ±63. Only the velocity is randomised — the pitch stays the key's.
    #[serde(default)]
    pub random_velocity: f32,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            enabled: false,
            chord: 0,
            zones: [MapZone::default(); ZONES],
            length_ms: 0,
            delay_ms: 0,
            echo_repeats: 0,
            random_velocity: 0.0,
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

/// **Milliseconds to samples**, rounded to the nearest sample. Integer math so the offset
/// is exactly `ms * sr / 1000` and a test can compare it with `==`.
fn ms_to_samples(ms: u32, sample_rate: f32) -> u32 {
    let sr = sample_rate.round().max(1.0) as u64;
    ((ms as u64 * sr + 500) / 1000) as u32
}

/// Anslaget en repetition dämpas med: 0,6 per steg, alltså 0,6 / 0,36 / 0,216.
const ECHO_GAIN_PER_REPEAT: f32 = 0.6;

impl KeyMap {
    /// Samples from the note-on to the automatic release, or `None` when the length is off.
    pub fn length_release_samples(&self, sample_rate: f32) -> Option<u32> {
        if !self.enabled || self.length_ms == 0 {
            return None;
        }
        Some(ms_to_samples(self.length_ms, sample_rate)).filter(|&n| n > 0)
    }

    /// The echoes: `(samples after the note-on, gain)`. The note itself is not in here, so
    /// `echo_repeats` = N gives N-1 entries at `k * delay_ms`, each one
    /// `ECHO_GAIN_PER_REPEAT` quieter than the one before.
    pub fn echo_offsets(&self, sample_rate: f32) -> Vec<(u32, f32)> {
        if !self.enabled || self.delay_ms == 0 || self.echo_repeats < 2 {
            return Vec::new();
        }
        let per = ms_to_samples(self.delay_ms, sample_rate);
        (1..self.echo_repeats.min(MAX_ECHO_REPEATS as u8) as u32)
            .map(|k| (per.saturating_mul(k), ECHO_GAIN_PER_REPEAT.powi(k as i32)))
            .collect()
    }

    /// Per hit: the velocity wanders by ±`random_velocity * 127` around `velocity`, clamped
    /// to 1–127 (a 0 would be a note-off). The rng is the caller's so a test can seed it.
    pub fn randomize_velocity(&self, velocity: u8, rng: &mut crate::rng::Rng) -> u8 {
        if !self.enabled || self.random_velocity <= 0.0 {
            return velocity;
        }
        let spread = self.random_velocity.clamp(0.0, 1.0) * 127.0;
        (velocity as f32 + rng.next_sym() * spread)
            .round()
            .clamp(1.0, 127.0) as u8
    }
}

/// Fler slag än så är ett eko som aldrig tar slut.
pub const MAX_ECHO_REPEATS: u8 = 3;

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

    /// **Notlängd:** släppet ligger exakt `length_ms * sr / 1000` samples efter anslaget.
    #[test]
    fn the_length_release_lands_exactly_on_the_millisecond_sample() {
        let mut map = map_with(MapZone::default());
        map.length_ms = 250;
        assert_eq!(map.length_release_samples(48_000.0), Some(12_000));
        assert_eq!(map.length_release_samples(44_100.0), Some(11_025));
        map.length_ms = 333;
        assert_eq!(map.length_release_samples(44_100.0), Some(14_685));
        // 0 = av, och en karta som är avstängd rör inte noten.
        map.length_ms = 0;
        assert_eq!(map.length_release_samples(48_000.0), None);
        map.length_ms = 250;
        map.enabled = false;
        assert_eq!(map.length_release_samples(48_000.0), None);
    }

    /// **Eko:** `echo_repeats` = N gånger noten hörs ger N-1 eko, på `k * delay_ms` samples,
    /// varje steg 0,6 gånger så starkt som föregående (0,6 / 0,36 / 0,216).
    #[test]
    fn echoes_land_on_every_delay_step_and_get_quieter() {
        let mut map = map_with(MapZone::default());
        map.delay_ms = 100;
        map.echo_repeats = 3;
        let echoes = map.echo_offsets(48_000.0);
        assert_eq!(echoes.len(), 2);
        assert_eq!(echoes[0].0, 4_800);
        assert_eq!(echoes[1].0, 9_600);
        assert!((echoes[0].1 - 0.6).abs() < 1e-6, "{}", echoes[0].1);
        assert!((echoes[1].1 - 0.36).abs() < 1e-6, "{}", echoes[1].1);

        // Ett eko på 44,1 kHz ligger på samma millisekund, inte samma sampletal.
        map.delay_ms = 50;
        assert_eq!(map.echo_offsets(44_100.0)[0].0, 2_205);

        // Inget eko utan fördröjning, utan upprepningar eller med kartan av.
        map.delay_ms = 0;
        assert!(map.echo_offsets(48_000.0).is_empty());
        map.delay_ms = 100;
        map.echo_repeats = 1;
        assert!(map.echo_offsets(48_000.0).is_empty());
        map.echo_repeats = 2;
        assert_eq!(map.echo_offsets(48_000.0).len(), 1);
        map.enabled = false;
        assert!(map.echo_offsets(48_000.0).is_empty());
    }

    /// **Slumpat anslag:** ±`random_velocity` kring det inkommande anslaget, alltid 1–127.
    #[test]
    fn random_velocity_spreads_within_the_band_and_never_leaves_1_127() {
        let mut map = map_with(MapZone::default());
        map.random_velocity = 0.5;
        let mut rng = crate::rng::Rng::new(0x5EED_1234);
        let (mut lo, mut hi) = (127u8, 1u8);
        for _ in 0..200 {
            let v = map.randomize_velocity(64, &mut rng);
            assert!((1..=127).contains(&v), "utanför 1–127: {v}");
            // ±0,5 * 127 = ±63,5 kring 64 — bandet rymmer hela spannet, så kanten är 1/127.
            lo = lo.min(v);
            hi = hi.max(v);
        }
        println!("random_velocity 0.5, 200 hits: min {lo}, max {hi}");
        assert!(hi > lo, "ingen spridning alls: {lo}..{hi}");

        // Smalare band: varje träff stannar inom ±0,2 * 127 kring 60.
        map.random_velocity = 0.2;
        let (mut lo, mut hi) = (127u8, 1u8);
        for _ in 0..200 {
            let v = map.randomize_velocity(60, &mut rng);
            assert!((v as f32 - 60.0).abs() <= 0.2 * 127.0 + 0.5, "utanför bandet: {v}");
            assert!((1..=127).contains(&v));
            lo = lo.min(v);
            hi = hi.max(v);
        }
        println!("random_velocity 0.2, 200 hits: min {lo}, max {hi}");

        // Av = anslaget orört, och en avstängd karta slumpar inte alls.
        map.random_velocity = 0.0;
        assert_eq!(map.randomize_velocity(64, &mut rng), 64);
        map.random_velocity = 1.0;
        map.enabled = false;
        assert_eq!(map.randomize_velocity(64, &mut rng), 64);
    }

    /// Gamla projekt har inte de nya fälten — de ska öppnas som förut, med transformerna av.
    #[test]
    fn an_old_project_loads_with_the_new_transforms_off() {
        // En zon som den skrevs innan `curve`/`snap_scale`/transformationerna fanns.
        let zone = serde_json::json!({
            "active": false, "low": 0, "high": 127, "transpose": 0,
            "channel": null, "velocity_scale": 1.0
        });
        let old = serde_json::json!({"enabled": true, "chord": 0, "zones": vec![zone; ZONES]});
        let map: KeyMap = serde_json::from_value(old)
            .expect("ett projekt utan de nya fälten ska gå att läsa");
        assert!(map.enabled);
        assert_eq!(map.length_ms, 0);
        assert_eq!(map.delay_ms, 0);
        assert_eq!(map.echo_repeats, 0);
        assert_eq!(map.random_velocity, 0.0);
        assert_eq!(map.length_release_samples(48_000.0), None);
        assert!(map.echo_offsets(48_000.0).is_empty());
    }
}
