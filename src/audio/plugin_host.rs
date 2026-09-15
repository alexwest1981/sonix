use eframe::egui::Color32;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FlStudioPreset {
    pub file_path: String,
    pub preset_name: String,
    pub target_plugin: String,
    pub preset_type: String, // "Channel State", "Mixer Track State", "Plugin Preset"
    pub filesize_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Standardläget **skannar disken** — det är vad en ny manager alltid har gjort, och vad
    /// gränssnittet räknar med när användaren trycker "Skanna". Vill man slippa skanningen (t.ex.
    /// för att läsa en sparad databas i stället) används [`PluginManager::without_scan`] eller
    /// [`PluginManager::with_cached_database`].
    fn default() -> Self {
        let mut mgr = Self::without_scan();
        mgr.scan_disk();
        mgr
    }
}

impl PluginManager {
    /// Standardläget **utan** diskskanning: sökvägarna och tomma listor.
    pub fn without_scan() -> Self {
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

        PluginManager {
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
        }
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

/// **Plugin-databasens fil** (Fas 6.0, stängd 2026-09-15): `plugin_db.json` i konfigmappen.
pub fn plugin_db_file() -> PathBuf {
    crate::paths::paths().config_dir().join("plugin_db.json")
}

/// **Formatet på disk.** Versionen är inte dekoration: en fil skriven av en nyare Sonix ska
/// **läsas som ingenting** i stället för att läsas halvt — en plugin-databas som tappar hälften av
/// sina fält ser ut som en tom databas, och då letar man felet i skanningen i stället för i filen.
pub const PLUGIN_DB_VERSION: u32 = 1;

/// Plugin-databasen så som den ligger på disk.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginDb {
    pub version: u32,
    /// Unix-sekunder när filen skrevs. Åldern visas i gränssnittet, så en gammal lista inte ser
    /// ut som en ny.
    pub saved_at_unix: u64,
    pub scan_paths: Vec<ScanPath>,
    pub plugins: Vec<PluginDescriptor>,
    pub presets: Vec<FlStudioPreset>,
}

/// **Vad starten gör med plugin-databasen** — se [`startup_plan`].
#[derive(Debug, Clone, PartialEq)]
pub enum StartupSource {
    /// Filen fanns och lästes: **ingen skanning**, listan är den som var.
    Cached(PluginDb),
    /// Skanna disken. `Some(skäl)` när en fil fanns men inte kunde användas — skälet ska visas,
    /// för annars ser en trasig fil ut som en tom och nästa skanning verkar omotiverad.
    Scan(Option<String>),
}

/// Avgör vad starten ska göra — **ren funktion**, så beslutet går att pröva utan en konfigmapp
/// och utan att röra användarens filer.
pub fn startup_plan(loaded: Result<Option<PluginDb>, String>) -> StartupSource {
    match loaded {
        Ok(Some(db)) => StartupSource::Cached(db),
        Ok(None) => StartupSource::Scan(None),
        Err(e) => StartupSource::Scan(Some(e)),
    }
}

/// Sekunder sedan en tidsstämpel — ren funktion, så åldersvisningen går att pröva.
pub fn age_seconds(now_unix: u64, saved_at_unix: u64) -> u64 {
    now_unix.saturating_sub(saved_at_unix)
}

/// Åldern i ord: "nyss", "N minuter sedan", "N timmar sedan", "N dygn sedan".
pub fn age_text(seconds: u64) -> String {
    match seconds {
        0..=59 => crate::i18n::t("nyss").to_string(),
        60..=3_599 => crate::tstatus!("{} minuter sedan", seconds / 60),
        3_600..=86_399 => crate::tstatus!("{} timmar sedan", seconds / 3_600),
        _ => crate::tstatus!("{} dygn sedan", seconds / 86_400),
    }
}

impl PluginManager {

    /// **Startläget: med databasen från förra gången om den finns** (2026-09-15).
    ///
    /// En plugin-skanning är det långsammaste appen gör vid start, och listan ändras sällan. Finns
    /// filen läses den därför i stället för att disken gås igenom. Går den **inte** att läsa står
    /// skälet i statusraden — annars hade en trasig fil sett ut precis som en tom, och nästa
    /// skanning hade verkat helt omotiverad.
    ///
    /// `Default` lämnas **orörd och utan fil-IO**: proven ska inte läsa användarens riktiga
    /// konfigmapp.
    pub fn with_cached_database() -> Self {
        match startup_plan(Self::load_from_disk()) {
            // **Cachen används utan att disken rörs.** Därför är `without_scan` en egen
            // konstruktor: `Default::default()` skannar, så en "cache" byggd på den hade skannat
            // först och läst in efteråt — och då hade den inte sparat någonting alls.
            StartupSource::Cached(db) => {
                let mut manager = Self::without_scan();
                manager.apply_db(db);
                manager
            }
            // Ingen fil (första starten) eller en fil som inte gick att läsa: skanna disken, som
            // förut. Skälet sätts **efter** skanningen, annars hade skanningsraden skrivit över det.
            StartupSource::Scan(reason) => {
                let mut manager = Self::default();
                if let Some(e) = reason {
                    manager.scan_status = crate::tstatus!(
                        "Plugin-databasen kunde inte läsa ({}) — skannade om disken.",
                        e
                    );
                }
                manager
            }
        }
    }

    /// Databasen som den ser ut nu.
    pub fn to_db(&self) -> PluginDb {
        PluginDb {
            version: PLUGIN_DB_VERSION,
            saved_at_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            scan_paths: self.scan_paths.clone(),
            plugins: self.plugins.clone(),
            presets: self.presets.clone(),
        }
    }

    /// Skriver databasen **atomiskt** (temp + rename), samma regel som autosaven: en halvskriven
    /// fil är värre än ingen fil, för den läses vid nästa start och ser ut som en tom databas.
    pub fn save_to(&self, file: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.to_db()).map_err(|e| e.to_string())?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, file).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(())
    }

    /// Skriver till den riktiga platsen — se [`plugin_db_file`].
    pub fn save_to_disk(&self) -> Result<(), String> {
        self.save_to(&plugin_db_file())
    }

    /// Läser en databasfil. `Ok(None)` = filen finns inte (första starten, **inte** ett fel).
    /// `Err` = filen finns men går inte att använda, och **skälet står i felet**.
    pub fn load_from(file: &Path) -> Result<Option<PluginDb>, String> {
        if !file.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let db: PluginDb = serde_json::from_str(&text)
            .map_err(|e| format!("{}: inte en plugin-databas ({e})", file.display()))?;
        if db.version != PLUGIN_DB_VERSION {
            return Err(format!(
                "{}: version {} (jag kan läsa {}) — filen lämnas orörd",
                file.display(),
                db.version,
                PLUGIN_DB_VERSION
            ));
        }
        Ok(Some(db))
    }

    /// Läser från den riktiga platsen — se [`plugin_db_file`].
    pub fn load_from_disk() -> Result<Option<PluginDb>, String> {
        Self::load_from(&plugin_db_file())
    }

    /// Lägger in en läst databas: sökvägarna, pluginsen och presetsen.
    pub fn apply_db(&mut self, db: PluginDb) {
        if !db.scan_paths.is_empty() {
            self.scan_paths = db.scan_paths;
        }
        self.plugins = db.plugins;
        self.presets = db.presets;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.last_scan_time = crate::tstatus!(
            "{} (från databasen)",
            age_text(age_seconds(now, db.saved_at_unix))
        );
    }

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
        // **Databasen sparas direkt efter skanningen** (2026-09-15). Förut levde den bara i
        // minnet, så varje start skannade om disken — och en plugin-skanning är den långsammaste
        // saken appen gör vid start. Ett fel att spara får **inte** tigas bort: det står i
        // statusraden tillsammans med resultatet, för en databas som inte sparas ser ut som en
        // databas som fungerar, ända till nästa start.
        if let Err(e) = self.save_to_disk() {
            self.scan_status = crate::tstatus!(
                "Skanning klar ({} plugins) — men databasen kunde inte sparas: {}",
                self.plugins.len(),
                e
            );
        }
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
        // Även **föräldern** till ett nästlat namn: testerna för plugin-databasen lägger sin fil i
        // en egen underkatalog, och en hjälpare som bara gör den yttre katalogen hade fällt dem på
        // "No such file or directory" — ett fel i provet, inte i koden det prövar.
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// **Databasen överlever en omstart** (Fas 6.0, 2026-09-15): en manager skrivs till en fil och
    /// en **ny** manager läser den. Provet jämför **fälten**, inte antalet poster — en fil som
    /// tappar format, sökväg eller verifieringsnoten är en databas som ser full ut och ändå inte
    /// går att använda.
    #[test]
    fn the_plugin_database_survives_a_restart() {
        let file = temp_file("db_roundtrip/plugin_db.json", b"{}");
        let mut skriven = PluginManager::default();
        // Tomt från början: standardlistan innehåller redan exempelposter, och provet ska räkna
        // **sina egna** poster — annars mäter det standardlistans längd och kallar det en rundtur.
        skriven.plugins.clear();
        skriven.plugins.push(PluginDescriptor {
            id: "disc_0".to_string(),
            name: "SuperSynth".to_string(),
            vendor: "Mock Audio".to_string(),
            version: "1.2.3".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "/tmp/SuperSynth.clap".to_string(),
            file_size_bytes: 4242,
            verified: true,
            verify_note: "hittad på disk".to_string(),
            author_notes: "min anteckning".to_string(),
        });
        skriven.last_scan_time = "Nyss".to_string();
        skriven.save_to(&file).expect("skrivningen ska lyckas");

        let db = PluginManager::load_from(&file)
            .expect("läsningen ska lyckas")
            .expect("filen ska finnas");
        assert_eq!(db.version, PLUGIN_DB_VERSION);
        let mut läst = PluginManager::default();
        läst.apply_db(db);
        assert_eq!(läst.plugins.len(), skriven.plugins.len());
        assert_eq!(läst.plugins.len(), 1);
        let p = &läst.plugins[0];
        assert_eq!(p.name, "SuperSynth");
        assert_eq!(p.version, "1.2.3");
        assert_eq!(p.format, PluginFormat::Clap);
        assert_eq!(p.category, PluginCategory::Synth);
        assert_eq!(p.file_path, "/tmp/SuperSynth.clap");
        assert_eq!(p.file_size_bytes, 4242);
        assert!(p.verified);
        assert_eq!(p.author_notes, "min anteckning");
        // Sökvägarna följer också med — annars hade en egen tillagd mapp försvunnit varje start.
        assert_eq!(läst.scan_paths.len(), skriven.scan_paths.len());
        // Och tiden står kvar som "från databasen", alltså syns det att listan inte är ny.
        assert!(läst.last_scan_time.contains("databasen"), "{}", läst.last_scan_time);
        // Ingen temp-fil kvar efter den atomiska skrivningen.
        assert!(!file.with_extension("json.tmp").exists(), "temp-filen ska vara borta");
    }

    /// **En saknad fil är inte ett fel, men en trasig fil är det — och skälet står i klartext**
    /// (Fas 6.0). Skillnaden är hela poängen: en första start ska vara tyst, en trasig fil ska
    /// gå att begripa.
    #[test]
    fn a_missing_or_broken_plugin_database_is_handled_with_a_reason() {
        let saknad = std::env::temp_dir().join("sonix_plugin_tests/finns_inte/plugin_db.json");
        let _ = std::fs::remove_file(&saknad);
        assert!(
            PluginManager::load_from(&saknad)
                .expect("saknad fil är inget fel")
                .is_none(),
            "en fil som inte finns ska ge Ok(None)"
        );

        let trasig = temp_file("db_broken/plugin_db.json", b"{ inte json");
        let fel = PluginManager::load_from(&trasig).expect_err("trasig fil ska ge ett fel");
        assert!(fel.contains("plugin_db.json"), "skälet ska peka på filen: {fel}");

        // En fil som är giltig JSON men inte en databas: samma sak — fel med skäl, ingen panik.
        let fel_typ = temp_file("db_wrong/plugin_db.json", b"{\"hello\":1}");
        assert!(PluginManager::load_from(&fel_typ).is_err());
    }

    /// **En fil från en nyare Sonix läses inte halvt** (Fas 6.0): versionen känns igen och filen
    /// **lämnas orörd**. En halvläst databas ser ut som en tom databas — och då letar man felet i
    /// skanningen i stället för i filen.
    #[test]
    fn a_newer_database_version_is_refused_with_the_reason() {
        let file = temp_file("db_newer/plugin_db.json", b"{}");
        let mgr = PluginManager::default();
        mgr.save_to(&file).expect("skriv");
        // Skriv upp versionen för hand, som en framtida Sonix skulle ha gjort.
        let text = std::fs::read_to_string(&file).unwrap();
        let höjd = text.replacen(&format!("\"version\": {PLUGIN_DB_VERSION}"), "\"version\": 99", 1);
        assert_ne!(text, höjd, "versionen ska finnas i filen");
        std::fs::write(&file, höjd).unwrap();

        let fel = PluginManager::load_from(&file).expect_err("nyare version ska nekas");
        assert!(fel.contains("version 99"), "skälet ska nämna versionen: {fel}");
        assert!(fel.contains("lämnas orörd"), "och att filen inte rörs: {fel}");
        // Filen är kvar och orörd.
        assert!(std::fs::read_to_string(&file).unwrap().contains("99"));
    }

    /// **En läst fil betyder ingen skanning** (2026-09-15). Provet frågar den **rena** funktionen i
    /// stället för appen: det är varianten `Cached` som avgör att disken inte rörs, och den
    /// skillnaden hade varit osynlig om beslutet låg inbäddat i en `if`-kedja med fil-IO.
    /// De tre fall som finns är alla med: filen läst, filen saknas (första starten), filen trasig.
    #[test]
    fn the_startup_plan_uses_the_cache_instead_of_scanning() {
        let db = PluginManager::without_scan().to_db();
        match startup_plan(Ok(Some(db.clone()))) {
            StartupSource::Cached(från_fil) => assert_eq!(från_fil, db),
            annat => panic!("en läst fil ska ge cachen, inte en skanning: {annat:?}"),
        }
        assert_eq!(startup_plan(Ok(None)), StartupSource::Scan(None));
        assert_eq!(
            startup_plan(Err("trasig fil".to_string())),
            StartupSource::Scan(Some("trasig fil".to_string())),
            "en trasig fil ska ge en skanning **med** skälet"
        );
        // Och en manager byggd utan skanning har tomma listor — det är den vägen `Cached` tar.
        assert!(PluginManager::without_scan().plugins.is_empty());
    }

    /// Åldern visas i ord — ren funktion, så gränssnittet inte kan säga "nyss" om en vecka.
    #[test]
    fn the_age_of_the_database_is_told_in_words() {
        assert_eq!(age_seconds(1_000, 1_000), 0);
        assert_eq!(age_seconds(1_000, 1_200), 0, "en framtida stämpel blir noll, inte negativ");
        assert!(age_text(0).contains("nyss"));
        assert!(age_text(120).contains("2 minuter"));
        assert!(age_text(7_200).contains("2 timmar"));
        assert!(age_text(172_800).contains("2 dygn"));
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

