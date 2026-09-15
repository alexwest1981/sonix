use eframe::egui::Color32;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFormat {
    Clap,
    Vst3,
    Vst2,
    Lv2,
    WineYabridge,
    FlStudioNative,
}

impl PluginFormat {
    pub fn name(&self) -> &'static str {
        match self {
            PluginFormat::Clap => "CLAP (Native)",
            PluginFormat::Vst3 => "VST3 (Native Linux)",
            PluginFormat::Vst2 => "VST 2.4 (Legacy)",
            PluginFormat::Lv2 => "LV2 (Linux Audio)",
            PluginFormat::WineYabridge => "Windows VST3 (Yabridge/Wine)",
            PluginFormat::FlStudioNative => "FL Studio VSTi / Image-Line",
        }
    }

    pub fn badge_color(&self) -> Color32 {
        match self {
            PluginFormat::Clap => Color32::from_rgb(0, 220, 255),          // Cyan
            PluginFormat::Vst3 => Color32::from_rgb(255, 140, 0),          // Orange
            PluginFormat::Vst2 => Color32::from_rgb(220, 180, 50),         // Gold
            PluginFormat::Lv2 => Color32::from_rgb(46, 204, 113),          // Green
            PluginFormat::WineYabridge => Color32::from_rgb(180, 100, 255), // Purple Wine
            PluginFormat::FlStudioNative => Color32::from_rgb(255, 100, 30),// FL Orange/Red
        }
    }
}

/// **Är filen en LV2-bunt?** (2026-09-15)
///
/// Sonix har ingen LV2-värd. Utan den här frågan skickades bunten till CLAP-laddaren och svaret
/// blev `undefined symbol: clap_entry` — ett svar som beskriver **vår** laddare, inte användarens
/// fil. Regeln bor här och inte i kommandoraden så att den går att pröva utan plugin och utan
/// fönster, och så att appen och kommandoraden ger samma svar.
///
/// Både själva bunten (`…/Surge XT.lv2`) och en fil inuti den (`…/Surge XT.lv2/libSurge XT.so`)
/// räknas: en användare pekar på vilket som helst av dem.
pub fn is_lv2_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".lv2")
        || lower.ends_with(".lv2/")
        || lower.contains(".lv2/")
        || lower
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("manifest.ttl") || name.ends_with(".ttl"))
}

#[cfg(test)]
mod lv2_probe_tests {
    use super::is_lv2_path;

    /// Bunten, filen inuti den, och en `.so` som **inte** är LV2.
    #[test]
    fn lv2_paths_are_recognised_and_others_are_not() {
        assert!(is_lv2_path("/usr/lib/lv2/Surge XT.lv2"));
        assert!(is_lv2_path("/tmp/surge/Surge XT.lv2/libSurge XT.so"));
        assert!(is_lv2_path("/tmp/x/Surge XT.lv2/manifest.ttl"));
        assert!(!is_lv2_path("/tmp/surge/Surge XT.clap"));
        assert!(!is_lv2_path("/tmp/surge/Surge XT.vst3"));
        assert!(!is_lv2_path("/home/alex/.vst3/Surge XT Effects.vst3"));
    }

    /// **Ingen påslagen skanningsväg lovar LV2** (2026-09-15). Det är hela poängen med att
    /// stänga av raden: en påslagen väg som hittar filer vi inte kan ladda är ett löfte koden
    /// inte håller. Provet läser **listan**, inte texten — en omskriven beskrivning hade sett
    /// bra ut medan vägen fortfarande skannade.
    #[test]
    fn no_enabled_scan_path_promises_lv2() {
        let manager = crate::audio::plugin_host::PluginManager::default();
        let enabled: Vec<&str> = manager
            .scan_paths
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.path.as_str())
            .collect();
        assert!(
            !enabled
                .iter()
                .any(|p| p.to_ascii_lowercase().contains("lv2")),
            "en påslagen skanningsväg lovar LV2: {enabled:?}"
        );
        // Raden ska stå **kvar**, avstängd och med orsaken: den som undrar var sina LV2-plugins
        // är ska få svaret i gränssnittet i stället för att leta i en logg.
        let lv2 = manager
            .scan_paths
            .iter()
            .find(|p| p.path.contains("lv2"))
            .expect("LV2-raden ska stå kvar, avstängd");
        assert!(!lv2.enabled, "LV2-raden ska vara avstängd");
        assert!(
            !lv2.description.is_empty(),
            "och orsaken ska stå i beskrivningen"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCategory {
    Synth,
    Effect,
    Compressor,
    Equalizer,
    Reverb,
    Delay,
    Distortion,
    PitchCorrection,
    Mastering,
}

impl PluginCategory {
    pub fn label(&self) -> &'static str {
        match self {
            PluginCategory::Synth => "Synthesizer / Instrument",
            PluginCategory::Effect => "Creative Effect / Modulation",
            PluginCategory::Compressor => "Compressor / Dynamics",
            PluginCategory::Equalizer => "Equalizer / Filter",
            PluginCategory::Reverb => "Reverb / Space",
            PluginCategory::Delay => "Delay / Echo",
            PluginCategory::Distortion => "Distortion / Saturation",
            PluginCategory::PitchCorrection => "Pitch Correction / Vocoder",
            PluginCategory::Mastering => "Mastering / Limiter",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PluginCategory::Synth => "🎹",
            PluginCategory::Effect => "✨",
            PluginCategory::Compressor => "🗜",
            PluginCategory::Equalizer => "📊",
            PluginCategory::Reverb => "🌌",
            PluginCategory::Delay => "⏳",
            PluginCategory::Distortion => "🔥",
            PluginCategory::PitchCorrection => "🎤",
            PluginCategory::Mastering => "🛡",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub format: PluginFormat,
    pub category: PluginCategory,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub verified: bool,
    pub verify_note: String,
    pub author_notes: String,
}

#[derive(Debug, Clone)]
pub struct FlStudioPreset {
    pub file_path: String,
    pub preset_name: String,
    pub target_plugin: String,
    pub preset_type: String, // "Channel State", "Mixer Track State", "Plugin Preset"
    pub filesize_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ScanPath {
    pub path: String,
    pub enabled: bool,
    pub description: String,
    pub is_wine: bool,
    pub is_fl_path: bool,
}

#[derive(Debug, Clone)]
pub struct PluginManager {
    pub plugins: Vec<PluginDescriptor>,
    pub presets: Vec<FlStudioPreset>,
    pub scan_paths: Vec<ScanPath>,
    pub active_tab: usize, // 0 = Plugin Databas, 1 = Mappar & Sökvägar, 2 = Importera Plugin / .FST, 3 = FL Studio & Yabridge Assistent
    pub yabridge_installed: bool,
    pub wine_version: String,
    pub search_query: String,
    pub selected_format_filter: Option<PluginFormat>,
    pub selected_category_filter: Option<PluginCategory>,
    pub last_scan_time: String,
    pub scan_status: String,
    /// Last live CLAP inspection (Fas 4.1), if the user asked to load a plugin.
    pub inspection: Option<crate::audio::plugin_host_live::PluginInspection>,
    /// Target stem track for "load into track" (Fas 4.2).
    pub plugin_target_track: usize,
    /// Name of the last plugin instantiated into a track, for status display.
    pub instantiated_plugin: Option<String>,
    /// Filesystem path to a native preset for `clap.preset-load/2` (Fas 4.3).
    pub plugin_preset_location: String,
    // Custom inputs
    pub new_custom_path_input: String,
    pub manual_import_file_input: String,
    pub manual_import_vendor_input: String,
    pub manual_import_category_idx: usize,
}

/// Expanderar ett inledande `~/` mot hemkatalogen. All sökvägslogik bor i
/// [`crate::paths`] (Fas 6.0); denna wrapper finns för att plugin-koden ska
/// slippa importera modulen överallt.
fn expand_tilde(path: &str) -> PathBuf {
    crate::paths::expand_tilde(path)
}

impl Default for PluginManager {
    fn default() -> Self {
        let default_scan_paths = vec![
            ScanPath {
                path: "~/.vst3".to_string(),
                enabled: true,
                description: crate::i18n::t("Linux Användar-VST3").to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "/usr/lib/vst3".to_string(),
                enabled: true,
                description: crate::i18n::t("Linux System-VST3").to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.clap".to_string(),
                enabled: true,
                description: crate::i18n::t("Linux Användar-CLAP").to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "/usr/lib/clap".to_string(),
                enabled: true,
                description: crate::i18n::t("Linux System-CLAP").to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            // **LV2: avstängd, och det står varför** (2026-09-15). Sonix har ingen LV2-värd —
            // bunten skickas till CLAP-laddaren och svarar `undefined symbol: clap_entry`. Att
            // låta raden vara på och beskrivas som ett stöd var ett löfte koden inte höll: filen
            // dök upp i listan och gick inte att ladda. Raden står kvar, avstängd och med
            // orsaken utskriven, i stället för att tyst försvinna — den som undrar var sina
            // LV2-plugins är får svaret här.
            ScanPath {
                path: "/usr/lib/lv2".to_string(),
                enabled: false,
                description: crate::i18n::t("Linux LV2 (ingen LV2-värd i Sonix än)").to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            // Wine & FL Studio Default Paths
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Common Files/VST3".to_string(),
                enabled: true,
                description: crate::i18n::t("Windows VST3 Standard (Yabridge / Wine)").to_string(),
                is_wine: true,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST".to_string(),
                enabled: true,
                description: crate::i18n::t("FL Studio 64-bit VST Plugins (Image-Line)").to_string(),
                is_wine: true,
                is_fl_path: true,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files (x86)/Image-Line/FL Studio/Plugins/VST".to_string(),
                enabled: true,
                description: crate::i18n::t("FL Studio 32-bit VST Plugins (Legacy)").to_string(),
                is_wine: true,
                is_fl_path: true,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/VstPlugins".to_string(),
                enabled: true,
                description: crate::i18n::t("Windows VST2 Standard (Yabridge)").to_string(),
                is_wine: true,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets".to_string(),
                enabled: true,
                description: crate::i18n::t("FL Studio .FST Presets (Kanaler & Mixer)").to_string(),
                is_wine: true,
                is_fl_path: true,
            },
        ];

        let mut mgr = Self {
            plugins: Vec::new(),
            presets: Vec::new(),
            scan_paths: default_scan_paths,
            active_tab: 0,
            yabridge_installed: detect_yabridge_installed(),
            wine_version: detect_wine_version(),
            search_query: String::new(),
            selected_format_filter: None,
            selected_category_filter: None,
            last_scan_time: crate::i18n::t("Inte skannad ännu").to_string(),
            scan_status: crate::i18n::t("Redo").to_string(),
            inspection: None,
            plugin_target_track: 0,
            instantiated_plugin: None,
            plugin_preset_location: String::new(),
            new_custom_path_input: String::new(),
            manual_import_file_input: String::new(),
            manual_import_vendor_input: String::new(),
            manual_import_category_idx: 0,
        };

        mgr.scan_disk();
        mgr
    }
}

/// Detects the installed Wine version, or reports that Wine is unavailable.
pub fn detect_wine_version() -> String {
    let candidates = ["wine", "wine64"];
    for exe in candidates {
        if let Ok(out) = std::process::Command::new(exe).arg("--version").output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    crate::i18n::t("Wine ej installerat").to_string()
}

/// Returns true if the `yabridgectl` helper is available in PATH.
pub fn detect_yabridge_installed() -> bool {
    std::process::Command::new("yabridgectl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Runs `yabridgectl sync` for real and returns its combined output.
pub fn run_yabridge_sync() -> Result<String, String> {
    let out = std::process::Command::new("yabridgectl")
        .arg("sync")
        .output()
        .map_err(|e| crate::tstatus!("Kunde inte köra yabridgectl: {}", e))?;
    let mut text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !err.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if out.status.success() {
        Ok(if text.is_empty() { crate::i18n::t("Yabridge-synk klar.").to_string() } else { text })
    } else {
        Err(if text.is_empty() { crate::i18n::t("yabridgectl sync misslyckades.").to_string() } else { text })
    }
}

/// Extracts a version number embedded in a plugin file name (e.g. "Serum 1.3.5"),
/// returning "—" when none is present instead of inventing one.
fn extract_version(file_name: &str) -> String {
    let chars: Vec<char> = file_name.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut dots = 0;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                if chars[i] == '.' {
                    dots += 1;
                }
                i += 1;
            }
            if dots >= 1 {
                let token: String = chars[start..i].iter().collect();
                let token = token.trim_matches('.').to_string();
                if !token.is_empty() {
                    return token;
                }
            }
        } else {
            i += 1;
        }
    }
    crate::i18n::t("—").to_string()
}

/// Verifies that a plugin binary/bundle is a real, loadable artifact.
/// Returns (verified, human-readable note).
pub fn verify_plugin_artifact(path: &Path) -> (bool, String) {
    if !path.exists() {
        return (false, crate::i18n::t("Filen finns inte på disken").to_string());
    }
    if path.is_dir() {
        // VST3/LV2/CLAP bundles: look for a native shared object inside.
        let has_so = walk_find_so(path, 0);
        return if has_so {
            (true, crate::i18n::t("Plugin-paket med binär hittad").to_string())
        } else {
            (false, crate::i18n::t("Paket saknar binär (.so)").to_string())
        };
    }
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() >= 4 => {
            if bytes.starts_with(&[0x7f, b'E', b'L', b'F']) {
                (true, crate::i18n::t("Giltig Linux-binär (ELF)").to_string())
            } else if bytes.starts_with(b"MZ") {
                (true, crate::i18n::t("Giltig Windows-binär (PE) – kräver Yabridge").to_string())
            } else {
                (false, crate::i18n::t("Okänt filformat (inte ELF/PE)").to_string())
            }
        }
        _ => (false, crate::i18n::t("Kunde inte läsa filen").to_string()),
    }
}

fn walk_find_so(dir: &Path, depth: usize) -> bool {
    if depth > 4 {
        return false;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if walk_find_so(&p, depth + 1) {
                return true;
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("so") {
            return true;
        }
    }
    false
}

impl PluginManager {

    /// Scans configured real directories on the file system for plugin bundles & files.
    pub fn scan_disk(&mut self) {
        let mut found_count = 0;
        // Drop previously discovered entries so a rescan reflects the real disk.
        self.plugins.retain(|p| !p.id.starts_with("disc_"));
        self.presets.retain(|p| !p.file_path.is_empty() && std::path::Path::new(&expand_tilde(&p.file_path)).exists());
        let paths_to_scan: Vec<(PathBuf, bool)> = self
            .scan_paths
            .iter()
            .filter(|p| p.enabled)
            .map(|p| (expand_tilde(&p.path), p.is_wine))
            .collect();

        for (expanded, is_wine) in paths_to_scan {
            if !expanded.exists() {
                continue;
            }

            self.scan_directory_recursive(&expanded, 0, &mut found_count, is_wine);
        }

        self.last_scan_time = "Nyss (Auto-skannad)".to_string();
        self.scan_status = crate::tstatus!(
            "Skanning klar. {} plugins och {} presets identifierade på disk.",
            self.plugins.len(),
            self.presets.len()
        );
    }

    fn scan_directory_recursive(&mut self, dir: &Path, depth: usize, count: &mut usize, is_wine: bool) {
        if depth > 4 {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                // If it ends with .vst3, .lv2, or .clap, treat the directory itself as a plugin bundle
                if file_name.ends_with(".vst3") || file_name.ends_with(".lv2") || file_name.ends_with(".clap") {
                    self.register_discovered_file(&path, &file_name, is_wine);
                    *count += 1;
                } else {
                    self.scan_directory_recursive(&path, depth + 1, count, is_wine);
                }
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ["so", "dll", "vst3", "clap", "fst"].contains(&ext.to_lowercase().as_str()) {
                    self.register_discovered_file(&path, &file_name, is_wine);
                    *count += 1;
                }
            }
        }
    }

    fn register_discovered_file(&mut self, path: &Path, file_name: &str, is_wine: bool) {
        let path_str = path.to_string_lossy().to_string();

        // Check if already in plugins or presets
        if self.plugins.iter().any(|p| p.file_path == path_str) || self.presets.iter().any(|pr| pr.file_path == path_str) {
            return;
        }

        let lower = file_name.to_lowercase();

        if lower.ends_with(".fst") {
            let name_clean = file_name.trim_end_matches(".fst").to_string();
            let target_plugin = if lower.contains("gross") {
                "Gross Beat".to_string()
            } else if lower.contains("sytrus") {
                "Sytrus".to_string()
            } else if lower.contains("maximus") {
                "Maximus".to_string()
            } else {
                "FL Studio Generic".to_string()
            };

            self.presets.push(FlStudioPreset {
                file_path: path_str,
                preset_name: name_clean,
                target_plugin,
                preset_type: "FL State (.fst)".to_string(),
                filesize_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            });
            return;
        }

        let (format, vendor) = if is_wine || path_str.contains(".wine") || lower.ends_with(".dll") {
            if lower.contains("image-line") || lower.contains("fl studio") || lower.starts_with("sytrus") || lower.starts_with("harmor") || lower.starts_with("gross") {
                (PluginFormat::FlStudioNative, "Image-Line (FL Studio)".to_string())
            } else {
                (PluginFormat::WineYabridge, "Windows VST (Yabridge)".to_string())
            }
        } else if lower.ends_with(".clap") {
            (PluginFormat::Clap, "CLAP Native".to_string())
        } else if lower.ends_with(".vst3") {
            (PluginFormat::Vst3, "VST3 Native".to_string())
        } else if lower.ends_with(".lv2") {
            (PluginFormat::Lv2, "LV2 Linux".to_string())
        } else {
            (PluginFormat::Vst2, "VST2 Linux".to_string())
        };

        let clean_name = file_name
            .trim_end_matches(".vst3")
            .trim_end_matches(".clap")
            .trim_end_matches(".dll")
            .trim_end_matches(".so")
            .trim_end_matches(".lv2")
            .replace('_', " ");

        let category = if lower.contains("synth") || lower.contains("generator") || lower.contains("vital") || lower.contains("diva") || lower.contains("sytrus") || lower.contains("harmor") || lower.contains("serum") {
            PluginCategory::Synth
        } else if lower.contains("eq") || lower.contains("filter") || lower.contains("pro-q") {
            PluginCategory::Equalizer
        } else if lower.contains("comp") || lower.contains("limiter") || lower.contains("maximus") || lower.contains("ott") {
            PluginCategory::Compressor
        } else if lower.contains("reverb") || lower.contains("space") || lower.contains("room") {
            PluginCategory::Reverb
        } else if lower.contains("delay") || lower.contains("echo") {
            PluginCategory::Delay
        } else if lower.contains("dist") || lower.contains("saturat") || lower.contains("drive") || lower.contains("decap") {
            PluginCategory::Distortion
        } else if lower.contains("vocod") || lower.contains("pitch") || lower.contains("autotune") {
            PluginCategory::PitchCorrection
        } else {
            PluginCategory::Effect
        };

        let (verified, verify_note) = verify_plugin_artifact(path);
        let file_size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        self.plugins.push(PluginDescriptor {
            id: format!("disc_{}", self.plugins.len()),
            name: clean_name,
            vendor,
            version: extract_version(&file_name),
            format,
            category,
            file_path: path_str,
            file_size_bytes,
            verified,
            verify_note: verify_note.clone(),
            author_notes: crate::tstatus!("Upptäcktes automatiskt under skanning av {}. {}", path.display(), verify_note),
        });
    }

    /// Manually import a single plugin file or bundle
    pub fn import_file(&mut self, raw_path: &str, custom_vendor: &str, category: PluginCategory) -> Result<PluginDescriptor, String> {
        let expanded = expand_tilde(raw_path.trim());
        let file_name = expanded.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| crate::i18n::t("Okänt Plugin").to_string());
        let lower = file_name.to_lowercase();

        if !expanded.exists() {
            return Err(crate::tstatus!("Filen finns inte: {}", raw_path));
        }

        if lower.ends_with(".fst") {
            let preset_name = file_name.trim_end_matches(".fst").to_string();
            let p = FlStudioPreset {
                file_path: raw_path.to_string(),
                preset_name: preset_name.clone(),
                target_plugin: if custom_vendor.is_empty() { "FL Studio Generic".to_string() } else { custom_vendor.to_string() },
                preset_type: "Importerad FL Preset (.fst)".to_string(),
                filesize_bytes: std::fs::metadata(&expanded).map(|m| m.len()).unwrap_or(0),
            };
            self.presets.push(p);
            return Err(crate::tstatus!("Filen '{}' importerades som en FL Studio .FST preset!", preset_name));
        }

        let format = if lower.ends_with(".clap") {
            PluginFormat::Clap
        } else if lower.ends_with(".vst3") {
            if raw_path.contains(".wine") || raw_path.contains("drive_c") { PluginFormat::WineYabridge } else { PluginFormat::Vst3 }
        } else if lower.ends_with(".dll") {
            if lower.contains("fl") || lower.contains("sytrus") || lower.contains("harmor") || lower.contains("gross") {
                PluginFormat::FlStudioNative
            } else {
                PluginFormat::WineYabridge
            }
        } else if lower.ends_with(".lv2") {
            PluginFormat::Lv2
        } else {
            PluginFormat::Vst2
        };

        let vendor = if !custom_vendor.trim().is_empty() {
            custom_vendor.trim().to_string()
        } else if format == PluginFormat::FlStudioNative {
            "Image-Line (FL Studio)".to_string()
        } else {
            "Importerad Tredjepart".to_string()
        };

        let clean_name = file_name
            .trim_end_matches(".vst3")
            .trim_end_matches(".clap")
            .trim_end_matches(".dll")
            .trim_end_matches(".so")
            .trim_end_matches(".lv2")
            .replace('_', " ");

        let (verified, verify_note) = verify_plugin_artifact(&expanded);
        let desc = PluginDescriptor {
            id: format!("custom_{}", self.plugins.len()),
            name: clean_name,
            vendor,
            version: extract_version(&file_name),
            format,
            category,
            file_path: raw_path.to_string(),
            file_size_bytes: std::fs::metadata(&expanded).map(|m| m.len()).unwrap_or(0),
            verified,
            verify_note: verify_note.clone(),
            author_notes: crate::tstatus!("Manuellt importerad pluginfil. {}", verify_note),
        };

        self.plugins.push(desc.clone());
        self.scan_status = crate::tstatus!("✔ Importerade '{}' ({}) – {}", desc.name, desc.format.name(), verify_note);
        Ok(desc)
    }

    pub fn add_scan_path(&mut self, path: String, description: String, is_wine: bool, is_fl_path: bool) {
        let trimmed = path.trim().to_string();
        if !trimmed.is_empty() && !self.scan_paths.iter().any(|p| p.path == trimmed) {
            self.scan_paths.push(ScanPath {
                path: trimmed,
                enabled: true,
                description,
                is_wine,
                is_fl_path,
            });
        }
    }

    pub fn remove_scan_path(&mut self, index: usize) {
        if index < self.scan_paths.len() {
            self.scan_paths.remove(index);
        }
    }

    pub fn rescan(&mut self) {
        self.scan_disk();
        self.yabridge_installed = detect_yabridge_installed();
        self.wine_version = detect_wine_version();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("sonix_plugin_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn manager_does_not_fabricate_plugins() {
        let mgr = PluginManager::default();
        // Only real files discovered on disk are listed — never hardcoded names.
        assert!(!mgr.plugins.iter().any(|p| p.name.contains("Sytrus")));
        assert!(!mgr.scan_paths.is_empty());
    }

    #[test]
    fn verify_detects_elf_and_rejects_garbage() {
        let elf = temp_file("fake.clap", &[0x7f, b'E', b'L', b'F', 0, 0, 0, 0]);
        let (ok, _) = verify_plugin_artifact(&elf);
        assert!(ok, "ELF magic should verify");

        let bad = temp_file("bad.clap", b"not a plugin");
        let (ok2, _) = verify_plugin_artifact(&bad);
        assert!(!ok2, "garbage should not verify");
    }

    #[test]
    fn manual_import_requires_real_file() {
        let mut mgr = PluginManager::default();
        let res = mgr.import_file("/nonexistent/path/Nexus.dll", "reFX", PluginCategory::Synth);
        assert!(res.is_err());
    }

    #[test]
    fn manual_plugin_import_records_real_file() {
        let mut mgr = PluginManager::default();
        let initial_len = mgr.plugins.len();
        let path = temp_file("Nexus.dll", b"MZ\x90\x00fakepe");

        let res = mgr.import_file(path.to_str().unwrap(), "reFX", PluginCategory::Synth);
        assert!(res.is_ok());
        assert_eq!(mgr.plugins.len(), initial_len + 1);
        let imported = mgr.plugins.last().unwrap();
        assert_eq!(imported.name, "Nexus");
        assert_eq!(imported.vendor, "reFX");
        assert_eq!(imported.format, PluginFormat::WineYabridge);
        assert!(imported.verified);
    }

    #[test]
    fn test_fst_preset_import() {
        let mut mgr = PluginManager::default();
        let initial_presets = mgr.presets.len();
        let path = temp_file("Custom808.fst", b"FSTDATA");

        let res = mgr.import_file(path.to_str().unwrap(), "Image-Line", PluginCategory::Distortion);
        assert!(res.is_err()); // Returns Err with Swedish notification explaining it went to presets
        assert_eq!(mgr.presets.len(), initial_presets + 1);
        let preset = mgr.presets.last().unwrap();
        assert_eq!(preset.preset_name, "Custom808");
    }

    fn isolated_manager(scan_dir: &std::path::Path) -> PluginManager {
        let mut mgr = PluginManager::default();
        mgr.plugins.clear();
        mgr.presets.clear();
        mgr.scan_paths = vec![ScanPath {
            path: scan_dir.to_string_lossy().to_string(),
            enabled: true,
            description: "test".to_string(),
            is_wine: false,
            is_fl_path: false,
        }];
        mgr
    }

    #[test]
    fn scan_disk_discovers_and_classifies_plugins() {
        let dir = std::env::temp_dir().join("sonix_plugin_scan_test");
        std::fs::create_dir_all(&dir).unwrap();
        let clap = dir.join("SuperSynth.clap");
        std::fs::write(&clap, [0x7f, b'E', b'L', b'F', 0, 0, 0, 0]).unwrap();
        let vst3 = dir.join("RoomReverb.vst3");
        std::fs::write(&vst3, b"not really a plugin").unwrap();

        let mut mgr = isolated_manager(&dir);
        mgr.scan_disk();

        let found_clap = mgr.plugins.iter().find(|p| p.name == "SuperSynth").expect("clap discovered");
        assert_eq!(found_clap.format, PluginFormat::Clap);
        assert_eq!(found_clap.category, PluginCategory::Synth);
        assert!(found_clap.verified, "ELF magic should verify");

        let found_vst3 = mgr.plugins.iter().find(|p| p.name == "RoomReverb").expect("vst3 discovered");
        assert_eq!(found_vst3.format, PluginFormat::Vst3);
        assert_eq!(found_vst3.category, PluginCategory::Reverb);
        assert!(!found_vst3.verified, "garbage should not verify");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scan_disk_classifies_fst_as_preset_not_plugin() {
        let dir = std::env::temp_dir().join("sonix_plugin_scan_fst");
        std::fs::create_dir_all(&dir).unwrap();
        let fst = dir.join("GrossBeatPattern.fst");
        std::fs::write(&fst, b"FSTDATA").unwrap();

        let mut mgr = isolated_manager(&dir);
        mgr.scan_disk();

        assert!(mgr.plugins.is_empty(), "a .fst must never be listed as a plugin");
        let preset = mgr.presets.iter().find(|p| p.preset_name == "GrossBeatPattern").expect("fst preset discovered");
        assert_eq!(preset.target_plugin, "Gross Beat");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

