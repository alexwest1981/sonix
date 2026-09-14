//! Import — stämmor från Suno, filer, separatorn och genererat ljud.
//!
//! Varje väg in här har samma regel (8.5): en väg som skapar ett klipp eller en fil får
//! **aldrig hitta på ljud**. Går filen inte att läsa rapporteras den (den delade listan
//! `UNREADABLE_SOURCES`) och vägen avbryter — den gör inte ett tyst klipp.
//!
//! Status: byggs — importvägarna; nya format läggs här, inte i UI:t.
//! Rör inte: `load_audio_or_report` är den enda vägen in till PCM; gå inte förbi den.

use super::*;

/// Har sökvägen den ändelsen? (skiftlägesoberoende)
fn has_extension(path: &std::path::Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

/// Nyckeln som avgör om två filer är SAMMA stämma: filnamnet utan ändelse.
fn stem_key(path: &std::path::Path) -> Option<String> {
    path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_lowercase())
}

/// Väljer vilka stämmor som ska importeras (Fas 8.5). Returnerar urvalet och hur
/// många mp3:er som hoppades över.
///
/// **Varför:** appen kan inte spela mp3 — bara konvertera den till wav. Finns
/// samma stämma redan som wav är mp3:n ett extra varv genom konverteringen utan
/// att tillföra något, och det var precis det Alex såg: importören läste mp3:erna
/// några sekunder och fortsatte sedan med wav-filerna. Finns stämman *bara* som
/// mp3 ska den förstås med — då ÄR den stämman.
///
/// Regeln ligger som en egen funktion för att kunna prövas utan fönster, och för
/// att den dialog Alex föreslog ska kunna återanvända exakt samma logik: frågan
/// är bara om man vill behålla mp3-filerna när wav redan finns.
pub(crate) fn stem_files_for_import(
    files: Vec<std::path::PathBuf>,
    keep_mp3: bool,
) -> (Vec<std::path::PathBuf>, usize) {
    if keep_mp3 {
        return (files, 0);
    }
    let wav_stems: std::collections::HashSet<String> = files
        .iter()
        .filter(|p| has_extension(p, "wav"))
        .filter_map(|p| stem_key(p))
        .collect();
    let mut kept = Vec::with_capacity(files.len());
    let mut skipped = 0usize;
    for f in files {
        let duplicate = has_extension(&f, "mp3")
            && stem_key(&f)
                .map(|k| wav_stems.contains(&k))
                .unwrap_or(false);
        if duplicate {
            skipped += 1;
        } else {
            kept.push(f);
        }
    }
    (kept, skipped)
}

/// Filändelserna appen läser in som stämmor (samma lista som mappläsningen).
const STEM_AUDIO_EXTS: [&str; 4] = ["wav", "mp3", "flac", "ogg"];

/// Ljudfilerna i en mapp, sorterade.
///
/// Ren funktion utan fönster, så att importen och frågan **ovanför** den ser
/// exakt samma lista — annars kan frågan nämna ett antal som importen sedan inte
/// känner igen.
pub(crate) fn list_stem_audio_files(dir: &str) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && STEM_AUDIO_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Hur många mp3:er i en namnmängd som har sin stämma som wav också.
///
/// Samma regel som [`stem_files_for_import`] (filnamnet utan ändelse avgör om två
/// filer är samma stämma), men den här **räknar** i stället för att välja: frågan
/// till användaren behöver siffran. Namnen kommer antingen ur en mapp eller ur
/// ett ZIP-arkivs innehållsförteckning, och därför jämförs bara filnamnet —
/// `unzip -Z1` ger `Stems/vocals.mp3` där mappen ger en hel sökväg.
pub(crate) fn duplicate_mp3_count(names: &[String]) -> usize {
    let paths: Vec<std::path::PathBuf> = names.iter().map(std::path::PathBuf::from).collect();
    let wav_keys: std::collections::HashSet<String> = paths
        .iter()
        .filter(|p| has_extension(p, "wav"))
        .filter_map(|p| stem_key(p))
        .collect();
    paths
        .iter()
        .filter(|p| has_extension(p, "mp3"))
        .filter_map(|p| stem_key(p))
        .filter(|k| wav_keys.contains(k))
        .count()
}

/// Wav-syskonet till en fil, om det finns i samma mapp (Fas 8.5).
///
/// Stämseparatorn tar **en** fil. Är det en mp3 som har sin wav bredvid sig finns
/// ingen anledning att gå omvägen över ett format appen inte kan spela — men
/// valet är användarens, så frågan ställs med den här sökningen som underlag.
pub(crate) fn wav_sibling(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    if has_extension(p, "wav") {
        return None;
    }
    let key = stem_key(p)?;
    let dir = p.parent()?;
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries
        .into_iter()
        .find(|c| has_extension(c, "wav") && stem_key(c).as_deref() == Some(key.as_str()))
        .map(|c| c.to_string_lossy().into_owned())
}

/// Namnen i ett ZIP-arkiv, utan att packa upp det (`unzip -Z1`).
///
/// Frågan om mp3:erna måste ställas **innan** uppackningen, och uppackningen sker
/// i bakgrunden. `unzip -Z1` läser bara innehållsförteckningen — millisekunder
/// för ett stäm-arkiv. Går det inte (unzip saknas, trasigt arkiv, arkivet är
/// krypterat) blir listan tom, frågan ställs inte, och importen gör som förut.
pub(crate) fn zip_entry_names(zip_path: &str) -> Vec<String> {
    std::process::Command::new("unzip")
        .args(["-Z1", zip_path])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Basnamnet stämfilerna får: källfilens namn utan ändelse, annars låtens titel.
///
/// Ren funktion, så att namnregeln kan prövas utan fönster — och så att
/// stämseparatorn och en framtida import kan använda samma namn.
pub(crate) fn stem_base_name(source_path: Option<&str>, title: &str) -> String {
    let from_path = source_path
        .map(std::path::Path::new)
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let from_title = title.trim();
    from_path
        .or(if from_title.is_empty() { None } else { Some(from_title) })
        .unwrap_or("stems")
        .to_string()
}

/// Källfiler som inte gick att läsa, för statusraden (Fas 8.5).
///
/// **Varför delad och inte en parameter.** Det finns elva ställen som läser ljud,
/// och alla utom ett gjorde `if let Ok(...)` **utan `else`**: en fil som inte kunde
/// läsas föll bort utan ett ord, och klippet ritades ändå ur sin sparade översikt.
/// Ett klipp som ser ut att ha ljud men är tyst är värre än ett som är tomt.
///
/// Med en lista per anrop måste varje ställe komma ihåg att skicka in den, och den
/// som glömmer får tystnad igen. En delad lista kan inte glömmas: anropet ser
/// likadant ut överallt, och den som läser av den (en gång per bildruta) får hellre
/// en rad för mycket än en tystnad.
static UNREADABLE_SOURCES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Laddar en regions ljud — och minns när det inte gick.
///
/// Samma form som `load_wav_pcm` (en `Option` i stället för en `Result`), så att
/// anropen bara byter namn: `&& let Some((l, r, sr)) = load_audio_or_report(path)`.
pub(crate) fn load_audio_or_report(path: &str) -> Option<(Vec<f32>, Vec<f32>, u32)> {
    match crate::audio::load_wav_pcm(path) {
        Ok((l, r, sr)) => Some((l, r, sr)),
        Err(_) => {
            report_unreadable(path);
            None
        }
    }
}

/// Minns en källfil som inte gick att läsa. En plats, två vägar (här och i
/// `load_sample_pcm_arcs`) — alla elva anrop i appen täcks av samma lista.
pub(crate) fn report_unreadable(path: &str) {
    if let Ok(mut list) = UNREADABLE_SOURCES.lock()
        && !list.iter().any(|p| p == path)
    {
        list.push(path.to_string());
    }
}

/// Tar listan av oläsbara källor och tömmer den. Anropas en gång per bildruta.
pub(crate) fn take_unreadable_sources() -> Vec<String> {
    UNREADABLE_SOURCES
        .lock()
        .map(|mut l| std::mem::take(&mut *l))
        .unwrap_or_default()
}

/// Var en stämimport kommer ifrån (Fas 8.5): frågan om mp3:erna ställs innan
/// arbetet börjar, och svaret måste kunna starta **samma** väg igen.
#[derive(Clone)]
pub enum StemImportSource {
    Folder(String),
    Zip(String),
}

/// Frågan som ställs innan en stämimport eller en separation börjar (Alex'
/// förslag, Fas 8.5): mp3:erna som redan har sin stämma som wav.
///
/// Regeln ligger i [`stem_files_for_import`] sedan tidigare — det här är bara
/// valet, och valet är användarens. Två sammanhang, samma fråga.
#[derive(Clone)]
pub enum WavQuestion {
    /// En import där `duplicates` mp3:er har wav-syskon.
    StemImport {
        source: StemImportSource,
        title: String,
        bpm: f32,
        duplicates: usize,
    },
    /// Stämseparatorn: den valda filen har ett wav-syskon.
    Separate { chosen: String, sibling: String },
}

#[derive(Clone, Default)]
pub struct StemImportProgress {
    pub is_importing: bool,
    pub stage: String,
    pub current_file: String,
    pub current_idx: usize,
    pub total_files: usize,
    pub progress_ratio: f32,
    pub completed_payload: Option<StemImportResult>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct DecodedStemTrack {
    pub clean_name: String,
    pub path_str: String,
    pub kind: TrackKind,
    pub icon: &'static str,
    pub color: Color32,
    pub pcm_left: Option<std::sync::Arc<Vec<f32>>>,
    pub pcm_right: Option<std::sync::Arc<Vec<f32>>>,
    pub sample_rate: u32,
    pub file_bars: f32,
    pub wave_env: Vec<f32>,
    /// Flernivåcachen för stämman (Fas 8.3), byggd i avkodningstråden.
    ///
    /// Här är enda stället den kan byggas utan att någon väntar: samplen är redan
    /// i cacheminnet efter avkodningen, och tråden är ändå en arbetstråd. Byggdes
    /// den först när spåret ritas stod den grova översikten i bildrutan tills
    /// `ensure_waveform_cache` hunnit ikapp i en andra tråd.
    pub waveform_cache: Option<crate::audio::waveform::WaveformCache>,
}

#[derive(Clone)]
pub struct StemImportResult {
    pub project_title: String,
    pub bpm: f32,
    pub max_stem_bars: f32,
    pub tracks: Vec<DecodedStemTrack>,
}

#[derive(Clone, Default)]
pub struct ProjectLoadProgress {
    pub is_loading: bool,
    pub project_name: String,
    pub stage: String,
    pub current_track: String,
    pub current_idx: usize,
    pub total_tracks: usize,
    pub progress_ratio: f32,
    pub completed_payload: Option<LoadedProjectPayload>,
    pub error_message: Option<String>,
}

impl SonixApp {
/// Öppnar plattformens egen filväljare i en egen tråd, så att appen fortsätter
/// rita medan dialogen är uppe.
///
/// `label` är vad filtret heter i dialogen och `extensions` är filändelserna
/// **utan punkt** (`["wav", "mp3"]`). Både skiftlägen tas med där äldre
/// filter listade dem, eftersom filtreringen är skiftlägeskänslig på vissa
/// plattformar.
///
/// En dialog som inte kunde öppnas ger `None`, precis som en avbruten dialog —
/// rfd lämnar ingen felorsak. Det är därför ingen skillnad görs här.
pub fn spawn_async_file_picker(
    &self,
    label: &'static str,
    extensions: &'static [&'static str],
    title: &'static str,
) {
    let active = self.is_file_dialog_active.clone();
    let target = self.pending_file_dialog_result.clone();
    if !active.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .set_title(title)
                .add_filter(label, extensions)
                .pick_file();
            if let Some(path) = picked
                && let Ok(mut lock) = target.lock()
            {
                *lock = Some(path.to_string_lossy().into_owned());
            }
            active.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
}
}

impl SonixApp {
/// Decodes the chosen file and runs stem separation on a background thread.
/// Uses the neural HTDemucs backend when a model is installed, otherwise the
/// built-in DSP separator.
/// Startar en separation — och frågar först om den valda filen har sin wav
/// bredvid sig (Fas 8.5).
///
/// Frågan finns för att båda vägarna leder till samma ljud men olika arbete:
/// mp3:an måste konverteras först, och appen kan inte spela den direkt. Den
/// som *vill* separera mp3:an (den kanske är den enda som finns i den
/// mastern) kan säga det.
pub fn start_stem_separation(&mut self, path: &str) {
    if let Some(sibling) = wav_sibling(path) {
        self.pending_wav_question = Some(WavQuestion::Separate {
            chosen: path.to_string(),
            sibling,
        });
        return;
    }
    self.start_stem_separation_with(path);
}
}

impl SonixApp {
/// Själva separationen (bakgrundstråden). Kallas också när frågan är besvarad.
pub fn start_stem_separation_with(&mut self, path: &str) {
    let (l, r, sr) = match crate::audio::load_audio_pcm(path) {
        Ok(v) => v,
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte läsa '{}': {}", path, e);
            return;
        }
    };

    self.stem_separation_active = true;
    self.stem_project.is_separating = true;
    self.stem_project.progress = 0.0;
    self.stem_project.source_path = Some(path.to_string());
    self.stem_project.track_title = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    if let Ok(mut p) = self.stem_separation_progress.lock() {
        *p = 0.0;
    }
    if let Ok(mut slot) = self.stem_separation_result.lock() {
        *slot = None;
    }
    self.view_mode = ViewMode::StemSeparator;

    let backend = if crate::audio::neural_available() {
        crate::i18n::t("neural (HTDemucs)").to_string()
    } else {
        crate::i18n::t("DSP").to_string()
    };
    self.status_message = crate::tstatus!(
        "⏳ Separerar '{}' i bakgrunden med {}...",
        self.stem_project.track_title,
        backend
    );

    let progress = self.stem_separation_progress.clone();
    let slot = self.stem_separation_result.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_separation(&l, &r, sr, &|value| {
                if let Ok(mut p) = progress.lock() {
                    *p = value;
                }
            })
        }));
        let value = match result {
            Ok(r) => Ok(r),
            Err(_) => Err(crate::i18n::t("Separationen kraschade oväntat").to_string()),
        };
        if let Ok(mut s) = slot.lock() {
            *s = Some(value);
        }
    });
}
}

impl SonixApp {
/// Polls the background separation thread and installs the result.
pub fn poll_stem_separation(&mut self) {
    if !self.stem_separation_active {
        return;
    }
    if let Ok(p) = self.stem_separation_progress.lock() {
        self.stem_project.progress = *p;
    }
    let taken = self
        .stem_separation_result
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let Some(result) = taken else {
        return;
    };
    self.stem_separation_active = false;
    match result {
        Ok(result) => {
            let source = self.stem_project.source_path.clone().unwrap_or_default();
            let neural = result.used_neural;
            let fallback = result.fallback_reason.clone();
            // Samma mapp och samma basnamn som *exportera stämmorna* använder:
            // då pekar regionen på en fil som redan finns, i stället för att
            // samma stämma skrivs två gånger på två ställen.
            let stems_dir = crate::paths::paths()
                .project_assets_dir(&self.project_name)
                .join("Stems");
            let base = stem_base_name(Some(&source), &self.stem_project.track_title);
            let write_error = self
                .stem_project
                .install_separation(result, &source, &stems_dir, &base)
                .err();
            self.load_separated_stems_to_engine();
            self.view_mode = ViewMode::StemSeparator;
            let backend = if neural {
                crate::i18n::t("HTDemucs (neural)")
            } else {
                crate::i18n::t("DSP")
            };
            self.status_message = crate::tstatus!(
                "✅ Separerade '{}' i 4 spelbara stämspår med {} ({:.1}s)",
                self.stem_project.track_title,
                backend,
                self.stem_project.duration_seconds
            );
            if !neural
                && let Some(reason) = fallback
            {
                self.status_message = format!("{} — {}", self.status_message, reason);
            }
            // Separationen lyckades, men filerna kom inte till disk. Då finns
            // stämmorna bara i minnet: att tiga om det vore att lova ett klipp
            // som inte kan spelas upp igen efter en omladdning.
            if let Some(err) = write_error {
                self.status_message = crate::tstatus!(
                    "⚠ Separationen är klar, men stämmorna kunde inte skrivas till disk: {} — de finns bara i minnet",
                    err
                );
            }
        }
        Err(err) => {
            self.stem_project.is_separating = false;
            self.stem_project.progress = 0.0;
            self.status_message = crate::tstatus!("⚠ Separation misslyckades: {}", err);
        }
    }
}
}

impl SonixApp {
pub fn load_separated_stems_to_engine(&mut self) {
    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    for i in 0..self.stem_project.stem_audio.len().min(self.stem_project.stems.len()) {
        let audio = self.stem_project.stem_audio[i].clone();
        let ch = &self.stem_project.stems[i];
        let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
            track_index: i,
            left: std::sync::Arc::new(audio.left),
            right: std::sync::Arc::new(audio.right),
            sample_rate: self.stem_project.sample_rate as f32,
            volume: ch.volume,
            pan: ch.pan,
            start_time_secs: 0.0,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
            track_index: i,
            volume: ch.volume,
            pan: ch.pan,
            muted: ch.muted,
            solo: ch.solo,
        });
    }
}
}

impl SonixApp {
/// Push live mixer changes from the stem separator view to the engine.
pub fn sync_stem_separator_engine(&mut self) {
    if self.stem_project.stem_audio.is_empty() {
        return;
    }
    let count = self.stem_project.stems.len().min(self.stem_project.stem_audio.len());
    for i in 0..count {
        let ch = &self.stem_project.stems[i];
        let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
            track_index: i,
            volume: ch.volume,
            pan: ch.pan,
            muted: ch.muted,
            solo: ch.solo,
        });
    }
}
}

impl SonixApp {
/// Loads a real audio file from disk and adds it as a new timeline track
/// with a playable region (used by the "Add Track → Import" templates).
pub fn import_audio_file_as_track(&mut self, path: &str, tmpl: &TrackTemplate) {
    let (l, r, sr) = match crate::audio::load_wav_pcm(path) {
        Ok(v) => v,
        Err(e) => {
            self.status_message = crate::tstatus!("⚠️ Kunde inte läsa '{}': {}", path, e);
            return;
        }
    };
    let duration_secs = if sr > 0 { l.len() as f32 / sr as f32 } else { 0.0 };
    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let length_bars = (tempo.bars_for_secs_at(0.0, duration_secs as f64) as f32).max(0.25);

    // Downsample a peak envelope for the timeline waveform display.
    let points = 256usize;
    let mut peaks = Vec::with_capacity(points);
    if !l.is_empty() {
        let chunk = (l.len() / points).max(1);
        let mut i = 0;
        while i < l.len() {
            let end = (i + chunk).min(l.len());
            let mut m = 0.0f32;
            for s in &l[i..end] {
                m = m.max(s.abs());
            }
            peaks.push(m);
            i = end;
        }
    }

    let clean_name = std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| crate::i18n::t("Importerat ljud").to_string());

    let arc_l = std::sync::Arc::new(l);
    let arc_r = std::sync::Arc::new(r);
    let new_id = self.playlist_tracks.len() + 1;

    let mut track = PlaylistTrack::new(
        format!("{} {}", tmpl.icon, clean_name),
        tmpl.icon,
        tmpl.kind,
        tmpl.color,
    );
    track.pcm_audio = Some((arc_l, arc_r, sr));
    track.custom_clip_name = Some(clean_name.clone());
    track.regions = vec![AudioRegion {
        source_bpm: 0.0, // filens tempo är okänt → rör inte ljudet (8.10)
        tape: false,
        id: new_id,
        name: clean_name.clone(),
        start_bar: 0.0,
        length_bars,
        sample_offset_sec: 0.0,
        source_path: Some(path.to_string()),
        waveform_peaks: peaks,
        volume: 1.0,
        fade_in_bars: 0.0,
        fade_out_bars: 0.0,
        muted: false,
        is_reverse: false,
        color: tmpl.color,
        loop_length_bars: 0.0,
    }];

    self.playlist_tracks.push(track);
    let idx = self.playlist_tracks.len() - 1;
    self.sync_track_stem_to_engine(idx);
    self.status_message = crate::tstatus!(
        "📁 Importerade '{}' som spår {} ({:.1}s, {} Hz)",
        clean_name,
        idx + 1,
        duration_secs,
        sr
    );
}
}

impl SonixApp {
pub fn trigger_suno_ai_generation(&mut self) {
    if self.suno_prompt_input.trim().is_empty() {
        return;
    }
    // Prefer a configured audio-generation API; fall back to the local
    // rule-based DSP renderer when none is available.
    if self.trigger_remote_audio_generation() {
        return;
    }
    self.generate_suno_local();
}
}

impl SonixApp {
/// Kicks off a background audio-API request. Returns `false` (and does
/// nothing) when no audio provider is configured.
pub fn trigger_remote_audio_generation(&mut self) -> bool {
    let config = crate::audio::ai_client::AiConfig::load();
    if !config.audio_is_ready() {
        return false;
    }
    let prompt = self.suno_prompt_input.clone();
    let bars = (self.loop_end_bar.max(self.loop_start_bar + 4) - self.loop_start_bar).clamp(1, 32);
    let duration_secs = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_for_bars_at(0.0, bars as f64) as f32;
    let slot: std::sync::Arc<std::sync::Mutex<Option<Result<Vec<u8>, String>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    self.ai_audio_pending = Some((slot.clone(), prompt.clone()));
    self.suno_is_generating = true;
    self.suno_generation_progress = 1.0;
    self.status_message = crate::tstatus!(
        "⏳ Genererar ljud via {} ...",
        config.audio_provider.label()
    );
    std::thread::spawn(move || {
        let result = crate::audio::ai_client::generate_audio(&config, &prompt, duration_secs);
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
    true
}
}

impl SonixApp {
pub fn poll_remote_audio_generation(&mut self) {
    let Some((slot, prompt)) = self.ai_audio_pending.clone() else {
        return;
    };
    let result = {
        let mut guard = match slot.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.take()
    };
    let Some(result) = result else {
        return;
    };
    self.ai_audio_pending = None;
    self.suno_is_generating = false;
    match result {
        Ok(bytes) => match self.import_generated_audio_bytes(&bytes, &prompt) {
            Ok(()) => {
                self.suno_context_clip = Some(prompt);
            }
            Err(e) => {
                self.status_message = crate::tstatus!(
                    "⚠ Kunde inte importera AI-ljud ({}), använder lokal motor.",
                    e
                );
                self.generate_suno_local();
            }
        },
        Err(e) => {
            self.status_message = crate::tstatus!(
                "⚠ AI-ljudgenerering misslyckades ({}), använder lokal motor.",
                e
            );
            self.generate_suno_local();
        }
    }
}
}

impl SonixApp {
/// Decodes API audio bytes (WAV/MP3/…) and adds them as a timeline track.
fn import_generated_audio_bytes(&mut self, bytes: &[u8], prompt: &str) -> Result<(), String> {
    let ext = if bytes.starts_with(b"RIFF") {
        "wav"
    } else if bytes.starts_with(b"fLaC") {
        "flac"
    } else if bytes.starts_with(b"OggS") {
        "ogg"
    } else {
        "mp3"
    };
    let dir = std::env::temp_dir().join("sonix_ai_audio");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("kunde inte skapa {}: {e}", dir.display()))?;
    let hash = prompt.bytes().fold(0xcbf29ce4u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    let path = dir.join(format!("ai_{hash:016x}.{ext}"));
    std::fs::write(&path, bytes)
        .map_err(|e| format!("kunde inte skriva {}: {e}", path.display()))?;
    let path_str = path.to_string_lossy().to_string();

    let (l, r, sr) = crate::audio::load_audio_pcm(&path_str)?;

    let title: String = prompt.chars().take(24).collect();
    let mut track = PlaylistTrack::new(
        format!("🎧 AI Audio: {title}"),
        "🎧",
        TrackKind::CustomAudio,
        Color32::from_rgb(120, 200, 255),
    );
    track.volume = 0.90;
    track.pcm_audio = Some((
        std::sync::Arc::new(l),
        std::sync::Arc::new(r),
        sr,
    ));
    track.custom_clip_name = Some(format!("AI Audio: {prompt}"));

    let start_b = self.loop_start_bar.min(31);
    let end_b = self.loop_end_bar.max(start_b + 4).min(32);
    for b in start_b..end_b {
        track.clips[b] = Some(self.selected_pattern);
    }

    self.playlist_tracks.push(track);
    let idx = self.playlist_tracks.len() - 1;
    self.sync_track_stem_to_engine(idx);
    self.status_message = crate::tstatus!(
        "✅ Importerade AI-genererat ljud som spår {}: '{}'",
        idx + 1,
        title
    );
    Ok(())
}
}

impl SonixApp {
pub fn generate_suno_local(&mut self) {
    if self.suno_prompt_input.trim().is_empty() {
        return;
    }
    self.suno_is_generating = true;
    self.suno_generation_progress = 1.0;

    let prompt = self.suno_prompt_input.clone();
    let prompt_lower = prompt.to_lowercase();

    let bars = (self.loop_end_bar.max(self.loop_start_bar + 4) - self.loop_start_bar).clamp(1, 32);
    let bpm = self.bpm.max(40.0);
    let sr = 44100u32;
    let prompt_seed = prompt.bytes().fold(0xcbf29ce4u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });

    if prompt_lower.contains("stem") || prompt_lower.contains("full") || prompt_lower.contains("hela låt") {
        // Multi-Stem Generation (Drums, Bass, Vocal, Synth)
        let stem_defs = [
            ("🎙 AI Lead Vocal (ACE-Step)", TrackKind::VocalAudio, Color32::from_rgb(180, 110, 255), GenStyle::Vocal),
            ("🥁 AI Drum Stems (Stable Audio)", TrackKind::Drums, Theme::FL_CYAN, GenStyle::Drums),
            ("🎸 AI Sub Bass (MusicGen)", TrackKind::Bassline, Color32::from_rgb(255, 80, 140), GenStyle::Bass),
            ("✨ AI Synth Harmony (ACE-Step)", TrackKind::SynthLead, Theme::FL_GREEN, GenStyle::Synth),
        ];

        let first_new = self.playlist_tracks.len();
        for (idx, (s_name, kind, col, style)) in stem_defs.iter().enumerate() {
            let (pcm_l, pcm_r) = crate::audio::ai_generator::render_generated_audio(
                *style,
                bpm,
                bars,
                sr,
                prompt_seed ^ (idx as u64).wrapping_mul(0x9e3779b97f4a7c15),
            );

            let mut track = PlaylistTrack::new(s_name.to_string(), "✨", *kind, *col);
            track.volume = 0.90;
            track.pcm_audio = Some((std::sync::Arc::new(pcm_l), std::sync::Arc::new(pcm_r), sr));
            track.custom_clip_name = Some(format!("Stem: {}", prompt));
            for b in self.loop_start_bar..self.loop_end_bar.min(32) {
                track.clips[b] = Some(self.selected_pattern);
            }
            self.playlist_tracks.push(track);
        }
        for i in first_new..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
        }
        self.status_message = crate::tstatus!("✨ ACE-Step/Stable Audio genererade 4 synkade stems (verklig PCM) för: '{}'", prompt);
    } else {
        // Single Track Generation or Inpainting
        let (track_name, style) = if prompt_lower.contains("trum") || prompt_lower.contains("drum") || prompt_lower.contains("beat") {
            ("🥁 AI Drum Groove (Stable Audio)", GenStyle::Drums)
        } else if prompt_lower.contains("sång") || prompt_lower.contains("vocal") || prompt_lower.contains("voice") {
            ("🎙 AI Vocal Lead (ACE-Step)", GenStyle::Vocal)
        } else if prompt_lower.contains("bas") || prompt_lower.contains("bass") {
            ("🎸 AI Sub Bassline (MusicGen)", GenStyle::Bass)
        } else {
            ("✨ AI Synth Harmony (ACE-Step)", GenStyle::Synth)
        };

        let (pcm_l, pcm_r) = crate::audio::ai_generator::render_generated_audio(style, bpm, bars, sr, prompt_seed);

        let mut new_track = PlaylistTrack::new(track_name.to_string(), "✨", TrackKind::CustomAudio, Color32::from_rgb(175, 115, 255));
        new_track.volume = 0.90;
        new_track.pcm_audio = Some((std::sync::Arc::new(pcm_l), std::sync::Arc::new(pcm_r), sr));
        new_track.custom_clip_name = Some(format!("AI Clip: {}", prompt));

        let start_b = self.loop_start_bar.min(31);
        let end_b = self.loop_end_bar.max(start_b + 4).min(32);
        for b in start_b..end_b {
            new_track.clips[b] = Some(self.selected_pattern);
        }

        self.playlist_tracks.push(new_track);
        let new_idx = self.playlist_tracks.len() - 1;
        self.sync_track_stem_to_engine(new_idx);
        self.status_message = crate::tstatus!("✨ Genererade ny tagning (Takt {}-{}) via {}: '{}'", start_b + 1, end_b, self.suno_model_version, prompt);
    }

    self.suno_context_clip = Some(prompt);
    self.suno_is_generating = false;
}
}

impl SonixApp {
/// Feeds live project state into the AI assistant so prompts are grounded
/// in the current key/tempo/selection (Etapp D).
pub fn refresh_ai_context(&mut self) {
    let root = (self.song_key_root % 12) as usize;
    // Tonarten kommer ur projektets EGEN tabell (Fas 8.11): etiketten AI:n får är
    // samma sträng som menyerna visar, inte en tredje lista med egna index.
    let key_label = crate::audio::scale::key_label(self.song_key_root, self.song_key_scale);
    let key_is_minor = crate::audio::scale::scale_at(self.song_key_scale).is_minor();
    let selected_track = self
        .playlist_tracks
        .get(self.selected_timeline_track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let selected_region = self.selected_audio_region.and_then(|(ti, ri)| {
        let track = self.playlist_tracks.get(ti)?;
        let region = track.regions.get(ri)?;
        Some(format!("{} – {}", track.name, region.name))
    });
    self.ai_assistant.context = crate::audio::ai_generator::GenContext {
        project_name: self.project_name.clone(),
        bpm: self.bpm,
        key_label,
        key_pc: Some(root as u8),
        is_minor: key_is_minor,
        selected_track,
        selected_region,
    };
}
}

impl SonixApp {
pub fn scan_for_suno_stems(&mut self) {
    let mut results = Vec::new();
    let scan_dirs: Vec<String> = crate::paths::paths()
        .scan_dirs()
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .chain(std::iter::once("./imported_stems".to_string()))
        .collect();

    for dir in &scan_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && ext.eq_ignore_ascii_case("zip")
                        && let Some(path_str) = path.to_str() {
                            results.push(path_str.to_string());
                        }
            }
        }
    }
    results.sort();
    results.dedup();
    self.detected_suno_zips = results;
}
}

impl SonixApp {
pub fn parse_suno_zip_info(zip_path: &str) -> (String, f32) {
    let filename = std::path::Path::new(zip_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Stems Project");

    // Parse BPM: e.g. "Stems (125.00000000000001BPM)" -> 125.0
    let bpm = if let Some(start_bpm) = filename.find('(') {
        if let Some(end_bpm) = filename[start_bpm..].find("BPM") {
            let bpm_str = &filename[start_bpm + 1..start_bpm + end_bpm];
            bpm_str.parse::<f32>().unwrap_or(120.0)
        } else {
            120.0
        }
    } else {
        120.0
    };

    // Parse Title: extract part before "Stems" or "("
    let title = if let Some(pos) = filename.find("Stems") {
        filename[..pos].trim().trim_end_matches('-').trim().to_string()
    } else if let Some(pos) = filename.find('(') {
        filename[..pos].trim().trim_end_matches('-').trim().to_string()
    } else {
        filename.trim_end_matches(".zip").trim_end_matches(".ZIP").to_string()
    };

    let title = if title.is_empty() { "Ljudprojekt".to_string() } else { title };
    (title, bpm)
}
}

impl SonixApp {
pub fn import_suno_zip(&mut self, zip_path: &str) {
    self.start_suno_zip_import(zip_path);
}
}

impl SonixApp {
/// Startar en ZIP-import — och frågar först om mp3:erna (Fas 8.5).
///
/// Frågan måste ställas **innan** uppackningen, eftersom uppackningen sker i
/// bakgrunden: innehållsförteckningen läses därför utan att packa upp.
pub fn start_suno_zip_import(&mut self, zip_path: &str) {
    let (title, bpm) = Self::parse_suno_zip_info(zip_path);
    let duplicates = duplicate_mp3_count(&zip_entry_names(zip_path));
    if duplicates > 0 {
        self.pending_wav_question = Some(WavQuestion::StemImport {
            source: StemImportSource::Zip(zip_path.to_string()),
            title,
            bpm,
            duplicates,
        });
        self.status_message = crate::tstatus!(
            "❓ {} mp3 i arkivet har redan sin stämma som wav — välj vad som ska läsas in",
            duplicates
        );
        return;
    }
    self.start_suno_zip_import_choice(zip_path, false);
}
}

impl SonixApp {
/// Själva ZIP-importen. Kallas också när frågan är besvarad.
pub fn start_suno_zip_import_choice(&mut self, zip_path: &str, keep_mp3: bool) {
    let (title, bpm) = Self::parse_suno_zip_info(zip_path);
    let sanitized = title.replace([' ', '/', '\\', '(', ')', '\'', '"'], "_");
    let dest_dir = format!("./imported_stems/{}", sanitized);

    let progress = self.stem_import_progress.clone();
    let zip_p = zip_path.to_string();

    {
        if let Ok(mut p) = progress.lock() {
            p.is_importing = true;
            p.stage = "Packar upp ZIP-arkiv i bakgrunden...".to_string();
            p.current_file = std::path::Path::new(&zip_p).file_name().and_then(|n| n.to_str()).unwrap_or("stems.zip").to_string();
            p.current_idx = 0;
            p.total_files = 8;
            p.progress_ratio = 0.05;
            p.completed_payload = None;
            p.error_message = None;
        }
    }

    self.status_message = crate::tstatus!("⏳ Startade inläsning av '{}' i bakgrunden...", title);

    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dest_dir);
        match std::process::Command::new("unzip")
            .args(["-o", &zip_p, "-d", &dest_dir])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    let err_str = String::from_utf8_lossy(&output.stderr);
                    eprintln!("Unzip warning: {}", err_str);
                }
            }
            Err(e) => {
                if let Ok(mut p) = progress.lock() {
                    p.is_importing = false;
                    p.error_message = Some(crate::tstatus!("Kunde inte packa upp ZIP: {}", e));
                }
                return;
            }
        }

        Self::background_decode_stems(&dest_dir, &title, bpm, keep_mp3, progress);
    });
}
}

impl SonixApp {
/// Startar en mappimport — och frågar först om mp3:erna (Fas 8.5).
///
/// Frågan ställs bara när den har något att fråga om: finns inga mp3:er med
/// wav-syskon går importen rakt igenom som förut.
pub fn import_suno_stems_from_folder(&mut self, folder_path: &str, project_title: &str, bpm: f32) {
    let files = list_stem_audio_files(folder_path);
    let (_, duplicates) = stem_files_for_import(files, false);
    if duplicates > 0 {
        self.pending_wav_question = Some(WavQuestion::StemImport {
            source: StemImportSource::Folder(folder_path.to_string()),
            title: project_title.to_string(),
            bpm,
            duplicates,
        });
        self.status_message = crate::tstatus!(
            "❓ {} mp3 har redan sin stämma som wav — välj vad som ska läsas in",
            duplicates
        );
        return;
    }
    self.start_suno_folder_import(folder_path, project_title, bpm, false);
}
}

impl SonixApp {
/// Själva mappimporten. `keep_mp3` är svaret på frågan ovan (`false` = regeln:
/// en mp3 vars stämma redan finns som wav hoppas över).
pub fn start_suno_folder_import(
    &mut self,
    folder_path: &str,
    project_title: &str,
    bpm: f32,
    keep_mp3: bool,
) {
    let progress = self.stem_import_progress.clone();
    let folder = folder_path.to_string();
    let title = project_title.to_string();

    {
        if let Ok(mut p) = progress.lock() {
            p.is_importing = true;
            p.stage = crate::i18n::t("Förbereder avkodning av stämmor...").to_string();
            p.current_file = String::new();
            p.current_idx = 0;
            p.total_files = 8;
            p.progress_ratio = 0.05;
            p.completed_payload = None;
            p.error_message = None;
        }
    }

    self.status_message = crate::tstatus!("⏳ Startade inläsning av stämmor för '{}' i bakgrunden...", project_title);

    std::thread::spawn(move || {
        Self::background_decode_stems(&folder, &title, bpm, keep_mp3, progress);
    });
}
}

impl SonixApp {
pub(crate) fn background_decode_stems(
    folder_path: &str,
    project_title: &str,
    bpm: f32,
    keep_mp3: bool,
    progress: std::sync::Arc<std::sync::Mutex<StemImportProgress>>,
) {
    // Samma listning som frågan ovanför använder — annars kunde frågan nämna
    // ett antal som den här vägen inte känner igen.
    let stem_files = list_stem_audio_files(folder_path);

    if stem_files.is_empty() {
        if let Ok(mut p) = progress.lock() {
            p.is_importing = false;
            p.error_message = Some(crate::tstatus!("Inga ljudfiler hittades i mappen: {}", folder_path));
        }
        return;
    }

    // Samma stämma som wav behöver sin mp3 inte — appen kan inte spela mp3,
    // bara konvertera, och det är ett varv utan vinst. Utan wav-syskon tas
    // mp3:n med. `keep_mp3` är användarens svar på frågan (Fas 8.5); den här
    // vägen kör samma regel som frågan bygger på.
    let (stem_files, skipped_mp3) = stem_files_for_import(stem_files, keep_mp3);
    if skipped_mp3 > 0
        && let Ok(mut p) = progress.lock()
    {
        p.error_message = Some(crate::tstatus!(
            "⏭ Hoppade över {} mp3 — samma stämma finns som wav",
            skipped_mp3
        ));
    }
    let total_files = stem_files.len();

    // 🧹 Städa bort AI-/Sunohärkomsten ur filerna INNAN de läses in (8.5b).
    //
    // Här är rätt ögonblick: det som städas nu kan inte följa med ut i ett projekt
    // eller en export senare, och det sker medan användaren redan väntar på
    // importen. Suno skriver t.ex. `comment = "Made with Suno; Created=…; id=…"`
    // i sina filer — deras anspråk och spår, i användarens fil.
    //
    // **Bara härkomsten.** Titel, artist, låttext och omslag är musik, och de
    // lämnas (Alex 2026-09-13: den första versionen tog hela taggen, alltså också
    // låttexten och omslaget — för mycket). Regeln står i `metadata.rs`.
    //
    // Tyst städning vore samma sorts tystnad som gav de tysta klippen. Därför
    // räknas den och skrivs i statusraden: vad som togs bort, och hur mycket.
    let mut cleaned_tags: Vec<String> = Vec::new();
    for f in &stem_files {
        let f_str = f.to_string_lossy().to_string();
        if let Ok(report) = crate::audio::metadata::scan(&f_str)
            && report.removable_bytes > 0
            && crate::audio::metadata::strip_tags(&f_str).is_ok()
        {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| f_str.clone());
            cleaned_tags.push(format!("{name} ({} byte)", report.removable_bytes));
        }
    }
    if !cleaned_tags.is_empty()
        && let Ok(mut p) = progress.lock()
    {
        p.error_message = Some(crate::tstatus!(
            "🧹 Tog bort AI-/Sunohärkomst ur {} fil(er) (text, id, C2PA): {} — titel, sångtext och omslag lämnas.",
            cleaned_tags.len(),
            cleaned_tags.join(", ")
        ));
    }


    {
        if let Ok(mut p) = progress.lock() {
            p.total_files = total_files;
            p.stage = crate::tstatus!("Hittade {} stämspår. Avkodar PCM-ljud...", total_files);
            p.progress_ratio = 0.10;
        }
    }

    let tempo = crate::audio::tempo::TempoMap::single(bpm.max(40.0));
    let mut max_stem_bars = 32.0_f32;
    let mut decoded_tracks = Vec::with_capacity(total_files);

    for (idx, file_path) in stem_files.iter().enumerate() {
        let fname = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("Stem");
        let clean_name = fname
            .trim_end_matches(".wav")
            .trim_end_matches(".WAV")
            .trim_end_matches(".mp3")
            .trim_end_matches(".MP3")
            .trim_end_matches(".flac")
            .trim_end_matches(".FLAC")
            .trim_end_matches(".ogg")
            .trim_end_matches(".OGG");

        let path_str = file_path.to_string_lossy().to_string();

        {
            if let Ok(mut p) = progress.lock() {
                p.current_idx = idx + 1;
                p.current_file = fname.to_string();
                p.stage = crate::tstatus!("Läser & avkodar spår {}/{}: {}", idx + 1, total_files, clean_name);
                p.progress_ratio = 0.10 + 0.85 * (idx as f32 / total_files as f32);
            }
        }

        let mut file_bars = 32.0_f32;
        let mut pcm_l_opt = None;
        let mut pcm_r_opt = None;
        let mut sample_rate = 44100u32;
        let mut wave_env = Vec::new();
        // Vågformscachen byggs HÄR och inte i ritningen (Fas 8.3): samplen är
        // redan varma efter avkodningen, och den här tråden är ändå en
        // arbetstråd. Annars står den grova översikten i bildrutan tills en
        // andra tråd hunnit bygga cachen — och Alex' krav är att vågformen är
        // exakt från första bildrutan.
        let mut waveform_cache = None;

        if let Ok((l, r, sr)) = crate::audio::load_audio_pcm(&path_str) {
                sample_rate = sr;
                let total_secs = l.len() as f32 / sr.max(1) as f32;
                file_bars =
                    (tempo.bars_for_secs_at(0.0, total_secs as f64) as f32).max(1.0);
                max_stem_bars = max_stem_bars.max(file_bars);

                let arc_l = std::sync::Arc::new(l);
                let arc_r = std::sync::Arc::new(r);

                // EN genomgång av samplen ger både det tidslinjen ritar
                // (cachen) och den grova översikt som följer med regionen.
                //
                // Den gamla översikten läste var 64:e sample, så ett enstaka
                // anslag kunde falla mellan två läsningar och försvinna helt.
                // Ett (min, max) per fack kan inte missa något — varje sample
                // ligger i ett fack.
                let cache = crate::audio::waveform::WaveformCache::build(&arc_l);
                let num_points = ((file_bars * 16.0) as usize).clamp(240, 4800);
                wave_env = overview_peaks_from(&cache, &arc_l, num_points);

                waveform_cache = Some(cache);
                pcm_l_opt = Some(arc_l);
                pcm_r_opt = Some(arc_r);
            }

        if wave_env.is_empty() {
            // Decode failed: draw an honest flat line instead of a fake waveform.
            let num_points = ((file_bars * 16.0) as usize).clamp(240, 4800);
            wave_env = vec![0.04; num_points];
        }

        let (kind, icon, color) = classify_track_style(clean_name);

        decoded_tracks.push(DecodedStemTrack {
            clean_name: clean_name.to_string(),
            path_str,
            kind,
            icon,
            color,
            pcm_left: pcm_l_opt,
            pcm_right: pcm_r_opt,
            sample_rate,
            file_bars,
            wave_env,
            waveform_cache,
        });
    }

    if let Ok(mut p) = progress.lock() {
        p.is_importing = false;
        p.progress_ratio = 1.0;
        p.stage = crate::i18n::t("Slutför inläsning...").to_string();
        p.completed_payload = Some(StemImportResult {
            project_title: project_title.to_string(),
            bpm,
            max_stem_bars,
            tracks: decoded_tracks,
        });
    }
}
}

impl SonixApp {
pub fn apply_imported_stems(&mut self, res: StemImportResult) {
    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    let mut new_tracks = Vec::new();

    for (idx, dt) in res.tracks.into_iter().enumerate() {
        let DecodedStemTrack {
            clean_name,
            path_str,
            kind,
            icon,
            color,
            pcm_left,
            pcm_right,
            sample_rate,
            file_bars,
            wave_env,
            waveform_cache,
        } = dt;

        // Motorn och spåret delar SAMMA buffert (Arc), inte en kopia var: spåret
        // behöver ljudet för att kunna rita vågformen och för att kunna klippa
        // och exportera ur minnet, och `LoadStemTrack` tar redan ett Arc.
        //
        // Cachen byggdes ur just den bufferten i avkodningstråden, så nyckeln
        // stämmer och tidslinjen ritar det exakta höljet redan i första
        // bildrutan — i stället för den grova översikten tills en andra tråd
        // hunnit ikapp.
        let (track_pcm, track_waveform) = imported_track_waveform(
            pcm_left.zip(pcm_right).map(|(l, r)| (l, r, sample_rate)),
            waveform_cache,
        );

        if let Some((arc_l, arc_r, sr)) = track_pcm.clone() {
            let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                track_index: idx,
                left: arc_l,
                right: arc_r,
                sample_rate: sr as f32,
                volume: 0.9,
                pan: 0.0,
                start_time_secs: 0.0,
            });
        }

        let initial_region = AudioRegion {
            id: idx + 1,
            name: clean_name.clone(),
            start_bar: 0.0,
            length_bars: file_bars,
            sample_offset_sec: 0.0,
            source_path: Some(path_str),
            waveform_peaks: wave_env,
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color,
            loop_length_bars: 0.0,
            // Klippet spelades in i Sunos tempo (Fas 8.10). Ändras tempot sedan
            // följer ljudet med i stället för att hamna ur takt.
            //
            // **Saknar Suno ett BPM** (arkivnamnet "Rock and Hard Place Stems.zip"
            // bär inget tal, och taggarna bär inget heller) är projektets tempo det
            // rätta: takterna räknades just ur filens längd i *det* tempot
            // (`bars_for_secs_at` ovan), så filen täcker klippet exakt. Att lämna 0
            // där gjorde att klippen varken följde tempot eller räckte till låtens
            // slut så snart tempot flyttades — Alex' 200 BPM skar av 103,71 s.
            source_bpm: if res.bpm > 0.0 { res.bpm } else { self.bpm },
            tape: false,
        };

        let mut track = PlaylistTrack::new(format!("{} {}", icon, clean_name), icon, kind, color);
        track.volume = 0.9;
        track.is_rec_armed = false;
        track.regions = vec![initial_region];
        track.custom_clip_name = Some(clean_name);
        track.waveform_cache = track_waveform;
        track.pcm_audio = track_pcm;

        for b in 0..32 {
            track.clips[b] = Some(idx);
        }

        new_tracks.push(track);
    }

    self.playlist_tracks = new_tracks;
    self.ensure_mic_track_exists();
    let total_tracks_len = self.playlist_tracks.len();
    self.project_name = res.project_title.clone();
    self.bpm = res.bpm;
    self.loop_start_bar = 0;
    self.loop_end_bar = (res.max_stem_bars.ceil() as usize).max(32);
    for t_idx in 0..self.playlist_tracks.len() {
        self.sync_track_regions(t_idx);
    }
    self.suno_context_clip = Some(format!("{} - Master Mix", res.project_title));
    self.status_message = crate::tstatus!("✨ Importerade {} stämspår för '{}' ({:.1} BPM) med kategorifärger & Mic-spår!", total_tracks_len, res.project_title, res.bpm);
    self.view_mode = ViewMode::PlaylistArranger;
}
}

