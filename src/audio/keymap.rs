//! **Multi-samples: en keymap av zoner** (Fas 8.4/7).
//!
//! En sampler som bara kan spela **ett** sampel per kanal kan inte det ett instrument gör: olika
//! inspelningar för olika register och olika anslag. Modellen här är den etablerade — samma som
//! Kontakt, DirectWave och Abletons Simpler använder — en **lista av zoner**, där varje zon är ett
//! sampel med ett tonhöjds- och ett anslagsintervall.
//!
//! **Tom lista = exakt som förut.** Utan zoner spelar kanalen sitt eget sampel
//! (`ChannelStrip::pcm_audio`), och det är vad ett projekt från före keymappen har. Därför kan den
//! här modulen läggas till utan att något gammalt ljud ändras — och provet
//! `a_channel_without_zones_is_unchanged` håller den regeln.
//!
//! Zonen bär **färdiglästa** kanaler (`Arc`), precis som kanalen gör: motorn kan inte läsa filer,
//! och ljudet sparas aldrig i projektfilen (det ligger redan på disk).

use std::sync::Arc;

/// **En zon**: ett sampel som gäller ett tonhöjds- och ett anslagsintervall.
/// `PartialEq` jämför även ljudet (`Arc<Vec<f32>>` jämförs till **innehåll**), vilket är vad
/// proven vill: två zoner är samma zon om filen, grundtonen, intervallen och ljudet är samma.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleZone {
    /// Filen zonen kom från. Ljudet nedan **läses** ur den och sparas aldrig i projektfilen —
    /// samma regel som för kanalens eget sampel.
    pub sample_path: Option<String>,
    /// Filens kanaler och samplingsfrekvens, färdiglästa. `None` = filen kunde inte läsas (8.5:s
    /// regel: säg det, hitta aldrig på ett ljud).
    pub pcm: Option<(Arc<Vec<f32>>, Arc<Vec<f32>>, u32)>,
    /// Sampelns **grundton**: transponeringen räknas härifrån, så en zon kan spelas i sitt eget
    /// register utan att låta som en smurf.
    pub root: u8,
    pub key_low: u8,
    pub key_high: u8,
    /// Anslagsintervallet i samma enhet som kommandots `velocity` och kanalens anslagsratt
    /// (0,0–1,0) — inte i MIDI:s 1–127, så det finns **ett** tal att jämföra med.
    pub vel_low: f32,
    pub vel_high: f32,
}

impl SampleZone {
    /// En zon för **hela** registret och **alla** anslag: utgångsläget när man lägger till en zon,
    /// så att den hörs direkt och man sedan kan snäva in den.
    pub fn full_range(root: u8) -> Self {
        Self {
            sample_path: None,
            pcm: None,
            root,
            key_low: 0,
            key_high: 127,
            vel_low: 0.0,
            vel_high: 1.0,
        }
    }

    /// Täcker zonen noten **och** anslaget?
    ///
    /// Ett **bakvänt** intervall (`key_low > key_high`) täcker ingenting. Det är ett medvetet val
    /// och inte en glidning: alternativet — att byta plats på ändarna — hade gjort en felskrivning
    /// i ett intervall till en zon som spelar överallt i stället för ingenstans, och det syns
    /// sämre. Gränssnittet klämmer ändarna så paret inte kan bli bakvänt där.
    pub fn covers(&self, note: u8, velocity: f32) -> bool {
        let v = velocity.clamp(0.0, 1.0);
        self.key_low <= note && note <= self.key_high && v >= self.vel_low && v <= self.vel_high
    }

    /// Har zonen ett ljud att spela?
    pub fn is_playable(&self) -> bool {
        self.pcm.as_ref().is_some_and(|(l, _, _)| !l.is_empty())
    }
}

/// **Vilken zon spelar noten?** (Fas 8.4/7)
///
/// Den **sista** zonen i listan som täcker både tonhöjden och anslaget. Listan är alltså
/// prioritetsordning **nedifrån och upp**, precis som i Kontakt och DirectWave: den zon man lade
/// till sist (eller flyttade ned) är den som hörs vid överlapp. Alternativet — "den mest
/// specifika" — låter rimligt men gör svaret beroende av en beräkning man inte ser i listan; här
/// *är* ordningen svaret, och den står i gränssnittet.
///
/// `None` = ingen zon täcker noten, och då spelar kanalens **eget** sampel.
pub fn zone_for_note(zones: &[SampleZone], note: u8, velocity: f32) -> Option<usize> {
    zones.iter().rposition(|z| z.covers(note, velocity))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zon(root: u8, keys: (u8, u8), vels: (f32, f32)) -> SampleZone {
        SampleZone {
            key_low: keys.0,
            key_high: keys.1,
            vel_low: vels.0,
            vel_high: vels.1,
            ..SampleZone::full_range(root)
        }
    }

    #[test]
    fn a_full_range_zone_covers_every_note_and_every_velocity() {
        let z = SampleZone::full_range(60);
        for note in [0u8, 36, 60, 127] {
            for v in [0.0f32, 0.1, 0.5, 1.0] {
                assert!(z.covers(note, v), "not {note}, anslag {v}");
            }
        }
    }

    #[test]
    fn the_key_range_decides_which_register_the_zone_answers_in() {
        let z = zon(60, (48, 60), (0.0, 1.0));
        assert!(z.covers(48, 0.5) && z.covers(60, 0.5));
        assert!(!z.covers(47, 0.5) && !z.covers(61, 0.5));
    }

    #[test]
    fn the_velocity_range_makes_velocity_layers() {
        // Ett svagt och ett starkt lager: samma not, olika sampel.
        let svag = zon(60, (0, 127), (0.0, 0.49));
        let stark = zon(60, (0, 127), (0.5, 1.0));
        assert!(svag.covers(60, 0.2) && !svag.covers(60, 0.7));
        assert!(!stark.covers(60, 0.2) && stark.covers(60, 0.7));
    }

    #[test]
    fn the_last_matching_zone_wins() {
        // Tre zoner över samma not: den sista i listan är den som hörs.
        let zones = vec![
            zon(60, (0, 127), (0.0, 1.0)),
            zon(62, (0, 127), (0.0, 1.0)),
            zon(64, (0, 127), (0.0, 1.0)),
        ];
        assert_eq!(zone_for_note(&zones, 60, 0.5), Some(2));
        // En not bara den första täcker: den svarar, trots att de andra inte gör det.
        let zones = vec![zon(60, (0, 40), (0.0, 1.0)), zon(62, (60, 127), (0.0, 1.0))];
        assert_eq!(zone_for_note(&zones, 36, 0.5), Some(0));
    }

    #[test]
    fn no_zone_covers_the_note_means_the_channels_own_sample() {
        let zones = vec![zon(60, (60, 72), (0.5, 1.0))];
        assert_eq!(zone_for_note(&zones, 36, 0.5), None, "fel register");
        assert_eq!(zone_for_note(&zones, 64, 0.25), None, "fel anslag");
        assert_eq!(zone_for_note(&[], 60, 0.5), None, "ingen keymap alls");
    }

    #[test]
    fn a_backwards_range_covers_nothing_instead_of_everything() {
        let z = zon(60, (72, 48), (0.0, 1.0));
        assert!(!z.covers(60, 0.5));
        assert_eq!(zone_for_note(&[z], 60, 0.5), None);
    }

    #[test]
    fn an_out_of_range_velocity_is_clamped_before_the_comparison() {
        let hog = zon(60, (0, 127), (0.9, 1.0));
        assert!(hog.covers(60, 1.5), "1,5 ska läsas som 1,0");
        assert!(hog.covers(60, 0.95));
        assert!(!hog.covers(60, -0.5), "-0,5 ska läsas som 0,0");
    }
}
