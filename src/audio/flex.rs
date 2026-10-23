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
}
