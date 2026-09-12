//! Kanoniska sökvägar för Sonix (Fas 6.0).
//!
//! **Detta är den enda modulen som får bygga sökvägar.** Ingen annan modul ska
//! läsa `HOME`, `XDG_*` eller `SONIX_*` för att hitta filer — den frågar hit.
//!
//! # Struktur
//!
//! Användarsynliga filer ligger i musikmappen (där filhanteraren letar),
//! maskindata i XDG:s baskataloger:
//!
//! ```text
//! ~/Music/Sonix/                      biblioteket (flyttbart, XDG_MUSIC_DIR)
//! ├── Projects/<namn>.sonix           projektfiler
//! ├── Projects/<namn>/                projektets eget material (Recordings/, Stems/, Samples/, Renders/)
//! ├── Samples/                        egna samplingar (delade mellan projekt)
//! ├── Sample_Packs/                   importerade paket
//! ├── Factory_Samples/                medföljande (skrivskyddat)
//! ├── Templates/                      egna startmallar
//! └── Renders/                        exporter (äldre `Exporterat` läses fortfarande)
//!
//! ~/.config/sonix/                    konfiguration (små filer)
//! ├── config.json  audio.json  ai.json
//!
//! ~/.local/share/sonix/               maskindata
//! ├── models/  plugins/
//!
//! ~/.local/state/sonix/               tillstånd som ska överleva krasch
//! ├── recent.json  window.json  autosave/  logs/
//!
//! ~/.cache/sonix/                     återskapbart
//! ├── waveforms/  library_cache.tsv
//! ```
//!
//! # Överstyrning
//!
//! `SONIX_PROJECTS_DIR`, `SONIX_SAMPLES_DIR`, `SONIX_CONFIG_DIR`,
//! `SONIX_DATA_DIR`, `SONIX_STATE_DIR`, `SONIX_CACHE_DIR` vinner över XDG.
//! XDG följs i sin tur först, med `$HOME`-standardvärden som sista utväg.
//! Ett `~/`-prefix expanderas alltid. Se [`Paths::from_env`].

use std::io;
use std::path::{Path, PathBuf};

/// Katalognamnet under XDG-baskatalogerna (`~/.config/sonix` osv).
pub const APP_DIR: &str = "sonix";

/// Mappnamnet under musikmappen (`~/Music/Sonix`).
pub const LIBRARY_DIR: &str = "Sonix";

/// Miljövariabler som överstyr respektive rot.
pub const ENV_PROJECTS_DIR: &str = "SONIX_PROJECTS_DIR";
pub const ENV_SAMPLES_DIR: &str = "SONIX_SAMPLES_DIR";
pub const ENV_CONFIG_DIR: &str = "SONIX_CONFIG_DIR";
pub const ENV_DATA_DIR: &str = "SONIX_DATA_DIR";
pub const ENV_STATE_DIR: &str = "SONIX_STATE_DIR";
pub const ENV_CACHE_DIR: &str = "SONIX_CACHE_DIR";

fn env_path(key: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(key)?;
    let raw = raw.to_string_lossy().trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(expand_tilde(&raw))
}

/// Expanderar ett inledande `~` (och `~/`) mot `$HOME`. Övriga sökvägar lämnas
/// orörda (relativa sökvägar används som de är).
pub fn expand_tilde(path: &str) -> PathBuf {
    let path = path.trim();
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Läser en `XDG_*_DIR`-post ur innehållet i `~/.config/user-dirs.dirs`.
///
/// Filen är skal-syntax (`XDG_MUSIC_DIR="$HOME/Musik"`), så vi plockar ut
/// nyckeln, tar bort citattecken och expanderar ett inledande `$HOME/`.
/// Ren funktion → testbar utan filsystem.
pub fn parse_user_dir(contents: &str, key: &str) -> Option<PathBuf> {
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim().trim_matches('"').trim_matches('\'').trim();
        if v.is_empty() {
            return None;
        }
        if let Some(rest) = v.strip_prefix("$HOME/") {
            return home_dir().map(|h| h.join(rest));
        }
        if v == "$HOME" {
            return home_dir();
        }
        return Some(expand_tilde(v));
    }
    None
}

/// `XDG_MUSIC_DIR` läses från miljön, annars ur `~/.config/user-dirs.dirs`
/// (variabeln är normalt *inte* exporterad), annars `$HOME/Music`.
fn user_dir_from_user_dirs(key: &str) -> Option<PathBuf> {
    let home = home_dir()?;
    let path = home.join(".config").join("user-dirs.dirs");
    let contents = std::fs::read_to_string(path).ok()?;
    parse_user_dir(&contents, key)
}

/// En uppsättning sökvägar. Kan byggas explicit (för tester) eller från miljön.
///
/// Alla uppslag är rena mot fälten; inget läses från miljön efter
/// konstruktionen. Det gör testerna parallellsäkra.
#[derive(Debug, Clone, Default)]
pub struct Paths {
    home: Option<PathBuf>,
    config_home: Option<PathBuf>,
    data_home: Option<PathBuf>,
    state_home: Option<PathBuf>,
    cache_home: Option<PathBuf>,
    music_dir: Option<PathBuf>,
    downloads_dir: Option<PathBuf>,
    projects_override: Option<PathBuf>,
    samples_override: Option<PathBuf>,
}

impl Paths {
    /// Bygger sökvägarna från miljön: `SONIX_*` → `XDG_*` → `user-dirs.dirs` →
    /// `$HOME`-standardvärden.
    pub fn from_env() -> Self {
        let home = home_dir();
        Self {
            config_home: env_path("XDG_CONFIG_HOME")
                .or_else(|| home.as_ref().map(|h| h.join(".config"))),
            data_home: env_path("XDG_DATA_HOME")
                .or_else(|| home.as_ref().map(|h| h.join(".local/share"))),
            state_home: env_path("XDG_STATE_HOME")
                .or_else(|| home.as_ref().map(|h| h.join(".local/state"))),
            cache_home: env_path("XDG_CACHE_HOME")
                .or_else(|| home.as_ref().map(|h| h.join(".cache"))),
            music_dir: env_path("XDG_MUSIC_DIR")
                .or_else(|| user_dir_from_user_dirs("XDG_MUSIC_DIR"))
                .or_else(|| home.as_ref().map(|h| h.join("Music"))),
            downloads_dir: env_path("XDG_DOWNLOAD_DIR")
                .or_else(|| user_dir_from_user_dirs("XDG_DOWNLOAD_DIR"))
                .or_else(|| home.as_ref().map(|h| h.join("Downloads"))),
            projects_override: env_path(ENV_PROJECTS_DIR),
            samples_override: env_path(ENV_SAMPLES_DIR),
            home,
        }
    }

    /// Explicit hemkatalog (tester). Läser ingen miljö.
    #[allow(dead_code)]
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            config_home: None,
            data_home: None,
            state_home: None,
            cache_home: None,
            music_dir: None,
            downloads_dir: None,
            projects_override: None,
            samples_override: None,
            home: Some(home),
        }
    }

    /// Överstyr projektmappen (motsvarar `SONIX_PROJECTS_DIR`).
    #[allow(dead_code)]
    pub fn override_projects_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.projects_override = Some(dir.into());
        self
    }

    /// Överstyr sample-banken (motsvarar `SONIX_SAMPLES_DIR`).
    #[allow(dead_code)]
    pub fn override_samples_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.samples_override = Some(dir.into());
        self
    }

    /// Hemkatalogen, eller `.` om `HOME` saknas (samma försiktiga fallback som
    /// appen alltid haft — aldrig en hårdkodad användare).
    pub fn home(&self) -> PathBuf {
        self.home.clone().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn config_dir(&self) -> PathBuf {
        if let Some(dir) = env_path(ENV_CONFIG_DIR) {
            return dir;
        }
        self.config_home
            .clone()
            .unwrap_or_else(|| self.home().join(".config"))
            .join(APP_DIR)
    }

    pub fn data_dir(&self) -> PathBuf {
        if let Some(dir) = env_path(ENV_DATA_DIR) {
            return dir;
        }
        self.data_home
            .clone()
            .unwrap_or_else(|| self.home().join(".local/share"))
            .join(APP_DIR)
    }

    pub fn state_dir(&self) -> PathBuf {
        if let Some(dir) = env_path(ENV_STATE_DIR) {
            return dir;
        }
        self.state_home
            .clone()
            .unwrap_or_else(|| self.home().join(".local/state"))
            .join(APP_DIR)
    }

    pub fn cache_dir(&self) -> PathBuf {
        if let Some(dir) = env_path(ENV_CACHE_DIR) {
            return dir;
        }
        self.cache_home
            .clone()
            .unwrap_or_else(|| self.home().join(".cache"))
            .join(APP_DIR)
    }

    /// Musikmappen (`XDG_MUSIC_DIR`), t.ex. `~/Music` eller `~/Musik`.
    pub fn music_dir(&self) -> PathBuf {
        self.music_dir
            .clone()
            .unwrap_or_else(|| self.home().join("Music"))
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.downloads_dir
            .clone()
            .unwrap_or_else(|| self.home().join("Downloads"))
    }

    /// Biblioteket: `~/Music/Sonix`.
    pub fn library_dir(&self) -> PathBuf {
        self.music_dir().join(LIBRARY_DIR)
    }

    /// Projektmappen. Kan överstyras med [`ENV_PROJECTS_DIR`].
    pub fn projects_dir(&self) -> PathBuf {
        self.projects_override
            .clone()
            .unwrap_or_else(|| self.library_dir().join("Projects"))
    }

    /// Projektfilen `<projektmappen>/<namn>.sonix` (namnet saneras för `/`).
    pub fn project_file(&self, name: &str) -> PathBuf {
        self.projects_dir()
            .join(format!("{}.sonix", sanitize_name(name)))
    }

    /// Projektets egen materialmapp: `<projektmappen>/<namn>/`.
    pub fn project_assets_dir(&self, name: &str) -> PathBuf {
        self.projects_dir().join(sanitize_name(name))
    }

    /// Egna samplingar (delas mellan projekt). Överstyrs med [`ENV_SAMPLES_DIR`].
    pub fn samples_dir(&self) -> PathBuf {
        self.samples_override
            .clone()
            .unwrap_or_else(|| self.library_dir().join("Samples"))
    }

    /// Äldre namn på samma sak — läses fortfarande, skrivs aldrig nytt.
    pub fn legacy_samples_dir(&self) -> PathBuf {
        self.library_dir().join("User_Samples")
    }

    pub fn sample_packs_dir(&self) -> PathBuf {
        self.library_dir().join("Sample_Packs")
    }

    pub fn factory_samples_dir(&self) -> PathBuf {
        self.library_dir().join("Factory_Samples")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.library_dir().join("Templates")
    }

    /// Nya exporter: `~/Music/Sonix/Renders`.
    pub fn renders_dir(&self) -> PathBuf {
        self.library_dir().join("Renders")
    }

    /// Det äldre exportmappenamnet. Lämnas orört.
    pub fn legacy_exports_dir(&self) -> PathBuf {
        self.library_dir().join("Exporterat")
    }

    /// Standardmapp för export. Finns en äldre `Exporterat`-mapp och ingen
    /// `Renders` än, fortsätter vi använda den (inget material "försvinner" för
    /// en befintlig användare); annars `Renders`.
    pub fn exports_dir(&self) -> PathBuf {
        let legacy = self.legacy_exports_dir();
        let renders = self.renders_dir();
        if legacy.is_dir() && !renders.is_dir() {
            legacy
        } else {
            renders
        }
    }

    /// Allmän konfiguration (`config.json`: språk m.m.).
    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.json")
    }

    /// Ljudinställningar (`audio.json`).
    pub fn audio_config_file(&self) -> PathBuf {
        self.config_dir().join("audio.json")
    }

    /// AI-providers och nycklar (`ai.json`, chmod 0600).
    pub fn ai_config_file(&self) -> PathBuf {
        self.config_dir().join("ai.json")
    }

    /// ONNX-modeller m.m. Stora filer → data, inte konfiguration.
    pub fn models_dir(&self) -> PathBuf {
        self.data_dir().join("models")
    }

    /// Äldre modellplats (`~/.config/sonix/models`) — läses som fallback.
    pub fn legacy_models_dir(&self) -> PathBuf {
        self.config_dir().join("models")
    }

    pub fn plugin_db_dir(&self) -> PathBuf {
        self.data_dir().join("plugins")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.state_dir().join("logs")
    }

    /// Loggfil under [`Self::logs_dir`].
    pub fn log_file(&self, name: &str) -> PathBuf {
        self.logs_dir().join(name)
    }

    /// Äldre logg-/felsökningsplats: biblioteket (`~/Music/Sonix/...`).
    pub fn legacy_library_file(&self, name: &str) -> PathBuf {
        self.library_dir().join(name)
    }

    /// Markörfil för felsökningsloggning (`debug_on`).
    pub fn debug_marker_file(&self) -> PathBuf {
        self.state_dir().join("debug_on")
    }

    /// Läslista över senaste projekt.
    pub fn recent_file(&self) -> PathBuf {
        self.state_dir().join("recent.json")
    }

    /// Fönsterläge och senaste skärm (skrivs av UI:t när 6.1 är på plats).
    #[allow(dead_code)]
    pub fn window_file(&self) -> PathBuf {
        self.state_dir().join("window.json")
    }

    pub fn autosave_dir(&self) -> PathBuf {
        self.state_dir().join("autosave")
    }

    pub fn waveform_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("waveforms")
    }

    /// Cache för sample-bibliotekets genomsökning (Flyttad från biblioteket).
    pub fn library_cache_file(&self) -> PathBuf {
        self.cache_dir().join("library_cache.tsv")
    }

    /// Kataloger som genomsöks efter importerbara ljudpaket.
    pub fn scan_dirs(&self) -> Vec<PathBuf> {
        vec![self.music_dir(), self.downloads_dir()]
    }

    /// Kataloger som genomsöks efter egna samplingar (kanonisk först).
    pub fn user_sample_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.samples_dir()];
        let legacy = self.legacy_samples_dir();
        if legacy != dirs[0] {
            dirs.push(legacy);
        }
        dirs
    }

    /// Alla kataloger appen vill kunna skriva till.
    pub fn writable_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.projects_dir(),
            self.samples_dir(),
            self.sample_packs_dir(),
            self.templates_dir(),
            self.renders_dir(),
            self.config_dir(),
            self.models_dir(),
            self.plugin_db_dir(),
            self.logs_dir(),
            self.autosave_dir(),
            self.waveform_cache_dir(),
        ]
    }

    /// Skapar de kataloger appen behöver. Misslyckanden returneras, inte panik.
    pub fn ensure_dirs(&self) -> Vec<(PathBuf, io::Error)> {
        let mut errors = Vec::new();
        for dir in self.writable_dirs() {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                errors.push((dir, err));
            }
        }
        errors
    }
}

/// Sanerar ett projektnamn till ett filnamn (samma regel som appen använt).
pub fn sanitize_name(name: &str) -> String {
    let cleaned = name.trim().replace('/', "_");
    if cleaned.is_empty() {
        "Namnlöst Projekt".to_string()
    } else {
        cleaned
    }
}

/// Sökvägarna från den aktuella miljön.
pub fn paths() -> Paths {
    Paths::from_env()
}

/// Hela kartan som `(beskrivning, sökväg)` — används av `sonix --paths`.
pub fn table() -> Vec<(&'static str, PathBuf)> {
    let p = paths();
    vec![
        ("Projekt", p.projects_dir()),
        ("Projektets material", p.project_assets_dir("<namn>")),
        ("Egna samplingar", p.samples_dir()),
        ("Samplingpaket", p.sample_packs_dir()),
        ("Fabrikssamplingar", p.factory_samples_dir()),
        ("Mallar", p.templates_dir()),
        ("Exporter", p.exports_dir()),
        ("Konfiguration", p.config_dir()),
        ("  Allmän", p.config_file()),
        ("  Ljud", p.audio_config_file()),
        ("  AI", p.ai_config_file()),
        ("Maskindata", p.data_dir()),
        ("  Modeller", p.models_dir()),
        ("  Plugin-databas", p.plugin_db_dir()),
        ("Tillstånd", p.state_dir()),
        ("  Senaste projekt", p.recent_file()),
        ("  Autosave", p.autosave_dir()),
        ("  Loggar", p.logs_dir()),
        ("Cache", p.cache_dir()),
        ("  Vågformer", p.waveform_cache_dir()),
        ("Musikmapp", p.music_dir()),
        ("Bibliotek", p.library_dir()),
    ]
}

/// Flyttar äldre platser till de kanoniska. **Raderar aldrig något.**
///
/// Returnerar en lista med mänskliga notiser (visas i statusraden en gång).
/// Tål att köras flera gånger: allt är villkorat på att målet saknas.
pub fn migrate() -> Vec<String> {
    migrate_in(&paths())
}

/// Som [`migrate`], men mot en explicit uppsättning sökvägar (testbart).
pub fn migrate_in(p: &Paths) -> Vec<String> {
    let mut notes = Vec::new();

    // Modeller flyttades från konfigurationen till maskindata (de är data).
    // Tåligt mot båda ordningarna: `ensure_dirs()` kan redan ha skapat ett tomt
    // mål, och då flyttas filerna i stället för hela mappen.
    let legacy_models = p.legacy_models_dir();
    let models = p.models_dir();
    if dir_entry_count(&legacy_models) > 0 && dir_entry_count(&models) == 0 {
        if let Some(parent) = models.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let moved = move_dir_contents(&legacy_models, &models)
            .or_else(|_| copy_dir_contents(&legacy_models, &models, 0));
        match moved {
            Ok(count) => notes.push(format!(
                "📦 Modeller flyttade till {} ({count} fil(er))",
                models.display()
            )),
            Err(err) => notes.push(format!(
                "⚠ Kunde inte flytta modeller ({err}) — läser fortfarande {}",
                legacy_models.display()
            )),
        }
    }

    // Egna samplingar: gamla User_Samples läses vid sidan av Samples (flyttas inte
    // — en sammanslagning av två användarmappar får användaren själv avgöra).
    let legacy_samples = p.legacy_samples_dir();
    if legacy_samples.is_dir() && legacy_samples != p.samples_dir() {
        notes.push(format!(
            "ℹ Egna samplingar läses från både {} och {}",
            p.samples_dir().display(),
            legacy_samples.display()
        ));
    }

    // Exportmappen: äldre `Exporterat` fortsätter användas tills `Renders` finns.
    if p.legacy_exports_dir().is_dir() && !p.renders_dir().is_dir() {
        notes.push(format!(
            "ℹ Exporter hamnar i den befintliga mappen {}",
            p.legacy_exports_dir().display()
        ));
    }

    // Felsökningsmarkören flyttades till tillståndskatalogen.
    let legacy_marker = p.legacy_library_file("debug_on");
    if legacy_marker.is_file() && !p.debug_marker_file().is_file() {
        notes.push(format!(
            "ℹ Felsökningsloggning: markören {} läses fortfarande",
            legacy_marker.display()
        ));
    }

    notes
}

/// Maxdjup när en äldre mapp kopieras över filsystemsgränser.
const MAX_DEPTH: u32 = 8;

/// Antal barn i en mapp (0 om den inte finns eller är otillgänglig).
fn dir_entry_count(dir: &Path) -> usize {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).count(),
        Err(_) => 0,
    }
}

/// Flyttar varje barn från `from` till `to`. Befintliga mål hoppas över (skrivs
/// aldrig över), vilket gör en avbruten migrering säker att köra igen.
fn move_dir_contents(from: &Path, to: &Path) -> io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut count = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if target.exists() {
            continue;
        }
        let path = entry.path();
        match std::fs::rename(&path, &target) {
            Ok(()) => count += 1,
            // Över filsystemsgränser: kopiera i stället (originalet behålls).
            Err(_) if path.is_dir() => count += copy_dir_contents(&path, &target, 0)?,
            Err(_) => {
                std::fs::copy(&path, &target)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

fn copy_dir_contents(from: &Path, to: &Path, depth: u32) -> io::Result<usize> {
    if depth > MAX_DEPTH {
        return Ok(0);
    }
    std::fs::create_dir_all(to)?;
    let mut count = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            count += copy_dir_contents(&path, &target, depth + 1)?;
        } else {
            std::fs::copy(&path, &target)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("sonix-paths-{}-{}-{}", std::process::id(), tag, n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        dir
    }

    #[test]
    fn explicit_home_yields_the_canonical_layout() {
        let p = Paths::with_home("/home/tester");
        assert_eq!(p.music_dir(), PathBuf::from("/home/tester/Music"));
        assert_eq!(p.library_dir(), PathBuf::from("/home/tester/Music/Sonix"));
        assert_eq!(
            p.projects_dir(),
            PathBuf::from("/home/tester/Music/Sonix/Projects")
        );
        assert_eq!(
            p.project_file("Vägen hit"),
            PathBuf::from("/home/tester/Music/Sonix/Projects/Vägen hit.sonix")
        );
        assert_eq!(
            p.project_assets_dir("Vägen hit"),
            PathBuf::from("/home/tester/Music/Sonix/Projects/Vägen hit")
        );
        assert_eq!(
            p.samples_dir(),
            PathBuf::from("/home/tester/Music/Sonix/Samples")
        );
        assert_eq!(
            p.legacy_samples_dir(),
            PathBuf::from("/home/tester/Music/Sonix/User_Samples")
        );
        assert_eq!(
            p.templates_dir(),
            PathBuf::from("/home/tester/Music/Sonix/Templates")
        );
        assert_eq!(
            p.config_file(),
            PathBuf::from("/home/tester/.config/sonix/config.json")
        );
        assert_eq!(
            p.audio_config_file(),
            PathBuf::from("/home/tester/.config/sonix/audio.json")
        );
        assert_eq!(
            p.ai_config_file(),
            PathBuf::from("/home/tester/.config/sonix/ai.json")
        );
        assert_eq!(
            p.models_dir(),
            PathBuf::from("/home/tester/.local/share/sonix/models")
        );
        assert_eq!(
            p.legacy_models_dir(),
            PathBuf::from("/home/tester/.config/sonix/models")
        );
        assert_eq!(
            p.logs_dir(),
            PathBuf::from("/home/tester/.local/state/sonix/logs")
        );
        assert_eq!(
            p.autosave_dir(),
            PathBuf::from("/home/tester/.local/state/sonix/autosave")
        );
        assert_eq!(
            p.library_cache_file(),
            PathBuf::from("/home/tester/.cache/sonix/library_cache.tsv")
        );
        assert_eq!(
            p.waveform_cache_dir(),
            PathBuf::from("/home/tester/.cache/sonix/waveforms")
        );
    }

    /// Konfiguration, data, tillstånd och cache måste vara fyra olika träd —
    /// annars hamnar stora modeller i konfigkatalogen igen.
    #[test]
    fn the_four_roots_are_distinct_trees() {
        let p = Paths::with_home("/home/tester");
        let roots = [p.config_dir(), p.data_dir(), p.state_dir(), p.cache_dir()];
        for (i, a) in roots.iter().enumerate() {
            for b in roots.iter().skip(i + 1) {
                assert_ne!(a, b, "rot upprepad: {} == {}", a.display(), b.display());
                assert!(
                    !a.starts_with(b),
                    "{} ligger under {}",
                    a.display(),
                    b.display()
                );
            }
        }
    }

    #[test]
    fn tilde_and_relative_paths_expand_predictably() {
        // Utan att röra miljön: expand_tilde med okänt HOME-prefix lämnas orört.
        assert_eq!(expand_tilde("music/x.wav"), PathBuf::from("music/x.wav"));
        assert_eq!(expand_tilde("/abs/x.wav"), PathBuf::from("/abs/x.wav"));
        assert_eq!(sanitize_name("  A/B  "), "A_B");
        assert_eq!(sanitize_name("   "), "Namnlöst Projekt");
    }

    #[test]
    fn user_dirs_file_is_parsed_with_quotes_and_home_prefix() {
        let contents = r#"
# kommentar
XDG_DOWNLOAD_DIR="$HOME/Ner"
XDG_MUSIC_DIR="$HOME/Musik"
XDG_PICTURES_DIR="/mnt/bilder"
"#;
        assert!(parse_user_dir(contents, "XDG_NONEXISTENT").is_none());
        // Utan HOME i miljön kan $HOME-prefixet inte expanderas — men absolutvägen fungerar.
        let abs = parse_user_dir(contents, "XDG_PICTURES_DIR").expect("absolutväg");
        assert_eq!(abs, PathBuf::from("/mnt/bilder"));
        let quoted = parse_user_dir("XDG_MUSIC_DIR='/tmp/min musik'\n", "XDG_MUSIC_DIR")
            .expect("enkelcitat");
        assert_eq!(quoted, PathBuf::from("/tmp/min musik"));
    }

    #[test]
    fn overrides_win_over_xdg() {
        let p = Paths::with_home("/home/tester")
            .override_projects_dir("/mnt/projekt")
            .override_samples_dir("/mnt/samples");
        assert_eq!(p.projects_dir(), PathBuf::from("/mnt/projekt"));
        assert_eq!(p.samples_dir(), PathBuf::from("/mnt/samples"));
        // Övriga rötter opåverkade.
        assert_eq!(
            p.models_dir(),
            PathBuf::from("/home/tester/.local/share/sonix/models")
        );
    }

    #[test]
    fn exports_dir_follows_a_pre_existing_legacy_folder() {
        let tmp = tmp_dir("exports-legacy");
        let p = Paths::with_home(&tmp);
        // Ingen mapp alls → den nya kanoniska.
        assert_eq!(p.exports_dir(), p.renders_dir());
        // Bara den äldre mappen → den används (inget material ska "försvinna").
        std::fs::create_dir_all(p.legacy_exports_dir()).expect("legacy");
        assert_eq!(p.exports_dir(), p.legacy_exports_dir());
        // Finns båda → den nya vinner.
        std::fs::create_dir_all(p.renders_dir()).expect("renders");
        assert_eq!(p.exports_dir(), p.renders_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn user_sample_dirs_list_the_canonical_first() {
        let p = Paths::with_home("/home/tester");
        let dirs = p.user_sample_dirs();
        assert_eq!(dirs[0], p.samples_dir());
        assert!(dirs.contains(&p.legacy_samples_dir()));
        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn ensure_dirs_creates_every_writable_dir() {
        let tmp = tmp_dir("ensure");
        let p = Paths::with_home(&tmp);
        let errors = p.ensure_dirs();
        assert!(errors.is_empty(), "oväntade fel: {errors:?}");
        for dir in p.writable_dirs() {
            assert!(dir.is_dir(), "{} skapades inte", dir.display());
        }
        // Allt hamnar under hemkatalogen — inget läcker utanför.
        for dir in p.writable_dirs() {
            assert!(
                dir.starts_with(&tmp),
                "{} läcker utanför hemmet",
                dir.display()
            );
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn migrate_moves_models_and_never_deletes() {
        let tmp = tmp_dir("migrate");
        let p = Paths::with_home(&tmp);
        let errors = p.ensure_dirs();
        assert!(errors.is_empty());

        // Låtsas om en äldre installation: modeller i konfigkatalogen, inklusive
        // en underkatalog (Demucs-exporter kan ha flera filer).
        std::fs::create_dir_all(p.legacy_models_dir().join("sub")).expect("legacy models");
        std::fs::write(p.legacy_models_dir().join("htdemucs.onnx"), b"modell").expect("fil");
        std::fs::write(p.legacy_models_dir().join("sub/demucs.onnx"), b"modell2").expect("fil");

        // `ensure_dirs` har redan skapat ett TOMT mål — migreringen ska ändå
        // flytta (det var buggen: ordningen fick flytten att utebli).
        let notes = migrate_in(&p);
        assert!(
            notes.iter().any(|n| n.contains("Modeller flyttade")),
            "förväntade en flyttnotis: {notes:?}"
        );
        assert!(
            p.models_dir().join("htdemucs.onnx").is_file(),
            "modellen flyttades inte"
        );
        assert!(
            p.models_dir().join("sub/demucs.onnx").is_file(),
            "underkatalogen flyttades inte"
        );
        assert!(
            !p.legacy_models_dir().join("htdemucs.onnx").exists(),
            "originalet ligger kvar — filen kopierades i stället för att flyttas"
        );

        // Idempotent: andra körningen ska vara tyst.
        let again = migrate_in(&p);
        assert!(
            !again.iter().any(|n| n.contains("Modeller flyttade")),
            "flyttade igen: {again:?}"
        );

        // Ett mål som redan innehåller filer röres inte (ingen överskrivning).
        std::fs::write(p.legacy_models_dir().join("senare.onnx"), b"ny").expect("fil");
        let third = migrate_in(&p);
        assert!(
            !third.iter().any(|n| n.contains("Modeller flyttade")),
            "skrev över ett mål som redan hade filer: {third:?}"
        );
        assert!(p.legacy_models_dir().join("senare.onnx").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn migrate_is_idempotent_and_reports_legacy_samples() {
        let tmp = tmp_dir("migrate-idempotent");
        let p = Paths::with_home(&tmp);
        p.ensure_dirs();
        std::fs::create_dir_all(p.legacy_samples_dir()).expect("legacy samples");

        let first = migrate_in(&p);
        let second = migrate_in(&p);
        assert!(first.iter().any(|n| n.contains("Egna samplingar")));
        assert_eq!(first, second, "migreringen ska vara idempotent");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn table_has_no_empty_paths() {
        for (label, path) in table() {
            assert!(!label.is_empty());
            assert!(!path.as_os_str().is_empty(), "{label} saknar sökväg");
        }
    }
}

/// Filen för en renderad stämma i ett projekts egen materialmapp (8.5a).
///
/// Fri funktion i stället för en metod på `Paths`, eftersom projektmappen redan
/// kommer färdig från [`Paths::project_assets_dir`]. Poängen är densamma som för
/// hela modulen: sökvägen byggs på **ett** ställe, så namn och ändelse inte kan
/// glida isär mellan separatorn, sparandet och inläsningen.
pub fn stem_file(project_assets_dir: &std::path::Path, stem_name: &str) -> std::path::PathBuf {
    project_assets_dir.join(format!("{} (stämma).wav", sanitize_name(stem_name)))
}
