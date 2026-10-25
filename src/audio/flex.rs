//! **FLEX**: användarens egna ljud, sparade mellan sessioner.
//!
//! Ett förval är exakt de parametrar som `apply_settings` skriver in i reglagen — inget mer.
//! Därför kan ett sparat ljud återställas utan att någon parameter glöms: det finns ingen
//! andra lista att hålla i synk.
//!
//! Filen ligger i appens data-mapp (`~/.local/share/sonix/flex_presets.json`) och skrivs hel:
//! ett halvskrivet förvalsbibliotek är värre än inget, så sparningen går via en tempfil som
//! byts in.

use crate::audio::command::PresetSettings;
use std::path::{Path, PathBuf};

/// Ett sparat ljud: namnet användaren ser och parametrarna bakom.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FlexPreset {
    pub name: String,
    pub settings: PresetSettings,
}

/// **Fabriken**: åtta ljud som följer med appen. De ligger i koden, inte i användarens fil —
/// annars hade de gått att radera och kommit tillbaka vid nästa start, och "återställ"
/// betyder olika saker beroende på var man tittar.
pub fn factory_presets() -> Vec<FlexPreset> {
    let adsr = |a: f32, d: f32, s: f32, r: f32| crate::audio::envelope::AdsrParams {
        attack: a,
        decay: d,
        sustain: s,
        release: r,
    };
    let patch = |name: &str, waveform, a, d, su, r, cutoff, res, s, uni, det, drv| FlexPreset {
        name: name.to_string(),
        settings: PresetSettings {
            waveform,
            adsr: adsr(a, d, su, r),
            filter: crate::audio::filter::FilterParams { cutoff, resonance: res },
            filter_env_amount: 0.0,
            sub: s,
            unison: uni,
            detune: det,
            glide: 0.0,
            drive: drv,
            harmonics: [0.0; crate::audio::synth::HARMONIC_COUNT],
        },
    };
    use crate::audio::command::Waveform as W;
    vec![
        patch("Init — ren sågtand", W::Saw, 0.005, 0.2, 0.7, 0.25, 12_000.0, 0.7, 0.0, 1, 0.0, 0.0),
        patch("Bas — sub och kort släpp", W::Saw, 0.002, 0.15, 0.6, 0.08, 900.0, 1.6, 0.5, 1, 0.0, 0.25),
        patch("Pad — bred och långsam", W::Saw, 0.6, 1.2, 0.85, 1.6, 3_200.0, 0.6, 0.0, 3, 0.12, 0.0),
        patch("Pluck — kort och filtrerad", W::Triangle, 0.001, 0.28, 0.0, 0.2, 4_000.0, 1.1, 0.15, 1, 0.0, 0.1),
        patch("Lead — unison och drive", W::Square, 0.01, 0.25, 0.8, 0.18, 6_500.0, 1.8, 0.2, 2, 0.08, 0.35),
        patch("Stråke — Sakura", W::Bowed, 0.25, 0.5, 0.9, 0.7, 5_000.0, 0.9, 0.0, 1, 0.0, 0.0),
        patch("Gitarr — Slayer", W::Guitar, 0.002, 0.6, 0.35, 0.4, 7_000.0, 0.8, 0.0, 1, 0.0, 0.15),
        patch("Klocka — inharmonisk", W::Inharmonic, 0.001, 0.9, 0.0, 0.8, 9_000.0, 0.5, 0.0, 1, 0.0, 0.0),
    ]
}

/// Var biblioteket bor. Skapas inte här — `save_all` gör mappen när den behövs.
pub fn store_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("sonix").join("flex_presets.json"))
}

/// Läser biblioteket. En trasig eller saknad fil ger en tom lista — appen ska starta även om
/// förvalsfilen har blivit skadad, och ett fel här får inte kosta projektet.
pub fn load_all(path: &Path) -> Vec<FlexPreset> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Skriver hela biblioteket. Tempfil + rename, så en krasch mitt i skrivningen lämnar den
/// gamla filen intakt.
pub fn save_all(path: &Path, presets: &[FlexPreset]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("kunde inte skapa {}: {e}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(presets).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("kunde inte skriva {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("kunde inte byta in {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::command::PresetSettings;
    use crate::audio::envelope::AdsrParams;
    use crate::audio::filter::FilterParams;

    fn settings(cutoff: f32) -> PresetSettings {
        PresetSettings {
            waveform: crate::audio::command::Waveform::Saw,
            adsr: AdsrParams { attack: 0.01, decay: 0.2, sustain: 0.6, release: 0.3 },
            filter: FilterParams { cutoff, resonance: 1.5 },
            filter_env_amount: 0.4,
            sub: 0.2,
            unison: 3,
            detune: 12.0,
            glide: 0.05,
            drive: 0.0,
            harmonics: crate::audio::synth::HARMONICS_DEFAULT,
        }
    }

    /// **Ett sparat ljud kommer tillbaka som det var** — alla parametrar, inte bara de
    /// uppenbara. Och en saknad fil ger en tom lista i stället för panik.
    #[test]
    fn presets_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("sonix-flex-{}", std::process::id()));
        let path = dir.join("flex_presets.json");
        let _ = std::fs::remove_file(&path);

        assert!(load_all(&path).is_empty(), "saknad fil = tomt bibliotek");

        let presets = vec![
            FlexPreset { name: "Min bas".to_string(), settings: settings(420.0) },
            FlexPreset { name: "Min pad".to_string(), settings: settings(1900.0) },
        ];
        save_all(&path, &presets).unwrap();

        let back = load_all(&path);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "Min bas");
        assert_eq!(back[0].settings.filter.cutoff, 420.0);
        assert_eq!(back[1].settings.filter.cutoff, 1900.0);
        assert_eq!(back[1].settings.unison, 3);
        assert_eq!(back[1].settings.filter_env_amount, 0.4);

        // En fil som inte är JSON får inte kosta appens start.
        std::fs::write(&path, "{ trasig").unwrap();
        assert!(load_all(&path).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// **Fabriken har distinkta ljud.** Åtta förval som låter likadant är åtta rader skräp;
    /// provet kräver att inget par delar både vågform och filter.
    #[test]
    fn the_factory_presets_are_distinct() {
        let presets = factory_presets();
        assert_eq!(presets.len(), 8);
        for (i, a) in presets.iter().enumerate() {
            for b in presets.iter().skip(i + 1) {
                let same = a.settings.waveform == b.settings.waveform
                    && (a.settings.filter.cutoff - b.settings.filter.cutoff).abs() < 1.0
                    && (a.settings.adsr.attack - b.settings.adsr.attack).abs() < 1e-4;
                assert!(!same, "{} och {} är samma ljud", a.name, b.name);
            }
        }
    }

}
