use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::HashSet;
use std::time::Instant;

use crate::audio::{
    AdsrParams, AudioCommand, AudioEngine, AudioSettings,
    DelayParams, DrumType, FilterParams, Preset, ReverbParams,
    StemRegionPlayback, TrackEqSettings, Waveform,
};
use crate::audio::ai_generator::{AiMusicAssistant, GenStyle};
use crate::audio::hardware_control::{control_channel, ControlEvent, McuInput, OscServer};
use crate::audio::MidiKeyboardInput;
use crate::audio::patcher::{ModularGraph, PatchSpec};
use crate::audio::plugin_host::PluginManager;
use crate::audio::recorder::{LiveMicrophoneCapture, VocalStudioTrack};
use crate::audio::stem_separator::{run_separation, SeparationResult, StemProject};
use crate::audio::vocal_harmonizer::VocalHarmonizer;
use super::add_track_modal::{render_add_track_modal, AddTrackModalState, TrackTemplate};
use super::ai_assistant_view::render_ai_assistant_view;
use super::chord_generator_modal::{render_chord_generator_modal, ChordGeneratorState, GeneratedChord};
use super::dice_generator_modal::{render_dice_generator_modal, DiceGeneratorState};
use super::fx_rack_modal::{render_fx_rack_modal, FxRackState};
use super::patcher_view::render_patcher_view;
use super::plugins_view::render_plugins_view;
use super::song_structure_modal::{render_song_structure_modal, SongStructureState, SongSectionItem};
use super::stem_view::render_stem_separator_view;
use super::theme::Theme;
use super::tuner_modal::{render_tuner_modal, TunerState};
use super::vocal_studio_view::render_vocal_studio_view;
use super::widgets::{
    alchemy_transform_matrix, drummer_xy_matrix, eq_curve_visualizer, fl_step_button,
    mini_track_eq_curve, oscilloscope_display, pitch_knob, rotary_knob, vertical_fader,
};

fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn note_name(note: u8) -> &'static str {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    names[(note % 12) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    ChannelRack,
    PianoRoll,
    PlaylistArranger,
    SessionDrummer,
    AlchemySynth,
    RemixFx,
    VocalStudio,
    AiMusicAssistant,
    ModularPatcher,
    StemSeparator,
    PluginManager,
    EffectsMixer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangerTool {
    Select, // ⇱ Markör / Flytta / Trimma
    Paint,  // ✎ Rita
    Slice,  // ✂ Klipp / Sax
    Erase,  // 🗑 Radera
    Mute,   // 🔇 Muta
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionDragMode {
    Move,
    TrimStart,
    TrimEnd,
}

#[derive(Debug, Clone)]
pub struct RegionDragState {
    pub track_idx: usize,
    pub region_idx: usize,
    pub mode: RegionDragMode,
    pub initial_start_bar: f32,
    pub initial_length_bars: f32,
    pub initial_sample_offset_sec: f32,
    pub initial_loop_length_bars: f32,
    pub drag_start_mouse_x: f32,
}

#[derive(Clone)]
pub struct LibrarySampleItem {
    #[allow(dead_code)]
    pub id: usize,
    pub name: String,
    pub category: String,
    pub icon: String,
    pub default_note: u8,
    pub color: Color32,
    pub waveform: Vec<f32>,
    pub file_path: Option<String>,
}

#[derive(Clone)]
pub struct ChannelStrip {
    pub name: String,
    pub icon: String,
    pub color: Color32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub steps: [bool; 16],
    pub notes: [u8; 16],
    pub pitch_semitones: i8, // -24 .. +24 st
    pub pitch_fine_cents: f32, // -100.0 .. +100.0 cents
    pub sample_start: f32, // 0.0 .. 1.0 (Chopping start)
    pub sample_end: f32, // 0.0 .. 1.0 (Chopping end)
    pub attack_decay: f32, // 0.0 .. 1.0
    pub is_reverse: bool,
    pub waveform_preview: Vec<f32>,
    pub sample_path: Option<String>,
    pub pcm_audio: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    pub sample_base_note: u8,
}

#[derive(Clone, Debug)]
pub struct Pattern {
    pub name: String,
    pub color: Color32,
    pub channel_steps: Vec<[bool; 16]>,
    pub channel_notes: Vec<[u8; 16]>,
    pub piano_roll_grid: [[bool; 16]; 24],
    /// Den inspelade tagningen med sin faktiska tajming (Fas 6.4). Rutnätet
    /// säger *vilka* steg som spelar, tagningen säger *när* inom steget — det är
    /// den som gör kvantisering och humanisering möjliga i efterhand.
    pub take: crate::midi_take::Take,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeSnapMode {
    FreeHundredth, // ⚡ 0.01s (Fri / Högsta precision)
    Snap16th,      // 1/16-dels takt
    SnapBeat,      // 1/4 Beat
    SnapBar,       // 1 Hel takt
}

pub fn format_time_hundredths(seconds: f32) -> String {
    let total_cs = (seconds.max(0.0) * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_sec = total_cs / 100;
    let s = total_sec % 60;
    let m = total_sec / 60;
    format!("{:02}:{:02}.{:02}", m, s, cs)
}

pub fn format_bar_subdivisions(bar_float: f32) -> String {
    let bar_idx = bar_float.floor() as i32 + 1;
    let rem_bar = (bar_float - bar_float.floor()).max(0.0);
    let beat = (rem_bar * 4.0).floor() as i32 + 1;
    let step = ((rem_bar * 16.0) % 4.0).floor() as i32 + 1;
    let cs = ((rem_bar * 100.0) % 25.0).floor() as i32;
    format!("{} {:03}.{}.{} (+{:02}cs)", crate::i18n::t("Takt"), bar_idx, beat, step, cs)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackKind {
    VocalAudio,
    CustomAudio,
    Drums,
    SynthLead,
    Bassline,
    Fx,
}

#[derive(Clone, Debug)]
pub struct SongMarker {
    pub start_bar: usize,
    pub length_bars: usize,
    pub name: String,
    pub color: Color32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AudioRegion {
    pub id: usize,
    pub name: String,
    pub start_bar: f32,
    pub length_bars: f32,
    pub sample_offset_sec: f32,
    pub source_path: Option<String>,
    pub waveform_peaks: Vec<f32>,
    pub volume: f32,
    pub fade_in_bars: f32,
    pub fade_out_bars: f32,
    pub muted: bool,
    #[serde(default)]
    pub is_reverse: bool,
    #[serde(skip, default = "default_region_color")]
    pub color: Color32,
    #[serde(default)]
    pub loop_length_bars: f32,
}

fn default_region_color() -> Color32 {
    Color32::from_rgb(100, 180, 255)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrackEq {
    #[serde(default)]
    pub low_gain_db: f32,
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default)]
    pub mid_gain_db: f32,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub high_gain_db: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_low_freq() -> f32 { 100.0 }
fn default_mid_freq() -> f32 { 1500.0 }
fn default_mid_q() -> f32 { 1.0 }
fn default_high_freq() -> f32 { 8000.0 }
fn default_true() -> bool { true }
fn default_comp_thresh() -> f32 { 0.0 }
fn default_comp_ratio() -> f32 { 1.0 }

impl TrackEq {
    pub fn to_settings(&self) -> TrackEqSettings {
        TrackEqSettings {
            enabled: self.enabled,
            low_freq: self.low_freq,
            low_gain_db: self.low_gain_db,
            mid_freq: self.mid_freq,
            mid_gain_db: self.mid_gain_db,
            mid_q: self.mid_q,
            high_freq: self.high_freq,
            high_gain_db: self.high_gain_db,
        }
    }
}

impl Default for TrackEq {
    fn default() -> Self {
        Self {
            low_gain_db: 0.0,
            low_freq: 100.0,
            mid_gain_db: 0.0,
            mid_freq: 1500.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            high_freq: 8000.0,
            enabled: true,
        }
    }
}

/// Automatable per-track parameters. The order defines the cache index used by
/// `PlaylistTrack::automation_last`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum AutomationParam {
    Volume,
    Pan,
    ReverbSend,
    DelaySend,
}

impl AutomationParam {
    pub const ALL: [AutomationParam; 4] = [
        AutomationParam::Volume,
        AutomationParam::Pan,
        AutomationParam::ReverbSend,
        AutomationParam::DelaySend,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AutomationParam::Volume => crate::i18n::t("Volym"),
            AutomationParam::Pan => crate::i18n::t("Panorering"),
            AutomationParam::ReverbSend => crate::i18n::t("Reverb-send"),
            AutomationParam::DelaySend => crate::i18n::t("Delay-send"),
        }
    }

    /// Inclusive value range used both for editing and for clamping.
    pub fn range(self) -> (f32, f32) {
        match self {
            AutomationParam::Volume => (0.0, 1.5),
            AutomationParam::Pan => (-1.0, 1.0),
            AutomationParam::ReverbSend | AutomationParam::DelaySend => (0.0, 1.0),
        }
    }

    fn index(self) -> usize {
        match self {
            AutomationParam::Volume => 0,
            AutomationParam::Pan => 1,
            AutomationParam::ReverbSend => 2,
            AutomationParam::DelaySend => 3,
        }
    }
}

/// A single breakpoint on an automation curve.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationPoint {
    pub time_secs: f32,
    pub value: f32,
}

/// One automated parameter for a track. Points are kept sorted by time.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationLane {
    pub param: AutomationParam,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
}

impl AutomationLane {
    /// Linearly interpolated value at `t`; constant before the first and after
    /// the last point. `None` when the lane has no points.
    pub fn value_at(&self, t: f32) -> Option<f32> {
        if self.points.is_empty() {
            return None;
        }
        let first = &self.points[0];
        if t <= first.time_secs {
            return Some(first.value);
        }
        let last = self.points.last().unwrap();
        if t >= last.time_secs {
            return Some(last.value);
        }
        for w in self.points.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t >= a.time_secs && t <= b.time_secs {
                let span = (b.time_secs - a.time_secs).max(1e-6);
                let f = (t - a.time_secs) / span;
                return Some(a.value + (b.value - a.value) * f);
            }
        }
        Some(last.value)
    }

    fn sort_points(&mut self) {
        self.points.sort_by(|a, b| {
            a.time_secs
                .partial_cmp(&b.time_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

fn snap_time_secs(mode: TimeSnapMode, raw: f32, sec_per_bar: f32) -> f32 {
    match mode {
        TimeSnapMode::FreeHundredth => (raw * 100.0).round() / 100.0,
        TimeSnapMode::Snap16th => {
            let step = sec_per_bar / 16.0;
            (raw / step).round() * step
        }
        TimeSnapMode::SnapBeat => {
            let step = sec_per_bar / 4.0;
            (raw / step).round() * step
        }
        TimeSnapMode::SnapBar => (raw / sec_per_bar).round() * sec_per_bar,
    }
}

/// En autosave som erbjuds vid start (Fas 6.1).
#[derive(Clone)]
pub struct RecoveryCandidate {
    pub entry: crate::autosave::AutosaveEntry,
    /// Projektets läsbara namn, läst ur autosavens JSON (annars sluggen).
    pub name: String,
}

/// Autosaves som är nyare än motsvarande manuella projektfil.
///
/// Jämförelsen görs mot den *manuella* filens ändringstid, inte mot en sparad
/// flagga: har användaren sparat efter autosaven finns det inget att rädda, och
/// då ska ingen dialog stå i vägen. Saknas den manuella filen helt (krasch före
/// första sparningen) är autosaven allt som finns och erbjuds alltid.
fn collect_recovery_candidates() -> Vec<RecoveryCandidate> {
    collect_recovery_candidates_in(&crate::paths::paths())
}

/// Samma sak mot en explicit sökvägsuppsättning (testbar utan miljöberoende).
fn collect_recovery_candidates_in(paths: &crate::paths::Paths) -> Vec<RecoveryCandidate> {
    let mut out: Vec<RecoveryCandidate> = Vec::new();
    for entry in crate::autosave::latest_per_project(&paths.autosave_dir()) {
        let Ok(bytes) = std::fs::read(&entry.path) else {
            continue;
        };
        let name = serde_json::from_slice::<SonixProjectData>(&bytes)
            .map(|d| d.name)
            .unwrap_or_else(|_| entry.project.replace('-', " "));
        let manual = std::fs::metadata(paths.project_file(&name))
            .and_then(|m| m.modified())
            .ok();
        if entry.worth_offering(manual) {
            out.push(RecoveryCandidate { entry, name });
        }
    }
    out.sort_by(|a, b| b.entry.stamp.cmp(&a.entry.stamp));
    out.truncate(5);
    out
}

/// Ett projekt i "Senaste projekt"-listan (`recent.json`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    /// Unix-sekunder då projektet senast öppnades eller sparades.
    #[serde(default)]
    pub opened: u64,
}

/// Hur många projekt läslistan minns.
const RECENT_MAX: usize = 8;

/// Läser `recent.json`. Trasig eller saknad fil ger en tom lista — läslistan är
/// en bekvämlighet, aldrig en förutsättning för att kunna öppna ett projekt.
fn load_recent_projects() -> Vec<RecentProject> {
    load_recent_projects_in(&crate::paths::paths())
}

/// Samma läsning mot en explicit sökvägsuppsättning (testbar).
fn load_recent_projects_in(paths: &crate::paths::Paths) -> Vec<RecentProject> {
    let path = paths.recent_file();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut entries: Vec<RecentProject> = serde_json::from_str(&text).unwrap_or_default();
    // Filen kan ha flyttats eller städats bort utanför appen.
    entries.retain(|e| std::path::Path::new(&e.path).exists());
    entries.truncate(RECENT_MAX);
    entries
}

/// Skriver läslistan atomiskt (samma temp+rename-väg som autosaven).
fn store_recent_projects(entries: &[RecentProject]) {
    store_recent_projects_in(&crate::paths::paths(), entries)
}

/// Samma skrivning mot en explicit sökvägsuppsättning (testbar).
fn store_recent_projects_in(paths: &crate::paths::Paths, entries: &[RecentProject]) {
    let path = paths.recent_file();
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = crate::autosave::write_atomic(&path, json.as_bytes());
    }
}

/// Lägger ett projekt först i läslistan (flyttar upp det om det redan finns).
fn push_recent_project(entries: &mut Vec<RecentProject>, name: &str, path: &str) {
    entries.retain(|e| e.path != path);
    entries.insert(
        0,
        RecentProject {
            name: name.to_string(),
            path: path.to_string(),
            opened: crate::autosave::now_stamp(),
        },
    );
    entries.truncate(RECENT_MAX);
}

/// Ett pattern som det sparas i projektfilen (Fas 6.7).
///
/// `Pattern` själv kan inte serialiseras (Color32 saknar serde), och färgen
/// sparas därför som RGBA.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedPattern {
    pub name: String,
    #[serde(default = "default_ui_color")]
    pub color: [u8; 4],
    pub channel_steps: Vec<[bool; 16]>,
    pub channel_notes: Vec<[u8; 16]>,
    pub piano_roll_grid: [[bool; 16]; 24],
    /// Tagningen med sin tajming (Fas 6.4). Tom i äldre projekt.
    #[serde(default)]
    pub take: crate::midi_take::Take,
}

/// En kanal i Channel Racket som den sparas (Fas 6.7).
///
/// `pcm_audio` och `waveform_preview` sparas **inte**: ljudet ligger redan på
/// disk och läses tillbaka från `sample_path`, och vågformen räknas om ur
/// ljudet. Att spara dem skulle blåsa upp projektfilen med hundratals kilobyte
/// per kanal utan att tillföra något.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedChannel {
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_ui_color")]
    pub color: [u8; 4],
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub steps: [bool; 16],
    pub notes: [u8; 16],
    #[serde(default)]
    pub pitch_semitones: i8,
    #[serde(default)]
    pub pitch_fine_cents: f32,
    #[serde(default)]
    pub sample_start: f32,
    #[serde(default = "default_sample_end")]
    pub sample_end: f32,
    #[serde(default)]
    pub attack_decay: f32,
    #[serde(default)]
    pub is_reverse: bool,
    #[serde(default)]
    pub sample_path: Option<String>,
    #[serde(default)]
    pub sample_base_note: u8,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SonixProjectData {
    pub name: String,
    pub bpm: f32,
    /// Tempobyten (Fas 8.2). Saknas i äldre projekt — då gäller `bpm` som förut,
    /// och filen läses exakt som den skrevs.
    #[serde(default)]
    pub tempo_points: Vec<crate::audio::tempo::TempoPoint>,
    pub swing: f32,
    pub master_volume: f32,
    pub master_pan: f32,
    pub tracks: Vec<SavedTrackData>,
    /// Per-engine-stem-track plugin inserts. Index-aligned with the engine's
    /// stem tracks; `None` means "no plugin on this track".
    #[serde(default)]
    pub plugin_slots: Vec<Option<SavedPluginData>>,
    /// Sub-mix bus group gains (Fas 5.2). Defaults to unity for old projects.
    #[serde(default = "default_bus_volume")]
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    #[serde(default)]
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    #[serde(default)]
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    /// VCA group gains (Fas 5.2). Defaults to unity for old projects.
    #[serde(default = "default_vca_volume")]
    pub vca_volume: [f32; crate::audio::synth::NUM_VCAS],
    #[serde(default)]
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    #[serde(default)]
    pub vca_solo: [bool; crate::audio::synth::NUM_VCAS],
    /// Mönstren med noterna (Fas 6.7). Saknas i äldre projekt — då skapas
    /// standardpattern som förut, men nya projekt får med sig arbetet.
    #[serde(default)]
    pub patterns: Vec<SavedPattern>,
    #[serde(default)]
    pub selected_pattern: usize,
    /// Stegvolymerna i trumsekvenserna (Fas 6.7). `None` = filen skrevs före
    /// den här ändringen, och då lämnas appens nuvarande värden orörda.
    #[serde(default)]
    pub step_velocities: Option<[f32; 16]>,
    /// Channel Racket (Fas 6.7).
    #[serde(default)]
    pub channels: Vec<SavedChannel>,
}

fn default_bus_volume() -> [f32; crate::audio::synth::NUM_BUSES] {
    [1.0; crate::audio::synth::NUM_BUSES]
}

fn default_ui_color() -> [u8; 4] {
    [120, 120, 130, 255]
}

fn default_sample_end() -> f32 {
    1.0
}

/// Identiteten hos en ljudbuffert: adress, längd och första/sista sample.
///
/// Räcker för att avgöra om vågformscachen hör till ljudet som ligger där nu, och
/// är billig nog att räkna varje bildruta. Innehållet kontrolleras i båda ändar
/// eftersom en frigjord buffert kan få samma adress igen.
fn waveform_key(buf: &std::sync::Arc<Vec<f32>>) -> u64 {
    let ptr = std::sync::Arc::as_ptr(buf) as usize as u64;
    let len = buf.len() as u64;
    let first = buf.first().copied().unwrap_or(0.0).to_bits() as u64;
    let last = buf.last().copied().unwrap_or(0.0).to_bits() as u64;
    ptr.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ len.rotate_left(17)
        ^ first.rotate_left(31)
        ^ last.rotate_left(47)
}

/// Vågform för kanalvisningen, räknad ur ljudet (Fas 6.7). Toppvärdet per
/// fönster räcker — det är samma sorts översikt kanalracket ritar.
fn waveform_preview_from_pcm(pcm: &[f32], points: usize) -> Vec<f32> {
    if pcm.is_empty() || points == 0 {
        return Vec::new();
    }
    let bucket = (pcm.len() / points).max(1);
    pcm.chunks(bucket)
        .map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())))
        .take(points)
        .collect()
}

/// Det som ska tillbaka in i appen när en projektfil öppnas (Fas 6.7).
pub struct SavedMusic<'a> {
    pub patterns: &'a [SavedPattern],
    pub selected_pattern: usize,
    pub step_velocities: Option<[f32; 16]>,
    pub channels: &'a [SavedChannel],
}

/// Lägger tillbaka mönster, kanalrack och stegvolymer.
///
/// Ren funktion (Fas 6.7), så att *inläsningsvägen* kan testas utan GUI — det
/// var just den vägen som saknades: filen hade inga noter att läsa, och ingen
/// kod som ens försökte. Har filen inga mönster (skriven före den här
/// ändringen) lämnas appens standardpatterns och standardrack orörda.
pub fn restore_saved_music(
    saved: &SavedMusic,
    patterns: &mut Vec<Pattern>,
    channels: &mut [ChannelStrip],
    selected_pattern: &mut usize,
    step_velocities: &mut [f32; 16],
) {
    if !saved.patterns.is_empty() {
        for (i, s) in saved.patterns.iter().enumerate() {
            let restored = saved_to_pattern(s);
            if i < patterns.len() {
                patterns[i] = restored;
            } else {
                patterns.push(restored);
            }
        }
        *selected_pattern = saved.selected_pattern.min(patterns.len().saturating_sub(1));
    }
    if let Some(v) = saved.step_velocities {
        *step_velocities = v;
    }
    for (i, s) in saved.channels.iter().enumerate() {
        if i < channels.len() {
            channels[i] = saved_to_channel(s);
        }
    }
}

fn pattern_to_saved(p: &Pattern) -> SavedPattern {
    SavedPattern {
        name: p.name.clone(),
        color: p.color.to_array(),
        channel_steps: p.channel_steps.clone(),
        channel_notes: p.channel_notes.clone(),
        piano_roll_grid: p.piano_roll_grid,
        take: p.take.clone(),
    }
}

fn saved_to_pattern(s: &SavedPattern) -> Pattern {
    Pattern {
        name: s.name.clone(),
        color: Color32::from_rgba_premultiplied(s.color[0], s.color[1], s.color[2], s.color[3]),
        channel_steps: s.channel_steps.clone(),
        channel_notes: s.channel_notes.clone(),
        piano_roll_grid: s.piano_roll_grid,
        take: s.take.clone(),
    }
}

fn channel_to_saved(c: &ChannelStrip) -> SavedChannel {
    SavedChannel {
        name: c.name.clone(),
        icon: c.icon.clone(),
        color: c.color.to_array(),
        volume: c.volume,
        pan: c.pan,
        muted: c.muted,
        solo: c.solo,
        steps: c.steps,
        notes: c.notes,
        pitch_semitones: c.pitch_semitones,
        pitch_fine_cents: c.pitch_fine_cents,
        sample_start: c.sample_start,
        sample_end: c.sample_end,
        attack_decay: c.attack_decay,
        is_reverse: c.is_reverse,
        sample_path: c.sample_path.clone(),
        sample_base_note: c.sample_base_note,
    }
}

/// Bygger tillbaka en kanal. Ljudet läses från `sample_path` igen och vågformen
/// räknas om ur ljudet — det är därför de två inte sparas.
fn saved_to_channel(s: &SavedChannel) -> ChannelStrip {
    let pcm = s.sample_path.as_deref().and_then(load_sample_pcm_arcs);
    let preview = match &pcm {
        Some((left, _, _)) => waveform_preview_from_pcm(left, 128),
        None => Vec::new(),
    };
    ChannelStrip {
        name: s.name.clone(),
        icon: s.icon.clone(),
        color: Color32::from_rgba_premultiplied(s.color[0], s.color[1], s.color[2], s.color[3]),
        volume: s.volume,
        pan: s.pan,
        muted: s.muted,
        solo: s.solo,
        steps: s.steps,
        notes: s.notes,
        pitch_semitones: s.pitch_semitones,
        pitch_fine_cents: s.pitch_fine_cents,
        sample_start: s.sample_start,
        sample_end: s.sample_end,
        attack_decay: s.attack_decay,
        is_reverse: s.is_reverse,
        waveform_preview: preview,
        sample_path: s.sample_path.clone(),
        pcm_audio: pcm,
        sample_base_note: s.sample_base_note,
    }
}


fn default_vca_volume() -> [f32; crate::audio::synth::NUM_VCAS] {
    [1.0; crate::audio::synth::NUM_VCAS]
}

/// A plugin insert persisted with the project: its shared-object path plus the
/// opaque state blob produced by `clap.state`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedPluginData {
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: Vec<u8>,
    /// Whether the plugin was loaded in an out-of-process sandbox (Fas 4.5b).
    #[serde(default)]
    pub sandboxed: bool,
}

/// In-memory mirror of [`SavedPluginData`] kept on [`SonixApp`].
#[derive(Clone)]
pub struct PluginSlot {
    pub path: String,
    pub name: String,
    pub state: Vec<u8>,
    pub sandboxed: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedTrackData {
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub clips: [Option<usize>; 32],
    pub regions: Vec<AudioRegion>,
    #[serde(default)]
    pub eq: TrackEq,
    #[serde(default = "default_comp_thresh")]
    pub comp_threshold_db: f32,
    #[serde(default = "default_comp_ratio")]
    pub comp_ratio: f32,
    #[serde(default)]
    pub reverb_send: f32,
    #[serde(default)]
    pub delay_send: f32,
    #[serde(default)]
    pub automation: Vec<AutomationLane>,
    /// Sub-mix bus assignment (Fas 5.2). Defaults to bus 0 for old projects.
    #[serde(default)]
    pub bus: usize,
    /// Optional VCA group assignment (Fas 5.2).
    #[serde(default)]
    pub vca: Option<usize>,
    /// Fruset spår (Tier 2): var ljudet ligger och fingeravtrycket av källan.
    /// Äldre projektfil utan fältet läses som ofrusade.
    #[serde(default)]
    pub frozen: Option<SavedFrozenTrack>,
}

/// Ett fruset spår i projektfilen. Ljudet ligger i en fil i projektets egen
/// materialmapp — filen bär sökvägen, inte ljudet, precis som för samplingar.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SavedFrozenTrack {
    pub path: String,
    pub digest: u64,
    #[serde(default)]
    pub stamp: u64,
}

/// Ett fruset spår (Tier 2): spåret ligger färdigrenderat som ljud, och
/// pattern-uppspelningen hoppas över till förmån för det. Motorn strömmar
/// ljudet som ett stem-spår — samma väg ljudspåren redan använder — så ingen ny
/// uppspelningsväg behöver underhållas.
#[derive(Clone, Debug)]
pub struct FrozenTrack {
    /// WAV-filen i projektets egen materialmapp (`Frozen/`).
    pub path: String,
    /// Fingeravtryck av det som påverkade renderingen: tempo, spårets klipp och
    /// fader, mönstren klippen pekar på, kanalracket och röstinställningarna.
    /// Ändras något av det visas frysningen som inaktuell i stället för att
    /// spela fel ljud tyst. Det är en varning, inte ett bevis: fingeravtrycket
    /// ser att något skiljer sig, det avgör inte vad som är rätt.
    pub digest: u64,
    /// Unix-tid för frysningen, så att åldern går att visa.
    pub stamp: u64,
}

/// Klippen som ska trigga i en rendering: ett fruset spår har sitt ljud i
/// stället, så dess patterns ska vara tysta — annars räknas spåret två gånger i
/// både uppspelning och export. Ren funktion, så att regeln går att pröva.
pub fn render_clips_for(track: &PlaylistTrack) -> [Option<usize>; 32] {
    if track.is_frozen() {
        [None; 32]
    } else {
        track.clips
    }
}

/// Ska ett spårs ljud med i en rendering?
///
/// Ett fruset spår hörs som ljud i **sång-läget**. Pattern-läget renderar
/// kanalracket, och där hör spårets frysta ljud inte hemma — utan den regeln
/// hamnar det i en pattern-export som inte bad om det. Regeln ligger som en egen
/// funktion för att den gick att tappa bort i en omskrivning (den gjorde det,
/// under arbetet med tempokartan) och för att den nu prövas av sviten.
pub fn frozen_audio_in_render(track: &PlaylistTrack, pattern_mode: bool) -> bool {
    !(pattern_mode && track.frozen_pcm.is_some())
}

/// Regionen ett fruset spår spelar: hela filen från början. Längden räknas ur
/// bufferten i stället för ur takter, så den följer med automatiskt när
/// frysningen görs om eller tempot ändras.
pub fn frozen_region(left: &[f32], sample_rate: u32) -> crate::audio::StemRegionPlayback {
    crate::audio::StemRegionPlayback {
        start_time_secs: 0.0,
        length_secs: left.len() as f32 / sample_rate.max(1) as f32,
        sample_offset_sec: 0.0,
        gain: 1.0,
        fade_in_sec: 0.0,
        fade_out_sec: 0.0,
        muted: false,
        is_reverse: false,
        loop_length_secs: 0.0,
    }
}

#[derive(Clone, Debug)]
pub struct PlaylistTrack {
    pub name: String,
    #[allow(dead_code)]
    pub icon: &'static str,
    pub kind: TrackKind,
    pub color: Color32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    #[allow(dead_code)]
    pub is_rec_armed: bool,
    pub regions: Vec<AudioRegion>,   // Continuous multi-track audio stems and slices
    pub clips: [Option<usize>; 32],  // Pattern triggers
    #[allow(dead_code)]
    /// Vågformens flernivå-cache (Fas 8.3): `(nyckel, cache)`.
    ///
    /// Byggs vid behov när spårets regioner ritas, och görs om när ljudet byts —
    /// nyckeln är buffertens identitet, så en ny tagning eller en frysning ger en
    /// ny cache utan att någon behöver komma ihåg att säga till. Att bygga den på
    /// de tio ställen där `pcm_audio` sätts vore tio chanser att glömma ett.
    pub waveform_cache: Option<(u64, crate::audio::waveform::WaveformCache)>,
    #[allow(dead_code)]
    pub custom_clip_name: Option<String>,
    pub pcm_audio: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    /// Fruset spår (Tier 2): färdigrenderat ljud + fingeravtrycket av källan.
    pub frozen: Option<FrozenTrack>,
    /// Det frusna ljudet inläst, så att en omladdning (ClearAllStemTracks +
    /// resync, som vid projektinläsning) kan skicka det igen.
    pub frozen_pcm: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    pub eq: TrackEq,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    pub pitch_semitones: f32,
    /// Per-parameter automation curves for this track (Fas 5.4).
    pub automation: Vec<AutomationLane>,
    /// Last automation value sent to the engine per `AutomationParam` index
    /// (NaN = never sent). Prevents command spam while playing.
    pub automation_last: [f32; 4],
    /// Sub-mix bus this track feeds (`0..NUM_BUSES`) — Fas 5.2.
    pub bus: usize,
    /// Optional VCA control group (`0..NUM_VCAS`) — Fas 5.2.
    pub vca: Option<usize>,
}

/// Default sub-mix bus for a track kind (Fas 5.2): drums → Trummor, bass and
/// melodic synths → Synth, vocals → Vocal, FX/other → FX.
pub fn default_bus_for_kind(kind: TrackKind) -> usize {
    match kind {
        TrackKind::Drums => 1,
        TrackKind::Bassline | TrackKind::SynthLead => 2,
        TrackKind::VocalAudio => 0,
        TrackKind::Fx | TrackKind::CustomAudio => 3,
    }
}

impl PlaylistTrack {
    /// Ett spår kan frysas om det spelas upp av patterns — ljudspår ligger
    /// redan som färdigt ljud och har inget att tjäna på det.
    pub fn can_freeze(&self) -> bool {
        self.frozen.is_none()
            && matches!(
                self.kind,
                TrackKind::Drums | TrackKind::SynthLead | TrackKind::Bassline
            )
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }

    pub fn new(name: String, icon: &'static str, kind: TrackKind, color: Color32) -> Self {
        Self {
            name,
            icon,
            kind,
            color,
            volume: 0.85,
            pan: 0.0,
            muted: false,
            solo: false,
            is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            waveform_cache: None,
            custom_clip_name: None,
            pcm_audio: None,
            frozen: None,
            frozen_pcm: None,
            eq: TrackEq::default(),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: 0.0,
            automation: Vec::new(),
            automation_last: [f32::NAN; 4],
            bus: default_bus_for_kind(kind),
            vca: None,
        }
    }
}

/// Automatically classify stem and project tracks by name to assign distinct category color, icon, and track kind.
pub fn classify_track_style(name: &str) -> (TrackKind, &'static str, Color32) {
    let n = name.to_lowercase();
    if n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || (n.contains("voice") && (n.contains("rec") || n.contains("mic") || n.contains("sång"))) {
        (TrackKind::VocalAudio, "🎤", Color32::WHITE)
    } else if n.contains("lead") || n.contains("huvudsång") || n.contains("0 lead") || n.contains("leadvocal") || n.contains("lead vox") {
        (TrackKind::VocalAudio, "🎙", Color32::from_rgb(0, 225, 245)) // Vibrant Cyan
    } else if n.contains("backing") || n.contains("backvocal") || n.contains("back vocal") || n.contains("chorus") || n.contains("harmony") || n.contains("harmonies") || n.contains("bgv") || n.contains("1 backing") || n.contains("choir") || n.contains("kör") || n.contains("stämmor") || n.contains("vocal") || n.contains("vox") || n.contains("sång") {
        (TrackKind::VocalAudio, "🗣", Color32::from_rgb(225, 75, 235)) // Magenta / Purple
    } else if n.contains("percussion") || n.contains("6 percussion") || n.contains("perc") || n.contains("shaker") || n.contains("conga") || n.contains("bongo") || n.contains("tambourine") || n.contains("cowbell") {
        (TrackKind::Drums, "🪘", Color32::from_rgb(255, 105, 50)) // Vibrant Coral Rust
    } else if n.contains("drum") || n.contains("2 drums") || n.contains("trumm") || n.contains("kick") || n.contains("snare") || n.contains("hihat") || n.contains("hat") || n.contains("beat") || n.contains("slagverk") {
        (TrackKind::Drums, "🥁", Color32::from_rgb(255, 140, 25)) // Electric Orange
    } else if n.contains("bass") || n.contains("3 bass") || n.contains("bas") || n.contains("sub") || n.contains("808") {
        (TrackKind::Bassline, "🎸", Color32::from_rgb(45, 225, 105)) // Vivid Neon Green
    } else if n.contains("string") || n.contains("7 strings") || n.contains("stråk") || n.contains("violin") || n.contains("cello") || n.contains("orchestra") || n.contains("orchestral") {
        (TrackKind::SynthLead, "🎻", Color32::from_rgb(40, 220, 185)) // Emerald / Aquamarine Teal
    } else if n.contains("guitar") || n.contains("4 guitar") || n.contains("gitarr") || n.contains("acoustic") || n.contains("electric") || n.contains("strum") || n.contains("gtr") {
        (TrackKind::SynthLead, "🎸", Color32::from_rgb(255, 195, 30)) // Golden Amber
    } else if n.contains("key") || n.contains("5 keyboard") || n.contains("piano") || n.contains("flygel") || n.contains("organ") || n.contains("orgel") || n.contains("clav") || n.contains("rhodes") {
        (TrackKind::SynthLead, "🎹", Color32::from_rgb(65, 155, 255)) // Royal / Electric Blue
    } else if n.contains("synth") || n.contains("8 synth") || n.contains("synthesizer") || n.contains("pad") || n.contains("pluck") || n.contains("arp") || n.contains("brass") {
        (TrackKind::SynthLead, "🎛", Color32::from_rgb(175, 95, 255)) // Neon Violet
    } else if n.contains("fx") || n.contains("sfx") || n.contains("riser") || n.contains("sweep") || n.contains("impact") || n.contains("noise") || n.contains("effekt") || n.contains("atmos") || n.contains("other") || n.contains("9 other") {
        (TrackKind::Fx, "✨", Color32::from_rgb(255, 80, 150)) // Rose Pink
    } else {
        (TrackKind::CustomAudio, "🎵", Color32::from_rgb(160, 175, 200))
    }
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

#[derive(Clone)]
pub struct PreloadedTrackData {
    pub frozen: Option<SavedFrozenTrack>,
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub regions: Vec<AudioRegion>,
    pub clips: [Option<usize>; 32],
    pub eq: TrackEq,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    pub automation: Vec<AutomationLane>,
    pub bus: usize,
    pub vca: Option<usize>,
    pub stem_pcms: Vec<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
}

#[derive(Clone)]
pub struct LoadedProjectPayload {
    pub name: String,
    pub bpm: f32,
    /// Tempobyten (Fas 8.2). Saknas i äldre projekt — då gäller `bpm` som förut
    /// (fältet är `#[serde(default)]` i `SonixProjectData`, som är den som läses).
    pub tempo_points: Vec<crate::audio::tempo::TempoPoint>,
    pub swing: f32,
    pub master_volume: f32,
    pub master_pan: f32,
    pub tracks: Vec<PreloadedTrackData>,
    pub plugin_slots: Vec<Option<SavedPluginData>>,
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    pub vca_volume: [f32; crate::audio::synth::NUM_VCAS],
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    pub vca_solo: [bool; crate::audio::synth::NUM_VCAS],
    /// Mönster, kanalrack och stegvolymer (Fas 6.7).
    pub patterns: Vec<SavedPattern>,
    pub selected_pattern: usize,
    pub step_velocities: Option<[f32; 16]>,
    pub channels: Vec<SavedChannel>,
    pub file_path: String,
}

#[derive(Clone, Debug)]
pub struct TimelineUndoSnapshot {
    pub playlist_tracks: Vec<PlaylistTrack>,
    /// Mönstren med noterna och tagningen (Fas 6.4). Utan dem gick det inte att
    /// ångra en kvantisering: tagningen ligger i `patterns`, inte i spåren.
    pub patterns: Vec<Pattern>,
    /// Buss- och VCA-nivåer (Fas 6.2). De ligger utanför `playlist_tracks`, så
    /// utan dem kunde en bussändring inte ångras alls.
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    pub vca_faders: [f32; crate::audio::synth::NUM_VCAS],
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    pub vca_solos: [bool; crate::audio::synth::NUM_VCAS],
    pub selected_timeline_track: usize,
    pub selected_audio_region: Option<(usize, usize)>,
    pub song_time: f32,
    pub description: String,
}

/// Sammanfattning av mixerns ljudbild: varje värde som `sync_track_audio_state`
/// och `sync_group_state` skickar till motorn, hashat till ett tal.
///
/// Används av mixer-undon (Fas 6.2) för att upptäcka *att* mixern ändrats utan
/// att jämföra hela projektet, och av testet som bevisar att varje mixat fält
/// faktiskt ingår. Ändrar du vad motorn tar emot ska du ändra här också —
/// testet `mixer_digest_covers_every_mixed_field` räknar upp fälten.
fn mixer_digest(
    tracks: &[PlaylistTrack],
    bus_volume: &[f32],
    bus_muted: &[bool],
    bus_solo: &[bool],
    vca_faders: &[f32],
    vca_muted: &[bool],
    vca_solos: &[bool],
) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f32| {
        for b in v.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for t in tracks {
        mix(t.volume);
        mix(t.pan);
        mix(if t.muted { 1.0 } else { 0.0 });
        mix(if t.solo { 1.0 } else { 0.0 });
        mix(t.comp_threshold_db);
        mix(t.comp_ratio);
        mix(t.reverb_send);
        mix(t.delay_send);
        mix(t.pitch_semitones);
        mix(t.bus as f32);
        mix(t.vca.map(|v| v as f32).unwrap_or(-1.0));
        mix(t.eq.low_gain_db);
        mix(t.eq.low_freq);
        mix(t.eq.mid_gain_db);
        mix(t.eq.mid_freq);
        mix(t.eq.high_gain_db);
        mix(t.eq.high_freq);
    }
    for v in bus_volume.iter().chain(vca_faders.iter()).copied() {
        mix(v);
    }
    for b in bus_muted.iter().chain(bus_solo).chain(vca_muted).chain(vca_solos).copied() {
        mix(if b { 1.0 } else { 0.0 });
    }
    h
}


/// Standardnotnummer per trumkanal (kanal 0–5) — samma som trumspåret använder
/// när en kanal saknar eget sample.
const DRUM_CHANNEL_KEYS: [u8; 6] = [36, 38, 39, 42, 46, 49];

/// MIDI-noterna ett pattern ger upphov till för ett spår av en viss typ.
///
/// Ren funktion (Fas 6.3): mappningen från appens 16-stegsrutor till noter ska
/// kunna testas utan GUI eller ljudmotor. Mappningen speglar `trigger_song_step`
/// — trummor tar kanal 0–5, synthspåret tar piano-roll-rutnätet (med kanal 6 som
/// reserv när rutnätet är tomt) och basspåret kanal 7.
fn pattern_bar_notes(
    pattern: &Pattern,
    kind: TrackKind,
    channel: u8,
    bar_start_step: u32,
    velocities: &[u8; 16],
) -> Vec<crate::audio::smf::MidiNote> {
    use crate::audio::smf::{MidiNote, TICKS_PER_STEP_16TH};
    let mut out: Vec<MidiNote> = Vec::new();
    let place = |key: u8, step: usize, chan: u8, out: &mut Vec<MidiNote>| {
        out.push(MidiNote {
            start: (bar_start_step + step as u32) * TICKS_PER_STEP_16TH,
            length: TICKS_PER_STEP_16TH,
            channel: chan,
            key: key.min(127),
            velocity: velocities[step].clamp(1, 127),
        });
    };
    match kind {
        TrackKind::Drums => {
            for ch in 0..DRUM_CHANNEL_KEYS.len() {
                let Some(steps) = pattern.channel_steps.get(ch) else {
                    continue;
                };
                let notes = pattern.channel_notes.get(ch);
                for step in 0..16 {
                    if steps[step] {
                        let key = notes.map(|n| n[step]).unwrap_or(DRUM_CHANNEL_KEYS[ch]);
                        place(key, step, 9, &mut out);
                    }
                }
            }
        }
        TrackKind::SynthLead => {
            let grid_active = (0..24).any(|r| (0..16).any(|s| pattern.piano_roll_grid[r][s]));
            if grid_active {
                for row in 0..24 {
                    for step in 0..16 {
                        if pattern.piano_roll_grid[row][step] {
                            place(48 + row as u8, step, channel, &mut out);
                        }
                    }
                }
            } else if let (Some(steps), Some(notes)) = (
                pattern.channel_steps.get(6),
                pattern.channel_notes.get(6),
            ) {
                for step in 0..16 {
                    if steps[step] {
                        place(notes[step], step, channel, &mut out);
                    }
                }
            }
        }
        TrackKind::Bassline => {
            if let (Some(steps), Some(notes)) = (
                pattern.channel_steps.get(7),
                pattern.channel_notes.get(7),
            ) {
                for step in 0..16 {
                    if steps[step] {
                        place(notes[step], step, channel, &mut out);
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// MIDI-kanal för ett melodiskt spår. Kanal 9 är percussion, så den hoppas över.
fn midi_channel_for_track(track_index: usize) -> u8 {
    let c = if track_index >= 9 { track_index + 1 } else { track_index };
    (c % 16) as u8
}

/// Trumtangent → appens trumkanal 0–5 (nära General MIDI).
fn drum_channel_for_key(key: u8) -> Option<usize> {
    match key {
        35..=37 => Some(0),              // bastrumma
        38 | 40 => Some(1),              // virvel
        39 => Some(2),                   // handklapp
        41..=44 => Some(3),              // sluten hi-hat
        45..=48 => Some(4),              // öppen hi-hat
        49..=59 => Some(5),              // crash/cymbal (utom 54, 56, 58 = tamburin/klocka)
        _ => None,
    }
}

/// Vad en MIDI-import gjorde med filen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MidiImportReport {
    pub notes_placed: usize,
    /// Takter som fick ett eget mönster (och, vid fler än en, en kloss på spåret).
    pub bars_imported: usize,
    /// Noter vars takt ligger utanför arrangemanget (32 takter).
    pub dropped_beyond_arrangement: usize,
    /// Noter utanför appens rutnät (trumtangenter utan kanal, toner utanför 48–71).
    pub dropped_out_of_range: usize,
}

/// Steg per takt: samma rutnät som appens pattern — hämtat från tempokartan,
/// så att det bara finns en källa.
pub const STEPS_PER_BAR: usize = crate::audio::tempo::STEPS_PER_BAR;
/// Arrangemangets längd i takter — `clips` har 32 platser.
pub const ARRANGEMENT_BARS: usize = 32;

/// Noterna grupperade per takt, i taktordning.
///
/// Ren funktion (Fas 6.3), så att "inga noter tappas" går att pröva utan att
/// starta appen: en fil på fyra takter ska ge fyra grupper, inte en.
pub fn notes_by_bar(
    notes: &[crate::audio::smf::MidiNote],
    file_ppq: u16,
) -> Vec<(usize, Vec<crate::audio::smf::MidiNote>)> {
    let step_ticks = (file_ppq as u32 / 4).max(1);
    let mut groups: Vec<(usize, Vec<crate::audio::smf::MidiNote>)> = Vec::new();
    for n in notes {
        let bar = (n.start / step_ticks / STEPS_PER_BAR as u32) as usize;
        match groups.iter_mut().find(|(b, _)| *b == bar) {
            Some((_, list)) => list.push(n.clone()),
            None => groups.push((bar, vec![n.clone()])),
        }
    }
    groups.sort_by_key(|(b, _)| *b);
    groups
}

/// Lägger EN takts SMF-noter i ett pattern (steg 0–15).
///
/// Noterna routas dit appen själv spelar dem: percussion (kanal 9) till
/// trumkanalerna 0–5, 48–71 till piano-rollen, och allt under 48 till
/// baskanalen (7) — appens basspår spelar kanal 7 med råa notnummer, så en
/// basstämma från en annan DAW hör hemma där i stället för att slängas.
///
/// **Notlängden följer med** för toner: en not som håller över ett steg tänder
/// alla steg den klingar igenom. Trumslag tänder bara sitt eget steg — ett slag
/// är ett slag, och en lång not i en trumkanal är inte tre slag.
///
/// Ren funktion (Fas 6.3). Allt som ändå inte får plats räknas i stället för
/// att tyst försvinna — en import som tappar halva filen måste säga det.
fn apply_bar_to_pattern(
    notes: &[crate::audio::smf::MidiNote],
    file_ppq: u16,
    pattern: &mut Pattern,
) -> MidiImportReport {
    let step_ticks = (file_ppq as u32 / 4).max(1);
    let mut rep = MidiImportReport::default();
    while pattern.channel_steps.len() < 8 {
        pattern.channel_steps.push([false; 16]);
    }
    while pattern.channel_notes.len() < 8 {
        pattern.channel_notes.push([60; 16]);
    }

    for n in notes {
        let step = ((n.start / step_ticks) % STEPS_PER_BAR as u32) as usize;
        if n.channel == 9 {
            match drum_channel_for_key(n.key) {
                Some(ch) => {
                    pattern.channel_steps[ch][step] = true;
                    pattern.channel_notes[ch][step] = n.key;
                    rep.notes_placed += 1;
                }
                None => rep.dropped_out_of_range += 1,
            }
            continue;
        }
        // Toner: längden i steg, minst ett, klippt mot taktens slut.
        let span = (n.length.div_ceil(step_ticks).max(1) as usize).min(STEPS_PER_BAR - step);
        if (48..72).contains(&n.key) {
            for s in step..step + span {
                pattern.piano_roll_grid[(n.key - 48) as usize][s] = true;
            }
            rep.notes_placed += 1;
        } else if n.key < 48 {
            // Basstämma: appens basspår spelar kanal 7 med notnumret direkt.
            for s in step..step + span {
                pattern.channel_steps[7][s] = true;
                pattern.channel_notes[7][s] = n.key;
            }
            rep.notes_placed += 1;
        } else {
            rep.dropped_out_of_range += 1;
        }
    }
    rep
}

pub struct SonixApp {
    pub engine: AudioEngine,
    /// Monitor ring currently registered with the audio engine (re-sent after
    /// every reconfigure, which recreates the synth engine).
    pub monitor_ring_sent: Option<std::sync::Arc<std::sync::Mutex<Vec<f32>>>>,
    /// Senast skickade monitor-nivå (så kommandon inte spammas varje frame).
    pub monitor_level_sent: Option<f32>,
    pub stem_import_progress: std::sync::Arc<std::sync::Mutex<StemImportProgress>>,
    pub project_load_progress: std::sync::Arc<std::sync::Mutex<ProjectLoadProgress>>,
    // Transport & Clock
    pub is_playing: bool,
    /// Tempobyten (Fas 8.2). Tomt = projektet har ett enda tempo (`bpm`).
    ///
    /// Fältet är tomt så länge inga byten finns, och då är `bpm` sanningen. Finns
    /// en karta är `tempo_map()` sanningen — aldrig båda samtidigt.
    pub tempo_points: Vec<crate::audio::tempo::TempoPoint>,
    pub bpm: f32,
    pub swing: f32,
    pub current_step: usize,
    pub song_bar: usize,
    pub song_step_in_bar: usize,
    pub loop_start_bar: usize,
    pub loop_end_bar: usize,
    pub song_markers: Vec<SongMarker>,
    pub last_step_time: Instant,
    pub song_time: f32,
    pub pattern_mode: bool,
    pub view_mode: ViewMode,
    pub status_message: String,
    pub show_help_guide: bool,
    pub help_manual_active_tab: usize,
    pub help_search_query: String,
    // Browser Sidebar
    pub show_browser: bool,
    pub browser_search: String,
    // Master Section & EQ
    pub master_volume: f32,
    pub master_pan: f32,
    pub stereo_width: f32,
    // Synth Parameters
    pub current_preset: Preset,
    pub waveform: Waveform,
    pub adsr: AdsrParams,
    pub filter: FilterParams,
    pub filter_env_amount: f32,
    pub filter_env: AdsrParams,
    pub delay: DelayParams,
    pub reverb: ReverbParams,
    pub drive: f32,
    pub octave: i8,
    // Multi-Pattern & 8 Channel Rack
    pub patterns: Vec<Pattern>,
    pub selected_pattern: usize,
    pub channels: Vec<ChannelStrip>,
    pub selected_channel: usize,
    // Sample Library & Chopper
    pub sample_library: Vec<LibrarySampleItem>,
    pub active_sample_picker_channel: Option<usize>,
    pub active_chopper_channel: Option<usize>,
    pub selected_lib_category: usize,
    pub custom_import_name_input: String,
    pub custom_import_category: String,
    pub custom_import_icon: String,
    pub custom_import_note: u8,
    pub custom_import_freq: f32,
    pub custom_import_decay: f32,
    pub show_import_modal: bool,
    // Playlist Arranger
    pub playlist_tracks: Vec<PlaylistTrack>,
    pub arranger_tool: ArrangerTool,
    pub is_recording_timeline: bool,
    pub timeline_rec_start_bar: f32,
    pub show_add_track_modal: bool,
    pub add_track_state: AddTrackModalState,
    pub pending_add_track_import: Option<TrackTemplate>,
    pub metronome_enabled: bool,
    pub tap_tempo_times: Vec<std::time::Instant>,
    pub song_key_root: u8,
    pub song_key_scale: usize,
    pub time_signature: (u8, u8),
    // Piano Roll & Scale Snapping
    pub piano_roll_grid: [[bool; 16]; 24],
    pub selected_scale: usize,
    pub piano_roll_root_note: u8,
    pub piano_roll_snap_to_scale: bool,
    pub piano_roll_poly_mode: bool,
    pub chord_stamp: usize,
    pub step_velocities: [f32; 16],
    // GarageBand Virtual Session Drummer
    pub drummer_profile: usize,
    pub drummer_preset: usize,
    pub drummer_complexity: f32,
    pub drummer_loudness: f32,
    pub drummer_kick_snare_var: usize,
    pub drummer_hihat_var: usize,
    pub drummer_fills: f32,
    pub drummer_percussion_on: bool,
    pub drummer_cymbals_on: bool,
    pub drummer_toms_on: bool,
    pub drummer_tom_steps: [bool; 16],
    pub drummer_tom_high: bool,
    // Alchemy Transform Morph Pad
    pub alchemy_puck: [f32; 2],
    // Remix FX & Gross Beat
    pub remix_xy: [f32; 2],
    pub remix_tape_stop: bool,
    pub remix_stutter: usize,
    // Stompbox FX
    pub stompbox_drive: f32,
    pub stompbox_tone: f32,
    pub stompbox_cab: bool,
    // Modular Patcher, Stem Separator & Plugin Manager
    pub modular_graph: ModularGraph,
    pub patcher_enabled: bool,
    pub last_patcher_spec: Option<PatchSpec>,
    pub stem_project: StemProject,
    pub pending_stem_separation: bool,
    pub stem_separation_progress: std::sync::Arc<std::sync::Mutex<f32>>,
    pub stem_separation_result: std::sync::Arc<std::sync::Mutex<Option<Result<SeparationResult, String>>>>,
    pub stem_separation_active: bool,
    pub plugin_manager: PluginManager,
    /// Per-engine-stem-track plugin insert, persisted with the project so a
    /// reload can re-instantiate the plugin and restore its `clap.state` blob.
    pub plugin_slots: Vec<Option<PluginSlot>>,
    /// Main-thread handles to the live plugin instances (Fas 4.4b). Holding one
    /// keeps the shared `ClapCore` alive so it is never dropped on the audio
    /// thread; the GUI session uses it to talk to the *same* instance.
    pub plugin_handles: Vec<Option<crate::audio::plugin_host_live::PluginHandle>>,
    /// Open plugin-GUI windows, one slot per stem track.
    pub plugin_gui_sessions: Vec<Option<crate::audio::plugin_gui::GuiSession>>,
    /// Handles for replaced/removed plugins. Kept until shutdown so the final
    /// `ClapCore` drop happens on the main thread, not the audio thread.
    pub retired_plugin_handles: Vec<crate::audio::plugin_host_live::PluginHandle>,
    /// Out-of-process sandbox worker for the last plugin the user inspected in
    /// isolation (Fas 4.5a).
    #[cfg(feature = "plugin-host")]
    pub plugin_sandbox: Option<crate::audio::plugin_sandbox::SandboxHost>,
    /// Result of the last sandbox inspection, shown in the plugin manager.
    #[cfg(feature = "plugin-host")]
    pub sandbox_inspection: Option<crate::audio::plugin_sandbox::SandboxInspection>,
    /// Per-stem-track out-of-process sandbox hosts that stream audio over shared
    /// memory (Fas 4.5b). Index-aligned with the engine's stem tracks.
    #[cfg(feature = "plugin-host")]
    pub plugin_sandboxes: Vec<Option<crate::audio::plugin_sandbox::SandboxHost>>,
    // Vocal Studio, Comping, Harmonizer & AI Music Assistant
    pub vocal_studio: VocalStudioTrack,
    pub vocal_harmonizer: VocalHarmonizer,
    pub ai_assistant: AiMusicAssistant,
    // Interactive Piano
    pub active_mouse_note: Option<u8>,
    pub active_keys: HashSet<u8>,
    pub anim_phase: f32,
    pub scope_history: Vec<f32>,
    pub language: crate::i18n::Language,
    // Sub-Mixing, VCA Groups & PDC (Fas 5.2)
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    pub vca_faders: [f32; crate::audio::synth::NUM_VCAS],
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    pub vca_solos: [bool; crate::audio::synth::NUM_VCAS],
    // Linux Native Hardware Controller (MCU / OSC)
    pub show_controller_modal: bool,
    pub mcu_connected: bool,
    pub mcu_device_name: String,
    pub mcu_bank: usize,
    pub osc_enabled: bool,
    pub osc_rx_port: u16,
    pub osc_tx_port: u16,
    pub osc_rx_count: usize,
    pub midi_learn_active: bool,
    pub mcu_input: Option<McuInput>,
    pub osc_server: Option<OscServer>,
    // Real MIDI keyboard input (Fas 5.3)
    pub midi_input: Option<crate::audio::MidiKeyboardInput>,
    pub midi_auto_connect_attempted: bool,
    pub midi_keyboard_connected: bool,
    pub midi_device_name: String,
    pub midi_note_count: usize,
    pub midi_record_armed: bool,
    /// Räknare som ger humaniseringen ett nytt frö varje gång (Fas 6.4).
    pub take_seed_counter: u64,
    /// Pågående bakgrundsskanning av ljudbiblioteket (Fas 7.4), om cachen saknades.
    pub library_scan: Option<crate::audio::factory_samples::LibraryScan>,
    /// När skanningen startade, så statusraden kan visa hur länge den kört.
    pub library_scan_started: std::time::Instant,
    /// Rutnätet kvantiseringen drar noterna mot (Fas 6.4), som index i `TakeGrid::ALL`.
    pub take_grid_idx: usize,
    /// Dither vid 16-bitars export (Fas 6.5). På som standard.
    pub export_dither: bool,
    /// Noise shaping vid 16-bitars export (Fas 6.5). Av som standard.
    pub export_noise_shaping: bool,
    /// Kvantiseringens styrka 0–1 (Fas 6.4).
    pub take_strength: f32,
    /// Hur mycket humaniseringen ska lägga på, 0–1 (Fas 6.4).
    pub take_humanize: f32,
    pub midi_held_notes: std::collections::HashSet<u8>,
    pub control_tx: std::sync::mpsc::Sender<ControlEvent>,
    pub control_rx: std::sync::mpsc::Receiver<ControlEvent>,
    // Batch Export & Render Queue
    pub show_render_queue_modal: bool,
    pub render_format_idx: usize,
    pub render_scope_idx: usize,
    pub render_sample_rate_idx: usize,
    #[allow(dead_code)]
    pub render_bit_depth_idx: usize,
    pub is_rendering: bool,
    pub render_progress: f32,
    pub render_queue_status: String,
    // Export destination & clean metadata (only Sonix tags, never AI info)
    pub export_folder: String,
    pub export_base_name: String,
    pub export_metadata: crate::audio::ExportMeta,
    pub export_loudness_idx: usize,
    pub export_preset_idx: usize,
    // Suno AI Prompt Bar & Studio Timeline View
    pub suno_prompt_input: String,
    pub suno_model_version: String,
    pub suno_context_clip: Option<String>,
    pub suno_is_generating: bool,
    pub suno_generation_progress: f32,
    pub suno_zoom_level: f32,
    /// Background AI audio-API request: (result slot, prompt).
    pub ai_audio_pending: Option<(
        std::sync::Arc<std::sync::Mutex<Option<Result<Vec<u8>, String>>>>,
        String,
    )>,
    pub timeline_snap_mode: TimeSnapMode,
    pub timeline_auto_scroll: bool,
    pub selected_timeline_track: usize,
    // Automation curves (Fas 5.4)
    pub show_automation: bool,
    pub automation_param: AutomationParam,
    /// (track, lane, point) currently being dragged.
    pub automation_drag: Option<(usize, usize, usize)>,
    // Project Metadata & Suno Multi-Track Stems
    pub project_name: String,
    pub show_suno_import_modal: bool,
    pub detected_suno_zips: Vec<String>,
    pub custom_stem_path_input: String,
    pub custom_project_path_input: String,
    pub pending_file_dialog_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub is_file_dialog_active: std::sync::Arc<std::sync::atomic::AtomicBool>,
    // Selected Audio Region Inspector & Drag-to-Edit State
    pub selected_audio_region: Option<(usize, usize)>,
    pub region_drag_state: Option<RegionDragState>,
    pub copied_region: Option<AudioRegion>,
    pub sample_drag_item: Option<LibrarySampleItem>,
    pub undo_stack: Vec<TimelineUndoSnapshot>,
    pub redo_stack: Vec<TimelineUndoSnapshot>,
    // Focused Stem Detail & Sound Editor Modal
    pub focused_stem_track: Option<usize>,
    pub show_stem_focus_modal: bool,
    /// Tempokartan som fönster (Fas 8.2).
    pub show_tempo_modal: bool,
    /// Takten som högerklicket på linjalen gällde.
    ///
    /// Sparas för att menyn ritas om varje bildruta: inne i menyn står pekaren
    /// över menyn, så takten går inte att räkna ut där.
    pub tempo_menu_bar: Option<u32>,
    pub stem_focus_active_tab: usize,
    // Modals
    pub show_about_modal: bool,
    pub show_ai_settings_modal: bool,
    pub show_audio_settings_modal: bool,
    pub show_mic_settings_modal: bool,
    pub show_chord_generator_modal: bool,
    pub chord_generator_state: ChordGeneratorState,
    pub show_tuner_modal: bool,
    pub tuner_state: TunerState,
    pub show_dice_generator_modal: bool,
    pub dice_generator_state: DiceGeneratorState,
    pub show_fx_rack_modal: bool,
    pub fx_rack_state: FxRackState,
    pub show_song_structure_modal: bool,
    pub song_structure_state: SongStructureState,
    pub show_project_manager_modal: bool,
    pub project_file_path: Option<String>,
    /// Autosave (Fas 6.1): sekunder sedan senaste kontroll, och fingeravtryck
    /// för det läge som senast skyddades — så ett oförändrat projekt inte
    /// roterar bort sin egen historik.
    pub autosave_accum: f32,
    pub autosave_last_fp: Option<u64>,
    pub autosave_last_write: Option<u64>,
    /// Satt av `push_undo` när något strukturellt ändrats. Skrivningen sker
    /// först vid frame-gränsen — `push_undo` anropas nämligen *först* i varje
    /// muterande funktion, så en autosave som skrevs där skulle fånga läget
    /// före ändringen (och därmed hoppas över som "oförändrat").
    pub autosave_pending: bool,
    /// Mixer-undon (Fas 6.2): mixerns läge senast det var i vila, plus dess
    /// sammanfattning och om pekaren var nere förra frameen. Se `update`.
    pub mixer_settled_snapshot: Option<TimelineUndoSnapshot>,
    pub mixer_settled_digest: u64,
    pub mixer_pointer_was_down: bool,
    /// MIDI-fil import/export (Fas 6.3).
    pub show_midi_import_modal: bool,
    pub midi_import_path: String,
    /// Satt när en MCU/OSC-kontroll använts det här frameen — en sådan ändring
    /// har ingen pekare men ska ändå ge en ångringspunkt.
    pub mixer_control_event: bool,
    /// Autosaves som är nyare än den manuella filen, ifyllda vid start.
    pub recovery_candidates: Vec<RecoveryCandidate>,
    pub show_recovery_modal: bool,
    pub new_project_name_input: String,
    // Audio / MIDI Configuration
    pub audio_sample_rate_idx: usize,
    pub audio_buffer_size_idx: usize,
    // Automated Screenshot System
    pub screenshot_queue: Vec<(ScreenshotTarget, std::path::PathBuf)>,
    pub screenshot_state: ScreenshotState,
    pub screenshot_mode_active: bool,
}

#[derive(Clone, Debug)]
pub enum ScreenshotTarget {
    View(ViewMode),
    VocalTakeLanes,
    VocalCustomSounds,
    AudioSettingsModal,
    AiSettingsModal,
    ProjectManagerModal,
    RenderQueueModal,
    SunoImportModal,
    ControllerModal,
    StemFocusModal,
    MicSettingsModal,
    ChordGeneratorModal,
    TunerModal,
    DiceGeneratorModal,
    FxRackModal,
    SongStructureModal,
    AddTrackModal,
    HelpGuideModal,
    AboutModal,
}

#[derive(Clone, Debug)]
pub enum ScreenshotState {
    Idle,
    Preparing { target: ScreenshotTarget, dest: std::path::PathBuf, frames_left: usize },
    AwaitingCapture { dest: std::path::PathBuf },
}

/// Startar ljudbiblioteket (Fas 7.4).
///
/// Finns cachen läses den direkt — det tar en bråkdel av en sekund. Saknas den
/// startas en **bakgrundsskanning** i stället för att fönstret ska vänta: den
/// kalla skanningen läser ~10 GB och tog 125 sekunder.
fn init_sample_library_start() -> (
    Vec<LibrarySampleItem>,
    Option<crate::audio::factory_samples::LibraryScan>,
) {
    if let Some(cached) = crate::audio::factory_samples::read_library_cache_items() {
        eprintln!("🎵 Ljudbibliotek läst från cache: {} samplar", cached.len());
        (library_items_from_scanned(cached), None)
    } else {
        eprintln!("🎵 Ingen cache — skannar ljudbiblioteket i bakgrunden (fönstret visas direkt)");
        (
            Vec::new(),
            Some(crate::audio::factory_samples::LibraryScan::spawn()),
        )
    }
}

/// Slår ihop en färdig biblioteksskanning med det som redan låg i biblioteket
/// (Fas 7.4).
///
/// Skanningen tar minuter, och under tiden kan användaren ha importerat ett eget
/// sample. En rak ersättning skulle tysta tappa den importen — alltså behålls
/// poster som inte finns i skanningen.
fn merge_library(
    scanned: Vec<LibrarySampleItem>,
    existing: &[LibrarySampleItem],
) -> Vec<LibrarySampleItem> {
    let mut merged = scanned;
    for old in existing {
        if !merged.iter().any(|new| new.file_path == old.file_path) {
            merged.push(old.clone());
        }
    }
    merged
}

fn library_items_from_scanned(scanned: Vec<crate::audio::factory_samples::ScannedSampleItem>) -> Vec<LibrarySampleItem> {
    let mut items = Vec::new();

    for (idx, sc) in scanned.into_iter().enumerate() {
        let color = Color32::from_rgb(sc.color_rgb.0, sc.color_rgb.1, sc.color_rgb.2);
        items.push(LibrarySampleItem {
            id: idx,
            name: sc.name,
            category: sc.category,
            icon: sc.icon,
            default_note: sc.default_note,
            color,
            waveform: sc.waveform,
            file_path: Some(sc.file_path),
        });
    }

    items
}

/// Loads a WAV file once and returns Arc-wrapped stereo PCM for cheap cloning into audio commands.
fn load_sample_pcm_arcs(path: &str) -> Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)> {
    match crate::audio::load_wav_pcm(path) {
        Ok((l, r, sr)) => Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr)),
        Err(_) => None,
    }
}

/// Assigns a library sample to a Channel Rack channel strip. Loads the real
/// PCM into memory so step playback triggers the actual WAV instead of the
/// built-in synthesizer drum voices.
fn assign_library_sample_to_channel(ch: &mut ChannelStrip, item: &LibrarySampleItem) {
    ch.name = item.name.clone();
    ch.icon = item.icon.clone();
    ch.color = item.color;
    ch.notes = [item.default_note; 16];
    ch.waveform_preview = item.waveform.clone();
    ch.sample_start = 0.0;
    ch.sample_end = 1.0;
    ch.pitch_semitones = 0;
    ch.pitch_fine_cents = 0.0;
    ch.sample_base_note = item.default_note;
    match &item.file_path {
        Some(path) => {
            ch.sample_path = Some(path.clone());
            ch.pcm_audio = load_sample_pcm_arcs(path);
        }
        None => {
            ch.sample_path = None;
            ch.pcm_audio = None;
        }
    }
}

/// Picks a fitting real WAV from the library for each built-in drum channel
/// (Kick/Snare/Clap/Hat/Crash) when the user has downloaded sample packs, so the
/// Channel Rack plays actual recorded samples out of the box.
fn auto_assign_default_kit(channels: &mut [ChannelStrip], library: &[LibrarySampleItem]) {
    const KICK: &[&str] = &["bass drum", "bd0", "bd-", "kick"];
    const SNARE: &[&str] = &["snare"];
    const CLAP: &[&str] = &["handclap", "clap"];
    const HAT_CLOSED: &[&str] = &["closed hat", "chh", "hat closed"];
    const HAT_OPEN: &[&str] = &["open hat", "ohh", "hat open"];
    const CRASH: &[&str] = &["crash", "cymbal", "cy"];
    let rules: [(&[&str], usize); 6] = [
        (KICK, 0),
        (SNARE, 1),
        (CLAP, 2),
        (HAT_CLOSED, 3),
        (HAT_OPEN, 4),
        (CRASH, 5),
    ];

    let mut used_paths: Vec<String> = Vec::new();
    for (keywords, ch_idx) in rules {
        if ch_idx >= channels.len() {
            continue;
        }
        if channels[ch_idx].pcm_audio.is_some() {
            continue;
        }
        let found = library.iter().find(|item| {
            item.file_path.as_deref().is_some_and(|p| !used_paths.iter().any(|u| u == p))
                && keywords.iter().any(|k| item.name.to_lowercase().contains(k))
        });
        if let Some(item) = found {
            if let Some(p) = item.file_path.clone() {
                used_paths.push(p);
            }
            assign_library_sample_to_channel(&mut channels[ch_idx], item);
        }
    }
}

/// Builds the audio command that plays a channel's loaded WAV sample for one
/// sequencer step. Returns None when the channel has no PCM loaded, in which
/// case the caller falls back to the built-in synthesizer.
fn channel_sample_trigger_command(ch: &ChannelStrip, note: u8, velocity: f32) -> Option<AudioCommand> {
    ch.pcm_audio.as_ref().map(|(l, r, sr)| AudioCommand::TriggerSampleVoice {
        left: l.clone(),
        right: r.clone(),
        sample_rate: *sr,
        base_note: ch.sample_base_note,
        note,
        pitch_semitones: ch.pitch_semitones,
        pitch_cents: ch.pitch_fine_cents,
        velocity,
        volume: ch.volume,
        reverse: ch.is_reverse,
        start01: ch.sample_start,
        end01: ch.sample_end,
    })
}

fn ui_dbg(msg: &str) {
    let paths = crate::paths::paths();
    let on = std::env::var("SONIX_AUDIO_DEBUG").is_ok()
        || paths.debug_marker_file().exists()
        || paths.legacy_library_file("debug_on").exists();
    if !on {
        return;
    }
    let path = paths.log_file("audio_debug.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "[{}] UI: {}", std::process::id(), msg);
    }
}

impl SonixApp {
    pub fn tr(&self, key: &'static str) -> &'static str {
        crate::i18n::translate(self.language, key)
    }

    pub fn new(engine: AudioEngine) -> Self {
        let language = crate::i18n::load_language();
        crate::i18n::set_current(language);
        let current_preset = Preset::CleanPluck;
        let (waveform, adsr, filter) = current_preset.settings();
        let (control_tx, control_rx) = control_channel();

        // Helper for mini waveform generator
        let make_wave = |freq: f32, decay: f32| -> Vec<f32> {
            let mut w = Vec::with_capacity(50);
            for i in 0..50 {
                let t = i as f32 / 50.0;
                let val = (1.0 - t * decay) * (t * freq).sin().abs();
                w.push(val.clamp(0.05, 0.95));
            }
            w
        };

        let kick = ChannelStrip {
            name: "808 Kick".to_string(), icon: "💥".to_string(), color: Theme::FL_ORANGE,
            volume: 0.95, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.3, is_reverse: false,
            waveform_preview: make_wave(20.0, 0.8),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let snare = ChannelStrip {
            name: "909 Snare".to_string(), icon: "🥁".to_string(), color: Theme::FL_CYAN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [38; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, is_reverse: false,
            waveform_preview: make_wave(45.0, 0.9),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let clap = ChannelStrip {
            name: "Electro Clap".to_string(), icon: "👏".to_string(), color: Theme::FL_YELLOW,
            volume: 0.80, pan: -0.1, muted: false, solo: false, steps: [false; 16], notes: [39; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, is_reverse: false,
            waveform_preview: make_wave(35.0, 0.85),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let hat = ChannelStrip {
            name: "Crisp Hat".to_string(), icon: "⚡".to_string(), color: Theme::FL_PURPLE,
            volume: 0.75, pan: 0.15, muted: false, solo: false, steps: [false; 16], notes: [42; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.2, is_reverse: false,
            waveform_preview: make_wave(70.0, 0.95),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let open_hat = ChannelStrip {
            name: "Open Hat".to_string(), icon: "🌊".to_string(), color: Theme::FL_CYAN,
            volume: 0.70, pan: -0.2, muted: false, solo: false, steps: [false; 16], notes: [46; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.6, is_reverse: false,
            waveform_preview: make_wave(50.0, 0.5),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let crash = ChannelStrip {
            name: "Cyber Crash".to_string(), icon: "✨".to_string(), color: Color32::from_rgb(255, 180, 50),
            volume: 0.75, pan: 0.25, muted: false, solo: false, steps: [false; 16], notes: [49; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.7, is_reverse: false,
            waveform_preview: make_wave(30.0, 0.4),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let synth_lead = ChannelStrip {
            name: "303 Lead".to_string(), icon: "🎹".to_string(), color: Theme::FL_GREEN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [60; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, is_reverse: false,
            waveform_preview: make_wave(60.0, 0.3),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let sub_bass = ChannelStrip {
            name: "Sub Bass".to_string(), icon: "🎸".to_string(), color: Color32::from_rgb(255, 80, 140),
            volume: 0.90, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, is_reverse: false,
            waveform_preview: make_wave(25.0, 0.6),
            sample_path: None, pcm_audio: None, sample_base_note: 60,
        };

        let mut channels = vec![kick, snare, clap, hat, open_hat, crash, synth_lead, sub_bass];

        // Sample Library Initial Items (Categorized with Rich Factory & Downloaded Sample Packs).
        // Cachen läses direkt; saknas den skannas biblioteket i bakgrunden (Fas 7.4).
        let (sample_library, library_scan_job) = init_sample_library_start();

        // Give the six built-in drum channels real recorded samples when sample
        // packs are installed, so the Channel Rack plays WAV files by default.
        if !sample_library.is_empty() {
            auto_assign_default_kit(&mut channels, &sample_library);
        }



        let pat1 = Pattern {
            name: "Pat 1: Beats & Groove".to_string(),
            color: Theme::FL_ORANGE,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat2 = Pattern {
            name: "Pat 2: Lead & Synthesizer".to_string(),
            color: Theme::FL_CYAN,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat3 = Pattern {
            name: "Pat 3: Synth Arp".to_string(),
            color: Theme::FL_GREEN,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat4 = Pattern {
            name: "Pat 4: Deep Bass".to_string(),
            color: Color32::from_rgb(255, 80, 140),
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        };

        let patterns = vec![pat1, pat2, pat3, pat4];

        // 1. Drums & Beat Track
        let mut t_drums = PlaylistTrack::new(crate::i18n::t("🥁 Trummor & Beat").to_string(), "🥁", TrackKind::Drums, Color32::from_rgb(255, 140, 25));
        t_drums.volume = 0.90;

        // 2. Synth & Melody Track
        let mut t_synth = PlaylistTrack::new(crate::i18n::t("🎹 Synt & Ackord").to_string(), "🎹", TrackKind::SynthLead, Color32::from_rgb(65, 155, 255));
        t_synth.volume = 0.85;

        // 3. Bassline Track
        let mut t_bass = PlaylistTrack::new(crate::i18n::t("🎸 Baslinje").to_string(), "🎸", TrackKind::Bassline, Color32::from_rgb(45, 225, 105));
        t_bass.volume = 0.90;

        // 4. FX & Transitions
        let mut t_fx = PlaylistTrack::new(crate::i18n::t("✨ FX & Ljudeffekter").to_string(), "✨", TrackKind::Fx, Color32::from_rgb(255, 80, 150));
        t_fx.volume = 0.80;

        // 5. Mic Vocal Track (Armed and Ready for Recording - placed at the END)
        let mut t_mic = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
        t_mic.volume = 1.0;
        t_mic.is_rec_armed = true;

        let playlist_tracks = vec![t_drums, t_synth, t_bass, t_fx, t_mic];
        let selected_pattern = 0;
        let initial_channels = channels;
        let initial_grid = [[false; 16]; 24];

        // Reflect the real stream configuration in the settings UI.
        let initial_rate_idx = match engine.sample_rate {
            44100 => 0,
            96000 => 2,
            _ => 1,
        };
        let initial_buffer_idx = match engine.buffer_frames {
            Some(128) => 0,
            Some(512) => 2,
            Some(1024) => 3,
            _ => 1,
        };

        // Kraschåterställning (Fas 6.1): finns det en autosave som är nyare än
        // sin manuella projektfil är det senaste arbetsläget bara där.
        let recovery_candidates = collect_recovery_candidates();
        let show_recovery_modal = !recovery_candidates.is_empty();

        let mut app = Self {
            engine,
            monitor_ring_sent: None,
            monitor_level_sent: None,
            stem_import_progress: std::sync::Arc::new(std::sync::Mutex::new(StemImportProgress::default())),
            project_load_progress: std::sync::Arc::new(std::sync::Mutex::new(ProjectLoadProgress::default())),
            is_playing: false,
            bpm: 120.0,
            tempo_points: Vec::new(),
            swing: 0.0,
            current_step: 0,
            song_bar: 0,
            song_step_in_bar: 0,
            loop_start_bar: 0,
            loop_end_bar: 16,
            song_markers: Vec::new(),
            last_step_time: Instant::now(),
            song_time: 0.0,
            pattern_mode: false,
            view_mode: ViewMode::PlaylistArranger,
            status_message: crate::i18n::t("Välkommen till Sonix Studio! Skapa ett beat eller importera dina stämmor (Stems).").to_string(),
            show_help_guide: false,
            help_manual_active_tab: 0,
            help_search_query: String::new(),
            show_browser: true,
            browser_search: String::new(),
            master_volume: 0.85,
            master_pan: 0.0,
            stereo_width: 1.0,
            current_preset,
            waveform,
            adsr,
            filter,
            filter_env_amount: 0.0,
            filter_env: AdsrParams { attack: 0.005, decay: 0.25, sustain: 0.0, release: 0.2 },
            delay: DelayParams::default(),
            reverb: ReverbParams::default(),
            drive: 1.0,
            octave: 4,
            patterns,
            selected_pattern,
            channels: initial_channels,
            selected_channel: 0,
            sample_library,
            active_sample_picker_channel: None,
            active_chopper_channel: None,
            selected_lib_category: 0,
            custom_import_name_input: crate::i18n::t("Mitt Nya Ljud").to_string(),
            custom_import_category: "Egna Importerade".to_string(),
            custom_import_icon: "📂".to_string(),
            custom_import_note: 48,
            custom_import_freq: 35.0,
            custom_import_decay: 0.7,
            show_import_modal: false,
            playlist_tracks,
            arranger_tool: ArrangerTool::Paint,
            is_recording_timeline: false,
            timeline_rec_start_bar: 0.0,
            show_add_track_modal: false,
            add_track_state: AddTrackModalState::default(),
            pending_add_track_import: None,
            metronome_enabled: false,
            tap_tempo_times: Vec::new(),
            song_key_root: 3, // Eb
            song_key_scale: 0, // Dur
            time_signature: (4, 4),
            piano_roll_grid: initial_grid,
            selected_scale: 0,
            piano_roll_root_note: 3, // Eb
            piano_roll_snap_to_scale: false,
            piano_roll_poly_mode: false,
            chord_stamp: 0,
            step_velocities: [0.85; 16],
            drummer_profile: 0,
            drummer_preset: 0,
            drummer_complexity: 0.5,
            drummer_loudness: 0.85,
            drummer_kick_snare_var: 1,
            drummer_hihat_var: 1,
            drummer_fills: 0.4,
            drummer_percussion_on: true,
            drummer_cymbals_on: true,
            drummer_toms_on: true,
            drummer_tom_steps: [false; 16],
            drummer_tom_high: true,
            alchemy_puck: [0.0, 0.0],
            remix_xy: [0.5, 0.5],
            remix_tape_stop: false,
            remix_stutter: 0,
            stompbox_drive: 2.5,
            stompbox_tone: 5000.0,
            stompbox_cab: true,
            modular_graph: ModularGraph::default(),
            patcher_enabled: false,
            last_patcher_spec: None,
            stem_project: StemProject::default(),
            pending_stem_separation: false,
            stem_separation_progress: std::sync::Arc::new(std::sync::Mutex::new(0.0)),
            stem_separation_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            stem_separation_active: false,
            plugin_manager: PluginManager::default(),
            plugin_slots: Vec::new(),
            plugin_handles: Vec::new(),
            plugin_gui_sessions: Vec::new(),
            retired_plugin_handles: Vec::new(),
            #[cfg(feature = "plugin-host")]
            plugin_sandbox: None,
            #[cfg(feature = "plugin-host")]
            sandbox_inspection: None,
            #[cfg(feature = "plugin-host")]
            plugin_sandboxes: Vec::new(),
            vocal_studio: VocalStudioTrack::default(),
            vocal_harmonizer: VocalHarmonizer::default(),
            ai_assistant: AiMusicAssistant::default(),
            active_mouse_note: None,
            active_keys: HashSet::new(),
            anim_phase: 0.0,
            scope_history: Vec::new(),
            language,
            // Sub-Mixing, VCA Groups & PDC (Fas 5.2)
            bus_volume: [1.0; crate::audio::synth::NUM_BUSES],
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_faders: [1.0; crate::audio::synth::NUM_VCAS],
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solos: [false; crate::audio::synth::NUM_VCAS],
            // Linux Native Hardware Controller (MCU / OSC)
            show_controller_modal: false,
            mcu_connected: false,
            mcu_device_name: crate::i18n::t("Ingen enhet ansluten").to_string(),
            mcu_bank: 0,
            osc_enabled: false,
            osc_rx_port: 8000,
            osc_tx_port: 9000,
            osc_rx_count: 0,
            midi_learn_active: false,
            mcu_input: None,
            osc_server: None,
            midi_input: None,
            midi_auto_connect_attempted: false,
            midi_keyboard_connected: false,
            midi_device_name: crate::i18n::t("Inte ansluten").to_string(),
            midi_note_count: 0,
            midi_record_armed: false,
            take_seed_counter: 0,
            library_scan: library_scan_job,
            library_scan_started: std::time::Instant::now(),
            take_grid_idx: crate::midi_take::TakeGrid::Sixteenth.index(),
            export_dither: true,
            export_noise_shaping: false,
            take_strength: 1.0,
            take_humanize: 0.5,
            midi_held_notes: std::collections::HashSet::new(),
            control_tx,
            control_rx,
            // Batch Export & Render Queue
            show_render_queue_modal: false,
            render_format_idx: 0,
            render_scope_idx: 0,
            render_sample_rate_idx: 0,
            render_bit_depth_idx: 0,
            is_rendering: false,
            render_progress: 0.0,
            render_queue_status: crate::i18n::t("Klar för rendering").to_string(),
            export_folder: {
                crate::paths::paths().exports_dir().to_string_lossy().to_string()
            },
            export_base_name: crate::i18n::t("Min_Låt").to_string(),
            export_metadata: crate::audio::ExportMeta::default(),
            export_loudness_idx: 0,
            export_preset_idx: 0,
            // Suno AI Prompt Bar & Studio Timeline View
            suno_prompt_input: crate::i18n::t("Generera ett 8-takters synthwave-trumkomp och vokalmelodi i A-moll").to_string(),
            suno_model_version: "Sonix AI v5.5 (Music & Stems)".to_string(),
            suno_context_clip: None,
            suno_is_generating: false,
            suno_generation_progress: 0.0,
            suno_zoom_level: 1.0,
            ai_audio_pending: None,
            timeline_snap_mode: TimeSnapMode::Snap16th,
            timeline_auto_scroll: true,
            selected_timeline_track: 0,
            show_automation: false,
            automation_param: AutomationParam::Volume,
            automation_drag: None,
            // Project Metadata & Suno Multi-Track Stems
            project_name: crate::i18n::t("Namnlöst Projekt").to_string(),
            show_suno_import_modal: false,
            detected_suno_zips: Vec::new(),
            custom_stem_path_input: crate::paths::paths().music_dir().to_string_lossy().to_string(),
            custom_project_path_input: crate::paths::paths()
                .projects_dir()
                .to_string_lossy()
                .to_string(),
            pending_file_dialog_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            is_file_dialog_active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            // Selected Audio Region Inspector & Drag-to-Edit State
            selected_audio_region: None,
            region_drag_state: None,
            copied_region: None,
            sample_drag_item: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            // Focused Stem Detail & Sound Editor Modal
            focused_stem_track: None,
            show_stem_focus_modal: false,
            show_tempo_modal: false,
            tempo_menu_bar: None,
            stem_focus_active_tab: 0,
            // Modals
            show_about_modal: false,
            show_ai_settings_modal: false,
            show_audio_settings_modal: false,
            show_mic_settings_modal: false,
            show_chord_generator_modal: false,
            chord_generator_state: ChordGeneratorState::default(),
            show_tuner_modal: false,
            tuner_state: TunerState::default(),
            show_dice_generator_modal: false,
            dice_generator_state: DiceGeneratorState::default(),
            show_fx_rack_modal: false,
            fx_rack_state: FxRackState::default(),
            show_song_structure_modal: false,
            song_structure_state: SongStructureState::default(),
            show_project_manager_modal: false,
            project_file_path: None,
            autosave_accum: 0.0,
            autosave_last_fp: None,
            autosave_last_write: None,
            autosave_pending: false,
            mixer_settled_snapshot: None,
            mixer_settled_digest: 0,
            mixer_pointer_was_down: false,
            show_midi_import_modal: false,
            midi_import_path: crate::paths::paths().exports_dir().to_string_lossy().to_string(),
            mixer_control_event: false,
            recovery_candidates,
            show_recovery_modal,
            new_project_name_input: crate::i18n::t("Mitt Beat").to_string(),
            // Audio / MIDI Configuration
            audio_sample_rate_idx: initial_rate_idx,
            audio_buffer_size_idx: initial_buffer_idx,
            // Automated Screenshot System
            screenshot_queue: Vec::new(),
            screenshot_state: ScreenshotState::Idle,
            screenshot_mode_active: false,
        };
        app.sync_group_state();
        app.sync_all_stems_to_engine();

        // Utgångsläget är inte "osparat arbete". Utan detta skulle en helt orörd
        // start skriva en autosave, och nästa start erbjuda att återställa ett
        // tomt projekt — en varning som inte betyder något.
        if let Ok(json) = serde_json::to_string_pretty(&app.project_data(&app.project_name)) {
            app.autosave_last_fp = Some(crate::autosave::fingerprint(json.as_bytes()));
        }

        // Mixerns viloläge (Fas 6.2) måste finnas redan från start, annars har
        // den första reglageändringen inget "läge före" att lägga på historiken.
        app.mixer_settled_digest = app.mixer_state_digest();
        app.mixer_settled_snapshot = Some(app.current_snapshot("mixer"));
        app
    }

    pub fn enable_screenshot_mode(&mut self, out_dir: &std::path::Path) {
        self.screenshot_mode_active = true;
        self.screenshot_state = ScreenshotState::Idle;
        self.populate_demo_data();
        let _ = std::fs::create_dir_all(out_dir);


        let targets = vec![
            (ScreenshotTarget::View(ViewMode::PlaylistArranger), out_dir.join("01_tidslinje_arranger.png")),
            (ScreenshotTarget::View(ViewMode::ChannelRack), out_dir.join("02_channel_rack.png")),
            (ScreenshotTarget::View(ViewMode::PianoRoll), out_dir.join("03_piano_roll.png")),
            (ScreenshotTarget::VocalTakeLanes, out_dir.join("04_vocal_studio_leads.png")),
            (ScreenshotTarget::VocalCustomSounds, out_dir.join("05_vocal_studio_sampler.png")),
            (ScreenshotTarget::View(ViewMode::EffectsMixer), out_dir.join("06_effects_mixer.png")),
            (ScreenshotTarget::View(ViewMode::AiMusicAssistant), out_dir.join("07_ai_music_assistant.png")),
            (ScreenshotTarget::View(ViewMode::AlchemySynth), out_dir.join("08_alchemy_synth.png")),
            (ScreenshotTarget::View(ViewMode::SessionDrummer), out_dir.join("09_session_drummer.png")),
            (ScreenshotTarget::View(ViewMode::ModularPatcher), out_dir.join("10_modular_patcher.png")),
            (ScreenshotTarget::View(ViewMode::StemSeparator), out_dir.join("11_stem_separator.png")),
            (ScreenshotTarget::View(ViewMode::PluginManager), out_dir.join("12_plugin_manager.png")),
            (ScreenshotTarget::View(ViewMode::RemixFx), out_dir.join("13_remix_fx.png")),
            (ScreenshotTarget::AudioSettingsModal, out_dir.join("14_dialog_ljudinstallningar.png")),
            (ScreenshotTarget::AiSettingsModal, out_dir.join("15_dialog_ai_installningar.png")),
            (ScreenshotTarget::ProjectManagerModal, out_dir.join("16_dialog_projekthanterare.png")),
            (ScreenshotTarget::RenderQueueModal, out_dir.join("17_dialog_render_queue.png")),
            (ScreenshotTarget::SunoImportModal, out_dir.join("18_dialog_suno_import.png")),
            (ScreenshotTarget::ControllerModal, out_dir.join("19_dialog_midi_controller.png")),
            (ScreenshotTarget::StemFocusModal, out_dir.join("20_dialog_stem_focus_editor.png")),
            (ScreenshotTarget::MicSettingsModal, out_dir.join("21_dialog_mikrofon_installningar.png")),
            (ScreenshotTarget::ChordGeneratorModal, out_dir.join("22_dialog_ackord_matris.png")),
            (ScreenshotTarget::TunerModal, out_dir.join("23_dialog_strobe_tuner.png")),
            (ScreenshotTarget::DiceGeneratorModal, out_dir.join("24_dialog_tarning_generator.png")),
            (ScreenshotTarget::FxRackModal, out_dir.join("25_dialog_modular_fx_rack.png")),
            (ScreenshotTarget::SongStructureModal, out_dir.join("26_dialog_latstruktur_arrangemang.png")),
            (ScreenshotTarget::AddTrackModal, out_dir.join("27_dialog_lagg_till_spar.png")),
            (ScreenshotTarget::HelpGuideModal, out_dir.join("28_dialog_hjalpguide_manual.png")),
            (ScreenshotTarget::AboutModal, out_dir.join("29_dialog_om_sonix.png")),
        ];

        self.screenshot_queue = targets;
    }

    pub fn apply_screenshot_target(&mut self, target: ScreenshotTarget) {
        self.show_help_guide = false;
        self.show_project_manager_modal = false;
        self.show_suno_import_modal = false;
        self.show_render_queue_modal = false;
        self.show_stem_focus_modal = false;
        self.show_controller_modal = false;
        self.show_about_modal = false;
        self.show_ai_settings_modal = false;
        self.show_audio_settings_modal = false;
        self.show_import_modal = false;
        self.show_mic_settings_modal = false;
        self.show_chord_generator_modal = false;
        self.show_tuner_modal = false;
        self.show_dice_generator_modal = false;
        self.show_fx_rack_modal = false;
        self.show_song_structure_modal = false;
        self.show_add_track_modal = false;

        match target {
            ScreenshotTarget::View(vm) => {
                self.view_mode = vm;
            }
            ScreenshotTarget::VocalTakeLanes => {
                self.view_mode = ViewMode::VocalStudio;
                self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::LeadVocals;
            }
            ScreenshotTarget::VocalCustomSounds => {
                self.view_mode = ViewMode::VocalStudio;
                self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::CustomSounds;
            }
            ScreenshotTarget::AudioSettingsModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_audio_settings_modal = true;
            }
            ScreenshotTarget::AiSettingsModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_ai_settings_modal = true;
            }
            ScreenshotTarget::ProjectManagerModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_project_manager_modal = true;
            }
            ScreenshotTarget::RenderQueueModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.open_export_modal();
            }
            ScreenshotTarget::SunoImportModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_suno_import_modal = true;
            }
            ScreenshotTarget::ControllerModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_controller_modal = true;
            }
            ScreenshotTarget::StemFocusModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.focused_stem_track = Some(0);
                self.show_stem_focus_modal = true;
            }
            ScreenshotTarget::MicSettingsModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_mic_settings_modal = true;
            }
            ScreenshotTarget::ChordGeneratorModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_chord_generator_modal = true;
            }
            ScreenshotTarget::TunerModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_tuner_modal = true;
            }
            ScreenshotTarget::DiceGeneratorModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_dice_generator_modal = true;
            }
            ScreenshotTarget::FxRackModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_fx_rack_modal = true;
            }
            ScreenshotTarget::SongStructureModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_song_structure_modal = true;
            }
            ScreenshotTarget::AddTrackModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_add_track_modal = true;
            }
            ScreenshotTarget::HelpGuideModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_help_guide = true;
            }
            ScreenshotTarget::AboutModal => {
                self.view_mode = ViewMode::PlaylistArranger;
                self.show_about_modal = true;
            }
        }
    }

    pub fn populate_demo_data(&mut self) {
        self.project_name = "Cyberpunk Odyssey (Suno AI + Sonix Lead)".to_string();
        self.bpm = 124.0;
        self.swing = 0.15;
        self.song_bar = 4;
        self.song_step_in_bar = 2;

        // Populate Channel rack steps
        if self.channels.len() >= 6 {
            self.channels[0].steps = [true, false, false, false, true, false, false, false, true, false, false, false, true, false, false, false];
            self.channels[1].steps = [false, false, false, false, true, false, false, false, false, false, false, false, true, false, false, false];
            self.channels[2].steps = [true, false, true, false, true, false, true, false, true, false, true, false, true, false, true, false];
            self.channels[3].steps = [false, false, true, false, false, false, true, false, false, false, true, false, false, false, true, false];
            self.channels[4].steps = [true, false, false, true, false, true, false, false, true, false, true, false, false, true, false, false];
            self.channels[5].steps = [true, true, false, false, true, true, false, false, true, true, false, false, true, true, false, false];
        }

        // Populate Piano Roll Grid (24 notes x 16 steps)
        for r in 0..24 {
            for c in 0..16 {
                self.piano_roll_grid[r][c] = false;
            }
        }
        let notes_to_set = [
            (12, 0), (12, 1), (15, 2), (19, 3), (12, 4), (17, 6), (15, 7),
            (10, 8), (14, 10), (17, 11), (10, 12), (12, 14), (15, 15),
            (5, 0), (5, 4), (5, 8), (5, 12), (0, 0), (0, 4), (0, 8), (0, 12)
        ];
        for (r, c) in notes_to_set {
            if r < 24 && c < 16 {
                self.piano_roll_grid[r][c] = true;
            }
        }

        // Populate Audio Regions in Playlist Tracks
        let sample_waveform: Vec<f32> = (0..120).map(|i| {
            let t = i as f32 / 120.0;
            ((t * 24.0).sin() * 0.5 + (t * 50.0).sin() * 0.3 + 0.1).abs().clamp(0.05, 0.95)
        }).collect();

        if self.playlist_tracks.is_empty() {
            self.playlist_tracks = vec![
                PlaylistTrack::new("🎤 Lead Vocals (Suno AI)".to_string(), "🎤", TrackKind::VocalAudio, Color32::from_rgb(255, 100, 120)),
                PlaylistTrack::new("🎧 Backing Choir".to_string(), "🎧", TrackKind::VocalAudio, Color32::from_rgb(220, 140, 255)),
                PlaylistTrack::new("🥁 808 & Drums".to_string(), "🥁", TrackKind::Drums, Theme::FL_ORANGE),
                PlaylistTrack::new("🎹 Nexus Synth Lead".to_string(), "🎹", TrackKind::SynthLead, Theme::FL_CYAN),
                PlaylistTrack::new("🎸 Moog Sub Bass".to_string(), "🎸", TrackKind::Bassline, Theme::FL_GREEN),
                PlaylistTrack::new("🌌 Space FX & Risers".to_string(), "🌌", TrackKind::Fx, Theme::FL_YELLOW),
            ];
        }

        if let Some(t0) = self.playlist_tracks.get_mut(0) {
            t0.name = "🎤 Lead Vocals (Suno AI)".to_string();
            t0.regions = vec![
                AudioRegion {
                    id: 101,
                    name: "Chorus Take 1 (Main)".to_string(),
                    start_bar: 1.0,
                    length_bars: 7.0,
                    sample_offset_sec: 0.0,
                    source_path: Some("stems/lead_vocals.wav".to_string()),
                    waveform_peaks: sample_waveform.clone(),
                    volume: 0.95,
                    fade_in_bars: 0.1,
                    fade_out_bars: 0.2,
                    muted: false,
                    is_reverse: false,
                    color: Color32::from_rgb(255, 100, 120),
                    loop_length_bars: 0.0,
                },
                AudioRegion {
                    id: 102,
                    name: "Verse Hook (Harmonized)".to_string(),
                    start_bar: 9.0,
                    length_bars: 6.0,
                    sample_offset_sec: 0.0,
                    source_path: Some("stems/lead_vocals_hook.wav".to_string()),
                    waveform_peaks: sample_waveform.clone(),
                    volume: 0.90,
                    fade_in_bars: 0.1,
                    fade_out_bars: 0.2,
                    muted: false,
                    is_reverse: false,
                    color: Color32::from_rgb(255, 120, 140),
                    loop_length_bars: 0.0,
                },
            ];
        }

        if let Some(t1) = self.playlist_tracks.get_mut(1) {
            t1.name = "🎧 Backing Choir".to_string();
            t1.regions = vec![
                AudioRegion {
                    id: 201,
                    name: "Stereo Choir Pad (4-Part)".to_string(),
                    start_bar: 3.0,
                    length_bars: 5.0,
                    sample_offset_sec: 0.0,
                    source_path: Some("stems/choir.wav".to_string()),
                    waveform_peaks: sample_waveform.clone(),
                    volume: 0.80,
                    fade_in_bars: 0.5,
                    fade_out_bars: 0.5,
                    muted: false,
                    is_reverse: false,
                    color: Color32::from_rgb(200, 130, 255),
                    loop_length_bars: 0.0,
                },
            ];
        }

        if let Some(t2) = self.playlist_tracks.get_mut(2) {
            t2.name = "🥁 808 Drum Beat".to_string();
            t2.clips[0] = Some(0);
            t2.clips[1] = Some(0);
            t2.clips[2] = Some(0);
            t2.clips[3] = Some(0);
        }

        if let Some(t3) = self.playlist_tracks.get_mut(3) {
            t3.name = "🎹 Nexus Synth Lead".to_string();
            t3.clips[0] = Some(1);
            t3.clips[1] = Some(1);
        }

        if let Some(t4) = self.playlist_tracks.get_mut(4) {
            t4.name = "🎸 Moog Sub Bass".to_string();
            t4.clips[0] = Some(2);
            t4.clips[1] = Some(2);
        }

        // Modular graph default connections
        self.modular_graph.load_default_preset();

        // AI Assistant prompt
        self.ai_assistant.prompt_input = crate::i18n::t("Cyberpunk 80s synthwave lead med mörk rezonans i A-moll").to_string();
        self.ai_assistant.last_status = crate::i18n::t("✨ AI genererade 3 variationer av Synth Lead & Bassline").to_string();
    }


    pub fn new_empty_project(&mut self) {
        self.stop_playback();
        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        for t in &mut self.playlist_tracks {
            t.regions.clear();
            t.clips = [None; 32];
        }
        for p in &mut self.patterns {
            p.channel_steps.fill([false; 16]);
            p.piano_roll_grid = [[false; 16]; 24];
        }
        for ch in &mut self.channels {
            ch.steps = [false; 16];
        }
        self.piano_roll_grid = [[false; 16]; 24];
        self.selected_audio_region = None;
        self.project_name = crate::i18n::t("Namnlöst Projekt").to_string();
        self.project_file_path = None;
        self.ensure_mic_track_exists();
        self.status_message = crate::i18n::t("✨ Skapade ett nytt tomt projekt!").to_string();
    }

    pub fn load_demo_project(&mut self) {
        let progress = self.project_load_progress.clone();
        {
            if let Ok(mut p) = progress.lock() {
                p.is_loading = true;
                p.project_name = "Sonix Synthwave Demo".to_string();
                p.stage = crate::i18n::t("Läser in mönster och instrumentspår...").to_string();
                p.current_track = "Trumsektion & Kick (Electro)".to_string();
                p.current_idx = 1;
                p.total_tracks = 6;
                p.progress_ratio = 0.15;
                p.completed_payload = None;
                p.error_message = None;
            }
        }

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(140));
            if let Ok(mut p) = progress.lock() {
                p.stage = crate::i18n::t("Konfigurerar mönster & arpeggios...").to_string();
                p.current_track = "Acid Groove & Synth Lead".to_string();
                p.current_idx = 3;
                p.progress_ratio = 0.55;
            }

            std::thread::sleep(std::time::Duration::from_millis(140));
            if let Ok(mut p) = progress.lock() {
                p.stage = crate::i18n::t("Bygger tidslinje och effekter...").to_string();
                p.current_track = "303 Resonant Sub Bass".to_string();
                p.current_idx = 6;
                p.progress_ratio = 0.90;
            }

            let mut tracks = Vec::new();
            let track_defs = [
                ("🥁 Trumsektion (Electro)", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
                ("💥 Kick & Punch", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
                ("👏 Clap & Percussion", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
                ("🧂 Hi-Hats & Shakers", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
                ("🎹 80s Poly Synth Lead", TrackKind::SynthLead, Color32::from_rgb(65, 155, 255)),
                ("🎸 303 Resonant Sub Bass", TrackKind::Bassline, Color32::from_rgb(45, 225, 105)),
                (crate::i18n::t("🎤 Mic (Voice & Sång)"), TrackKind::VocalAudio, Color32::WHITE),
            ];

            for (name, kind, _col) in track_defs {
                let clips = [None; 32];
                tracks.push(PreloadedTrackData {
                    frozen: None,
                    name: name.to_string(),
                    volume: 0.90,
                    pan: 0.0,
                    muted: false,
                    solo: false,
                    regions: Vec::new(),
                    clips,
                    eq: TrackEq::default(),
                    comp_threshold_db: 0.0,
                    comp_ratio: 1.0,
                    reverb_send: 0.15,
                    delay_send: 0.10,
                    automation: Vec::new(),
                    bus: default_bus_for_kind(kind),
                    vca: None,
                    stem_pcms: Vec::new(),
                });
            }

            if tracks.len() >= 6 {
                for b in 0..4 { tracks[1].clips[b] = Some(1); }
                for b in 4..16 { tracks[1].clips[b] = Some(0); }
                for b in 4..8 { tracks[2].clips[b] = Some(0); }
                for b in 8..12 { tracks[2].clips[b] = Some(2); }
                for b in 12..16 { tracks[2].clips[b] = Some(0); }
                for b in 0..16 { tracks[3].clips[b] = Some(3); }
                for b in 0..8 { tracks[4].clips[b] = Some(2); }
                tracks[5].clips[7] = Some(1);
                tracks[5].clips[15] = Some(1);
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Ok(mut p) = progress.lock() {
                p.progress_ratio = 1.0;
                p.stage = crate::i18n::t("Färdig! Laddar Synthwave Demo...").to_string();
                p.completed_payload = Some(LoadedProjectPayload {
                    name: "Sonix Synthwave Demo".to_string(),
                    bpm: 126.0,
                    swing: 0.15,
                    tempo_points: Vec::new(),
                    master_volume: 0.90,
                    master_pan: 0.0,
                    tracks,
                    plugin_slots: Vec::new(),
                    bus_volume: default_bus_volume(),
                    bus_muted: [false; crate::audio::synth::NUM_BUSES],
                    bus_solo: [false; crate::audio::synth::NUM_BUSES],
                    vca_volume: default_vca_volume(),
                    vca_muted: [false; crate::audio::synth::NUM_VCAS],
                    vca_solo: [false; crate::audio::synth::NUM_VCAS],
                    patterns: Vec::new(),
                    selected_pattern: 0,
                    step_velocities: None,
                    channels: Vec::new(),
                    file_path: "demo".to_string(),
                });
            }
        });
    }

    /// Regionerna ett spår spelar, i sekunder, som motorn vill ha dem.
    ///
    /// Projektets tempokarta.
    ///
    /// I dag en enda punkt: projektets tempo. Det är avsiktligt att allt som
    /// räknar tid frågar den här i stället för att läsa `bpm` själv — annars
    /// Sätter ett tempobyte (Fas 8.2).
    ///
    /// `bpm` speglar kartans första punkt så länge en karta finns. Det är en
    /// avbild av sanningen, inte en andra sanning: `tempo_map()` läser kartan.
    /// Skälet att spegla är att resten av appen visar `bpm`, och en siffra som
    /// visar något annat än det som hörs är en lögn.
    fn set_tempo_point(&mut self, bar: u32, bpm: f32) {
        crate::audio::tempo::set_tempo_point(&mut self.tempo_points, bar, bpm);
        self.bpm = self.tempo_map().bpm_at(0.0);
        self.status_message = crate::tstatus!("⏱ Tempobyte: Takt {} = {:.1} BPM", bar + 1, bpm);
    }

    /// Tar bort ett tempobyte. Takt 1 går inte — den är kartans början.
    fn remove_tempo_point(&mut self, bar: u32) {
        if crate::audio::tempo::remove_tempo_point(&mut self.tempo_points, bar) {
            self.bpm = self.tempo_map().bpm_at(0.0);
            self.status_message = crate::tstatus!("🗑 Tempobyte i takt {} borttaget", bar + 1);
        } else if bar == 0 {
            self.status_message = crate::i18n::t(
                "Tempot i takt 1 går inte att ta bort — ändra det i stället.",
            )
            .to_string();
        }
    }
    /// uppstår den andra sanningen om takter och sekunder, och den här gången
    /// blir det ingen.
    /// Tempokartan som lista (Fas 8.2).
    ///
    /// Punkterna sätts med högerklick på taktlinjalen; här syns de och kan
    /// ändras eller tas bort. En rad per byte är den enklaste form som går att
    /// förstå utan att någon visat den.
    fn render_tempo_modal(&mut self, ctx: &egui::Context) {
        if !self.show_tempo_modal {
            return;
        }
        let mut open = self.show_tempo_modal;
        let mut remove: Option<u32> = None;
        let mut set: Option<(u32, f32)> = None;
        egui::Window::new(crate::i18n::t("⏱ Tempokarta"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(340.0, 220.0))
            .show(ctx, |ui| {
                if self.tempo_points.is_empty() {
                    ui.label(crate::tstatus!(
                        "Projektet har ett enda tempo: {:.1} BPM.",
                        self.bpm
                    ));
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(crate::i18n::t(
                            "Högerklicka på taktlinjalen för att sätta ett byte.",
                        ))
                        .size(9.5)
                        .color(Theme::TEXT_MUTED),
                    );
                    return;
                }
                ui.label(crate::i18n::t("Ett byte gäller från sin takt och framåt:"));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for point in &self.tempo_points {
                        let bar = point.start_bar;
                        ui.horizontal(|ui| {
                            ui.label(crate::tstatus!("Takt {}", bar + 1));
                            let mut bpm = point.bpm;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut bpm)
                                        .speed(0.5)
                                        .range(40.0..=260.0)
                                        .suffix(" BPM"),
                                )
                                .changed()
                            {
                                set = Some((bar, bpm));
                            }
                            if bar > 0 {
                                if ui
                                    .button("🗑")
                                    .on_hover_text(crate::i18n::t("Ta bort bytet"))
                                    .clicked()
                                {
                                    remove = Some(bar);
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new(crate::i18n::t("(start)"))
                                        .size(9.5)
                                        .color(Theme::TEXT_MUTED),
                                );
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Högerklicka på taktlinjalen för att lägga till eller ta bort.",
                    ))
                    .size(9.5)
                    .color(Theme::TEXT_MUTED),
                );
            });
        self.show_tempo_modal = open;
        if let Some((bar, bpm)) = set {
            self.set_tempo_point(bar, bpm);
        }
        if let Some(bar) = remove {
            self.remove_tempo_point(bar);
        }
    }

    /// Ser till att spårets vågformscache finns och hör till dess nuvarande ljud.
    ///
    /// Anropas en gång per spår och bildruta. Är cachen byggd ur samma buffert
    /// som spåret har nu, kostar anropet en jämförelse av fyra tal.
    fn ensure_waveform_cache(&mut self, t_idx: usize) {
        let Some(track) = self.playlist_tracks.get_mut(t_idx) else {
            return;
        };
        let Some((left, _right, _sr)) = track.frozen_pcm.as_ref().or(track.pcm_audio.as_ref())
        else {
            track.waveform_cache = None;
            return;
        };
        let key = waveform_key(left);
        if track.waveform_cache.as_ref().map(|(k, _)| *k) == Some(key) {
            return;
        }
        track.waveform_cache = Some((key, crate::audio::waveform::WaveformCache::build(left)));
    }

    fn tempo_map(&self) -> crate::audio::tempo::TempoMap {
        if self.tempo_points.is_empty() {
            return crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        }
        crate::audio::tempo::TempoMap::from_points(self.tempo_points.clone())
    }

    /// **Ett ställe för omräkningen takter→sekunder.** Tre funktioner gjorde
    /// samma sak förut (två synkvägar och exporten), vilket är hur två svar på
    /// samma fråga uppstår. Nu går de genom tempokartan (Fas 8.2), så att en
    /// framtida tempokarta slår igenom i uppspelning och export samtidigt.
    ///
    /// Ett fruset spår är ETT långt ljud från början; ett ljudspår har sina egna
    /// regioner. Längder räknas som längder (inte som en differens av två
    /// positioner), eftersom en kloss kan sträcka sig över ett tempobyte.
    fn stem_regions_for(&self, t: &PlaylistTrack) -> Vec<StemRegionPlayback> {
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let mut regions: Vec<StemRegionPlayback> = Vec::new();
        if let Some((l, _r, sr)) = t.frozen_pcm.as_ref() {
            regions.push(frozen_region(l, *sr));
        }
        regions.extend(t.regions.iter().map(|r| {
            let from = r.start_bar as f64;
            StemRegionPlayback {
                start_time_secs: tempo.secs_at_bar(from) as f32,
                length_secs: tempo.secs_for_bars_at(from, r.length_bars as f64) as f32,
                sample_offset_sec: r.sample_offset_sec,
                gain: r.volume,
                fade_in_sec: tempo.secs_for_bars_at(from, r.fade_in_bars as f64) as f32,
                fade_out_sec: tempo.secs_for_bars_at(from, r.fade_out_bars as f64) as f32,
                muted: r.muted,
                is_reverse: r.is_reverse,
                loop_length_secs: tempo.secs_for_bars_at(from, r.loop_length_bars as f64) as f32,
            }
        }));
        regions
    }

    pub fn sync_track_regions(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            let region_playbacks = self.stem_regions_for(&self.playlist_tracks[track_idx]);
            let _ = self.engine.send_command(AudioCommand::SetStemTrackRegions {
                track_index: track_idx,
                regions: region_playbacks,
            });
        }
    }

    pub fn find_region_at_playhead(&self) -> Option<(usize, usize)> {
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let current_playhead_bar = tempo.bar_at_secs(self.song_time as f64) as f32;

        // First check selected track
        if self.selected_timeline_track < self.playlist_tracks.len() {
            for (r_i, r) in self.playlist_tracks[self.selected_timeline_track].regions.iter().enumerate() {
                if current_playhead_bar >= r.start_bar && current_playhead_bar <= r.start_bar + r.length_bars {
                    return Some((self.selected_timeline_track, r_i));
                }
            }
        }

        // Then check all tracks
        for (t_i, track) in self.playlist_tracks.iter().enumerate() {
            for (r_i, r) in track.regions.iter().enumerate() {
                if current_playhead_bar >= r.start_bar && current_playhead_bar <= r.start_bar + r.length_bars {
                    return Some((t_i, r_i));
                }
            }
        }
        None
    }

    pub fn split_selected_region_at_playhead(&mut self) {
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let current_playhead_bar = tempo.bar_at_secs(self.song_time as f64) as f32;
        let cur_cut_sec = (self.song_time * 100.0).round() / 100.0;

        // Target region: either explicitly selected (and its target split point), or region under playhead, or selected track's region
        let target: Option<(usize, usize, f32)> = if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                let orig = &self.playlist_tracks[t_idx].regions[r_idx];
                if current_playhead_bar >= orig.start_bar && current_playhead_bar <= (orig.start_bar + orig.length_bars) {
                    Some((t_idx, r_idx, current_playhead_bar))
                } else if let Some((f_t, f_r)) = self.find_region_at_playhead() {
                    Some((f_t, f_r, current_playhead_bar))
                } else {
                    // Playhead is outside: split selected region at midpoint (50%)
                    let mid_bar = orig.start_bar + orig.length_bars * 0.5;
                    Some((t_idx, r_idx, mid_bar))
                }
            } else {
                self.find_region_at_playhead().map(|(t, r)| (t, r, current_playhead_bar))
            }
        } else if let Some((f_t, f_r)) = self.find_region_at_playhead() {
            Some((f_t, f_r, current_playhead_bar))
        } else if self.selected_timeline_track < self.playlist_tracks.len() && !self.playlist_tracks[self.selected_timeline_track].regions.is_empty() {
            let orig = &self.playlist_tracks[self.selected_timeline_track].regions[0];
            let mid_bar = orig.start_bar + orig.length_bars * 0.5;
            Some((self.selected_timeline_track, 0, mid_bar))
        } else {
            None
        };

        if let Some((t_idx, r_idx, cut_bar)) = target {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
                let orig_start_sec = tempo.secs_at_bar(orig.start_bar as f64) as f32;
                let orig_len_sec =
                    tempo.secs_for_bars_at(orig.start_bar as f64, orig.length_bars as f64) as f32;
                let raw_cut_sec = tempo.secs_at_bar(cut_bar as f64) as f32;
                let mut cut_sec = (raw_cut_sec * 100.0).round() / 100.0;

                // Ensure cut point is inside this region
                let min_cut = orig_start_sec + 0.02;
                let max_cut = orig_start_sec + orig_len_sec - 0.02;
                if cut_sec < min_cut || cut_sec > max_cut {
                    cut_sec = orig_start_sec + orig_len_sec * 0.5;
                }

                let split_offset_sec = (cut_sec - orig_start_sec).clamp(0.01, orig_len_sec - 0.01);
                let split_offset_bar = (tempo.bars_for_secs_at(
                    orig.start_bar as f64,
                    split_offset_sec as f64,
                ) as f32
                    * 100.0)
                    .round()
                    / 100.0;

                let split_points = if orig_len_sec > 0.001 {
                    ((split_offset_sec / orig_len_sec) * orig.waveform_peaks.len() as f32) as usize
                } else {
                    orig.waveform_peaks.len() / 2
                };
                let split_idx = split_points.clamp(1, orig.waveform_peaks.len().saturating_sub(1).max(1));
                let left_peaks = orig.waveform_peaks[..split_idx.min(orig.waveform_peaks.len())].to_vec();
                let right_peaks = orig.waveform_peaks[split_idx.min(orig.waveform_peaks.len())..].to_vec();

                let clean_title = orig.name
                    .replace(" [Del 1]", "")
                    .replace(" [Del 2]", "")
                    .replace(" (Kopia)", "")
                    .trim()
                    .to_string();

                // Distinct alternating color for Part 2 so both slices stand out immediately
                let new_color = match orig.color {
                    c if c == Theme::FL_CYAN => Theme::FL_ORANGE,
                    c if c == Theme::FL_ORANGE => Theme::FL_GREEN,
                    c if c == Theme::FL_GREEN => Theme::FL_PURPLE,
                    c if c == Theme::FL_PURPLE => Color32::from_rgb(80, 180, 255),
                    _ => Theme::FL_CYAN,
                };

                let r_left = AudioRegion {
                    id: orig.id,
                    name: format!("{} [Del 1]", clean_title),
                    start_bar: orig.start_bar,
                    length_bars: split_offset_bar,
                    sample_offset_sec: orig.sample_offset_sec,
                    source_path: orig.source_path.clone(),
                    waveform_peaks: left_peaks.clone(),
                    volume: orig.volume,
                    fade_in_bars: orig.fade_in_bars.min(split_offset_bar * 0.5),
                    fade_out_bars: 0.0,
                    muted: orig.muted,
                    is_reverse: orig.is_reverse,
                    color: orig.color,
                    loop_length_bars: 0.0,
                };

                let r_right = AudioRegion {
                    id: orig.id + 1000 + (self.playlist_tracks[t_idx].regions.len() * 10),
                    name: format!("{} [Del 2]", clean_title),
                    start_bar: orig.start_bar + split_offset_bar,
                    length_bars: ((orig.length_bars - split_offset_bar) * 100.0).round() / 100.0,
                    sample_offset_sec: ((orig.sample_offset_sec + split_offset_sec) * 100.0).round() / 100.0,
                    source_path: orig.source_path,
                    waveform_peaks: if right_peaks.is_empty() { left_peaks } else { right_peaks },
                    volume: orig.volume,
                    fade_in_bars: 0.0,
                    fade_out_bars: orig.fade_out_bars.min((orig.length_bars - split_offset_bar) * 0.5),
                    muted: orig.muted,
                    is_reverse: orig.is_reverse,
                    color: new_color,
                    loop_length_bars: 0.0,
                };

                self.push_undo(&format!("Klipp '{}' vid spelhuvud", clean_title));
                self.playlist_tracks[t_idx].regions.remove(r_idx);
                self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
                self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
                self.sync_track_regions(t_idx);
                self.selected_timeline_track = t_idx;
                self.selected_audio_region = Some((t_idx, r_idx + 1)); // Highlight Part 2
                self.status_message = crate::tstatus!("✂ Klippte '{}' vid {} (Takt {})! [Ångra: Ctrl+Z]", clean_title, format_time_hundredths(orig_start_sec + split_offset_sec), format_bar_subdivisions(orig.start_bar + split_offset_bar));
            }
        } else {
            self.status_message = crate::tstatus!("⚠ Inget ljudklipp markerat eller vid spelhuvudet ({}) att klippa.", format_time_hundredths(cur_cut_sec));
        }
    }

    pub fn save_region_to_sound_browser(&mut self, track_idx: usize, region_idx: usize) {
        if track_idx >= self.playlist_tracks.len() || region_idx >= self.playlist_tracks[track_idx].regions.len() {
            return;
        }

        let region = self.playlist_tracks[track_idx].regions[region_idx].clone();
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let reg_len_sec =
            (tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32)
                .max(0.05);
        let reg_offset_sec = region.sample_offset_sec.max(0.0);

        // Destination: the canonical user sample bank (Fas 6.0).
        let save_dir = crate::paths::paths().samples_dir();
        let _ = std::fs::create_dir_all(&save_dir);

        // Sanitize name for file
        let clean_name = region.name
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "_")
            .trim()
            .to_string();
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let filename = format!("{}_{}.wav", clean_name, timestamp);
        let dest_path = save_dir.join(&filename);
        let dest_path_str = dest_path.to_string_lossy().to_string();

        // Extract PCM audio
        let sr: u32 = 44100;
        let mut out_pcm: Vec<f32> = Vec::new();

        if let Some((ref l, ref r, track_sr)) = self.playlist_tracks[track_idx].pcm_audio {
            let start_sample = ((reg_offset_sec * track_sr as f32) as usize).min(l.len());
            let num_samples = ((reg_len_sec * track_sr as f32) as usize).min(l.len().saturating_sub(start_sample));
            if num_samples > 0 {
                out_pcm.reserve(num_samples);
                for i in 0..num_samples {
                    let s_l = l.get(start_sample + i).copied().unwrap_or(0.0);
                    let s_r = r.get(start_sample + i).copied().unwrap_or(s_l);
                    out_pcm.push((s_l + s_r) * 0.5 * region.volume);
                }
            }
        } else if let Some(ref src_p) = region.source_path
            && let Ok((l, r, track_sr)) = crate::audio::load_wav_pcm(src_p) {
                let start_sample = ((reg_offset_sec * track_sr as f32) as usize).min(l.len());
                let num_samples = ((reg_len_sec * track_sr as f32) as usize).min(l.len().saturating_sub(start_sample));
                if num_samples > 0 {
                    out_pcm.reserve(num_samples);
                    for i in 0..num_samples {
                        let s_l = l.get(start_sample + i).copied().unwrap_or(0.0);
                        let s_r = r.get(start_sample + i).copied().unwrap_or(s_l);
                        out_pcm.push((s_l + s_r) * 0.5 * region.volume);
                    }
                }
        }

        // Fallback: If no raw PCM was cached, synthesize wave from waveform peaks
        if out_pcm.is_empty() {
            let total_samples = (reg_len_sec * sr as f32) as usize;
            out_pcm = Vec::with_capacity(total_samples);
            let num_peaks = region.waveform_peaks.len().max(1);
            for i in 0..total_samples {
                let p_idx = ((i as f32 / total_samples as f32) * num_peaks as f32) as usize;
                let amp = region.waveform_peaks.get(p_idx).copied().unwrap_or(0.5);
                let t = i as f32 / sr as f32;
                let sample = (t * 440.0 * std::f32::consts::TAU).sin() * amp * region.volume;
                out_pcm.push(sample);
            }
        }

        // Write WAV file to disk
        let write_res = crate::audio::write_pcm_f32_to_wav(&dest_path_str, &out_pcm, sr, 1);
        if write_res.is_err() {
            let rel_path = format!("sample_{}_{}.wav", clean_name, timestamp);
            let _ = crate::audio::write_pcm_f32_to_wav(&rel_path, &out_pcm, sr, 1);
        }

        // Generate 32-point mini preview waveform
        let mut mini_wf = Vec::with_capacity(32);
        let chunk_sz = (out_pcm.len() / 32).max(1);
        for chunk in out_pcm.chunks(chunk_sz).take(32) {
            let max_val = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            mini_wf.push(max_val.clamp(0.05, 1.0));
        }
        while mini_wf.len() < 32 { mini_wf.push(0.1); }

        let item_id = self.sample_library.len() + 1;
        let item = LibrarySampleItem {
            id: item_id,
            name: region.name.clone(),
            category: "Egna Samples".to_string(),
            icon: "🎵".to_string(),
            default_note: 60,
            color: region.color,
            waveform: mini_wf,
            file_path: Some(dest_path_str.clone()),
        };

        self.sample_library.push(item);
        self.status_message = crate::tstatus!("💾 Sparade sample '{}' till Sound Browser! Fil: {}", region.name, filename);
    }

    pub fn add_sample_item_to_timeline(&mut self, item: &LibrarySampleItem) {
        let mut track = PlaylistTrack::new(format!("🎵 {}", item.name), "🎵", TrackKind::CustomAudio, item.color);
        track.volume = 0.90;

        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let mut pcm_opt = None;
        let duration_secs = if let Some(ref path) = item.file_path && let Ok((l, r, sr)) = crate::audio::load_wav_pcm(path) {
            let dur = l.len() as f32 / sr as f32;
            pcm_opt = Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr));
            dur
        } else {
            tempo.secs_for_bars_at(0.0, 4.0) as f32
        };
        let reg_len_bars = (tempo.bars_for_secs_at(0.0, duration_secs as f64) as f32).max(0.25);

        let region = AudioRegion {
            id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize,
            name: item.name.clone(),
            start_bar: 0.0,
            length_bars: reg_len_bars,
            sample_offset_sec: 0.0,
            source_path: item.file_path.clone(),
            waveform_peaks: item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: item.color,
            loop_length_bars: reg_len_bars,
        };

        track.regions.push(region);

        if let Some((pcm_l, pcm_r, sr)) = pcm_opt {
            track.pcm_audio = Some((pcm_l, pcm_r, sr));
        }

        self.playlist_tracks.push(track);
        let new_t_idx = self.playlist_tracks.len() - 1;
        self.sync_track_regions(new_t_idx);
        self.selected_timeline_track = new_t_idx;
        self.selected_audio_region = Some((new_t_idx, 0));
        self.status_message = crate::tstatus!("➕ Lade till '{}' som nytt ljudspår på tidslinjen ({:.1} takter)!", item.name, reg_len_bars);
    }

    pub fn add_sample_item_to_track_at_bar(&mut self, track_idx: usize, item: &LibrarySampleItem, start_bar: f32) {
        if track_idx >= self.playlist_tracks.len() {
            let mut track = PlaylistTrack::new(format!("🎵 {}", item.name), "🎵", TrackKind::CustomAudio, item.color);
            track.volume = 0.90;
            self.playlist_tracks.push(track);
        }

        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let mut pcm_opt = None;
        let duration_secs = if let Some(ref path) = item.file_path && let Ok((l, r, sr)) = crate::audio::load_wav_pcm(path) {
            let dur = l.len() as f32 / sr as f32;
            pcm_opt = Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr));
            dur
        } else {
            tempo.secs_for_bars_at(0.0, 4.0) as f32
        };
        // Längden mäts från takten där klippet hamnar, inte från noll: över ett
        // tempobyte är antalet takter inte samma sak beroende på var man mäter.
        let reg_len_bars =
            (tempo.bars_for_secs_at(start_bar as f64, duration_secs as f64) as f32).max(0.25);

        let reg_id = self.next_region_id();
        let region = AudioRegion {
            id: reg_id,
            name: item.name.clone(),
            start_bar: start_bar.max(0.0),
            length_bars: reg_len_bars,
            sample_offset_sec: 0.0,
            source_path: item.file_path.clone(),
            waveform_peaks: item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: item.color,
            loop_length_bars: reg_len_bars,
        };

        let target_track = &mut self.playlist_tracks[track_idx];
        if target_track.pcm_audio.is_none() {
            if let Some((pcm_l, pcm_r, sr)) = pcm_opt {
                target_track.pcm_audio = Some((pcm_l, pcm_r, sr));
            }
        }

        target_track.regions.push(region);
        let new_reg_idx = target_track.regions.len() - 1;
        self.sync_track_regions(track_idx);
        self.selected_timeline_track = track_idx;
        self.selected_audio_region = Some((track_idx, new_reg_idx));
        self.status_message = crate::tstatus!("🎵 Placerade sample '{}' på spår {} vid takt {:.2}!", item.name, track_idx + 1, start_bar + 1.0);
    }

    pub fn open_sample_in_vocal_studio(&mut self, item: &LibrarySampleItem) {
        let pcm = if let Some(ref path) = item.file_path && let Ok((l, _, _)) = crate::audio::load_wav_pcm(path) {
            l
        } else {
            (0..44100 * 2).map(|i| {
                let t = i as f32 / 44100.0;
                (t * midi_to_freq(item.default_note) * std::f32::consts::TAU).sin() * (1.0 - t * 0.45) * 0.8
            }).collect()
        };
        let new_take_idx = self.vocal_studio.load_sample_or_region_as_take(&item.name, pcm, 44100, item.color);
        self.view_mode = ViewMode::VocalStudio;
        self.status_message = crate::tstatus!("🎙 Öppnade sample '{}' i Sångstudion (Tagning {}) för isolerad provspelning & formning!", item.name, new_take_idx + 1);
    }

    pub fn open_region_in_vocal_studio(&mut self, track_idx: usize, region_idx: usize) {
        if track_idx >= self.playlist_tracks.len() || region_idx >= self.playlist_tracks[track_idx].regions.len() {
            self.view_mode = ViewMode::VocalStudio;
            return;
        }
        let reg = &self.playlist_tracks[track_idx].regions[region_idx];
        let r_name = reg.name.clone();
        let r_color = reg.color;
        let r_source = reg.source_path.clone();

        let mut extracted_pcm = Vec::new();
        let mut sr = 44100;

        if let Some((ref l, _, srate)) = self.playlist_tracks[track_idx].pcm_audio {
            sr = srate;
            let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
            let start_sec = tempo.secs_at_bar(reg.start_bar as f64) as f32 + reg.sample_offset_sec;
            let len_sec =
                tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32;
            let start_idx = (start_sec * sr as f32) as usize;
            let end_idx = ((start_sec + len_sec) * sr as f32) as usize;
            if start_idx < l.len() {
                extracted_pcm = l[start_idx..end_idx.min(l.len())].to_vec();
            }
        } else if let Some(ref path) = r_source && let Ok((l, _, srate)) = crate::audio::load_wav_pcm(path) {
            sr = srate;
            extracted_pcm = l;
        }

        if extracted_pcm.is_empty() {
            extracted_pcm = (0..44100 * 2).map(|i| {
                let t = i as f32 / 44100.0;
                (t * 220.0 * std::f32::consts::TAU).sin() * 0.7
            }).collect();
        }

        let new_take_idx = self.vocal_studio.load_sample_or_region_as_take(&r_name, extracted_pcm, sr, r_color);
        self.view_mode = ViewMode::VocalStudio;
        self.status_message = crate::tstatus!("🎙 Öppnade region '{}' i Sångstudion (Tagning {})!", r_name, new_take_idx + 1);
    }

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

    /// Samlar arrangemanget som MIDI-spår (Fas 6.3). Ljudspår (regioner) hoppas
    /// över — de har ingen MIDI-motsvarighet.
    fn collect_song_midi(&self) -> Vec<crate::audio::smf::MidiTrack> {
        let mut tracks: Vec<crate::audio::smf::MidiTrack> = Vec::new();
        for (t_idx, track) in self.playlist_tracks.iter().enumerate() {
            if !matches!(
                track.kind,
                TrackKind::Drums | TrackKind::SynthLead | TrackKind::Bassline
            ) {
                continue;
            }
            let velocities: [u8; 16] = std::array::from_fn(|i| {
                (self.step_velocities[i] * track.volume * 127.0).clamp(1.0, 127.0) as u8
            });
            let channel = midi_channel_for_track(t_idx);
            let mut notes = Vec::new();
            for bar in 0..32usize {
                if let Some(pat_idx) = track.clips[bar]
                    && let Some(pat) = self.patterns.get(pat_idx)
                {
                    notes.extend(pattern_bar_notes(
                        pat,
                        track.kind,
                        channel,
                        (bar * 16) as u32,
                        &velocities,
                    ));
                }
            }
            tracks.push(crate::audio::smf::MidiTrack {
                name: track.name.clone(),
                notes,
            });
        }
        if tracks.iter().all(|t| t.notes.is_empty()) {
            // Ingen spelad patternrad: exportera det valda patternet som en takt,
            // så knappen inte ger en tom fil.
            if let Some(pat) = self.patterns.get(self.selected_pattern) {
                let velocities: [u8; 16] = std::array::from_fn(|i| {
                    (self.step_velocities[i] * 127.0).clamp(1.0, 127.0) as u8
                });
                let mut notes = pattern_bar_notes(pat, TrackKind::Drums, 9, 0, &velocities);
                notes.extend(pattern_bar_notes(pat, TrackKind::SynthLead, 0, 0, &velocities));
                notes.extend(pattern_bar_notes(pat, TrackKind::Bassline, 1, 0, &velocities));
                tracks.clear();
                tracks.push(crate::audio::smf::MidiTrack {
                    name: pat.name.clone(),
                    notes,
                });
            }
        }
        tracks
    }

    /// Skriver arrangemanget som `.mid` i exportmappen (Fas 6.3).
    pub fn export_song_midi(&mut self) {
        let tracks = self.collect_song_midi();
        let note_count: usize = tracks.iter().map(|t| t.notes.len()).sum();
        if note_count == 0 {
            self.status_message =
                crate::i18n::t("⚠ Inga MIDI-noter att exportera — rita i ett pattern först.")
                    .to_string();
            return;
        }
        // Exporten frågar tempokartan i stället för att räkna själv: i dag har
        // kartan en enda punkt (projektets tempo) och filen blir byte för byte
        // den samma som förut — men den dag projekten har tempobyten följer de
        // med ut i MIDI-filen utan att någon behöver komma ihåg det här stället.
        let tempo_points: Vec<(f64, f32)> = self
            .tempo_map()
            .points()
            .iter()
            .map(|p| (p.start_bar as f64, p.bpm))
            .collect();
        let bytes = crate::audio::smf::write_midi_with_tempo(&tempo_points, &tracks);
        let dir = crate::paths::paths().exports_dir();
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join(format!("{}.mid", crate::autosave::slug(&self.project_name)));
        match crate::autosave::write_atomic(&file, &bytes) {
            Ok(()) => {
                self.status_message = crate::tstatus!(
                    "🎼 Exporterade {} noter i {} spår till {}",
                    note_count,
                    tracks.len(),
                    file.display()
                );
            }
            Err(e) => {
                self.status_message =
                    crate::tstatus!("⚠ Kunde inte skriva MIDI-filen: {}", e);
            }
        }
    }

    /// Importerar en `.mid` till det valda patternet (Fas 6.3).
    pub fn import_midi_into_selected_pattern(&mut self, path: &str) -> Result<MidiImportReport, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("kunde inte läsa {path}: {e}"))?;
        let parsed = crate::audio::smf::parse_midi(&bytes)?;
        let total: usize = parsed.tracks.iter().map(|t| t.notes.len()).sum();
        if total == 0 {
            return Err("filen innehöll inga noter".to_string());
        }
        let notes: Vec<crate::audio::smf::MidiNote> = parsed
            .notes_with_track()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect();
        let groups = notes_by_bar(&notes, parsed.ppq);
        let bars_in_file = groups.len();
        let idx = self.selected_pattern.min(self.patterns.len().saturating_sub(1));
        let Some(pat) = self.patterns.get(idx) else {
            return Err("inget pattern att importera till".to_string());
        };
        let name = pat.name.clone();
        let color = pat.color;

        // En fil på en takt beter sig precis som förut — mönstret fylls, och
        // ingen kloss sätts, för den som bygger mönster för hand vill placera dem
        // själv. En fil på flera takter blir i stället ett arrangemang: en kloss
        // per takt på det valda spåret, annars skulle noterna bara hamna i
        // mönster man inte ser.
        let as_arrangement = bars_in_file > 1;
        let track_idx = self.selected_timeline_track;
        let mut report = MidiImportReport::default();

        for (bar, bar_notes) in groups {
            if bar >= ARRANGEMENT_BARS {
                report.dropped_beyond_arrangement += bar_notes.len();
                continue;
            }
            let pat_idx = if bar == 0 {
                idx
            } else {
                self.patterns.push(Pattern {
                    name: crate::tstatus!("{} ⋅ takt {}", name, bar + 1),
                    color,
                    channel_steps: Vec::new(),
                    channel_notes: Vec::new(),
                    piano_roll_grid: [[false; 16]; 24],
                    take: crate::midi_take::Take::new(),
                });
                self.patterns.len() - 1
            };
            if let Some(pat) = self.patterns.get_mut(pat_idx) {
                let r = apply_bar_to_pattern(&bar_notes, parsed.ppq, pat);
                report.notes_placed += r.notes_placed;
                report.dropped_out_of_range += r.dropped_out_of_range;
            }
            if as_arrangement
                && let Some(track) = self.playlist_tracks.get_mut(track_idx)
            {
                track.clips[bar] = Some(pat_idx);
            }
            report.bars_imported += 1;
        }
        if self.bpm_source_is_file() {
            // Tempot i filen används bara som förslag när projektet står kvar på
            // sin ursprungs-BPM; annars vore en import en tyst tempoändring.
            if let Some(bpm) = parsed.bpm {
                self.bpm = bpm.clamp(40.0, 260.0);
            }
        }
        let fmt = if parsed.format == 0 {
            crate::i18n::t(" [format 0: en spår]")
        } else {
            ""
        };
        let placed_where = if as_arrangement {
            crate::tstatus!(
                " i {} mönster på spåret (takt 1–{})",
                report.bars_imported,
                report.bars_imported
            )
        } else {
            format!(" till pattern '{}'", name)
        };
        self.status_message = if report.dropped_beyond_arrangement + report.dropped_out_of_range == 0 {
            crate::tstatus!(
                "🎼 Importerade {} noter{}{}",
                report.notes_placed,
                placed_where,
                fmt
            )
        } else {
            crate::tstatus!(
                "🎼 Importerade {} noter{}{} — hoppade över {} bortom takt {} och {} utanför rutnätet",
                report.notes_placed,
                placed_where,
                fmt,
                report.dropped_beyond_arrangement,
                ARRANGEMENT_BARS,
                report.dropped_out_of_range
            )
        };
        Ok(report)
    }

    /// Sant när projektets tempo inte ändrats sedan starten — då får en import
    /// föreslå filens tempo.
    fn bpm_source_is_file(&self) -> bool {
        (self.bpm - 120.0).abs() < 0.05
    }

    /// Import av `.mid` (Fas 6.3): fil, målpattern och vad som hände.
    fn render_midi_import_modal(&mut self, ctx: &egui::Context) {
        if !self.show_midi_import_modal {
            return;
        }
        let mut path = self.midi_import_path.clone();
        let target = self
            .patterns
            .get(self.selected_pattern)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let mut do_import = false;
        let mut close_clicked = false;
        let mut open = self.show_midi_import_modal;
        egui::Window::new(crate::i18n::t("🎼 Importera MIDI-fil (.mid)"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(600.0, 190.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Noterna läggs i det valda patternet (första takten). Trummor går till trumkanalerna, toner till piano-rollen.",
                    ))
                    .size(11.5)
                    .color(Theme::TEXT_BRIGHT),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Fil:"));
                    ui.add(
                        egui::TextEdit::singleline(&mut path)
                            .desired_width(440.0)
                            .hint_text("/sökväg/till/fil.mid"),
                    );
                });
                ui.add_space(8.0);
                if ui.button(crate::i18n::t("  Importera  ")).clicked() {
                    do_import = true;
                }
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    close_clicked = true;
                }
                if !target.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(crate::tstatus!("Målpattern: '{}'", target))
                            .size(11.0)
                            .color(Theme::FL_CYAN),
                    );
                }
            });
        self.midi_import_path = path;
        if do_import {
            let file = self.midi_import_path.clone();
            if let Err(e) = self.import_midi_into_selected_pattern(&file) {
                self.status_message = crate::tstatus!("⚠ Kunde inte importera MIDI: {}", e);
            }
        }
        self.show_midi_import_modal = open && !close_clicked;
    }

    /// Bygger en ångringspunkt av läget just nu (Fas 6.2: mixern ingår).
    fn current_snapshot(&self, description: &str) -> TimelineUndoSnapshot {
        TimelineUndoSnapshot {
            playlist_tracks: self.playlist_tracks.clone(),
            patterns: self.patterns.clone(),
            bus_volume: self.bus_volume,
            bus_muted: self.bus_muted,
            bus_solo: self.bus_solo,
            vca_faders: self.vca_faders,
            vca_muted: self.vca_muted,
            vca_solos: self.vca_solos,
            selected_timeline_track: self.selected_timeline_track,
            selected_audio_region: self.selected_audio_region,
            song_time: self.song_time,
            description: description.to_string(),
        }
    }

    /// Mixerns ljudbild just nu — se [`mixer_digest`].
    fn mixer_state_digest(&self) -> u64 {
        mixer_digest(
            &self.playlist_tracks,
            &self.bus_volume,
            &self.bus_muted,
            &self.bus_solo,
            &self.vca_faders,
            &self.vca_muted,
            &self.vca_solos,
        )
    }

    /// Lägger en färdig ångringspunkt på historiken.
    fn push_undo_snapshot(&mut self, snapshot: TimelineUndoSnapshot) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 60 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        // Ändringen skrivs av autosaven vid frame-gränsen (se `update`).
        self.autosave_pending = true;
    }

    pub fn push_undo(&mut self, description: &str) {
        let snapshot = self.current_snapshot(description);
        self.push_undo_snapshot(snapshot);
    }

    /// Lägger tillbaka ett snapshot i appens state **och** i ljudmotorn, så att
    /// en ångring hörs och inte bara syns (Fas 6.2). `sync_track_regions` räcker
    /// inte: volym, pan, mute/solo, EQ, kompressor, sends och routing ligger i
    /// `sync_track_audio_state`, och buss/VCA i `sync_group_state`.
    fn restore_snapshot(&mut self, snapshot: &TimelineUndoSnapshot) {
        self.playlist_tracks = snapshot.playlist_tracks.clone();
        // Mönstren först, sedan UI-spegeln: `load_pattern_into_ui` skriver inte
        // över `patterns` med det gamla UI-läget, vilket `select_pattern` hade gjort.
        self.patterns = snapshot.patterns.clone();
        self.load_pattern_into_ui(self.selected_pattern);
        self.bus_volume = snapshot.bus_volume;
        self.bus_muted = snapshot.bus_muted;
        self.bus_solo = snapshot.bus_solo;
        self.vca_faders = snapshot.vca_faders;
        self.vca_muted = snapshot.vca_muted;
        self.vca_solos = snapshot.vca_solos;
        self.selected_timeline_track = snapshot.selected_timeline_track;
        self.selected_audio_region = snapshot.selected_audio_region;
        self.song_time = snapshot.song_time;

        for t_idx in 0..self.playlist_tracks.len() {
            self.sync_track_regions(t_idx);
            self.sync_track_audio_state(t_idx);
        }
        self.sync_group_state();
        // Historiens läge är nu "viloläge" för mixer-undon, annars skulle
        // ångringen själv registreras som en ny mixerändring.
        self.mixer_settled_digest = self.mixer_state_digest();
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = self.current_snapshot(&snapshot.description);
            self.redo_stack.push(current);

            self.restore_snapshot(&snapshot);
            // Även en ångring är en strukturell ändring som ska skyddas.
            self.autosave_pending = true;
            self.status_message = crate::tstatus!("↶ Ångrade: {} (Ctrl+Z)", snapshot.description);
        } else {
            self.status_message = crate::i18n::t("Ingenting att ångra.").to_string();
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = self.current_snapshot(&snapshot.description);
            self.undo_stack.push(current);

            self.restore_snapshot(&snapshot);
            // Även en omgörning ändrar projektet — skyddas på samma sätt.
            self.autosave_pending = true;
            self.status_message = crate::tstatus!("↷ Gjorde om: {} (Ctrl+Y)", snapshot.description);
        } else {
            self.status_message = crate::i18n::t("Ingenting att göra om.").to_string();
        }
    }

    pub fn next_region_id(&self) -> usize {
        let max_id = self.playlist_tracks.iter()
            .flat_map(|t| t.regions.iter())
            .map(|r| r.id)
            .max()
            .unwrap_or(100);
        max_id + 1
    }

    const SCOPE_HISTORY_MAX: usize = 2048;

    /// Pulls the real output waveform samples produced since the last UI frame
    /// and keeps a sliding history for the oscilloscope display.
    fn update_scope_history(&mut self) {
        let new = self.engine.drain_scope_samples();
        if new.is_empty() {
            return;
        }
        self.scope_history.extend(new);
        if self.scope_history.len() > Self::SCOPE_HISTORY_MAX {
            let excess = self.scope_history.len() - Self::SCOPE_HISTORY_MAX;
            self.scope_history.drain(0..excess);
        }
    }

    pub fn duplicate_track(&mut self, track_idx: usize) {
        if track_idx >= self.playlist_tracks.len() {
            return;
        }
        let orig_name = self.playlist_tracks[track_idx].name.clone();
        ui_dbg(&format!(
            "duplicate_track idx={} name='{}' playing={} tracks_before={}",
            track_idx,
            orig_name,
            self.is_playing,
            self.playlist_tracks.len()
        ));
        self.push_undo(&crate::tstatus!("Duplicera spår '{}'", orig_name));

        let mut new_track = self.playlist_tracks[track_idx].clone();
        let clean_name = orig_name.replace(" (Kopia)", "");
        new_track.name = format!("{} (Kopia)", clean_name);
        for (i, r) in new_track.regions.iter_mut().enumerate() {
            r.id = self.next_region_id() + i * 10;
        }

        let insert_idx = track_idx + 1;
        self.playlist_tracks.insert(insert_idx, new_track);
        self.ensure_mic_track_exists();

        // Refresh all tracks in audio engine
        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        for i in 0..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
            self.sync_track_audio_state(i);
            self.sync_track_regions(i);
        }

        self.selected_timeline_track = insert_idx.min(self.playlist_tracks.len().saturating_sub(1));
        if !self.playlist_tracks[self.selected_timeline_track].regions.is_empty() {
            self.selected_audio_region = Some((self.selected_timeline_track, 0));
        } else {
            self.selected_audio_region = None;
        }

        self.status_message = crate::tstatus!("📋 Duplicerade spår '{}' under originalspåret!", orig_name);
        ui_dbg(&format!(
            "duplicate_track done name='{}' tracks_after={} playing={} selected={}",
            orig_name,
            self.playlist_tracks.len(),
            self.is_playing,
            self.selected_timeline_track
        ));
    }

    pub fn delete_track(&mut self, track_idx: usize) {
        if track_idx >= self.playlist_tracks.len() || self.playlist_tracks.len() <= 1 {
            return;
        }
        let orig_name = self.playlist_tracks[track_idx].name.clone();
        if orig_name.to_lowercase().contains("mic") && self.playlist_tracks.iter().filter(|t| t.name.to_lowercase().contains("mic")).count() <= 1 {
            self.status_message = crate::i18n::t("⚠️ Mikrofonspåret kan inte tas bort.").to_string();
            return;
        }
        self.push_undo(&crate::tstatus!("Ta bort spår '{}'", orig_name));
        self.playlist_tracks.remove(track_idx);
        self.ensure_mic_track_exists();

        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        for i in 0..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
            self.sync_track_audio_state(i);
            self.sync_track_regions(i);
        }
        self.selected_timeline_track = track_idx.min(self.playlist_tracks.len().saturating_sub(1));
        self.selected_audio_region = None;
        self.status_message = crate::tstatus!("🗑 Raderade spår '{}'.", orig_name);
    }

    pub fn copy_selected_region(&mut self) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                let reg = self.playlist_tracks[t_idx].regions[r_idx].clone();
                let name = reg.name.clone();
                self.copied_region = Some(reg);
                self.status_message = crate::tstatus!("📋 Kopierade sample '{}' till urklipp! [Klistra in: Ctrl+V]", name);
            }
        } else {
            self.status_message = crate::i18n::t("Ingen ljudregion markerad att kopiera.").to_string();
        }
    }

    pub fn paste_region(&mut self) {
        if let Some(ref copied) = self.copied_region.clone() {
            if self.playlist_tracks.is_empty() {
                return;
            }
            let target_track = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
            let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
            let paste_bar = (tempo.bar_at_secs(self.song_time as f64) as f32).max(0.0);

            self.push_undo(&format!("Klistra in '{}'", copied.name));
            let mut new_r = copied.clone();
            new_r.id = self.next_region_id();
            new_r.start_bar = paste_bar;
            let clean_name = copied.name.replace(" (Kopia)", "");
            new_r.name = format!("{} (Kopia)", clean_name);

            self.playlist_tracks[target_track].regions.push(new_r.clone());
            self.sync_track_regions(target_track);
            let new_r_idx = self.playlist_tracks[target_track].regions.len() - 1;
            self.selected_audio_region = Some((target_track, new_r_idx));
            self.status_message = crate::tstatus!("📋 Klistrade in sample '{}' på spår {} vid takt {:.2}! [Ångra: Ctrl+Z]", new_r.name, target_track + 1, paste_bar + 1.0);
        } else {
            self.status_message = crate::i18n::t("Urklipp är tomt. Kopiera en region först med Ctrl+C.").to_string();
        }
    }

    pub fn repeat_selected_region_loop(&mut self, factor: f32) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                self.push_undo("Repetera sample loop");
                let reg = &mut self.playlist_tracks[t_idx].regions[r_idx];
                if reg.loop_length_bars <= 0.001 {
                    reg.loop_length_bars = reg.length_bars;
                }
                let base_loop = reg.loop_length_bars;
                reg.length_bars = (reg.length_bars * factor).max(base_loop);
                let new_len = reg.length_bars;
                let num_reps = (new_len / base_loop).round() as usize;
                let name = reg.name.clone();
                self.sync_track_regions(t_idx);
                self.status_message = crate::tstatus!("🔁 Loopade sample '{}': {} repetitioner ({:.1} takter)!", name, num_reps, new_len);
            }
        }
    }

    pub fn duplicate_selected_region(&mut self) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                self.push_undo("Duplicera ljudregion");
                let mut dup = self.playlist_tracks[t_idx].regions[r_idx].clone();
                dup.start_bar += dup.length_bars;
                dup.name = format!("{} (Kopia)", dup.name.replace(" (Kopia)", ""));
                dup.id = self.next_region_id();
                self.playlist_tracks[t_idx].regions.push(dup);
                self.sync_track_regions(t_idx);
                let new_idx = self.playlist_tracks[t_idx].regions.len() - 1;
                self.selected_audio_region = Some((t_idx, new_idx));
                self.status_message = crate::i18n::t("📋 Duplicerade ljudregion till tidslinjen!").to_string();
            }
        }
    }

    pub fn delete_selected_region(&mut self) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                let name = self.playlist_tracks[t_idx].regions[r_idx].name.clone();
                self.push_undo(&format!("Radera '{}'", name));
                self.playlist_tracks[t_idx].regions.remove(r_idx);
                self.sync_track_regions(t_idx);
                self.selected_audio_region = None;
                self.status_message = crate::tstatus!("🗑 Raderade region '{}'. [Ångra: Ctrl+Z]", name);
            }
        }
    }

    pub fn toggle_mute_selected_region(&mut self) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                self.push_undo("Muta/Avmuta region");
                self.playlist_tracks[t_idx].regions[r_idx].muted = !self.playlist_tracks[t_idx].regions[r_idx].muted;
                let is_m = self.playlist_tracks[t_idx].regions[r_idx].muted;
                self.sync_track_regions(t_idx);
                self.status_message = if is_m { crate::i18n::t("🔇 Region mutad").to_string() } else { crate::i18n::t("🔊 Region aktiv").to_string() };
            }
        }
    }

    pub fn reverse_selected_region(&mut self) {
        if let Some((t_idx, r_idx)) = self.selected_audio_region {
            if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                self.push_undo(crate::i18n::t("Vänd region baklänges (Reverse)"));
                self.playlist_tracks[t_idx].regions[r_idx].is_reverse = !self.playlist_tracks[t_idx].regions[r_idx].is_reverse;
                self.sync_track_regions(t_idx);
                self.status_message = crate::i18n::t("🔄 Vände ljudregion baklänges (Reverse)!").to_string();
            }
        }
    }

    /// Bygger projektets serialiserbara form.
    ///
    /// Delas av manuell sparning och autosave. Om de två byggde sina egna
    /// objekt kunde de glida isär, och då skulle autosaven skydda något annat
    /// än det användaren faktiskt sparar.
    fn project_data(&self, name: &str) -> SonixProjectData {
        let saved_tracks: Vec<SavedTrackData> = self.playlist_tracks.iter().map(|t| SavedTrackData {
            name: t.name.clone(),
            volume: t.volume,
            pan: t.pan,
            muted: t.muted,
            solo: t.solo,
            clips: t.clips,
            regions: t.regions.clone(),
            frozen: t.frozen.as_ref().map(|f| SavedFrozenTrack {
                path: f.path.clone(),
                digest: f.digest,
                stamp: f.stamp,
            }),
            eq: t.eq.clone(),
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send: t.reverb_send,
            delay_send: t.delay_send,
            automation: t.automation.clone(),
            bus: t.bus,
            vca: t.vca,
        }).collect();

        SonixProjectData {
            name: name.to_string(),
            bpm: self.bpm,
            tempo_points: self.tempo_points.clone(),
            swing: self.swing,
            master_volume: self.master_volume,
            master_pan: self.master_pan,
            tracks: saved_tracks,
            bus_volume: self.bus_volume,
            bus_muted: self.bus_muted,
            bus_solo: self.bus_solo,
            vca_volume: self.vca_faders,
            vca_muted: self.vca_muted,
            vca_solo: self.vca_solos,
            patterns: self.patterns.iter().map(pattern_to_saved).collect(),
            selected_pattern: self.selected_pattern,
            step_velocities: Some(self.step_velocities),
            channels: self.channels.iter().map(channel_to_saved).collect(),
            plugin_slots: self
                .plugin_slots
                .iter()
                .map(|slot| {
                    slot.as_ref().map(|s| SavedPluginData {
                        path: s.path.clone(),
                        name: s.name.clone(),
                        state: s.state.clone(),
                        sandboxed: s.sandboxed,
                    })
                })
                .collect(),
        }
    }

    pub fn save_project(&mut self, name: &str) {
        let clean_name = if name.trim().is_empty() {
            crate::i18n::t("Namnlöst Projekt")
        } else {
            name.trim()
        };
        let dir = crate::paths::paths().projects_dir();
        let _ = std::fs::create_dir_all(&dir);

        let file_path = crate::paths::paths().project_file(clean_name);
        let data = self.project_data(clean_name);

        if let Ok(json) = serde_json::to_string_pretty(&data)
            // Temp + rename: en avbruten skrivning får aldrig ersätta en hel
            // projektfil med en halv.
            && crate::autosave::write_atomic(&file_path, json.as_bytes()).is_ok() {
                self.project_name = clean_name.to_string();
                self.project_file_path = Some(file_path.to_string_lossy().to_string());
                // Kom ihåg läget, så autosaven inte skriver en kopia av exakt
                // det som just sparades.
                self.autosave_last_fp = Some(crate::autosave::fingerprint(json.as_bytes()));
                self.remember_recent_project(clean_name, &file_path);
                self.status_message = crate::tstatus!("💾 Sparade projekt till '{}'!", file_path.display());
                return;
            }
        self.status_message = crate::i18n::t("❌ Misslyckades att spara projektet.").to_string();
    }

    /// Lägger projektet först i läslistan (`~/.local/state/sonix/recent.json`).
    fn remember_recent_project(&self, name: &str, path: &std::path::Path) {
        let mut entries = load_recent_projects();
        push_recent_project(&mut entries, name, &path.to_string_lossy());
        store_recent_projects(&entries);
    }

    /// Autosparar om projektet ändrats sedan det senast skyddades.
    ///
    /// Tre grindar innan en fil skrivs: läget får inte vara identiskt med förra
    /// autosaven, inte heller med filen på disk (då finns inget osparat), och
    /// skrivningen sker atomiskt i state-katalogen. Varje skrivning roterar
    /// historiken så att de senaste versionerna finns kvar.
    pub fn maybe_autosave(&mut self) {
        // Under en pågående projektinläsning är state halvfärdigt — att frysa
        // det som en autosave vore att skydda fel läge.
        if self
            .project_load_progress
            .try_lock()
            .map(|p| p.is_loading)
            .unwrap_or(false)
        {
            return;
        }

        let name = self.project_name.clone();
        let data = self.project_data(&name);
        let Ok(json) = serde_json::to_string_pretty(&data) else {
            return;
        };
        let bytes = json.as_bytes();
        let fp = crate::autosave::fingerprint(bytes);

        if self.autosave_last_fp == Some(fp) {
            return;
        }
        if let Some(path) = self.project_file_path.as_deref()
            && std::fs::read(path)
                .map(|disk| crate::autosave::fingerprint(&disk) == fp)
                .unwrap_or(false)
        {
            // Identiskt med det användaren redan sparat — inget att skydda.
            self.autosave_last_fp = Some(fp);
            return;
        }

        let dir = crate::paths::paths().autosave_dir();
        let stamp = crate::autosave::now_stamp();
        match crate::autosave::save(&dir, &name, bytes, stamp) {
            Ok(path) => {
                let removed = crate::autosave::prune(&dir, crate::autosave::KEEP_PER_PROJECT)
                    .unwrap_or(0);
                self.autosave_last_fp = Some(fp);
                self.autosave_last_write = Some(stamp);
                // Autosaven är en bakgrundshändelse och får inte skriva över en
                // färsk ångringsbekräftelse. Mätt i GUI: ångringen märker
                // ändringen för autosave, autosaven skriver nästa frame och
                // ersatte "↶ Ångrade …" inom millisekunder — så användaren kunde
                // inte läsa vad som just ångrades.
                let fresh_undo = self.status_message.starts_with("↶ ")
                    || self.status_message.starts_with("↷ ");
                if !fresh_undo {
                    self.status_message = if removed > 0 {
                        crate::tstatus!(
                            "🛟 Autosparade '{}' ({} äldre versioner städade)",
                            name,
                            removed
                        )
                    } else {
                        crate::tstatus!("🛟 Autosparade '{}'", name)
                    };
                }
                let _ = path;
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte autospara: {}", e);
            }
        }
    }

    /// Autosparar direkt efter en strukturell ändring (Fas 6.1: "vid varje
    /// strukturell ändring"), men högst var tionde sekund — annars skulle en
    /// jämn ström av undo-punkter skriva en fil per steg.
    pub fn autosave_after_structural_change(&mut self) {
        let now = crate::autosave::now_stamp();
        if let Some(last) = self.autosave_last_write
            && now.saturating_sub(last) < 10
        {
            return;
        }
        self.maybe_autosave();
    }

    /// Återställer en autosave från återställningsdialogen.
    ///
    /// Filen *pensioneras* (döps om till `*.restored`) i stället för att
    /// raderas: frågan ska inte ställas igen nästa start, men det återställda
    /// läget ska gå att gräva fram om användaren ångrar sig.
    pub fn restore_autosave(&mut self, index: usize) {
        let Some(candidate) = self.recovery_candidates.get(index).cloned() else {
            self.show_recovery_modal = false;
            return;
        };
        let path = candidate.entry.path.clone();
        self.recovery_candidates.clear();
        self.show_recovery_modal = false;
        match crate::autosave::retire(&path) {
            Ok(_) => {
                self.load_project_file(path.to_string_lossy().as_ref());
                self.autosave_accum = 0.0;
                self.autosave_last_fp = None;
                self.status_message = crate::tstatus!(
                    "🛟 Återställde autosave för '{}' — granska och spara projektet",
                    candidate.name
                );
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte återställa autosaven: {}", e);
            }
        }
    }

    pub fn load_project_file(&mut self, path_str: &str) {
        let path = path_str.to_string();
        let progress = self.project_load_progress.clone();

        {
            if let Ok(mut p) = progress.lock() {
                p.is_loading = true;
                p.project_name = std::path::Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| crate::i18n::t("Projekt").to_string());
                p.stage = crate::i18n::t("Läser projektfil och förbereder spår...").to_string();
                p.current_track = String::new();
                p.current_idx = 0;
                p.total_tracks = 0;
                p.progress_ratio = 0.05;
                p.completed_payload = None;
                p.error_message = None;
            }
        }

        std::thread::spawn(move || {
            // Small initial pause for UI modal display
            std::thread::sleep(std::time::Duration::from_millis(80));

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    if let Ok(mut p) = progress.lock() {
                        p.is_loading = false;
                        p.error_message = Some(crate::tstatus!("Kunde inte läsa filen: {}", e));
                    }
                    return;
                }
            };

            let data = match serde_json::from_str::<SonixProjectData>(&content) {
                Ok(d) => d,
                Err(e) => {
                    if let Ok(mut p) = progress.lock() {
                        p.is_loading = false;
                        p.error_message = Some(format!("Ogiltigt projektformat: {}", e));
                    }
                    return;
                }
            };

            let total_tracks = data.tracks.len();
            let mut preloaded_tracks = Vec::with_capacity(total_tracks);

            for (t_idx, st) in data.tracks.into_iter().enumerate() {
                let track_name = st.name.clone();
                let ratio = 0.10 + ((t_idx + 1) as f32 / total_tracks.max(1) as f32) * 0.85;

                if let Some(f) = &st.frozen
                    && !std::path::Path::new(&f.path).exists()
                {
                    // Säg det i stället för att tyst spela ett ofruset spår: en
                    // frysning vars fil försvunnit är inte samma sak.
                    if let Ok(mut p) = progress.lock() {
                        p.error_message = Some(crate::tstatus!(
                            "⚠ '{}' är fruset men filen saknas: {}",
                            st.name,
                            f.path
                        ));
                    }
                }
                if let Ok(mut p) = progress.lock() {
                    p.stage = crate::tstatus!("Läser in och avkodar ljudspår ({}/{})...", t_idx + 1, total_tracks);
                    p.current_track = track_name.clone();
                    p.current_idx = t_idx + 1;
                    p.total_tracks = total_tracks;
                    p.progress_ratio = ratio;
                }

                let mut stem_pcms = Vec::new();
                for r in &st.regions {
                    if let Some(ref p_src) = r.source_path {
                        if let Ok((pcm_l, pcm_r, sr)) = crate::audio::load_wav_pcm(p_src) {
                            stem_pcms.push((std::sync::Arc::new(pcm_l), std::sync::Arc::new(pcm_r), sr));
                        }
                    }
                }
                // Ett fruset spår har sitt ljud i en fil i projektets mapp. Att
                // lägga den i stem_pcms gör att samma väg som ljudspåren används
                // — både vid inläsning och vid en senare omsynk.
                if let Some(f) = &st.frozen
                    && let Ok((pcm_l, pcm_r, sr)) = crate::audio::load_wav_pcm(&f.path)
                {
                    stem_pcms.push((
                        std::sync::Arc::new(pcm_l),
                        std::sync::Arc::new(pcm_r),
                        sr,
                    ));
                }

                preloaded_tracks.push(PreloadedTrackData {
                    frozen: st.frozen,
                    name: st.name,
                    volume: st.volume,
                    pan: st.pan,
                    muted: st.muted,
                    solo: st.solo,
                    regions: st.regions,
                    clips: st.clips,
                    eq: st.eq,
                    comp_threshold_db: st.comp_threshold_db,
                    comp_ratio: st.comp_ratio,
                    reverb_send: st.reverb_send,
                    delay_send: st.delay_send,
                    automation: st.automation,
                    bus: st.bus,
                    vca: st.vca,
                    stem_pcms,
                });

                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            if let Ok(mut p) = progress.lock() {
                p.progress_ratio = 1.0;
                p.stage = crate::i18n::t("Färdigställer tidslinje och ljudmotor...").to_string();
                p.completed_payload = Some(LoadedProjectPayload {
                    name: data.name,
                    bpm: data.bpm,
                    tempo_points: data.tempo_points,
                    swing: data.swing,
                    master_volume: data.master_volume,
                    master_pan: data.master_pan,
                    tracks: preloaded_tracks,
                    plugin_slots: data.plugin_slots,
                    bus_volume: data.bus_volume,
                    bus_muted: data.bus_muted,
                    bus_solo: data.bus_solo,
                    vca_volume: data.vca_volume,
                    vca_muted: data.vca_muted,
                    vca_solo: data.vca_solo,
                    patterns: data.patterns,
                    selected_pattern: data.selected_pattern,
                    step_velocities: data.step_velocities,
                    channels: data.channels,
                    file_path: path,
                });
            }
        });
    }

    pub fn apply_loaded_project_payload(&mut self, payload: LoadedProjectPayload) {
        self.stop_playback();
        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        // The old plugins are being replaced: close their editors and hold the
        // shared handles until shutdown so the cores are not freed on the audio
        // thread while it may still be rendering the previous graph.
        self.close_all_plugin_guis();
        self.retire_all_plugin_handles();

        let plugin_slots = payload.plugin_slots;
        let bus_volume = payload.bus_volume;
        let bus_muted = payload.bus_muted;
        let bus_solo = payload.bus_solo;
        let vca_volume = payload.vca_volume;
        let vca_muted = payload.vca_muted;
        let vca_solo = payload.vca_solo;
        let patterns = payload.patterns;
        let selected_pattern = payload.selected_pattern;
        let step_velocities = payload.step_velocities;
        let channels = payload.channels;

        self.project_name = payload.name;
        self.bpm = payload.bpm;
        self.tempo_points = payload.tempo_points;
        self.swing = payload.swing;
        self.master_volume = payload.master_volume;
        self.master_pan = payload.master_pan;
        self.bus_volume = bus_volume;
        self.bus_muted = bus_muted;
        self.bus_solo = bus_solo;
        self.vca_faders = vca_volume;
        self.vca_muted = vca_muted;
        self.vca_solos = vca_solo;

        // Mönstren, kanalracket och stegvolymerna (Fas 6.7).
        restore_saved_music(
            &SavedMusic {
                patterns: &patterns,
                selected_pattern,
                step_velocities,
                channels: &channels,
            },
            &mut self.patterns,
            &mut self.channels,
            &mut self.selected_pattern,
            &mut self.step_velocities,
        );
        self.sync_group_state();

        self.playlist_tracks.clear();
        for (t_idx, st) in payload.tracks.into_iter().enumerate() {
            // Send preloaded PCM to engine, and keep a reference on the track so
            // a later ClearAllStemTracks + resync (duplicate/delete/reload) can
            // re-send it. Without this the app loses the PCM after project load.
            let mut track_pcm = None;
            for (pcm_l, pcm_r, sr) in st.stem_pcms {
                track_pcm = Some((pcm_l.clone(), pcm_r.clone(), sr));
                let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                    track_index: t_idx,
                    left: pcm_l,
                    right: pcm_r,
                    sample_rate: sr as f32,
                    volume: st.volume,
                    pan: st.pan,
                    start_time_secs: 0.0,
                });
            }

            let (kind, icon, color) = classify_track_style(&st.name);
            let mut loaded_track = PlaylistTrack::new(st.name, icon, kind, color);
            loaded_track.volume = st.volume;
            loaded_track.pan = st.pan;
            loaded_track.muted = st.muted;
            loaded_track.solo = st.solo;
            loaded_track.regions = st.regions;
            loaded_track.clips = st.clips;
            if st.frozen.is_some() {
                // Ljudet kom in via stem_pcms ovan; här knyts läget till spåret.
                loaded_track.frozen = st.frozen.map(|f| FrozenTrack {
                    path: f.path,
                    digest: f.digest,
                    stamp: f.stamp,
                });
                loaded_track.frozen_pcm = track_pcm.clone();
            }
            loaded_track.eq = st.eq;
            loaded_track.comp_threshold_db = st.comp_threshold_db;
            loaded_track.comp_ratio = st.comp_ratio;
            loaded_track.reverb_send = st.reverb_send;
            loaded_track.delay_send = st.delay_send;
            loaded_track.automation = st.automation;
            loaded_track.bus = st.bus.min(crate::audio::synth::NUM_BUSES - 1);
            loaded_track.vca = st.vca.filter(|&v| v < crate::audio::synth::NUM_VCAS);
            loaded_track.pcm_audio = track_pcm;
            self.playlist_tracks.push(loaded_track);
            self.sync_track_regions(t_idx);
        }

        // Ensure Mic track is always at the end & all track colors are properly classified
        self.ensure_mic_track_exists();

        // Re-instantiate per-track plugins and restore their state on the main
        // thread (clap.state is main-thread only, so it must not go through the
        // audio thread). Slot indices map 1:1 onto engine stem tracks.
        self.plugin_slots = plugin_slots
            .iter()
            .map(|slot| {
                slot.as_ref().map(|s| PluginSlot {
                    path: s.path.clone(),
                    name: s.name.clone(),
                    state: s.state.clone(),
                    sandboxed: s.sandboxed,
                })
            })
            .collect();
        let mut plugin_errors: Vec<String> = Vec::new();
        for (track_index, slot) in plugin_slots.iter().enumerate() {
            if let Some(data) = slot
                && let Some(err) = self.restore_plugin_slot(track_index, data)
            {
                plugin_errors.push(err);
            }
        }

        self.loop_end_bar = self.get_max_project_bars().max(32);
        self.project_file_path = Some(payload.file_path);
        // Öppnade projekt hör till läslistan precis som sparade (Fas 6.1).
        if let Some(path) = self.project_file_path.clone() {
            let name = self.project_name.clone();
            self.remember_recent_project(&name, std::path::Path::new(&path));
        }
        // Nytt läge: nästa autosave-kontroll jämför mot filen på disk i stället
        // för mot det förra projektets fingeravtryck.
        self.autosave_last_fp = None;
        self.autosave_accum = 0.0;
        self.selected_audio_region = None;
        let opened_name = self.project_name.clone();
        self.status_message = if plugin_errors.is_empty() {
            crate::tstatus!("📂 Öppnade projekt '{}'!", opened_name)
        } else {
            crate::tstatus!(
                "📂 Öppnade projekt '{}' – ⚠ {} plugin(s) kunde inte återställas",
                opened_name,
                plugin_errors.len()
            )
        };
    }

    /// Ensures that a dedicated Mic track exists at the very end of the playlist,
    /// and that all tracks have their categorized colors, icons, and region colors refreshed.
    pub fn ensure_mic_track_exists(&mut self) {
        if self.playlist_tracks.is_empty() {
            let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
            mic_track.volume = 1.0;
            mic_track.is_rec_armed = true;
            self.playlist_tracks.push(mic_track);
            return;
        }

        // 1. Check if any track is a mic track
        let mic_pos = self.playlist_tracks.iter().position(|t| {
            let n = t.name.to_lowercase();
            n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
        });

        if let Some(pos) = mic_pos {
            // If it's not already the last track, move it to the end
            if pos != self.playlist_tracks.len() - 1 {
                let mic_t = self.playlist_tracks.remove(pos);
                self.playlist_tracks.push(mic_t);
            }
        } else {
            // Create dedicated Mic track at the end
            let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
            mic_track.volume = 1.0;
            mic_track.is_rec_armed = true;
            self.playlist_tracks.push(mic_track);
        }

        // 2. Re-classify and ensure colors are applied to every track and its regions
        let last_idx = self.playlist_tracks.len() - 1;
        for (idx, track) in self.playlist_tracks.iter_mut().enumerate() {
            let is_mic = idx == last_idx && (track.name.to_lowercase().contains("mic") || track.name.to_lowercase().contains("mik") || track.name.to_lowercase().contains("voice"));
            if is_mic {
                track.color = Color32::WHITE;
                track.icon = "🎤";
                track.kind = TrackKind::VocalAudio;
                for r in &mut track.regions {
                    r.color = Color32::WHITE;
                }
            } else {
                let (kind, icon, color) = classify_track_style(&track.name);
                track.kind = kind;
                track.icon = icon;
                track.color = color;
                for r in &mut track.regions {
                    r.color = color;
                }
            }
        }
    }

    /// Lägger ett pattern i UI:t — kanalernas steg/toner och piano-rollen —
    /// **utan** att först spara det som står där.
    ///
    /// Används av ångringen (Fas 6.4), som redan har skrivit tillbaka rätt
    /// `patterns` och inte får skriva över dem med det gamla UI-läget.
    fn load_pattern_into_ui(&mut self, pat_idx: usize) {
        if pat_idx >= self.patterns.len() {
            return;
        }
        for (ch_i, ch) in self.channels.iter_mut().enumerate() {
            if ch_i < self.patterns[pat_idx].channel_steps.len() {
                ch.steps = self.patterns[pat_idx].channel_steps[ch_i];
                ch.notes = self.patterns[pat_idx].channel_notes[ch_i];
            }
        }
        self.piano_roll_grid = self.patterns[pat_idx].piano_roll_grid;
    }

    pub fn select_pattern(&mut self, pat_idx: usize) {
        if pat_idx < self.patterns.len() {
            // Save current channel steps to current pattern
            for (ch_i, ch) in self.channels.iter().enumerate() {
                if ch_i < self.patterns[self.selected_pattern].channel_steps.len() {
                    self.patterns[self.selected_pattern].channel_steps[ch_i] = ch.steps;
                    self.patterns[self.selected_pattern].channel_notes[ch_i] = ch.notes;
                }
            }
            self.patterns[self.selected_pattern].piano_roll_grid = self.piano_roll_grid;

            // Load new pattern
            self.selected_pattern = pat_idx;
            self.load_pattern_into_ui(pat_idx);
            self.status_message = crate::tstatus!("Aktivt mönster: {}", self.patterns[pat_idx].name);
        }
    }

    pub fn sync_active_pattern_from_ui(&mut self) {
        if self.selected_pattern < self.patterns.len() {
            for (ch_i, ch) in self.channels.iter().enumerate() {
                if ch_i < self.patterns[self.selected_pattern].channel_steps.len() {
                    self.patterns[self.selected_pattern].channel_steps[ch_i] = ch.steps;
                    self.patterns[self.selected_pattern].channel_notes[ch_i] = ch.notes;
                }
            }
            self.patterns[self.selected_pattern].piano_roll_grid = self.piano_roll_grid;
        }
    }

    pub fn select_sound_for_channel(&mut self, ch_idx: usize, item: &LibrarySampleItem) {
        if ch_idx < self.channels.len() {
            assign_library_sample_to_channel(&mut self.channels[ch_idx], item);
            let item_name = item.name.clone();
            self.status_message = crate::tstatus!("Kanal {} ändrad till: {}", ch_idx + 1, item_name);
            self.audition_library_sample(item);
        }
    }

    /// Auditions a library sample. Plays the real WAV when available, otherwise
    /// falls back to the built-in synthesizer for the matching sound category.
    pub fn audition_library_sample(&mut self, item: &LibrarySampleItem) {
        if let Some(ref path) = item.file_path
            && let Some((l, r, sr)) = load_sample_pcm_arcs(path)
        {
            let _ = self.engine.send_command(AudioCommand::PlayAudition {
                left: l,
                right: r,
                sample_rate: sr as f32,
                volume: 0.95,
                pitch_ratio: 1.0,
                time_stretch_ratio: 1.0,
                is_reverse: false,
                loop_playback: false,
            });
            return;
        }

        match item.category.as_str() {
            "Kicks" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
            "Snares" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
            "Claps" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
            "Hi-Hats" => {
                if item.name.to_lowercase().contains("open") {
                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
                } else {
                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
                }
            }
            "Percussion" => {
                if item.name.to_lowercase().contains("high") {
                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomHigh));
                } else {
                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomLow));
                }
            }
            _ => {
                let freq = midi_to_freq(item.default_note);
                let _ = self.engine.send_command(AudioCommand::NoteOn { note: item.default_note, freq, velocity: 0.9 });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn import_custom_sample(
        &mut self,
        name: String,
        category: String,
        icon: String,
        default_note: u8,
        color: Color32,
        freq: f32,
        decay: f32,
    ) {
        let id = self.sample_library.len() + 1;
        let wave: Vec<f32> = (0..32).map(|i| {
            let t = i as f32 / 32.0;
            let env = (-t * decay * 4.0).exp();
            (t * freq).sin() * env
        }).collect();
        let item = LibrarySampleItem {
            id,
            name: name.clone(),
            category,
            icon,
            default_note,
            color,
            waveform: wave,
            file_path: None,
        };
        self.sample_library.push(item);
        self.status_message = crate::tstatus!("Importerade '{}' till ljudbiblioteket!", name);
    }

    pub fn toggle_timeline_recording(&mut self) {
        if self.is_recording_timeline {
            let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
            let take_result = self.vocal_studio.stop_recording();
            self.is_recording_timeline = false;
            self.is_playing = false;
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));

            if let Err(e) = &take_result {
                self.status_message = format!("❌ {}", e);
            }
            if let Ok(take_idx) = take_result {
                if let Some(take) = self.vocal_studio.takes.get(take_idx).cloned() {
                    let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
                    let start_bar = self.timeline_rec_start_bar;
                    let length_bars =
                (tempo.bars_for_secs_at(start_bar as f64, take.duration_secs as f64) as f32)
                    .max(0.25);
                    let mic_sr = take.sample_rate;

                    let pcm_arc = std::sync::Arc::new(take.pcm_samples);
                    self.playlist_tracks[armed_idx].pcm_audio = Some((pcm_arc.clone(), pcm_arc, mic_sr));

                    let region = AudioRegion {
                        id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize,
                        name: format!("🎤 {}", take.name),
                        start_bar,
                        length_bars,
                        sample_offset_sec: 0.0,
                        source_path: None,
                        waveform_peaks: take.waveform_data,
                        volume: 1.0,
                        fade_in_bars: 0.0,
                        fade_out_bars: 0.0,
                        muted: false,
                        is_reverse: false,
                        color: self.playlist_tracks[armed_idx].color,
                        loop_length_bars: 0.0,
                    };
                    self.playlist_tracks[armed_idx].regions.push(region);
                    self.sync_track_stem_to_engine(armed_idx);
                    self.status_message = crate::tstatus!("✔ Spelade in '{}' direkt i spår {} ({}) vid takt {:.1} ({:.2}s)!", take.name, armed_idx + 1, self.playlist_tracks[armed_idx].name, start_bar + 1.0, take.duration_secs);
                }
            }
        } else {
            if !self.playlist_tracks.iter().any(|t| t.is_rec_armed) && !self.playlist_tracks.is_empty() {
                self.playlist_tracks[0].is_rec_armed = true;
            }
            let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
            let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
            self.timeline_rec_start_bar = tempo.bar_at_secs(self.song_time as f64) as f32;
            self.is_recording_timeline = true;
            if let Err(e) = self.vocal_studio.start_recording() {
                self.status_message = format!("❌ {}", e);
                self.is_recording_timeline = false;
                return;
            }
            self.is_playing = true;
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
            self.status_message = crate::tstatus!("🔴 Spelar in direkt i spår '{}' från takt {:.1}...", self.playlist_tracks[armed_idx].name, self.timeline_rec_start_bar + 1.0);
        }
    }

    pub fn toggle_playback(&mut self) {
        if self.is_recording_timeline {
            self.toggle_timeline_recording();
            return;
        }
        self.is_playing = !self.is_playing;
        if self.is_playing {
            ui_dbg(&format!("toggle_playback -> PLAY song_bar={} step={} pattern_mode={}", self.song_bar, self.song_step_in_bar, self.pattern_mode));
            for t in &mut self.playlist_tracks {
                t.automation_last = [f32::NAN; 4];
            }
            self.sync_all_stems_to_engine();
            let song_secs = if self.pattern_mode {
                0.0
            } else {
                crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_at_bar(
                self.song_bar as f64 + self.song_step_in_bar as f64 / 16.0,
            ) as f32
            };
            self.song_time = song_secs;
            let _ = self.engine.send_command(AudioCommand::SeekSongPosition(song_secs));
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
        } else {
            ui_dbg("toggle_playback -> STOP (pause)");
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
            let _ = self.engine.send_command(AudioCommand::StopAll);
            self.active_keys.clear();
            self.active_mouse_note = None;
        }
    }

    pub fn stop_playback(&mut self) {
        if self.is_recording_timeline {
            self.toggle_timeline_recording();
            return;
        }
        ui_dbg("stop_playback");
        self.is_playing = false;
        self.current_step = 0;
        self.song_bar = 0;
        self.song_step_in_bar = 0;
        self.song_time = 0.0;
        let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(0.0));
        let _ = self.engine.send_command(AudioCommand::StopAll);
        self.active_keys.clear();
        self.active_mouse_note = None;
    }

    #[allow(dead_code)]
    pub fn seek_song_bar(&mut self, bar: usize) {
        self.song_bar = bar;
        self.song_step_in_bar = 0;
        self.song_time =
            crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_at_bar(bar as f64)
                as f32;
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(self.song_time));
        self.status_message = crate::tstatus!("Flyttade markör till Takt {}", bar + 1);
    }

    pub fn seek_song_time(&mut self, time_secs: f32) {
        let clamped = time_secs.max(0.0);
        self.song_time = clamped;
        let bar_float = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
            .bar_at_secs(clamped as f64) as f32;
        self.song_bar = bar_float.floor() as usize;
        let rem_bar = (bar_float - self.song_bar as f32).max(0.0);
        self.song_step_in_bar = ((rem_bar * 16.0).floor() as usize).min(15);
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(clamped));
        self.status_message = crate::tstatus!("Flyttade markör till {}", format_time_hundredths(clamped));
    }

    pub fn sync_track_audio_state(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            let t = &self.playlist_tracks[track_idx];
            let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
                track_index: track_idx,
                volume: t.volume,
                pan: t.pan,
                muted: t.muted,
                solo: t.solo,
            });
            let _ = self.engine.send_command(AudioCommand::SetStemTrackRouting {
                track_index: track_idx,
                bus: t.bus,
                vca: t.vca,
            });
            let _ = self.engine.send_command(AudioCommand::SetTrackEq {
                track_index: track_idx,
                settings: t.eq.to_settings(),
            });
            let _ = self.engine.send_command(AudioCommand::SetTrackMix {
                track_index: track_idx,
                comp_threshold_db: t.comp_threshold_db,
                comp_ratio: t.comp_ratio,
                reverb_send: t.reverb_send,
                delay_send: t.delay_send,
                pitch_semitones: t.pitch_semitones,
            });
        }
    }

    /// Pushes the sub-mix bus and VCA group state (gain/mute/solo) to the audio
    /// engine (Fas 5.2). Cheap; safe to call every UI frame or on any change.
    pub fn sync_group_state(&mut self) {
        for bus in 0..self.bus_volume.len() {
            let _ = self.engine.send_command(AudioCommand::SetBusState {
                bus,
                volume: self.bus_volume[bus],
                muted: self.bus_muted[bus],
                solo: self.bus_solo[bus],
            });
        }
        for vca in 0..self.vca_faders.len() {
            let _ = self.engine.send_command(AudioCommand::SetVcaState {
                vca,
                volume: self.vca_faders[vca],
                muted: self.vca_muted[vca],
                solo: self.vca_solos[vca],
            });
        }
    }

    pub fn sync_track_stem_to_engine(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            let t = &self.playlist_tracks[track_idx];
            // Ett fruset spår har sitt ljud i frozen_pcm och sin region ur
            // bufferten. Utan det här skulle den här funktionen — som körs vid
            // varje uppspelningsstart — skicka en TOM regionlista för spåret och
            // tysta frysningen.
            let frozen_pcm = t.frozen_pcm.clone();
            if let Some((l, r, sr)) = frozen_pcm.as_ref().or(t.pcm_audio.as_ref()) {
                let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                    track_index: track_idx,
                    left: l.clone(),
                    right: r.clone(),
                    sample_rate: *sr as f32,
                    volume: t.volume,
                    pan: t.pan,
                    start_time_secs: 0.0,
                });
            }
            let stem_regions = self.stem_regions_for(t);
            let _ = self.engine.send_command(AudioCommand::SetStemTrackRegions {
                track_index: track_idx,
                regions: stem_regions,
            });
            let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
                track_index: track_idx,
                volume: t.volume,
                pan: t.pan,
                muted: t.muted,
                solo: t.solo,
            });
            let _ = self.engine.send_command(AudioCommand::SetStemTrackRouting {
                track_index: track_idx,
                bus: t.bus,
                vca: t.vca,
            });
            let _ = self.engine.send_command(AudioCommand::SetTrackEq {
                track_index: track_idx,
                settings: t.eq.to_settings(),
            });
            let _ = self.engine.send_command(AudioCommand::SetTrackMix {
                track_index: track_idx,
                comp_threshold_db: t.comp_threshold_db,
                comp_ratio: t.comp_ratio,
                reverb_send: t.reverb_send,
                delay_send: t.delay_send,
                pitch_semitones: t.pitch_semitones,
            });
        }
    }

    pub fn sync_all_stems_to_engine(&mut self) {
        for idx in 0..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(idx);
        }
    }

    /// Evaluates every enabled automation lane for all tracks at the current
    /// song position and forwards changed values to the audio engine. Values
    /// are cached per parameter so we only emit commands on real changes.
    pub fn apply_automation(&mut self) {
        if !self.is_playing {
            return;
        }
        let t = self.song_time;
        for ti in 0..self.playlist_tracks.len() {
            let mut target = [f32::NAN; 4];
            {
                let track = &self.playlist_tracks[ti];
                for lane in &track.automation {
                    if !lane.enabled {
                        continue;
                    }
                    if let Some(v) = lane.value_at(t) {
                        let (lo, hi) = lane.param.range();
                        target[lane.param.index()] = v.clamp(lo, hi);
                    }
                }
            }
            let mut state_cmd = None;
            let mut mix_cmd = None;
            {
                let track = &mut self.playlist_tracks[ti];
                let set = |cache: &mut f32, v: f32| -> bool {
                    if v.is_finite() && (v - *cache).abs() > 1e-4 {
                        *cache = v;
                        true
                    } else {
                        false
                    }
                };
                let mut state_changed = false;
                let mut mix_changed = false;
                if set(&mut track.automation_last[0], target[0]) {
                    track.volume = target[0];
                    state_changed = true;
                }
                if set(&mut track.automation_last[1], target[1]) {
                    track.pan = target[1];
                    state_changed = true;
                }
                if set(&mut track.automation_last[2], target[2]) {
                    track.reverb_send = target[2];
                    mix_changed = true;
                }
                if set(&mut track.automation_last[3], target[3]) {
                    track.delay_send = target[3];
                    mix_changed = true;
                }
                if state_changed {
                    state_cmd = Some((track.volume, track.pan, track.muted, track.solo));
                }
                if mix_changed {
                    mix_cmd = Some((
                        track.comp_threshold_db,
                        track.comp_ratio,
                        track.reverb_send,
                        track.delay_send,
                        track.pitch_semitones,
                    ));
                }
            }
            if let Some((volume, pan, muted, solo)) = state_cmd {
                let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
                    track_index: ti,
                    volume,
                    pan,
                    muted,
                    solo,
                });
            }
            if let Some((comp_threshold_db, comp_ratio, reverb_send, delay_send, pitch_semitones)) =
                mix_cmd
            {
                let _ = self.engine.send_command(AudioCommand::SetTrackMix {
                    track_index: ti,
                    comp_threshold_db,
                    comp_ratio,
                    reverb_send,
                    delay_send,
                    pitch_semitones,
                });
            }
        }
    }

    fn render_automation_lane(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        resp: &egui::Response,
        bar_w: f32,
        sec_per_bar: f32,
        track_idx: usize,
    ) {
        let param = self.automation_param;
        let (lo, hi) = param.range();
        let span = (hi - lo).max(1e-6);
        let val_to_y = |v: f32| rect.max.y - ((v - lo) / span).clamp(0.0, 1.0) * rect.height();
        let y_to_val = |y: f32| lo + ((rect.max.y - y) / rect.height()).clamp(0.0, 1.0) * span;
        let sec_to_x = |s: f32| rect.min.x + s / sec_per_bar * bar_w;
        let x_to_sec = |x: f32| ((x - rect.min.x) / bar_w * sec_per_bar).max(0.0);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(11, 14, 19));
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 10.0),
            egui::Align2::LEFT_CENTER,
            crate::tstatus!("📈 {} — {}", param.label(), self.playlist_tracks[track_idx].name),
            egui::FontId::proportional(10.5),
            Theme::FL_CYAN,
        );
        painter.line_segment(
            [
                Pos2::new(rect.min.x, val_to_y((lo + hi) * 0.5)),
                Pos2::new(rect.max.x, val_to_y((lo + hi) * 0.5)),
            ],
            Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 48)),
        );
        let total_bars = (rect.width() / bar_w).ceil() as usize;
        for b in 0..=total_bars {
            let x = rect.min.x + b as f32 * bar_w;
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(0.5_f32, Color32::from_rgb(26, 30, 40)),
            );
        }

        let lane_idx = self.playlist_tracks[track_idx]
            .automation
            .iter()
            .position(|l| l.param == param);

        if let Some(li) = lane_idx {
            let lane = &self.playlist_tracks[track_idx].automation[li];
            if lane.enabled && !lane.points.is_empty() {
                let first = &lane.points[0];
                let last = lane.points.last().unwrap();
                painter.line_segment(
                    [
                        Pos2::new(rect.min.x, val_to_y(first.value)),
                        Pos2::new(sec_to_x(first.time_secs), val_to_y(first.value)),
                    ],
                    Stroke::new(2.0_f32, Theme::FL_CYAN),
                );
                for w in lane.points.windows(2) {
                    painter.line_segment(
                        [
                            Pos2::new(sec_to_x(w[0].time_secs), val_to_y(w[0].value)),
                            Pos2::new(sec_to_x(w[1].time_secs), val_to_y(w[1].value)),
                        ],
                        Stroke::new(2.0_f32, Theme::FL_CYAN),
                    );
                }
                painter.line_segment(
                    [
                        Pos2::new(sec_to_x(last.time_secs), val_to_y(last.value)),
                        Pos2::new(rect.max.x, val_to_y(last.value)),
                    ],
                    Stroke::new(2.0_f32, Theme::FL_CYAN),
                );
                for (pi, p) in lane.points.iter().enumerate() {
                    let c = Pos2::new(sec_to_x(p.time_secs), val_to_y(p.value));
                    let dragging = self.automation_drag == Some((track_idx, li, pi));
                    painter.circle_filled(
                        c,
                        if dragging { 6.0 } else { 4.5 },
                        if dragging { Theme::FL_ORANGE } else { Theme::FL_CYAN },
                    );
                    painter.circle_stroke(c, 5.5, Stroke::new(1.0_f32, Color32::BLACK));
                }
            }
        }

        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let sec = snap_time_secs(self.timeline_snap_mode, x_to_sec(pos.x), sec_per_bar);
                let val = y_to_val(pos.y).clamp(lo, hi);
                let li = match lane_idx {
                    Some(li) => li,
                    None => {
                        self.playlist_tracks[track_idx].automation.push(AutomationLane {
                            param,
                            enabled: true,
                            points: Vec::new(),
                        });
                        self.playlist_tracks[track_idx].automation.len() - 1
                    }
                };
                let lane = &mut self.playlist_tracks[track_idx].automation[li];
                lane.points.push(AutomationPoint { time_secs: sec, value: val });
                lane.sort_points();
                self.status_message = crate::tstatus!("📈 Automation: la till punkt ({:.2}s, {:.2})", sec, val);
            }
        }

        if resp.drag_started() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(li) = lane_idx {
                    let pts = &self.playlist_tracks[track_idx].automation[li].points;
                    let mut best = None;
                    let mut best_d = 14.0_f32;
                    for (pi, p) in pts.iter().enumerate() {
                        let c = Pos2::new(sec_to_x(p.time_secs), val_to_y(p.value));
                        let d = c.distance(pos);
                        if d < best_d {
                            best_d = d;
                            best = Some(pi);
                        }
                    }
                    if let Some(pi) = best {
                        self.automation_drag = Some((track_idx, li, pi));
                    }
                }
            }
        }
        if resp.dragged() {
            if let Some((t, li, pi)) = self.automation_drag
                && t == track_idx
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let sec = snap_time_secs(self.timeline_snap_mode, x_to_sec(pos.x), sec_per_bar);
                let val = y_to_val(pos.y).clamp(lo, hi);
                if let Some(p) = self.playlist_tracks[track_idx]
                    .automation
                    .get_mut(li)
                    .and_then(|l| l.points.get_mut(pi))
                {
                    p.time_secs = sec;
                    p.value = val;
                }
            }
        }
        if resp.drag_stopped() {
            if let Some((t, li, _)) = self.automation_drag {
                if t == track_idx
                    && let Some(lane) = self.playlist_tracks[track_idx].automation.get_mut(li)
                {
                    lane.sort_points();
                }
            }
            self.automation_drag = None;
        }

        if resp.secondary_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if let Some(li) = lane_idx {
                    let pts = &self.playlist_tracks[track_idx].automation[li].points;
                    let mut best = None;
                    let mut best_d = 14.0_f32;
                    for (pi, p) in pts.iter().enumerate() {
                        let c = Pos2::new(sec_to_x(p.time_secs), val_to_y(p.value));
                        let d = c.distance(pos);
                        if d < best_d {
                            best_d = d;
                            best = Some(pi);
                        }
                    }
                    if let Some(pi) = best {
                        self.playlist_tracks[track_idx].automation[li].points.remove(pi);
                        self.status_message = crate::tstatus!("📈 Automation: tog bort punkt");
                    }
                }
            }
        }
    }

    /// Decodes the chosen file and runs stem separation on a background thread.
    /// Uses the neural HTDemucs backend when a model is installed, otherwise the
    /// built-in DSP separator.
    pub fn start_stem_separation(&mut self, path: &str) {
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
                self.stem_project.install_separation(result, &source);
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
            }
            Err(err) => {
                self.stem_project.is_separating = false;
                self.stem_project.progress = 0.0;
                self.status_message = crate::tstatus!("⚠ Separation misslyckades: {}", err);
            }
        }
    }

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

    /// Re-render one stem through the pitch-preserving time-stretch and reload
    /// it into the engine. Runs from the original (un-stretched) audio so the
    /// SPEED knob can be adjusted repeatedly without compounding artefacts.
    pub fn apply_stem_time_stretch(&mut self, idx: usize) {
        let sr = self.stem_project.sample_rate as f32;
        if idx >= self.stem_project.stems.len()
            || idx >= self.stem_project.stem_audio.len()
            || idx >= self.stem_project.stem_audio_base.len()
        {
            return;
        }
        let ratio = self.stem_project.stems[idx].time_stretch;
        let base = &self.stem_project.stem_audio_base[idx];
        let left = crate::audio::vocal_harmonizer::time_stretch(&base.left, sr, ratio);
        let right = crate::audio::vocal_harmonizer::time_stretch(&base.right, sr, ratio);
        self.stem_project.stem_audio[idx] = crate::audio::stem_separator::StemAudio { left, right };

        let audio = self.stem_project.stem_audio[idx].clone();
        let ch = &self.stem_project.stems[idx];
        let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
            track_index: idx,
            left: std::sync::Arc::new(audio.left),
            right: std::sync::Arc::new(audio.right),
            sample_rate: sr,
            volume: ch.volume,
            pan: ch.pan,
            start_time_secs: 0.0,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
            track_index: idx,
            volume: ch.volume,
            pan: ch.pan,
            muted: ch.muted,
            solo: ch.solo,
        });
    }

    /// Instantiates a CLAP plugin and sets it as the insert on a stem track
    /// (Fas 4.2). Runs on the UI thread; the live processor is moved to the
    /// audio thread through the command ring.
    pub fn load_plugin_into_track(&mut self, path: &str, track_index: usize) {
        let sample_rate = self.engine.sample_rate as f32;
        let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
        match crate::audio::plugin_host_live::load_processor(path, sample_rate, block as u32) {
            Ok(processor) => {
                let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
                let name = insert.info().name.clone();
                // Capture the fresh state now, on the main thread (clap.state is
                // main-thread only), so a project save can restore it.
                let state = insert.save_state();
                let handle = insert.core_handle();
                self.retire_plugin_handle(track_index);
                self.record_plugin_slot(
                    track_index,
                    PluginSlot {
                        path: path.to_string(),
                        name: name.clone(),
                        state,
                        sandboxed: false,
                    },
                );
                self.ensure_plugin_vecs(track_index);
                self.plugin_handles[track_index] = handle;
                let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                    track_index,
                    insert: Some(insert),
                });
                self.plugin_manager.instantiated_plugin = Some(name.clone());
                self.status_message = crate::tstatus!(
                    "✔ {} laddad som insert på stämspår {} (PDC-kompenserad)",
                    name,
                    track_index + 1
                );
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte ladda plugin: {}", e);
            }
        }
    }

    /// Instantiates `path` into `track_index` with a native preset applied via
    /// `clap.preset-load/2`, then captures the resulting state for the project.
    pub fn load_plugin_preset_into_track(&mut self, path: &str, track_index: usize, location: &str) {
        let sample_rate = self.engine.sample_rate as f32;
        let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
        match crate::audio::plugin_host_live::load_processor(path, sample_rate, block as u32) {
            Ok(processor) => {
                let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
                let name = insert.info().name.clone();
                if !insert.preset_load(location) {
                    self.status_message = crate::tstatus!(
                        "⚠ Pluginen '{}' stödjer inte clap.preset-load/2 (eller avvisade preseten)",
                        name
                    );
                    return;
                }
                let state = insert.save_state();
                let handle = insert.core_handle();
                self.retire_plugin_handle(track_index);
                self.record_plugin_slot(
                    track_index,
                    PluginSlot {
                        path: path.to_string(),
                        name: name.clone(),
                        state,
                        sandboxed: false,
                    },
                );
                self.ensure_plugin_vecs(track_index);
                self.plugin_handles[track_index] = handle;
                let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                    track_index,
                    insert: Some(insert),
                });
                self.plugin_manager.instantiated_plugin = Some(name.clone());
                self.status_message = crate::tstatus!(
                    "✔ {} laddad med preset på stämspår {} (PDC-kompenserad)",
                    name,
                    track_index + 1
                );
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte ladda plugin: {}", e);
            }
        }
    }

    /// Removes the plugin insert on `track_index` and forgets its saved slot.
    pub fn remove_plugin_from_track(&mut self, track_index: usize) {
        self.retire_plugin_handle(track_index);
        #[cfg(feature = "plugin-host")]
        self.remove_plugin_sandbox(track_index);
        let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
            track_index,
            insert: None,
        });
        if let Some(slot) = self.plugin_slots.get_mut(track_index) {
            *slot = None;
        }
        self.plugin_manager.instantiated_plugin = None;
        self.status_message = crate::tstatus!(
            "🗑 Tog bort plugin från stämspår {}",
            track_index + 1
        );
    }

    /// Instantiates `path` in an out-of-process sandbox and streams its audio
    /// over shared memory (Fas 4.5b). The plugin's GUI is not available for
    /// sandboxed instances.
    #[cfg(feature = "plugin-host")]
    pub fn load_plugin_into_sandbox_track(&mut self, path: &str, track_index: usize) {
        use crate::audio::plugin_sandbox::{
            SandboxHost, SandboxProcessor, SandboxRequest, SandboxResponse, info_from_dto,
            param_from_dto,
        };
        use crate::audio::sandbox_audio::{AudioBridge, DEFAULT_SLOTS};

        let sample_rate = self.engine.sample_rate as f32;
        let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;

        // Replace any previous plugin (in-process or sandboxed) on this track.
        self.retire_plugin_handle(track_index);
        self.remove_plugin_sandbox(track_index);

        let bridge = match AudioBridge::create(block, DEFAULT_SLOTS) {
            Ok(bridge) => bridge,
            Err(err) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte skapa ljudbryggan: {}", err);
                return;
            }
        };
        let mut host = match SandboxHost::new_audio(path, sample_rate, block as u32, bridge.raw_fd())
        {
            Ok(host) => host,
            Err(err) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte skapa sandbox: {}", err);
                return;
            }
        };
        let outcome = (|| -> Result<(crate::audio::plugin_host_live::PluginInfo, Vec<crate::audio::plugin_host_live::PluginParameter>, u32, Vec<u8>), String> {
            host.spawn().map_err(|e| e.to_string())?;
            let info = match host.request(&SandboxRequest::Info).map_err(|e| e.to_string())? {
                SandboxResponse::Info { info } => info_from_dto(&info),
                SandboxResponse::Error { message } => return Err(message),
                other => return Err(format!("oväntat svar: {other:?}")),
            };
            let parameters = match host
                .request(&SandboxRequest::Parameters)
                .map_err(|e| e.to_string())?
            {
                SandboxResponse::Parameters { parameters } => {
                    parameters.iter().map(param_from_dto).collect()
                }
                _ => Vec::new(),
            };
            let latency = match host
                .request(&SandboxRequest::Latency)
                .map_err(|e| e.to_string())?
            {
                SandboxResponse::Latency { frames } => frames,
                _ => 0,
            };
            let state = match host
                .request(&SandboxRequest::SaveState)
                .map_err(|e| e.to_string())?
            {
                SandboxResponse::State { data } => data,
                _ => Vec::new(),
            };
            Ok((info, parameters, latency, state))
        })();

        match outcome {
            Ok((info, parameters, latency, state)) => {
                let name = info.name.clone();
                let processor = SandboxProcessor::new(bridge, info, parameters, latency);
                let insert =
                    crate::audio::plugin_host_live::PluginInsert::new(Box::new(processor), block);
                self.record_plugin_slot(
                    track_index,
                    PluginSlot {
                        path: path.to_string(),
                        name: name.clone(),
                        state,
                        sandboxed: true,
                    },
                );
                self.ensure_plugin_sandboxes(track_index);
                self.plugin_sandboxes[track_index] = Some(host);
                let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                    track_index,
                    insert: Some(insert),
                });
                self.plugin_manager.instantiated_plugin = Some(name.clone());
                self.status_message = crate::tstatus!(
                    "🧪 {} laddad i sandbox på stämspår {} (separat process, PDC-kompenserad)",
                    name,
                    track_index + 1
                );
            }
            Err(err) => {
                self.status_message = crate::tstatus!("⚠ Sandbox misslyckades: {}", err);
                host.shutdown();
            }
        }
    }

    #[cfg(feature = "plugin-host")]
    fn ensure_plugin_sandboxes(&mut self, track_index: usize) {
        if self.plugin_sandboxes.len() <= track_index {
            self.plugin_sandboxes.resize_with(track_index + 1, || None);
        }
    }

    #[cfg(feature = "plugin-host")]
    fn remove_plugin_sandbox(&mut self, track_index: usize) {
        if let Some(mut host) = self
            .plugin_sandboxes
            .get_mut(track_index)
            .and_then(|slot| slot.take())
        {
            host.shutdown();
        }
    }

    fn ensure_plugin_vecs(&mut self, track_index: usize) {
        if self.plugin_handles.len() <= track_index {
            self.plugin_handles.resize_with(track_index + 1, || None);
        }
        if self.plugin_gui_sessions.len() <= track_index {
            self.plugin_gui_sessions.resize_with(track_index + 1, || None);
        }
    }

    /// Closes the GUI and moves the track's shared handle to the retirement
    /// list so the instance is not destroyed on the audio thread.
    fn retire_plugin_handle(&mut self, track_index: usize) {
        if let Some(slot) = self.plugin_gui_sessions.get_mut(track_index) {
            *slot = None;
        }
        if let Some(handle) = self.plugin_handles.get_mut(track_index).and_then(|h| h.take()) {
            self.retired_plugin_handles.push(handle);
        }
    }

    fn close_all_plugin_guis(&mut self) {
        for slot in self.plugin_gui_sessions.iter_mut() {
            *slot = None;
        }
    }

    fn retire_all_plugin_handles(&mut self) {
        for slot in self.plugin_handles.iter_mut() {
            if let Some(handle) = slot.take() {
                self.retired_plugin_handles.push(handle);
            }
        }
    }

    /// Whether a plugin GUI is currently open for `track_index`.
    pub fn is_plugin_gui_open(&self, track_index: usize) -> bool {
        self.plugin_gui_sessions
            .get(track_index)
            .and_then(|s| s.as_ref())
            .map(|s| s.is_alive())
            .unwrap_or(false)
    }

    /// Opens the plugin editor for `track_index` in a real X11 window, sharing
    /// the instance that is processing audio. Main-thread only.
    pub fn open_plugin_gui(&mut self, track_index: usize) {
        self.ensure_plugin_vecs(track_index);
        if self.plugin_gui_sessions[track_index].is_some() {
            return;
        }
        let Some(handle) = self.plugin_handles.get(track_index).and_then(|h| h.clone()) else {
            self.status_message =
                crate::tstatus!("⚠ Ingen aktiv plugin på stämspår {}", track_index + 1);
            return;
        };
        let name = handle.info().name.clone();
        let title = format!("{} – Sonix", name);
        match crate::audio::plugin_gui::GuiSession::open(handle, &title) {
            Ok(session) => {
                let size = session.size();
                self.plugin_gui_sessions[track_index] = Some(session);
                self.status_message = match size {
                    Some((width, height)) => crate::tstatus!(
                        "🪟 Öppnade plugin-GUI för '{}' ({}×{})",
                        name,
                        width,
                        height
                    ),
                    None => crate::tstatus!("🪟 Öppnade plugin-GUI för '{}'", name),
                };
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte öppna plugin-GUI: {}", e);
            }
        }
    }

    /// Closes the plugin editor for `track_index`, if open.
    pub fn close_plugin_gui(&mut self, track_index: usize) {
        if let Some(slot) = self.plugin_gui_sessions.get_mut(track_index)
            && slot.is_some()
        {
            *slot = None;
            self.status_message = crate::i18n::t("🪟 Stängde plugin-GUI").to_string();
        }
    }

    /// Pumps X11 events for every open plugin window and drops sessions whose
    /// window was closed by the user. Call once per UI frame.
    pub fn poll_plugin_guis(&mut self) {
        let mut closed = Vec::new();
        for (index, slot) in self.plugin_gui_sessions.iter_mut().enumerate() {
            if let Some(session) = slot
                && !session.poll()
            {
                closed.push(index);
            }
        }
        for index in closed {
            let title = self.plugin_gui_sessions[index]
                .as_ref()
                .map(|s| s.title().to_string());
            self.plugin_gui_sessions[index] = None;
            self.status_message = match title {
                Some(title) => crate::tstatus!("🪟 Plugin-GUI stängt ({})", title),
                None => crate::i18n::t("🪟 Plugin-GUI stängt").to_string(),
            };
        }
    }

    /// Spawns a sandbox worker for `path` and reads its info + parameters over
    /// the process boundary (Fas 4.5a). Replaces any previous worker.
    #[cfg(feature = "plugin-host")]
    pub fn sandbox_inspect(&mut self, path: &str) {
        use crate::audio::plugin_sandbox::{
            SandboxHost, SandboxInspection, SandboxRequest, SandboxResponse,
        };
        if let Some(mut previous) = self.plugin_sandbox.take() {
            previous.shutdown();
        }
        let mut host = match SandboxHost::new(
            path,
            self.engine.sample_rate as f32,
            crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES as u32,
        ) {
            Ok(host) => host,
            Err(err) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte skapa sandbox: {}", err);
                return;
            }
        };
        let outcome = (|| -> Result<SandboxInspection, String> {
            host.spawn().map_err(|e| e.to_string())?;
            let info = match host.request(&SandboxRequest::Info).map_err(|e| e.to_string())? {
                SandboxResponse::Info { info } => Some(info),
                SandboxResponse::Error { message } => return Err(message),
                other => return Err(format!("oväntat svar: {other:?}")),
            };
            let parameters = match host
                .request(&SandboxRequest::Parameters)
                .map_err(|e| e.to_string())?
            {
                SandboxResponse::Parameters { parameters } => parameters,
                _ => Vec::new(),
            };
            Ok(SandboxInspection {
                path: path.to_string(),
                info,
                parameters,
                restarts: 0,
                error: None,
            })
        })();
        match outcome {
            Ok(inspection) => {
                let name = inspection
                    .info
                    .as_ref()
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| path.to_string());
                self.status_message = crate::tstatus!(
                    "🧪 Sandbox: läste {} parametrar ur '{}' i en separat process",
                    inspection.parameters.len(),
                    name
                );
                self.sandbox_inspection = Some(inspection);
                self.plugin_sandbox = Some(host);
            }
            Err(err) => {
                self.status_message = crate::tstatus!("⚠ Sandbox misslyckades: {}", err);
                self.sandbox_inspection = Some(SandboxInspection {
                    path: path.to_string(),
                    info: None,
                    parameters: Vec::new(),
                    restarts: 0,
                    error: Some(err),
                });
                host.shutdown();
            }
        }
    }

    /// Health-checks the sandbox worker each frame and reports crashes (Fas 4.5a).
    #[cfg(feature = "plugin-host")]
    pub fn poll_plugin_sandbox(&mut self) {
        use crate::audio::plugin_sandbox::SandboxState;
        let state = match self.plugin_sandbox.as_mut() {
            Some(host) => host.poll(),
            None => return,
        };
        match state {
            SandboxState::Restarted => {
                let restarts = self
                    .plugin_sandbox
                    .as_ref()
                    .map(|h| h.restarts())
                    .unwrap_or(0);
                if let Some(inspection) = self.sandbox_inspection.as_mut() {
                    inspection.restarts = restarts;
                }
                self.status_message = crate::tstatus!(
                    "🧪 Sandbox: plugin-processen kraschade och startades om (omstart {})",
                    restarts
                );
            }
            SandboxState::Crashed => {
                let restarts = self
                    .plugin_sandbox
                    .as_ref()
                    .map(|h| h.restarts())
                    .unwrap_or(0);
                if let Some(mut host) = self.plugin_sandbox.take() {
                    host.shutdown();
                }
                if let Some(inspection) = self.sandbox_inspection.as_mut() {
                    inspection.restarts = restarts;
                    inspection.error =
                        Some(crate::i18n::t("sandbox-processen kraschade upprepade gånger").to_string());
                }
                self.status_message = crate::i18n::t(
                    "⚠ Sandbox: plugin-processen kraschade upprepade gånger",
                )
                .to_string();
            }
            _ => {}
        }

        // Supervise the per-track audio sandboxes (Fas 4.5b).
        for index in 0..self.plugin_sandboxes.len() {
            let state = match self.plugin_sandboxes[index].as_mut() {
                Some(host) => host.poll(),
                None => continue,
            };
            match state {
                SandboxState::Restarted => {
                    let restarts = self.plugin_sandboxes[index]
                        .as_ref()
                        .map(|h| h.restarts())
                        .unwrap_or(0);
                    self.status_message = crate::tstatus!(
                        "🧪 Sandbox på stämspår {}: plugin-processen startades om (omstart {})",
                        index + 1,
                        restarts
                    );
                }
                SandboxState::Crashed => {
                    if let Some(mut host) = self.plugin_sandboxes[index].take() {
                        host.shutdown();
                    }
                    let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                        track_index: index,
                        insert: None,
                    });
                    if let Some(slot) = self.plugin_slots.get_mut(index) {
                        *slot = None;
                    }
                    self.status_message = crate::tstatus!(
                        "⚠ Sandbox på stämspår {} kraschade upprepade gånger och togs bort",
                        index + 1
                    );
                }
                _ => {}
            }
        }
    }

    /// Formats the sandbox inspection for the plugin-manager view.
    #[cfg(feature = "plugin-host")]
    pub fn sandbox_status_text(&self) -> Option<String> {
        let inspection = self.sandbox_inspection.as_ref()?;
        Some(match &inspection.info {
            Some(info) => crate::tstatus!(
                "🧪 Sandbox (separat process): {} v{} – {} parametrar, {} omstarter",
                info.name,
                info.version,
                inspection.parameters.len(),
                inspection.restarts
            ),
            None => crate::tstatus!(
                "🧪 Sandbox misslyckades för '{}': {}",
                inspection.path,
                inspection.error.as_deref().unwrap_or("okänt fel")
            ),
        })
    }

    fn record_plugin_slot(&mut self, track_index: usize, slot: PluginSlot) {
        if self.plugin_slots.len() <= track_index {
            self.plugin_slots.resize_with(track_index + 1, || None);
        }
        self.plugin_slots[track_index] = Some(slot);
    }

    /// Re-instantiates a saved plugin on `track_index` and restores its state.
    /// Must run on the main thread. Returns an error message on failure.
    fn restore_plugin_slot(&mut self, track_index: usize, data: &SavedPluginData) -> Option<String> {
        if data.sandboxed {
            #[cfg(feature = "plugin-host")]
            {
                use crate::audio::plugin_sandbox::{SandboxRequest, SandboxResponse};
                self.load_plugin_into_sandbox_track(&data.path, track_index);
                let host = self
                    .plugin_sandboxes
                    .get_mut(track_index)
                    .and_then(|slot| slot.as_mut());
                let Some(host) = host else {
                    return Some(crate::tstatus!(
                        "kunde inte återställa sandboxad plugin '{}'",
                        data.name
                    ));
                };
                if !data.state.is_empty()
                    && !matches!(
                        host.request(&SandboxRequest::LoadState {
                            data: data.state.clone(),
                        }),
                        Ok(SandboxResponse::Ok)
                    )
                {
                    return Some(crate::tstatus!(
                        "kunde inte återställa state för sandboxad plugin '{}'",
                        data.name
                    ));
                }
                if let Some(slot) = self.plugin_slots.get_mut(track_index).and_then(|s| s.as_mut()) {
                    slot.state = data.state.clone();
                }
                return None;
            }
            #[cfg(not(feature = "plugin-host"))]
            return Some(crate::i18n::t(
                "sandboxade plugins kräver att Sonix byggs med --features plugin-host",
            )
            .to_string());
        }
        let sample_rate = self.engine.sample_rate as f32;
        let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
        match crate::audio::plugin_host_live::load_processor(&data.path, sample_rate, block as u32) {
            Ok(processor) => {
                let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
                if !data.state.is_empty() && !insert.load_state(&data.state) {
                    return Some(crate::tstatus!(
                        "kunde inte återställa state för '{}'",
                        data.name
                    ));
                }
                let handle = insert.core_handle();
                self.ensure_plugin_vecs(track_index);
                self.plugin_handles[track_index] = handle;
                let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                    track_index,
                    insert: Some(insert),
                });
                None
            }
            Err(e) => Some(e),
        }
    }

    /// Adds the four separated stems as real timeline tracks.
    pub fn export_separated_stems(&mut self) {
        if self.stem_project.stem_audio.is_empty() {
            self.status_message = crate::i18n::t("⚠ Ingen separerad mix att exportera.").to_string();
            return;
        }
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let bars =
            (tempo.bars_for_secs_at(0.0, self.stem_project.duration_seconds as f64) as f32)
                .max(1.0);
        let kinds = [TrackKind::VocalAudio, TrackKind::Drums, TrackKind::Bassline, TrackKind::CustomAudio];
        let mut new_tracks = Vec::new();
        for (i, ch) in self.stem_project.stems.iter().enumerate() {
            let audio = &self.stem_project.stem_audio[i];
            let peaks = ch.waveform_data.clone();
            let region = AudioRegion {
                id: i + 1,
                name: ch.stem_type.name().to_string(),
                start_bar: 0.0,
                length_bars: bars,
                sample_offset_sec: 0.0,
                source_path: self.stem_project.source_path.clone(),
                waveform_peaks: peaks.clone(),
                volume: 1.0,
                fade_in_bars: 0.0,
                fade_out_bars: 0.0,
                muted: false,
                is_reverse: false,
                color: ch.stem_type.color(),
                loop_length_bars: 0.0,
            };
            let mut track = PlaylistTrack::new(ch.stem_type.name().to_string(), "🎚", kinds[i], ch.stem_type.color());
            track.volume = ch.volume;
            track.pan = ch.pan;
            track.muted = ch.muted;
            track.solo = ch.solo;
            track.regions = vec![region];
            track.pcm_audio = Some((
                std::sync::Arc::new(audio.left.clone()),
                std::sync::Arc::new(audio.right.clone()),
                self.stem_project.sample_rate,
            ));
            new_tracks.push(track);
        }
        self.playlist_tracks.extend(new_tracks);
        self.sync_all_stems_to_engine();
        self.status_message = crate::tstatus!("📥 Exporterade 4 stämspår till Song Arranger");
    }

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

    fn step_duration(&self, step: usize) -> std::time::Duration {
        // Stegets längd kommer från tempokartan (Fas 8.2, steg 1). Med ett enda
        // tempo är talet exakt detsamma som förut — det är bevisat i
        // `audio::tempo`-testerna — så klockan är oförändrad tills kartan får
        // fler punkter.
        let map = crate::audio::tempo::TempoMap::single(self.bpm);
        let base_seconds = map.secs_per_step_at(self.song_bar as f64) as f32;
        let swing_factor = if step % 2 == 1 {
            1.0 + self.swing * 0.35
        } else {
            1.0 - self.swing * 0.35
        };
        std::time::Duration::from_secs_f32(base_seconds * swing_factor)
    }

    pub fn get_max_project_bars(&self) -> usize {
        let max_region_end = self.playlist_tracks.iter()
            .flat_map(|t| t.regions.iter())
            .map(|r| r.start_bar + r.length_bars)
            .fold(0.0_f32, |m, b| m.max(b));
        (max_region_end.ceil() as usize).max(self.loop_end_bar).max(32)
    }

    fn advance_sequencer(&mut self) {
        if !self.is_playing {
            return;
        }

        let cur = if self.pattern_mode { self.current_step } else { self.song_step_in_bar };
        let dur = self.step_duration(cur);
        if self.last_step_time.elapsed() >= dur {
            self.last_step_time = Instant::now();
            self.song_time += dur.as_secs_f32();

            if self.pattern_mode {
                self.current_step = (self.current_step + 1) % 16;
                self.trigger_step(self.current_step);
            } else {
                self.song_step_in_bar += 1;
                if self.song_step_in_bar >= 16 {
                    self.song_step_in_bar = 0;
                    self.song_bar += 1;
                    let max_bars = self.get_max_project_bars();
                    if self.song_bar >= self.loop_end_bar || self.song_bar >= max_bars {
                        self.song_bar = self.loop_start_bar;
                    }
                }
                self.current_step = self.song_step_in_bar;
                self.trigger_song_step(self.song_bar, self.song_step_in_bar);
            }

            // Sustain any held MIDI notes across the new step while recording.
            if self.midi_record_armed && !self.midi_held_notes.is_empty() {
                let held: Vec<u8> = self.midi_held_notes.iter().copied().collect();
                for note in held {
                    self.record_midi_note_at_step(note);
                }
            }
        }
    }

    pub fn tap_tempo(&mut self) {
        let now = Instant::now();
        self.tap_tempo_times.retain(|&t| now.duration_since(t).as_secs_f32() < 3.0);
        self.tap_tempo_times.push(now);
        if self.tap_tempo_times.len() >= 2 {
            let n = self.tap_tempo_times.len() - 1;
            let total_time: f32 = (0..n)
                .map(|i| self.tap_tempo_times[i + 1].duration_since(self.tap_tempo_times[i]).as_secs_f32())
                .sum();
            let avg_interval = total_time / n as f32;
            if avg_interval > 0.15 && avg_interval < 2.5 {
                let calculated_bpm = (60.0 / avg_interval).clamp(40.0, 260.0);
                self.bpm = (calculated_bpm * 10.0).round() / 10.0;
                self.status_message = crate::tstatus!("🎯 Tap Tempo: {:.1} BPM ({} tryck)", self.bpm, self.tap_tempo_times.len());
            }
        }
    }

    fn trigger_step(&mut self, step: usize) {
        if self.metronome_enabled && step % 4 == 0 {
            if step == 0 {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeHigh));
            } else {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeLow));
            }
        }

        let has_solo = self.channels.iter().any(|c| c.solo);
        let vel = self.step_velocities[step];
        let grid_active = (0..24).any(|r| self.piano_roll_grid[r][step]);

        for (idx, ch) in self.channels.iter().enumerate() {
            let is_audible = if has_solo { ch.solo } else { !ch.muted };
            if ch.steps[step] && is_audible {
                let note = ch.notes[step];
                if let Some(cmd) = channel_sample_trigger_command(ch, note, vel) {
                    let _ = self.engine.send_command(cmd);
                    continue;
                }
                match idx {
                    0 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
                    1 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
                    2 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
                    3 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed)); }
                    4 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen)); }
                    5 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Crash)); }
                    6 => {
                        if !grid_active {
                            let freq = midi_to_freq(note);
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
                        }
                    }
                    7 => {
                        let freq = midi_to_freq(note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
                    }
                    _ => {}
                }
            }
        }

        if self.drummer_tom_steps[step] {
            let tom = if self.drummer_tom_high { DrumType::TomHigh } else { DrumType::TomLow };
            let _ = self.engine.send_command(AudioCommand::TriggerDrum(tom));
        }

        let chan6_audible = self.channels.get(6)
            .map(|c| if has_solo { c.solo } else { !c.muted })
            .unwrap_or(false);
        // Tagningen (Fas 6.4 steg 2) läses **före** grinden: en not som spelades
        // sent i ett steg har sin ruta på nästa steg (det är `floor(pos)` som är
        // steget den klingar från), och med grinden först skulle en sådan not
        // aldrig spelas — varken härifrån eller från sin egen ruta.
        let take_plan = match self.patterns.get(self.selected_pattern) {
            Some(pat) => {
                let grid = &self.piano_roll_grid;
                crate::midi_take::plan_for_step(&pat.take, step, self.step_samples(step), &|key, slot| {
                    (48..72).contains(&key) && grid[(key - 48) as usize][slot]
                })
            }
            None => crate::midi_take::TakePlan::default(),
        };
        let grid_plays_here = grid_active || !take_plan.play.is_empty();
        if grid_plays_here && chan6_audible {
            for (note, delay, take_vel) in &take_plan.play {
                let freq = midi_to_freq(*note);
                let _ = self.engine.send_command(AudioCommand::NoteOnDelayed {
                    note: *note,
                    freq,
                    velocity: vel * take_vel,
                    delay_samples: *delay,
                });
            }
            for row in 0..24 {
                if self.piano_roll_grid[row][step] {
                    let note = 48 + row as u8;
                    if take_plan.skip.contains(&note) {
                        continue;
                    }
                    let freq = midi_to_freq(note);
                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: vel });
                }
            }
        }
    }

    fn trigger_song_step(&mut self, bar: usize, step_in_bar: usize) {
        if bar >= 32 || step_in_bar >= 16 {
            return;
        }

        if self.metronome_enabled && step_in_bar % 4 == 0 {
            if step_in_bar == 0 {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeHigh));
            } else {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeLow));
            }
        }
        let has_track_solo = self.playlist_tracks.iter().any(|t| t.solo);
        let vel = self.step_velocities[step_in_bar];

        for (_t_idx, track) in self.playlist_tracks.iter().enumerate() {
            let is_audible = if has_track_solo { track.solo } else { !track.muted };
            if !is_audible {
                continue;
            }
            // Ett fruset spår ligger färdigrenderat: ljudet strömmas av motorn
            // som ett stem-spår, och pattern-triggningen ska vara tyst — annars
            // hörs spåret två gånger. (Pattern-LÄGET rör kanalracket, inte
            // spåret, och påverkas därför inte.)
            if track.is_frozen() {
                continue;
            }

            if let Some(pat_idx) = track.clips[bar]
                && let Some(pat) = self.patterns.get(pat_idx) {
                    match track.kind {
                        TrackKind::Drums => {
                            for ch_idx in 0..=5 {
                                if ch_idx < pat.channel_steps.len() && pat.channel_steps[ch_idx][step_in_bar] {
                                    let note = pat.channel_notes.get(ch_idx).map(|n| n[step_in_bar]).unwrap_or(36);
                                    let sample_cmd = self.channels.get(ch_idx)
                                        .and_then(|ch| channel_sample_trigger_command(ch, note, track.volume * vel));
                                    if let Some(cmd) = sample_cmd {
                                        let _ = self.engine.send_command(cmd);
                                    } else {
                                        match ch_idx {
                                            0 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
                                            1 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
                                            2 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
                                            3 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed)); }
                                            4 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen)); }
                                            5 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Crash)); }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            if self.drummer_tom_steps[step_in_bar] {
                                let tom = if self.drummer_tom_high { DrumType::TomHigh } else { DrumType::TomLow };
                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(tom));
                            }
                        }
                        TrackKind::SynthLead | TrackKind::Bassline => {
                            let ch_idx = if track.kind == TrackKind::SynthLead { 6 } else { 7 };
                            // Tagningens mikro-tajming (Fas 6.4 steg 2), samma väg
                            // som i pattern-läget — och läses före grinden, annars
                            // tystnar en not som spelades sent i föregående steg.
                            let take_plan = crate::midi_take::plan_for_step(
                                &pat.take,
                                step_in_bar,
                                self.step_samples(step_in_bar),
                                &|key, slot| {
                                    (48..72).contains(&key) && pat.piano_roll_grid[(key - 48) as usize][slot]
                                },
                            );
                            let grid_active = track.kind == TrackKind::SynthLead
                                && ((0..24).any(|r| pat.piano_roll_grid[r][step_in_bar])
                                    || !take_plan.play.is_empty());
                            if grid_active {
                                for (note, delay, take_vel) in &take_plan.play {
                                    let freq = midi_to_freq(*note);
                                    let _ = self.engine.send_command(AudioCommand::NoteOnDelayed {
                                        note: *note,
                                        freq,
                                        velocity: track.volume * vel * take_vel,
                                        delay_samples: *delay,
                                    });
                                }
                                for row in 0..24 {
                                    if pat.piano_roll_grid[row][step_in_bar] {
                                        let note = 48 + row as u8;
                                        if take_plan.skip.contains(&note) {
                                            continue;
                                        }
                                        let freq = midi_to_freq(note);
                                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                                    }
                                }
                            } else if pat.channel_steps.len() > ch_idx && pat.channel_steps[ch_idx][step_in_bar] {
                                let note = pat.channel_notes[ch_idx][step_in_bar];
                                let sample_cmd = self.channels.get(ch_idx)
                                    .and_then(|ch| channel_sample_trigger_command(ch, note, track.volume * vel));
                                if let Some(cmd) = sample_cmd {
                                    let _ = self.engine.send_command(cmd);
                                } else {
                                    let freq = midi_to_freq(note);
                                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                                }
                            }
                        }
                        TrackKind::VocalAudio | TrackKind::CustomAudio | TrackKind::Fx => {}
                    }
                }
        }
    }

    pub fn export_wav(&mut self) {
        // Snabbexport öppnar exportdialogen (fullt projekt: format, mapp &
        // ren metadata – ingen hårdkodad sökväg kvar).
        self.open_export_modal();
    }

    fn play_note(&mut self, note: u8) {
        self.play_note_velocity(note, 0.85);
    }

    fn play_note_velocity(&mut self, note: u8, velocity: f32) {
        if !self.active_keys.contains(&note) {
            self.active_keys.insert(note);
            let freq = midi_to_freq(note);
            let _ = self.engine.send_command(AudioCommand::NoteOn {
                note,
                freq,
                velocity: velocity.clamp(0.05, 1.0),
            });
        }
    }

    fn release_note(&mut self, note: u8) {
        self.active_keys.remove(&note);
        let _ = self.engine.send_command(AudioCommand::NoteOff { note });
    }

    /// Handles a real MIDI keyboard note event: plays it, and (when armed and
    /// the transport is running) records it into the active Piano Roll pattern.
    fn handle_midi_note(&mut self, note: u8, velocity: u8, on: bool) {
        let is_on = on && velocity > 0;
        if is_on {
            if self.midi_held_notes.insert(note) {
                self.play_note_velocity(note, velocity as f32 / 127.0);
                if self.midi_record_armed && self.is_playing {
                    self.record_midi_note_at_step(note);
                    // Tagningen (Fas 6.4): noten skrivs till rutnätet *och* sparas
                    // med sin faktiska tid, så att den går att kvantisera eller
                    // humanisera i efterhand. Rutnätet ensamt kastade tiden.
                    let pos = self.current_take_pos();
                    if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
                        pat.take.push(pos, note, velocity as f32 / 127.0);
                    }
                }
            }
        } else if self.midi_held_notes.remove(&note) {
            self.release_note(note);
        }
    }

    /// Hur många samples ett steg är (Fas 6.4 steg 2). Tagningens mikro-tajming
    /// räknas om till samples med motorns egen frekvens, eftersom kön i motorn
    /// räknar ned per sample.
    fn step_samples(&self, step: usize) -> u32 {
        let secs = self.step_duration(step).as_secs_f32();
        (secs * self.engine.sample_rate as f32).max(1.0) as u32
    }

    /// Läser av en pågående biblioteksskanning (Fas 7.4).
    ///
    /// Statusraden visar förloppet medan den kör, och när den är klar fylls
    /// biblioteket — och de inbyggda trumkanalerna får riktiga inspelade samplar
    /// om de fortfarande står på standardljudet. Att tilldela i efterhand går
    /// bra: kanalens PCM skickas till motorn vid varje anslag.
    fn poll_library_scan(&mut self) {
        let Some(scan) = self.library_scan.as_mut() else {
            return;
        };
        let poll = scan.poll();
        let elapsed = self.library_scan_started.elapsed().as_secs_f32();
        match poll {
            crate::audio::factory_samples::ScanPoll::Idle => {
                // Bara om inget viktigare står där: en skanning får inte skriva
                // över en ångrings- eller felrad.
                if self.status_message.is_empty() || self.status_message.starts_with("🎵") {
                    self.status_message =
                        crate::tstatus!("🎵 Läser in ljudbiblioteket i bakgrunden… ({:.0} s)", elapsed);
                }
            }
            crate::audio::factory_samples::ScanPoll::Progress { found, dir } => {
                self.status_message = crate::tstatus!(
                    "🎵 Skannar ljudbiblioteket: {} samplar ({}, {:.0} s)",
                    found,
                    dir,
                    elapsed
                );
            }
            crate::audio::factory_samples::ScanPoll::Done(items) => {
                let count = items.len();
                let scanned = library_items_from_scanned(items);
                let existing = self.sample_library.clone();
                self.sample_library = merge_library(scanned, &existing);
                // Ge de inbyggda trumkanalerna riktiga samplar — men rör inte en
                // kanal användaren redan lagt ett eget ljud på. Kit-tilldelaren tar
                // en hel slice, så de orörda kanalerna lyfts ut, tilldelas och
                // läggs tillbaka på samma platser.
                if !self.sample_library.is_empty() {
                    let untouched: Vec<usize> = (0..self.channels.len().min(6))
                        .filter(|i| self.channels[*i].sample_path.is_none())
                        .collect();
                    if !untouched.is_empty() {
                        let mut scope: Vec<ChannelStrip> =
                            untouched.iter().map(|i| self.channels[*i].clone()).collect();
                        auto_assign_default_kit(&mut scope, &self.sample_library);
                        for (slot, i) in untouched.iter().enumerate() {
                            self.channels[*i] = scope[slot].clone();
                        }
                    }
                }
                self.status_message = crate::tstatus!(
                    "🎵 Ljudbiblioteket klart: {} samplar på {:.1} s",
                    count,
                    elapsed
                );
                self.library_scan = None;
            }
            crate::audio::factory_samples::ScanPoll::Aborted => {
                self.status_message = crate::i18n::t(
                    "⚠ Kunde inte läsa in ljudbiblioteket — skanningen avbröts",
                )
                .to_string();
                self.library_scan = None;
            }
        }
    }

    /// Positionen i takten (i steg) för en not som spelas just nu (Fas 6.4).
    ///
    /// Stegfasen mäts mot sekvenserns stegklocka, som går i UI-tråden:
    /// upplösningen är därför en bildruta (≈16 ms vid 60 Hz), inte samplen.
    fn current_take_pos(&self) -> f32 {
        let step_secs = self.step_duration(self.current_step).as_secs_f32();
        let phase = if step_secs > 0.0 {
            self.last_step_time.elapsed().as_secs_f32() / step_secs
        } else {
            0.0
        };
        crate::midi_take::take_pos_from(self.current_step, phase)
    }

    /// Börjar en ny tagning i det valda patternet (Fas 6.4). Utan detta skulle
    /// flera inspelningsförsök växa ihop till en enda tagning.
    fn begin_new_take(&mut self) {
        let name = self
            .patterns
            .get(self.selected_pattern)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
            pat.take = crate::midi_take::Take::new();
        }
        self.status_message = crate::tstatus!("🔴 Inspelning armar — ny tagning i '{}'", name);
    }

    /// Kvantiserar tagningen i det valda patternet och *mäter* skillnaden
    /// (Fas 6.4): "otajthet" är medelavståndet från noterna till rutnätet i steg,
    /// så både före och efter går att skriva ut i siffror i stället för att
    /// påstås.
    fn quantize_take(&mut self, strength: f32, grid: crate::midi_take::TakeGrid) {
        // Ångringspunkten tas *före* ändringen (Fas 6.4), som för allt annat som
        // ändrar projektet.
        self.push_undo("🎯 Kvantisering av tagningen");
        let swing = self.swing;
        let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
            return;
        };
        if pat.take.is_empty() {
            self.status_message = crate::i18n::t(
                "⚠ Ingen tagning att kvantisera — armera ⏺ MIDI-REC och spela in först",
            )
            .to_string();
            return;
        }
        let before = pat.take.tightness();
        let notes = pat.take.len();
        pat.take.quantize(strength, swing, grid);
        let after = pat.take.tightness();
        self.status_message = crate::tstatus!(
            "🎯 Kvantiserade {} noter ({}, styrka {:.0} %, sväng {:.0} %): {:.2} → {:.2} steg otajt",
            notes,
            grid.label(),
            strength * 100.0,
            swing * 100.0,
            before,
            after
        );
    }

    /// Lägger medveten mänsklig variation på tagningen (Fas 6.4). Slumptalet
    /// räknas upp varje gång, så två tryck ger inte exakt samma tagning.
    fn humanize_take(&mut self, amount: f32) {
        self.push_undo("🌀 Humanisering av tagningen");
        let amount = amount.clamp(0.0, 1.0);
        let timing_steps = 0.15 * amount;
        let velocity_amount = 0.3 * amount;
        self.take_seed_counter = self.take_seed_counter.wrapping_add(1);
        let seed = self
            .take_seed_counter
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x1234_5678);
        let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
            return;
        };
        if pat.take.is_empty() {
            self.status_message = crate::i18n::t(
                "⚠ Ingen tagning att humanisera — armera ⏺ MIDI-REC och spela in först",
            )
            .to_string();
            return;
        }
        let before = pat.take.tightness();
        let notes = pat.take.len();
        pat.take.humanize(timing_steps, velocity_amount, seed);
        let after = pat.take.tightness();
        self.status_message = crate::tstatus!(
            "🌀 Humaniserade {} noter (mängd {:.0} %: tid ±{:.2} steg, anslag ±{:.0} %): {:.2} → {:.2} steg otajt",
            notes,
            amount * 100.0,
            timing_steps,
            velocity_amount * 100.0,
            before,
            after
        );
    }

    /// Writes a held MIDI note into the Piano Roll grid at the current step.
    fn record_midi_note_at_step(&mut self, note: u8) {
        const BASE_MIDI: u8 = 48;
        const ROWS: usize = 24;
        if let Some(off) = crate::audio::note_to_roll_offset(note, BASE_MIDI, ROWS) {
            let step = self.current_step.min(15);
            self.piano_roll_grid[off][step] = true;
            self.channels[6].steps[step] = true;
            self.channels[6].notes[step] = BASE_MIDI + off as u8;
            self.sync_active_pattern_from_ui();
        }
    }

    /// Drains real MCU/OSC events and refreshes connection status.
    fn poll_hardware_control(&mut self) {
        while let Ok(ev) = self.control_rx.try_recv() {
            if self.midi_learn_active {
                self.midi_learn_active = false;
                self.status_message = crate::tstatus!("🎯 MIDI Learn fångade kontroll: {}", format!("{:?}", ev));
            }
            self.apply_control_event(ev);
            // En hårdvarukontroll (MCU/OSC) ändrar mixern utan pekare. Den ska
            // ändå ge en ångringspunkt (Fas 6.2), annars vore en faderändring
            // från en kontrollyta omöjlig att ångra.
            self.mixer_control_event = true;
        }
        if let Some(server) = &self.osc_server {
            self.osc_rx_count = server.received();
        }
        if let Some(mcu) = &self.mcu_input {
            self.mcu_connected = true;
            let devices = mcu.device_list();
            self.mcu_device_name = if devices.is_empty() {
                crate::i18n::t("Sonix MCU In (väntar – anslut enhet med aconnect)").to_string()
            } else {
                devices.join(", ")
            };
        } else {
            self.mcu_connected = false;
        }
        if let Some(midi) = &self.midi_input {
            self.midi_keyboard_connected = true;
            self.midi_note_count = midi.received();
            let devices = midi.device_list();
            self.midi_device_name = if devices.is_empty() {
                crate::i18n::t("Sonix MIDI In (väntar – anslut med aconnect)").to_string()
            } else {
                devices.join(", ")
            };
        } else {
            self.midi_keyboard_connected = false;
        }
    }

    fn apply_control_event(&mut self, ev: ControlEvent) {
        match ev {
            ControlEvent::Play(v) => {
                if v != self.is_playing {
                    self.toggle_playback();
                }
            }
            ControlEvent::Stop => {
                if self.is_playing {
                    self.stop_playback();
                }
            }
            ControlEvent::Record(v) => {
                if v != self.is_recording_timeline {
                    self.toggle_timeline_recording();
                }
            }
            ControlEvent::TrackVolume { track, value } => {
                if track < self.playlist_tracks.len() {
                    self.playlist_tracks[track].volume = value.clamp(0.0, 1.25);
                    self.sync_track_audio_state(track);
                }
            }
            ControlEvent::TrackMute { track, mute } => {
                if track < self.playlist_tracks.len() {
                    self.playlist_tracks[track].muted = mute;
                    self.sync_track_audio_state(track);
                }
            }
            ControlEvent::BusVolume { bus, value } => {
                if bus < self.bus_volume.len() {
                    self.bus_volume[bus] = value.clamp(0.0, 1.25);
                    self.sync_group_state();
                }
            }
            ControlEvent::VcaVolume { vca, value } => {
                if vca < self.vca_faders.len() {
                    self.vca_faders[vca] = value.clamp(0.0, 1.25);
                    self.sync_group_state();
                }
            }
            ControlEvent::MidiNote { note, velocity, on } => {
                self.handle_midi_note(note, velocity, on);
            }
        }
    }

    /// Push the modular patcher graph to the audio engine whenever it changes.
    fn sync_patcher_graph(&mut self) {
        let spec = self.modular_graph.to_spec();
        if self.last_patcher_spec.as_ref() != Some(&spec) {
            self.last_patcher_spec = Some(spec.clone());
            let _ = self.engine.send_command(AudioCommand::SetPatcherGraph(spec));
        }
    }

    /// Registers the microphone monitor ring with the engine (once, or after a
    /// reconfigure) and pushes the live monitoring / auto-tune parameters into
    /// the input callback. Cheap enough to call every UI frame.
    fn sync_mic_monitoring(&mut self) {
        let ring = self
            .vocal_studio
            .mic_capture
            .as_ref()
            .map(|m| std::sync::Arc::clone(&m.monitor_ring));
        let needs_send = match (&ring, &self.monitor_ring_sent) {
            (Some(a), Some(b)) => !std::sync::Arc::ptr_eq(a, b),
            (Some(_), None) => true,
            _ => false,
        };
        if needs_send && let Some(r) = ring {
            let _ = self.engine.send_command(AudioCommand::SetMonitorRing { ring: r.clone() });
            self.monitor_ring_sent = Some(r);
        }
        // Egen nivå för direktlyssningen: med den kan rundgång brytas utan att
        // sänka mastervolymen (som tidigare var enda reglaget).
        let level = self.vocal_studio.monitor_level.clamp(0.0, 1.0);
        if self.monitor_level_sent != Some(level) {
            let _ = self
                .engine
                .send_command(AudioCommand::SetMonitorLevel(level));
            self.monitor_level_sent = Some(level);
        }
        self.vocal_studio.sync_live_effects(
            self.vocal_studio.realtime_autotune,
            self.vocal_harmonizer.autotune_speed,
            self.vocal_harmonizer.autotune_speed,
            self.vocal_harmonizer.root_note,
            self.vocal_harmonizer.target_scale,
        );
    }

    /// Re-sends all persistent synth/mixer state after the output stream has
    /// been rebuilt (e.g. a sample-rate change), because `reconfigure()`
    /// recreates the `SynthEngine` from scratch.
    fn resync_engine_after_reconfigure(&mut self) {
        self.monitor_ring_sent = None;
        self.monitor_level_sent = None;
        let _ = self.engine.send_command(AudioCommand::SetWaveform(self.waveform));
        let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
        let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
        let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
            amount: self.filter_env_amount,
            adsr: self.filter_env,
        });
        let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay));
        let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb));
        let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
        let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: 0, bpm: self.bpm });
        self.last_patcher_spec = None;
        self.sync_patcher_graph();
        let _ = self.engine.send_command(AudioCommand::SetPatcherEnabled(self.patcher_enabled));
        self.is_playing = false;
        if self.view_mode == ViewMode::StemSeparator && !self.stem_project.stem_audio.is_empty() {
            self.load_separated_stems_to_engine();
        } else {
            self.sync_all_stems_to_engine();
        }
    }
}

impl eframe::App for SonixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-open the MIDI keyboard input port once, so external keyboards
        // work as soon as they are connected with `aconnect`.
        if !self.midi_auto_connect_attempted {
            self.midi_auto_connect_attempted = true;
            if let Ok(midi) = MidiKeyboardInput::connect(self.control_tx.clone()) {
                self.midi_input = Some(midi);
            }
        }
        self.poll_hardware_control();
        self.poll_library_scan();
        self.sync_patcher_graph();
        self.sync_stem_separator_engine();
        // Keep embedded plugin editors responsive (Fas 4.4b).
        self.poll_plugin_guis();
        // Supervise the out-of-process sandbox worker (Fas 4.5a).
        #[cfg(feature = "plugin-host")]
        self.poll_plugin_sandbox();

        // Screenshot event listener
        let mut received_screenshot = None;
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    received_screenshot = Some(image.clone());
                }
            }
        });

        if let Some(img) = received_screenshot {
            if let ScreenshotState::AwaitingCapture { dest } = &self.screenshot_state {
                let w = img.size[0] as u32;
                let h = img.size[1] as u32;
                let raw_bytes: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), c.a()]).collect();
                if let Some(rgba) = image::RgbaImage::from_raw(w, h, raw_bytes) {
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(e) = rgba.save(dest) {
                        eprintln!("❌ Kunde inte spara skärmdump till {:?}: {}", dest, e);
                    } else {
                        println!("📸 Sparade skärmdump: {:?}", dest);
                    }
                }
                self.screenshot_state = ScreenshotState::Idle;
            }
        }

        // Check for async file dialog results
        let mut picked_file = None;
        if let Ok(mut lock) = self.pending_file_dialog_result.try_lock() {
            if let Some(path) = lock.take() {
                picked_file = Some(path);
            }
        }
        if let Some(path) = picked_file {
            let p_lower = path.to_lowercase();
            let import_tmpl = self.pending_add_track_import.take();
            if self.pending_stem_separation {
                self.pending_stem_separation = false;
                self.start_stem_separation(&path);
            } else if p_lower.ends_with(".sonix") {
                self.load_project_file(&path);
                self.show_project_manager_modal = false;
            } else if p_lower.ends_with(".zip") {
                self.start_suno_zip_import(&path);
                self.show_suno_import_modal = false;
            } else if let Some(tmpl) = import_tmpl {
                self.import_audio_file_as_track(&path, &tmpl);
            }
        }

        self.poll_remote_audio_generation();
        self.poll_stem_separation();

        self.advance_sequencer();
        self.apply_automation();
        self.anim_phase += 0.08;
        self.update_scope_history();
        self.vocal_studio.update_live_stream();

        // Autosave (Fas 6.1): kontrollera med jämna mellanrum om projektet
        // ändrats sedan det senast skyddades. Fingeravtrycket gör att ett
        // orörligt projekt inte roterar bort sin egen historik.
        self.autosave_accum += ctx.input(|i| i.stable_dt).min(0.5);
        if self.autosave_accum >= crate::autosave::INTERVAL_SECS {
            self.autosave_accum = 0.0;
            self.maybe_autosave();
        }
        // En strukturell ändring (märkt i `push_undo`) skrivs här i stället, när
        // ändringen är genomförd. Ett frame senare är max ~33 ms när fönstret är
        // fokuserat (200 ms oanvänt) — jämfört med upp till 60 s via timern.
        if self.autosave_pending {
            self.autosave_pending = false;
            self.autosave_after_structural_change();
        }

        // Mixer-undon (Fas 6.2). Ett reglage ändras kontinuerligt medan man drar,
        // så en ångringspunkt per frame skulle fylla historiken på ett drag. I
        // stället hålls mixerns "viloläge" — läget från senaste frameen där inget
        // rördes — och läggs som ångringspunkt första gången mixern ändras under
        // en interaktion. Det är exakt läget före draget. Ett nytt viloläge tas
        // först när pekaren är släppt, så ett drag ger en ångring, inte sextio.
        let mixer_now = self.mixer_state_digest();
        let mixer_pointer_down = ctx.input(|i| i.pointer.any_down());
        let mixer_touching =
            mixer_pointer_down || self.mixer_pointer_was_down || self.mixer_control_event;
        if mixer_now != self.mixer_settled_digest {
            let pending = self.mixer_settled_snapshot.take();
            if mixer_touching {
                // Användaren rör mixern: lägg läget före ändringen på historiken.
                if let Some(mut prev) = pending {
                    prev.description = crate::i18n::t("🎚 Mixerändring").to_string();
                    self.push_undo_snapshot(prev);
                }
            } else {
                // Programmatisk ändring (projektladdning, preset, ångring): inget
                // att ångra, men viloläget måste följa med.
                self.mixer_settled_snapshot = Some(self.current_snapshot("mixer"));
                self.mixer_settled_digest = mixer_now;
            }
        }
        self.mixer_pointer_was_down = mixer_pointer_down;
        self.mixer_control_event = false;
        self.sync_mic_monitoring();

        // Check if window is minimized or not focused (Wayland / Hyprland safety)
        let is_minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        let is_focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));

        // Smooth 60 FPS while playing/recording & focused, 30 FPS when idle, gentle 200ms when unfocused/minimized to prevent Wayland crashes
        if self.is_playing || self.vocal_studio.is_recording {
            if is_minimized || !is_focused {
                ctx.request_repaint_after(std::time::Duration::from_millis(33));
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        } else if is_focused && !is_minimized {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // Check if user is typing into an input field or if a modal is open
        let is_loading_proj = self.project_load_progress.try_lock().map(|p| p.is_loading).unwrap_or(false);
        let is_importing_stems = self.stem_import_progress.try_lock().map(|p| p.is_importing).unwrap_or(false);
        let modal_open = self.show_help_guide
            || self.show_project_manager_modal
            || self.show_suno_import_modal
            || self.show_render_queue_modal
            || self.show_stem_focus_modal
            || self.show_tempo_modal
            || self.show_controller_modal
            || self.show_about_modal
            || self.show_recovery_modal
            || self.show_ai_settings_modal
            || self.show_audio_settings_modal
            || self.show_import_modal
            || is_loading_proj
            || is_importing_stems;

        let wants_keyboard = ctx.wants_keyboard_input();
        let piano_active = self.view_mode == ViewMode::PianoRoll && !modal_open && !wants_keyboard;

        // Release any stuck notes if piano was switched away or keyboard input was captured
        if !piano_active && !self.active_keys.is_empty() {
            let keys_to_release: Vec<u8> = self.active_keys.iter().copied().collect();
            for note in keys_to_release {
                self.release_note(note);
            }
        }
        if !piano_active && !self.midi_held_notes.is_empty() {
            self.midi_held_notes.clear();
        }

        // Keyboard Shortcuts
        ctx.input(|i| {
            // Virtual Piano keys ONLY when Piano Roll is active on screen, no modal is open, and not typing text
            if piano_active && !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.command {
                let key_map = [
                    (egui::Key::A, 0),
                    (egui::Key::W, 1),
                    (egui::Key::S, 2),
                    (egui::Key::E, 3),
                    (egui::Key::D, 4),
                    (egui::Key::F, 5),
                    (egui::Key::T, 6),
                    (egui::Key::G, 7),
                    (egui::Key::Y, 8),
                    (egui::Key::H, 9),
                    (egui::Key::U, 10),
                    (egui::Key::J, 11),
                    (egui::Key::K, 12),
                    (egui::Key::O, 13),
                    (egui::Key::L, 14),
                    (egui::Key::P, 15),
                ];

                for (k, semi) in key_map {
                    let note = ((self.octave + 1) * 12 + semi) as u8;
                    if i.key_pressed(k) {
                        self.play_note(note);
                    } else if i.key_released(k) {
                        self.release_note(note);
                    }
                }
            }

            // Global Command / Ctrl shortcuts (ALWAYS active)
            let is_cmd = i.modifiers.ctrl || i.modifiers.command;
            if is_cmd {
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.redo();
                    } else {
                        self.undo();
                    }
                }
                if i.key_pressed(egui::Key::Y) {
                    self.redo();
                }
                if i.key_pressed(egui::Key::C) {
                    self.copy_selected_region();
                }
                if i.key_pressed(egui::Key::V) {
                    self.paste_region();
                }
                if i.key_pressed(egui::Key::B) || i.key_pressed(egui::Key::X) {
                    self.split_selected_region_at_playhead();
                }
                if i.key_pressed(egui::Key::D) {
                    if i.modifiers.shift || self.selected_audio_region.is_none() {
                        self.duplicate_track(self.selected_timeline_track);
                    } else {
                        self.duplicate_selected_region();
                    }
                }
                if i.key_pressed(egui::Key::L) {
                    self.repeat_selected_region_loop(2.0);
                }
                if i.key_pressed(egui::Key::K) {
                    self.reverse_selected_region();
                }
                if i.key_pressed(egui::Key::N) {
                    self.new_empty_project();
                }
                if i.key_pressed(egui::Key::O) || i.key_pressed(egui::Key::P) {
                    self.show_project_manager_modal = true;
                }
                if i.key_pressed(egui::Key::S) {
                    let p_name = self.project_name.clone();
                    self.save_project(&p_name);
                }
                if i.key_pressed(egui::Key::I) {
                    self.show_suno_import_modal = true;
                }
                if i.key_pressed(egui::Key::E) {
                    self.open_export_modal();
                }
            }

            // Transport & Arranger shortcuts
            if !is_cmd {
                if i.key_pressed(egui::Key::Space) {
                    self.toggle_playback();
                }

                if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    self.delete_selected_region();
                }

                // Single key shortcuts when piano keys aren't intercepting (or in Arranger view)
                if !piano_active && !i.modifiers.alt {
                    if i.key_pressed(egui::Key::S) || i.key_pressed(egui::Key::B) || i.key_pressed(egui::Key::C) {
                        self.split_selected_region_at_playhead();
                    }
                    if i.key_pressed(egui::Key::M) {
                        self.toggle_mute_selected_region();
                    }
                    if i.key_pressed(egui::Key::R) && !self.is_recording_timeline {
                        self.reverse_selected_region();
                    }
                    if i.key_pressed(egui::Key::Num1) {
                        self.arranger_tool = ArrangerTool::Select;
                        self.status_message = crate::i18n::t("Verktyg: ⇱ Välj / Flytta / Trimma (1)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num2) {
                        self.arranger_tool = ArrangerTool::Paint;
                        self.status_message = crate::i18n::t("Verktyg: ✎ Rita (2)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num3) {
                        self.arranger_tool = ArrangerTool::Slice;
                        self.status_message = crate::i18n::t("Verktyg: ✂ Klipp / Sax (3)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num4) {
                        self.arranger_tool = ArrangerTool::Mute;
                        self.status_message = crate::i18n::t("Verktyg: 🔇 Muta (4)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num5) {
                        self.arranger_tool = ArrangerTool::Erase;
                        self.status_message = crate::i18n::t("Verktyg: 🗑 Radera (5)").to_string();
                    }
                }
            }

            // Global View Shortcuts (F-keys)
            if i.key_pressed(egui::Key::F1) {
                self.show_help_guide = !self.show_help_guide;
            }
            if i.key_pressed(egui::Key::F3) {
                self.view_mode = ViewMode::PlaylistArranger;
            }
            if i.key_pressed(egui::Key::F4) {
                self.view_mode = ViewMode::ChannelRack;
            }
            if i.key_pressed(egui::Key::F5) {
                self.view_mode = ViewMode::PianoRoll;
            }
            if i.key_pressed(egui::Key::F6) {
                self.view_mode = ViewMode::EffectsMixer;
            }
            if i.key_pressed(egui::Key::F7) {
                self.view_mode = ViewMode::AlchemySynth;
            }
            if i.key_pressed(egui::Key::F8) {
                self.view_mode = ViewMode::VocalStudio;
            }
            if i.key_pressed(egui::Key::F9) {
                self.view_mode = ViewMode::AiMusicAssistant;
            }
            if i.key_pressed(egui::Key::F10) {
                self.view_mode = ViewMode::ModularPatcher;
            }
            if i.key_pressed(egui::Key::F11) {
                self.show_chord_generator_modal = !self.show_chord_generator_modal;
            }
            if i.key_pressed(egui::Key::F12) {
                self.show_dice_generator_modal = !self.show_dice_generator_modal;
            }
        });

        // Drag & Drop support for Suno ZIP stems or folders
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(ref path) = file.path {
                    let path_str = path.to_string_lossy().to_string();
                    if path_str.to_lowercase().ends_with(".zip") {
                        self.import_suno_zip(&path_str);
                    } else if path.is_dir() {
                        let (title, bpm) = Self::parse_suno_zip_info(&path_str);
                        self.import_suno_stems_from_folder(&path_str, &title, bpm);
                    }
                }
            }
        }

        // ====================================================================
        // 0. PERMANENT TOP DESKTOP MENU BAR (ARKIV, REDIGERA, VY, AI, INSTÄLLNINGAR, HJÄLP)
        // ====================================================================
        egui::TopBottomPanel::top("desktop_menu_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgb(16, 18, 24)).inner_margin(egui::Margin::symmetric(8.0, 3.0)))
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    // Arkiv
                    ui.menu_button(self.tr("📁 Arkiv"), |ui| {
                        if ui.button(self.tr("📄 Nytt tomt projekt (Ctrl+N)")).clicked() {
                            self.new_empty_project();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📂 Öppna projekt... (Ctrl+O)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("💾 Spara projekt (Ctrl+S)")).clicked() {
                            let p_name = self.project_name.clone();
                            self.save_project(&p_name);
                            ui.close_menu();
                        }
                        if ui.button(self.tr("💾 Spara som... (Ctrl+Shift+S)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("📁 Filhanterare / Projektbläddrare (Ctrl+P)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }

                        // 🕘 Senaste projekt (Fas 6.1): läser recent.json. Läslistan
                        // är en bekvämlighet — stängs av sig själv om filen är tom.
                        let recent = load_recent_projects();
                        let mut open_recent: Option<String> = None;
                        ui.add_enabled_ui(!recent.is_empty(), |ui| {
                            ui.menu_button(self.tr("🕘 Senaste projekt"), |ui| {
                                for entry in &recent {
                                    let label = format!(
                                        "{}  ({})",
                                        entry.name,
                                        crate::autosave::relative_age(
                                            entry.opened,
                                            crate::autosave::now_stamp()
                                        )
                                    );
                                    if ui.button(label).on_hover_text(&entry.path).clicked() {
                                        open_recent = Some(entry.path.clone());
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        if let Some(path) = open_recent {
                            self.load_project_file(&path);
                        }

                        if ui.button(self.tr("📂 Visa projektmappen i filhanteraren")).clicked() {
                            // Öppnar den kanoniska projektmappen i systemets
                            // filhanterare. Ingen egen filbläddrare — det är
                            // användarens filhanterare som gäller.
                            let dir = crate::paths::paths().projects_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            if let Err(e) = std::process::Command::new("xdg-open")
                                .arg(&dir)
                                .spawn()
                            {
                                self.status_message = crate::tstatus!(
                                    "⚠ Kunde inte öppna '{}' i filhanteraren: {}",
                                    dir.display(),
                                    e
                                );
                            }
                            ui.close_menu();
                        }

                        if ui.button(self.tr("⚡ Ladda Demo-projekt")).clicked() {
                            self.load_demo_project();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Importera Stämmor / Stems... (Ctrl+I)")).clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("↗ Exportera projekt (Ctrl+E)")).clicked() {
                            self.open_export_modal();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Exportera sång som MIDI (.mid)")).clicked() {
                            self.export_song_midi();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎼 Importera MIDI-fil (.mid)...")).clicked() {
                            self.show_midi_import_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🚪 Avsluta")).clicked() {
                            std::process::exit(0);
                        }
                    });

                    // Redigera
                    ui.menu_button(self.tr("✏ Redigera"), |ui| {
                        if ui.button(self.tr("↶ Ångra (Ctrl+Z)")).clicked() {
                            self.undo();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("↷ Gör om (Ctrl+Y)")).clicked() {
                            self.redo();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("✂ Klipp vid spelhuvud (Ctrl+B / S)")).clicked() {
                            self.split_selected_region_at_playhead();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📋 Duplicera markerat (Ctrl+D)")).clicked() {
                            self.duplicate_selected_region();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🗑 Ta bort markerat (Del)")).clicked() {
                            self.delete_selected_region();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🧹 Rensa alla spår")).clicked() {
                            self.push_undo(crate::i18n::t("Rensa alla spår"));
                            for t in &mut self.playlist_tracks {
                                t.regions.clear();
                                t.clips = [None; 32];
                            }
                            self.selected_audio_region = None;
                            self.status_message = self.tr("Rensade alla spår och tidslinjeklipp.").to_string();
                            ui.close_menu();
                        }
                    });

                    // Vy
                    ui.menu_button(self.tr("👁 Vy"), |ui| {
                        if ui.button(self.tr("📊 Tidslinje / Arranger (F3)")).clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🥁 Sonix Channel Rack (F4)")).clicked() {
                            self.view_mode = ViewMode::ChannelRack;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎹 Sonix Piano Roll (F5)")).clicked() {
                            self.view_mode = ViewMode::PianoRoll;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Mixer Console (F6)")).clicked() {
                            self.view_mode = ViewMode::EffectsMixer;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Analog Synthesizer (F7)")).clicked() {
                            self.view_mode = ViewMode::AlchemySynth;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎙 Vocal Studio & Harmonizer (F8)")).clicked() {
                            self.view_mode = ViewMode::VocalStudio;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🤖 AI Music Studio (F9)")).clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🔌 Modulär Synt & Patcher (F10)")).clicked() {
                            self.view_mode = ViewMode::ModularPatcher;
                            ui.close_menu();
                        }
                        ui.separator();
                        let _browser_toggle = if self.show_browser { self.tr("📁 Växla Webbläsare / Bibliotek: PÅ") } else { self.tr("📁 Växla Webbläsare / Bibliotek: AV") };
                        if ui.button(_browser_toggle).clicked() {
                            self.show_browser = !self.show_browser;
                            ui.close_menu();
                        }
                    });

                    // AI & Providers
                    ui.menu_button(self.tr("🤖 AI & Providers"), |ui| {
                        if ui.button(self.tr("⚙ AI-inställningar & API-nycklar...")).clicked() {
                            self.show_ai_settings_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Importera Stämmor / Multi-Track Stems...")).clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("✨ ACE-Step & Stable Audio Generator...")).clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🤖 AI Co-Producer Assistent...")).clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                    });

                    // Verktyg & Kreativa Generatorer
                    ui.menu_button(self.tr("🎛 Verktyg"), |ui| {
                        if ui.button(self.tr("🎲 Melodi- & Beat-Tärning... (F12)")).clicked() {
                            self.show_dice_generator_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎹 Smart Ackord- & Skalgenerator... (F11)")).clicked() {
                            self.show_chord_generator_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎛 Modulärt FX-Pedalbord & Stompboxes...")).clicked() {
                            self.show_fx_rack_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎯 Hårdvarustämapparat & Pitch Scope (Tuner)...")).clicked() {
                            self.show_tuner_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📑 Låtstruktur & Formdelar...")).clicked() {
                            self.show_song_structure_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎙 Mikrofonjustering & Inmatningspanel...")).clicked() {
                            self.show_mic_settings_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Inställningar
                    ui.menu_button(self.tr("⚙ Inställningar"), |ui| {
                        if ui.button(self.tr("🎙 Mikrofoninställningar & Inmatningsenhet (Samson/USB)...")).clicked() {
                            self.show_mic_settings_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Ljud- & MIDI-inställningar (PipeWire/ALSA/JACK)...")).clicked() {
                            self.show_audio_settings_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Hårdvarukontroller (MCU / OSC)...")).clicked() {
                            self.show_controller_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Hjälp
                    ui.menu_button(self.tr("❓ Hjälp"), |ui| {
                        if ui.button(self.tr("📖 Snabbguide & Manual (F1)")).clicked() {
                            self.show_help_guide = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("ℹ Om Sonix Studio...")).clicked() {
                            self.show_about_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Project indicator & quick status
                    ui.separator();
                    let p_label = format!("{} {}", self.tr("📁 Projekt:"), self.project_name);
                    if ui.add(egui::Button::new(egui::RichText::new(p_label).size(10.5).color(Theme::FL_ORANGE)).frame(false)).on_hover_text(self.tr("Klicka för att hantera projekt")).clicked() {
                        self.show_project_manager_modal = true;
                    }
                });
            });

        // ====================================================================
        // 1. TOP METALLIC TOOLBAR
        // ====================================================================
        egui::TopBottomPanel::top("fl_toolbar")
            .frame(egui::Frame::none().fill(Theme::HEADER_BG).inner_margin(6.0))
            .show(ctx, |ui| {
                // ROW 1: Logo, Transport, Master Knobs, Clock, Export & Guide (Responsive Wrapped)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                    // Browser Toggle
                    let browser_btn = ui.selectable_label(self.show_browser, self.tr("📁 Browser"));
                    if browser_btn.clicked() { self.show_browser = !self.show_browser; }

                    ui.separator();

                    ui.label(egui::RichText::new(crate::i18n::t("🍊 SONIX")).strong().size(15.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new(crate::i18n::t("STUDIO")).strong().size(12.0).color(Theme::FL_CYAN));
                    ui.separator();

                    // Transport Buttons
                    let play_color = if self.is_playing { Theme::FL_GREEN } else { Color32::from_rgb(40, 50, 45) };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ▶ PLAY ")).strong().size(11.0).color(Color32::WHITE)).fill(play_color)).clicked()
                        && !self.is_playing {
                            self.toggle_playback();
                        }

                    let pause_color = if !self.is_playing { Theme::FL_ORANGE } else { Color32::from_rgb(45, 40, 35) };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ⏸ PAUSE ")).strong().size(11.0).color(Color32::WHITE)).fill(pause_color)).clicked()
                        && self.is_playing {
                            self.toggle_playback();
                        }

                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ⏹ STOP ")).strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(45, 48, 56))).clicked() {
                        self.stop_playback();
                    }

                    ui.separator();

                    // Digital LCD Display & Clock (Compact, monospace)
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("BPM")).size(9.0).color(Theme::TEXT_MUTED));
                            ui.add_sized(Vec2::new(38.0, 18.0), egui::Label::new(egui::RichText::new(format!("{:.1}", self.bpm)).monospace().strong().size(11.5).color(Theme::LCD_TEXT)));
                            if ui.button(crate::i18n::t("▲")).clicked() && self.bpm < 240.0 { self.bpm += 1.0; }
                            if ui.button(crate::i18n::t("▼")).clicked() && self.bpm > 40.0 { self.bpm -= 1.0; }

                            ui.separator();

                            let mode_text = if self.pattern_mode { "PAT" } else { "SONG" };
                            let mode_color = if self.pattern_mode { Theme::FL_ORANGE } else { Theme::FL_CYAN };
                            if ui.add_sized(Vec2::new(38.0, 18.0), egui::Button::new(egui::RichText::new(mode_text).strong().size(9.0).color(Color32::BLACK)).fill(mode_color)).clicked() {
                                self.pattern_mode = !self.pattern_mode;
                                self.status_message = (if self.pattern_mode { self.tr("Läge ändrat till: PAT (Mönsterloop)") } else { self.tr("Läge ändrat till: SONG (Låtläge)") }).to_string();
                            }

                            ui.separator();

                            let time_str = format_time_hundredths(self.song_time);
                            let (txt, color) = if self.pattern_mode {
                                let bar = (self.current_step / 4) + 1;
                                let step_in_bar = (self.current_step % 4) + 1;
                                (format!("⏱ {} [B{:02}:S{}]", time_str, bar, step_in_bar), Theme::LCD_ORANGE)
                            } else {
                                let bar = self.song_bar + 1;
                                let step = (self.song_step_in_bar / 4) + 1;
                                (format!("⏱ {} [T{:02}.{}]", time_str, bar, step), Theme::FL_CYAN)
                            };
                            ui.label(egui::RichText::new(txt).monospace().strong().size(11.0).color(color));
                        });
                    });

                    ui.separator();

                    // Pattern Quick Selector
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("PAT:")).size(9.0).color(Theme::TEXT_MUTED));
                            let p_len = self.patterns.len();
                            for pat_idx in 0..p_len {
                                let is_active = self.selected_pattern == pat_idx;
                                let p_color = self.patterns[pat_idx].color;
                                let fill = if is_active { p_color } else { Theme::PANEL_BG };
                                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                                if ui.add(egui::Button::new(egui::RichText::new(format!("P{}", pat_idx + 1)).strong().size(9.5).color(text_color)).fill(fill)).clicked() {
                                    self.select_pattern(pat_idx);
                                }
                            }
                        });
                    });

                    ui.separator();

                    // Swing Knob
                    rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 20.0);

                    ui.separator();

                    // Oscilloscope Display (real output waveform from audio thread)
                    let peak = self.engine.get_peak_level();
                    oscilloscope_display(ui, &self.scope_history, peak, Vec2::new(50.0, 22.0));

                    ui.separator();

                    // Master Volume & Pan Knobs
                    if rotary_knob(ui, &mut self.master_volume, 0.0, 1.0, "MASTER", Theme::FL_ORANGE, 20.0) {
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    }
                    rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", Color32::from_rgb(180, 190, 200), 20.0);

                    ui.separator();

                    // Export WAV Button
                    let export_label = if self.pattern_mode { "💾 EXPORT PAT" } else { "💾 EXPORT SONG" };
                    if ui.add(egui::Button::new(egui::RichText::new(export_label).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 90))).clicked() {
                        self.export_wav();
                    }

                    // Batch Export & Render Queue Button
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📤 BATCH EXPORT")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(45, 90, 140))).clicked() {
                        self.open_export_modal();
                    }

                    // Stems Importer Button
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📦 STEMS")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
                        self.scan_for_suno_stems();
                        self.show_suno_import_modal = true;
                    }

                    // Hardware Controller MCU / OSC Button
                    let mcu_col = if self.mcu_connected { Color32::from_rgb(110, 60, 130) } else { Theme::PANEL_BG };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎛 MCU")).strong().size(10.5).color(Color32::WHITE)).fill(mcu_col)).clicked() {
                        self.show_controller_modal = true;
                    }

                    ui.separator();

                    let guide_col = if self.show_help_guide { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                    if ui.button(egui::RichText::new(crate::i18n::t("💡 Guide")).color(guide_col)).clicked() {
                        self.show_help_guide = !self.show_help_guide;
                    }

                    ui.separator();

                    let lang_title = format!("🌐 {}", self.language.native_name());
                    egui::menu::menu_button(ui, egui::RichText::new(lang_title).size(11.0).color(if self.language != crate::i18n::Language::En { Theme::FL_YELLOW } else { Theme::TEXT_MUTED }), |ui| {
                        ui.label(egui::RichText::new(self.tr("🌐 Språk")).strong().size(11.0));
                        ui.separator();
                        for lang in crate::i18n::Language::all() {
                            if ui.selectable_label(self.language == lang, lang.native_name()).clicked() {
                                self.language = lang;
                                crate::i18n::set_current(lang);
                                ui.close_menu();
                            }
                        }
                    });
                });

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // ROW 2: Primary Workspaces & Advanced Studios Navigation Tabs (Responsive Wrapped)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                    ui.label(egui::RichText::new(self.tr("VY:")).strong().size(9.5).color(Theme::TEXT_MUTED));

                    let arr_btn = ui.selectable_label(self.view_mode == ViewMode::PlaylistArranger, self.tr("🎼 Tidslinje"));
                    if arr_btn.clicked() { self.view_mode = ViewMode::PlaylistArranger; }

                    let rack_btn = ui.selectable_label(self.view_mode == ViewMode::ChannelRack, self.tr("🥁 Rack"));
                    if rack_btn.clicked() { self.view_mode = ViewMode::ChannelRack; }

                    let roll_btn = ui.selectable_label(self.view_mode == ViewMode::PianoRoll, self.tr("🎹 Piano"));
                    if roll_btn.clicked() { self.view_mode = ViewMode::PianoRoll; }

                    let voc_btn = ui.selectable_label(self.view_mode == ViewMode::VocalStudio, self.tr("🎙 Sångstudio"));
                    if voc_btn.clicked() { self.view_mode = ViewMode::VocalStudio; }

                    let fx_btn = ui.selectable_label(self.view_mode == ViewMode::EffectsMixer, self.tr("🎚 Mixer"));
                    if fx_btn.clicked() { self.view_mode = ViewMode::EffectsMixer; }

                    let ai_btn = ui.selectable_label(self.view_mode == ViewMode::AiMusicAssistant, self.tr("🤖 AI"));
                    if ai_btn.clicked() { self.view_mode = ViewMode::AiMusicAssistant; }

                    ui.separator();
                    ui.label(egui::RichText::new(self.tr("STUDIOS:")).strong().size(9.5).color(Theme::TEXT_MUTED));

                    let alc_btn = ui.selectable_label(self.view_mode == ViewMode::AlchemySynth, self.tr("✨ Alchemy"));
                    if alc_btn.clicked() { self.view_mode = ViewMode::AlchemySynth; }

                    let drum_btn = ui.selectable_label(self.view_mode == ViewMode::SessionDrummer, self.tr("🥁 Trummor"));
                    if drum_btn.clicked() { self.view_mode = ViewMode::SessionDrummer; }

                    let patch_btn = ui.selectable_label(self.view_mode == ViewMode::ModularPatcher, self.tr("🧩 Patcher"));
                    if patch_btn.clicked() { self.view_mode = ViewMode::ModularPatcher; }

                    let stem_btn = ui.selectable_label(self.view_mode == ViewMode::StemSeparator, self.tr("🧠 Stems"));
                    if stem_btn.clicked() { self.view_mode = ViewMode::StemSeparator; }

                    let plug_btn = ui.selectable_label(self.view_mode == ViewMode::PluginManager, self.tr("🔌 Plugins"));
                    if plug_btn.clicked() { self.view_mode = ViewMode::PluginManager; }

                    let rmx_btn = ui.selectable_label(self.view_mode == ViewMode::RemixFx, self.tr("🎛 Remix"));
                    if rmx_btn.clicked() { self.view_mode = ViewMode::RemixFx; }

                    ui.separator();
                    ui.label(egui::RichText::new(self.tr("VERKTYG:")).strong().size(9.5).color(Theme::TEXT_MUTED));

                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎲 Tärning")).strong().size(9.5).color(Color32::WHITE)).fill(Color32::from_rgb(180, 80, 20))).on_hover_text(self.tr("Idé- & Slumptärning för Melodier & Beats (F12)")).clicked() {
                        self.show_dice_generator_modal = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎹 Ackord")).strong().size(9.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 110, 150))).on_hover_text(self.tr("Smart Ackord- & Skalgenerator (F11)")).clicked() {
                        self.show_chord_generator_modal = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎛 FX-Rack")).strong().size(9.5).color(Color32::WHITE)).fill(Color32::from_rgb(120, 50, 160))).on_hover_text(self.tr("Modulärt FX-Pedalbord & Stompbox Rack (Ctrl+R)")).clicked() {
                        self.show_fx_rack_modal = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎯 Stämmare")).strong().size(9.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 120, 80))).on_hover_text(self.tr("Hårdvarustämapparat & Pitch Analyzer (Ctrl+T)")).clicked() {
                        self.show_tuner_modal = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("📑 Formdelar")).strong().size(9.5).color(Color32::WHITE)).fill(Color32::from_rgb(140, 100, 30))).on_hover_text(self.tr("Låtstruktur & Formdelar")).clicked() {
                        self.show_song_structure_modal = true;
                    }
                });
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("fl_status_bar")
            .frame(egui::Frame::none().fill(Theme::PANEL_BG).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 2.0);
                    ui.label(egui::RichText::new(&self.status_message).size(11.0).color(Theme::FL_CYAN));
                    ui.separator();
                    ui.label(egui::RichText::new(self.tr("PipeWire / ALSA 44.1kHz • 16-Stämmor Polyfoni • Sonix Studio Pro DAW")).size(10.0).color(Theme::TEXT_MUTED));
                });
            });

        // ====================================================================
        // 2. LEFT SIDEBAR BROWSER (FL STUDIO & APPLE LOOPS LIBRARY)
        // ====================================================================
        if self.show_browser {
            egui::SidePanel::left("fl_browser")
                .frame(egui::Frame::none().fill(Theme::PANEL_BG).inner_margin(8.0))
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    self.render_browser_sidebar(ui);
                });
        }

        // ====================================================================
        // 3. MAIN WORKSPACE (VIEW MODES)
        // ====================================================================
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Theme::BG_DARK).inner_margin(12.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.view_mode {
                        ViewMode::PlaylistArranger => {
                            self.render_playlist_arranger(ui);
                        }
                        ViewMode::ChannelRack => {
                            self.render_channel_rack(ui);
                            ui.add_space(10.0);
                            self.render_synth_hardware_rack(ui);
                        }
                        ViewMode::PianoRoll => {
                            self.render_piano_roll_editor(ui);
                            ui.add_space(10.0);
                            self.render_touch_piano_keyboard(ui, ctx);
                            ui.add_space(10.0);
                            self.render_synth_hardware_rack(ui);
                        }
                        ViewMode::SessionDrummer => {
                            self.render_session_drummer(ui);
                        }
                        ViewMode::AlchemySynth => {
                            self.render_alchemy_morph_pad(ui);
                        }
                        ViewMode::RemixFx => {
                            self.render_remix_fx_view(ui);
                        }
                        ViewMode::ModularPatcher => {
                            let actions = render_patcher_view(
                                ui,
                                &mut self.modular_graph,
                                self.anim_phase,
                                &mut self.patcher_enabled,
                            );
                            if actions.enabled_changed {
                                let _ = self.engine.send_command(AudioCommand::SetPatcherEnabled(self.patcher_enabled));
                            }
                            if let Some((freq, vel)) = actions.note_on {
                                let _ = self.engine.send_command(AudioCommand::PatcherNoteOn { freq, velocity: vel });
                            } else if actions.note_off {
                                let _ = self.engine.send_command(AudioCommand::PatcherNoteOff);
                            }
                        }
                        ViewMode::StemSeparator => {
                            let actions = render_stem_separator_view(ui, &mut self.stem_project, self.song_time, self.is_playing, &mut self.status_message);
                            if actions.request_separation {
                                self.pending_stem_separation = true;
                                self.spawn_async_file_picker(
                                    "Ljudfiler",
                                    &[
                                        "wav", "WAV", "mp3", "MP3", "flac", "FLAC", "ogg", "OGG",
                                        "m4a", "aiff",
                                    ],
                                    "Välj mix att separera i stämspår",
                                );
                            }
                            if actions.export_stems {
                                self.export_separated_stems();
                            }
                            if let Some(idx) = actions.speed_changed {
                                self.apply_stem_time_stretch(idx);
                                self.status_message = crate::tstatus!("SPEED: tidssträckning tillämpad på stämspår {} (tonhöjd bevarad)", idx + 1);
                            }
                        }
                        ViewMode::PluginManager => {
                            let active_plugins: Vec<Option<String>> = self
                                .plugin_slots
                                .iter()
                                .map(|s| s.as_ref().map(|p| p.name.clone()))
                                .collect();
                            let stem_track_count = self
                                .stem_project
                                .stems
                                .len()
                                .max(self.playlist_tracks.len())
                                .max(active_plugins.len());
                            let gui_open: Vec<bool> = (0..active_plugins.len())
                                .map(|i| self.is_plugin_gui_open(i))
                                .collect();
                            #[cfg(feature = "plugin-host")]
                            let sandbox_status = self.sandbox_status_text();
                            #[cfg(not(feature = "plugin-host"))]
                            let sandbox_status: Option<String> = None;
                            let actions = render_plugins_view(
                                ui,
                                &mut self.plugin_manager,
                                &mut self.status_message,
                                stem_track_count,
                                self.selected_channel,
                                &active_plugins,
                                &gui_open,
                                sandbox_status.as_deref(),
                            );
                            if let Some((path, track)) = actions.load_into_track {
                                self.load_plugin_into_track(&path, track);
                            }
                            #[cfg(feature = "plugin-host")]
                            if let Some((path, track)) = actions.load_into_sandbox {
                                self.load_plugin_into_sandbox_track(&path, track);
                            }
                            #[cfg(not(feature = "plugin-host"))]
                            let _ = actions.load_into_sandbox;
                            if let Some((path, track, location)) = actions.load_preset_into_track {
                                self.load_plugin_preset_into_track(&path, track, &location);
                            }
                            if let Some(track) = actions.remove_track {
                                self.remove_plugin_from_track(track);
                            }
                            if let Some(track) = actions.open_gui {
                                self.open_plugin_gui(track);
                            }
                            if let Some(track) = actions.close_gui {
                                self.close_plugin_gui(track);
                            }
                            #[cfg(feature = "plugin-host")]
                            if let Some(path) = actions.sandbox_inspect {
                                self.sandbox_inspect(&path);
                            }
                            #[cfg(not(feature = "plugin-host"))]
                            let _ = actions.sandbox_inspect;
                        }
                        ViewMode::VocalStudio => {
                            render_vocal_studio_view(
                                ui,
                                &mut self.vocal_studio,
                                &mut self.vocal_harmonizer,
                                self.is_playing,
                                self.current_step,
                                self.bpm,
                                self.playlist_tracks.as_mut(),
                                &mut self.status_message,
                                &mut self.show_mic_settings_modal,
                                &mut self.engine,
                            );
                        }
                        ViewMode::AiMusicAssistant => {
                            self.refresh_ai_context();
                            render_ai_assistant_view(ui, &mut self.ai_assistant, &mut self.channels, &mut self.status_message);
                        }
                        ViewMode::EffectsMixer => {
                            self.render_effects_mixer_rack(ui);
                        }
                    }
                });
            });

        // 4. Modals & Dialogs
        self.render_import_sample_modal(ctx);
        self.render_hardware_controller_modal(ctx);
        self.render_batch_export_modal(ctx);
        self.render_suno_stem_import_modal(ctx);
        self.render_stem_import_progress_modal(ctx);
        self.render_project_load_progress_modal(ctx);
        self.render_about_modal(ctx);
        self.render_midi_import_modal(ctx);
        self.render_recovery_modal(ctx);
        self.render_project_manager_modal(ctx);
        self.render_ai_settings_modal(ctx);
        self.render_audio_settings_modal(ctx);
        self.render_mic_settings_modal(ctx);
        self.render_chord_generator_modal_view(ctx);
        self.render_tuner_modal_view(ctx);
        self.render_dice_generator_modal_view(ctx);
        self.render_fx_rack_modal_view(ctx);
        self.render_tempo_modal(ctx);
        self.render_song_structure_modal_view(ctx);
        self.render_stem_focus_modal(ctx);
        self.render_help_manual_modal(ctx);
        self.render_add_track_modal(ctx);

        // Screenshot automated capture loop
        if self.screenshot_mode_active {
            match self.screenshot_state.clone() {
                ScreenshotState::Idle => {
                    if !self.screenshot_queue.is_empty() {
                        let (target, dest) = self.screenshot_queue.remove(0);
                        self.apply_screenshot_target(target.clone());
                        self.screenshot_state = ScreenshotState::Preparing { target, dest, frames_left: 4 };
                        ctx.request_repaint();
                    } else {
                        println!("🎉 Alla 29 skärmdumpar har genererats och sparats framgångsrikt!");
                        self.screenshot_mode_active = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                ScreenshotState::Preparing { target, dest, frames_left } => {
                    if frames_left > 0 {
                        self.screenshot_state = ScreenshotState::Preparing { target, dest, frames_left: frames_left - 1 };
                        ctx.request_repaint();
                    } else {
                        self.screenshot_state = ScreenshotState::AwaitingCapture { dest };
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                        ctx.request_repaint();
                    }
                }
                ScreenshotState::AwaitingCapture { .. } => {
                    ctx.request_repaint();
                }
            }
        }
    }
}

impl SonixApp {
    fn render_browser_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("📁 SOUND BROWSER")).strong().size(13.0).color(Theme::FL_ORANGE));
                if self.sample_drag_item.is_some() {
                    ui.label(egui::RichText::new(crate::i18n::t("✋ Drar sample...")).size(10.0).color(Theme::FL_CYAN));
                }
            });
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("🔍"));
                ui.text_edit_singleline(&mut self.browser_search);
            });

            ui.separator();

            let mut to_delete_sample_id = None;
            let mut sample_to_add_timeline: Option<LibrarySampleItem> = None;
            let mut sample_to_vocal_studio: Option<LibrarySampleItem> = None;
            let mut sample_to_channel: Option<(usize, LibrarySampleItem)> = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Group library samples by category
                let mut categories_map: std::collections::BTreeMap<String, Vec<LibrarySampleItem>> = std::collections::BTreeMap::new();
                for item in &self.sample_library {
                    if !self.browser_search.is_empty() && !item.name.to_lowercase().contains(&self.browser_search.to_lowercase()) && !item.category.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                        continue;
                    }
                    categories_map.entry(item.category.clone()).or_default().push(item.clone());
                }

                // Render each category
                for (cat_name, items) in categories_map {
                    let (cat_icon, cat_color) = match cat_name.as_str() {
                        "Trumloopar" | "Drum Loops" => ("🥁", Theme::FL_ORANGE),
                        "Pads & Atmosfär" | "Pads & Atmosphere" => ("✨", Color32::from_rgb(100, 200, 255)),
                        "Bas & 808" | "Bass & 808" => ("🎸", Color32::from_rgb(255, 80, 140)),
                        "Keys & Melodier" | "Keys & Melodies" => ("🎹", Color32::from_rgb(255, 200, 100)),
                        "FX & Övergångar" | "FX & Transitions" => ("🌪", Color32::from_rgb(140, 220, 255)),
                        "Kicks" => ("💥", Theme::FL_ORANGE),
                        "Snares" => ("🥁", Theme::FL_CYAN),
                        "Claps" => ("👏", Theme::FL_CYAN),
                        "Hi-Hats" => ("🧂", Theme::FL_YELLOW),
                        "Percussion" => ("🪘", Color32::from_rgb(255, 140, 60)),
                        "Egna Samples" | "Egna Importerade" | "📂 Egna Samples" | "📂 My Samples" => ("⭐", Theme::FL_GREEN),
                        _ => ("🎵", Theme::FL_CYAN),
                    };

                    ui.collapsing(egui::RichText::new(format!("{} {} ({})", cat_icon, cat_name.to_uppercase(), items.len())).strong().color(cat_color), |ui| {
                        for item in &items {
                            let _item_id = ui.make_persistent_id(format!("browser_item_{}", item.id));
                            let (card_rect, card_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 4.0, 52.0), Sense::click_and_drag());

                            if card_resp.drag_started() || (card_resp.dragged() && self.sample_drag_item.is_none()) {
                                self.sample_drag_item = Some(item.clone());
                                self.status_message = crate::tstatus!("✋ Drar '{}' till tidslinjen... Släpp på önskat spår och takt!", item.name);
                            }

                            let is_hovered = card_resp.hovered();
                            let bg_col = if is_hovered { Color32::from_rgb(28, 34, 46) } else { Theme::PANEL_BG };
                            ui.painter().rect_filled(card_rect, Rounding::same(3.0), bg_col);
                            ui.painter().rect_stroke(card_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_hovered { item.color } else { Color32::from_rgb(32, 40, 52) }));

                            // Card Content
                            let mut card_ui = ui.new_child(egui::UiBuilder::new().max_rect(card_rect).layout(egui::Layout::top_down(egui::Align::Min)));
                            card_ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);

                            card_ui.horizontal(|ui| {
                                ui.label(&item.icon);
                                ui.label(egui::RichText::new(&item.name).strong().size(11.0).color(item.color));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let drag_btn = ui.add(
                                        egui::Button::new(egui::RichText::new(crate::i18n::t("✋ Dra")).size(9.5).color(Color32::WHITE))
                                            .fill(Color32::from_rgb(45, 55, 75))
                                            .sense(Sense::click_and_drag())
                                    ).on_hover_text(crate::i18n::t("Klicka & håll ned för att dra till tidslinjen"));
                                    if drag_btn.drag_started() || (drag_btn.dragged() && self.sample_drag_item.is_none()) {
                                        self.sample_drag_item = Some(item.clone());
                                        self.status_message = crate::tstatus!("✋ Drar '{}' till tidslinjen... Släpp på önskat spår och takt!", item.name);
                                    }
                                });
                            });

                            card_ui.horizontal_wrapped(|ui| {
                                if ui.button(egui::RichText::new(crate::i18n::t("▶")).size(9.5)).on_hover_text(crate::i18n::t("Provspela sample med äkta ljud")).clicked() {
                                    if let Some(ref path) = item.file_path && let Ok((l, r, sr)) = crate::audio::load_wav_pcm(path) {
                                        let _ = self.engine.send_command(AudioCommand::PlayAudition {
                                            left: std::sync::Arc::new(l),
                                            right: std::sync::Arc::new(r),
                                            sample_rate: sr as f32,
                                            volume: 0.95,
                                            pitch_ratio: 1.0,
                                            time_stretch_ratio: 1.0,
                                            is_reverse: false,
                                            loop_playback: false,
                                        });
                                    } else {
                                        match item.category.as_str() {
                                            "Kicks" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); },
                                            "Snares" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); },
                                            "Claps" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); },
                                            "Hi-Hats" => {
                                                if item.name.to_lowercase().contains("open") {
                                                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
                                                } else {
                                                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
                                                }
                                            },
                                            "Percussion" => {
                                                if item.name.to_lowercase().contains("high") {
                                                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomHigh));
                                                } else {
                                                    let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomLow));
                                                }
                                            },
                                            _ => {
                                                let note = item.default_note;
                                                let freq = midi_to_freq(note);
                                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: 0.9 });
                                            }
                                        }
                                    }
                                }
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Tidslinje")).size(9.5)).fill(Color32::from_rgb(30, 65, 90))).on_hover_text(crate::i18n::t("Skapa nytt spår på tidslinjen med denna sample")).clicked() {
                                    sample_to_add_timeline = Some(item.clone());
                                }
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sångstudio")).size(9.5)).fill(Color32::from_rgb(50, 45, 80))).on_hover_text(crate::i18n::t("Öppna i Sångstudion för isolerad solo-provspelning & formning")).clicked() {
                                    sample_to_vocal_studio = Some(item.clone());
                                }
                                if ui.button(egui::RichText::new(crate::i18n::t("🎛 Kanal")).size(9.0)).on_hover_text(crate::i18n::t("Ladda till Channel Rack")).clicked() {
                                    let ch_idx = self.active_sample_picker_channel.unwrap_or(0);
                                    sample_to_channel = Some((ch_idx, item.clone()));
                                }
                                if (item.category.starts_with("Egna") || item.category.starts_with("📂 My Samples")) && ui.button(egui::RichText::new(crate::i18n::t("🗑")).size(9.0)).on_hover_text(crate::i18n::t("Ta bort från bibliotek")).clicked() {
                                    to_delete_sample_id = Some(item.id);
                                }
                            });

                            ui.add_space(2.0);
                        }
                    });
                    ui.add_space(4.0);
                }

                if let Some(del_id) = to_delete_sample_id {
                    self.sample_library.retain(|s| s.id != del_id);
                }

                // Synth Presets
                ui.collapsing("🎹 SYNTH PRESETS", |ui| {
                    let synths = [
                        ("Clean Pluck (80s Poly)", Preset::CleanPluck),
                        ("Warm 80s Pad (Lush)", Preset::WarmPad),
                        ("303 Acid Bass (Resonant)", Preset::AcidBass),
                        ("8-Bit Chiptune (Retro)", Preset::ChiptuneLead),
                        ("Cosmic Brass (Synthwave)", Preset::CosmicBrass),
                    ];
                    for (name, p) in synths {
                        if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if ui.button(crate::i18n::t("▶")).clicked() {
                                self.current_preset = p;
                                let (wf, adsr, flt) = p.settings();
                                self.waveform = wf;
                                self.adsr = adsr;
                                self.filter = flt;
                                let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note: 60, freq: 261.63, velocity: 0.85 });
                            }
                            if ui.button(crate::i18n::t("Ladda")).clicked() {
                                self.current_preset = p;
                                let (wf, adsr, flt) = p.settings();
                                self.waveform = wf;
                                self.adsr = adsr;
                                self.filter = flt;
                                let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                                self.status_message = crate::tstatus!("Laddade preset: {}", name);
                            }
                            ui.label(name);
                        });
                    }
                });

                ui.add_space(4.0);

                // Groove Templates
                ui.collapsing("🎵 GROOVE INSPIRATION LOOPS", |ui| {
                    let grooves = [
                        ("Neon Synthwave (126 BPM)", 126.0, 0.15),
                        ("Cyberpunk Dark (128 BPM)", 128.0, 0.0),
                        ("Lo-Fi Chill Hop (88 BPM)", 88.0, 0.35),
                        ("House 4x4 (124 BPM)", 124.0, 0.10),
                    ];
                    for (name, bpm, swing) in grooves {
                        if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if ui.button(crate::i18n::t("⚡ Sätt BPM")).clicked() {
                                self.bpm = bpm;
                                self.swing = swing;
                                self.status_message = crate::tstatus!("Satte tempo till {} BPM med {:.0}% swing", bpm, swing * 100.0);
                            }
                            ui.label(name);
                        });
                    }
                });

                ui.add_space(4.0);

                // Sound FX
                ui.collapsing("🌊 SOUND FX & RISERS", |ui| {
                    let sfx = [
                        ("🌀 White Noise Riser", 60),
                        ("⚡ Laser Beam Zap", 72),
                        ("💥 Sub Impact Drop", 36),
                    ];
                    for (name, note) in sfx {
                        if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if ui.button(crate::i18n::t("▶")).clicked() {
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: midi_to_freq(note), velocity: 0.9 });
                            }
                            ui.label(name);
                        });
                    }
                });
            });

            if let Some(item) = sample_to_add_timeline {
                self.add_sample_item_to_timeline(&item);
            }
            if let Some(item) = sample_to_vocal_studio {
                self.open_sample_in_vocal_studio(&item);
            }
            if let Some((ch_idx, item)) = sample_to_channel {
                self.select_sound_for_channel(ch_idx, &item);
            }
        });
    }

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

    /// Feeds live project state into the AI assistant so prompts are grounded
    /// in the current key/tempo/selection (Etapp D).
    pub fn refresh_ai_context(&mut self) {
        const ROOT_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        const SCALE_NAMES: [&str; 5] = ["Dur", "Moll", "Dorian", "Blues", "Synthwave"];
        let root = (self.song_key_root % 12) as usize;
        let scale = self.song_key_scale.min(SCALE_NAMES.len() - 1);
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
            key_label: format!("{} {}", ROOT_NAMES[root], SCALE_NAMES[scale]),
            key_pc: Some(root as u8),
            is_minor: scale != 0,
            selected_track,
            selected_region,
        };
    }

    fn render_playlist_arranger(&mut self, ui: &mut egui::Ui) {
        let needs_mic = self.playlist_tracks.is_empty()
            || !self.playlist_tracks.iter().any(|t| {
                let n = t.name.to_lowercase();
                n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
            });
        if needs_mic {
            self.ensure_mic_track_exists();
        }

        ui.group(|ui| {
            // ================================================================
            // 1. ARRANGER TOP CONTROL & TRANSPORT BARS (2 Non-Overlapping Rows)
            // ================================================================
            // ROW 1: Project Pill, Stems Import, Capsule Transport & Clock, BPM (Responsive Wrapped)
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                // Project title pill
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("🎵 {}", self.project_name)).strong().size(11.5).color(Theme::TEXT_BRIGHT));
                        if ui.button(crate::i18n::t("•••")).on_hover_text(crate::i18n::t("Filhanterare & Projektinställningar")).clicked() {
                            self.show_project_manager_modal = true;
                        }
                    });
                });

                ui.separator();

                // Stems Importer Button
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📦 Stämmor (Stems)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
                    self.scan_for_suno_stems();
                    self.show_suno_import_modal = true;
                }

                ui.separator();

                // Centered Capsule Transport Bar (High Precision)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        // Level LED peak meter
                        let peak = self.engine.get_peak_level();
                        let (m_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 16.0), Sense::hover());
                        let h = (peak * 14.0).clamp(2.0, 14.0);
                        let bar_rect = Rect::from_min_max(Pos2::new(m_rect.min.x + 3.0, m_rect.max.y - h), Pos2::new(m_rect.max.x - 3.0, m_rect.max.y));
                        ui.painter().rect_filled(bar_rect, Rounding::same(1.0), Theme::FL_GREEN);

                        // Transport Buttons
                        if ui.button(crate::i18n::t("⏮")).on_hover_text(crate::i18n::t("Gå till start (00:00.00)")).clicked() {
                            self.seek_song_time(0.0);
                        }

                        let is_rec = self.is_recording_timeline || self.vocal_studio.is_recording;
                        let rec_col = if is_rec { Color32::from_rgb(255, 40, 40) } else { Color32::from_rgb(180, 30, 30) };
                        let rec_label = if is_rec { "⏹ REC" } else { "⏺ REC" };
                        if ui.add(egui::Button::new(egui::RichText::new(rec_label).size(10.5).strong().color(Color32::WHITE)).fill(rec_col)).clicked() {
                            self.toggle_timeline_recording();
                        }

                        let play_txt = if self.is_playing { "⏸" } else { "▶" };
                        let play_bg = if self.is_playing { Theme::FL_GREEN } else { Color32::from_rgb(38, 45, 55) };
                        if ui.add(egui::Button::new(egui::RichText::new(play_txt).strong().size(11.0).color(Color32::WHITE)).fill(play_bg)).clicked() {
                            self.toggle_playback();
                        }

                        let loop_bg = Color32::from_rgb(38, 45, 55);
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁")).size(10.0).color(Theme::FL_CYAN)).fill(loop_bg)).on_hover_text(crate::i18n::t("Sätt loop")).clicked() {
                            self.status_message = crate::tstatus!("Loop-region satt till Takt {}-{}", self.loop_start_bar + 1, self.loop_end_bar);
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(crate::i18n::t("⏱ Tempo"))
                                        .size(10.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(45, 40, 70)),
                            )
                            .on_hover_text(crate::i18n::t("Tempokarta — byten i låten"))
                            .clicked()
                        {
                            self.show_tempo_modal = true;
                        }

                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Mik")).size(10.5).strong().color(Color32::WHITE)).fill(Color32::from_rgb(30, 80, 110))).on_hover_text(crate::i18n::t("Mikrofoninställningar")).clicked() {
                            self.show_mic_settings_modal = true;
                        }

                        ui.separator();

                        // High Precision LCD Time Display (Hundredths of a second)
                        let time_str = format_time_hundredths(self.song_time);
                        let bar_float =
                            self.tempo_map().bar_at_secs(self.song_time as f64) as f32;
                        let bar_text = format_bar_subdivisions(bar_float);
                        let full_time_str = format!("⏱ {}  {}", time_str, bar_text);
                        ui.label(egui::RichText::new(full_time_str).monospace().strong().size(11.5).color(Theme::FL_CYAN));
                    });
                });

                ui.separator();

                // BPM & Tap-Tempo Pill
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        let (bpm_rect, bpm_resp) = ui.allocate_exact_size(Vec2::new(65.0, 20.0), Sense::click_and_drag());
                        if bpm_resp.dragged() {
                            let dy = bpm_resp.drag_delta().y;
                            self.bpm = (self.bpm - dy * 0.5).clamp(40.0, 280.0);
                        }
                        ui.painter().rect_filled(bpm_rect, Rounding::same(2.0), Color32::from_rgb(26, 32, 42));
                        ui.painter().text(
                            bpm_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{:.1} BPM", self.bpm),
                            egui::FontId::monospace(11.0),
                            Theme::TEXT_BRIGHT,
                        );

                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 TAP")).strong().size(10.0).color(Color32::WHITE)).fill(Color32::from_rgb(180, 70, 20))).on_hover_text(crate::i18n::t("Klicka i takt för att sätta tempo (Tap Tempo)")).clicked() {
                            self.tap_tempo();
                        }
                    });
                });

                ui.separator();

                // Key Signature Picker
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎼")).size(11.0));
                        let root_names = ["C", "C#", "D", "D#", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
                        let cur_root = root_names.get(self.song_key_root as usize % 12).unwrap_or(&"Eb");
                        egui::ComboBox::from_id_salt("arr_key_root")
                            .selected_text(*cur_root)
                            .width(38.0)
                            .show_ui(ui, |ui| {
                                for (idx, &r_name) in root_names.iter().enumerate() {
                                    if ui.selectable_label(self.song_key_root as usize == idx, r_name).clicked() {
                                        self.song_key_root = idx as u8;
                                        self.piano_roll_root_note = idx as u8;
                                        self.status_message = crate::tstatus!("🎼 Tonart ändrad till {}", r_name);
                                    }
                                }
                            });

                        let scale_names = ["Dur", "Moll", "Dorian", "Blues", "Synthwave"];
                        let cur_sc = scale_names.get(self.song_key_scale.min(4)).unwrap_or(&"Dur");
                        egui::ComboBox::from_id_salt("arr_key_scale")
                            .selected_text(*cur_sc)
                            .width(55.0)
                            .show_ui(ui, |ui| {
                                for (s_idx, &s_name) in scale_names.iter().enumerate() {
                                    if ui.selectable_label(self.song_key_scale == s_idx, s_name).clicked() {
                                        self.song_key_scale = s_idx;
                                        self.selected_scale = s_idx;
                                    }
                                }
                            });
                    });
                });

                ui.separator();

                // Time Signature & Metronome
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("arr_time_sig")
                            .selected_text(format!("{}/{}", self.time_signature.0, self.time_signature.1))
                            .width(42.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(self.time_signature == (4, 4), "4/4").clicked() { self.time_signature = (4, 4); }
                                if ui.selectable_label(self.time_signature == (3, 4), "3/4").clicked() { self.time_signature = (3, 4); }
                                if ui.selectable_label(self.time_signature == (6, 8), "6/8").clicked() { self.time_signature = (6, 8); }
                            });

                        let metro_bg = if self.metronome_enabled { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                        let metro_fg = if self.metronome_enabled { Color32::BLACK } else { Theme::TEXT_MUTED };
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔔 Metronom")).strong().size(10.0).color(metro_fg)).fill(metro_bg)).on_hover_text(crate::i18n::t("Hörbar klick-metronom vid uppspelning")).clicked() {
                            self.metronome_enabled = !self.metronome_enabled;
                            self.status_message = if self.metronome_enabled { crate::i18n::t("🔔 Metronom aktiverad").to_string() } else { crate::i18n::t("🔕 Metronom avstängd").to_string() };
                        }
                    });
                });
            });

            ui.add_space(2.0);

            // ROW 2: Arranger Tools, Quick Slicing / Action Buttons, Snap Grid, Follow Playhead, and Zoom Controls (Responsive Wrapped)
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                // Arranger Tools Selector (Select, Paint, Slice ✂, Mute, Erase)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        let tools = [
                            (ArrangerTool::Select, crate::i18n::t("⇱ Välj (1)")),
                            (ArrangerTool::Paint, crate::i18n::t("✎ Rita (2)")),
                            (ArrangerTool::Slice, crate::i18n::t("✂ Klipp (3)")),
                            (ArrangerTool::Mute, "🔇 Muta (4)"),
                            (ArrangerTool::Erase, "🗑 Radera (5)"),
                        ];
                        for (tool_val, tool_label) in tools {
                            let is_sel = std::mem::discriminant(&self.arranger_tool) == std::mem::discriminant(&tool_val);
                            let btn_bg = if is_sel {
                                if matches!(tool_val, ArrangerTool::Slice) { Color32::from_rgb(255, 120, 30) } else { Theme::FL_ORANGE }
                            } else {
                                Color32::from_rgb(32, 36, 44)
                            };
                            let btn_fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(tool_label).strong().size(10.5).color(btn_fg)).fill(btn_bg)).clicked() {
                                self.arranger_tool = tool_val;
                            }
                        }
                    });
                });

                // Direct Quick-Cut Action Button (Always available!)
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Klipp vid spelhuvud (Ctrl+B / S)")).strong().size(10.5).color(Color32::BLACK)).fill(Theme::FL_GREEN))
                    .on_hover_text(crate::i18n::t("Dela vald region exakt vid den nuvarande tidsmarkören (Genväg: Ctrl+B eller S)"))
                    .clicked() {
                        self.split_selected_region_at_playhead();
                    }

                ui.separator();

                // Snap Selector (0.01s Precision, 1/16, Beat, Bar)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Snäpp:")).size(10.0).color(Theme::TEXT_MUTED));
                        let snaps = [
                            (TimeSnapMode::FreeHundredth, "⚡ 0.01s"),
                            (TimeSnapMode::Snap16th, "1/16"),
                            (TimeSnapMode::SnapBeat, "Beat"),
                            (TimeSnapMode::SnapBar, crate::i18n::t("Takt")),
                        ];
                        for (mode, label) in snaps {
                            let is_sel = self.timeline_snap_mode == mode;
                            let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(28, 32, 40) };
                            let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(label).strong().size(10.0).color(fg)).fill(bg)).clicked() {
                                self.timeline_snap_mode = mode;
                            }
                        }
                    });
                });

                ui.separator();

                // Follow Playhead Toggle
                let auto_label = if self.timeline_auto_scroll { crate::i18n::t("🏃 Följ: PÅ") } else { crate::i18n::t("⏸ Följ: AV") };
                let auto_bg = if self.timeline_auto_scroll { Theme::FL_GREEN } else { Color32::from_rgb(36, 42, 54) };
                let auto_fg = if self.timeline_auto_scroll { Color32::BLACK } else { Theme::TEXT_MUTED };
                let auto_tip = if self.timeline_auto_scroll {
                    crate::i18n::t("Tidslinjen rullar automatiskt med spelhuvudet under uppspelning. Klicka för att stänga av.")
                } else {
                    crate::i18n::t("Tidslinjen står stilla så du kan redigera i lugn och ro. Klicka för att aktivera följning.")
                };
                if ui.add(egui::Button::new(egui::RichText::new(auto_label).strong().size(10.5).color(auto_fg)).fill(auto_bg)).on_hover_text(auto_tip).clicked() {
                    self.timeline_auto_scroll = !self.timeline_auto_scroll;
                    self.status_message = crate::tstatus!("Följ spelhuvud / Autoscroll: {}", if self.timeline_auto_scroll { "AKTIVERAD" } else { "INAKTIVERAD" });
                }

                ui.separator();

                // Automation Curves Toggle + Parameter Selector (Fas 5.4)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        let auto_on = self.show_automation;
                        let auto_lbl = if auto_on { crate::i18n::t("📈 Automation: PÅ") } else { crate::i18n::t("📈 Automation: AV") };
                        let auto_bg = if auto_on { Theme::FL_CYAN } else { Color32::from_rgb(36, 42, 54) };
                        let auto_fg = if auto_on { Color32::BLACK } else { Theme::TEXT_MUTED };
                        if ui.add(egui::Button::new(egui::RichText::new(auto_lbl).strong().size(10.5).color(auto_fg)).fill(auto_bg))
                            .on_hover_text(crate::i18n::t("Visa/redigera automationskurvor för valt spår. Vänsterklicka i kurvan för att lägga till punkter, dra för att flytta, högerklicka för att ta bort."))
                            .clicked()
                        {
                            self.show_automation = !self.show_automation;
                            self.status_message = crate::tstatus!("Automationskurvor: {}", if self.show_automation { "AKTIVERADE" } else { "INAKTIVERADE" });
                        }
                        if auto_on {
                            for p in AutomationParam::ALL {
                                let sel = self.automation_param == p;
                                let pbg = if sel { Theme::FL_ORANGE } else { Color32::from_rgb(28, 32, 40) };
                                let pfg = if sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                                if ui.add(egui::Button::new(egui::RichText::new(p.label()).strong().size(10.0).color(pfg)).fill(pbg)).clicked() {
                                    self.automation_param = p;
                                }
                            }
                        }
                    });
                });

                ui.separator();

                // Zoom Controls Group (Now part of wrapped row, no right_to_left collision!)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t("🔍+")).on_hover_text(crate::i18n::t("Zooma in (Ctrl+Skrolla)")).clicked() {
                            self.suno_zoom_level = (self.suno_zoom_level * 1.25).min(20.0);
                        }
                        let zoom_presets = [(0.5, "50%"), (1.0, "100%"), (2.0, "200%"), (4.0, "400%"), (8.0, "800%")];
                        for (z_val, z_lbl) in zoom_presets {
                            if ui.selectable_label((self.suno_zoom_level - z_val).abs() < 0.1, z_lbl).clicked() {
                                self.suno_zoom_level = z_val;
                            }
                        }
                        if ui.button(crate::i18n::t("🔍-")).on_hover_text(crate::i18n::t("Zooma ut (Ctrl+Skrolla)")).clicked() {
                            self.suno_zoom_level = (self.suno_zoom_level * 0.8).max(0.25);
                        }
                        ui.label(egui::RichText::new(format!("{:.0}%", self.suno_zoom_level * 100.0)).monospace().size(10.0).color(Theme::TEXT_MUTED));
                    });
                });
            });

            ui.add_space(4.0);

            // ================================================================
            // 2. SUNO STUDIO ROLLING TIMELINE CANVAS & MULTI-TIER RULER
            // ================================================================
            // Handle Ctrl + Mouse Wheel Smooth Zoom
            if ui.input(|i| i.modifiers.ctrl && i.raw_scroll_delta.y != 0.0) {
                let delta = ui.input(|i| i.raw_scroll_delta.y);
                let zoom_mult = if delta > 0.0 { 1.15 } else { 0.87 };
                self.suno_zoom_level = (self.suno_zoom_level * zoom_mult).clamp(0.25, 20.0);
            }

            let header_w = 210.0;
            let bar_w = (60.0 * self.suno_zoom_level).clamp(16.0, 1500.0);
            let row_h = 46.0;
            let ruler_h = 32.0;
            let sec_per_bar = 60.0 / self.bpm * 4.0;
            let px_per_sec = bar_w / sec_per_bar;

            // Calculate max song bars based on audio regions
            let max_region_end = self.playlist_tracks.iter()
                .flat_map(|t| t.regions.iter())
                .map(|r| r.start_bar + r.length_bars)
                .fold(32.0_f32, |m, b| m.max(b));
            let total_bars = ((max_region_end + 8.0).ceil() as usize).clamp(32, 1024);
            let timeline_total_w = total_bars as f32 * bar_w;

            let mut scrub_target_sec: Option<f32> = None;
            let mut drop_sample_action: Option<(usize, LibrarySampleItem, f32)> = None;
            let mut track_delete_idx: Option<usize> = None;
            let mut track_duplicate_idx: Option<usize> = None;
            let mut track_paste_idx: Option<usize> = None;

            let num_tracks = self.playlist_tracks.len();

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // ================================================================
                // 2.A FIXED LEFT TRACK HEADERS (Sticky / Frozen Column)
                // ================================================================
                ui.vertical(|ui| {
                    ui.set_width(header_w);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    // Corner Track Header Spacer
                    let (corner_rect, _) = ui.allocate_exact_size(Vec2::new(header_w, ruler_h), Sense::hover());
                    ui.painter().rect_filled(corner_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 26));
                    ui.painter().text(
                        Pos2::new(corner_rect.min.x + 10.0, corner_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        crate::i18n::t("SPÅR / INSTRUMENT"),
                        egui::FontId::proportional(11.0),
                        Theme::TEXT_MUTED,
                    );

                    ui.add_space(2.0);

                    // Track Header Cards (Spår 1 .. N)
                    for t_idx in 0..self.playlist_tracks.len() {
                        let is_sel_track = self.selected_timeline_track == t_idx;
                        let track_color = self.playlist_tracks[t_idx].color;
                        let track_muted = self.playlist_tracks[t_idx].muted;
                        let track_solo = self.playlist_tracks[t_idx].solo;
                        let track_vol = self.playlist_tracks[t_idx].volume;
                        let track_name = self.playlist_tracks[t_idx].name.clone();
                        let icon = self.playlist_tracks[t_idx].icon;
                        let is_vocal_track = self.playlist_tracks[t_idx].kind == TrackKind::VocalAudio;
                        let is_mic_track = track_name.to_lowercase().contains("mic")
                            || track_name.to_lowercase().contains("mikrofon")
                            || track_name.to_lowercase().contains("microphone")
                            || (t_idx == self.playlist_tracks.len() - 1 && track_name.to_lowercase().contains("mik"));
                        let track_row_h = if is_mic_track { row_h * 2.0 } else { row_h };

                        // Left Track Header Card (Sonix Style)
                        let (h_rect, h_resp) = ui.allocate_exact_size(Vec2::new(header_w, track_row_h), Sense::click());
                        let card_bg = if is_mic_track {
                            if is_sel_track { Color32::from_rgb(255, 255, 255) } else { Color32::from_rgb(248, 250, 254) }
                        } else if is_sel_track {
                            Color32::from_rgb(28, 32, 42)
                        } else {
                            Color32::from_rgb(18, 21, 28)
                        };
                        let card_stroke = if is_mic_track {
                            if is_sel_track { Color32::from_rgb(0, 180, 240) } else { Color32::from_rgb(205, 215, 228) }
                        } else if is_sel_track {
                            track_color
                        } else {
                            Color32::from_rgb(34, 40, 52)
                        };
                        ui.painter().rect_filled(h_rect, Rounding::same(3.0), card_bg);
                        ui.painter().rect_stroke(h_rect, Rounding::same(3.0), Stroke::new(if is_sel_track { 1.5_f32 } else { 1.0_f32 }, card_stroke));

                        // Left Accent Bar
                        let accent_color = if is_mic_track { Color32::from_rgb(0, 180, 240) } else { track_color };
                        let color_bar = Rect::from_min_size(h_rect.min, Vec2::new(4.0, h_rect.height()));
                        ui.painter().rect_filled(color_bar, Rounding::same(1.5), accent_color);

                        // Track Number & Name
                        let display_name = if is_mic_track {
                            format!("{} 🎤 MIKROFON (Huvudinspelning)", t_idx + 1)
                        } else if track_name.starts_with(icon) {
                            format!("{} {}", t_idx + 1, track_name)
                        } else {
                            format!("{} {} {}", t_idx + 1, icon, track_name)
                        };
                        let title_col = if is_mic_track {
                            Color32::from_rgb(15, 20, 28)
                        } else if is_sel_track {
                            Color32::WHITE
                        } else {
                            track_color
                        };
                        ui.painter().text(
                            Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + if is_mic_track { 16.0 } else { 12.0 }),
                            egui::Align2::LEFT_CENTER,
                            display_name,
                            egui::FontId::proportional(if is_mic_track { 11.5 } else { 11.0 }),
                            title_col,
                        );

                        let ctrl_y_offset = if is_mic_track { 22.0 } else { 0.0 };

                        // Live Input Meter for Mic/Vocal tracks or armed tracks
                        let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                        if is_vocal_track || is_armed || is_mic_track {
                            let vu = self.vocal_studio.mic_vu_level;
                            let vu_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 116.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(20.0, if is_mic_track { 28.0 } else { 18.0 }));
                            ui.painter().rect_filled(vu_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(220, 226, 238) } else { Color32::from_rgb(22, 26, 34) });
                            let vu_h = (vu * (if is_mic_track { 24.0 } else { 14.0 })).clamp(2.0, if is_mic_track { 24.0 } else { 14.0 });
                            let vu_fill = Rect::from_min_max(
                                Pos2::new(vu_rect.min.x + 4.0, vu_rect.max.y - 2.0 - vu_h),
                                Pos2::new(vu_rect.max.x - 4.0, vu_rect.max.y - 2.0)
                            );
                            let vu_col = if vu > 0.85 { Color32::from_rgb(255, 60, 60) } else if vu > 0.5 { Color32::from_rgb(255, 200, 40) } else { Color32::from_rgb(0, 190, 240) };
                            ui.painter().rect_filled(vu_fill, Rounding::same(1.0), vu_col);
                        }

                        // Controls Row inside header (Vol Slider, Pan Slider, Mute, Solo, Record Arm R, Focus Editor 🔍)
                        let vol_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 8.0, h_rect.min.y + 26.0 + ctrl_y_offset), Vec2::new(38.0, 12.0));
                        ui.painter().rect_filled(vol_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(210, 218, 230) } else { Color32::from_rgb(30, 35, 45) });
                        let vol_fill_w = vol_rect.width() * (track_vol / 1.25).clamp(0.0, 1.0);
                        ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), accent_color);
                        ui.painter().text(vol_rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", track_vol * 100.0), egui::FontId::proportional(8.0), Color32::WHITE);

                        // Pan Slider
                        let pan_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 50.0, h_rect.min.y + 26.0 + ctrl_y_offset), Vec2::new(36.0, 12.0));
                        ui.painter().rect_filled(pan_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(210, 218, 230) } else { Color32::from_rgb(26, 30, 40) });
                        let track_pan = self.playlist_tracks[t_idx].pan;
                        let pan_norm = ((track_pan + 1.0) / 2.0).clamp(0.0, 1.0);
                        let center_x = pan_rect.center().x;
                        let thumb_x = pan_rect.min.x + pan_norm * pan_rect.width();
                        let pan_fill_rect = if thumb_x >= center_x {
                            Rect::from_min_max(Pos2::new(center_x, pan_rect.min.y + 2.0), Pos2::new(thumb_x, pan_rect.max.y - 2.0))
                        } else {
                            Rect::from_min_max(Pos2::new(thumb_x, pan_rect.min.y + 2.0), Pos2::new(center_x, pan_rect.max.y - 2.0))
                        };
                        ui.painter().rect_filled(pan_fill_rect, Rounding::same(1.0), Theme::FL_CYAN);
                        let pan_str = if track_pan.abs() < 0.05 { "C".to_string() } else if track_pan < 0.0 { format!("L{:.0}", track_pan.abs() * 50.0) } else { format!("R{:.0}", track_pan * 50.0) };
                        ui.painter().text(pan_rect.center(), egui::Align2::CENTER_CENTER, pan_str, egui::FontId::proportional(8.0), if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                        // Mute icon (speaker)
                        let m_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 90.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                        let m_bg = if track_muted { Color32::from_rgb(180, 40, 40) } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                        ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
                        ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(8.5), if track_muted { Color32::WHITE } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                        // Solo (S)
                        let s_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 68.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                        let s_bg = if track_solo { Theme::FL_ORANGE } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                        ui.painter().rect_filled(s_rect, Rounding::same(2.0), s_bg);
                        ui.painter().text(s_rect.center(), egui::Align2::CENTER_CENTER, "S", egui::FontId::proportional(9.0), if track_solo { Color32::BLACK } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                        // Record Arm (R)
                        let r_arm_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 46.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                        let r_arm_bg = if is_armed { Color32::from_rgb(220, 30, 30) } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                        ui.painter().rect_filled(r_arm_rect, Rounding::same(2.0), r_arm_bg);
                        ui.painter().text(r_arm_rect.center(), egui::Align2::CENTER_CENTER, "R", egui::FontId::proportional(9.0), if is_armed { Color32::WHITE } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                        // Stem Focus / Detail Editor button (🔍)
                        let f_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 24.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                        let is_focused = self.show_stem_focus_modal && self.focused_stem_track == Some(t_idx);
                        let f_bg = if is_focused { Theme::FL_CYAN } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                        ui.painter().rect_filled(f_rect, Rounding::same(2.0), f_bg);
                        ui.painter().text(f_rect.center(), egui::Align2::CENTER_CENTER, "🔍", egui::FontId::proportional(8.5), if is_focused { Color32::BLACK } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                        if h_resp.clicked() || h_resp.double_clicked() {
                            ui.memory_mut(|m| m.stop_text_input());
                            self.selected_timeline_track = t_idx;
                            if !self.playlist_tracks[t_idx].regions.is_empty() {
                                self.selected_audio_region = Some((t_idx, 0));
                                let reg = &self.playlist_tracks[t_idx].regions[0];
                                self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(reg.start_bar * sec_per_bar), format_time_hundredths(reg.length_bars * sec_per_bar));
                            }
                            if let Some(mouse_pos) = h_resp.hover_pos() {
                                let vu_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 116.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(20.0, if is_mic_track { 28.0 } else { 18.0 }));
                                if (is_vocal_track || is_armed || is_mic_track) && vu_rect.contains(mouse_pos) {
                                    self.show_mic_settings_modal = true;
                                } else if m_rect.contains(mouse_pos) {
                                    self.playlist_tracks[t_idx].muted = !self.playlist_tracks[t_idx].muted;
                                    self.sync_track_audio_state(t_idx);
                                } else if s_rect.contains(mouse_pos) {
                                    self.playlist_tracks[t_idx].solo = !self.playlist_tracks[t_idx].solo;
                                    self.sync_track_audio_state(t_idx);
                                } else if r_arm_rect.contains(mouse_pos) {
                                    self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                                    if self.playlist_tracks[t_idx].is_rec_armed {
                                        self.status_message = crate::tstatus!("🔴 Spår {} ({}) är nu armerat för mikrofoninspelning!", t_idx + 1, self.playlist_tracks[t_idx].name);
                                    }
                                } else if f_rect.contains(mouse_pos) || h_resp.double_clicked() {
                                    self.open_stem_focus(t_idx);
                                } else if vol_rect.contains(mouse_pos) {
                                    let ratio = (mouse_pos.x - vol_rect.min.x) / vol_rect.width();
                                    self.playlist_tracks[t_idx].volume = (ratio * 1.25).clamp(0.0, 1.25);
                                } else if pan_rect.contains(mouse_pos) {
                                    let ratio = (mouse_pos.x - pan_rect.min.x) / pan_rect.width();
                                    self.playlist_tracks[t_idx].pan = (ratio * 2.0 - 1.0).clamp(-1.0, 1.0);
                                    self.sync_track_audio_state(t_idx);
                                }
                            }
                        }

                        h_resp.context_menu(|ui| {
                            ui.set_min_width(230.0);
                            ui.label(egui::RichText::new(crate::tstatus!("Spår {}: {}", t_idx + 1, self.playlist_tracks[t_idx].name)).strong().color(Theme::FL_CYAN));
                            ui.separator();
                            if ui.button(crate::i18n::t("📋 Duplicera spår (under detta spår)")).clicked() {
                                track_duplicate_idx = Some(t_idx);
                                ui.close_menu();
                            }
                            if self.copied_region.is_some() {
                                if ui.button(crate::i18n::t("📋 Klistra in sample här (Ctrl+V)")).clicked() {
                                    track_paste_idx = Some(t_idx);
                                    ui.close_menu();
                                }
                            }
                            if ui.button(crate::i18n::t("➕ Lägg till nytt spår...")).clicked() {
                                self.show_add_track_modal = true;
                                ui.close_menu();
                            }
                            if ui.button(crate::i18n::t("🔴 Spela in på detta spår")).clicked() {
                                for t in &mut self.playlist_tracks { t.is_rec_armed = false; }
                                self.playlist_tracks[t_idx].is_rec_armed = true;
                                self.toggle_timeline_recording();
                                ui.close_menu();
                            }
                            if !is_mic_track && self.playlist_tracks.len() > 1 {
                                ui.separator();
                                if ui.button(egui::RichText::new(crate::i18n::t("🗑 Ta bort spår")).color(Color32::from_rgb(255, 90, 90))).clicked() {
                                    track_delete_idx = Some(t_idx);
                                    ui.close_menu();
                                }
                            }
                        });

                        ui.add_space(2.0);
                    }

                    // "+ Add Track" Action Row on Left
                    let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(header_w, 28.0), Sense::click());
                    let is_h = add_resp.hovered();
                    ui.painter().rect_filled(add_rect, Rounding::same(4.0), if is_h { Color32::from_rgb(32, 38, 50) } else { Color32::from_rgb(20, 24, 32) });
                    ui.painter().rect_stroke(add_rect, Rounding::same(4.0), Stroke::new(1.0_f32, if is_h { Theme::FL_ORANGE } else { Color32::from_rgb(45, 52, 66) }));
                    ui.painter().text(add_rect.center(), egui::Align2::CENTER_CENTER, crate::i18n::t("➕ Lägg till spår"), egui::FontId::proportional(11.0), if is_h { Theme::FL_ORANGE } else { Theme::TEXT_BRIGHT });
                    if add_resp.clicked() {
                        self.show_add_track_modal = true;
                    }

                    ui.add_space(4.0);

                    // Master "Main" Header Card on Left
                    let (m_header_rect, m_header_resp) = ui.allocate_exact_size(Vec2::new(header_w, row_h), Sense::click_and_drag());
                    let is_master_muted = self.master_volume < 0.001;
                    let card_bg = Color32::from_rgb(18, 22, 32);
                    ui.painter().rect_filled(m_header_rect, Rounding::same(3.0), card_bg);
                    ui.painter().rect_stroke(m_header_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Theme::FL_ORANGE));

                    // Left Accent Bar (Orange for Master)
                    let color_bar = Rect::from_min_size(m_header_rect.min, Vec2::new(4.0, m_header_rect.height()));
                    ui.painter().rect_filled(color_bar, Rounding::same(1.5), Theme::FL_ORANGE);

                    // "Main" Track Label
                    ui.painter().text(
                        Pos2::new(m_header_rect.min.x + 10.0, m_header_rect.min.y + 12.0),
                        egui::Align2::LEFT_CENTER,
                        "🎛 Main Master",
                        egui::FontId::proportional(11.0),
                        Theme::FL_ORANGE,
                    );

                    // Controls Row: Master Volume Slider & Mute Speaker Icon
                    let vol_rect = Rect::from_min_size(Pos2::new(m_header_rect.min.x + 10.0, m_header_rect.min.y + 26.0), Vec2::new(125.0, 12.0));
                    ui.painter().rect_filled(vol_rect, Rounding::same(2.0), Color32::from_rgb(30, 35, 45));
                    let vol_fill_w = vol_rect.width() * (self.master_volume / 1.25).clamp(0.0, 1.0);
                    ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), Theme::FL_ORANGE);

                    // Mute speaker button
                    let m_rect = Rect::from_min_size(Pos2::new(m_header_rect.max.x - 26.0, m_header_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                    let m_bg = if is_master_muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 35, 45) };
                    ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
                    ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(9.0), if is_master_muted { Color32::WHITE } else { Theme::TEXT_MUTED });

                    if (m_header_resp.clicked() || m_header_resp.dragged())
                        && let Some(mouse_pos) = m_header_resp.hover_pos() {
                            if m_rect.contains(mouse_pos) && m_header_resp.clicked() {
                                if is_master_muted {
                                    self.master_volume = 0.85;
                                } else {
                                    self.master_volume = 0.0;
                                }
                                let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                            } else if vol_rect.contains(mouse_pos) || (mouse_pos.x >= vol_rect.min.x && mouse_pos.x <= vol_rect.max.x && mouse_pos.y >= m_header_rect.min.y + 18.0) {
                                let ratio = ((mouse_pos.x - vol_rect.min.x) / vol_rect.width()).clamp(0.0, 1.0);
                                self.master_volume = (ratio * 1.25).clamp(0.0, 1.25);
                                let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                            }
                        }
                });

                // Vertical Divider Line between Left Frozen Column and Right Timeline ScrollArea
                ui.painter().line_segment(
                    [Pos2::new(ui.cursor().min.x, ui.cursor().min.y), Pos2::new(ui.cursor().min.x, ui.cursor().min.y + (num_tracks as f32 * 50.0).max(200.0))],
                    Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 64)),
                );

                // ================================================================
                // 2.B HORIZONTALLY SCROLLABLE TIMELINE & WAVEFORM LANES
                // ================================================================
                egui::ScrollArea::horizontal()
                    .id_salt("sonix_arranger_timeline_lanes_scroll")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;

                            // 1. Timeline Ruler Strip
                            let (ruler_rect, ruler_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, ruler_h), Sense::click_and_drag());
                            let timeline_left_x = ruler_rect.min.x;
                            // Högerklick på linjalen: sätt eller ta bort ett tempobyte
                            // (Fas 8.2). Taktnumret räknas ut HÄR, på klicket — inne i
                            // menyn står pekaren över menyn, inte över linjalen, och då
                            // hade bytet hamnat på fel takt.
                            if ruler_resp.secondary_clicked()
                                && let Some(pos) = ruler_resp.interact_pointer_pos()
                            {
                                let bar =
                                    (((pos.x - ruler_rect.min.x) / bar_w).floor().max(0.0)) as u32;
                                self.tempo_menu_bar = Some(bar);
                            }
                            let mut tempo_set_here = false;
                            let mut tempo_remove_here = false;
                            ruler_resp.clone().context_menu(|ui| {
                                let bar = self.tempo_menu_bar.unwrap_or(0);
                                ui.label(crate::tstatus!("Takt {}", bar + 1));
                                if ui.button(crate::i18n::t("⏱ Sätt tempo här")).clicked() {
                                    tempo_set_here = true;
                                    ui.close_menu();
                                }
                                let has_point =
                                    self.tempo_points.iter().any(|p| p.start_bar == bar);
                                if has_point
                                    && ui
                                        .button(crate::i18n::t("🗑 Ta bort tempobyte här"))
                                        .clicked()
                                {
                                    tempo_remove_here = true;
                                    ui.close_menu();
                                }
                            });
                            if tempo_set_here {
                                let bar = self.tempo_menu_bar.unwrap_or(0);
                                let bpm = self.tempo_map().bpm_at(bar as f64);
                                self.set_tempo_point(bar, bpm);
                            } else if tempo_remove_here {
                                self.remove_tempo_point(self.tempo_menu_bar.unwrap_or(0));
                            }
                            ui.painter().rect_filled(ruler_rect, Rounding::ZERO, Color32::from_rgb(16, 19, 26));
                            ui.painter().line_segment([Pos2::new(ruler_rect.min.x, ruler_rect.max.y), Pos2::new(ruler_rect.max.x, ruler_rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)));

                            // Highlight loop region on ruler
                            let loop_start_x = ruler_rect.min.x + self.loop_start_bar as f32 * bar_w;
                            let loop_end_x = ruler_rect.min.x + self.loop_end_bar as f32 * bar_w;
                            let loop_rect = Rect::from_min_max(Pos2::new(loop_start_x, ruler_rect.min.y), Pos2::new(loop_end_x, ruler_rect.max.y));
                            ui.painter().rect_filled(loop_rect, Rounding::ZERO, Color32::from_rgb(26, 36, 52));

                            // Song section markers (created by the Song Structure arranger)
                            for marker in &self.song_markers {
                                let mx = ruler_rect.min.x + marker.start_bar as f32 * bar_w;
                                let mw = (marker.length_bars as f32 * bar_w).max(2.0);
                                let band = Rect::from_min_size(
                                    Pos2::new(mx, ruler_rect.max.y - 6.0),
                                    Vec2::new(mw, 6.0),
                                );
                                ui.painter().rect_filled(band, Rounding::ZERO, marker.color);
                                ui.painter().line_segment(
                                    [Pos2::new(mx, ruler_rect.min.y), Pos2::new(mx, ruler_rect.max.y)],
                                    Stroke::new(1.5_f32, marker.color),
                                );
                                if mw > 28.0 {
                                    ui.painter().text(
                                        Pos2::new(mx + 4.0, ruler_rect.max.y - 8.0),
                                        egui::Align2::LEFT_BOTTOM,
                                        &marker.name,
                                        egui::FontId::proportional(9.0),
                                        marker.color,
                                    );
                                }
                            }

                            // Draw dynamic tick marks & time codes
                            for bar_idx in 0..total_bars {
                                let bar_start_x = ruler_rect.min.x + bar_idx as f32 * bar_w;
                                let bar_time_sec = bar_idx as f32 * sec_per_bar;

                                // Major Bar Tick Line
                                ui.painter().line_segment(
                                    [Pos2::new(bar_start_x, ruler_rect.min.y), Pos2::new(bar_start_x, ruler_rect.max.y)],
                                    Stroke::new(1.0_f32, Color32::from_rgb(65, 75, 95)),
                                );

                                // Major Bar Number Label
                                ui.painter().text(
                                    Pos2::new(bar_start_x + 4.0, ruler_rect.min.y + 10.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}", bar_idx + 1),
                                    egui::FontId::proportional(11.0),
                                    Theme::FL_CYAN,
                                );

                                // Major Time Code (MM:SS.cs)
                                ui.painter().text(
                                    Pos2::new(bar_start_x + 4.0, ruler_rect.min.y + 22.0),
                                    egui::Align2::LEFT_CENTER,
                                    format_time_hundredths(bar_time_sec),
                                    egui::FontId::proportional(9.0),
                                    Theme::TEXT_MUTED,
                                );

                                // Beat Ticks (Rendered when bar_w >= 70.0)
                                if bar_w >= 70.0 {
                                    for beat in 1..4 {
                                        let bx = bar_start_x + beat as f32 * (bar_w / 4.0);
                                        ui.painter().line_segment(
                                            [Pos2::new(bx, ruler_rect.min.y + 14.0), Pos2::new(bx, ruler_rect.max.y)],
                                            Stroke::new(0.8_f32, Color32::from_rgb(45, 52, 68)),
                                        );
                                        if bar_w >= 140.0 {
                                            ui.painter().text(
                                                Pos2::new(bx + 2.0, ruler_rect.min.y + 22.0),
                                                egui::Align2::LEFT_CENTER,
                                                format!(".{}", beat + 1),
                                                egui::FontId::proportional(8.5),
                                                Color32::from_rgb(110, 120, 140),
                                            );
                                        }
                                    }
                                }

                                // 16th-Step Ticks (Rendered when bar_w >= 220.0)
                                if bar_w >= 220.0 {
                                    for step in 1..16 {
                                        if step % 4 != 0 {
                                            let sx = bar_start_x + step as f32 * (bar_w / 16.0);
                                            ui.painter().line_segment(
                                                [Pos2::new(sx, ruler_rect.min.y + 22.0), Pos2::new(sx, ruler_rect.max.y)],
                                                Stroke::new(0.5_f32, Color32::from_rgb(38, 44, 56)),
                                            );
                                        }
                                    }
                                }

                                // Sub-second / Hundredth Precision Ticks (Rendered when bar_w >= 500.0)
                                if bar_w >= 500.0 {
                                    let tenths = (sec_per_bar * 10.0) as usize;
                                    for t in 1..tenths {
                                        let tx = bar_start_x + (t as f32 * 0.10) * px_per_sec;
                                        if tx < bar_start_x + bar_w - 2.0 {
                                            ui.painter().line_segment(
                                                [Pos2::new(tx, ruler_rect.min.y + 25.0), Pos2::new(tx, ruler_rect.max.y)],
                                                Stroke::new(0.5_f32, Color32::from_rgb(70, 80, 100)),
                                            );
                                        }
                                    }
                                }
                            }

                            // Ruler Interaction: Click / Drag / Scrub with high hundredth-second precision
                            if ruler_resp.clicked() || ruler_resp.dragged() {
                                ui.memory_mut(|m| m.stop_text_input());
                                if let Some(mouse_pos) = ruler_resp.hover_pos() {
                                    let click_bar = (mouse_pos.x - ruler_rect.min.x) / bar_w;
                                    // Snäppningen sker i TAKTER, inte i sekunder: ett
                                    // 16-delssteg är en plats i takten, och över ett
                                    // tempobyte finns inget enda "sekunder per takt" att
                                    // räkna steg med. Sekunderna kommer ur kartan på ett
                                    // ställe, efteråt.
                                    let tempo = self.tempo_map();
                                    let target_sec = match self.timeline_snap_mode {
                                        TimeSnapMode::FreeHundredth => {
                                            // Hundradelar är en TID, inte en plats:
                                            // där är sekunden själv enheten.
                                            let raw_sec = tempo.secs_at_bar(click_bar as f64) as f32;
                                            (raw_sec * 100.0).round() / 100.0
                                        }
                                        TimeSnapMode::Snap16th => {
                                            let bar = (click_bar * 16.0).round() / 16.0;
                                            tempo.secs_at_bar(bar as f64) as f32
                                        }
                                        TimeSnapMode::SnapBeat => {
                                            let bar = (click_bar * 4.0).round() / 4.0;
                                            tempo.secs_at_bar(bar as f64) as f32
                                        }
                                        TimeSnapMode::SnapBar => {
                                            tempo.secs_at_bar(click_bar.round() as f64) as f32
                                        }
                                    };
                                    scrub_target_sec = Some(target_sec.max(0.0));
                                    self.pattern_mode = false;
                                }
                            }

                            // Ruler Hover Tooltip with exact hundredths
                            if let Some(hover_pos) = ruler_resp.hover_pos() {
                                let h_bar = (hover_pos.x - ruler_rect.min.x) / bar_w;
                                let h_sec = (self.tempo_map().secs_at_bar(h_bar as f64) as f32)
                                    .max(0.0);
                                ruler_resp.clone().on_hover_text(format!("⏱ {} {}\n{}", crate::i18n::t("Tid:"), format_time_hundredths(h_sec), format_bar_subdivisions(h_bar)));
                            }

                            ui.add_space(2.0);
                            let sec_per_bar = 60.0 / self.bpm * 4.0;

                            // 2. Track Lanes (Audio Waveforms, Clips, Grids)
                            for t_idx in 0..self.playlist_tracks.len() {
                                // Vågformscachen byggs här och inte på de tio ställen
                                // där pcm_audio sätts (Fas 8.3).
                                self.ensure_waveform_cache(t_idx);
                                let track_color = self.playlist_tracks[t_idx].color;
                                let track_name = self.playlist_tracks[t_idx].name.clone();
                                let is_mic_track = track_name.to_lowercase().contains("mic")
                                    || track_name.to_lowercase().contains("mikrofon")
                                    || track_name.to_lowercase().contains("microphone")
                                    || (t_idx == self.playlist_tracks.len() - 1 && track_name.to_lowercase().contains("mik"));
                                let track_row_h = if is_mic_track { row_h * 2.0 } else { row_h };

                                // Full Timeline Track Lane (Scalable width with total_bars)
                                let (lane_rect, lane_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, track_row_h), Sense::click_and_drag());

                                // Draw Background Bar Grid Lines
                                for bar_i in 0..total_bars {
                                    let b_min_x = lane_rect.min.x + bar_i as f32 * bar_w;
                                    let b_rect = Rect::from_min_size(Pos2::new(b_min_x, lane_rect.min.y), Vec2::new(bar_w, track_row_h));
                                    let is_active_bar = !self.pattern_mode && self.is_playing && self.song_bar == bar_i;
                                    let bg = if is_mic_track {
                                        if is_active_bar {
                                            Color32::from_rgb(228, 240, 255)
                                        } else if (bar_i / 4) % 2 == 0 {
                                            Color32::from_rgb(255, 255, 255)
                                        } else {
                                            Color32::from_rgb(247, 249, 254)
                                        }
                                    } else if is_active_bar {
                                        Color32::from_rgb(32, 44, 58)
                                    } else if (bar_i / 4) % 2 == 0 {
                                        Color32::from_rgb(16, 18, 24)
                                    } else {
                                        Color32::from_rgb(22, 25, 32)
                                    };
                                    ui.painter().rect_filled(b_rect, Rounding::ZERO, bg);
                                    let grid_stroke_col = if is_mic_track { Color32::from_rgb(218, 225, 238) } else { Color32::from_rgb(36, 42, 54) };
                                    ui.painter().line_segment([Pos2::new(b_min_x, lane_rect.min.y), Pos2::new(b_min_x, lane_rect.max.y)], Stroke::new(0.5_f32, grid_stroke_col));

                                    // Beat Sub-Grid Lines when zoomed in
                                    if bar_w >= 70.0 {
                                        let beat_col = if is_mic_track { Color32::from_rgb(232, 236, 245) } else { Color32::from_rgb(26, 30, 40) };
                                        for beat in 1..4 {
                                            let bx = b_min_x + beat as f32 * (bar_w / 4.0);
                                            ui.painter().line_segment([Pos2::new(bx, lane_rect.min.y), Pos2::new(bx, lane_rect.max.y)], Stroke::new(0.5_f32, beat_col));
                                        }
                                    }

                                    // 16th-note sub-grid lines when zoomed in
                                    if bar_w >= 220.0 {
                                        let step_col = if is_mic_track { Color32::from_rgb(240, 243, 250) } else { Color32::from_rgb(20, 24, 32) };
                                        for step in 1..16 {
                                            if step % 4 != 0 {
                                                let sx = b_min_x + step as f32 * (bar_w / 16.0);
                                                ui.painter().line_segment([Pos2::new(sx, lane_rect.min.y), Pos2::new(sx, lane_rect.max.y)], Stroke::new(0.3_f32, step_col));
                                            }
                                        }
                                    }
                                }

                                // Sound Browser Sample Drag & Drop Ghost Preview onto this Track Lane
                                if let Some(ref drag_item) = self.sample_drag_item {
                                    let pointer_opt = ui.ctx().pointer_latest_pos()
                                        .or_else(|| ui.ctx().pointer_interact_pos())
                                        .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                                        .or_else(|| lane_resp.hover_pos());
                                    if let Some(mouse_pos) = pointer_opt {
                                        if lane_rect.contains(mouse_pos) {
                                            let raw_drop_bar = ((mouse_pos.x - lane_rect.min.x) / bar_w).max(0.0);
                                            let raw_drop_sec = raw_drop_bar * sec_per_bar;
                                            let drop_sec = match self.timeline_snap_mode {
                                                TimeSnapMode::FreeHundredth => (raw_drop_sec * 100.0).round() / 100.0,
                                                TimeSnapMode::Snap16th => {
                                                    let step_sec = sec_per_bar / 16.0;
                                                    (raw_drop_sec / step_sec).round() * step_sec
                                                }
                                                TimeSnapMode::SnapBeat => {
                                                    let beat_sec = sec_per_bar / 4.0;
                                                    (raw_drop_sec / beat_sec).round() * beat_sec
                                                }
                                                TimeSnapMode::SnapBar => (raw_drop_sec / sec_per_bar).round() * sec_per_bar,
                                            };
                                            let drop_bar = (drop_sec / sec_per_bar).max(0.0);
                                            let item_dur_sec: f32 = 2.0;
                                            let item_dur_bars = (item_dur_sec / sec_per_bar).max(0.25);
                                            let ghost_x_start = lane_rect.min.x + drop_bar * bar_w;
                                            let ghost_x_end = (ghost_x_start + item_dur_bars * bar_w).min(lane_rect.max.x);
                                            let ghost_rect = Rect::from_min_max(
                                                Pos2::new(ghost_x_start, lane_rect.min.y + 2.0),
                                                Pos2::new(ghost_x_end, lane_rect.max.y - 2.0),
                                            );
                                            ui.painter().rect_filled(ghost_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(255, 170, 0, 85));
                                            ui.painter().rect_stroke(ghost_rect, Rounding::same(4.0), Stroke::new(1.8_f32, Theme::FL_ORANGE));
                                            ui.painter().text(
                                                Pos2::new(ghost_rect.min.x + 6.0, ghost_rect.center().y),
                                                egui::Align2::LEFT_CENTER,
                                                crate::tstatus!("📥 Släpp här: {} (Takt {:.2})", drag_item.name, drop_bar + 1.0),
                                                egui::FontId::proportional(10.0),
                                                Color32::WHITE,
                                            );
                                            if ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
                                                drop_sample_action = Some((t_idx, drag_item.clone(), drop_bar));
                                            }
                                        }
                                    }
                                }

                                // Bottom line separator between tracks
                                ui.painter().line_segment([Pos2::new(lane_rect.min.x, lane_rect.max.y), Pos2::new(lane_rect.max.x, lane_rect.max.y)], Stroke::new(0.5_f32, if is_mic_track { Color32::from_rgb(200, 210, 225) } else { Color32::from_rgb(30, 36, 48) }));

                                // Render continuous Audio Regions on this track
                                let mut split_action: Option<(usize, f32)> = None;
                                let mut delete_action: Option<usize> = None;
                                let mut mute_action: Option<usize> = None;
                                let mut select_action: Option<usize> = None;
                                let mut clip_action: Option<(usize, Option<usize>)> = None;

                                {
                                    let (has_regions, regions_snapshot) = if t_idx < self.playlist_tracks.len() {
                                        (!self.playlist_tracks[t_idx].regions.is_empty(), self.playlist_tracks[t_idx].regions.clone())
                                    } else {
                                        (false, Vec::new())
                                    };

                                    if has_regions {
                                        for (r_i, region) in regions_snapshot.iter().enumerate() {
                                            let is_region_selected = self.selected_audio_region == Some((t_idx, r_i));
                                            let rx_start = lane_rect.min.x + region.start_bar * bar_w;
                                            let rx_end = rx_start + region.length_bars * bar_w;
                                            let r_rect = Rect::from_min_max(Pos2::new(rx_start + 1.5, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.5, lane_rect.max.y - 2.0));

                                            if r_rect.width() > 1.0 {
                                                let col = if is_mic_track {
                                                    Color32::WHITE
                                                } else if region.color == default_region_color() || region.color == Color32::from_rgb(100, 180, 255) {
                                                    track_color
                                                } else {
                                                    region.color
                                                };
                                                let fill = if is_mic_track {
                                                    if region.muted {
                                                        Color32::from_rgb(225, 228, 235)
                                                    } else if is_region_selected {
                                                        Color32::from_rgb(255, 255, 255)
                                                    } else {
                                                        Color32::from_rgb(252, 254, 255)
                                                    }
                                                } else if region.muted {
                                                    Color32::from_rgb(24, 26, 32)
                                                } else if is_region_selected {
                                                    Color32::from_rgb(
                                                        (col.r() as f32 * 0.45).min(255.0) as u8,
                                                        (col.g() as f32 * 0.45).min(255.0) as u8,
                                                        (col.b() as f32 * 0.45).min(255.0) as u8,
                                                    )
                                                } else {
                                                    Color32::from_rgb(
                                                        (col.r() as f32 * 0.28) as u8,
                                                        (col.g() as f32 * 0.28) as u8,
                                                        (col.b() as f32 * 0.28) as u8,
                                                    )
                                                };

                                                ui.painter().rect_filled(r_rect, Rounding::same(4.0), fill);

                                                // Top Header Banner strip for Region
                                                let header_h = 14.0_f32;
                                                let header_rect = Rect::from_min_max(r_rect.min, Pos2::new(r_rect.max.x, (r_rect.min.y + header_h).min(r_rect.max.y)));
                                                let header_bg = if is_mic_track {
                                                    if region.muted {
                                                        Color32::from_rgb(205, 210, 220)
                                                    } else if is_region_selected {
                                                        Color32::from_rgb(215, 230, 250)
                                                    } else {
                                                        Color32::from_rgb(235, 240, 248)
                                                    }
                                                } else if region.muted {
                                                    Color32::from_rgb(36, 40, 48)
                                                } else if is_region_selected {
                                                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 190)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 120)
                                                };
                                                ui.painter().rect_filled(header_rect, Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, header_bg);

                                                let stroke_col = if is_region_selected {
                                                    if is_mic_track { Color32::from_rgb(0, 180, 240) } else { Color32::WHITE }
                                                } else if region.muted {
                                                    Color32::from_rgb(70, 75, 85)
                                                } else if is_mic_track {
                                                    Color32::from_rgb(180, 190, 205)
                                                } else {
                                                    col
                                                };
                                                ui.painter().rect_stroke(r_rect, Rounding::same(4.0), Stroke::new(if is_region_selected { 2.0_f32 } else { 1.2_f32 }, stroke_col));

                                                // Region Header Banner & Exact Time readout (Start & Length down to hundredths!)
                                                let gain_db = if region.volume <= 0.001 { -60.0 } else { 20.0 * region.volume.log10() };
                                                let r_start_str = format_time_hundredths(region.start_bar * sec_per_bar);
                                                let r_len_str = format_time_hundredths(region.length_bars * sec_per_bar);
                                                let rev_tag = if region.is_reverse { " 🔄[REV]" } else { "" };
                                                let loop_tag = if region.loop_length_bars > 0.001 && region.length_bars > region.loop_length_bars + 0.01 {
                                                    let reps = (region.length_bars / region.loop_length_bars).ceil() as usize;
                                                    format!(" 🔁[x{}]", reps)
                                                } else {
                                                    "".to_string()
                                                };
                                                let title_text = format!("{}{rev_tag}{loop_tag} [⏱ {} | 📏 {} | {:+.1}dB]", region.name, r_start_str, r_len_str, gain_db);

                                                let header_text_col = if is_mic_track {
                                                    Color32::from_rgb(18, 22, 30)
                                                } else if region.muted {
                                                    Color32::from_rgb(140, 140, 150)
                                                } else {
                                                    Color32::WHITE
                                                };

                                                ui.painter().text(
                                                    Pos2::new(r_rect.min.x + 5.0, r_rect.min.y + 7.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    title_text,
                                                    egui::FontId::proportional(9.0),
                                                    header_text_col,
                                                );

                                                // Draw Fade-in / Fade-out curve indicators
                                                if region.fade_in_bars > 0.01 {
                                                    let fi_w = (region.fade_in_bars * bar_w).min(r_rect.width() * 0.5);
                                                    let fi_pts = [
                                                        Pos2::new(r_rect.min.x, r_rect.max.y),
                                                        Pos2::new(r_rect.min.x + fi_w, r_rect.min.y),
                                                    ];
                                                    ui.painter().line_segment(fi_pts, Stroke::new(1.2_f32, Theme::FL_CYAN));
                                                }
                                                if region.fade_out_bars > 0.01 {
                                                    let fo_w = (region.fade_out_bars * bar_w).min(r_rect.width() * 0.5);
                                                    let fo_pts = [
                                                        Pos2::new(r_rect.max.x - fo_w, r_rect.min.y),
                                                        Pos2::new(r_rect.max.x, r_rect.max.y),
                                                    ];
                                                    ui.painter().line_segment(fo_pts, Stroke::new(1.2_f32, Theme::FL_ORANGE));
                                                }

                                                // Draw Continuous Soundtrap-Style Waveform Inside Region
                                                let mid_y = r_rect.center().y + 3.0;
                                                let wave_col = if is_mic_track {
                                                    if region.muted {
                                                        Color32::from_rgb(120, 125, 135)
                                                    } else if is_region_selected {
                                                        Color32::from_rgb(0, 0, 0)
                                                    } else {
                                                        Color32::from_rgb(15, 18, 24) // Jet Black Waveform!
                                                    }
                                                } else if region.muted {
                                                    Color32::from_rgb(90, 95, 105)
                                                } else if is_region_selected {
                                                    Color32::WHITE
                                                } else {
                                                    Color32::from_rgb(
                                                        ((col.r() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                                        ((col.g() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                                        ((col.b() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                                    )
                                                };

                                                let is_looped = region.loop_length_bars > 0.001 && region.length_bars > region.loop_length_bars + 0.01;
                                                let loop_sec = if is_looped {
                                                    region.loop_length_bars * sec_per_bar
                                                } else {
                                                    if region.length_bars > 0.001 { region.length_bars * sec_per_bar } else { 1.0 }
                                                };

                                                // Fas 8.3: rita ur spårets cache när den finns —
                                                // ett äkta (min, max) per bildpunkt, i stället för
                                                // en fast array utsträckt över regionens bredd.
                                                //
                                                // En slingad region hoppas över: dess bildpunkter
                                                // följer upprepningen, inte en sammanhängande
                                                // sampelmängd, och det är en egen fråga.
                                                let mut drew_exact = false;
                                                if region.length_bars > 0.001
                                                    && let Some(wave) = {
                                                        let track = &self.playlist_tracks[t_idx];
                                                        track.waveform_cache.as_ref().and_then(|(_, c)| {
                                                            track
                                                                .frozen_pcm
                                                                .as_ref()
                                                                .or(track.pcm_audio.as_ref())
                                                                .map(|(l, _, sr)| (c, l.as_slice(), *sr))
                                                        })
                                                    }
                                                {
                                                    // Hela rektangeln, utan indrag: indraget
                                                    // försköt hela mappningen, och det är
                                                    // precis då startpunkten blir svår att pricka.
                                                    let draw_start_x = r_rect.min.x;
                                                    let draw_end_x = r_rect.max.x;
                                                    let (cache, pcm, sr) = wave;
                                                    let region_secs = self
                                                        .tempo_map()
                                                        .secs_for_bars_at(
                                                            region.start_bar as f64,
                                                            region.length_bars as f64,
                                                        ) as f32;
                                                    let start_sample =
                                                        (region.sample_offset_sec.max(0.0) * sr as f32)
                                                            as usize;
                                                    let total_samples =
                                                        (region_secs * sr as f32).max(1.0) as usize;
                                                    let cols =
                                                        ((draw_end_x - draw_start_x).max(1.0)).ceil() as usize;
                                                    // En slingad kloss upprepar ett kortare
                                                    // stycke: då följer bildpunkterna
                                                    // upprepningen, inte en sammanhängande
                                                    // sampelmängd.
                                                    let loop_samples =
                                                        (loop_sec * sr as f32).max(1.0) as usize;
                                                    let env = if is_looped && loop_sec > 0.01 {
                                                        cache.envelope_looped(
                                                            pcm,
                                                            start_sample,
                                                            loop_samples,
                                                            total_samples,
                                                            cols,
                                                        )
                                                    } else {
                                                        cache.envelope_at(
                                                            pcm,
                                                            start_sample,
                                                            total_samples,
                                                            cols,
                                                        )
                                                    };
                                                    let half = r_rect.height() * 0.42;
                                                    let vol = region.volume.clamp(0.2, 1.8);
                                                    // På pixelcentra: en en-pixels linje som
                                                    // hamnar mellan två pixlar flyter ut över
                                                    // båda och ser mjuk ut. Halva pixeln är
                                                    // centrum i egui.
                                                    let start_x = draw_start_x.round() + 0.5;
                                                    // Läget avgör vad som ritas (Fas 8.4): ett
                                                    // hölje är en approximation, och den riktiga
                                                    // vågformen finns bara där varje sampel får
                                                    // sin egen bildpunkt. Att rita finare kolumner
                                                    // gör den aldrig tydligare — representationen
                                                    // måste bytas.
                                                    match crate::audio::waveform::zoom_regime(
                                                        total_samples as f64 / cols as f64,
                                                    ) {
                                                        crate::audio::waveform::ZoomRegime::Envelope => {
                                                            let mut x = start_x;
                                                            for (lo, hi) in &env {
                                                                // Toppen är max och botten är min: en
                                                                // osymmetrisk signal ska se osymmetrisk ut.
                                                                let top =
                                                                    mid_y - (hi * half * vol).min(half);
                                                                let bot =
                                                                    mid_y - (lo * half * vol).max(-half);
                                                                ui.painter().line_segment(
                                                                    [
                                                                        Pos2::new(x, top),
                                                                        Pos2::new(
                                                                            x,
                                                                            bot.max(top + 0.5),
                                                                        ),
                                                                    ],
                                                                    Stroke::new(1.0_f32, wave_col),
                                                                );
                                                                x += 1.0;
                                                            }
                                                        }
                                                        regime => {
                                                            // Ett sampel per bildpunkt: kurva genom
                                                            // samplarna, och punkter när de hunnit
                                                            // åtskilda (som Audacity).
                                                            let dots = regime
                                                                == crate::audio::waveform::ZoomRegime::SampleDots;
                                                            let mut prev: Option<Pos2> = None;
                                                            for i in 0..cols {
                                                                let k = if is_looped
                                                                    && loop_samples > 0
                                                                {
                                                                    start_sample
                                                                        + (i * total_samples / cols)
                                                                            % loop_samples
                                                                } else {
                                                                    start_sample
                                                                        + i * total_samples / cols
                                                                };
                                                                let v = pcm.get(k).copied().unwrap_or(0.0);
                                                                let y = mid_y
                                                                    - (v * half * vol).clamp(-half, half);
                                                                let x = start_x + i as f32;
                                                                let p = Pos2::new(x, y);
                                                                if let Some(q) = prev {
                                                                    ui.painter().line_segment(
                                                                        [q, p],
                                                                        Stroke::new(1.0_f32, wave_col),
                                                                    );
                                                                }
                                                                if dots {
                                                                    ui.painter().circle_filled(
                                                                        p, 1.6, wave_col,
                                                                    );
                                                                }
                                                                prev = Some(p);
                                                            }
                                                        }
                                                    }
                                                    drew_exact = true;
                                                }

                                                // Den gamla vägen, för regioner utan PCM och för
                                                // slingade regioner. `drew_exact` gör att den inte
                                                // ritar ovanpå det exakta höljet.
                                                let num_peaks = if drew_exact {
                                                    0
                                                } else {
                                                    region.waveform_peaks.len()
                                                };
                                                if num_peaks > 0 {
                                                    // Draw subtle center baseline
                                                    ui.painter().line_segment(
                                                        [Pos2::new(r_rect.min.x + 2.0, mid_y), Pos2::new(r_rect.max.x - 2.0, mid_y)],
                                                        Stroke::new(0.8_f32, if is_mic_track { Color32::from_rgb(70, 75, 85) } else { Color32::from_rgba_unmultiplied(wave_col.r(), wave_col.g(), wave_col.b(), 60) })
                                                    );

                                                    // Draw vertical waveform bars across the entire region width
                                                    let draw_start_x = r_rect.min.x + 2.0;
                                                    let draw_end_x = r_rect.max.x - 2.0;
                                                    let step_px = 3.0_f32;
                                                    let mut cur_x = draw_start_x;
                                                    while cur_x <= draw_end_x {
                                                        let rel_x = cur_x - r_rect.min.x;
                                                        let rel_time = (rel_x / bar_w) * sec_per_bar;

                                                        let sample_time = if is_looped && loop_sec > 0.01 {
                                                            (region.sample_offset_sec + rel_time) % loop_sec
                                                        } else {
                                                            region.sample_offset_sec + rel_time
                                                        };

                                                        let norm_pos = if loop_sec > 0.001 { (sample_time / loop_sec).clamp(0.0, 0.999) } else { 0.0 };
                                                        let peak_idx = ((norm_pos * num_peaks as f32).floor() as usize).min(num_peaks - 1);
                                                        let amp = region.waveform_peaks[peak_idx];

                                                        let h = (amp * (r_rect.height() * 0.42) * region.volume.clamp(0.2, 1.8)).max(0.5);
                                                        ui.painter().line_segment([Pos2::new(cur_x, mid_y - h), Pos2::new(cur_x, mid_y + h)], Stroke::new(2.2_f32, wave_col));

                                                        cur_x += step_px;
                                                    }

                                                    // Draw loop repeat dividers (exact positions where loop cycles restart)
                                                    if is_looped && loop_sec > 0.01 {
                                                        let first_cycle_rem_sec = (loop_sec - (region.sample_offset_sec % loop_sec)) % loop_sec;
                                                        let mut div_sec = if first_cycle_rem_sec > 0.02 { first_cycle_rem_sec } else { loop_sec };
                                                        let total_reg_sec = region.length_bars * sec_per_bar;
                                                        while div_sec < total_reg_sec - 0.05 {
                                                            let div_x = rx_start + (div_sec / sec_per_bar) * bar_w;
                                                            if div_x > r_rect.min.x + 4.0 && div_x < r_rect.max.x - 4.0 {
                                                                ui.painter().line_segment(
                                                                    [Pos2::new(div_x, r_rect.min.y), Pos2::new(div_x, r_rect.max.y)],
                                                                    Stroke::new(1.0_f32, Theme::FL_CYAN),
                                                                );
                                                                ui.painter().text(
                                                                    Pos2::new(div_x + 3.0, r_rect.min.y + 7.0),
                                                                    egui::Align2::LEFT_CENTER,
                                                                    "🔁",
                                                                    egui::FontId::proportional(8.0),
                                                                    Theme::FL_CYAN,
                                                                );
                                                            }
                                                            div_sec += loop_sec;
                                                        }
                                                    }
                                                }

                                                // Visual Edge Drag Handles for ALL regions (extra prominent when hovered/selected)
                                                let handle_w = 10.0_f32;
                                                let is_lane_h = lane_resp.hover_pos().map(|p| r_rect.contains(p)).unwrap_or(false)
                                                    || ui.ctx().pointer_latest_pos().map(|p| r_rect.contains(p)).unwrap_or(false);

                                                // Left trim handle (Trim Start / Korta start från vänster kant)
                                                let left_h_rect = Rect::from_min_size(Pos2::new(r_rect.min.x, r_rect.min.y), Vec2::new(handle_w, r_rect.height()));
                                                let left_h_col = if is_region_selected {
                                                    Theme::FL_CYAN
                                                } else if is_lane_h {
                                                    Color32::from_rgb(180, 230, 255)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(255, 255, 255, 120)
                                                };
                                                ui.painter().rect_filled(left_h_rect, Rounding { nw: 3.0, ne: 0.0, sw: 3.0, se: 0.0 }, left_h_col);
                                                ui.painter().line_segment([Pos2::new(left_h_rect.min.x + 4.0, left_h_rect.min.y + 6.0), Pos2::new(left_h_rect.min.x + 4.0, left_h_rect.max.y - 6.0)], Stroke::new(1.2_f32, Color32::from_rgb(30, 40, 55)));

                                                // Right trim/loop handle (Trim End / Dra för att förlänga, korta eller loopa från höger kant)
                                                let right_h_rect = Rect::from_min_max(Pos2::new(r_rect.max.x - handle_w, r_rect.min.y), r_rect.max);
                                                let right_h_col = if is_region_selected {
                                                    Theme::FL_ORANGE
                                                } else if is_lane_h {
                                                    Color32::from_rgb(255, 200, 140)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(255, 255, 255, 120)
                                                };
                                                ui.painter().rect_filled(right_h_rect, Rounding { nw: 0.0, ne: 3.0, sw: 0.0, se: 3.0 }, right_h_col);
                                                ui.painter().line_segment([Pos2::new(right_h_rect.min.x + 4.0, right_h_rect.min.y + 6.0), Pos2::new(right_h_rect.min.x + 4.0, right_h_rect.max.y - 6.0)], Stroke::new(1.2_f32, Color32::from_rgb(55, 40, 30)));

                                                // Hover Cursor Feedback for Select and Draw/Paint tools (FL Studio style)
                                                if matches!(self.arranger_tool, ArrangerTool::Select | ArrangerTool::Paint) {
                                                    let pointer_opt = lane_resp.hover_pos().or_else(|| ui.ctx().pointer_latest_pos());
                                                    if let Some(mouse_pos) = pointer_opt {
                                                        if r_rect.contains(mouse_pos) {
                                                            if (mouse_pos.x - rx_start).abs() <= 16.0 || mouse_pos.x <= rx_start + 16.0 {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                            } else if (mouse_pos.x - rx_end).abs() <= 16.0 || mouse_pos.x >= rx_end - 16.0 {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                            } else {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Start Drag on Region (Edge Trim Start vs Edge Trim End / Loop vs Move)
                                        let is_pointer_primary_down = ui.input(|i| i.pointer.primary_down());
                                        let is_lane_dragging = lane_resp.drag_started()
                                            || lane_resp.dragged()
                                            || (lane_resp.hovered() && is_pointer_primary_down)
                                            || (is_pointer_primary_down && ui.input(|i| i.pointer.is_decidedly_dragging()));

                                        if matches!(self.arranger_tool, ArrangerTool::Select | ArrangerTool::Paint) && is_lane_dragging && self.region_drag_state.is_none() {
                                            let mouse_pos_opt = ui.ctx().pointer_interact_pos()
                                                .or_else(|| ui.input(|i| i.pointer.press_origin()))
                                                .or_else(|| ui.ctx().pointer_latest_pos())
                                                .or_else(|| lane_resp.hover_pos());
                                            if let Some(mouse_pos) = mouse_pos_opt {
                                                for (r_i, r) in regions_snapshot.iter().enumerate() {
                                                    let rx_start = lane_rect.min.x + r.start_bar * bar_w;
                                                    let rx_end = rx_start + r.length_bars * bar_w;
                                                    let region_hit_rect = Rect::from_min_max(
                                                        Pos2::new(rx_start - 8.0, lane_rect.min.y),
                                                        Pos2::new(rx_end + 8.0, lane_rect.max.y),
                                                    );
                                                    if region_hit_rect.contains(mouse_pos) {
                                                        let (mode, init_loop_bars) = if mouse_pos.x <= rx_start + 16.0 || (mouse_pos.x - rx_start).abs() <= 10.0 {
                                                            (RegionDragMode::TrimStart, if r.loop_length_bars > 0.001 { r.loop_length_bars } else { r.length_bars })
                                                        } else if mouse_pos.x >= rx_end - 16.0 || (mouse_pos.x - rx_end).abs() <= 10.0 {
                                                            (RegionDragMode::TrimEnd, if r.loop_length_bars > 0.001 { r.loop_length_bars } else { r.length_bars })
                                                        } else {
                                                            (RegionDragMode::Move, if r.loop_length_bars > 0.001 { r.loop_length_bars } else { r.length_bars })
                                                        };
                                                        self.push_undo(match mode {
                                                            RegionDragMode::TrimStart => crate::i18n::t("Trimma start (Vänster)"),
                                                            RegionDragMode::TrimEnd => crate::i18n::t("Justera längd / loop (Höger)"),
                                                            RegionDragMode::Move => crate::i18n::t("Flytta region"),
                                                        });
                                                        self.region_drag_state = Some(RegionDragState {
                                                            track_idx: t_idx,
                                                            region_idx: r_i,
                                                            mode,
                                                            initial_start_bar: r.start_bar,
                                                            initial_length_bars: r.length_bars,
                                                            initial_sample_offset_sec: r.sample_offset_sec,
                                                            initial_loop_length_bars: init_loop_bars,
                                                            drag_start_mouse_x: mouse_pos.x,
                                                        });
                                                        self.selected_audio_region = Some((t_idx, r_i));
                                                        self.selected_timeline_track = t_idx;
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        // Process active dragging
                                        if let Some(ref drag) = self.region_drag_state {
                                            if drag.track_idx == t_idx {
                                                let pointer_pos_opt = ui.ctx().pointer_latest_pos()
                                                    .or_else(|| ui.ctx().pointer_interact_pos())
                                                    .or_else(|| lane_resp.hover_pos());
                                                if let Some(mouse_pos) = pointer_pos_opt {
                                                    let is_alt_held = ui.input(|i| i.modifiers.alt);
                                                    let raw_delta_bars = (mouse_pos.x - drag.drag_start_mouse_x) / bar_w;
                                                    let grid_step_bars = match self.timeline_snap_mode {
                                                        TimeSnapMode::FreeHundredth => (0.01 / sec_per_bar).max(0.001),
                                                        TimeSnapMode::Snap16th => 1.0 / 16.0,
                                                        TimeSnapMode::SnapBeat => 0.25,
                                                        TimeSnapMode::SnapBar => 1.0,
                                                    };

                                                    if drag.region_idx < self.playlist_tracks[t_idx].regions.len() {
                                                        let reg = &mut self.playlist_tracks[t_idx].regions[drag.region_idx];
                                                        match drag.mode {
                                                            RegionDragMode::Move => {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                                                                let raw_start = drag.initial_start_bar + raw_delta_bars;
                                                                let new_start = if is_alt_held {
                                                                    raw_start.max(0.0)
                                                                } else {
                                                                    ((raw_start / grid_step_bars).round() * grid_step_bars).max(0.0)
                                                                };
                                                                reg.start_bar = new_start;
                                                                self.status_message = crate::tstatus!("↔ Flyttar '{}' till takt {:.2} (⏱ {})", reg.name, new_start + 1.0, format_time_hundredths(new_start * sec_per_bar));
                                                            }
                                                            RegionDragMode::TrimStart => {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                                let right_edge = drag.initial_start_bar + drag.initial_length_bars;
                                                                let max_expand_sec = drag.initial_sample_offset_sec;
                                                                let orig_sample_start = (drag.initial_start_bar - (max_expand_sec / sec_per_bar)).max(0.0);
                                                                let raw_start = drag.initial_start_bar + raw_delta_bars;

                                                                let mut new_start = if is_alt_held {
                                                                    raw_start
                                                                } else {
                                                                    // Magnetic snap to sample start (0 offset)
                                                                    let snap_px_bars = (16.0 / bar_w).max(0.04);
                                                                    if (raw_start - orig_sample_start).abs() <= snap_px_bars {
                                                                        orig_sample_start
                                                                    } else {
                                                                        (raw_start / grid_step_bars).round() * grid_step_bars
                                                                    }
                                                                };
                                                                new_start = new_start.clamp(orig_sample_start, right_edge - 0.05);
                                                                let new_len = right_edge - new_start;
                                                                let new_offset = (drag.initial_sample_offset_sec + (new_start - drag.initial_start_bar) * sec_per_bar).max(0.0);
                                                                reg.start_bar = new_start;
                                                                reg.length_bars = new_len;
                                                                reg.sample_offset_sec = new_offset;
                                                                self.status_message = crate::tstatus!("◀ Trimmar start på '{}': Start takt {:.2} | Bortklippt start: +{:.2}s | Längd: {:.2} takter", reg.name, new_start + 1.0, new_offset, new_len);
                                                            }
                                                            RegionDragMode::TrimEnd => {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                                let base_loop_len = if drag.initial_loop_length_bars > 0.001 { drag.initial_loop_length_bars } else { drag.initial_length_bars };
                                                                let raw_len = (drag.initial_length_bars + raw_delta_bars).max(0.05);

                                                                let mut new_len = if is_alt_held {
                                                                    raw_len
                                                                } else {
                                                                    // 1. Magnetic snap to exact full loop cycles (1x, 2x, 3x, 4x, etc.)
                                                                    let nearest_rep = (raw_len / base_loop_len).round().max(1.0);
                                                                    let rep_snap_len = nearest_rep * base_loop_len;
                                                                    let snap_px_bars = (18.0 / bar_w).max(0.05);
                                                                    if (raw_len - rep_snap_len).abs() <= snap_px_bars {
                                                                        rep_snap_len
                                                                    } else {
                                                                        // 2. Snap to active grid (1/16, beat, bar)
                                                                        ((raw_len / grid_step_bars).round() * grid_step_bars).max(grid_step_bars.min(0.05))
                                                                    }
                                                                };
                                                                new_len = new_len.max(0.05);
                                                                reg.loop_length_bars = base_loop_len;
                                                                reg.length_bars = new_len;

                                                                let reps = new_len / base_loop_len;
                                                                let is_exact_rep = (reps - reps.round()).abs() < 0.001;
                                                                if new_len > base_loop_len + 0.05 {
                                                                    if is_exact_rep {
                                                                        self.status_message = crate::tstatus!("🧲 Loop-snap: '{}' loopad exakt {:.0}x ({} takter, ⏱ {})", reg.name, reps.round(), new_len, format_time_hundredths(new_len * sec_per_bar));
                                                                    } else {
                                                                        self.status_message = crate::tstatus!("▶ Loopar '{}': {:.2} takter ({:.1}x repetitioner, ⏱ {})", reg.name, new_len, reps, format_time_hundredths(new_len * sec_per_bar));
                                                                    }
                                                                } else {
                                                                    self.status_message = crate::tstatus!("▶ Längd för '{}': {:.2} takter (⏱ {})", reg.name, new_len, format_time_hundredths(new_len * sec_per_bar));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        let is_drag_released = lane_resp.drag_stopped() || ui.input(|i| i.pointer.any_released() || i.pointer.primary_released() || !i.pointer.primary_down());
                                        if is_drag_released && self.region_drag_state.is_some() {
                                            if let Some(ref drag) = self.region_drag_state {
                                                let t_idx_synced = drag.track_idx;
                                                self.sync_track_regions(t_idx_synced);
                                                self.status_message = crate::i18n::t("✅ Ändring sparad på tidslinjen!").to_string();
                                            }
                                            self.region_drag_state = None;
                                        }

                                        // Tool Click Interactions
                                        if lane_resp.clicked() && self.region_drag_state.is_none() {
                                            ui.memory_mut(|m| m.stop_text_input());
                                            self.selected_timeline_track = t_idx;
                                            if let Some(mouse_pos) = lane_resp.hover_pos() {
                                                let click_bar = (mouse_pos.x - lane_rect.min.x) / bar_w;
                                                for (r_i, r) in regions_snapshot.iter().enumerate() {
                                                    if click_bar >= r.start_bar && click_bar <= (r.start_bar + r.length_bars) {
                                                        match self.arranger_tool {
                                                            ArrangerTool::Select => {
                                                                select_action = Some(r_i);
                                                            }
                                                            ArrangerTool::Slice => {
                                                                split_action = Some((r_i, click_bar));
                                                            }
                                                            ArrangerTool::Erase => {
                                                                delete_action = Some(r_i);
                                                            }
                                                            ArrangerTool::Mute => {
                                                                mute_action = Some(r_i);
                                                            }
                                                            _ => {
                                                                select_action = Some(r_i);
                                                            }
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Fallback pattern clips
                                        for bar_idx in 0..32 {
                                            if let Some(pat_idx) = self.playlist_tracks[t_idx].clips[bar_idx] {
                                                let c_min_x = lane_rect.min.x + bar_idx as f32 * bar_w;
                                                let c_rect = Rect::from_min_size(Pos2::new(c_min_x + 1.0, lane_rect.min.y + 2.0), Vec2::new(bar_w - 2.0, row_h - 4.0));
                                                ui.painter().rect_filled(c_rect, Rounding::same(2.0), track_color);
                                                ui.painter().text(c_rect.center(), egui::Align2::CENTER_CENTER, format!("P{}", pat_idx + 1), egui::FontId::proportional(10.0), Color32::BLACK);
                                            }
                                        }

                                        if (lane_resp.clicked() || lane_resp.secondary_clicked())
                                            && let Some(mouse_pos) = lane_resp.hover_pos() {
                                                let clicked_bar = ((mouse_pos.x - lane_rect.min.x) / bar_w) as usize;
                                                if clicked_bar < 32 {
                                                    if lane_resp.secondary_clicked() || self.arranger_tool == ArrangerTool::Erase {
                                                        clip_action = Some((clicked_bar, None));
                                                    } else if self.arranger_tool == ArrangerTool::Paint {
                                                        clip_action = Some((clicked_bar, Some(self.selected_pattern)));
                                                    }
                                                }
                                            }
                                    }
                                }

                                // Context Menu for Region & Track Lane
                                lane_resp.context_menu(|ui| {
                                    ui.set_min_width(240.0);
                                    if let Some((sel_t, sel_r)) = self.selected_audio_region
                                        && sel_t == t_idx
                                        && sel_r < self.playlist_tracks[t_idx].regions.len() {
                                            let r_name = self.playlist_tracks[t_idx].regions[sel_r].name.clone();
                                            let is_muted = self.playlist_tracks[t_idx].regions[sel_r].muted;
                                            let is_rev = self.playlist_tracks[t_idx].regions[sel_r].is_reverse;

                                            ui.label(egui::RichText::new(format!("🎵 {}", r_name)).strong().color(Theme::FL_CYAN));
                                            ui.separator();

                                            if ui.button(crate::i18n::t("📋 Kopiera sample (Ctrl+C)")).clicked() {
                                                self.copy_selected_region();
                                                ui.close_menu();
                                            }
                                            if self.copied_region.is_some() {
                                                if ui.button(crate::i18n::t("📋 Klistra in sample vid spelhuvud (Ctrl+V)")).clicked() {
                                                    self.paste_region();
                                                    ui.close_menu();
                                                }
                                            }
                                            if ui.button(crate::i18n::t("📋 Duplicera region (Ctrl+D)")).clicked() {
                                                self.duplicate_selected_region();
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button(crate::i18n::t("🔁 Loopa/Repetera loop (x2 - Ctrl+L)")).clicked() {
                                                self.repeat_selected_region_loop(2.0);
                                                ui.close_menu();
                                            }
                                            if ui.button(crate::i18n::t("🔁 Loopa/Repetera loop (x4)")).clicked() {
                                                self.repeat_selected_region_loop(4.0);
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button(self.tr("✂ Klipp vid spelhuvud (Ctrl+B / S)")).clicked() {
                                                self.split_selected_region_at_playhead();
                                                ui.close_menu();
                                            }
                                            if ui.button(crate::i18n::t("💾 Spara som sample i Sound Browser")).clicked() {
                                                self.save_region_to_sound_browser(sel_t, sel_r);
                                                ui.close_menu();
                                            }
                                            if ui.button(if is_rev { crate::i18n::t("🔄 Återställ riktning (Normal)") } else { crate::i18n::t("🔄 Vänd baklänges (Reverse - Ctrl+K)") }).clicked() {
                                                self.reverse_selected_region();
                                                ui.close_menu();
                                            }
                                            if ui.button(if is_muted { crate::i18n::t("🔊 Avmuta region (M)") } else { crate::i18n::t("🔇 Muta region (M)") }).clicked() {
                                                self.toggle_mute_selected_region();
                                                ui.close_menu();
                                            }
                                            if ui.button(crate::i18n::t("🎙 Öppna i Sångstudion (Isolerad provspelning & formning)")).clicked() {
                                                self.open_region_in_vocal_studio(sel_t, sel_r);
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button(crate::i18n::t("📋 Duplicera hela detta spår")).clicked() {
                                                track_duplicate_idx = Some(t_idx);
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            ui.menu_button(crate::i18n::t("🎨 Ändra färg"), |ui| {
                                                let colors = [
                                                    ("Cyan", Theme::FL_CYAN),
                                                    ("Orange", Theme::FL_ORANGE),
                                                    ("Grön", Theme::FL_GREEN),
                                                    ("Lila", Theme::FL_PURPLE),
                                                    ("Blå", Color32::from_rgb(80, 150, 255)),
                                                    ("Guld", Color32::from_rgb(255, 200, 50)),
                                                ];
                                                for (c_name, c_val) in colors {
                                                    if ui.button(crate::i18n::t(c_name)).clicked() {
                                                        self.playlist_tracks[t_idx].regions[sel_r].color = c_val;
                                                        ui.close_menu();
                                                    }
                                                }
                                            });
                                            ui.separator();
                                            if ui.button(egui::RichText::new(crate::i18n::t("🗑 Radera region (Delete)")).color(Color32::from_rgb(255, 90, 90))).clicked() {
                                                self.delete_selected_region();
                                                ui.close_menu();
                                            }
                                    } else {
                                        ui.label(egui::RichText::new(crate::tstatus!("Spår {}: {}", t_idx + 1, self.playlist_tracks[t_idx].name)).strong());
                                        ui.separator();
                                        if self.copied_region.is_some() {
                                            if ui.button(crate::i18n::t("📋 Klistra in sample vid spelhuvud (Ctrl+V)")).clicked() {
                                                track_paste_idx = Some(t_idx);
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                        }
                                        if ui.button(crate::i18n::t("📋 Duplicera spår (under detta)")).clicked() {
                                            track_duplicate_idx = Some(t_idx);
                                            ui.close_menu();
                                        }
                                        if ui.button(crate::i18n::t("➕ Lägg till nytt spår...")).clicked() {
                                            self.show_add_track_modal = true;
                                            ui.close_menu();
                                        }
                                        if ui.button(crate::i18n::t("🔴 Spela in på detta spår")).clicked() {
                                            for t in &mut self.playlist_tracks { t.is_rec_armed = false; }
                                            self.playlist_tracks[t_idx].is_rec_armed = true;
                                            self.toggle_timeline_recording();
                                            ui.close_menu();
                                        }
                                    }
                                });

                                // Render live in-track recording region if this track is actively recording!
                                if self.is_recording_timeline && self.playlist_tracks[t_idx].is_rec_armed {
                                    let live_s_bar = self.timeline_rec_start_bar;
                                    let live_e_bar = (self.song_time / sec_per_bar).max(live_s_bar + 0.05);
                                    let rx_start = lane_rect.min.x + live_s_bar * bar_w;
                                    let rx_end = lane_rect.min.x + live_e_bar * bar_w;
                                    let live_rect = Rect::from_min_max(Pos2::new(rx_start + 1.0, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.0, lane_rect.max.y - 2.0));

                                    ui.painter().rect_filled(live_rect, Rounding::same(3.0), Color32::from_rgba_unmultiplied(220, 30, 30, 85));
                                    ui.painter().rect_stroke(live_rect, Rounding::same(3.0), Stroke::new(1.5_f32, Color32::from_rgb(255, 60, 60)));

                                    let live_peaks = &self.vocal_studio.live_recording_peaks;
                                    if !live_peaks.is_empty() && live_rect.width() > 6.0 {
                                        let step_x = (live_rect.width() - 4.0) / live_peaks.len().max(1) as f32;
                                        let mid_y = live_rect.center().y;
                                        for (pi, &p) in live_peaks.iter().enumerate() {
                                            let sx = live_rect.min.x + 2.0 + pi as f32 * step_x;
                                            let h = p * (live_rect.height() * 0.42);
                                            ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.3_f32, Color32::from_rgb(255, 190, 190)));
                                        }
                                    }

                                    ui.painter().text(
                                        Pos2::new(live_rect.min.x + 6.0, live_rect.min.y + 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        format!("🔴 SPELAR IN... [⏱ {}]", format_time_hundredths((live_e_bar - live_s_bar) * sec_per_bar)),
                                        egui::FontId::proportional(9.5),
                                        Color32::WHITE,
                                    );
                                }

                                if let Some((c_bar, maybe_pat)) = clip_action {
                                    self.playlist_tracks[t_idx].clips[c_bar] = maybe_pat;
                                    if maybe_pat.is_some() {
                                        self.status_message = crate::tstatus!("Placerade mönster {} vid takt {}", self.selected_pattern + 1, c_bar + 1);
                                    } else {
                                        self.status_message = crate::tstatus!("Raderade mönsterblock vid takt {}", c_bar + 1);
                                    }
                                }

                                if let Some(s_idx) = select_action {
                                    self.selected_audio_region = Some((t_idx, s_idx));
                                    self.selected_timeline_track = t_idx;
                                    if s_idx < self.playlist_tracks[t_idx].regions.len() {
                                        let reg = &self.playlist_tracks[t_idx].regions[s_idx];
                                        self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(reg.start_bar * sec_per_bar), format_time_hundredths(reg.length_bars * sec_per_bar));
                                    }
                                }

                                // Execute Slice with Hundredth-Second Accuracy
                                if let Some((r_idx, click_bar)) = split_action {
                                    let mut performed_split = false;
                                    let mut cut_feedback_msg = String::new();

                                    if r_idx < self.playlist_tracks[t_idx].regions.len() {
                                        let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
                                        let raw_cut_sec = click_bar * sec_per_bar;

                                        // Apply Snapping Mode
                                        let cut_sec = match self.timeline_snap_mode {
                                            TimeSnapMode::FreeHundredth => (raw_cut_sec * 100.0).round() / 100.0,
                                            TimeSnapMode::Snap16th => {
                                                let step_sec = sec_per_bar / 16.0;
                                                (raw_cut_sec / step_sec).round() * step_sec
                                            }
                                            TimeSnapMode::SnapBeat => {
                                                let beat_sec = sec_per_bar / 4.0;
                                                (raw_cut_sec / beat_sec).round() * beat_sec
                                            }
                                            TimeSnapMode::SnapBar => (raw_cut_sec / sec_per_bar).round() * sec_per_bar,
                                        };

                                        let orig_start_sec = orig.start_bar * sec_per_bar;
                                        let orig_len_sec = orig.length_bars * sec_per_bar;
                                        let split_offset_sec = cut_sec - orig_start_sec;

                                        if split_offset_sec > 0.05 && split_offset_sec < orig_len_sec - 0.05 {
                                            let split_offset_bar = split_offset_sec / sec_per_bar;
                                            let split_points = ((split_offset_sec / orig_len_sec) * orig.waveform_peaks.len() as f32) as usize;

                                            let left_peaks = orig.waveform_peaks[..split_points.min(orig.waveform_peaks.len())].to_vec();
                                            let right_peaks = orig.waveform_peaks[split_points.min(orig.waveform_peaks.len())..].to_vec();

                                            let clean_title = orig.name
                                                .replace(" [Del 1]", "")
                                                .replace(" [Del 2]", "")
                                                .replace(" (Kopia)", "")
                                                .trim()
                                                .to_string();

                                            let new_color = match orig.color {
                                                c if c == Theme::FL_CYAN => Theme::FL_ORANGE,
                                                c if c == Theme::FL_ORANGE => Theme::FL_GREEN,
                                                c if c == Theme::FL_GREEN => Theme::FL_PURPLE,
                                                c if c == Theme::FL_PURPLE => Color32::from_rgb(80, 180, 255),
                                                _ => Theme::FL_CYAN,
                                            };

                                            let r_left = AudioRegion {
                                                id: orig.id,
                                                name: format!("{} [Del 1]", clean_title),
                                                start_bar: orig.start_bar,
                                                length_bars: split_offset_bar,
                                                sample_offset_sec: orig.sample_offset_sec,
                                                source_path: orig.source_path.clone(),
                                                waveform_peaks: left_peaks,
                                                volume: orig.volume,
                                                fade_in_bars: orig.fade_in_bars.min(split_offset_bar * 0.5),
                                                fade_out_bars: 0.0,
                                                muted: orig.muted,
                                                is_reverse: orig.is_reverse,
                                                color: orig.color,
                                                loop_length_bars: 0.0,
                                            };

                                            let r_right = AudioRegion {
                                                id: orig.id + 1000 + (self.playlist_tracks[t_idx].regions.len() * 10),
                                                name: format!("{} [Del 2]", clean_title),
                                                start_bar: orig.start_bar + split_offset_bar,
                                                length_bars: ((orig.length_bars - split_offset_bar) * 100.0).round() / 100.0,
                                                sample_offset_sec: ((orig.sample_offset_sec + split_offset_sec) * 100.0).round() / 100.0,
                                                source_path: orig.source_path,
                                                waveform_peaks: right_peaks,
                                                volume: orig.volume,
                                                fade_in_bars: 0.0,
                                                fade_out_bars: orig.fade_out_bars.min((orig.length_bars - split_offset_bar) * 0.5),
                                                muted: orig.muted,
                                                is_reverse: orig.is_reverse,
                                                color: new_color,
                                                loop_length_bars: 0.0,
                                            };

                                            self.push_undo(&format!("Klipp '{}' med saxverktyg", clean_title));
                                            self.playlist_tracks[t_idx].regions.remove(r_idx);
                                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
                                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
                                            performed_split = true;
                                            cut_feedback_msg = crate::tstatus!("✂ Klippte '{}' vid {} (Takt {})! [Ångra: Ctrl+Z]", clean_title, format_time_hundredths(cut_sec), format_bar_subdivisions(orig.start_bar + split_offset_bar));
                                        }
                                    }
                                    if performed_split {
                                        self.sync_track_regions(t_idx);
                                        self.selected_audio_region = Some((t_idx, r_idx));
                                        self.status_message = cut_feedback_msg;
                                    }
                                }

                                if let Some(d_idx) = delete_action
                                    && d_idx < self.playlist_tracks[t_idx].regions.len() {
                                        let name = self.playlist_tracks[t_idx].regions[d_idx].name.clone();
                                        self.push_undo(&format!("Radera '{}'", name));
                                        self.playlist_tracks[t_idx].regions.remove(d_idx);
                                        self.sync_track_regions(t_idx);
                                        self.selected_audio_region = None;
                                        self.status_message = crate::tstatus!("🗑 Raderade region '{}'. [Ångra: Ctrl+Z]", name);
                                    }

                                if let Some(m_idx) = mute_action
                                    && m_idx < self.playlist_tracks[t_idx].regions.len() {
                                        self.push_undo("Muta/Avmuta region");
                                        self.playlist_tracks[t_idx].regions[m_idx].muted = !self.playlist_tracks[t_idx].regions[m_idx].muted;
                                        self.sync_track_regions(t_idx);
                                        let name = self.playlist_tracks[t_idx].regions[m_idx].name.clone();
                                        self.status_message = crate::tstatus!("🔇 Toggla mute för '{}'", name);
                                    }

                                ui.add_space(2.0);
                            }

                            // "+ Add Track" Blank Lane Placeholder on Right
                            let (add_lane_rect, add_lane_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, 28.0), Sense::click());
                            ui.painter().rect_filled(add_lane_rect, Rounding::same(4.0), Color32::from_rgb(15, 17, 22));
                            if add_lane_resp.clicked() {
                                self.show_add_track_modal = true;
                            }

                            if let Some(ref drag_item) = self.sample_drag_item {
                                let pointer_opt = ui.ctx().pointer_latest_pos()
                                    .or_else(|| ui.ctx().pointer_interact_pos())
                                    .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                                    .or_else(|| add_lane_resp.hover_pos());
                                if let Some(mouse_pos) = pointer_opt {
                                    if add_lane_rect.contains(mouse_pos) {
                                        let raw_drop_bar = ((mouse_pos.x - add_lane_rect.min.x) / bar_w).max(0.0);
                                        let raw_drop_sec = raw_drop_bar * sec_per_bar;
                                        let drop_sec = match self.timeline_snap_mode {
                                            TimeSnapMode::FreeHundredth => (raw_drop_sec * 100.0).round() / 100.0,
                                            TimeSnapMode::Snap16th => {
                                                let step_sec = sec_per_bar / 16.0;
                                                (raw_drop_sec / step_sec).round() * step_sec
                                            }
                                            TimeSnapMode::SnapBeat => {
                                                let beat_sec = sec_per_bar / 4.0;
                                                (raw_drop_sec / beat_sec).round() * beat_sec
                                            }
                                            TimeSnapMode::SnapBar => (raw_drop_sec / sec_per_bar).round() * sec_per_bar,
                                        };
                                        let drop_bar = (drop_sec / sec_per_bar).max(0.0);
                                        let item_dur_sec: f32 = 2.0;
                                        let item_dur_bars = (item_dur_sec / sec_per_bar).max(0.25);
                                        let ghost_x_start = add_lane_rect.min.x + drop_bar * bar_w;
                                        let ghost_x_end = (ghost_x_start + item_dur_bars * bar_w).min(add_lane_rect.max.x);
                                        let ghost_rect = Rect::from_min_max(
                                            Pos2::new(ghost_x_start, add_lane_rect.min.y + 2.0),
                                            Pos2::new(ghost_x_end, add_lane_rect.max.y - 2.0),
                                        );
                                        ui.painter().rect_filled(ghost_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(0, 200, 255, 85));
                                        ui.painter().rect_stroke(ghost_rect, Rounding::same(4.0), Stroke::new(1.8_f32, Theme::FL_CYAN));
                                        ui.painter().text(
                                            Pos2::new(ghost_rect.min.x + 6.0, ghost_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            crate::tstatus!("➕ Nytt spår: Släpp {} (Takt {:.2})", drag_item.name, drop_bar + 1.0),
                                            egui::FontId::proportional(10.0),
                                            Color32::WHITE,
                                        );
                                        if ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
                                            drop_sample_action = Some((self.playlist_tracks.len(), drag_item.clone(), drop_bar));
                                        }
                                    }
                                }
                            }

                            ui.add_space(4.0);

                            // Master "Main" Track Lane (Suno Studio Style Master Strip)
                            let (m_lane_rect, _) = ui.allocate_exact_size(Vec2::new(timeline_total_w, row_h), Sense::hover());
                            ui.painter().rect_filled(m_lane_rect, Rounding::ZERO, Color32::from_rgb(14, 16, 22));
                            ui.painter().line_segment([Pos2::new(m_lane_rect.min.x, m_lane_rect.max.y), Pos2::new(m_lane_rect.max.x, m_lane_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 48)));

                            // Master peak meter visualization along the lane
                            let peak = self.engine.get_peak_level();
                            let meter_w = (peak * 350.0).min(timeline_total_w);
                            let meter_rect = Rect::from_min_size(Pos2::new(m_lane_rect.min.x, m_lane_rect.min.y + 12.0), Vec2::new(meter_w, row_h - 24.0));
                            ui.painter().rect_filled(meter_rect, Rounding::same(2.0), Color32::from_rgba_unmultiplied(255, 140, 0, 45));

                            ui.painter().text(
                                Pos2::new(m_lane_rect.min.x + 12.0, m_lane_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                crate::tstatus!("Stereo Master Utgång • Volym: {:.0}% • Peak: {:.2}", self.master_volume * 100.0, peak),
                                egui::FontId::proportional(10.0),
                                Color32::from_rgb(130, 140, 160),
                            );

                            // ====================================================
                            // AUTOMATION CURVE EDITOR (Fas 5.4)
                            // ====================================================
                            let mut auto_bottom = m_lane_rect.max.y;
                            if self.show_automation && !self.playlist_tracks.is_empty() {
                                let sel = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
                                let auto_h = 96.0;
                                let (auto_rect, auto_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, auto_h), Sense::click_and_drag());
                                self.render_automation_lane(ui, auto_rect, &auto_resp, bar_w, sec_per_bar, sel);
                                auto_bottom = auto_rect.max.y;
                            }

                            // ====================================================
                            // 2.3 VERTICAL PLAYHEAD NEEDLE ACROSS ALL TRACKS
                            // ====================================================
                            let playhead_bar = self.song_time / sec_per_bar;
                            let playhead_x = timeline_left_x + playhead_bar * bar_w;
                            let track_area_top = ruler_rect.min.y;
                            let track_area_bottom = auto_bottom;

                            if playhead_x >= timeline_left_x && playhead_x <= timeline_left_x + timeline_total_w {
                                // Auto-scroll timeline to follow playhead when enabled and playing
                                if self.timeline_auto_scroll && self.is_playing {
                                    let follow_rect = Rect::from_min_max(
                                        Pos2::new(playhead_x - 60.0, track_area_top),
                                        Pos2::new(playhead_x + 60.0, track_area_top + 10.0),
                                    );
                                    ui.scroll_to_rect(follow_rect, Some(egui::Align::Center));
                                }

                                // Glowing line strictly bounded to track lanes
                                ui.painter().line_segment(
                                    [Pos2::new(playhead_x, track_area_top), Pos2::new(playhead_x, track_area_bottom)],
                                    Stroke::new(1.8_f32, Theme::FL_ORANGE),
                                );

                                // Triangle head badge on ruler
                                let tri = [
                                    Pos2::new(playhead_x - 5.0, track_area_top),
                                    Pos2::new(playhead_x + 5.0, track_area_top),
                                    Pos2::new(playhead_x, track_area_top + 8.0),
                                ];
                                ui.painter().add(egui::Shape::convex_polygon(tri.to_vec(), Theme::FL_ORANGE, Stroke::NONE));
                            }
                        });
                    });
            });

            // Floating drag badge tooltip that follows mouse cursor in foreground tooltip layer
            if let Some(ref drag_item) = self.sample_drag_item {
                let pointer_opt = ui.ctx().pointer_latest_pos()
                    .or_else(|| ui.ctx().pointer_interact_pos())
                    .or_else(|| ui.input(|i| i.pointer.hover_pos()));
                if let Some(pos) = pointer_opt {
                    let tooltip_rect = Rect::from_min_size(Pos2::new(pos.x + 16.0, pos.y + 12.0), Vec2::new(200.0, 26.0));
                    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("sample_drag_tooltip")));
                    painter.rect_filled(tooltip_rect, Rounding::same(4.0), Color32::from_rgb(20, 26, 38));
                    painter.rect_stroke(tooltip_rect, Rounding::same(4.0), Stroke::new(1.5_f32, Theme::FL_CYAN));
                    painter.text(
                        Pos2::new(tooltip_rect.min.x + 8.0, tooltip_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("📦 Dra: {} ({})", drag_item.name, drag_item.category),
                        egui::FontId::proportional(10.5),
                        Color32::WHITE,
                    );
                }
            }

            // Process Sample Drop onto Playlist Timeline
            if let Some((target_t, item, drop_bar)) = drop_sample_action {
                self.add_sample_item_to_track_at_bar(target_t, &item, drop_bar);
                self.sample_drag_item = None;
            } else if self.sample_drag_item.is_some() && ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
                self.sample_drag_item = None;
            }

            // Execute deferred track actions safely AFTER UI rendering
            if let Some(del_idx) = track_delete_idx {
                self.delete_track(del_idx);
            }
            if let Some(dup_idx) = track_duplicate_idx {
                self.duplicate_track(dup_idx);
            }
            if let Some(paste_idx) = track_paste_idx {
                self.selected_timeline_track = paste_idx;
                self.paste_region();
            }

            // Apply scrub seeking if user interacted with ruler
            if let Some(target) = scrub_target_sec {
                self.seek_song_time(target);
            }

            // ================================================================
            // 2.5 SELECTED AUDIO REGION INSPECTOR (RESPONSIVE 2-TIER / WRAPPED)
            // ================================================================
            if let Some((t_idx, r_idx)) = self.selected_audio_region
                && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                    let mut do_delete = false;
                    let mut do_split = false;
                    let mut do_copy = false;
                    let mut do_paste = false;
                    let mut do_loop_factor: Option<f32> = None;
                    let mut do_duplicate = false;
                    let mut do_reverse = false;
                    let mut do_open_focus = false;
                    let mut do_open_vocal_studio = false;
                    let mut do_save_sample = false;
                    let has_copied = self.copied_region.is_some();
                    let copied_name = self.copied_region.as_ref().map(|c| c.name.clone()).unwrap_or_default();
                    let r = &mut self.playlist_tracks[t_idx].regions[r_idx];
                    let r_start_sec = r.start_bar * sec_per_bar;
                    let r_len_sec = r.length_bars * sec_per_bar;
                    let r_name = r.name.clone();
                    let is_rev = r.is_reverse;

                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        ui.spacing_mut().slider_width = 75.0;

                        // Row 1: Title, Time offsets, Sliders (Responsive Wrapped)
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(format!("🎵 {}", r_name)).strong().color(Theme::FL_CYAN));
                            ui.separator();

                            // Start time readout with coarse/fine nudging
                            ui.label(crate::i18n::t("⏱ Start:"));
                            ui.label(egui::RichText::new(format!("{:.2}t ({})", r.start_bar + 1.0, format_time_hundredths(r_start_sec))).monospace().strong().color(Theme::TEXT_BRIGHT));
                            if ui.button(crate::i18n::t("-1t")).on_hover_text(crate::i18n::t("Flytta 1 takt bakåt")).clicked() {
                                r.start_bar = (r.start_bar - 1.0).max(0.0);
                            }
                            if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Flytta 0.1s bakåt")).clicked() {
                                r.start_bar = ((r_start_sec - 0.1).max(0.0)) / sec_per_bar;
                            }
                            if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Flytta 0.1s framåt")).clicked() {
                                r.start_bar = (r_start_sec + 0.1) / sec_per_bar;
                            }
                            if ui.button(crate::i18n::t("+1t")).on_hover_text(crate::i18n::t("Flytta 1 takt framåt")).clicked() {
                                r.start_bar += 1.0;
                            }

                            ui.separator();

                            // Duration readout with coarse/fine nudging
                            ui.label(crate::i18n::t("📏 Längd:"));
                            ui.label(egui::RichText::new(format!("{:.2}t ({})", r.length_bars, format_time_hundredths(r_len_sec))).monospace().strong().color(Theme::TEXT_BRIGHT));
                            if ui.button(crate::i18n::t("-1t")).on_hover_text(crate::i18n::t("Korta 1 takt")).clicked() {
                                r.length_bars = (r.length_bars - 1.0).max(0.05);
                            }
                            if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Korta 0.1s")).clicked() {
                                r.length_bars = ((r_len_sec - 0.1).max(0.05)) / sec_per_bar;
                            }
                            if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Förläng 0.1s")).clicked() {
                                r.length_bars = (r_len_sec + 0.1) / sec_per_bar;
                            }
                            if ui.button(crate::i18n::t("+1t")).on_hover_text(crate::i18n::t("Förläng 1 takt")).clicked() {
                                r.length_bars += 1.0;
                            }

                            ui.separator();

                            // Start-trim offset readout
                            ui.label(crate::i18n::t("✂ Start-trim:"));
                            ui.label(egui::RichText::new(format!("{:.2}s", r.sample_offset_sec)).monospace().strong().color(Theme::FL_CYAN));
                            if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Minska start-trim (visa mer av början)")).clicked() {
                                r.sample_offset_sec = (r.sample_offset_sec - 0.1).max(0.0);
                            }
                            if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Öka start-trim (klipp bort mer av början)")).clicked() {
                                r.sample_offset_sec += 0.1;
                            }

                            ui.separator();

                            // Gain Slider
                            ui.label(crate::i18n::t("🎚 Vol:"));
                            ui.add(egui::Slider::new(&mut r.volume, 0.0..=2.0).custom_formatter(|v, _| {
                                let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                                format!("{:.0}dB", db)
                            }));

                            ui.separator();

                            // Fade In Slider
                            ui.label(crate::i18n::t("📈 In:"));
                            ui.add(egui::Slider::new(&mut r.fade_in_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                                format!("{:.2}s", v as f32 * sec_per_bar)
                            }));

                            ui.separator();

                            // Fade Out Slider
                            ui.label(crate::i18n::t("📉 Ut:"));
                            ui.add(egui::Slider::new(&mut r.fade_out_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                                format!("{:.2}s", v as f32 * sec_per_bar)
                            }));
                        });

                        ui.separator();

                        // Row 2: Action Buttons (Always clearly visible & wrapped)
                        ui.horizontal_wrapped(|ui| {
                            let cur_song_time = self.song_time;
                            let can_cut = cur_song_time > r_start_sec + 0.02 && cur_song_time < r_start_sec + r_len_sec - 0.02;
                            let cut_label = if can_cut {
                                format!("✂ Dela vid spelhuvud ({})", format_time_hundredths(cur_song_time))
                            } else {
                                "✂ Dela vid spelhuvud (S / Ctrl+B)".to_string()
                            };
                            let cut_bg = if can_cut { Theme::FL_GREEN } else { Color32::from_rgb(45, 60, 50) };
                            let cut_fg = if can_cut { Color32::BLACK } else { Theme::TEXT_MUTED };

                            if ui.add(egui::Button::new(egui::RichText::new(cut_label).strong().size(10.5).color(cut_fg)).fill(cut_bg))
                                .on_hover_text(crate::i18n::t("Klipp/dela regionen på exakt denna tidpunkt"))
                                .clicked() {
                                    do_split = true;
                                }

                            // Copy
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📋 Kopiera (Ctrl+C)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 60, 95)))
                                .on_hover_text(crate::i18n::t("Kopiera detta sample till urklipp"))
                                .clicked() {
                                    do_copy = true;
                                }

                            // Paste (if clipboard has region)
                            if has_copied {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("📋 Klistra in ({}) (Ctrl+V)", copied_name)).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(35, 85, 125)))
                                    .on_hover_text(crate::i18n::t("Klistra in kopierat sample vid spelhuvudet på aktivt spår"))
                                    .clicked() {
                                        do_paste = true;
                                    }
                            }

                            // Duplicate
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📋 Duplicera (Ctrl+D)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(35, 75, 115)))
                                .on_hover_text(crate::i18n::t("Duplicera samplen direkt efter den nuvarande"))
                                .clicked() {
                                    do_duplicate = true;
                                }

                            // Loop x2 / x4
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Loop x2 (Ctrl+L)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(20, 85, 100)))
                                .on_hover_text(crate::i18n::t("Repetera och fördubbla loop-längden"))
                                .clicked() {
                                    do_loop_factor = Some(2.0);
                                }
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Loop x4")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(18, 75, 90)))
                                .on_hover_text(crate::i18n::t("Repetera samplen 4 gånger i följd"))
                                .clicked() {
                                    do_loop_factor = Some(4.0);
                                }

                            // Reverse
                            let rev_label = if is_rev { crate::i18n::t("🔄 Normal (Framlänges)") } else { crate::i18n::t("🔄 Vänd baklänges (Reverse)") };
                            let rev_bg = if is_rev { Theme::FL_ORANGE } else { Color32::from_rgb(70, 45, 90) };
                            if ui.add(egui::Button::new(egui::RichText::new(rev_label).strong().size(10.5).color(Color32::WHITE)).fill(rev_bg))
                                .on_hover_text(crate::i18n::t("Spela upp ljudregionen baklänges i realtid (Ctrl+K / R)"))
                                .clicked() {
                                    do_reverse = true;
                                }

                            // Mute toggle
                            let mute_bg = if r.muted { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                            if ui.add(egui::Button::new(egui::RichText::new(if r.muted { "🔇 Mutad" } else { "🔊 Aktiv" }).strong().size(10.5).color(Color32::WHITE)).fill(mute_bg)).clicked() {
                                r.muted = !r.muted;
                            }

                            // Save to Sound Browser
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara i Sound Browser")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(32, 95, 80)))
                                .on_hover_text(crate::i18n::t("Spara denna ljudregion som sample i Sound Browser & på disk"))
                                .clicked() {
                                    do_save_sample = true;
                                }

                            // Open Stem Focus Editor
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔍 Öppna Stämeditor")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(140, 70, 20))).clicked() {
                                do_open_focus = true;
                            }

                            // Open in Vocal Studio
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Öppna i Sångstudio")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 90, 120)))
                                .on_hover_text(crate::i18n::t("Ladda in detta sample/region i Sångstudion för isolerad provspelning, pitch, time stretch & effekter"))
                                .clicked() {
                                    do_open_vocal_studio = true;
                                }

                            // Delete
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗑 Ta bort (Del)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(160, 40, 40))).clicked() {
                                do_delete = true;
                            }

                            // Close Inspector
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("❌ Stäng")).size(10.0))).clicked() {
                                self.selected_audio_region = None;
                            }
                        });
                    });

                    if do_open_focus {
                        self.open_stem_focus(t_idx);
                    } else if do_open_vocal_studio {
                        self.open_region_in_vocal_studio(t_idx, r_idx);
                    } else if do_copy {
                        self.copy_selected_region();
                    } else if do_paste {
                        self.paste_region();
                    } else if let Some(factor) = do_loop_factor {
                        self.repeat_selected_region_loop(factor);
                    } else if do_save_sample {
                        self.save_region_to_sound_browser(t_idx, r_idx);
                    } else if do_reverse {
                        self.reverse_selected_region();
                    } else if do_delete {
                        self.delete_selected_region();
                    } else if do_duplicate {
                        self.duplicate_selected_region();
                    } else if do_split {
                        self.split_selected_region_at_playhead();
                    } else {
                        self.sync_track_regions(t_idx);
                    }
                }

            ui.add_space(6.0);

            // ================================================================
            // 3. SUNO STUDIO FLOATING AI PROMPT & GENERATION CAPSULE (BETA)
            // ================================================================
            ui.vertical_centered(|ui| {
                ui.group(|ui| {
                    ui.set_max_width(680.0);
                    ui.horizontal(|ui| {
                        // Beta Tag
                        ui.label(egui::RichText::new(crate::i18n::t("BETA")).strong().size(9.0).color(Theme::FL_ORANGE));
                        ui.separator();

                        // Add Context (+)
                        if ui.button(crate::i18n::t("➕")).on_hover_text(crate::i18n::t("Bifoga aktuellt spår/mix som AI-kontext")).clicked() {
                            self.suno_context_clip = Some(format!("{} - Master Mix", self.project_name));
                        }

                        // Context Chip
                        let mut clear_context = false;
                        if let Some(ref ctx_clip) = self.suno_context_clip {
                            let fill = Color32::from_rgb(60, 35, 100);
                            let clip_label = format!("🟣 {}", ctx_clip);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(clip_label).size(10.0).color(Color32::WHITE));
                                    if ui.add(egui::Button::new(crate::i18n::t("✕")).small().fill(fill)).clicked() {
                                        clear_context = true;
                                    }
                                });
                            });
                        }
                        if clear_context {
                            self.suno_context_clip = None;
                        }

                        // Prompt Text Field
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.suno_prompt_input)
                                .hint_text("Select a spot on the timeline and ask me to add a part...")
                                .desired_width(320.0),
                        );

                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.trigger_suno_ai_generation();
                        }

                        // Model Selector Pill (Local & Open Source AI Engines)
                        egui::ComboBox::from_id_salt("suno_model_combo")
                            .selected_text(&self.suno_model_version)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.suno_model_version, "v5.5".to_string(), "v5.5");
                                ui.selectable_value(&mut self.suno_model_version, "ACE-Step (Lokal Suno)".to_string(), "ACE-Step (Lokal Suno)");
                                ui.selectable_value(&mut self.suno_model_version, "Stable Audio 3.0".to_string(), "Stable Audio 3.0");
                                ui.selectable_value(&mut self.suno_model_version, "Meta MusicGen".to_string(), "MusicGen (Lokal)");
                                ui.selectable_value(&mut self.suno_model_version, "Demucs UVR5".to_string(), "Demucs UVR5");
                            });

                        // Submit / Generate Button (↑)
                        let gen_bg = if self.suno_is_generating { Color32::RED } else { Theme::FL_GREEN };
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ⬆ ")).strong().size(12.0).color(Color32::WHITE)).fill(gen_bg)).clicked() {
                            self.trigger_suno_ai_generation();
                        }
                    });
                });

                ui.add_space(4.0);

                // Center Action: "+ Add Track Effects" Button (Suno Studio Style)
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Add Track Effects")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 48, 65))).clicked() {
                    self.view_mode = ViewMode::EffectsMixer;
                }
            });

            ui.add_space(4.0);

            // ================================================================
            // 4. SUNO STUDIO BOTTOM UTILITY BAR (Quick Switcher, Browser, Guide)
            // ================================================================
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

                // Mode Switcher Icons
                if ui.selectable_label(self.view_mode == ViewMode::EffectsMixer, "🎛 Mixer").clicked() {
                    self.view_mode = ViewMode::EffectsMixer;
                }
                if ui.selectable_label(self.view_mode == ViewMode::PianoRoll, "✏ Piano Roll").clicked() {
                    self.view_mode = ViewMode::PianoRoll;
                }
                if ui.selectable_label(self.view_mode == ViewMode::ChannelRack, "☰ Channel Rack").clicked() {
                    self.view_mode = ViewMode::ChannelRack;
                }
                if ui.selectable_label(self.view_mode == ViewMode::VocalStudio, "🎤 Vocal Studio").clicked() {
                    self.view_mode = ViewMode::VocalStudio;
                }

                ui.separator();

                if ui.button(crate::i18n::t("💡 Snabbguide (F1)")).clicked() {
                    self.show_help_guide = !self.show_help_guide;
                }

                if ui.button(crate::i18n::t("🗂 Projektfiler")).clicked() {
                    self.show_browser = !self.show_browser;
                }
            });
        });
    }

    fn render_channel_rack(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // Pattern Selector Bar (FL Studio Style)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX CHANNEL RACK")).strong().size(13.0).color(Theme::FL_ORANGE));
                ui.separator();

                ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(11.0).color(Theme::TEXT_MUTED));
                let p_len = self.patterns.len();
                for pat_idx in 0..p_len {
                    let is_active = self.selected_pattern == pat_idx;
                    let p_color = self.patterns[pat_idx].color;
                    let fill = if is_active { p_color } else { Theme::PANEL_BG };
                    let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                    let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                    if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(11.0).color(text_color)).fill(fill)).clicked() {
                        self.select_pattern(pat_idx);
                    }
                }

                ui.separator();

                // Swing Knob
                rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 22.0);

                ui.separator();

                if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Ljud")).color(Theme::FL_GREEN)).clicked() {
                    self.show_import_modal = true;
                }

                if ui.button(crate::i18n::t("⚡ Slumpa Beats")).clicked() {
                    for ch in &mut self.channels {
                        for s in 0..16 {
                            ch.steps[s] = (s % 4 == 0) || (rand_simple(s * 13 + ch.name.len()) > 0.68);
                        }
                    }
                    self.sync_active_pattern_from_ui();
                }
                if ui.button(crate::i18n::t("🗑 Rensa Steg")).clicked() {
                    for ch in &mut self.channels {
                        ch.steps = [false; 16];
                    }
                    self.sync_active_pattern_from_ui();
                }
                if ui.button(crate::i18n::t("🎹 Öppna Piano Roll")).clicked() {
                    self.view_mode = ViewMode::PianoRoll;
                }
                if ui.button(crate::i18n::t("🎼 Öppna Arranger")).clicked() {
                    self.view_mode = ViewMode::PlaylistArranger;
                }
            });

            ui.add_space(8.0);

            let mut toggle_picker_for = None;
            let mut toggle_chopper_for = None;

            for ch_idx in 0..self.channels.len() {
                let is_selected = self.selected_channel == ch_idx;
                let is_picker_open = self.active_sample_picker_channel == Some(ch_idx);
                let is_chopper_open = self.active_chopper_channel == Some(ch_idx);
                let ch = &mut self.channels[ch_idx];

                ui.horizontal(|ui| {
                    // Mute & Solo LEDs
                    let mute_color = if ch.muted { Color32::from_rgb(50, 20, 20) } else { Theme::FL_GREEN };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).strong().size(10.0).color(Color32::WHITE)).fill(mute_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
                        ch.muted = !ch.muted;
                    }
                    let solo_color = if ch.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).strong().size(10.0).color(Color32::WHITE)).fill(solo_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
                        ch.solo = !ch.solo;
                    }

                    // VOL & PAN Knobs
                    rotary_knob(ui, &mut ch.volume, 0.0, 1.0, "VOL", ch.color, 24.0);
                    rotary_knob(ui, &mut ch.pan, -1.0, 1.0, "PAN", Color32::from_rgb(180, 190, 200), 24.0);

                    // Channel Select Button
                    let btn_fill = if is_selected { Theme::PANEL_BG } else { Theme::CHANNEL_BG };
                    let ch_btn = egui::Button::new(egui::RichText::new(format!("{} {}", ch.icon, ch.name)).strong().color(ch.color))
                        .fill(btn_fill)
                        .min_size(Vec2::new(110.0, 26.0));
                    if ui.add(ch_btn).clicked() {
                        self.selected_channel = ch_idx;
                    }

                    // Quick Switch Sound Button [🔄 Byt]
                    let byt_fill = if is_picker_open { Theme::FL_CYAN } else { Color32::from_rgb(32, 38, 48) };
                    let byt_text_color = if is_picker_open { Color32::BLACK } else { Theme::FL_CYAN };
                    let byt_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Byt")).size(10.0).color(byt_text_color))
                        .fill(byt_fill)
                        .min_size(Vec2::new(42.0, 24.0));
                    if ui.add(byt_btn).on_hover_text(crate::i18n::t("Byt ut detta beat/ljud direkt från biblioteket")).clicked() {
                        toggle_picker_for = Some(ch_idx);
                    }

                    // Chop & Pitch Inspector Button [✂ Chop]
                    let chop_fill = if is_chopper_open { Theme::FL_YELLOW } else { Color32::from_rgb(38, 36, 28) };
                    let chop_text_color = if is_chopper_open { Color32::BLACK } else { Theme::FL_YELLOW };
                    let chop_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Chop")).size(10.0).color(chop_text_color))
                        .fill(chop_fill)
                        .min_size(Vec2::new(52.0, 24.0));
                    if ui.add(chop_btn).on_hover_text(crate::i18n::t("Öppna Waveform Chopper & Pitch Slicer")).clicked() {
                        toggle_chopper_for = Some(ch_idx);
                    }

                    ui.add_space(4.0);

                    // 16 FL-Studio Style Tactile Step Buttons
                    for step_idx in 0..16 {
                        let is_active = ch.steps[step_idx];
                        let is_playhead = self.is_playing && self.current_step == step_idx;
                        let beat_group_a = (step_idx / 4) % 2 == 0;

                        let resp = fl_step_button(
                            ui,
                            is_active,
                            is_playhead,
                            beat_group_a,
                            ch.color,
                            Vec2::new(26.0, 24.0),
                        );

                        if resp.clicked() {
                            ch.steps[step_idx] = !is_active;
                        }
                    }
                });
                ui.add_space(3.0);
            }

            if let Some(ch_idx) = toggle_picker_for {
                self.active_sample_picker_channel = if self.active_sample_picker_channel == Some(ch_idx) { None } else { Some(ch_idx) };
            }
            if let Some(ch_idx) = toggle_chopper_for {
                self.active_chopper_channel = if self.active_chopper_channel == Some(ch_idx) { None } else { Some(ch_idx) };
            }

            // Interactive Sound Library Picker Overlay
            if let Some(picker_ch) = self.active_sample_picker_channel {
                ui.add_space(6.0);
                ui.group(|ui| {
                    let cur_name = self.channels.get(picker_ch).map(|c| c.name.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::tstatus!("📚 BYT LJUD: Välj sample för Kanal {} ({})", picker_ch + 1, cur_name)).strong().size(12.5).color(Theme::FL_CYAN));
                        ui.separator();
                        if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Eget Ljud")).color(Theme::FL_GREEN)).clicked() {
                            self.show_import_modal = true;
                        }
                        if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Väljare")).color(Color32::from_rgb(255, 100, 100))).clicked() {
                            self.active_sample_picker_channel = None;
                        }
                    });

                    ui.add_space(4.0);

                    // Category Filter Tabs
                    let categories = ["Alla", "Kicks", "Snares", "Claps", "Hi-Hats", "Percussion", "Vocal Chops", "808 & Bass", "Egna Importerade"];
                    ui.horizontal_wrapped(|ui| {
                        for (cat_i, &cat_name) in categories.iter().enumerate() {
                            let is_sel = self.selected_lib_category == cat_i;
                            let fill = if is_sel { Theme::FL_CYAN } else { Theme::PANEL_BG };
                            let text_col = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(cat_name).strong().size(11.0).color(text_col)).fill(fill)).clicked() {
                                self.selected_lib_category = cat_i;
                            }
                        }
                    });

                    ui.add_space(4.0);

                    // Sample Items Grid
                    let sel_cat = categories[self.selected_lib_category.min(categories.len() - 1)];
                    let items_to_show: Vec<LibrarySampleItem> = self.sample_library.iter()
                        .filter(|item| sel_cat == "Alla" || item.category == sel_cat)
                        .cloned()
                        .collect();

                    let mut chosen_item = None;
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        egui::Grid::new("sample_picker_grid").num_columns(4).spacing([8.0, 8.0]).show(ui, |ui| {
                            for (i, item) in items_to_show.iter().enumerate() {
                                ui.group(|ui| {
                                    ui.set_width(190.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&item.icon).size(15.0));
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(&item.name).strong().size(11.0).color(item.color));
                                            ui.label(egui::RichText::new(&item.category).size(9.0).color(Theme::TEXT_MUTED));
                                        });
                                    });

                                     // Mini wave preview
                                    let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(175.0, 18.0), egui::Sense::hover());
                                    ui.painter().rect_filled(wf_rect, Rounding::same(2.0), Color32::from_rgb(16, 18, 22));
                                    let mid_y = wf_rect.center().y;
                                    let num_pts = item.waveform.len().max(1);
                                    let dx = wf_rect.width() / num_pts as f32;
                                    for (wi, &val) in item.waveform.iter().enumerate() {
                                        let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                        let h = (val.abs() * 7.5).max(1.0);
                                        ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.3_f32, item.color));
                                    }

                                    ui.horizontal(|ui| {
                                        if ui.button(egui::RichText::new(crate::i18n::t("▶ Prov")).size(9.5)).on_hover_text(crate::i18n::t("Provspela med riktigt ljud")).clicked() {
                                            self.audition_library_sample(item);
                                        }
                                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✅ Välj")).strong().size(9.5).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                                            chosen_item = Some(item.clone());
                                        }
                                    });
                                });

                                if (i + 1) % 4 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    });

                    if let Some(item) = chosen_item {
                        self.select_sound_for_channel(picker_ch, &item);
                        self.active_sample_picker_channel = None;
                    }
                });
            }

            // Interactive Sample Chopper & Pitch Transpose Inspector
            if let Some(chop_ch) = self.active_chopper_channel
                && chop_ch < self.channels.len() {
                    ui.add_space(6.0);
                    ui.group(|ui| {
                        let ch = &mut self.channels[chop_ch];
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("✂ SAMPLE CHOPPER & PITCH INSPECTOR - Kanal {} ({} {})", chop_ch + 1, ch.icon, ch.name)).strong().size(12.5).color(Theme::FL_YELLOW));
                            ui.separator();
                            if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela Chop")).color(Theme::FL_GREEN)).clicked() {
                                let note = (ch.notes[0] as i16 + ch.pitch_semitones as i16).clamp(0, 127) as u8;
                                let base_freq = midi_to_freq(note);
                                let fine_mul = (ch.pitch_fine_cents / 1200.0).exp2();
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: base_freq * fine_mul, velocity: ch.volume });
                            }
                            if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Chopper")).color(Color32::from_rgb(255, 100, 100))).clicked() {
                                self.active_chopper_channel = None;
                            }
                        });

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            // Waveform Chopper Slicer
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Waveform Chop & Slice Trim:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                                let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(380.0, 65.0), egui::Sense::click_and_drag());
                                ui.painter().rect_filled(wf_rect, Rounding::same(3.0), Color32::from_rgb(14, 16, 20));
                                ui.painter().rect_stroke(wf_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 65)));

                                let mid_y = wf_rect.center().y;
                                let num_pts = ch.waveform_preview.len().max(1);
                                let dx = wf_rect.width() / num_pts as f32;
                                for (wi, &val) in ch.waveform_preview.iter().enumerate() {
                                    let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                    let h = (val.abs() * 26.0).max(1.0);
                                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.8_f32, ch.color));
                                }

                                let start_x = wf_rect.min.x + ch.sample_start * wf_rect.width();
                                let end_x = wf_rect.min.x + ch.sample_end * wf_rect.width();

                                if ch.sample_start > 0.0 {
                                    let dim_rect = Rect::from_min_max(wf_rect.min, Pos2::new(start_x, wf_rect.max.y));
                                    ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                                }
                                if ch.sample_end < 1.0 {
                                    let dim_rect = Rect::from_min_max(Pos2::new(end_x, wf_rect.min.y), wf_rect.max);
                                    ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                                }

                                let active_slice_rect = Rect::from_min_max(Pos2::new(start_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y));
                                ui.painter().rect_stroke(active_slice_rect, Rounding::ZERO, Stroke::new(1.5_f32, Theme::FL_YELLOW));
                                ui.painter().line_segment([Pos2::new(start_x, wf_rect.min.y), Pos2::new(start_x, wf_rect.max.y)], Stroke::new(2.5_f32, Theme::FL_GREEN));
                                ui.painter().line_segment([Pos2::new(end_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y)], Stroke::new(2.5_f32, Color32::from_rgb(255, 70, 70)));

                                // Quick Slicer Presets
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("Chop Snabbval:")).size(10.0).color(Theme::TEXT_MUTED));
                                    if ui.button(crate::i18n::t("Fullt")).clicked() { ch.sample_start = 0.0; ch.sample_end = 1.0; }
                                    if ui.button(crate::i18n::t("1/2")).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.5; }
                                    if ui.button(crate::i18n::t("2/2")).clicked() { ch.sample_start = 0.5; ch.sample_end = 1.0; }
                                    if ui.button(crate::i18n::t("1/4")).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.25; }
                                    if ui.button(crate::i18n::t("2/4")).clicked() { ch.sample_start = 0.25; ch.sample_end = 0.5; }
                                    if ui.button(crate::i18n::t("3/4")).clicked() { ch.sample_start = 0.50; ch.sample_end = 0.75; }
                                    if ui.button(crate::i18n::t("4/4")).clicked() { ch.sample_start = 0.75; ch.sample_end = 1.0; }
                                    if ui.button(crate::i18n::t("⚡ Transient")).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.18; }
                                });
                            });

                            ui.separator();

                            // Pitch & Envelope Controls
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Pitch & Tonhöjd:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Semitoner:"));
                                    ui.add(egui::Slider::new(&mut ch.pitch_semitones, -24..=24).text("st"));
                                    if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_semitones = 0; }
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Fine Cents:"));
                                    ui.add(egui::Slider::new(&mut ch.pitch_fine_cents, -100.0..=100.0).text("ct"));
                                    if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_fine_cents = 0.0; }
                                });

                                let total_cents = ch.pitch_semitones as f32 * 100.0 + ch.pitch_fine_cents;
                                let semitone_text = if ch.pitch_semitones >= 0 { format!("+{}", ch.pitch_semitones) } else { format!("{}", ch.pitch_semitones) };
                                ui.label(egui::RichText::new(format!("Totalt: {} st ({:+.0} ct)", semitone_text, total_cents)).color(Theme::FL_CYAN).size(10.0));

                                ui.separator();

                                ui.label(egui::RichText::new(crate::i18n::t("Chop Trim & Envelope:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Start Trim:"));
                                    ui.add(egui::Slider::new(&mut ch.sample_start, 0.0..=1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("End Trim:"));
                                    ui.add(egui::Slider::new(&mut ch.sample_end, 0.0..=1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Attack / Decay:"));
                                    ui.add(egui::Slider::new(&mut ch.attack_decay, 0.01..=1.0));
                                });
                                ui.checkbox(&mut ch.is_reverse, "🔄 Reverse Waveform");
                            });
                        });
                    });
                }

            self.sync_active_pattern_from_ui();
        });
    }

    fn render_add_track_modal(&mut self, ctx: &egui::Context) {
        let import_req = render_add_track_modal(
            &mut self.show_add_track_modal,
            &mut self.add_track_state,
            &mut self.playlist_tracks,
            &mut self.status_message,
            ctx,
        );
        if let Some(tmpl) = import_req {
            self.pending_add_track_import = Some(tmpl);
            self.spawn_async_file_picker(
                "WAV",
                &["wav", "WAV"],
                crate::i18n::t("Välj ljudfil (WAV) att importera"),
            );
        }
    }

    fn render_import_sample_modal(&mut self, ctx: &egui::Context) {
        if !self.show_import_modal {
            return;
        }

        let mut close = false;
        egui::Window::new(crate::i18n::t("📥 Importera & Skapa Nytt Sample Ljud"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(egui::RichText::new(crate::i18n::t("Utöka ditt personliga ljudbibliotek med egna samples!")).size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Ljudets Namn:"));
                    ui.text_edit_singleline(&mut self.custom_import_name_input);
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Kategori:"));
                    egui::ComboBox::from_id_salt("import_cat_combo")
                        .selected_text(&self.custom_import_category)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.custom_import_category, "Egna Importerade".to_string(), "Egna Importerade");
                            ui.selectable_value(&mut self.custom_import_category, "Kicks".to_string(), "Kicks");
                            ui.selectable_value(&mut self.custom_import_category, "Snares".to_string(), "Snares");
                            ui.selectable_value(&mut self.custom_import_category, "Claps".to_string(), "Claps");
                            ui.selectable_value(&mut self.custom_import_category, "Hi-Hats".to_string(), "Hi-Hats");
                            ui.selectable_value(&mut self.custom_import_category, "Percussion".to_string(), "Percussion");
                            ui.selectable_value(&mut self.custom_import_category, "Vocal Chops".to_string(), "Vocal Chops");
                            ui.selectable_value(&mut self.custom_import_category, "808 & Bass".to_string(), "808 & Bass");
                        });
                });

                ui.add_space(8.0);
                ui.label(egui::RichText::new(crate::i18n::t("Sample Karaktär & Syntes:")).strong().size(11.0));

                let icons = ["📂", "💥", "🥁", "👏", "⚡", "🌊", "🗣", "🎸", "🎹", "✨", "🔔"];
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::i18n::t("Ikon:"));
                    for &ic in icons.iter() {
                        let is_sel = self.custom_import_icon == ic;
                        let fill = if is_sel { Theme::FL_GREEN } else { Theme::PANEL_BG };
                        if ui.add(egui::Button::new(ic).fill(fill)).clicked() {
                            self.custom_import_icon = ic.to_string();
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Grundton (MIDI):"));
                    ui.add(egui::Slider::new(&mut self.custom_import_note, 24..=84).text("Ton"));
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Frekvens & Tonhöjd:"));
                    ui.add(egui::Slider::new(&mut self.custom_import_freq, 10.0..=90.0));
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Decay Tid:"));
                    ui.add(egui::Slider::new(&mut self.custom_import_decay, 0.1..=1.5));
                });

                ui.add_space(8.0);

                // Waveform Preview
                let preview_wave: Vec<f32> = (0..32).map(|i| {
                    let t = i as f32 / 32.0;
                    let env = (-t * self.custom_import_decay * 4.0).exp();
                    (t * self.custom_import_freq).sin() * env
                }).collect();

                let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(380.0, 32.0), egui::Sense::hover());
                ui.painter().rect_filled(wf_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 24));
                let mid_y = wf_rect.center().y;
                let dx = wf_rect.width() / 32.0;
                for (wi, &val) in preview_wave.iter().enumerate() {
                    let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                    let h = (val.abs() * 13.0).max(1.0);
                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.5_f32, Theme::FL_GREEN));
                }

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela")).size(11.0)).clicked() {
                        let freq = midi_to_freq(self.custom_import_note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn {
                            note: self.custom_import_note,
                            freq,
                            velocity: 0.9,
                        });
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara i Bibliotek")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                        self.import_custom_sample(
                            self.custom_import_name_input.clone(),
                            self.custom_import_category.clone(),
                            self.custom_import_icon.clone(),
                            self.custom_import_note,
                            Color32::from_rgb(255, 200, 80),
                            self.custom_import_freq,
                            self.custom_import_decay,
                        );
                        close = true;
                    }

                    if ui.button(egui::RichText::new(crate::i18n::t("Avbryt")).size(11.0)).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_import_modal = false;
        }
    }

    pub fn get_scale_notes(&self) -> Vec<u8> {
        let root = self.piano_roll_root_note % 12;
        let intervals: &[u8] = match self.selected_scale {
            1 => &[0, 2, 4, 5, 7, 9, 11], // Dur (Major)
            2 => &[0, 2, 3, 5, 7, 8, 10], // Moll (Minor)
            3 => &[0, 2, 3, 5, 7, 8, 11], // Harmonisk Moll
            4 => &[0, 2, 3, 5, 7, 9, 11], // Melodisk Moll
            5 => &[0, 2, 3, 5, 7, 9, 10], // Dorian
            6 => &[0, 2, 4, 5, 7, 9, 10], // Mixolydian
            7 => &[0, 2, 4, 7, 9],        // Pentatonisk Dur
            8 => &[0, 3, 5, 7, 10],       // Pentatonisk Moll
            9 => &[0, 3, 5, 6, 7, 10],    // Blues
            10 => &[0, 1, 3, 5, 7, 8, 10], // Synthwave (Phrygian)
            _ => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], // Kromatisk
        };
        intervals.iter().map(|&i| (root + i) % 12).collect()
    }

    pub fn humanize_active_pattern(&mut self) {
        let mut rng_state: u32 = 987654321;
        for v in self.step_velocities.iter_mut() {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 17;
            rng_state ^= rng_state << 5;
            let delta = (rng_state as f32 / u32::MAX as f32) * 0.3 - 0.15;
            *v = (*v + delta).clamp(0.2, 1.0);
        }
        self.status_message = crate::i18n::t("🎲 Humanize: Anslagsdynamik och sväng varierat!").to_string();
    }

    pub fn transpose_active_pattern(&mut self, semitones: i8) {
        let mut new_grid = [[false; 16]; 24];
        for offset in 0..24 {
            for step in 0..16 {
                if self.piano_roll_grid[offset][step] {
                    let target = offset as i8 + semitones;
                    if target >= 0 && target < 24 {
                        new_grid[target as usize][step] = true;
                    }
                }
            }
        }
        self.piano_roll_grid = new_grid;
        for step in 0..16 {
            for offset in 0..24 {
                if self.piano_roll_grid[offset][step] {
                    self.channels[6].notes[step] = 48 + offset as u8;
                    break;
                }
            }
        }
        self.sync_active_pattern_from_ui();
        self.status_message = crate::tstatus!("⬆️ Transponerade mönster {} halvtoner", semitones);
    }

    pub fn arpeggiate_pattern(&mut self) {
        let base_scale = self.get_scale_notes();
        let scale_len = base_scale.len().max(1);
        self.piano_roll_grid = [[false; 16]; 24];
        for step in 0..16 {
            let note_in_scale = base_scale[step % scale_len];
            let octave = (step / 4) % 2;
            let note_offset = (note_in_scale as usize + octave * 12).min(23);
            self.piano_roll_grid[note_offset][step] = true;
            self.channels[6].steps[step] = true;
            self.channels[6].notes[step] = 48 + note_offset as u8;
        }
        self.sync_active_pattern_from_ui();
        self.status_message = crate::i18n::t("⚡ Arpeggiator: Skapade melodiskt arpeggio!").to_string();
    }

    pub fn reverse_pattern(&mut self) {
        for row in self.piano_roll_grid.iter_mut() {
            row.reverse();
        }
        self.step_velocities.reverse();
        self.channels[6].steps.reverse();
        self.channels[6].notes.reverse();
        self.sync_active_pattern_from_ui();
        self.status_message = crate::i18n::t("🔁 Vände mönster baklänges (Reverse)").to_string();
    }

    fn render_piano_roll_editor(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // ROW 1: Pattern Selector, Root Key, Scale, Snap Toggle
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO ROLL")).strong().size(13.0).color(Theme::FL_GREEN));
                ui.separator();

                ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(11.0).color(Theme::TEXT_MUTED));
                let p_len = self.patterns.len();
                for pat_idx in 0..p_len {
                    let is_active = self.selected_pattern == pat_idx;
                    let p_color = self.patterns[pat_idx].color;
                    let fill = if is_active { p_color } else { Theme::PANEL_BG };
                    let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                    let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                    if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(11.0).color(text_color)).fill(fill)).clicked() {
                        self.select_pattern(pat_idx);
                    }
                }

                ui.separator();

                // Root Key Picker
                ui.label(egui::RichText::new(crate::i18n::t("Grundton:")).size(11.0).color(Theme::TEXT_MUTED));
                let root_names = ["C", "C#", "D", "D#", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
                let cur_root = root_names.get(self.piano_roll_root_note as usize % 12).unwrap_or(&"C");
                egui::ComboBox::from_id_salt("pr_root_combo")
                    .selected_text(*cur_root)
                    .width(45.0)
                    .show_ui(ui, |ui| {
                        for (idx, &r_name) in root_names.iter().enumerate() {
                            if ui.selectable_label(self.piano_roll_root_note as usize == idx, r_name).clicked() {
                                self.piano_roll_root_note = idx as u8;
                                self.song_key_root = idx as u8;
                            }
                        }
                    });

                // Scale Snapping Selector
                ui.label(egui::RichText::new(crate::i18n::t("Skala:")).size(11.0).color(Theme::TEXT_MUTED));
                let scales = [
                    "Kromatisk", "Dur (Maj)", "Moll (Min)", "Harm. Moll", "Mel. Moll",
                    "Dorian", "Mixolydian", "Pentatonisk", "Blues", "Synthwave",
                ];
                let cur_scale = scales.get(self.selected_scale).unwrap_or(&"Kromatisk");
                egui::ComboBox::from_id_salt("pr_scale_combo")
                    .selected_text(*cur_scale)
                    .width(95.0)
                    .show_ui(ui, |ui| {
                        for (s_idx, &s_name) in scales.iter().enumerate() {
                            if ui.selectable_label(self.selected_scale == s_idx, s_name).clicked() {
                                self.selected_scale = s_idx;
                                self.song_key_scale = s_idx;
                            }
                        }
                    });

                let snap_bg = if self.piano_roll_snap_to_scale { Theme::FL_CYAN } else { Color32::from_rgb(32, 38, 48) };
                let snap_fg = if self.piano_roll_snap_to_scale { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔒 Skal-lås")).strong().size(10.5).color(snap_fg)).fill(snap_bg)).on_hover_text(crate::i18n::t("Lås tangenter till vald skala")).clicked() {
                    self.piano_roll_snap_to_scale = !self.piano_roll_snap_to_scale;
                }

                // Tagningen (Fas 6.4): kvantisera eller humanisera det som spelades in.
                let take_notes = self
                    .patterns
                    .get(self.selected_pattern)
                    .map(|p| p.take.len())
                    .unwrap_or(0);
                let take_tightness = self
                    .patterns
                    .get(self.selected_pattern)
                    .map(|p| p.take.tightness())
                    .unwrap_or(0.0);
                // Spannet och hur många noter som faktiskt ligger utanför rutnätet:
                // "4 med tajming" säger att humaniseringen hörs, inte bara mäts.
                let (take_first, take_last, take_moved) = self
                    .patterns
                    .get(self.selected_pattern)
                    .map(|p| {
                        let first = p.take.notes.iter().map(crate::midi_take::Take::slot).min();
                        let last = p.take.notes.iter().map(crate::midi_take::Take::slot).max();
                        let moved = p
                            .take
                            .notes
                            .iter()
                            .filter(|n| crate::midi_take::Take::playback_slot(n).1 > 0.001)
                            .count();
                        (first, last, moved)
                    })
                    .unwrap_or((None, None, 0));
                let take_span = match (take_first, take_last) {
                    (Some(a), Some(b)) => crate::tstatus!("steg {}–{}", a, b),
                    _ => crate::i18n::t("tom").to_string(),
                };
                ui.separator();
                ui.label(
                    egui::RichText::new(crate::tstatus!(
                        "Tagning: {} noter ({}, {} med tajming) · {:.2} steg otajt",
                        take_notes,
                        take_span,
                        take_moved,
                        take_tightness
                    ))
                    .size(10.5)
                    .color(if take_notes == 0 { Theme::TEXT_MUTED } else { Theme::TEXT_BRIGHT }),
                );
                ui.label(egui::RichText::new(crate::i18n::t("Rutnät:")).size(11.0).color(Theme::TEXT_MUTED));
                let cur_grid = crate::midi_take::TakeGrid::from_index(self.take_grid_idx);
                egui::ComboBox::from_id_salt("take_grid_combo")
                    .selected_text(cur_grid.label())
                    .width(74.0)
                    .show_ui(ui, |ui| {
                        for grid in crate::midi_take::TakeGrid::ALL {
                            if ui.selectable_label(grid == cur_grid, grid.label()).clicked() {
                                self.take_grid_idx = grid.index();
                            }
                        }
                    });
                ui.label(egui::RichText::new(crate::i18n::t("Styrka")).size(11.0).color(Theme::TEXT_MUTED));
                ui.add(
                    egui::Slider::new(&mut self.take_strength, 0.0..=1.0)
                        .show_value(false)
                        .fixed_decimals(2),
                )
                .on_hover_text(crate::i18n::t("Hur hårt noterna dras mot rutnätet (0 % = oförändrat, 100 % = exakt på rutnätet)"));

                let take_btn = Color32::from_rgb(32, 38, 48);
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 Kvantisera")).strong().size(10.5).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Dra tagningens noter till närmaste rutnätslinje, med projektets sväng")).clicked() {
                    let grid = crate::midi_take::TakeGrid::from_index(self.take_grid_idx);
                    let strength = self.take_strength;
                    self.quantize_take(strength, grid);
                }
                ui.label(egui::RichText::new(crate::i18n::t("Humanisering")).size(11.0).color(Theme::TEXT_MUTED));
                ui.add(
                    egui::Slider::new(&mut self.take_humanize, 0.0..=1.0)
                        .show_value(false)
                        .fixed_decimals(2),
                )
                .on_hover_text(crate::i18n::t("Hur mycket mänsklig otajthet och dynamik som läggs på"));
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🌀 Humanisera")).strong().size(10.5).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Lägg på mänsklig otajthet och dynamik (varierar mellan trycken)")).clicked() {
                    let amount = self.take_humanize;
                    self.humanize_take(amount);
                }

                ui.separator();

                // Chord Stamp Selector
                ui.label(egui::RichText::new(crate::i18n::t("Ackord:")).size(11.0).color(Theme::TEXT_MUTED));
                let chords = ["Enkel", "Dur", "Moll", "7th", "Sus4", "Oktav"];
                let cur_chord = chords.get(self.chord_stamp).unwrap_or(&"Enkel");
                egui::ComboBox::from_id_salt("pr_chord_combo")
                    .selected_text(*cur_chord)
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for (c_idx, &c_name) in chords.iter().enumerate() {
                            if ui.selectable_label(self.chord_stamp == c_idx, c_name).clicked() {
                                self.chord_stamp = c_idx;
                            }
                        }
                    });
            });

            ui.add_space(4.0);

            // ROW 2: MIDI Transformation Tools (Humanize, Arp, Transpose, Invert, Clear)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("VERKTYG:")).strong().size(10.5).color(Theme::TEXT_MUTED));

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎲 Humanize")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(180, 80, 20))).on_hover_text(crate::i18n::t("Variera anslagsdynamik (velocity) för levande sväng")).clicked() {
                    self.humanize_active_pattern();
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Arpeggio")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 110, 160))).on_hover_text(crate::i18n::t("Skapa automatiskt arpeggiomönster från vald skala")).clicked() {
                    self.arpeggiate_pattern();
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬆ +Okt")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Transponera upp 1 oktav (+12 halvtoner)")).clicked() {
                    self.transpose_active_pattern(12);
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬇ -Okt")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Transponera ner 1 oktav (-12 halvtoner)")).clicked() {
                    self.transpose_active_pattern(-12);
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Backa")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Vänd tonföljden baklänges")).clicked() {
                    self.reverse_pattern();
                }

                let poly_bg = if self.piano_roll_poly_mode { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                let poly_fg = if self.piano_roll_poly_mode { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎹 Polyfoni")).strong().size(10.5).color(poly_fg)).fill(poly_bg)).on_hover_text(crate::i18n::t("Tillåt flera toner samtidigt i samma steg (ackordmålning)")).clicked() {
                    self.piano_roll_poly_mode = !self.piano_roll_poly_mode;
                }

                // Real MIDI keyboard input indicator + record arm (Fas 5.3)
                let midi_col = if self.midi_keyboard_connected { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                ui.label(egui::RichText::new(if self.midi_keyboard_connected { "🎹 MIDI ●" } else { "🎹 MIDI ○" }).strong().size(10.0).color(midi_col))
                    .on_hover_text(if self.midi_keyboard_connected { crate::i18n::t("MIDI-klaviatur ansluten (ALSA Seq).") } else { crate::i18n::t("Ingen MIDI-klaviatur ansluten. Öppna Hårdvarukontroller för att koppla upp.") });
                let mrec_bg = if self.midi_record_armed { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(38, 45, 56) };
                let mrec_fg = if self.midi_record_armed { Color32::WHITE } else { Theme::TEXT_BRIGHT };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏺ MIDI-REC")).strong().size(10.0).color(mrec_fg)).fill(mrec_bg)).on_hover_text(crate::i18n::t("Spela in hållna MIDI-toner i rutnätet under uppspelning.")).clicked() {
                    self.midi_record_armed = !self.midi_record_armed;
                    if self.midi_record_armed {
                        self.begin_new_take();
                    }
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🧹 Töm")).size(10.5).color(Color32::from_rgb(255, 120, 120))).fill(Color32::from_rgb(50, 20, 25))).on_hover_text(crate::i18n::t("Rensa mönster")).clicked() {
                    self.piano_roll_grid = [[false; 16]; 24];
                    self.channels[6].steps = [false; 16];
                    self.sync_active_pattern_from_ui();
                    self.status_message = crate::i18n::t("🧹 Pianorullens mönster rensat!").to_string();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(crate::i18n::t("🎼 Arranger")).clicked() {
                        self.view_mode = ViewMode::PlaylistArranger;
                    }
                    if ui.button(crate::i18n::t("◀ Channel Rack")).clicked() {
                        self.view_mode = ViewMode::ChannelRack;
                    }
                });
            });

            ui.add_space(6.0);

            let base_midi = 48; // C3
            let semitones_count = 24;
            let scale_notes = self.get_scale_notes();
            let root_note_val = self.piano_roll_root_note % 12;

            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                for note_offset in (0..semitones_count).rev() {
                    let midi_note = base_midi + note_offset as u8;
                    let note_val = midi_note % 12;
                    let note_in_scale = scale_notes.contains(&note_val);
                    let is_root = note_val == root_note_val;
                    let is_sharp = [1, 3, 6, 8, 10].contains(&note_val);
                    let n_name = note_name(midi_note);
                    let octave_num = (midi_note / 12) as i8 - 1;

                    ui.horizontal(|ui| {
                        // Piano Key label on left with Root and Scale Highlighting
                        let key_bg = if is_root {
                            Color32::from_rgb(180, 110, 20) // Amber for root note
                        } else if !note_in_scale {
                            Color32::from_rgb(18, 20, 26)
                        } else if is_sharp {
                            Color32::from_rgb(32, 42, 58)
                        } else {
                            Color32::from_rgb(220, 228, 240)
                        };
                        let key_text = if is_root || is_sharp || !note_in_scale { Color32::WHITE } else { Color32::BLACK };
                        let (k_rect, _) = ui.allocate_exact_size(Vec2::new(52.0, 16.0), Sense::hover());
                        ui.painter().rect_filled(k_rect, Rounding::same(2.0), key_bg);

                        let label_str = if is_root {
                            format!("✦ {}{}", n_name, octave_num)
                        } else {
                            format!("{}{}", n_name, octave_num)
                        };
                        ui.painter().text(
                            k_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label_str,
                            egui::FontId::proportional(10.0),
                            key_text,
                        );

                        // 16 Grid Columns
                        for step in 0..16 {
                            let (cell_rect, response) = ui.allocate_exact_size(Vec2::new(26.0, 16.0), Sense::click());
                            let is_on = self.piano_roll_grid[note_offset][step];
                            let is_playhead = self.is_playing && self.current_step == step;
                            let is_beat_start = step % 4 == 0;

                            if response.clicked() {
                                if is_on {
                                    self.piano_roll_grid[note_offset][step] = false;
                                    let any_left = (0..24).any(|o| self.piano_roll_grid[o][step]);
                                    self.channels[6].steps[step] = any_left;
                                } else {
                                    if !self.piano_roll_poly_mode {
                                        for o in 0..24 {
                                            self.piano_roll_grid[o][step] = false;
                                        }
                                    }

                                    // Check chord stamp
                                    let stamp_offsets: &[usize] = match self.chord_stamp {
                                        1 => &[0, 4, 7],      // Major
                                        2 => &[0, 3, 7],      // Minor
                                        3 => &[0, 4, 7, 10],  // 7th
                                        4 => &[0, 5, 7],      // Sus4
                                        5 => &[0, 12],        // Octave
                                        _ => &[0],            // Single
                                    };

                                    for &c_off in stamp_offsets {
                                        let target_off = note_offset + c_off;
                                        if target_off < 24 {
                                            self.piano_roll_grid[target_off][step] = true;
                                        }
                                    }
                                    self.channels[6].steps[step] = true;
                                    self.channels[6].notes[step] = midi_note;
                                }
                                self.sync_active_pattern_from_ui();
                            }

                            let mut bg = if is_on {
                                Theme::FL_GREEN
                            } else if is_root {
                                Color32::from_rgb(36, 32, 22)
                            } else if !note_in_scale {
                                Color32::from_rgb(14, 16, 22)
                            } else if is_sharp {
                                Color32::from_rgb(24, 28, 36)
                            } else if is_beat_start {
                                Color32::from_rgb(38, 44, 56)
                            } else {
                                Color32::from_rgb(28, 32, 42)
                            };

                            if is_playhead {
                                bg = Color32::from_rgb(255, 230, 100);
                            }

                            ui.painter().rect_filled(cell_rect, Rounding::same(1.5), bg);
                            ui.painter().rect_stroke(cell_rect, Rounding::same(1.5), Stroke::new(0.5_f32, Color32::from_rgb(45, 52, 65)));
                        }
                    });
                }
            });

            ui.add_space(4.0);

            // Velocity Editor Lane (FL Studio Note Velocity Stalks)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("VELOCITY")).size(10.0).color(Theme::TEXT_MUTED));
                ui.allocate_exact_size(Vec2::new(10.0, 24.0), Sense::hover());
                for s in 0..16 {
                    let (v_rect, v_resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click_and_drag());
                    if v_resp.dragged() {
                        let dy = v_resp.drag_delta().y;
                        self.step_velocities[s] = (self.step_velocities[s] - dy * 0.05).clamp(0.1, 1.0);
                    }
                    let v = self.step_velocities[s];
                    let bar_h = v * v_rect.height();
                    let bar_rect = Rect::from_min_max(Pos2::new(v_rect.center().x - 2.5, v_rect.max.y - bar_h), Pos2::new(v_rect.center().x + 2.5, v_rect.max.y));
                    ui.painter().rect_filled(v_rect, Rounding::same(1.0), Color32::from_rgb(18, 22, 28));
                    ui.painter().rect_filled(bar_rect, Rounding::same(1.0), Theme::FL_CYAN);
                }
            });
        });
    }

    pub fn apply_drummer_generation(&mut self) {
        let comp = self.drummer_complexity;
        let loud = self.drummer_loudness;
        let ksv = self.drummer_kick_snare_var;
        let hv = self.drummer_hihat_var;
        let fills = self.drummer_fills.clamp(0.0, 1.0);

        let mut kick = [false; 16];
        let mut snare = [false; 16];
        let mut clap = [false; 16];
        let mut hat_c = [false; 16];
        let mut hat_o = [false; 16];
        let mut crash = [false; 16];

        // 1. Kick Pattern (variation controlled by drummer_kick_snare_var)
        kick[0] = true;
        match ksv {
            0 => {
                if comp >= 0.3 {
                    kick[8] = true;
                }
            }
            1 => {
                kick[8] = true;
                if comp > 0.4 {
                    kick[6] = true;
                }
                if comp > 0.6 {
                    kick[14] = true;
                }
            }
            2 => {
                kick[4] = true;
                kick[8] = true;
                kick[12] = true;
                if comp > 0.5 {
                    kick[10] = true;
                }
            }
            _ => {
                kick[3] = true;
                kick[6] = true;
                kick[8] = true;
                if comp > 0.4 {
                    kick[10] = true;
                }
                if comp > 0.6 {
                    kick[14] = true;
                }
            }
        }
        // Fills add extra kick hits at the end of the bar.
        if fills > 0.5 {
            kick[15] = true;
        }
        if fills > 0.75 {
            kick[13] = true;
        }

        // 2. Snare / Clap (variation controlled by drummer_kick_snare_var)
        if self.drummer_profile == 3 {
            // Trap / Hip Hop uses claps on 4 and 12
            clap[4] = true;
            clap[12] = true;
            if comp > 0.6 {
                clap[7] = true;
                clap[15] = true;
            }
        } else {
            snare[4] = true;
            snare[12] = true;
            match ksv {
                0 => {}
                1 => {
                    if comp > 0.5 {
                        snare[15] = true;
                    }
                }
                2 => {
                    if comp > 0.5 {
                        snare[7] = true;
                    }
                    if comp > 0.7 {
                        snare[11] = true;
                    }
                }
                _ => {
                    snare[7] = true;
                    snare[11] = true;
                    snare[15] = true;
                }
            }
            if self.drummer_percussion_on && comp > 0.4 {
                clap[12] = true;
            }
        }

        // 3. Hi-Hats (style controlled by drummer_hihat_var)
        match hv {
            0 => {
                for step in (0..16).step_by(2) {
                    hat_c[step] = true;
                }
            }
            1 => {
                for step in 0..16 {
                    if step % 4 == 2 {
                        hat_o[step] = true;
                    } else {
                        hat_c[step] = true;
                    }
                }
            }
            2 => {
                for step in 0..16 {
                    if step % 2 == 0 {
                        hat_c[step] = true;
                    } else if comp > 0.4 {
                        hat_c[step] = true;
                    }
                }
            }
            _ => {
                for step in 0..16 {
                    hat_c[step] = true;
                }
                if comp > 0.5 {
                    hat_o[14] = true;
                }
            }
        }

        // 4. Crash Cymbal
        if self.drummer_cymbals_on {
            crash[0] = true;
            if comp > 0.75 {
                crash[14] = true;
            }
        }

        // 5. Toms – a fill in the final beat, only when enabled.
        let mut toms = [false; 16];
        if self.drummer_toms_on {
            toms[12] = true;
            toms[13] = true;
            toms[14] = true;
            if fills > 0.5 {
                toms[11] = true;
            }
            if fills > 0.75 {
                toms[15] = true;
            }
            self.drummer_tom_high = comp > 0.5;
        }
        self.drummer_tom_steps = toms;

        self.channels[0].steps = kick;
        self.channels[1].steps = snare;
        self.channels[2].steps = clap;
        self.channels[3].steps = hat_c;
        self.channels[4].steps = hat_o;
        self.channels[5].steps = crash;

        let vel_mult = 0.5 + loud * 0.5;
        self.step_velocities.fill(vel_mult);

        self.sync_active_pattern_from_ui();
        self.status_message = crate::tstatus!("🥁 Drummer genererade mönster (Komplexitet: {:.0}%, Volym: {:.0}%)", comp * 100.0, loud * 100.0);
    }

    fn render_session_drummer(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🥁 SONIX SESSION DRUMMER")).strong().size(14.0).color(Theme::FL_ORANGE));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Intelligent trummis med 2D XY-radar för realtids-generering av trumspår")).size(11.0).color(Theme::TEXT_MUTED));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Skriv till Mönster")).strong().color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                        self.apply_drummer_generation();
                    }
                });
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                // Left Column: Drummer Profiles & Beat Presets
                ui.group(|ui| {
                    ui.set_width(220.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("DRUMMERS (STILAR)")).strong().size(11.0).color(Theme::FL_CYAN));
                        let drummers = [
                            ("Kyle", "Pop Rock (Punchy)", "🎸"),
                            ("Logan", "Retro Synthwave", "🕹"),
                            ("Anders", "Electronic / House", "⚡"),
                            ("Jesse", "Hip Hop / 808 Trap", "🎤"),
                            ("Max", "Punk & Hard Rock", "🔥"),
                            ("Ian", "Funk & Neo-Soul", "🎷"),
                        ];

                        for (idx, (name, desc, ico)) in drummers.iter().enumerate() {
                            let is_sel = self.drummer_profile == idx;
                            let bg = if is_sel { Color32::from_rgb(40, 55, 75) } else { Theme::PANEL_BG };
                            let stroke = if is_sel { Stroke::new(1.0_f32, Theme::FL_CYAN) } else { Stroke::NONE };

                            egui::Frame::none().fill(bg).stroke(stroke).rounding(Rounding::same(4.0)).inner_margin(4.0).show(ui, |ui| {
                                if ui.selectable_label(is_sel, format!("{} {} - {}", ico, name, desc)).clicked() {
                                    self.drummer_profile = idx;
                                    self.apply_drummer_generation();
                                }
                            });
                        }

                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::t("BEAT PRESETS")).strong().size(11.0).color(Theme::FL_ORANGE));
                        let presets = [
                            "Golden State (Standard 4/4)",
                            "Half-Pipe (Skate Punk)",
                            "Mixtape 808 (Trap Rolls)",
                            "Synth City (Driving 16ths)",
                            "Neon Drive (Outrun)",
                            "Slow Jam R&B",
                        ];
                        for (p_idx, p_name) in presets.iter().enumerate() {
                            let is_p = self.drummer_preset == p_idx;
                            if ui.selectable_label(is_p, *p_name).clicked() {
                                self.drummer_preset = p_idx;
                                match p_idx {
                                    0 => { self.drummer_complexity = 0.35; self.drummer_loudness = 0.8; }
                                    1 => { self.drummer_complexity = 0.85; self.drummer_loudness = 0.95; }
                                    2 => { self.drummer_complexity = 0.70; self.drummer_loudness = 0.9; }
                                    3 => { self.drummer_complexity = 0.50; self.drummer_loudness = 0.85; }
                                    4 => { self.drummer_complexity = 0.60; self.drummer_loudness = 0.88; }
                                    _ => { self.drummer_complexity = 0.25; self.drummer_loudness = 0.65; }
                                }
                                self.apply_drummer_generation();
                            }
                        }
                    });
                });

                ui.separator();

                // Center Column: Interactive 2D XY Matrix Radar Pad
                ui.group(|ui| {
                    ui.set_width(320.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("XY PERFORMANCE RADAR")).strong().size(11.0).color(Theme::FL_YELLOW));
                        ui.label(egui::RichText::new(crate::i18n::t("Dra den gula pucken för att ändra dynamik & komplexitet i realtid")).size(10.0).color(Theme::TEXT_MUTED));
                        ui.add_space(4.0);

                        if drummer_xy_matrix(ui, &mut self.drummer_complexity, &mut self.drummer_loudness, Vec2::new(300.0, 260.0)) {
                            self.apply_drummer_generation();
                        }

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(format!("{} {:.0}%  •  {} {:.0}%", crate::i18n::t("Komplexitet:"), self.drummer_complexity * 100.0, crate::i18n::t("Ljudstyrka:"), self.drummer_loudness * 100.0)).size(11.0).color(Theme::FL_YELLOW));
                    });
                });

                ui.separator();

                // Right Column: Drum Kit Parts & Fine Controls
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("TRUMSET & INSTRUMENT")).strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(4.0);

                        if ui.checkbox(&mut self.drummer_percussion_on, "👏 Percussion & Handclaps").changed() {
                            self.apply_drummer_generation();
                        }
                        if ui.checkbox(&mut self.drummer_cymbals_on, "✨ Crash Cymbals & Accents").changed() {
                            self.apply_drummer_generation();
                        }
                        if ui.checkbox(&mut self.drummer_toms_on, "🔊 Acoustic & Electronic Toms").changed() {
                            self.apply_drummer_generation();
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Kick & Snare:")).size(10.0).color(Theme::TEXT_MUTED));
                            for v in 1..=4 {
                                if ui.selectable_label(self.drummer_kick_snare_var == v, format!("{}", v)).clicked() {
                                    self.drummer_kick_snare_var = v;
                                    self.apply_drummer_generation();
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Hi-Hat Rytm:")).size(10.0).color(Theme::TEXT_MUTED));
                            for v in 1..=4 {
                                if ui.selectable_label(self.drummer_hihat_var == v, format!("{}", v)).clicked() {
                                    self.drummer_hihat_var = v;
                                    self.apply_drummer_generation();
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::t("FILLS & GROOVE")).strong().size(11.0).color(Theme::FL_CYAN));

                        ui.horizontal(|ui| {
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.drummer_fills, 0.0, 1.0, "FILLS", Theme::FL_CYAN, 30.0);
                            ch |= rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 30.0);
                            if ch {
                                self.apply_drummer_generation();
                            }
                        });

                        ui.add_space(12.0);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("💡 TIPS")).strong().size(10.0).color(Theme::FL_GREEN));
                            ui.label(egui::RichText::new(crate::i18n::t("Alla förändringar i Session Drummer speglas direkt i FL Channel Rack och tidslinjen!")).size(9.5).color(Theme::TEXT_MUTED));
                        });
                    });
                });
            });
        });
    }

    pub fn apply_alchemy_morph(&mut self) {
        let px = self.alchemy_puck[0];
        let py = self.alchemy_puck[1];
        use std::f32::consts::PI;

        let snapshots = [
            (-0.5 * PI, 0.005, 0.35, 4200.0, 1.8, Waveform::Saw),
            (-0.25 * PI, 0.35, 0.6, 2800.0, 1.2, Waveform::Saw),
            (0.0 * PI, 0.005, 0.15, 1200.0, 4.5, Waveform::Saw),
            (0.25 * PI, 0.01, 0.25, 6000.0, 2.0, Waveform::Square),
            (0.5 * PI, 0.5, 0.8, 1500.0, 0.8, Waveform::Triangle),
            (0.75 * PI, 0.08, 0.4, 3500.0, 2.5, Waveform::Triangle),
            (1.0 * PI, 0.002, 0.6, 8000.0, 3.0, Waveform::Sine),
            (1.25 * PI, 0.001, 0.08, 12000.0, 0.7, Waveform::Square),
        ];

        let mut total_weight = 0.0;
        let mut morph_attack = 0.0;
        let mut morph_decay = 0.0;
        let mut morph_cutoff = 0.0;
        let mut morph_res = 0.0;
        let mut closest_wf = Waveform::Saw;
        let mut min_dist = f32::MAX;

        for (angle, att, dec, cut, res, wf) in snapshots {
            let sx = angle.cos();
            let sy = angle.sin();
            let dist = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt().max(0.05);
            let w = 1.0 / (dist.powf(2.5));
            total_weight += w;
            morph_attack += att * w;
            morph_decay += dec * w;
            morph_cutoff += cut * w;
            morph_res += res * w;

            if dist < min_dist {
                min_dist = dist;
                closest_wf = wf;
            }
        }

        if total_weight > 0.0001 {
            self.adsr.attack = morph_attack / total_weight;
            self.adsr.decay = morph_decay / total_weight;
            self.filter.cutoff = (morph_cutoff / total_weight).clamp(200.0, 18000.0);
            self.filter.resonance = (morph_res / total_weight).clamp(0.5, 9.0);
            self.waveform = closest_wf;

            let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
            let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
            let _ = self.engine.send_command(AudioCommand::SetWaveform(self.waveform));
        }
    }

    fn render_alchemy_morph_pad(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("✨ SONIX ALCHEMY SYNTHESIZER (Transform Pad)")).strong().size(14.0).color(Theme::FL_CYAN));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("8-punkters realtids-morphing mellan subtraktiva, FM- och wavetable-synteser")).size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                // Center Morphing Pad
                ui.group(|ui| {
                    ui.set_width(380.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("TRANSFORM PAD")).strong().size(12.0).color(Theme::FL_CYAN));
                        ui.label(egui::RichText::new(crate::i18n::t("Dra den lysande markören för att sömlöst smälta samman filter, vågformer och ADSR")).size(10.0).color(Theme::TEXT_MUTED));
                        ui.add_space(6.0);

                        if alchemy_transform_matrix(ui, &mut self.alchemy_puck, Vec2::new(360.0, 300.0)) {
                            self.apply_alchemy_morph();
                        }
                    });
                });

                ui.separator();

                // Live Morphed Parameters
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("AKTIVA SYNTPARAMETRAR (MORPHED)")).strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(6.0);

                        ui.label(crate::tstatus!("Vågform: {}", self.waveform.name()));
                        ui.label(format!("Filter Cutoff: {:.0} Hz", self.filter.cutoff));
                        ui.label(crate::tstatus!("Resonans (Q): {:.2}", self.filter.resonance));
                        ui.label(format!("Attack: {:.3}s  •  Decay: {:.2}s", self.adsr.attack, self.adsr.decay));

                        ui.add_space(8.0);
                        ui.separator();

                        ui.label(egui::RichText::new(crate::i18n::t("SNABBVAL SNAPSHOTS")).strong().size(11.0).color(Theme::FL_CYAN));
                        let snaps = [
                            ("1. Pluck", 0.0_f32, -1.0_f32),
                            ("2. Lush String", 0.707, -0.707),
                            ("3. 303 Acid", 1.0, 0.0),
                            ("4. Synthwave", 0.707, 0.707),
                            ("5. Warm Pad", 0.0, 1.0),
                            ("6. Cosmic Brass", -0.707, 0.707),
                            ("7. Digital Bell", -1.0, 0.0),
                            ("8. Chiptune", -0.707, -0.707),
                        ];

                        ui.columns(2, |cols| {
                            for (i, (s_name, px, py)) in snaps.iter().enumerate() {
                                let col_i = i % 2;
                                if cols[col_i].button(*s_name).clicked() {
                                    self.alchemy_puck = [*px, *py];
                                    self.apply_alchemy_morph();
                                }
                            }
                        });

                        ui.add_space(8.0);
                        if ui.button(crate::i18n::t("🎹 Provspela Ton (C4)")).clicked() {
                            self.play_note(60);
                        }
                    });
                });
            });
        });
    }

    fn render_remix_fx_view(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎛 REMIX FX & GROSS BEAT (DJ Performance Touch)")).strong().size(14.0).color(Theme::FL_GREEN));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("DJ-effekter, Tape Stop, Filter Sweeps och Stutter Glitch")).size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                // DJ XY Filter Touch Surface
                ui.group(|ui| {
                    ui.set_width(340.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("DJ FILTER & CRUSH XY PAD")).strong().size(11.0).color(Theme::FL_GREEN));
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(320.0, 240.0), Sense::click_and_drag());
                        let painter = ui.painter();

                        if (resp.dragged() || resp.clicked())
                            && let Some(pos) = resp.interact_pointer_pos() {
                                self.remix_xy[0] = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                                self.remix_xy[1] = (1.0 - (pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);

                                self.filter.cutoff = 200.0 + self.remix_xy[0] * 12000.0;
                                self.filter.resonance = 0.5 + self.remix_xy[1] * 7.5;
                                let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                            }

                        painter.rect_filled(rect, Rounding::same(6.0), Color32::from_rgb(14, 22, 18));
                        painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, Theme::FL_GREEN));

                        let c = rect.center();
                        painter.line_segment([Pos2::new(rect.min.x, c.y), Pos2::new(rect.max.x, c.y)], Stroke::new(1.0_f32, Color32::from_rgb(25, 45, 30)));
                        painter.line_segment([Pos2::new(c.x, rect.min.y), Pos2::new(c.x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(25, 45, 30)));

                        let puck_x = rect.min.x + self.remix_xy[0] * rect.width();
                        let puck_y = rect.max.y - self.remix_xy[1] * rect.height();
                        painter.circle_filled(Pos2::new(puck_x, puck_y), 12.0, Color32::from_rgba_unmultiplied(46, 204, 113, 100));
                        painter.circle_filled(Pos2::new(puck_x, puck_y), 6.0, Theme::FL_GREEN);

                        ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz  •  Res: {:.1}", self.filter.cutoff, self.filter.resonance)).size(10.0).color(Theme::FL_GREEN));
                    });
                });

                ui.separator();

                // DJ Performance Buttons
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("DJ PERFORMANCE PADS")).strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            // Tape Stop
                            let stop_col = if self.remix_tape_stop { Theme::FL_ORANGE } else { Color32::from_rgb(50, 40, 30) };
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏸ TAPE STOP")).strong().size(12.0).color(Color32::WHITE)).fill(stop_col).min_size(Vec2::new(100.0, 40.0))).clicked() {
                                self.remix_tape_stop = !self.remix_tape_stop;
                                let _ = self.engine.send_command(AudioCommand::SetTapeStop { active: self.remix_tape_stop });
                            }

                            // Vinyl Scratch
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📻 VINYL SCRATCH")).strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(30, 45, 60)).min_size(Vec2::new(120.0, 40.0))).clicked() {
                                self.drive = 4.5;
                                let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(crate::i18n::t("GROSS BEAT / STUTTER GLITCH")).strong().size(11.0).color(Theme::FL_CYAN));
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.remix_stutter == 1, "⚡ 1/4 Repeat").clicked() {
                                self.remix_stutter = if self.remix_stutter == 1 { 0 } else { 1 };
                                let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                                self.status_message = crate::i18n::t("⚡ 1/4 Beat Repeat aktiv").to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 2, "⚡ 1/8 Stutter").clicked() {
                                self.remix_stutter = if self.remix_stutter == 2 { 0 } else { 2 };
                                let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                                self.status_message = crate::i18n::t("⚡ 1/8 Stutter aktiv").to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 3, "⚡ 1/16 Roll").clicked() {
                                self.remix_stutter = if self.remix_stutter == 3 { 0 } else { 3 };
                                let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                                self.status_message = crate::i18n::t("⚡ 1/16 High-Speed Roll aktiv").to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 4, "🌀 Reverse").clicked() {
                                self.remix_stutter = if self.remix_stutter == 4 { 0 } else { 4 };
                                let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                                self.status_message = crate::i18n::t("🌀 Gross Beat Reverse aktiv").to_string();
                            }
                        });

                        ui.add_space(10.0);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Styr DJ-effekter under uppspelning för live-remixing av dina mönster!")).size(10.0).color(Theme::TEXT_MUTED));
                        });
                    });
                });
            });
        });
    }

    fn render_effects_mixer_rack(&mut self, ui: &mut egui::Ui) {
        // Ensure selected timeline track is within bounds
        if !self.playlist_tracks.is_empty() && self.selected_timeline_track >= self.playlist_tracks.len() {
            self.selected_timeline_track = 0;
        }

        ui.group(|ui| {
            // Header Bar (Responsive Wrapped)
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX PRO MULTI-TRACK MIXER & DEDIKERAD EQ-STUDIO")).strong().size(13.5).color(Theme::FL_CYAN));
                ui.separator();
                ui.label(egui::RichText::new(crate::tstatus!("Låt: '{}' • {} låtspår med 3-bands parametrisk EQ, dynamik & effekter", self.project_name, self.playlist_tracks.len())).size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(6.0);

            // ================================================================
            // 1. TIER 1: MULTI-TRACK MIXER CHANNEL STRIPS (MASTER + ACTUAL SONG TRACKS)
            // ================================================================
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

                    // Master Channel Strip
                    ui.group(|ui| {
                        ui.set_width(78.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("MASTER")).strong().size(11.0).color(Theme::FL_ORANGE));
                            ui.add_space(2.0);
                            rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", Color32::WHITE, 16.0);
                            rotary_knob(ui, &mut self.stereo_width, 0.0, 2.0, "BREDD", Theme::FL_CYAN, 16.0);
                            ui.add_space(2.0);
                            let peak = self.engine.get_peak_level();
                            if vertical_fader(ui, &mut self.master_volume, 0.0, 1.25, peak, Theme::FL_ORANGE, 110.0) {
                                let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                            }
                            let gain_db = if self.master_volume <= 0.001 { -60.0 } else { 20.0 * self.master_volume.log10() };
                            ui.label(egui::RichText::new(format!("{:.0}%\n{:+.1}dB", self.master_volume * 100.0, gain_db)).strong().size(8.5).color(Theme::FL_ORANGE));
                        });
                    });

                    ui.separator();

                    // Channel Strips for each Playlist Track in the user's project
                    let num_tracks = self.playlist_tracks.len();
                    for t_idx in 0..num_tracks {
                        let is_selected = self.selected_timeline_track == t_idx;
                        let t = &self.playlist_tracks[t_idx];
                        let track_name = t.name.clone();
                        let track_color = t.color;
                        let track_icon = t.icon;
                        let is_mic = (t_idx == num_tracks - 1 && (track_name.to_lowercase().contains("mic") || track_name.to_lowercase().contains("mik") || t.kind == TrackKind::VocalAudio))
                            || track_name.to_lowercase().contains("mic (")
                            || track_name.to_lowercase().contains("mikrofon");

                        let border_color = if is_selected {
                            if is_mic { Color32::from_rgb(0, 200, 255) } else { track_color }
                        } else {
                            Color32::from_rgb(32, 40, 52)
                        };

                        let card_bg = if is_selected {
                            Color32::from_rgb(26, 32, 44)
                        } else if is_mic {
                            Color32::from_rgb(20, 25, 35)
                        } else {
                            Theme::CHANNEL_BG
                        };

                        ui.group(|ui| {
                            ui.set_width(82.0);
                            ui.painter().rect_stroke(ui.max_rect(), Rounding::same(3.0), Stroke::new(if is_selected { 1.5_f32 } else { 1.0_f32 }, border_color));

                            ui.vertical_centered(|ui| {
                                // Track Title / Select Button
                                let btn_label = if is_mic {
                                    format!("{} Mic", track_icon)
                                } else {
                                    let short_n = if track_name.chars().count() > 9 {
                                        format!("{}…", track_name.chars().take(8).collect::<String>())
                                    } else {
                                        track_name.clone()
                                    };
                                    format!("{} {}", track_icon, short_n)
                                };

                                let btn_col = if is_selected { Color32::WHITE } else { track_color };
                                let name_btn = egui::Button::new(egui::RichText::new(btn_label).strong().size(9.0).color(btn_col))
                                    .fill(card_bg)
                                    .min_size(Vec2::new(76.0, 18.0));
                                if ui.add(name_btn).on_hover_text(crate::tstatus!("Spår {}: {}
Klicka för att öppna dedikerad EQ & detaljer", t_idx + 1, track_name)).clicked() {
                                    self.selected_timeline_track = t_idx;
                                    self.status_message = crate::tstatus!("Valde '{}' för EQ & mixjustering", track_name);
                                }

                                // Quick EQ Status / Bypass Pill & Rec Arm
                                ui.horizontal(|ui| {
                                    let eq_on = self.playlist_tracks[t_idx].eq.enabled;
                                    let eq_pill_bg = if eq_on { Color32::from_rgb(20, 50, 40) } else { Color32::from_rgb(45, 30, 30) };
                                    let eq_pill_col = if eq_on { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                                    let eq_txt = if eq_on { "EQ ON" } else { "EQ OFF" };
                                    if ui.add(egui::Button::new(egui::RichText::new(eq_txt).size(7.0).color(eq_pill_col)).fill(eq_pill_bg).min_size(Vec2::new(36.0, 12.0))).clicked() {
                                        self.playlist_tracks[t_idx].eq.enabled = !self.playlist_tracks[t_idx].eq.enabled;
                                        self.sync_track_audio_state(t_idx);
                                    }

                                    let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                                    let rec_bg = if is_armed { Color32::from_rgb(200, 30, 30) } else { Color32::from_rgb(26, 32, 42) };
                                    let rec_col = if is_armed { Color32::WHITE } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("REC")).size(7.0).color(rec_col)).fill(rec_bg).min_size(Vec2::new(28.0, 12.0))).clicked() {
                                        self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                                    }
                                });

                                // Mini EQ Curve Thumbnail
                                let (thumb_rect, _) = ui.allocate_exact_size(Vec2::new(68.0, 22.0), Sense::hover());
                                let eq_snapshot = self.playlist_tracks[t_idx].eq.clone();
                                mini_track_eq_curve(
                                    ui.painter(),
                                    thumb_rect,
                                    eq_snapshot.low_gain_db,
                                    eq_snapshot.mid_gain_db,
                                    eq_snapshot.high_gain_db,
                                    if is_mic { Color32::from_rgb(0, 220, 255) } else { track_color },
                                    eq_snapshot.enabled,
                                );

                                // Pan Rotary Knob
                                let mut pan_val = self.playlist_tracks[t_idx].pan;
                                if rotary_knob(ui, &mut pan_val, -1.0, 1.0, "PAN", Color32::from_rgb(180, 195, 210), 15.0) {
                                    self.playlist_tracks[t_idx].pan = pan_val;
                                    self.sync_track_audio_state(t_idx);
                                }

                                // Mute / Solo Buttons
                                ui.horizontal(|ui| {
                                    let muted = self.playlist_tracks[t_idx].muted;
                                    let m_col = if muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 36, 48) };
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).size(7.5).color(if muted { Color32::WHITE } else { Theme::TEXT_MUTED })).fill(m_col).min_size(Vec2::new(16.0, 14.0))).clicked() {
                                        self.playlist_tracks[t_idx].muted = !muted;
                                        self.sync_track_audio_state(t_idx);
                                    }

                                    let solo = self.playlist_tracks[t_idx].solo;
                                    let s_col = if solo { Theme::FL_ORANGE } else { Color32::from_rgb(30, 36, 48) };
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).size(7.5).color(if solo { Color32::BLACK } else { Theme::TEXT_MUTED })).fill(s_col).min_size(Vec2::new(16.0, 14.0))).clicked() {
                                        self.playlist_tracks[t_idx].solo = !solo;
                                        self.sync_track_audio_state(t_idx);
                                    }
                                });

                                ui.add_space(2.0);

                                // Track Vertical Volume Fader with peak meter
                                let mut vol_val = self.playlist_tracks[t_idx].volume;
                                let meter_level = if self.is_playing && !self.playlist_tracks[t_idx].muted {
                                    if is_mic {
                                        self.vocal_studio.mic_vu_level
                                    } else {
                                        self.engine.get_peak_level()
                                    }
                                } else {
                                    0.0
                                };

                                let fader_col = if is_mic { Color32::from_rgb(0, 220, 255) } else { track_color };
                                if vertical_fader(ui, &mut vol_val, 0.0, 1.25, meter_level, fader_col, 100.0) {
                                    self.playlist_tracks[t_idx].volume = vol_val;
                                    self.sync_track_audio_state(t_idx);
                                }

                                let gain_db = if vol_val <= 0.001 { -60.0 } else { 20.0 * vol_val.log10() };
                                ui.label(egui::RichText::new(format!("{:.0}%\n{:+.1}dB", vol_val * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                            });
                        });
                    }
                });
            });

            ui.add_space(8.0);

            // ================================================================
            // 1b. TIER 1B: SUB-MIX BUSSES & VCA GROUPS (Fas 5.2)
            // ================================================================
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🧩 SUB-MIX-BUSSAR & VCA-GRUPPER")).strong().size(11.5).color(Theme::FL_CYAN));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("Bussar summerar spår; VCA styr grupper utan att routa ljudet.")).size(9.5).color(Theme::TEXT_MUTED));
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    let mut group_changed = false;

                    for b in 0..crate::audio::synth::NUM_BUSES {
                        let bus_name = crate::i18n::t(crate::audio::synth::BUS_NAMES[b]);
                        ui.group(|ui| {
                            ui.set_width(76.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(format!("BUS {}", bus_name)).strong().size(9.5).color(Theme::FL_CYAN));
                                let mut vol = self.bus_volume[b];
                                if vertical_fader(ui, &mut vol, 0.0, 1.25, 0.0, Theme::FL_CYAN, 78.0) {
                                    self.bus_volume[b] = vol;
                                    group_changed = true;
                                }
                                ui.horizontal(|ui| {
                                    let muted = self.bus_muted[b];
                                    let m_col = if muted { Theme::FL_RED } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new("M").size(10.0).color(m_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                        self.bus_muted[b] = !muted;
                                        group_changed = true;
                                    }
                                    let solo = self.bus_solo[b];
                                    let s_col = if solo { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new("S").size(10.0).color(s_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                        self.bus_solo[b] = !solo;
                                        group_changed = true;
                                    }
                                });
                                let gain_db = if self.bus_volume[b] <= 0.001 { -60.0 } else { 20.0 * self.bus_volume[b].log10() };
                                ui.label(egui::RichText::new(format!("{:.0}% {:+.1}dB", self.bus_volume[b] * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                            });
                        });
                    }

                    ui.separator();

                    for v in 0..crate::audio::synth::NUM_VCAS {
                        ui.group(|ui| {
                            ui.set_width(76.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(format!("VCA {}", v + 1)).strong().size(9.5).color(Theme::FL_PURPLE));
                                let mut vol = self.vca_faders[v];
                                if vertical_fader(ui, &mut vol, 0.0, 1.25, 0.0, Theme::FL_PURPLE, 78.0) {
                                    self.vca_faders[v] = vol;
                                    group_changed = true;
                                }
                                ui.horizontal(|ui| {
                                    let muted = self.vca_muted[v];
                                    let m_col = if muted { Theme::FL_RED } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new("M").size(10.0).color(m_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                        self.vca_muted[v] = !muted;
                                        group_changed = true;
                                    }
                                    let solo = self.vca_solos[v];
                                    let s_col = if solo { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new("S").size(10.0).color(s_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                        self.vca_solos[v] = !solo;
                                        group_changed = true;
                                    }
                                });
                                let gain_db = if self.vca_faders[v] <= 0.001 { -60.0 } else { 20.0 * self.vca_faders[v].log10() };
                                ui.label(egui::RichText::new(format!("{:.0}% {:+.1}dB", self.vca_faders[v] * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                            });
                        });
                    }

                    if group_changed {
                        self.sync_group_state();
                    }
                });
            });

            ui.add_space(8.0);

            // ================================================================
            // 2. TIER 2: DEDICATED PARAMETRIC EQ & CHANNEL STRIP FOR SELECTED TRACK
            // ================================================================
            if !self.playlist_tracks.is_empty() {
                let sel_idx = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
                let sel_t_name = self.playlist_tracks[sel_idx].name.clone();
                let sel_t_col = self.playlist_tracks[sel_idx].color;
                let sel_t_icon = self.playlist_tracks[sel_idx].icon;
                let mut track_dirty = false;

                ui.group(|ui| {
                    // Header for Selected Track EQ Console
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(format!("🎛 DEDIKERAD PARAMETRISK EQ — {} {}", sel_t_icon, sel_t_name)).strong().size(12.5).color(sel_t_col));
                        ui.separator();
                        let mut eq_enabled = self.playlist_tracks[sel_idx].eq.enabled;
                        let eq_txt = if eq_enabled { "EQ Aktiv" } else { "Bypass (Av)" };
                        let eq_col = if eq_enabled { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                        if ui.checkbox(&mut eq_enabled, egui::RichText::new(eq_txt).size(11.0).color(eq_col)).changed() {
                            self.playlist_tracks[sel_idx].eq.enabled = eq_enabled;
                            track_dirty = true;
                        }

                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::t("Buss:")).size(10.0).color(Theme::TEXT_MUTED));
                        let cur_bus = self.playlist_tracks[sel_idx].bus.min(crate::audio::synth::NUM_BUSES - 1);
                        egui::ComboBox::from_id_salt("sel_track_bus")
                            .selected_text(crate::i18n::t(crate::audio::synth::BUS_NAMES[cur_bus]))
                            .width(84.0)
                            .show_ui(ui, |ui| {
                                for b in 0..crate::audio::synth::NUM_BUSES {
                                    if ui.selectable_label(cur_bus == b, crate::i18n::t(crate::audio::synth::BUS_NAMES[b])).clicked() && cur_bus != b {
                                        self.playlist_tracks[sel_idx].bus = b;
                                        track_dirty = true;
                                    }
                                }
                            });

                        ui.label(egui::RichText::new("VCA:").size(10.0).color(Theme::TEXT_MUTED));
                        let cur_vca = self.playlist_tracks[sel_idx].vca;
                        egui::ComboBox::from_id_salt("sel_track_vca")
                            .selected_text(match cur_vca {
                                Some(v) => format!("VCA {}", v + 1),
                                None => crate::i18n::t("Ingen").to_string(),
                            })
                            .width(84.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(cur_vca.is_none(), crate::i18n::t("Ingen")).clicked() {
                                    self.playlist_tracks[sel_idx].vca = None;
                                    track_dirty = true;
                                }
                                for v in 0..crate::audio::synth::NUM_VCAS {
                                    if ui.selectable_label(cur_vca == Some(v), format!("VCA {}", v + 1)).clicked() {
                                        self.playlist_tracks[sel_idx].vca = Some(v);
                                        track_dirty = true;
                                    }
                                }
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}", sel_idx + 1, self.playlist_tracks.len())).size(10.5).color(Theme::TEXT_MUTED));
                        });
                    });

                    ui.add_space(4.0);

                    // Responsive 2-Row / Multi-Column Section for Parametric EQ, Curves, Presets & Dynamics
                    let mut status_msg_update: Option<String> = None;
                    let track_mut = &mut self.playlist_tracks[sel_idx];

                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);

                        // Block A: 3-Band Parametric EQ Knobs
                        ui.group(|ui| {
                            ui.set_width(260.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("📊 3-BANDS FREKVENSJUSTERING")).strong().size(10.5).color(Theme::FL_ORANGE));
                                ui.separator();

                                ui.horizontal(|ui| {
                                    // Low Band
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("LOW SHELF")).size(8.5).color(Theme::FL_ORANGE));
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.low_gain_db, -12.0, 12.0, "GAIN", Theme::FL_ORANGE, 20.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.low_freq, 30.0, 400.0, "FREQ", Theme::FL_ORANGE, 16.0);
                                        ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.low_freq, track_mut.eq.low_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                    });

                                    ui.separator();

                                    // Mid Band
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("MID BELL")).size(8.5).color(Theme::FL_YELLOW));
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_gain_db, -12.0, 12.0, "GAIN", Theme::FL_YELLOW, 20.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_freq, 200.0, 8000.0, "FREQ", Theme::FL_YELLOW, 16.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_q, 0.4, 4.0, "Q", Theme::FL_YELLOW, 16.0);
                                        ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.mid_freq, track_mut.eq.mid_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                    });

                                    ui.separator();

                                    // High Band
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("HIGH SHELF")).size(8.5).color(Theme::FL_CYAN));
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.high_gain_db, -12.0, 12.0, "GAIN", Theme::FL_CYAN, 20.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.eq.high_freq, 2500.0, 16000.0, "FREQ", Theme::FL_CYAN, 16.0);
                                        ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.high_freq, track_mut.eq.high_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                    });
                                });
                            });
                        });

                        // Block B: Interactive EQ Curve Display
                        ui.group(|ui| {
                            ui.set_width(320.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("📈 INTERAKTIV FREKVENSKURVA")).strong().size(10.5).color(Theme::FL_CYAN));
                                ui.separator();
                                track_dirty |= eq_curve_visualizer(
                                    ui,
                                    &mut track_mut.eq.low_gain_db,
                                    &mut track_mut.eq.mid_gain_db,
                                    &mut track_mut.eq.high_gain_db,
                                    Vec2::new(305.0, 84.0),
                                );
                            });
                        });

                        // Block C: Quick EQ Presets & Reset
                        ui.group(|ui| {
                            ui.set_width(200.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("⚡ SNABBPRESETS")).strong().size(10.5).color(Theme::FL_GREEN));
                                ui.separator();

                                ui.horizontal_wrapped(|ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sång (Vocal Air)")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 50, 70))).clicked() {
                                        track_mut.eq.low_gain_db = -2.5;
                                        track_mut.eq.low_freq = 120.0;
                                        track_mut.eq.mid_gain_db = 2.0;
                                        track_mut.eq.mid_freq = 3200.0;
                                        track_mut.eq.high_gain_db = 4.0;
                                        track_mut.eq.high_freq = 11000.0;
                                        track_mut.eq.enabled = true;
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Sång (Vocal Air)' på {}", sel_t_name));
                                    }

                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗣 Kör / Harmoni")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 30, 70))).clicked() {
                                        track_mut.eq.low_gain_db = -4.0;
                                        track_mut.eq.low_freq = 160.0;
                                        track_mut.eq.mid_gain_db = -1.5;
                                        track_mut.eq.mid_freq = 1200.0;
                                        track_mut.eq.high_gain_db = 3.0;
                                        track_mut.eq.high_freq = 9000.0;
                                        track_mut.eq.enabled = true;
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Kör / Harmoni' på {}", sel_t_name));
                                    }

                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🥁 Trum-Punch")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(70, 45, 20))).clicked() {
                                        track_mut.eq.low_gain_db = 4.5;
                                        track_mut.eq.low_freq = 80.0;
                                        track_mut.eq.mid_gain_db = -2.5;
                                        track_mut.eq.mid_freq = 450.0;
                                        track_mut.eq.high_gain_db = 3.5;
                                        track_mut.eq.high_freq = 7000.0;
                                        track_mut.eq.enabled = true;
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Trum-Punch' på {}", sel_t_name));
                                    }

                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Bas / Sub Power")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(20, 60, 35))).clicked() {
                                        track_mut.eq.low_gain_db = 5.0;
                                        track_mut.eq.low_freq = 65.0;
                                        track_mut.eq.mid_gain_db = -2.0;
                                        track_mut.eq.mid_freq = 600.0;
                                        track_mut.eq.high_gain_db = -4.0;
                                        track_mut.eq.high_freq = 4000.0;
                                        track_mut.eq.enabled = true;
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Bas / Sub Power' på {}", sel_t_name));
                                    }

                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Gitarr / Presence")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(65, 55, 20))).clicked() {
                                        track_mut.eq.low_gain_db = -2.0;
                                        track_mut.eq.low_freq = 100.0;
                                        track_mut.eq.mid_gain_db = 3.0;
                                        track_mut.eq.mid_freq = 2400.0;
                                        track_mut.eq.high_gain_db = 2.0;
                                        track_mut.eq.high_freq = 6000.0;
                                        track_mut.eq.enabled = true;
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Gitarr / Presence' på {}", sel_t_name));
                                    }

                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Nollställ EQ")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(45, 50, 60))).clicked() {
                                        track_mut.eq = TrackEq::default();
                                        track_dirty = true;
                                        status_msg_update = Some(crate::tstatus!("🔄 Nollställde EQ för {}", sel_t_name));
                                    }
                                });
                            });
                        });

                        // Block D: Dynamics & FX Sends for this Track
                        ui.group(|ui| {
                            ui.set_width(250.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("🎚 DYNAMIK & EFFEKT-SÄNDNING")).strong().size(10.5).color(Theme::FL_PURPLE));
                                ui.separator();

                                ui.horizontal(|ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("KOMPRESSOR")).size(8.0).color(Theme::FL_GREEN));
                                        track_dirty |= rotary_knob(ui, &mut track_mut.comp_threshold_db, -36.0, 0.0, "TRSH", Theme::FL_GREEN, 17.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.comp_ratio, 1.0, 10.0, "RATIO", Theme::FL_GREEN, 17.0);
                                    });

                                    ui.separator();

                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("SÄNDNING")).size(8.0).color(Theme::FL_PURPLE));
                                        track_dirty |= rotary_knob(ui, &mut track_mut.reverb_send, 0.0, 1.0, "REV", Theme::FL_PURPLE, 17.0);
                                        track_dirty |= rotary_knob(ui, &mut track_mut.delay_send, 0.0, 1.0, "DLY", Theme::FL_CYAN, 17.0);
                                    });

                                    ui.separator();

                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("TONHÖJD")).size(8.0).color(Theme::FL_YELLOW));
                                        track_dirty |= pitch_knob(ui, &mut track_mut.pitch_semitones, -12.0, 12.0, "PITCH", Theme::FL_YELLOW, 17.0);
                                        ui.label(egui::RichText::new(format!("{:+.2} st", track_mut.pitch_semitones)).size(7.5).color(Theme::TEXT_MUTED));
                                    });
                                });
                            });
                        });
                    });

                    if let Some(msg) = status_msg_update {
                        self.status_message = msg;
                    }
                });

                if track_dirty {
                    self.sync_track_audio_state(sel_idx);
                }
            }

            ui.add_space(8.0);

            // ================================================================
            // 3. TIER 3: MASTER FX STUDIO (SPACE, DELAY & ANALOG DRIVE)
            // ================================================================
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 6.0);

                    // Delay FX
                    ui.group(|ui| {
                        ui.set_width(320.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🌊 MASTER STEREO DELAY")).strong().size(10.5).color(Theme::FL_CYAN));
                            ui.separator();
                            ui.horizontal(|ui| {
                                let mut ch = false;
                                ch |= rotary_knob(ui, &mut self.delay.time_ms, 30.0, 800.0, "TID (ms)", Theme::FL_CYAN, 19.0);
                                ch |= rotary_knob(ui, &mut self.delay.feedback, 0.0, 0.88, "FEEDBACK", Theme::FL_CYAN, 19.0);
                                ch |= rotary_knob(ui, &mut self.delay.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 19.0);
                                if ch { let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay)); }
                            });
                        });
                    });

                    // Reverb FX
                    ui.group(|ui| {
                        ui.set_width(320.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("✨ MASTER SPACE REVERB")).strong().size(10.5).color(Theme::FL_PURPLE));
                            ui.separator();
                            ui.horizontal(|ui| {
                                let mut ch = false;
                                ch |= rotary_knob(ui, &mut self.reverb.room_size, 0.1, 0.95, "RUM", Theme::FL_PURPLE, 19.0);
                                ch |= rotary_knob(ui, &mut self.reverb.damping, 0.05, 0.9, "DÄMP", Theme::FL_PURPLE, 19.0);
                                ch |= rotary_knob(ui, &mut self.reverb.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 19.0);
                                if ch { let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb)); }
                            });
                        });
                    });

                    // Drive & Limiter FX
                    ui.group(|ui| {
                        ui.set_width(340.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🔥 MASTER ANALOG DRIVE & LIMITER")).strong().size(10.5).color(Theme::FL_YELLOW));
                            ui.separator();
                            ui.horizontal(|ui| {
                                let mut ch = false;
                                ch |= rotary_knob(ui, &mut self.stompbox_drive, 1.0, 8.0, "DRIVE", Theme::FL_ORANGE, 19.0);
                                ch |= rotary_knob(ui, &mut self.stompbox_tone, 1000.0, 10000.0, "TON", Theme::FL_YELLOW, 19.0);
                                ui.checkbox(&mut self.stompbox_cab, "4x12 Cab");
                                if ch {
                                    self.drive = self.stompbox_drive;
                                    let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                                }
                            });
                        });
                    });
                });
            });
        });
    }

    fn render_synth_hardware_rack(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX HARDWARE SYNTHESIZER")).strong().size(13.0).color(Theme::FL_CYAN));
                ui.separator();

                // Presets
                ui.label(egui::RichText::new(crate::i18n::t("Presets:")).color(Theme::TEXT_MUTED));
                let presets = [
                    (Preset::CleanPluck, "Clean Pluck"),
                    (Preset::WarmPad, "Warm Pad"),
                    (Preset::AcidBass, "303 Acid Bass"),
                    (Preset::ChiptuneLead, "8-Bit Chiptune"),
                    (Preset::CosmicBrass, "Cosmic Brass"),
                ];
                for (p, name) in presets {
                    let is_active = self.current_preset == p;
                    let fill = if is_active { Theme::FL_CYAN } else { Theme::PANEL_BG };
                    let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                    if ui.add(egui::Button::new(egui::RichText::new(name).strong().color(text_color)).fill(fill)).clicked() {
                        self.current_preset = p;
                        let (wf, adsr, flt) = p.settings();
                        self.waveform = wf;
                        self.adsr = adsr;
                        self.filter = flt;
                        let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                    }
                }
            });

            ui.add_space(8.0);

            ui.columns(4, |cols| {
                // Column 1: Waveform & Octave
                cols[0].group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("OSCILLATOR")).strong());
                    let waveforms = [
                        (Waveform::Sine, crate::i18n::t("∿ Sinus")),
                        (Waveform::Saw, crate::i18n::t("⩘ Sågtand")),
                        (Waveform::Square, crate::i18n::t("⊓ Fyrkant")),
                        (Waveform::Triangle, crate::i18n::t("⋀ Triangel")),
                    ];
                    for (wf, name) in waveforms {
                        let is_wf = self.waveform == wf;
                        if ui.selectable_label(is_wf, name).clicked() {
                            self.waveform = wf;
                            let _ = self.engine.send_command(AudioCommand::SetWaveform(wf));
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Oktav:"));
                        if ui.button(crate::i18n::t("➖ (Z)")).clicked() && self.octave > 1 { self.octave -= 1; }
                        ui.label(egui::RichText::new(format!("C{}", self.octave)).strong().color(Theme::FL_CYAN));
                        if ui.button(crate::i18n::t("➕ (X)")).clicked() && self.octave < 8 { self.octave += 1; }
                    });
                });

                // Column 2: Resonant filter controls
                cols[1].group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("RESONANT SVF FILTER (12 dB)")).strong().color(Theme::FL_ORANGE));
                    ui.horizontal(|ui| {
                        let mut flt_changed = false;
                        let mut fenv_changed = false;
                        flt_changed |= rotary_knob(ui, &mut self.filter.cutoff, 60.0, 18000.0, "CUTOFF", Theme::FL_ORANGE, 40.0);
                        flt_changed |= rotary_knob(ui, &mut self.filter.resonance, 0.5, 9.5, "RESO (Q)", Theme::FL_YELLOW, 40.0);
                        fenv_changed |= rotary_knob(ui, &mut self.filter_env_amount, -6.0, 6.0, "ENV ±oct", Theme::FL_ORANGE, 40.0);
                        if flt_changed {
                            let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                        }
                        if fenv_changed {
                            let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
                                amount: self.filter_env_amount,
                                adsr: self.filter_env,
                            });
                        }
                    });
                    ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz | Q: {:.1} | Env: {:+.1} oct", self.filter.cutoff, self.filter.resonance, self.filter_env_amount)).size(10.0).color(Theme::TEXT_MUTED));
                });

                // Column 3: ADSR Rotary Envelopes
                cols[2].group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("ADSR ENVELOPE")).strong().color(Theme::FL_PURPLE));
                    let mut adsr_changed = false;
                    ui.horizontal(|ui| {
                        adsr_changed |= rotary_knob(ui, &mut self.adsr.attack, 0.002, 2.0, "ATTK", Theme::FL_PURPLE, 32.0);
                        adsr_changed |= rotary_knob(ui, &mut self.adsr.decay, 0.01, 3.0, "DECAY", Theme::FL_PURPLE, 32.0);
                        adsr_changed |= rotary_knob(ui, &mut self.adsr.sustain, 0.0, 1.0, "SUST", Theme::FL_PURPLE, 32.0);
                        adsr_changed |= rotary_knob(ui, &mut self.adsr.release, 0.01, 4.0, "REL", Theme::FL_PURPLE, 32.0);
                    });
                    if adsr_changed {
                        let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
                    }
                });

                // Column 4: ADSR Visual Curve Display
                cols[3].group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("ENVELOPE GRAF")).strong().color(Theme::FL_GREEN));
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(130.0, 68.0), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, Rounding::same(3.0), Theme::LCD_BG);
                    painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 52)));

                    let p0 = Pos2::new(rect.min.x + 4.0, rect.max.y - 4.0);
                    let p1 = Pos2::new(rect.min.x + 4.0 + (self.adsr.attack * 20.0).min(30.0), rect.min.y + 6.0);
                    let p2 = Pos2::new(p1.x + (self.adsr.decay * 18.0).min(30.0), rect.max.y - 4.0 - (self.adsr.sustain * (rect.height() - 14.0)));
                    let p3 = Pos2::new(p2.x + 25.0, p2.y);
                    let p4 = Pos2::new((p3.x + (self.adsr.release * 16.0).min(35.0)).min(rect.max.x - 4.0), rect.max.y - 4.0);

                    painter.line_segment([p0, p1], Stroke::new(2.0_f32, Theme::FL_GREEN));
                    painter.line_segment([p1, p2], Stroke::new(2.0_f32, Theme::FL_GREEN));
                    painter.line_segment([p2, p3], Stroke::new(2.0_f32, Theme::FL_GREEN));
                    painter.line_segment([p3, p4], Stroke::new(2.0_f32, Theme::FL_GREEN));
                });
            });
        });
    }

    fn render_touch_piano_keyboard(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO")).strong().size(13.0).color(Theme::TEXT_BRIGHT));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Spela med tangentbordet [A, S, D...] eller klicka med musen")).size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(6.0);

            let key_width = 38.0;
            let white_key_height = 100.0;
            let black_key_height = 64.0;
            let black_key_width = 24.0;

            let start_note = (self.octave + 1) as u8 * 12;

            let (keyboard_rect, _) = ui.allocate_exact_size(
                Vec2::new(key_width * 14.0, white_key_height),
                Sense::hover(),
            );

            let white_offsets = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17, 19, 21, 23];
            let black_defs = [
                (1, 0), (3, 1), (6, 3), (8, 4), (10, 5),
                (13, 7), (15, 8), (18, 10), (20, 11), (22, 12),
            ];

            let mut hovered_note = None;
            let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
            let pointer_down = ctx.input(|i| i.pointer.primary_down());

            // 1. White Keys
            for (i, &semi) in white_offsets.iter().enumerate() {
                let note = start_note + semi as u8;
                let key_rect = Rect::from_min_size(
                    Pos2::new(keyboard_rect.min.x + i as f32 * key_width, keyboard_rect.min.y),
                    Vec2::new(key_width - 2.0, white_key_height),
                );

                let is_active = self.active_keys.contains(&note);
                let bg_color = if is_active {
                    Theme::FL_CYAN
                } else {
                    Color32::from_rgb(240, 243, 250)
                };

                ui.painter().rect_filled(key_rect, Rounding::same(3.0), bg_color);
                ui.painter().rect_stroke(key_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(170, 178, 190)));

                ui.painter().text(
                    Pos2::new(key_rect.center().x, key_rect.max.y - 12.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{}{}", note_name(note), (note / 12) as i8 - 1),
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(40, 45, 55),
                );

                if let Some(pos) = pointer_pos
                    && key_rect.contains(pos) {
                        hovered_note = Some(note);
                    }
            }

            // 2. Black Keys
            for &(semi, white_idx) in &black_defs {
                let note = start_note + semi as u8;
                let x_pos = keyboard_rect.min.x + (white_idx as f32 + 0.68) * key_width;
                let key_rect = Rect::from_min_size(
                    Pos2::new(x_pos, keyboard_rect.min.y),
                    Vec2::new(black_key_width, black_key_height),
                );

                let is_active = self.active_keys.contains(&note);
                let bg_color = if is_active {
                    Theme::FL_ORANGE
                } else {
                    Color32::from_rgb(28, 32, 40)
                };

                ui.painter().rect_filled(key_rect, Rounding::same(2.0), bg_color);
                ui.painter().rect_stroke(key_rect, Rounding::same(2.0), Stroke::new(1.0_f32, Color32::BLACK));

                if let Some(pos) = pointer_pos
                    && key_rect.contains(pos) {
                        hovered_note = Some(note);
                    }
            }

            // Mouse Touch Interaction
            if pointer_down {
                if let Some(note) = hovered_note
                    && self.active_mouse_note != Some(note) {
                        if let Some(old) = self.active_mouse_note {
                            self.release_note(old);
                        }
                        self.active_mouse_note = Some(note);
                        self.play_note(note);
                    }
            } else if let Some(old) = self.active_mouse_note {
                self.release_note(old);
                self.active_mouse_note = None;
            }
        });
    }

    fn render_hardware_controller_modal(&mut self, ctx: &egui::Context) {
        if !self.show_controller_modal {
            return;
        }

        let mut close = false;
        egui::Window::new(crate::i18n::t("🎛 Hårdvarukontroller & MCU / OSC Routing"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(egui::RichText::new(crate::i18n::t("Direkt hårdvaruintegration med Mackie Control Universal (MCU) och Open Sound Control (OSC)")).size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                // Section 1: Mackie MCU
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎚 MACKIE CONTROL UNIVERSAL (MCU)")).strong().size(12.0).color(Theme::FL_ORANGE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (status_txt, status_col) = if self.mcu_input.is_some() {
                                (crate::i18n::t("● ALSA SEQUENCER-PORT ÖPPEN"), Theme::FL_GREEN)
                            } else {
                                (crate::i18n::t("○ FRÅNKOPPLAD"), Theme::TEXT_MUTED)
                            };
                            ui.label(egui::RichText::new(status_txt).strong().size(10.0).color(status_col));
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Enhet:"));
                        ui.label(egui::RichText::new(&self.mcu_device_name).strong().color(Theme::FL_CYAN));
                    });

                    ui.horizontal(|ui| {
                        if self.mcu_input.is_some() {
                            if ui.button(crate::i18n::t("🔌 Koppla från MCU")).clicked() {
                                self.mcu_input = None;
                                self.mcu_connected = false;
                                self.status_message = crate::i18n::t("MCU frånkopplad.").to_string();
                            }
                        } else if ui.button(crate::i18n::t("🔌 Anslut MCU (ALSA Seq)")).clicked() {
                            match McuInput::connect(self.control_tx.clone()) {
                                Ok(m) => {
                                    self.mcu_input = Some(m);
                                    self.status_message = crate::i18n::t("✔ MCU-port öppnad. Anslut enhet med 'aconnect'.").to_string();
                                }
                                Err(e) => {
                                    self.status_message = crate::tstatus!("⚠ Kunde inte öppna MCU: {}", e);
                                }
                            }
                        }
                        if self.mcu_input.is_some() {
                            ui.label(egui::RichText::new(format!("{} events", self.mcu_input.as_ref().map(|m| m.received()).unwrap_or(0))).size(10.0).color(Theme::FL_GREEN));
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Fader Bank:"));
                        let banks = [crate::i18n::t("Bank 1 (Spår 1–8)"), crate::i18n::t("Bank 2 (Spår 9–16)"), crate::i18n::t("Bank 3 (Bussar & VCA)")];
                        for (b_i, name) in banks.iter().enumerate() {
                            let is_b = self.mcu_bank == b_i;
                            if ui.selectable_label(is_b, *name).clicked() {
                                self.mcu_bank = b_i;
                                self.status_message = crate::tstatus!("MCU växlade till {}", name);
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Motorfaders feedback status:")).size(10.0).color(Theme::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        for i in 0..8 {
                            let ch_vol = if i < self.playlist_tracks.len() { self.playlist_tracks[i].volume } else if i < self.channels.len() { self.channels[i].volume } else { 0.8 };
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(format!("CH{}", i + 1)).size(8.0));
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 36.0), Sense::hover());
                                ui.painter().rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(20, 24, 30));
                                let h = (ch_vol * 34.0).clamp(0.0, 34.0);
                                let fill_rect = Rect::from_min_max(Pos2::new(rect.min.x + 1.0, rect.max.y - h), Pos2::new(rect.max.x - 1.0, rect.max.y - 1.0));
                                ui.painter().rect_filled(fill_rect, Rounding::same(1.0), Theme::FL_ORANGE);
                            });
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let learn_txt = if self.midi_learn_active { "🔴 MIDI LEARN: LYSSNAR..." } else { "🎯 Aktivera MIDI Learn" };
                        let learn_bg = if self.midi_learn_active { Color32::RED } else { Theme::PANEL_BG };
                        if ui.add(egui::Button::new(egui::RichText::new(learn_txt).strong().size(10.5).color(Color32::WHITE)).fill(learn_bg)).clicked() {
                            self.midi_learn_active = !self.midi_learn_active;
                        }
                    });
                });

                ui.add_space(8.0);

                // Section 1b: MIDI Keyboard (Piano Roll input) — Fas 5.3
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎹 MIDI-KLAVIATUR (Piano Roll)")).strong().size(12.0).color(Theme::FL_GREEN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (status_txt, status_col) = if self.midi_keyboard_connected {
                                (crate::i18n::t("● MIDI-IN-PORT ÖPPEN"), Theme::FL_GREEN)
                            } else {
                                (crate::i18n::t("○ FRÅNKOPPLAD"), Theme::TEXT_MUTED)
                            };
                            ui.label(egui::RichText::new(status_txt).strong().size(10.0).color(status_col));
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Enhet:"));
                        ui.label(egui::RichText::new(&self.midi_device_name).strong().color(Theme::FL_CYAN));
                    });

                    ui.horizontal(|ui| {
                        if self.midi_input.is_some() {
                            if ui.button(crate::i18n::t("🔌 Koppla från MIDI")).clicked() {
                                self.midi_input = None;
                                self.midi_keyboard_connected = false;
                                self.midi_held_notes.clear();
                                self.status_message = crate::i18n::t("MIDI-klaviatur frånkopplad.").to_string();
                            }
                        } else if ui.button(crate::i18n::t("🔌 Anslut MIDI (ALSA Seq)")).clicked() {
                            match MidiKeyboardInput::connect(self.control_tx.clone()) {
                                Ok(m) => {
                                    self.midi_input = Some(m);
                                    self.status_message = crate::i18n::t("✔ MIDI-in-port öppnad. Anslut ett klaviatur med 'aconnect'.").to_string();
                                }
                                Err(e) => {
                                    self.status_message = crate::tstatus!("⚠ Kunde inte öppna MIDI-in: {}", e);
                                }
                            }
                        }
                        if self.midi_input.is_some() {
                            ui.label(egui::RichText::new(crate::tstatus!("{} nothändelser", self.midi_note_count)).size(10.0).color(Theme::FL_GREEN));
                        }
                    });

                    ui.add_space(4.0);
                    let rec_bg = if self.midi_record_armed { Color32::from_rgb(180, 40, 40) } else { Theme::PANEL_BG };
                    let rec_txt = if self.midi_record_armed {
                        crate::i18n::t("🔴 MIDI-REC: ARMED (spelar in i Piano Roll)")
                    } else {
                        crate::i18n::t("⏺ Armera MIDI-inspelning till Piano Roll")
                    };
                    if ui.add(egui::Button::new(egui::RichText::new(rec_txt).strong().size(10.5).color(Color32::WHITE)).fill(rec_bg)).on_hover_text(crate::i18n::t("När armerad skrivs hållna MIDI-klaviaturtoner in i det aktiva mönstrets rutnät under uppspelning.")).clicked() {
                        self.midi_record_armed = !self.midi_record_armed;
                    if self.midi_record_armed {
                        self.begin_new_take();
                    }
                    }
                    ui.label(egui::RichText::new(crate::i18n::t("Tips: koppla ihop porten med 'aconnect <klaviatur> 'Sonix Keys:0'' och aktivera läget i Piano Roll.")).size(9.5).color(Theme::TEXT_MUTED));
                });

                ui.add_space(8.0);

                // Section 2: Open Sound Control (OSC)
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("📡 OPEN SOUND CONTROL (OSC / UDP)")).strong().size(12.0).color(Theme::FL_CYAN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.osc_server.is_some() {
                                if ui.button(crate::i18n::t("⏹ Stoppa server")).clicked() {
                                    self.osc_server = None;
                                    self.osc_enabled = false;
                                    self.osc_rx_count = 0;
                                    self.status_message = crate::i18n::t("OSC-server stoppad.").to_string();
                                }
                            } else if ui.button(crate::i18n::t("▶ Starta server")).clicked() {
                                match OscServer::start(self.osc_rx_port, self.control_tx.clone()) {
                                    Ok(s) => {
                                        let bound = s.bound_port;
                                        self.osc_server = Some(s);
                                        self.osc_enabled = true;
                                        self.status_message = crate::tstatus!("📡 OSC-server lyssnar på UDP-port {}", bound);
                                    }
                                    Err(e) => {
                                        self.status_message = crate::tstatus!("⚠ Kunde inte starta OSC: {}", e);
                                    }
                                }
                            }
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Lyssningsport (RX):"));
                        ui.add(egui::DragValue::new(&mut self.osc_rx_port).range(1024..=65535));
                        ui.separator();
                        ui.label(crate::i18n::t("Sändningsport (TX):"));
                        ui.add(egui::DragValue::new(&mut self.osc_tx_port).range(1024..=65535));
                    });

                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        let status_col = if self.osc_server.is_some() { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                        let status_txt = match &self.osc_server {
                            Some(s) => format!("Lyssnar på UDP-port {} • mottagna OSC-paket: {}", s.bound_port, self.osc_rx_count),
                            None => format!("Mottagna OSC-paket: {}", self.osc_rx_count),
                        };
                        ui.label(egui::RichText::new(status_txt).size(10.5).color(status_col));
                        if let Some(err) = self.osc_server.as_ref().and_then(|s| s.last_error.lock().ok().and_then(|g| g.clone())) {
                            ui.label(egui::RichText::new(crate::tstatus!("⚠ {}", err)).size(10.0).color(Theme::FL_YELLOW));
                        }
                        if ui.button(crate::i18n::t("⚡ Skicka Test Ping (/sonix/ping)")).clicked() {
                            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], self.osc_tx_port));
                            let mut pkt = Vec::new();
                            pkt.extend_from_slice(b"/sonix/ping\0\0\0");
                            pkt.push(b','); pkt.push(b'f'); pkt.push(0); pkt.push(0);
                            pkt.extend_from_slice(&1.0f32.to_be_bytes());
                            match std::net::UdpSocket::bind("0.0.0.0:0")
                                .and_then(|s| s.send_to(&pkt, addr))
                            {
                                Ok(n) => self.status_message = crate::tstatus!("📡 Sände {} byte OSC till {}", n, addr.to_string()),
                                Err(e) => self.status_message = crate::tstatus!("⚠ OSC-sändning misslyckades: {}", e),
                            }
                        }
                    });

                    ui.collapsing("📋 OSC Adress-mappningar", |ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("• /sonix/transport/play (1/0)\n• /sonix/track/{1..8}/volume (0.0 .. 1.25)\n• /sonix/track/{1..8}/mute (1/0)\n• /sonix/bus/{vocal|drum|synth|fx}/volume (0.0 .. 1.25)\n• /sonix/vca/{1..4}/volume (0.0 .. 1.25)")).size(10.0).color(Theme::TEXT_MUTED));
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(self.tr("Stäng")).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_controller_modal = false;
        }
    }

    const EXPORT_FORMATS: [crate::audio::ExportFormat; 7] = [
        crate::audio::ExportFormat::Wav16,
        crate::audio::ExportFormat::Wav24,
        crate::audio::ExportFormat::Wav32,
        crate::audio::ExportFormat::Flac,
        crate::audio::ExportFormat::Mp3,
        crate::audio::ExportFormat::Ogg,
        crate::audio::ExportFormat::Aac,
    ];

    pub fn open_export_modal(&mut self) {
        if self.export_base_name.trim().is_empty()
            || self.export_base_name == crate::i18n::t("Min_Låt")
            || self.export_metadata.title.is_empty() {
            let safe = crate::audio::sanitize_filename(&self.project_name);
            if !safe.is_empty() {
                self.export_base_name = safe;
            }
            if self.export_metadata.title.is_empty() {
                self.export_metadata.title = self.project_name.clone();
            }
        }
        self.show_render_queue_modal = true;
        self.render_queue_status = crate::i18n::t("Klar för rendering").to_string();
    }

    fn export_bars(&self) -> usize {
        self.loop_end_bar.clamp(4, 32)
    }

    /// Dither-inställningarna från exportdialogen (Fas 6.5).
    fn export_dither_settings(&self) -> crate::audio::DitherSettings {
        crate::audio::DitherSettings {
            enabled: self.export_dither,
            noise_shaping: self.export_noise_shaping,
            seed: crate::audio::dither::DEFAULT_SEED,
        }
    }

    fn export_sample_rate(&self) -> u32 {
        match self.render_sample_rate_idx {
            1 => 48000,
            2 => 96000,
            _ => 44100,
        }
    }

    /// Snapshot the current project (real Channel Rack samples, patterns,
    /// timeline stems) into the pure renderer data model.
    fn build_render_spec(&self, solo_track: Option<usize>, sample_rate: u32) -> crate::audio::RenderSpec {
        use crate::audio::{PatternSnap, RackChannel, TrackAudioSnap, TrackRole, TrackSnap, VoiceSpec};

        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let pattern_mode = self.pattern_mode;

        let rack = self.channels.iter().map(|ch| {
            let voice = ch.pcm_audio.as_ref().map(|(l, r, sr)| VoiceSpec {
                left: l.clone(),
                right: r.clone(),
                sample_rate: *sr,
                base_note: ch.sample_base_note,
                semitones: ch.pitch_semitones,
                cents: ch.pitch_fine_cents,
                volume: ch.volume,
                reverse: ch.is_reverse,
                start: ch.sample_start,
                end: ch.sample_end,
            });
            RackChannel {
                voice,
                fallback_volume: ch.volume,
                steps: ch.steps,
                notes: ch.notes,
            }
        }).collect();

        let patterns = self.patterns.iter().map(|p| PatternSnap {
            steps: p.channel_steps.clone(),
            notes: p.channel_notes.clone(),
            piano_roll: p.piano_roll_grid,
            take: p.take.clone(),
        }).collect();

        let tracks = self.playlist_tracks.iter().map(|t| {
            let role = match t.kind {
                TrackKind::Drums => TrackRole::Drums,
                TrackKind::SynthLead => TrackRole::Synth,
                TrackKind::Bassline => TrackRole::Bass,
                TrackKind::VocalAudio | TrackKind::CustomAudio | TrackKind::Fx => TrackRole::Audio,
            };
            // Ett fruset spår bidrar med sitt LJUd i stället för sina patterns
            // (se render_clips_for); ljudet ligger i tidslinjen nedan.
            let clips = render_clips_for(t);
            TrackSnap {
                role,
                clips,
                volume: t.volume,
                muted: t.muted,
                solo: t.solo,
            }
        }).collect();

        let timeline = self.playlist_tracks.iter().enumerate().filter_map(|(idx, t)| {
            if !frozen_audio_in_render(t, pattern_mode) {
                return None;
            }
            let (l, r, sr) = t.frozen_pcm.as_ref().or(t.pcm_audio.as_ref())?;
            // Samma väg som uppspelningen använder — ett ställe för
            // omräkningen takter→sekunder, i stället för ett här och ett där.
            let regions = self.stem_regions_for(t);
            // Sends hör till ljudspårens väg. Pattern-vägen som frysningen
            // ersätter har inga, så de nollas för det frusna spåret — annars
            // skulle det plötsligt få klang som live-uppspelningen inte hade.
            let (reverb_send, delay_send) = if t.frozen_pcm.is_some() {
                (0.0, 0.0)
            } else {
                (t.reverb_send, t.delay_send)
            };
            Some(TrackAudioSnap {
                track_index: idx,
                left: l.clone(),
                right: r.clone(),
                sample_rate: *sr,
                volume: t.volume,
                pan: t.pan,
                muted: t.muted,
                regions,
                eq: t.eq.to_settings(),
                comp_threshold_db: t.comp_threshold_db,
                comp_ratio: t.comp_ratio,
                reverb_send,
                delay_send,
                pitch_semitones: t.pitch_semitones,
                bus: t.bus,
                vca: t.vca,
            })
        }).collect();

        crate::audio::RenderSpec {
            sample_rate,
            bpm: self.bpm,
            swing: self.swing,
            num_bars: self.export_bars(),
            pattern_mode,
            rack,
            patterns,
            tracks,
            timeline,
            velocities: self.step_velocities,
            solo_track,
            tail_secs: (tempo.secs_per_beat_at(0.0) as f32).min(2.0),
            bus_volume: self.bus_volume,
            bus_muted: self.bus_muted,
            bus_solo: self.bus_solo,
            vca_volume: self.vca_faders,
            vca_muted: self.vca_muted,
            vca_solo: self.vca_solos,
        }
    }

    fn export_fx_state(&self, dry: bool) -> crate::audio::FxState {
        let mut delay = self.delay;
        let mut reverb = self.reverb;
        if dry {
            delay.mix = 0.0;
            reverb.mix = 0.0;
        }
        crate::audio::FxState {
            waveform: self.waveform,
            adsr: self.adsr,
            filter: self.filter,
            delay,
            reverb,
            drive: if dry { 1.0 } else { self.drive },
            master_volume: self.master_volume,
            master_fx: if dry {
                crate::audio::MasterFxParams::default()
            } else {
                self.fx_rack_state.build_master_fx_params()
            },
        }
    }

    /// Render one spec to an interleaved float buffer.
    fn render_buffer(&self, spec: &crate::audio::RenderSpec, dry: bool) -> Result<Vec<f32>, String> {
        let mut engine = crate::audio::build_offline_engine(spec, &self.export_fx_state(dry));
        Ok(crate::audio::render_project_offline(&mut engine, spec))
    }

    /// Peak level, used to skip completely empty stem files.
    fn peak_of(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, &s| m.max(s.abs()))
    }

    /// Fingeravtryck av allt som påverkar hur ett spår låter i en offline-render.
    ///
    /// Det är en **varning** om något ändrats, inte ett bevis på att ljudet är
    /// identiskt: fingeravtrycket ser att något skiljer sig, det avgör inte vad
    /// som är rätt. Mastern och bussarna är medvetet inte med — frysningen
    /// renderas torr och mastern gäller live även efteråt — och en ändring av ett
    /// *annat* spår spelar ingen roll, eftersom renderingen är solad.
    fn frozen_digest(&self, t_idx: usize) -> u64 {
        use std::fmt::Write as _;
        let mut text = String::new();
        let _ = write!(text, "bpm={:.4};swing={:.4};", self.bpm, self.swing);
        if let Some(t) = self.playlist_tracks.get(t_idx) {
            let _ = write!(
                text,
                "spår={:?}|{:.4},{:.4},{};",
                t.clips, t.volume, t.pan, t.muted
            );
            // Mönstren klippen pekar på. Andra spår kan peka på samma mönster —
            // då är de frusna på samma innehåll, vilket är riktigt.
            for pat_idx in t.clips.iter().flatten() {
                if let Some(p) = self.patterns.get(*pat_idx) {
                    let _ = write!(text, "mönster{:?};", p);
                }
            }
        }
        for ch in self.channels.iter() {
            let _ = write!(
                text,
                "kanal={:?},{:.4},{:.4},{:.4},{:.4},{},{};",
                ch.sample_path,
                ch.volume,
                ch.sample_start,
                ch.sample_end,
                ch.pitch_semitones,
                ch.sample_base_note,
                ch.is_reverse
            );
        }
        let _ = write!(text, "röst={:?},{:?},{:?};", self.waveform, self.adsr, self.filter);
        crate::autosave::fingerprint(text.as_bytes())
    }

    /// Har spåret ändrats sedan det frystes? Då spelar det frusna ljudet inte
    /// längre det som står i projektet, och det sägs i stället för att tigas.
    pub fn frozen_is_stale(&self, t_idx: usize) -> bool {
        self.playlist_tracks
            .get(t_idx)
            .and_then(|t| t.frozen.as_ref())
            .is_some_and(|f| f.digest != self.frozen_digest(t_idx))
    }

    /// Var ett fruset spår ligger: projektets egen materialmapp, samma plats som
    /// inspelningar och stems. Namnet är deterministiskt, så en ny frysning
    /// skriver över sin egen fil i stället för att lämna skräp.
    pub fn frozen_path(&self, t_idx: usize, name: &str) -> String {
        crate::paths::paths()
            .project_assets_dir(&self.project_name)
            .join("Frozen")
            .join(format!("{}-{}.wav", crate::autosave::slug(name), t_idx))
            .to_string_lossy()
            .into_owned()
    }

    /// Fryser ett spår: renderar det offline och lägger in ljudet som ett
    /// stem-spår, så att uppspelningen slipper köra syntesen för det.
    ///
    /// Renderingen går genom **samma väg som exporten** (torr, solad på spåret),
    /// och spårets fader lämnas utanför — den ska fortsätta gälla live, som på
    /// ett ofruset spår.
    pub fn freeze_track(&mut self, t_idx: usize) {
        let Some(track) = self.playlist_tracks.get(t_idx) else {
            return;
        };
        if !track.can_freeze() {
            self.status_message = crate::i18n::t(
                "❄ Spåret kan inte frysas — det är redan fruset, eller ett ljudspår som redan ligger som ljud",
            )
            .to_string();
            return;
        }
        if self.plugin_slots.get(t_idx).is_some_and(|s| s.is_some()) {
            self.status_message = crate::i18n::t(
                "❄ Spåret har en plugin-insert, och offline-renderingen kan inte återskapa den — ta bort den först",
            )
            .to_string();
            return;
        }
        let name = track.name.clone();
        let (volume, pan) = (track.volume, track.pan);
        let sample_rate = self.engine.sample_rate;

        // Sola på spåret och ta bort dess fader ur specen: det som renderas är
        // spårets eget ljud, inte dess placering i mixen.
        let mut spec = self.build_render_spec(Some(t_idx), sample_rate);
        if let Some(t) = spec.tracks.get_mut(t_idx) {
            t.volume = 1.0;
        }
        let buffer = match self.render_buffer(&spec, true) {
            Ok(b) => b,
            Err(e) => {
                self.status_message =
                    crate::tstatus!("❄ Kunde inte rendera spåret: {}", e);
                return;
            }
        };
        if buffer.is_empty() || Self::peak_of(&buffer) <= 0.0 {
            self.status_message = crate::tstatus!(
                "❄ '{}' är tyst i den här låten — det finns inget att frysa",
                name
            );
            return;
        }

        let path = self.frozen_path(t_idx, &name);
        if let Some(dir) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // 32-bitars flyttal: en frysning är ett mellansteg och ska inte kvantisera
        // en enda gång innan den riktiga exporten gör det.
        let meta = crate::audio::ExportMeta {
            title: name.clone(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            year: String::new(),
            comment: "Fruset spår (mellansteg i Sonix)".to_string(),
        };
        if let Err(e) = crate::audio::write_export_with(
            &path,
            crate::audio::ExportFormat::Wav32,
            &buffer,
            sample_rate,
            &meta,
            crate::audio::DitherSettings::default(),
        ) {
            self.status_message =
                crate::tstatus!("❄ Kunde inte skriva den frusna filen: {}", e);
            return;
        }
        // Läs tillbaka från filen i stället för att behålla bufferten: då är det
        // filen som är sanningen, och en trasig skrivning upptäcks nu.
        let Some((l, r, pcm_sr)) = load_sample_pcm_arcs(&path) else {
            self.status_message = crate::tstatus!(
                "❄ Den frusna filen gick inte att läsa tillbaka: {}",
                path
            );
            return;
        };

        self.push_undo(&format!("Frys '{}'", name));
        let len_secs = l.len() as f32 / pcm_sr as f32;
        let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
            track_index: t_idx,
            left: l.clone(),
            right: r.clone(),
            sample_rate: pcm_sr as f32,
            volume,
            pan,
            start_time_secs: 0.0,
        });
        let digest = self.frozen_digest(t_idx);
        if let Some(t) = self.playlist_tracks.get_mut(t_idx) {
            t.frozen = Some(FrozenTrack {
                path: path.clone(),
                digest,
                stamp: crate::autosave::now_stamp(),
            });
            t.frozen_pcm = Some((l, r, pcm_sr));
        }
        self.sync_track_regions(t_idx);
        self.status_message = crate::tstatus!(
            "❄ Frös '{}': {:.1} s ljud, och pattern-uppspelningen hoppas över [Ångra: Ctrl+Z]",
            name,
            len_secs
        );
    }

    /// Tina upp ett spår: pattern-uppspelningen tar över igen.
    ///
    /// Den frusna filen lämnas kvar i projektets mapp — den är användarens
    /// material, och en ny frysning skriver över samma namn.
    pub fn unfreeze_track(&mut self, t_idx: usize) {
        let Some(track) = self.playlist_tracks.get(t_idx) else {
            return;
        };
        if !track.is_frozen() {
            return;
        }
        let name = track.name.clone();
        let (volume, pan) = (track.volume, track.pan);
        self.push_undo(&format!("Tina '{}'", name));
        // Att lasta ett stem med TOM ljudbuffert tömmer platsen i motorn: den
        // behåller eq/comp/plugin och det positionella indexet, vilket en
        // borttagen post inte skulle göra.
        let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
            track_index: t_idx,
            left: std::sync::Arc::new(Vec::new()),
            right: std::sync::Arc::new(Vec::new()),
            sample_rate: self.engine.sample_rate as f32,
            volume,
            pan,
            start_time_secs: 0.0,
        });
        if let Some(t) = self.playlist_tracks.get_mut(t_idx) {
            t.frozen = None;
            t.frozen_pcm = None;
        }
        self.sync_track_regions(t_idx);
        self.status_message = crate::tstatus!(
            "🔥 '{}' är upptinat — pattern-uppspelningen är tillbaka [Ångra: Ctrl+Z]",
            name
        );
    }

    fn render_batch_export_modal(&mut self, ctx: &egui::Context) {
        if !self.show_render_queue_modal {
            return;
        }

        let mut close = false;
        egui::Window::new(self.tr("📤 Exportera projekt"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(620.0);
                ui.label(egui::RichText::new(self.tr("Rendera hela projektet offline (riktiga WAV-samples, tidslinje-audio & effekter) med ren metadata – ingen AI- eller leverantörsinformation läggs någonsin till.")).size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                // 1. Scope
                ui.group(|ui| {
                    ui.label(egui::RichText::new(self.tr("1. VAD SKALL EXPORTERAS")).strong().size(11.5).color(Theme::FL_ORANGE));
                    ui.separator();
                    let scopes: [(&'static str, &'static str); 3] = [
                        (crate::i18n::t("Hel låt – Master Mix (Fullt Projekt)"), crate::i18n::t("Renderar hela låten/arrangemanget med alla Channel Rack-samples, tidslinje-audio (stems/mic) och master-effekter.")),
                        (crate::i18n::t("Individuella spår – Torra (Stems Dry)"), crate::i18n::t("Exporterar varje tidslinjespår för sig utan reverb/delay för extern mixning.")),
                        (crate::i18n::t("Individuella spår – Med FX (Stems Wet)"), crate::i18n::t("Exporterar varje tidslinjespår för sig med alla effekter och modulation.")),
                    ];
                    for (i, (title, desc)) in scopes.iter().enumerate() {
                        if ui.selectable_label(self.render_scope_idx == i, format!("⦿ {}", self.tr(title))).clicked() {
                            self.render_scope_idx = i;
                        }
                        ui.label(egui::RichText::new(format!("    {}", self.tr(desc))).size(9.5).color(Theme::TEXT_MUTED));
                    }
                });

                ui.add_space(6.0);

                // 2. Format & quality
                ui.group(|ui| {
                    ui.label(egui::RichText::new(self.tr("2. FORMAT & LJUDKVALITET")).strong().size(11.5).color(Theme::FL_CYAN));
                    ui.separator();

                    ui.horizontal_wrapped(|ui| {
                        ui.label(self.tr("Format:"));
                        for (f_i, fname) in Self::EXPORT_FORMATS.iter().enumerate() {
                            let label = crate::i18n::t(fname.label());
                            if ui.selectable_label(self.render_format_idx == f_i, label).clicked() {
                                self.render_format_idx = f_i;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(self.tr("Samplingsfrekvens:"));
                        let rates = ["44.1 kHz (CD Standard)", "48.0 kHz (Film & Video)", "96.0 kHz (Hi-Res Studio)"];
                        for (r_i, rname) in rates.iter().enumerate() {
                            if ui.selectable_label(self.render_sample_rate_idx == r_i, *rname).clicked() {
                                self.render_sample_rate_idx = r_i;
                            }
                        }
                    });

                    // Dither (Fas 6.5): gäller 16-bitars WAV, där kvantiseringen
                    // annars lägger felet som distorsion i stället för brus.
                    // Etiketterna läses först: `&mut self.fält` och `self.tr(...)` får
                    // inte samsas i samma anrop.
                    let dither_label = self.tr("Dither (TPDF, 16-bitars WAV)");
                    let shaping_label = self.tr("Noise shaping");
                    let dither_hint = crate::i18n::t("Gör kvantiseringsfelet okorrelerat med materialet: jämnt brusgolv i stället för distorsion på svaga partier.");
                    let shaping_hint = crate::i18n::t("Flyttar bruset uppåt i frekvens, där örat hör det sämre. Lägger mer energi i diskanten.");
                    let dither_on = self.export_dither;
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.export_dither, dither_label)
                            .on_hover_text(dither_hint);
                        ui.add_enabled_ui(dither_on, |ui| {
                            ui.checkbox(&mut self.export_noise_shaping, shaping_label)
                                .on_hover_text(shaping_hint);
                        });
                    });
                });

                ui.add_space(6.0);

                // 3. Filnamn & mapp
                ui.group(|ui| {
                    ui.label(egui::RichText::new(self.tr("3. FILNAMN & MAPPA")).strong().size(11.5).color(Theme::FL_YELLOW));
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(self.tr("Låt-/filnamn:"));
                        ui.text_edit_singleline(&mut self.export_base_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tr("Mapp:"));
                        ui.add(egui::TextEdit::singleline(&mut self.export_folder).desired_width(400.0));
                    });
                    let fmt = Self::EXPORT_FORMATS[self.render_format_idx.min(Self::EXPORT_FORMATS.len() - 1)];
                    ui.label(egui::RichText::new(format!("{} {}_{:.0}bpm.{}", self.tr("Exempel:"), self.export_base_name, self.bpm, fmt.ext())).size(10.0).color(Theme::FL_GREEN));
                });

                ui.add_space(6.0);

                // 4. Ren metadata
                ui.group(|ui| {
                    ui.label(egui::RichText::new(self.tr("4. METADATA (endast Sonix – ingen AI-info)")).strong().size(11.5).color(Theme::FL_PURPLE));
                    ui.separator();
                    let l_title = self.tr("Titel:");
                    let l_artist = self.tr("Artist:");
                    let l_album = self.tr("Album:");
                    let l_genre = self.tr("Genre:");
                    let l_year = self.tr("År:");
                    let l_comment = self.tr("Kommentar:");
                    let meta = &mut self.export_metadata;
                    ui.horizontal(|ui| {
                        ui.label(l_title);
                        ui.add(egui::TextEdit::singleline(&mut meta.title).desired_width(200.0));
                        ui.label(l_artist);
                        ui.add(egui::TextEdit::singleline(&mut meta.artist).desired_width(160.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(l_album);
                        ui.add(egui::TextEdit::singleline(&mut meta.album).desired_width(200.0));
                        ui.label(l_genre);
                        ui.add(egui::TextEdit::singleline(&mut meta.genre).desired_width(160.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(l_year);
                        ui.add(egui::TextEdit::singleline(&mut meta.year).desired_width(80.0));
                        ui.label(l_comment);
                        ui.add(egui::TextEdit::singleline(&mut meta.comment).desired_width(340.0));
                    });
                    ui.label(egui::RichText::new(self.tr("Software-markören sätts alltid till Sonix Studio. Fält lämnas tomma om du vill utelämna dem.")).size(9.5).color(Theme::TEXT_MUTED));
                });

                ui.add_space(6.0);

                // 5. Export preset & loudness normalization (EBU R128)
                ui.group(|ui| {
                    ui.label(egui::RichText::new(self.tr("5. EXPORT-PRESET & LOUDNESS (EBU R128)")).strong().size(11.5).color(Theme::FL_GREEN));
                    ui.separator();

                    let presets: [(&str, usize, usize, usize); 5] = [
                        ("Streaming (WAV 24/48, −14 LUFS)", 1, 1, 1),
                        ("Apple Music (WAV 24/48, −16 LUFS)", 1, 1, 2),
                        ("Broadcast EBU R128 (WAV 24/48, −23 LUFS)", 1, 1, 3),
                        ("Klubb/Loud (WAV 24/44.1, −9 LUFS)", 1, 0, 4),
                        ("FLAC Master (24-bit, −14 LUFS)", 3, 1, 1),
                    ];
                    ui.horizontal_wrapped(|ui| {
                        ui.label(self.tr("Preset:"));
                        if ui.selectable_label(self.export_preset_idx == 0, self.tr("Anpassad")).clicked() {
                            self.export_preset_idx = 0;
                        }
                        for (i, (label, f, s, l)) in presets.iter().enumerate() {
                            if ui.selectable_label(self.export_preset_idx == i + 1, self.tr(label)).clicked() {
                                self.export_preset_idx = i + 1;
                                self.render_format_idx = *f;
                                self.render_sample_rate_idx = *s;
                                self.export_loudness_idx = *l;
                            }
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label(self.tr("Loudness-normalisering:"));
                        for (i, p) in crate::audio::LoudnessPreset::ALL.iter().enumerate() {
                            if ui.selectable_label(self.export_loudness_idx == i, self.tr(p.label())).clicked() {
                                self.export_loudness_idx = i;
                                self.export_preset_idx = 0;
                            }
                        }
                    });
                    let active = crate::audio::LoudnessPreset::ALL[self.export_loudness_idx.min(crate::audio::LoudnessPreset::ALL.len() - 1)];
                    if active.enabled() {
                        ui.label(egui::RichText::new(crate::tstatus!("Mäts med K-viktning och grindas (BS.1770), normaliseras till {} LUFS med tak på {} dBTP (äkta peak, 4× oversampling). Gäller master-mixen; stems lämnas orörda för extern mixning.", active.target_lufs(), active.ceiling_dbtp())).size(9.5).color(Theme::FL_GREEN));
                    } else {
                        ui.label(egui::RichText::new(self.tr("Ingen normalisering – exporten sker med exakt den nivå du hör.")).size(9.5).color(Theme::TEXT_MUTED));
                    }
                });

                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.tr("Status:")).strong());
                        ui.label(egui::RichText::new(&self.render_queue_status).color(Theme::FL_CYAN));
                    });
                    if self.is_rendering {
                        ui.add(egui::ProgressBar::new(self.render_progress).show_percentage());
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("🚀 STARTA EXPORT")).strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 90))).clicked() {
                        self.execute_batch_export();
                    }
                    if ui.button(self.tr("Stäng")).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_render_queue_modal = false;
        }
    }

    pub fn execute_batch_export(&mut self) {
        self.sync_active_pattern_from_ui();
        self.is_rendering = true;
        self.render_progress = 0.0;

        let fmt_idx = self.render_format_idx.min(Self::EXPORT_FORMATS.len() - 1);
        let fmt = Self::EXPORT_FORMATS[fmt_idx];
        let sample_rate = self.export_sample_rate();
        let folder = if self.export_folder.trim().is_empty() {
            crate::paths::paths().exports_dir().to_string_lossy().to_string()
        } else {
            self.export_folder.trim().to_string()
        };

        let base = {
            let raw = if self.export_base_name.trim().is_empty() {
                self.project_name.clone()
            } else {
                self.export_base_name.clone()
            };
            let safe = crate::audio::sanitize_filename(&raw);
            if safe.is_empty() { crate::i18n::t("Min_Låt").to_string() } else { safe }
        };

        let mut meta = self.export_metadata.clone();
        if meta.title.trim().is_empty() {
            meta.title = base.clone();
        }

        let dry = self.render_scope_idx == 1;
        let stem_mode = self.render_scope_idx == 1 || self.render_scope_idx == 2;

        std::fs::create_dir_all(&folder).map_err(|e| {
            self.is_rendering = false;
            self.render_queue_status = crate::tstatus!("✘ Kan inte skapa mapp: {}", e);
            self.status_message = crate::tstatus!("✘ Exportfel: {}", e);
        }).ok();

        let mut written_files: Vec<String> = Vec::new();
        let mut last_error: Option<String> = None;
        let mut normalize_note: Option<String> = None;
        let loudness = crate::audio::LoudnessPreset::ALL
            [self.export_loudness_idx.min(crate::audio::LoudnessPreset::ALL.len() - 1)];

        if stem_mode {
            let track_count = self.playlist_tracks.len();
            for t_idx in 0..track_count {
                let track_name = self.playlist_tracks[t_idx].name.replace([' ', '/', '\\'], "_");
                let spec = self.build_render_spec(Some(t_idx), sample_rate);
                let buf = match self.render_buffer(&spec, dry) {
                    Ok(b) => b,
                    Err(e) => { last_error = Some(e); break; }
                };
                if Self::peak_of(&buf) < 1.0e-6 {
                    continue; // tomt spår – hoppa över
                }
                let file_stem = format!("{}_Trk{:02}_{}", base, t_idx + 1, track_name);
                let path = format!("{}/{}.{}", folder, file_stem, fmt.ext());
                let mut stem_meta = meta.clone();
                if stem_meta.title.trim().is_empty() || stem_meta.title == base {
                    stem_meta.title = format!("{} ({})", base, track_name);
                }
                let dither = self.export_dither_settings();
                match crate::audio::write_export_with(&path, fmt, &buf, sample_rate, &stem_meta, dither) {
                    Ok(_) => written_files.push(path),
                    Err(e) => { last_error = Some(e); break; }
                }
            }
        } else {
            let spec = self.build_render_spec(None, sample_rate);
            let mut buf = match self.render_buffer(&spec, false) {
                Ok(b) => b,
                Err(e) => { last_error = Some(e); Vec::new() }
            };
            if last_error.is_none() {
                if let Some(measured) =
                    crate::audio::normalize_to_preset(&mut buf, sample_rate, loudness)
                {
                    normalize_note = Some(crate::tstatus!(
                        "🔊 Normaliserad till {} LUFS (mätt {} LUFS, tak {} dBTP)",
                        loudness.target_lufs(),
                        (measured * 10.0).round() / 10.0,
                        loudness.ceiling_dbtp()
                    ));
                }
                let path = format!("{}/{}_{:.0}bpm.{}", folder, base, self.bpm, fmt.ext());
                let dither = self.export_dither_settings();
                match crate::audio::write_export_with(&path, fmt, &buf, sample_rate, &meta, dither) {
                    Ok(_) => written_files.push(path),
                    Err(e) => last_error = Some(e),
                }
            }
        }

        self.is_rendering = false;
        self.render_progress = 1.0;
        match last_error {
            Some(e) => {
                self.render_queue_status = crate::tstatus!("✘ Exporten avbröts: {}", e);
                self.status_message = crate::tstatus!("✘ Exportfel: {}", e);
            }
            None if written_files.is_empty() => {
                self.render_queue_status = crate::i18n::t("⚠ Inga ljudfiler skapades (alla spår var tomma?).").to_string();
                self.status_message = crate::i18n::t("⚠ Inga ljud skapades.").to_string();
            }
            None => {
                self.render_queue_status = crate::tstatus!("✅ {} fil(er) exporterade till {}", written_files.len(), folder);
                if let Some(note) = &normalize_note {
                    self.render_queue_status = crate::tstatus!("✅ {} fil(er) → {} · {}", written_files.len(), folder, note);
                }
                self.status_message = crate::tstatus!("✔ Klar! Exporterade {} fil(er) utan AI-metadata → {}", written_files.len(), folder);
            }
        }
    }

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

    pub fn import_suno_zip(&mut self, zip_path: &str) {
        self.start_suno_zip_import(zip_path);
    }

    pub fn start_suno_zip_import(&mut self, zip_path: &str) {
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

            Self::background_decode_stems(&dest_dir, &title, bpm, progress);
        });
    }

    pub fn import_suno_stems_from_folder(&mut self, folder_path: &str, project_title: &str, bpm: f32) {
        self.start_suno_folder_import(folder_path, project_title, bpm);
    }

    pub fn start_suno_folder_import(&mut self, folder_path: &str, project_title: &str, bpm: f32) {
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
            Self::background_decode_stems(&folder, &title, bpm, progress);
        });
    }

    fn background_decode_stems(
        folder_path: &str,
        project_title: &str,
        bpm: f32,
        progress: std::sync::Arc<std::sync::Mutex<StemImportProgress>>,
    ) {
        let mut stem_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ext_lower == "wav" || ext_lower == "mp3" || ext_lower == "flac" || ext_lower == "ogg" {
                        stem_files.push(path);
                    }
                }
            }
        }

        if stem_files.is_empty() {
            if let Ok(mut p) = progress.lock() {
                p.is_importing = false;
                p.error_message = Some(crate::tstatus!("Inga ljudfiler hittades i mappen: {}", folder_path));
            }
            return;
        }

        stem_files.sort();
        let total_files = stem_files.len();

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

            if let Ok((l, r, sr)) = crate::audio::load_audio_pcm(&path_str) {
                    sample_rate = sr;
                    let total_secs = l.len() as f32 / sr.max(1) as f32;
                    file_bars =
                        (tempo.bars_for_secs_at(0.0, total_secs as f64) as f32).max(1.0);
                    max_stem_bars = max_stem_bars.max(file_bars);

                    // Compute waveform peaks in-memory
                    let num_points = ((file_bars * 16.0) as usize).clamp(240, 4800);
                    let chunk_size = (l.len() / num_points).max(1);
                    wave_env.reserve(num_points);
                    for p_i in 0..num_points {
                        let s_start = p_i * chunk_size;
                        let s_end = (s_start + chunk_size).min(l.len());
                        let mut peak: f32 = 0.04;
                        let step = ((s_end - s_start) / 64).max(1);
                        let mut s = s_start;
                        while s < s_end {
                            let v = l[s].abs();
                            if v > peak {
                                peak = v;
                            }
                            s += step;
                        }
                        wave_env.push(peak.clamp(0.04, 0.98));
                    }

                    pcm_l_opt = Some(std::sync::Arc::new(l));
                    pcm_r_opt = Some(std::sync::Arc::new(r));
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

    pub fn apply_imported_stems(&mut self, res: StemImportResult) {
        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        let mut new_tracks = Vec::new();

        for (idx, dt) in res.tracks.into_iter().enumerate() {
            if let (Some(arc_l), Some(arc_r)) = (dt.pcm_left, dt.pcm_right) {
                let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                    track_index: idx,
                    left: arc_l,
                    right: arc_r,
                    sample_rate: dt.sample_rate as f32,
                    volume: 0.9,
                    pan: 0.0,
                    start_time_secs: 0.0,
                });
            }

            let initial_region = AudioRegion {
                id: idx + 1,
                name: dt.clean_name.clone(),
                start_bar: 0.0,
                length_bars: dt.file_bars,
                sample_offset_sec: 0.0,
                source_path: Some(dt.path_str),
                waveform_peaks: dt.wave_env.clone(),
                volume: 1.0,
                fade_in_bars: 0.0,
                fade_out_bars: 0.0,
                muted: false,
                is_reverse: false,
                color: dt.color,
                loop_length_bars: 0.0,
            };

            let mut track = PlaylistTrack::new(format!("{} {}", dt.icon, dt.clean_name), dt.icon, dt.kind, dt.color);
            track.volume = 0.9;
            track.is_rec_armed = false;
            track.regions = vec![initial_region];
            track.custom_clip_name = Some(dt.clean_name);

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

    fn render_suno_stem_import_modal(&mut self, ctx: &egui::Context) {
        if !self.show_suno_import_modal {
            return;
        }

        let mut close = false;
        let mut zip_to_import: Option<String> = None;
        let mut folder_to_import: Option<(String, String, f32)> = None;

        egui::Window::new(crate::i18n::t("📦 Import Stems / Multi-Track Importer"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(580.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎵")).size(24.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Multi-Track Stämmor & Ljudspår (Stems)")).strong().size(13.0).color(Theme::FL_ORANGE));
                            ui.label(egui::RichText::new(crate::i18n::t("Importera nedladdade ZIP-paket eller mappar med stämmor (Suno, FL Studio, Ableton, Logic m.fl.). Sonix läser ut äkta 48kHz WAV-vågformer, detekterar tempo (BPM) och mappar spåren i tidslinjen.")).size(10.5).color(Theme::TEXT_MUTED));
                        });
                    });
                });

                ui.add_space(8.0);

                // Quick Rescan Bar
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("UPPTÄCKTA STEMPAKET:")).strong().size(11.0).color(Theme::FL_CYAN));
                    if ui.button(crate::i18n::t("🔄 Skanna ~/Music & ~/Downloads")).clicked() {
                        self.scan_for_suno_stems();
                    }
                });

                ui.add_space(4.0);

                // List of detected ZIPs
                ui.group(|ui| {
                    if self.detected_suno_zips.is_empty() {
                        ui.label(egui::RichText::new(crate::i18n::t("Inga .zip-stempaket hittades i ~/Music eller ~/Downloads. Klicka på knappen ovan för att skanna, eller ange sökväg manuellt nedan.")).size(10.5).color(Theme::TEXT_MUTED));
                    } else {
                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            for zip_p in &self.detected_suno_zips {
                                let (title, bpm) = Self::parse_suno_zip_info(zip_p);
                                let is_wav = zip_p.contains("(1).zip") || zip_p.to_lowercase().contains("wav");
                                let badge = if is_wav { "🔊 48kHz WAV Stems" } else { "🎧 MP3 Stems" };

                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(format!("🎵 {}", title)).strong().size(11.5).color(Color32::WHITE));
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(format!("Tempo: {:.1} BPM", bpm)).size(10.0).color(Theme::FL_GREEN));
                                                ui.label(egui::RichText::new(badge).size(10.0).color(Theme::FL_CYAN));
                                            });
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Importera Alla Stämmor")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                                                zip_to_import = Some(zip_p.clone());
                                            }
                                        });
                                    });
                                });
                                ui.add_space(2.0);
                            }
                        });
                    }
                });

                ui.add_space(8.0);

                // Manual Path Input & File Browser
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("VÄLJ ZIP-FIL ELLER MAPP")).strong().size(11.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📁 Välj ZIP-fil från datorn...")).strong().size(11.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked() {
                            self.spawn_async_file_picker("Stempaket", &["zip"], crate::i18n::t("Välj ZIP Stempaket"));
                        }

                        ui.separator();
                        ui.label(crate::i18n::t("eller ange sökväg:"));
                        ui.text_edit_singleline(&mut self.custom_stem_path_input);
                        if ui.add(egui::Button::new(crate::i18n::t("Ladda")).fill(Color32::from_rgb(50, 60, 80))).clicked() {
                            let path = self.custom_stem_path_input.trim().to_string();
                            if path.to_lowercase().ends_with(".zip") {
                                zip_to_import = Some(path);
                            } else {
                                let (title, bpm) = Self::parse_suno_zip_info(&path);
                                folder_to_import = Some((path, title, bpm));
                            }
                        }
                    });

                    ui.add_space(3.0);
                    ui.label(egui::RichText::new(crate::i18n::t("💡 Tips: Du kan även dra & släppa (Drag & Drop) .zip-filer direkt in i fönstret!")).size(9.5).color(Theme::FL_GREEN));
                });

                ui.add_space(8.0);

                // Stems Layout Preview Info
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("SPÅRSTRUKTUR I SONIX:")).strong().size(10.5).color(Theme::TEXT_MUTED));
                    ui.horizontal_wrapped(|ui| {
                        let stems_info = [
                            ("🎙 0 Lead Vocals", "Vocal Bus"),
                            ("🗣 1 Backing Vocals", "Vocal Bus"),
                            ("🥁 2 Drums", "Drum Bus"),
                            ("🎸 3 Bass", "Drum Bus / Sidechain"),
                            ("🎸 4 Guitar", "Synth Bus"),
                            ("🪘 5 Percussion", "Drum Bus"),
                            ("🎹 6 Synth", "Synth Bus"),
                            ("✨ 7 Other", "FX Send"),
                        ];
                        for (st, bus) in stems_info.iter() {
                            ui.label(egui::RichText::new(format!("• {} ➔ [{}]  ", st, bus)).size(10.0).color(Theme::TEXT_BRIGHT));
                        }
                    });
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new(crate::i18n::t("Stäng")).size(11.0)).clicked() {
                        close = true;
                    }
                });
            });

        if let Some(zip_p) = zip_to_import {
            self.start_suno_zip_import(&zip_p);
            close = true;
        }

        if let Some((folder, title, bpm)) = folder_to_import {
            self.start_suno_folder_import(&folder, &title, bpm);
            close = true;
        }

        if close {
            self.show_suno_import_modal = false;
        }
    }

    fn render_stem_import_progress_modal(&mut self, ctx: &egui::Context) {
        let mut completed_payload = None;
        let mut progress_info = None;

        if let Ok(mut p) = self.stem_import_progress.try_lock() {
            if p.is_importing {
                ctx.request_repaint();
                progress_info = Some((
                    p.stage.clone(),
                    p.current_file.clone(),
                    p.current_idx,
                    p.total_files,
                    p.progress_ratio,
                ));
            }
            if let Some(res) = p.completed_payload.take() {
                completed_payload = Some(res);
            }
            if let Some(err) = p.error_message.take() {
                self.status_message = crate::tstatus!("❌ Fel vid stämimport: {}", err);
            }
        }

        if let Some((stage, file, idx, total, ratio)) = progress_info {
            egui::Window::new(crate::i18n::t("⏳ Importerar stämspår..."))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(460.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Läser in och avkodar stämmor (Stems)")).strong().size(13.0).color(Theme::FL_ORANGE));
                                ui.label(egui::RichText::new(&stage).size(11.0).color(Color32::WHITE));
                            });
                        });
                    });

                    ui.add_space(8.0);
                    ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if !file.is_empty() {
                            ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}: {}", idx, total, file)).size(10.0).color(Theme::TEXT_MUTED));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(11.0).color(Theme::FL_CYAN));
                        });
                    });
                    ui.add_space(4.0);
                });
        }

        if let Some(res) = completed_payload {
            self.apply_imported_stems(res);
        }
    }

    fn render_project_load_progress_modal(&mut self, ctx: &egui::Context) {
        let mut completed_payload = None;
        let mut progress_info = None;

        if let Ok(mut p) = self.project_load_progress.try_lock() {
            if p.is_loading {
                ctx.request_repaint();
                progress_info = Some((
                    p.project_name.clone(),
                    p.stage.clone(),
                    p.current_track.clone(),
                    p.current_idx,
                    p.total_tracks,
                    p.progress_ratio,
                ));
            }
            if let Some(res) = p.completed_payload.take() {
                p.is_loading = false;
                completed_payload = Some(res);
            }
            if let Some(err) = p.error_message.take() {
                p.is_loading = false;
                self.status_message = crate::tstatus!("❌ Fel vid projektladdning: {}", err);
            }
        }

        if let Some((proj, stage, track, idx, total, ratio)) = progress_info {
            egui::Window::new(crate::i18n::t("⏳ Öppnar projekt..."))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(480.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(crate::tstatus!("Öppnar: {}", proj)).strong().size(13.0).color(Theme::FL_CYAN));
                                ui.label(egui::RichText::new(&stage).size(11.0).color(Color32::WHITE));
                            });
                        });
                    });

                    ui.add_space(8.0);
                    ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if !track.is_empty() {
                            ui.label(egui::RichText::new(crate::tstatus!("Läser spår {}/{}: {}", idx, total, track)).size(10.5).color(Theme::TEXT_MUTED));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(11.0).color(Theme::FL_ORANGE));
                        });
                    });
                    ui.add_space(4.0);
                });
        }

        if let Some(res) = completed_payload {
            self.apply_loaded_project_payload(res);
        }
    }

    /// Kraschåterställning (Fas 6.1): erbjuder de autosaves som är nyare än sin
    /// manuella projektfil. Dialogen visas bara när det finns något att rädda.
    fn render_recovery_modal(&mut self, ctx: &egui::Context) {
        if !self.show_recovery_modal {
            return;
        }

        // Snapshot av listan: fönster-closuren får inte låna self samtidigt som
        // knapptryckningar vill ändra self.
        let rows: Vec<(String, String, u64)> = self
            .recovery_candidates
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    c.entry.path.to_string_lossy().to_string(),
                    c.entry.stamp,
                )
            })
            .collect();
        let now = crate::autosave::now_stamp();

        let mut restore: Option<usize> = None;
        let mut dismiss = false;
        let mut open = self.show_recovery_modal;
        egui::Window::new(crate::i18n::t("🛟 Osparat arbete hittades"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(560.0, 280.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Sonix avslutades innan projektet sparades. Dessa automatiska kopior är nyare än filen på disk:",
                    ))
                    .size(11.5)
                    .color(Theme::TEXT_BRIGHT),
                );
                ui.add_space(8.0);

                for (i, (name, path, stamp)) in rows.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "🛟 {} — {}",
                                name,
                                crate::autosave::relative_age(*stamp, now)
                            ))
                            .size(11.5)
                            .color(Theme::FL_CYAN),
                        );
                        if ui
                            .button(crate::i18n::t("  Återställ  "))
                            .on_hover_text(path)
                            .clicked()
                        {
                            restore = Some(i);
                        }
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Autosparningarna ligger i ~/.local/state/sonix/autosave/ (5 senaste per projekt). En återställd kopia pensioneras dit utan att raderas.",
                    ))
                    .size(10.0)
                    .color(Theme::TEXT_MUTED),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::i18n::t("  Fortsätt utan att återställa  "))
                                    .size(11.5),
                            )
                            .fill(Theme::FL_ORANGE),
                        )
                        .clicked()
                    {
                        // Stäng bara: autosparningarna ligger kvar och kan
                        // öppnas manuellt — inget raderas åt användaren.
                        dismiss = true;
                    }
                });
            });

        if let Some(i) = restore {
            self.restore_autosave(i);
        } else if !open || dismiss {
            self.show_recovery_modal = false;
        }
    }

    fn render_about_modal(&mut self, ctx: &egui::Context) {
        if !self.show_about_modal {
            return;
        }

        let mut close = false;
        let mut open = self.show_about_modal;
        egui::Window::new(crate::i18n::t("ℹ Om Sonix Studio"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(460.0, 420.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(crate::i18n::t("🍊")).size(64.0));
                    ui.heading(egui::RichText::new(crate::i18n::t("SONIX STUDIO")).strong().size(22.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new(crate::i18n::t("Professionell Digital Audio Workstation & AI Musikstudio för Linux")).size(12.0).color(Theme::FL_CYAN));
                    ui.label(egui::RichText::new(crate::tstatus!("Version {} (PipeWire / ALSA / JACK Audio Engine)", env!("CARGO_PKG_VERSION"))).size(10.5).color(Theme::TEXT_MUTED));

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new(crate::i18n::t("🚀 Huvudfunktioner i Sonix Studio:")).strong().size(12.0).color(Theme::TEXT_BRIGHT));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::i18n::t("• Realtids Multi-Track Audio Streaming & Mixmotor i Rust")).size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new(crate::i18n::t("• Fullt stöd för Suno AI Stems med linjär vågformsredigering")).size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new(crate::i18n::t("• 16-Stegs Sonix Channel Rack, Piano Roll & Touch Piano")).size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new(crate::i18n::t("• Klippverktyg (Slice ✂), Fading & Dynamisk Gain-justering")).size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new(crate::i18n::t("• 64-bit SIMD Audio DSP med Zero-Latency Process Thread")).size(11.0).color(Theme::TEXT_MUTED));

                    ui.add_space(16.0);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("  Stäng  ")).strong().size(12.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
                        close = true;
                    }
                    ui.add_space(8.0);
                });
            });

        if close || !open {
            self.show_about_modal = false;
        }
    }

    fn render_project_manager_modal(&mut self, ctx: &egui::Context) {
        if !self.show_project_manager_modal {
            return;
        }

        let mut close = false;
        let mut create_new = false;
        let mut load_demo = false;
        let mut show_suno = false;
        let mut save_name: Option<String> = None;
        let mut load_file: Option<String> = None;
        let mut open = self.show_project_manager_modal;

        egui::Window::new(crate::i18n::t("📁 Filhanterare & Projekt"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(580.0, 440.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(egui::RichText::new(crate::i18n::t("📄 Nytt tomt projekt")).strong().color(Theme::FL_CYAN)).clicked() {
                        create_new = true;
                        close = true;
                    }
                    if ui.button(egui::RichText::new(crate::i18n::t("⚡ Ladda Demo-projekt")).strong().color(Theme::FL_GREEN)).clicked() {
                        load_demo = true;
                        close = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📂 Välj .sonix från datorn...")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked() {
                        self.spawn_async_file_picker(
                            "Sonix-projekt",
                            &["sonix"],
                            crate::i18n::t("Välj Sonix Projektfil (.sonix)"),
                        );
                    }
                    if ui.button(egui::RichText::new(crate::i18n::t("🎼 Importera Suno ZIP...")).strong().color(Theme::FL_ORANGE)).clicked() {
                        show_suno = true;
                        close = true;
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Öppna från sökväg
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📂 ÖPPNA PROJEKT FRÅN SÖKVÄG")).strong().color(Theme::FL_CYAN));
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Sökväg:"));
                        ui.text_edit_singleline(&mut self.custom_project_path_input);
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                            let p = self.custom_project_path_input.trim().to_string();
                            if !p.is_empty() {
                                load_file = Some(p);
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // Spara som
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("💾 SPARA PROJEKT")).strong().color(Theme::FL_ORANGE));
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Projektnamn:"));
                        ui.text_edit_singleline(&mut self.new_project_name_input);
                        if ui.button(crate::i18n::t("Spara till disk")).clicked() {
                            save_name = Some(self.new_project_name_input.clone());
                        }
                    });
                });

                ui.add_space(8.0);
                let projects_dir = crate::paths::paths().projects_dir();
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        crate::i18n::t("📂 SPARADE PROJEKT:"),
                        projects_dir.display()
                    ))
                    .strong()
                    .color(Theme::FL_CYAN),
                );
                let mut saved_files = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("sonix")
                            && let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                saved_files.push((stem.to_string(), path.to_string_lossy().to_string()));
                            }
                    }
                }

                if saved_files.is_empty() {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Inga sparade .sonix-projekt hittades ännu. Spara ditt nuvarande projekt ovan eller ladda ett demo-projekt!")).italics().color(Theme::TEXT_MUTED));
                    });
                } else {
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for (stem, full_path) in saved_files {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("🎵 {}", stem)).strong().color(Theme::TEXT_BRIGHT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna Projekt")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                                            load_file = Some(full_path.clone());
                                        }
                                    });
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });
                }

                ui.add_space(10.0);
                if ui.button(self.tr("Stäng")).clicked() {
                    close = true;
                }
            });

        if create_new {
            self.new_empty_project();
        }
        if load_demo {
            self.load_demo_project();
        }
        if show_suno {
            self.show_suno_import_modal = true;
        }
        if let Some(name) = save_name {
            self.save_project(&name);
        }
        if let Some(path) = load_file {
            self.load_project_file(&path);
            close = true;
        }

        if close || !open {
            self.show_project_manager_modal = false;
        }
    }

    fn render_ai_settings_modal(&mut self, ctx: &egui::Context) {
        if !self.show_ai_settings_modal {
            return;
        }

        let mut close = false;
        let mut open = self.show_ai_settings_modal;
        egui::Window::new(crate::i18n::t("🤖 AI-inställningar"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(560.0, 480.0))
            .show(ctx, |ui| {
                ui.label(crate::i18n::t("Samma konfiguration som i AI Music Studio – sparas till ~/.config/sonix/ai.json."));
                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("💬 Textgenerering (AI Co-Producer)")).strong().color(Theme::FL_GREEN));
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Provider:"));
                        egui::ComboBox::from_id_salt("legacy_ai_provider_combo")
                            .selected_text(self.ai_assistant.config.provider.label())
                            .show_ui(ui, |ui| {
                                for p in crate::audio::ai_client::AiProvider::ALL {
                                    ui.selectable_value(&mut self.ai_assistant.config.provider, p, p.label());
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Bas-URL:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.base_url).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Modell:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.model).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("API-nyckel:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.api_key).password(true).desired_width(340.0));
                    });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎧 AI-ljudgenerering (Stable Audio m.fl.)")).strong().color(Theme::FL_CYAN));
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Ljud-provider:"));
                        egui::ComboBox::from_id_salt("legacy_ai_audio_provider_combo")
                            .selected_text(self.ai_assistant.config.audio_provider.label())
                            .show_ui(ui, |ui| {
                                for p in crate::audio::ai_client::AudioProvider::ALL {
                                    ui.selectable_value(&mut self.ai_assistant.config.audio_provider, p, p.label());
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Bas-URL:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_base_url).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Modell:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_model).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                    });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("API-nyckel:"));
                        ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_api_key).password(true).desired_width(340.0));
                    });
                });

                ui.add_space(6.0);
                let ready = self.ai_assistant.config.is_ready();
                ui.label(
                    egui::RichText::new(if ready {
                        crate::i18n::t("Redo: använder AI-API")
                    } else {
                        crate::i18n::t("Offline: använder lokal regelbaserad motor")
                    })
                    .size(10.5)
                    .color(if ready { Theme::FL_GREEN } else { Theme::TEXT_MUTED }),
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara inställningar")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                        match self.ai_assistant.config.save() {
                            Ok(path) => self.status_message = crate::tstatus!("✔ Sparade AI-inställningar till {}", path.display()),
                            Err(e) => self.status_message = crate::tstatus!("⚠ Kunde inte spara AI-inställningar: {}", e),
                        }
                        close = true;
                    }
                    if ui.button(crate::i18n::t("Avbryt")).clicked() {
                        close = true;
                    }
                });
            });

        if close || !open {
            self.show_ai_settings_modal = false;
        }
    }

    fn render_audio_settings_modal(&mut self, ctx: &egui::Context) {
        if !self.show_audio_settings_modal {
            return;
        }

        let mut close = false;
        let mut open = self.show_audio_settings_modal;
        egui::Window::new(crate::i18n::t("🎛 Ljud- & MIDI-inställningar"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(500.0, 380.0))
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor & Drivrutiner")).strong().color(Theme::FL_CYAN));
                    ui.label(crate::tstatus!("Värd: {} · Enhet: {}", self.engine.host_name, self.engine.device_name));
                    let buffer_txt = self.engine.buffer_frames
                        .map(|f| format!("{f} frames"))
                        .unwrap_or_else(|| crate::i18n::t("enhetens standard").to_string());
                    ui.label(egui::RichText::new(crate::tstatus!(
                        "Aktiv ström: {} Hz · {} · {} kanaler",
                        self.engine.sample_rate,
                        buffer_txt,
                        self.engine.channels
                    )).size(10.5).color(Theme::TEXT_MUTED));

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(self.tr("Samplingsfrekvens:"));
                        let rates = ["44.1 kHz", "48.0 kHz", "96.0 kHz"];
                        for (i, rate) in rates.iter().enumerate() {
                            if ui.selectable_label(self.audio_sample_rate_idx == i, *rate).clicked() {
                                self.audio_sample_rate_idx = i;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Bufferstorlek:"));
                        let buffers = ["128 (2.9ms)", "256 (5.8ms)", "512 (11.6ms)", "1024 (23.2ms)"];
                        for (i, buf) in buffers.iter().enumerate() {
                            if ui.selectable_label(self.audio_buffer_size_idx == i, *buf).clicked() {
                                self.audio_buffer_size_idx = i;
                            }
                        }
                    });

                    ui.add_space(6.0);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Tillämpa på ljudströmmen")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                        let rates = [44100_u32, 48000, 96000];
                        let buffers = [128_u32, 256, 512, 1024];
                        let requested_rate = rates[self.audio_sample_rate_idx.min(rates.len() - 1)];
                        let requested_buffer = buffers[self.audio_buffer_size_idx.min(buffers.len() - 1)];
                        match self.engine.reconfigure(Some(requested_rate), Some(requested_buffer)) {
                            Ok(()) => {
                                let actual_rate = self.engine.sample_rate;
                                let actual_buffer = self.engine.buffer_frames;
                                let settings = AudioSettings {
                                    sample_rate: Some(actual_rate),
                                    buffer_frames: actual_buffer,
                                };
                                let save_note = match settings.save() {
                                    Ok(_) => String::new(),
                                    Err(e) => crate::tstatus!(" (kunde inte spara: {})", e),
                                };
                                self.audio_sample_rate_idx = match actual_rate { 44100 => 0, 96000 => 2, _ => 1 };
                                self.audio_buffer_size_idx = match actual_buffer { Some(128) => 0, Some(512) => 2, Some(1024) => 3, _ => 1 };
                                self.resync_engine_after_reconfigure();
                                let buf_txt = actual_buffer.map(|f| format!("{f} frames")).unwrap_or_else(|| crate::i18n::t("enhetens standard").to_string());
                                self.status_message = crate::tstatus!(
                                    "🔁 Ljudströmmen omstartad: {} Hz · {}{}",
                                    actual_rate, buf_txt, save_note
                                );
                                close = true;
                            }
                            Err(e) => {
                                self.status_message = crate::tstatus!("⚠ Kunde inte byta ljudkonfiguration: {}", e);
                            }
                        }
                    }
                });

                ui.add_space(10.0);
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    close = true;
                }
            });

        if close || !open {
            self.show_audio_settings_modal = false;
        }
    }

    fn render_mic_settings_modal(&mut self, ctx: &egui::Context) {
        if !self.show_mic_settings_modal {
            return;
        }

        let mut close = false;
        let mut open = self.show_mic_settings_modal;
        egui::Window::new(crate::i18n::t("🎙 Mikrofonjustering & Enhetsinställningar (Microphone Panel)"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(560.0, 480.0))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Group 1: Device Selection & Host Audio Stream
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Ljudenhet & Mikrofonkälla")).strong().size(13.0).color(Theme::FL_CYAN));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(crate::i18n::t("🔄 Uppdatera enheter")).on_hover_text(crate::i18n::t("Sök efter anslutna USB- och hårdvarumikrofoner")).clicked() {
                                    let devs = LiveMicrophoneCapture::list_devices();
                                    self.vocal_studio.mic_settings.available_devices = devs;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let dev_list = self.vocal_studio.mic_settings.available_devices.clone();
                        let mut current_idx = self.vocal_studio.mic_settings.selected_device_idx;

                        egui::ComboBox::from_label(crate::i18n::t("Välj Mikrofon"))
                            .selected_text(dev_list.get(current_idx).cloned().unwrap_or_else(|| crate::i18n::t("Standardmikrofon").to_string()))
                            .width(360.0)
                            .show_ui(ui, |ui| {
                                for (idx, dev_name) in dev_list.iter().enumerate() {
                                    let is_rec_pref = dev_name.to_lowercase().contains("samson") || dev_name.to_lowercase().contains("usb");
                                    let label = if is_rec_pref {
                                        format!("🎤 {} (Rekommenderad)", dev_name)
                                    } else {
                                        format!("🎙 {}", dev_name)
                                    };
                                    if ui.selectable_value(&mut current_idx, idx, label).clicked() {
                                        let switched = self.vocal_studio.select_microphone(idx);
                                        if switched {
                                            self.status_message = crate::tstatus!("✔ Mikrofon ändrad till: {}", dev_name);
                                        }
                                    }
                                }
                            });

                        let active_dev = self.vocal_studio.mic_capture.as_ref().map(|m| m.device_name.as_str()).unwrap_or("Standard");
                        let sr = self.vocal_studio.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
                        ui.label(egui::RichText::new(crate::tstatus!("🟢 Aktiv ström: {} • {} Hz 32-bit Float", active_dev, sr)).size(10.5).color(Theme::FL_GREEN));
                    });

                    ui.add_space(6.0);

                    // Group 2: Real-time VU & Level Meter with Gain
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Ingångsnivå & Förförstärkare (Gain & VU)")).strong().size(13.0).color(Theme::FL_ORANGE));
                        ui.add_space(4.0);

                        let vu = self.vocal_studio.mic_vu_level;
                        let vu_db = if vu > 0.0001 {
                            (20.0 * vu.log10()).max(-48.0)
                        } else {
                            -48.0
                        };

                        ui.horizontal(|ui| {
                            let text_col = if vu > 0.85 {
                                Color32::from_rgb(255, 60, 60)
                            } else if vu > 0.50 {
                                Theme::FL_ORANGE
                            } else {
                                Theme::FL_GREEN
                            };
                            ui.label(egui::RichText::new(crate::tstatus!("Ingångssignal: {:.1} dB", vu_db)).monospace().strong().color(text_col));

                            if vu > 0.95 {
                                ui.label(egui::RichText::new(crate::i18n::t("⚠ CLIP")).strong().color(Color32::from_rgb(255, 40, 40)));
                            }
                        });

                        // Visual LED Bar Meter
                        let (m_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                        ui.painter().rect_filled(m_rect, Rounding::same(3.0), Color32::from_rgb(18, 22, 30));
                        ui.painter().rect_stroke(m_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 68)));

                        let fill_w = (m_rect.width() * vu.clamp(0.0, 1.0)).max(2.0);
                        let bar_color = if vu > 0.85 {
                            Color32::from_rgb(255, 60, 60)
                        } else if vu > 0.50 {
                            Theme::FL_ORANGE
                        } else {
                            Theme::FL_GREEN
                        };
                        ui.painter().rect_filled(Rect::from_min_size(m_rect.min, Vec2::new(fill_w, m_rect.height())), Rounding::same(3.0), bar_color);

                        ui.add_space(4.0);
                        let mut gain = self.vocal_studio.mic_settings.input_gain;
                        let gain_db = 20.0 * gain.log10();
                        let gain_str = crate::tstatus!("Mikrofonförstärkning: {:.2}x ({:+.1} dB)", gain, gain_db);
                        ui.horizontal(|ui| {
                            if ui.add(egui::Slider::new(&mut gain, 0.0..=4.0).text(gain_str)).changed() {
                                self.vocal_studio.set_input_gain(gain);
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Group 3: Noise Gate & Feedback Suppression
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Brusreducering & Anti-rundgång")).strong().size(13.0).color(Theme::FL_YELLOW));
                        ui.add_space(4.0);

                        let mut gate = self.vocal_studio.mic_settings.noise_gate_thresh;
                        let gate_db = if gate > 0.0001 { 20.0 * gate.log10() } else { -60.0 };
                        let gate_open = self.vocal_studio.mic_vu_level >= gate;
                        let gate_str = crate::tstatus!("Bruströskel (Gate): {:.3} ({:.1} dB)", gate, gate_db);

                        ui.horizontal(|ui| {
                            if ui.add(egui::Slider::new(&mut gate, 0.000..=0.100).text(gate_str)).changed() {
                                self.vocal_studio.set_noise_gate(gate);
                            }
                            let led_color = if gate_open { Theme::FL_GREEN } else { Color32::from_rgb(180, 40, 40) };
                            let led_text = if gate_open { crate::i18n::t("🟢 ÖPPEN") } else { crate::i18n::t("🔴 STÄNGD") };
                            ui.label(egui::RichText::new(led_text).strong().color(led_color).size(10.5));
                        });

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.vocal_studio.mic_settings.feedback_reduction, crate::i18n::t("🔇 Feedback Suppression / Anti-rundgång"));
                            let mut low_cut = self.vocal_studio.mic_settings.low_cut_80hz;
                            if ui
                                .checkbox(&mut low_cut, "📉 80Hz Low-Cut (Tar bort muller & bordsvibrationer)")
                                .on_hover_text(crate::i18n::t("Högpass som tar bort rummets lågfrekvens innan den når mastern. Utan den kan rummet börja vibrera (baston) när direktlyssningen är på."))
                                .changed()
                            {
                                self.vocal_studio.set_low_cut(low_cut);
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Group 4: Live DSP Effects (Reverb, De-Esser, Compressor, Direct Monitoring)
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Vokaleffekter i realtid (Live DSP)")).strong().size(13.0).color(Theme::FL_PURPLE));
                        ui.add_space(4.0);

                        let rev_str = format!("Rumsklang: {:.0}%", self.vocal_studio.mic_settings.vocal_reverb * 100.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.vocal_reverb, 0.0..=1.0).text(rev_str));
                        });

                        let ess_str = crate::tstatus!("De-Esser (S-dämpning): {:.0}%", self.vocal_studio.mic_settings.de_esser_amount * 100.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.de_esser_amount, 0.0..=1.0).text(ess_str));
                        });

                        let comp_str = format!("Vokal Kompressor: {:.0}%", self.vocal_studio.mic_settings.compressor_amount * 100.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.compressor_amount, 0.0..=1.0).text(comp_str));
                        });

                        ui.checkbox(&mut self.vocal_studio.mic_settings.direct_monitoring, crate::i18n::t("🎧 Direktlyssning i hörlurar (Zero-Latency Direct Monitoring)"));
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t("OK / Stäng")).clicked() {
                            close = true;
                        }

                        let is_rec = self.vocal_studio.is_recording;
                        let btn_label = if is_rec { "⏹ Avsluta Provpratning" } else { "🎙 Provprata & Testa Inspelning" };
                        let btn_col = if is_rec { Color32::from_rgb(220, 40, 40) } else { Theme::FL_GREEN };
                        if ui.add(egui::Button::new(egui::RichText::new(btn_label).strong().color(Color32::WHITE)).fill(btn_col)).clicked() {
                            if is_rec {
                                match self.vocal_studio.stop_recording() {
                                    Ok(_) => {
                                        self.sync_track_stem_to_engine(4);
                                        self.status_message = crate::i18n::t("✔ Testinspelning klar och sparad i Sångstudion!").to_string();
                                    }
                                    Err(e) => self.status_message = format!("❌ {}", e),
                                }
                            } else {
                                match self.vocal_studio.start_recording() {
                                    Ok(()) => self.status_message = crate::i18n::t("🔴 Provpratar... Säg några ord i mikrofonen!").to_string(),
                                    Err(e) => self.status_message = format!("❌ {}", e),
                                }
                            }
                        }
                    });
                });
            });

        if close || !open {
            self.show_mic_settings_modal = false;
        }
    }

    fn render_chord_generator_modal_view(&mut self, ctx: &egui::Context) {
        let p_idx = self.selected_pattern;
        let c_idx = self.selected_channel;
        let num_chans = self.channels.len();

        let mut insert_chords: Option<Vec<GeneratedChord>> = None;
        let mut status = self.status_message.clone();

        render_chord_generator_modal(
            ctx,
            &mut self.show_chord_generator_modal,
            &mut self.chord_generator_state,
            &mut self.engine,
            &mut status,
            |chords| {
                insert_chords = Some(chords.to_vec());
            },
        );
        self.status_message = status;

        if let Some(chords) = insert_chords {
            if p_idx < self.patterns.len() && c_idx < num_chans {
                for (step_i, chord) in chords.iter().enumerate() {
                    let s_start = (step_i * 4) % 16;
                    self.channels[c_idx].steps[s_start] = true;
                    self.channels[c_idx].notes[s_start] = chord.root_midi;
                    // Insert the full voicing into the piano roll grid so every
                    // chord tone is stored (and played back), not just the root.
                    for &n in &chord.notes {
                        let row = n as i32 - 48;
                        if (0..24).contains(&row) {
                            self.piano_roll_grid[row as usize][s_start] = true;
                        }
                    }
                    if let Some(pat_steps) = self.patterns[p_idx].channel_steps.get_mut(c_idx) {
                        pat_steps[s_start] = true;
                    }
                    if let Some(pat_notes) = self.patterns[p_idx].channel_notes.get_mut(c_idx) {
                        pat_notes[s_start] = chord.root_midi;
                    }
                }
                self.patterns[p_idx].piano_roll_grid = self.piano_roll_grid;
                self.status_message = crate::tstatus!("🎹 Infogade {} ackord (fulla voicings) i Mönster {} (Kanal: {})!", chords.len(), p_idx + 1, self.channels[c_idx].name);
            }
        }
    }

    fn render_tuner_modal_view(&mut self, ctx: &egui::Context) {
        let mic_vu = self.vocal_studio.mic_vu_level;
        let live_pitch = self.vocal_studio.detect_live_pitch_hz();
        render_tuner_modal(
            ctx,
            &mut self.show_tuner_modal,
            &mut self.tuner_state,
            &mut self.engine,
            mic_vu,
            live_pitch,
        );
    }

    fn render_dice_generator_modal_view(&mut self, ctx: &egui::Context) {
        let p_idx = self.selected_pattern;
        let c_idx = self.selected_channel;
        let num_chans = self.channels.len();

        let mut insert_dice: Option<DiceGeneratorState> = None;
        let mut status = self.status_message.clone();

        render_dice_generator_modal(
            ctx,
            &mut self.show_dice_generator_modal,
            &mut self.dice_generator_state,
            &mut self.engine,
            &mut status,
            |state| {
                insert_dice = Some(state.clone());
            },
        );
        self.status_message = status;

        if let Some(dice) = insert_dice {
            if dice.category == crate::ui::dice_generator_modal::DiceCategory::DrumBeat {
                for d in 0..4.min(num_chans) {
                    self.channels[d].steps = dice.generated_drum_grid[d];
                    if p_idx < self.patterns.len() && d < self.patterns[p_idx].channel_steps.len() {
                        self.patterns[p_idx].channel_steps[d] = dice.generated_drum_grid[d];
                    }
                }
                self.status_message = crate::i18n::t("🎲 Klistrade in slumpat trumgroove på de 4 första trumspåren!").to_string();
            } else if p_idx < self.patterns.len() && c_idx < num_chans {
                self.channels[c_idx].steps = [false; 16];
                for n in &dice.generated_notes {
                    if n.step < 16 {
                        self.channels[c_idx].steps[n.step] = true;
                        self.channels[c_idx].notes[n.step] = n.note;
                    }
                }
                if let Some(pat_steps) = self.patterns[p_idx].channel_steps.get_mut(c_idx) {
                    *pat_steps = self.channels[c_idx].steps;
                }
                if let Some(pat_notes) = self.patterns[p_idx].channel_notes.get_mut(c_idx) {
                    *pat_notes = self.channels[c_idx].notes;
                }
                self.status_message = crate::tstatus!("🎲 Klistrade in slumpad melodi på '{}' i Mönster {}!", self.channels[c_idx].name, p_idx + 1);
            }
        }
    }

    fn render_fx_rack_modal_view(&mut self, ctx: &egui::Context) {
        let mut status = self.status_message.clone();
        render_fx_rack_modal(
            ctx,
            &mut self.show_fx_rack_modal,
            &mut self.fx_rack_state,
            &mut self.engine,
            &mut status,
        );
        self.status_message = status;
    }

    fn render_song_structure_modal_view(&mut self, ctx: &egui::Context) {
        let mut apply_sections: Option<Vec<SongSectionItem>> = None;
        let mut status = self.status_message.clone();

        render_song_structure_modal(
            ctx,
            &mut self.show_song_structure_modal,
            &mut self.song_structure_state,
            self.bpm,
            &mut status,
            |sections| {
                apply_sections = Some(sections.to_vec());
            },
        );
        self.status_message = status;

        if let Some(sections) = apply_sections {
            let total_bars: usize = sections.iter().map(|s| s.length_bars).sum();
            self.loop_start_bar = 0;
            self.loop_end_bar = total_bars.max(16);

            self.song_markers.clear();
            let mut bar = 0usize;
            for s in &sections {
                let (_, color) = s.section_type.name_and_color();
                self.song_markers.push(SongMarker {
                    start_bar: bar,
                    length_bars: s.length_bars,
                    name: s.name.clone(),
                    color,
                });
                bar += s.length_bars;
            }

            self.status_message = crate::tstatus!("📑 Applicerade låtstruktur med {} sektioner och skapade {} markörer (Totalt {} takter)!", sections.len(), self.song_markers.len(), total_bars);
        }
    }

    pub fn open_stem_focus(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            self.focused_stem_track = Some(track_idx);
            self.selected_timeline_track = track_idx;
            self.show_stem_focus_modal = true;
            self.status_message = crate::tstatus!("🔍 Öppnade stämeditor för '{}'", self.playlist_tracks[track_idx].name);
        }
    }

    fn render_stem_focus_modal(&mut self, ctx: &egui::Context) {
        if !self.show_stem_focus_modal {
            return;
        }

        let num_tracks = self.playlist_tracks.len();
        if num_tracks == 0 {
            self.show_stem_focus_modal = false;
            return;
        }

        let t_idx = self.focused_stem_track.unwrap_or(0).min(num_tracks.saturating_sub(1));
        let mut close_modal = false;
        let mut open_window = self.show_stem_focus_modal;
        let mut trigger_audio_sync = false;
        let mut trigger_region_sync = false;
        let mut navigate_idx: Option<usize> = None;
        let mut seek_to_sec: Option<f32> = None;
        // Frysningen görs efter stängningen: anropet behöver &mut self, och
        // inne i panelen lånas spåret.
        let mut freeze_request: Option<usize> = None;
        let mut unfreeze_request: Option<usize> = None;
        let frozen_stale = self.frozen_is_stale(t_idx);

        let sec_per_bar = 60.0 / self.bpm * 4.0;

        egui::Window::new(crate::i18n::t("🎛 Stämeditor & Ljudfokus"))
            .open(&mut open_window)
            .collapsible(true)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(760.0, 560.0))
            .show(ctx, |ui| {
                let track = &mut self.playlist_tracks[t_idx];

                // ============================================================
                // 1. TOP HEADER & STEM NAVIGATION & SOLO ISOLATION
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        // Prev / Next Stem Navigator
                        if ui.button(crate::i18n::t("◀ Föregående")).on_hover_text(crate::i18n::t("Växla till föregående stämma")).clicked() {
                            if t_idx > 0 {
                                navigate_idx = Some(t_idx - 1);
                            } else {
                                navigate_idx = Some(num_tracks - 1);
                            }
                        }
                        if ui.button(crate::i18n::t("Nästa ▶")).on_hover_text(crate::i18n::t("Växla till nästa stämma")).clicked() {
                            if t_idx + 1 < num_tracks {
                                navigate_idx = Some(t_idx + 1);
                            } else {
                                navigate_idx = Some(0);
                            }
                        }

                        ui.separator();

                        // Stem Icon & Name TextEdit
                        ui.label(egui::RichText::new(track.icon).size(18.0));
                        ui.add(egui::TextEdit::singleline(&mut track.name).desired_width(180.0).font(egui::FontId::proportional(13.0)));

                        ui.separator();

                        // SOLO / ISOLERA STÄMMA BUTTON (Prominent & Glowing)
                        let solo_bg = if track.solo { Theme::FL_ORANGE } else { Color32::from_rgb(45, 40, 30) };
                        let solo_fg = if track.solo { Color32::BLACK } else { Theme::FL_ORANGE };
                        if ui.add(egui::Button::new(egui::RichText::new(if track.solo { "🎧 ISOLERAD (SOLO ON)" } else { crate::i18n::t("🎧 ISOLERA STÄMMA") }).strong().color(solo_fg)).fill(solo_bg)).on_hover_text(crate::i18n::t("Isolera och lyssna enbart på denna stämma i realtid")).clicked() {
                            track.solo = !track.solo;
                            trigger_audio_sync = true;
                        }

                        // MUTE TOGGLE
                        let mute_bg = if track.muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(35, 40, 50) };
                        let mute_fg = if track.muted { Color32::WHITE } else { Theme::TEXT_MUTED };
                        if ui.add(egui::Button::new(egui::RichText::new(if track.muted { "🔇 MUTAD" } else { "🔊 AKTIV" }).strong().color(mute_fg)).fill(mute_bg)).clicked() {
                            track.muted = !track.muted;
                            trigger_audio_sync = true;
                        }

                        // FRYS / TINA (Tier 2). Ett fruset spår ligger som
                        // färdigt ljud: uppspelningen slipper köra syntesen, och
                        // patterns finns kvar i projektet så att en upptining
                        // återställer exakt samma musik.
                        ui.separator();
                        if track.is_frozen() {
                            let label = if frozen_stale {
                                crate::i18n::t("🔥 Tina (inaktuell)")
                            } else {
                                crate::i18n::t("🔥 Tina")
                            };
                            let bg = if frozen_stale {
                                Color32::from_rgb(150, 100, 30)
                            } else {
                                Color32::from_rgb(40, 70, 90)
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(label).strong().color(Color32::WHITE)).fill(bg))
                                .on_hover_text(crate::i18n::t("Spåret spelas som färdigt ljud. Tina upp det för att köra patterns igen — musiken är oförändrad."))
                                .clicked()
                            {
                                unfreeze_request = Some(t_idx);
                            }
                            if frozen_stale {
                                ui.label(egui::RichText::new(crate::i18n::t("⚠ ändrat sedan frysningen")).size(10.0).color(Theme::FL_ORANGE))
                                    .on_hover_text(crate::i18n::t("Mönstret eller ljudet har ändrats efter frysningen, så ljudet är inte längre det du hör av patterns. Tina och frys igen."));
                            }
                        } else if track.can_freeze() {
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("❄ Frys spår")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(35, 60, 80)))
                                .on_hover_text(crate::i18n::t("Renderar spåret till ljud och spelar det i stället för patterns — sparar CPU i stora projekt. Patterns finns kvar, och Ctrl+Z tar tillbaka frysningen."))
                                .clicked()
                            {
                                freeze_request = Some(t_idx);
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(crate::i18n::t("❌ Stäng")).clicked() {
                                close_modal = true;
                            }
                        });
                    });
                });

                ui.add_space(4.0);

                // ============================================================
                // 2. TAB SELECTOR BAR
                // ============================================================
                ui.horizontal(|ui| {
                    let tabs = [
                        (0, "🎚 Volym, Pan & Dynamik"),
                        (1, "📈 3-Bands Parametrisk EQ"),
                        (2, "⏱ Fading & Tidsredigering (0.01s)"),
                        (3, crate::i18n::t("🎵 Klipp & Vågform")),
                    ];
                    for (tab_idx, label) in tabs {
                        let is_sel = self.stem_focus_active_tab == tab_idx;
                        let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(25, 30, 40) };
                        let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(label)).strong().size(11.0).color(fg)).fill(bg)).clicked() {
                            self.stem_focus_active_tab = tab_idx;
                        }
                    }
                });

                ui.add_space(4.0);

                // ============================================================
                // 3. TAB CONTENT
                // ============================================================
                match self.stem_focus_active_tab {
                    0 => {
                        // TAB 1: VOLYM, PAN & DYNAMIK
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Nivåer & Stereobild för stämman")).strong().color(Theme::FL_ORANGE));
                            ui.add_space(6.0);

                            ui.horizontal(|ui| {
                                // Volume Section
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎚 Exakt Ljudvolym & Gain")).strong().size(12.0).color(Theme::FL_CYAN));
                                        ui.add_space(4.0);

                                        let cur_vol = track.volume;
                                        let gain_db = if cur_vol <= 0.001 { -60.0 } else { 20.0 * cur_vol.log10() };
                                        ui.label(egui::RichText::new(crate::tstatus!("Gain: {:+.2} dB  (Nivå: {:.1}%)", gain_db, cur_vol * 100.0)).strong().size(13.0).color(Theme::TEXT_BRIGHT));

                                        ui.add_space(4.0);
                                        let mut vol_slider = track.volume;
                                        if ui.add(egui::Slider::new(&mut vol_slider, 0.0..=2.0).custom_formatter(|v, _| {
                                            let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                                            format!("{:+.1} dB ({:.0}%)", db, v * 100.0)
                                        })).changed() {
                                            track.volume = vol_slider;
                                            trigger_audio_sync = true;
                                        }

                                        ui.add_space(6.0);
                                        ui.label(crate::i18n::t("Snabbjustering (0.1 dB precision):"));
                                        ui.horizontal(|ui| {
                                            if ui.button(crate::i18n::t("-0.5 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-0.5 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("-0.1 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-0.1 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("0.0 dB (100%)")).clicked() {
                                                track.volume = 1.0;
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("+0.1 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(0.1 / 20.0)).min(2.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("+0.5 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(0.5 / 20.0)).min(2.0);
                                                trigger_audio_sync = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            if ui.button(crate::i18n::t("📉 -3.0 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-3.0 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("📉 -6.0 dB")).clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-6.0 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button(crate::i18n::t("🔇 Muta")).clicked() {
                                                track.volume = 0.0;
                                                trigger_audio_sync = true;
                                            }
                                        });
                                    });
                                });

                                ui.separator();

                                // Pan & Dynamics Section
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("↔ Stereopanorering")).strong().size(12.0).color(Theme::FL_CYAN));
                                        let pan_text = if track.pan < -0.01 {
                                            crate::tstatus!("Vänster {:.0}%", track.pan.abs() * 100.0)
                                        } else if track.pan > 0.01 {
                                            crate::tstatus!("Höger {:.0}%", track.pan * 100.0)
                                        } else {
                                            crate::i18n::t("Center (Mitt)").to_string()
                                        };
                                        ui.label(egui::RichText::new(format!("Pan: {}", pan_text)).color(Theme::TEXT_BRIGHT));

                                        let mut pan_val = track.pan;
                                        if ui.add(egui::Slider::new(&mut pan_val, -1.0..=1.0).text("L / R")).changed() {
                                            track.pan = pan_val;
                                            trigger_audio_sync = true;
                                        }

                                        ui.horizontal(|ui| {
                                            if ui.button(crate::i18n::t("100% V")).clicked() { track.pan = -1.0; trigger_audio_sync = true; }
                                            if ui.button(crate::i18n::t("50% V")).clicked() { track.pan = -0.5; trigger_audio_sync = true; }
                                            if ui.button(crate::i18n::t("Center")).clicked() { track.pan = 0.0; trigger_audio_sync = true; }
                                            if ui.button(crate::i18n::t("50% H")).clicked() { track.pan = 0.5; trigger_audio_sync = true; }
                                            if ui.button(crate::i18n::t("100% H")).clicked() { track.pan = 1.0; trigger_audio_sync = true; }
                                        });

                                        ui.add_space(8.0);
                                        ui.label(egui::RichText::new(crate::i18n::t("🎛 Dynamik & Kompressor")).strong().size(12.0).color(Theme::FL_CYAN));
                                        ui.add(egui::Slider::new(&mut track.comp_threshold_db, -30.0..=0.0).text("Threshold (dB)"));
                                        ui.add(egui::Slider::new(&mut track.comp_ratio, 1.0..=8.0).text("Ratio"));
                                    });
                                });
                            });
                        });
                    }

                    1 => {
                        // TAB 2: 3-BANDS PARAMETRISK EQ MED GRAFISK KURVA
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("📈 3-Bands Parametrisk Stäm-EQ")).strong().color(Theme::FL_ORANGE));
                                ui.checkbox(&mut track.eq.enabled, "Aktivera EQ");
                            });

                            ui.add_space(4.0);

                            // Graphical EQ Curve Canvas
                            let (eq_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 130.0), Sense::hover());
                            let p = ui.painter();
                            p.rect_filled(eq_rect, Rounding::same(4.0), Color32::from_rgb(14, 18, 26));
                            p.rect_stroke(eq_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 40, 56)));

                            // Draw Grid Lines (100 Hz, 1 kHz, 10 kHz and 0 dB, +6 dB, -6 dB)
                            let mid_y = eq_rect.center().y;
                            p.line_segment([Pos2::new(eq_rect.min.x, mid_y), Pos2::new(eq_rect.max.x, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)));
                            p.line_segment([Pos2::new(eq_rect.min.x, mid_y - 30.0), Pos2::new(eq_rect.max.x, mid_y - 30.0)], Stroke::new(0.5_f32, Color32::from_rgb(35, 42, 56)));
                            p.line_segment([Pos2::new(eq_rect.min.x, mid_y + 30.0), Pos2::new(eq_rect.max.x, mid_y + 30.0)], Stroke::new(0.5_f32, Color32::from_rgb(35, 42, 56)));

                            p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y - 30.0), egui::Align2::LEFT_CENTER, "+6 dB", egui::FontId::proportional(9.0), Color32::from_rgb(100, 110, 130));
                            p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y), egui::Align2::LEFT_CENTER, "0 dB", egui::FontId::proportional(9.0), Color32::from_rgb(120, 140, 160));
                            p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y + 30.0), egui::Align2::LEFT_CENTER, "-6 dB", egui::FontId::proportional(9.0), Color32::from_rgb(100, 110, 130));

                            // Frequency Grid Markers
                            let freq_x = |f: f32| -> f32 {
                                let min_log = 20.0_f32.log10();
                                let max_log = 20000.0_f32.log10();
                                let norm = (f.log10() - min_log) / (max_log - min_log);
                                eq_rect.min.x + norm * eq_rect.width()
                            };

                            for &(freq, lbl) in &[(100.0, "100Hz"), (1000.0, "1kHz"), (10000.0, "10kHz")] {
                                let fx = freq_x(freq);
                                p.line_segment([Pos2::new(fx, eq_rect.min.y), Pos2::new(fx, eq_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 50)));
                                p.text(Pos2::new(fx, eq_rect.max.y - 8.0), egui::Align2::CENTER_CENTER, lbl, egui::FontId::proportional(9.0), Color32::from_rgb(90, 100, 120));
                            }

                            // Plot Smooth EQ Curve
                            if track.eq.enabled {
                                let num_points = 80;
                                let mut curve_pts = Vec::with_capacity(num_points);
                                for i in 0..num_points {
                                    let norm = i as f32 / (num_points - 1) as f32;
                                    let freq = 20.0 * 1000.0_f32.powf(norm); // 20 Hz to 20 kHz

                                    // Low shelf calculation
                                    let low_resp = track.eq.low_gain_db / (1.0 + (freq / track.eq.low_freq).powi(2));

                                    // Mid bell calculation
                                    let mid_ratio = (freq / track.eq.mid_freq).ln();
                                    let mid_resp = track.eq.mid_gain_db * (-0.5 * (mid_ratio * track.eq.mid_q).powi(2)).exp();

                                    // High shelf calculation
                                    let high_resp = track.eq.high_gain_db * (freq / track.eq.high_freq).powi(2) / (1.0 + (freq / track.eq.high_freq).powi(2));

                                    let total_db = low_resp + mid_resp + high_resp;
                                    let px = eq_rect.min.x + norm * eq_rect.width();
                                    let py = mid_y - (total_db * 5.0).clamp(-55.0, 55.0);
                                    curve_pts.push(Pos2::new(px, py));
                                }

                                for pair in curve_pts.windows(2) {
                                    p.line_segment([pair[0], pair[1]], Stroke::new(2.0_f32, Theme::FL_CYAN));
                                }

                                // Draw Control Points for Low, Mid, High
                                let low_pt = Pos2::new(freq_x(track.eq.low_freq), mid_y - track.eq.low_gain_db * 5.0);
                                let mid_pt = Pos2::new(freq_x(track.eq.mid_freq), mid_y - track.eq.mid_gain_db * 5.0);
                                let high_pt = Pos2::new(freq_x(track.eq.high_freq), mid_y - track.eq.high_gain_db * 5.0);

                                p.circle_filled(low_pt, 4.5, Theme::FL_ORANGE);
                                p.circle_filled(mid_pt, 4.5, Theme::FL_GREEN);
                                p.circle_filled(high_pt, 4.5, Color32::from_rgb(180, 110, 255));
                            }

                            ui.add_space(6.0);

                            // EQ Knobs & Sliders Row
                            ui.horizontal(|ui| {
                                // Low Band
                                ui.group(|ui| {
                                    ui.set_width(220.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("🔊 Bas (Low Shelf)")).strong().color(Theme::FL_ORANGE));
                                    ui.add(egui::Slider::new(&mut track.eq.low_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.low_freq, 40.0..=400.0).text("Frekvens").suffix(" Hz"));
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("-1dB")).clicked() { track.eq.low_gain_db = (track.eq.low_gain_db - 1.0).max(-12.0); }
                                        if ui.button(crate::i18n::t("0dB")).clicked() { track.eq.low_gain_db = 0.0; }
                                        if ui.button(crate::i18n::t("+1dB")).clicked() { track.eq.low_gain_db = (track.eq.low_gain_db + 1.0).min(12.0); }
                                    });
                                });

                                // Mid Band
                                ui.group(|ui| {
                                    ui.set_width(220.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("🎙 Mellanregister (Mid Peak)")).strong().color(Theme::FL_GREEN));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_freq, 200.0..=6000.0).text("Frekvens").suffix(" Hz"));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_q, 0.5..=3.0).text("Q (Bredd)"));
                                });

                                // High Band
                                ui.group(|ui| {
                                    ui.set_width(220.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("✨ Diskant (High Shelf)")).strong().color(Color32::from_rgb(180, 110, 255)));
                                    ui.add(egui::Slider::new(&mut track.eq.high_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.high_freq, 3000.0..=16000.0).text("Frekvens").suffix(" Hz"));
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("-1dB")).clicked() { track.eq.high_gain_db = (track.eq.high_gain_db - 1.0).max(-12.0); }
                                        if ui.button(crate::i18n::t("0dB")).clicked() { track.eq.high_gain_db = 0.0; }
                                        if ui.button(crate::i18n::t("+1dB")).clicked() { track.eq.high_gain_db = (track.eq.high_gain_db + 1.0).min(12.0); }
                                    });
                                });
                            });

                            ui.add_space(4.0);

                            // EQ Presets
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Förinställningar:")).size(10.5).color(Theme::TEXT_MUTED));
                                if ui.button(crate::i18n::t("🎙 Sång: Luft & Värme")).clicked() {
                                    track.eq.low_gain_db = -2.0; track.eq.low_freq = 100.0;
                                    track.eq.mid_gain_db = 1.5; track.eq.mid_freq = 3000.0; track.eq.mid_q = 1.2;
                                    track.eq.high_gain_db = 3.5; track.eq.high_freq = 9000.0;
                                }
                                if ui.button(crate::i18n::t("🥁 Trummor: Punch & Snap")).clicked() {
                                    track.eq.low_gain_db = 3.0; track.eq.low_freq = 75.0;
                                    track.eq.mid_gain_db = -2.5; track.eq.mid_freq = 450.0; track.eq.mid_q = 1.0;
                                    track.eq.high_gain_db = 2.0; track.eq.high_freq = 7500.0;
                                }
                                if ui.button(crate::i18n::t("🎸 Gitarr: Presence")).clicked() {
                                    track.eq.low_gain_db = -3.0; track.eq.low_freq = 120.0;
                                    track.eq.mid_gain_db = 2.0; track.eq.mid_freq = 2400.0; track.eq.mid_q = 1.4;
                                    track.eq.high_gain_db = 1.0; track.eq.high_freq = 6000.0;
                                }
                                if ui.button(crate::i18n::t("🔊 Bas: Deep Sub")).clicked() {
                                    track.eq.low_gain_db = 4.0; track.eq.low_freq = 65.0;
                                    track.eq.mid_gain_db = -3.0; track.eq.mid_freq = 1000.0; track.eq.mid_q = 1.0;
                                    track.eq.high_gain_db = -4.0; track.eq.high_freq = 5000.0;
                                }
                                if ui.button(crate::i18n::t("🔄 Nollställ (Flat)")).clicked() {
                                    track.eq = TrackEq::default();
                                }
                            });
                        });

                        trigger_audio_sync = true;
                    }

                    2 => {
                        // TAB 3: FADING & TIDSREDIGERING (0.01s Precision)
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("⏱ Fading & Fade-kurvor (Exakt hundradels precision)")).strong().color(Theme::FL_ORANGE));
                            ui.add_space(4.0);

                            // If there are regions on this track, allow editing the first / selected region or batch-apply
                            let num_regions = track.regions.len();
                            let mut apply_all_fades = false;
                            let mut global_fade_in_sec = 0.05_f32;
                            let mut global_fade_out_sec = 0.05_f32;

                            if num_regions > 0 {
                                global_fade_in_sec = track.regions[0].fade_in_bars * sec_per_bar;
                                global_fade_out_sec = track.regions[0].fade_out_bars * sec_per_bar;
                            }

                            ui.horizontal(|ui| {
                                // Fade In Box
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("📈 Fade In (In-toning)")).strong().size(12.0).color(Theme::FL_CYAN));
                                    let cs_in = (global_fade_in_sec * 100.0).round() as i32;
                                    ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_in_sec, cs_in)).strong().color(Theme::TEXT_BRIGHT));

                                    ui.add_space(4.0);
                                    if ui.add(egui::Slider::new(&mut global_fade_in_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                        apply_all_fades = true;
                                    }

                                    ui.add_space(4.0);
                                    ui.label(crate::i18n::t("Finjustera med hundradelar:"));
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("-0.10s")).clicked() { global_fade_in_sec = (global_fade_in_sec - 0.10).max(0.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("-0.01s")).clicked() { global_fade_in_sec = (global_fade_in_sec - 0.01).max(0.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("0.00s")).clicked() { global_fade_in_sec = 0.0; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("+0.01s")).clicked() { global_fade_in_sec = (global_fade_in_sec + 0.01).min(10.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("+0.10s")).clicked() { global_fade_in_sec = (global_fade_in_sec + 0.10).min(10.0); apply_all_fades = true; }
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("20 ms")).clicked() { global_fade_in_sec = 0.02; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("50 ms")).clicked() { global_fade_in_sec = 0.05; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("100 ms")).clicked() { global_fade_in_sec = 0.10; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("500 ms")).clicked() { global_fade_in_sec = 0.50; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("1.00 s")).clicked() { global_fade_in_sec = 1.00; apply_all_fades = true; }
                                    });
                                });

                                ui.separator();

                                // Fade Out Box
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("📉 Fade Out (Ut-toning)")).strong().size(12.0).color(Theme::FL_ORANGE));
                                    let cs_out = (global_fade_out_sec * 100.0).round() as i32;
                                    ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_out_sec, cs_out)).strong().color(Theme::TEXT_BRIGHT));

                                    ui.add_space(4.0);
                                    if ui.add(egui::Slider::new(&mut global_fade_out_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                        apply_all_fades = true;
                                    }

                                    ui.add_space(4.0);
                                    ui.label(crate::i18n::t("Finjustera med hundradelar:"));
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("-0.10s")).clicked() { global_fade_out_sec = (global_fade_out_sec - 0.10).max(0.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("-0.01s")).clicked() { global_fade_out_sec = (global_fade_out_sec - 0.01).max(0.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("0.00s")).clicked() { global_fade_out_sec = 0.0; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("+0.01s")).clicked() { global_fade_out_sec = (global_fade_out_sec + 0.01).min(10.0); apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("+0.10s")).clicked() { global_fade_out_sec = (global_fade_out_sec + 0.10).min(10.0); apply_all_fades = true; }
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("20 ms")).clicked() { global_fade_out_sec = 0.02; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("50 ms")).clicked() { global_fade_out_sec = 0.05; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("100 ms")).clicked() { global_fade_out_sec = 0.10; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("500 ms")).clicked() { global_fade_out_sec = 0.50; apply_all_fades = true; }
                                        if ui.button(crate::i18n::t("1.00 s")).clicked() { global_fade_out_sec = 1.00; apply_all_fades = true; }
                                    });
                                });
                            });

                            if apply_all_fades {
                                for r in &mut track.regions {
                                    r.fade_in_bars = global_fade_in_sec / sec_per_bar;
                                    r.fade_out_bars = global_fade_out_sec / sec_per_bar;
                                }
                                trigger_region_sync = true;
                            }

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Tonhöjd (Pitch Shift):")).size(11.0).color(Theme::TEXT_MUTED));
                                ui.add(
                                    egui::Slider::new(&mut track.pitch_semitones, -12.0..=12.0)
                                        .step_by(0.01)
                                        .fixed_decimals(2)
                                        .suffix(" st"),
                                );
                                if ui.button(crate::i18n::t("Nollställ Pitch")).clicked() {
                                    track.pitch_semitones = 0.0;
                                }
                            });
                        });
                    }

                    3 => {
                        // TAB 4: KLIPP & VÅGFORM
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(format!("🎵 Klipp & Regioner i {}", track.name)).strong().color(Theme::FL_ORANGE));
                            ui.add_space(4.0);

                            // Waveform Overview
                            let (wave_rect, wave_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 90.0), Sense::click_and_drag());
                            let p = ui.painter();
                            p.rect_filled(wave_rect, Rounding::same(4.0), Color32::from_rgb(12, 15, 22));
                            p.rect_stroke(wave_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 40, 56)));

                            let mid_y = wave_rect.center().y;

                            // Draw Regions & Waves
                            let track_color = track.color;
                            for r in &track.regions {
                                let total_max_bars = 140.0_f32;
                                let rx_start = wave_rect.min.x + (r.start_bar / total_max_bars) * wave_rect.width();
                                let rx_end = rx_start + (r.length_bars / total_max_bars) * wave_rect.width();
                                let r_rect = Rect::from_min_max(Pos2::new(rx_start, wave_rect.min.y + 2.0), Pos2::new(rx_end, wave_rect.max.y - 2.0));

                                if r_rect.width() > 2.0 {
                                    p.rect_filled(r_rect, Rounding::same(2.0), Color32::from_rgba_unmultiplied(track_color.r(), track_color.g(), track_color.b(), 60));
                                    p.rect_stroke(r_rect, Rounding::same(2.0), Stroke::new(1.0_f32, track_color));

                                    // Draw Wave peaks
                                    let pts = r.waveform_peaks.len().max(1);
                                    let w_step = r_rect.width() / pts as f32;
                                    for (wi, &amp) in r.waveform_peaks.iter().enumerate() {
                                        let wx = r_rect.min.x + wi as f32 * w_step;
                                        let wh = amp * (r_rect.height() * 0.40);
                                        p.line_segment([Pos2::new(wx, mid_y - wh), Pos2::new(wx, mid_y + wh)], Stroke::new(1.0_f32, Color32::WHITE));
                                    }
                                }
                            }

                            // Interactive Scrubbing on Waveform
                            if (wave_resp.clicked() || wave_resp.dragged())
                                && let Some(m_pos) = wave_resp.hover_pos() {
                                    let norm = ((m_pos.x - wave_rect.min.x) / wave_rect.width()).clamp(0.0, 1.0);
                                    let target_sec = norm * (140.0 * sec_per_bar);
                                    seek_to_sec = Some(target_sec);
                                }

                            ui.add_space(6.0);

                            // Region Table
                            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                                let mut del_region_idx: Option<usize> = None;
                                for (ri, r) in track.regions.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("#{}: {}", ri + 1, r.name)).strong().color(Theme::TEXT_BRIGHT));
                                        ui.separator();

                                        let r_start_sec = r.start_bar * sec_per_bar;
                                        let r_len_sec = r.length_bars * sec_per_bar;
                                        ui.label(crate::tstatus!("Start: {}  |  Längd: {}", format_time_hundredths(r_start_sec), format_time_hundredths(r_len_sec)));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(crate::i18n::t("🗑 Ta bort")).clicked() {
                                                del_region_idx = Some(ri);
                                            }
                                            let m_txt = if r.muted { crate::i18n::t("🔇 Mutad") } else { crate::i18n::t("🔊 På") };
                                            if ui.button(m_txt).clicked() {
                                                r.muted = !r.muted;
                                                trigger_region_sync = true;
                                            }
                                        });
                                    });
                                    ui.separator();
                                }

                                if let Some(del_i) = del_region_idx
                                    && del_i < track.regions.len() {
                                        track.regions.remove(del_i);
                                        trigger_region_sync = true;
                                    }
                            });
                        });
                    }

                    _ => {}
                }
            });

        if let Some(target) = freeze_request {
            self.freeze_track(target);
        }
        if let Some(target) = unfreeze_request {
            self.unfreeze_track(target);
        }

        if let Some(target_idx) = navigate_idx {
            self.focused_stem_track = Some(target_idx);
            self.selected_timeline_track = target_idx;
        }

        if let Some(target_sec) = seek_to_sec {
            self.seek_song_time(target_sec);
        }

        if trigger_audio_sync {
            self.sync_track_audio_state(t_idx);
        }
        if trigger_region_sync {
            self.sync_track_regions(t_idx);
        }

        if close_modal || !open_window {
            self.show_stem_focus_modal = false;
        }
    }

    fn render_help_manual_modal(&mut self, ctx: &egui::Context) {
        if !self.show_help_guide {
            return;
        }

        let mut close = false;
        let mut open = self.show_help_guide;

        egui::Window::new(crate::i18n::t("📖 Sonix Studio – Komplett Bruksanvisning & Master Manual"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(920.0, 620.0))
            .min_size(Vec2::new(760.0, 480.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                // Top Header with quick search and close
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📖 SONIX STUDIO MASTER MANUAL")).strong().size(14.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new(crate::i18n::t("• Komplett Referensguide & Handbok")).size(11.0).color(Theme::FL_CYAN));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✖ Stäng Manual")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
                            close = true;
                        }
                    });
                });

                ui.separator();

                // 2-Column Layout: Left Chapters Sidebar + Right Scrollable Documentation
                ui.horizontal(|ui| {
                    // Left Chapter Navigation Sidebar
                    ui.group(|ui| {
                        ui.set_width(230.0);
                        ui.set_height(ui.available_height());
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("KAPITEL & AVSNITT:")).strong().size(10.5).color(Theme::TEXT_MUTED));
                            ui.add_space(2.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("🔍")).size(10.0));
                                ui.add(egui::TextEdit::singleline(&mut self.help_search_query).hint_text(crate::i18n::t("Sök i manualen...")).desired_width(170.0));
                            });
                            ui.add_space(4.0);

                            let chapters = [
                                (0, "🚀 1. Snabbstart & Översikt"),
                                (1, "⌨ 2. Tangentbord & Kommandon"),
                                (2, "📊 3. Tidslinje & 0.01s Snäpp"),
                                (3, "📦 4. Multi-Track Stämimport"),
                                (4, "🔍 5. Stämeditor & 3-Band EQ"),
                                (5, "🥁 6. Channel Rack & Beats"),
                                (6, "🎹 7. Piano Roll & Synth"),
                                (7, "🎤 8. Vocal Studio & Pitch"),
                                (8, "🎛 9. Mixer & WAV-Export"),
                                (9, "⚙ 10. PipeWire & Wayland"),
                            ];

                            egui::ScrollArea::vertical().id_salt("manual_nav_scroll").show(ui, |ui| {
                                for (ch_idx, title) in chapters {
                                    let is_sel = self.help_manual_active_tab == ch_idx;
                                    let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(22, 26, 34) };
                                    let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };

                                    if ui.add_sized(Vec2::new(215.0, 26.0), egui::Button::new(egui::RichText::new(crate::i18n::t(title)).strong().size(10.5).color(fg)).fill(bg)).clicked() {
                                        self.help_manual_active_tab = ch_idx;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                        });
                    });

                    // Right Main Content Scroll Area
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height(ui.available_height());

                        egui::ScrollArea::vertical()
                            .id_salt("manual_content_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                match self.help_manual_active_tab {
                                    0 => {
                                        // 1. SNABBSTART & ÖVERSIKT
                                        ui.heading(egui::RichText::new(crate::i18n::t("🚀 1. Snabbstart & Översikt")).color(Theme::FL_ORANGE));
                                        ui.label(egui::RichText::new(crate::i18n::t("Välkommen till Sonix Studio – en blixtsnabb Digital Audio Workstation skapad för Linux med äkta realtidsprestanda.")).size(12.0).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(8.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🎯 Typiskt produktionsarbetsflöde:")).strong().color(Theme::FL_CYAN));
                                            ui.add_space(4.0);
                                            ui.label(crate::i18n::t("1. Importera stämmor: Klicka på '📦 IMPORT STEMS' (Ctrl+I) för att läsa in ett ZIP-paket med sång, bas, trummor m.m."));
                                            ui.label(crate::i18n::t("2. Arrangera & Klipp: Använd saxverktyget (✂ Klipp) och zooma in djupt (Ctrl+Scroll) för att dela med 0.01s precision."));
                                            ui.label(crate::i18n::t("3. Fokusera & Förädla: Klicka på '🔍' på ett spår för att öppna Stämeditorn, isolera stämman med Solo och ratta 3-bands EQ."));
                                            ui.label(crate::i18n::t("4. Skapa trumkomp: Tryck F4 för att öppna Channel Rack och klicka in 16-stegs beats."));
                                            ui.label(crate::i18n::t("5. Spela in melodier: Tryck F5 för Piano Roll eller spela live med datortangentbordet."));
                                            ui.label(crate::i18n::t("6. Mixa & Exportera: Tryck F6 för Mixern och Ctrl+E för att exportera mastrad 48kHz WAV."));
                                        });

                                        ui.add_space(8.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("⚡ Systemarkitektur & Prestanda:")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• 100% Rust Audio DSP – Inget hack, noll skräpsamling (Garbage Collection), noll latens."));
                                            ui.label(crate::i18n::t("• PipeWire & ALSA Native – Ansluter direkt till Linux moderna ljudserver."));
                                            ui.label(crate::i18n::t("• Asynkron bakgrundsavkodning – Gränssnittet fryser aldrig vid inläsning av tunga ljudfiler."));
                                        });
                                    }

                                    1 => {
                                        // 2. TANGENTBORD & KOMMANDON
                                        ui.heading(egui::RichText::new(crate::i18n::t("⌨ 2. Tangentbord & Komplett Kortkommandoreferens")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("NAVIGERING MELLAN VYER (Funktionstangenter):")).strong().color(Theme::FL_CYAN));
                                            ui.separator();
                                            let f_keys = [
                                                ("F1", "Bruksanvisning / Manual & Hjälpcenter"),
                                                ("F3", "Tidslinje / Multi-Track Arranger"),
                                                ("F4", "Sonix Channel Rack (16-stegs trummaskin)"),
                                                ("F5", "Piano Roll (Notinmatning & melodieditor)"),
                                                ("F6", "Mixer Console & Master Effektrack"),
                                                ("F7", "Analog Alchemy Synthesizer"),
                                                ("F8", "Vocal Studio (Melodyne & Harmonizer)"),
                                                ("F9", "AI Music Studio & Prompt Engine"),
                                                ("F10", "Modulär Synt & Patcher"),
                                            ];
                                            for (k, desc) in f_keys {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(format!("[ {:<3} ]", k)).monospace().strong().color(Theme::FL_ORANGE));
                                                    ui.label(crate::i18n::t(desc));
                                                });
                                            }
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("PROJEKT & REDIGERINGSKOMMANDON:")).strong().color(Theme::FL_GREEN));
                                            ui.separator();
                                            let shortcuts = [
                                                ("Mellanslag (Space)", "Starta / Pausa uppspelning"),
                                                ("Ctrl + N", "Skapa nytt tomt projekt"),
                                                ("Ctrl + O / P", "Öppna Projektbläddrare"),
                                                ("Ctrl + S", "Spara projektfil"),
                                                ("Ctrl + I", "Importera Stämmor / Multi-Track Stems"),
                                                ("Ctrl + E", "Exportera projekt (WAV/FLAC/MP3/OGG/AAC)"),
                                                ("Del / Backspace", "Radera markerat ljudklipp"),
                                                ("Ctrl + Scroll", "Mjuk horisontell zoomning i tidslinjen"),
                                                ("A, W, S, E, D...", "Klaviatur – Spela synthen live med tangentbordet"),
                                            ];
                                            for (k, desc) in shortcuts {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(format!("{:<18}", k)).monospace().strong().color(Theme::FL_CYAN));
                                                    ui.label(crate::i18n::t(desc));
                                                });
                                            }
                                        });
                                    }

                                    2 => {
                                        // 3. TIDSLINJE & 0.01s PRECISION
                                        ui.heading(egui::RichText::new(crate::i18n::t("📊 3. Tidslinje, Snäpp & 0.01s Precision")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new(crate::i18n::t("Tidslinjen hanterar obegränsat med ljudspår och regioner med precision ned till 0.01 sekunder (hundradelar).")).size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🔍 Dynamisk 4-Nivåers Tidslinjelinjal:")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("1. Takter (Bars): Stora vertikala streck med taktnummer (1, 2, 3...) och tidskod (MM:SS.cs)."));
                                            ui.label(crate::i18n::t("2. Beats: Fjärdedelstikar (.2, .3, .4) som syns vid normal zoom."));
                                            ui.label(crate::i18n::t("3. 1/16-delssteg: Tunt rutnät för exakt rytmisk klippning och placering."));
                                            ui.label(crate::i18n::t("4. Hundradelar (0.01s): Aktiveras vid djup inzoomning (200%–800%) för millimeterexakta snitt i sång och trummor."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("✂ Redigeringsverktyg & Snäpp:")).strong().color(Theme::FL_ORANGE));
                                            ui.label(crate::i18n::t("• ⇱ Välj: Klicka på en region för att markera och se dess egenskaper i inspektorn."));
                                            ui.label(crate::i18n::t("• ✎ Rita: Klicka i tidslinjen för att rita ut mönster och aktiva klipp."));
                                            ui.label(crate::i18n::t("• ✂ Klipp (0.01s): Saxverktyg. Klicka var som helst på ett ljudspår för att klyva klippet i två."));
                                            ui.label(crate::i18n::t("• 🔇 Muta / 🗑 Radera: Tysta eller radera regioner direkt med ett klick."));
                                            ui.separator();
                                            ui.label(crate::i18n::t("• Snäppläge '⚡ 0.01s (Fri)': Frikopplar från musikaliska takter och låter dig klippa med 10ms precision."));
                                            ui.label(crate::i18n::t("• Snäpplägen 1/16, Beat, Takt: Snäpper automatiskt till det musikaliska tempot (BPM)."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🏃 Följ Tidslinje (Auto-Scroll):")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• '🏃 Följ tidslinje: PÅ': Tidslinjen rullar automatiskt och håller spelhuvudet centrerat på skärmen."));
                                            ui.label(crate::i18n::t("• '⏸ Följ tidslinje: AV': Tidslinjen står stilla så att du kan redigera i lugn och ro medan låten spelar."));
                                        });
                                    }

                                    3 => {
                                        // 4. MULTI-TRACK STÄMIMPORT
                                        ui.heading(egui::RichText::new(crate::i18n::t("📦 4. Multi-Track Stämimport (Stems)")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new(crate::i18n::t("Importera kompletta stämpaket (WAV, MP3, FLAC, OGG) från Suno AI, FL Studio, Ableton, Logic m.fl.")).size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("📥 Hur du importerar:")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("1. Tryck '📦 IMPORT STEMS' i verktygsfältet eller tryck Ctrl+I."));
                                            ui.label(crate::i18n::t("2. Välj bland automatiskt upptäckta paket i ~/Music / ~/Downloads eller välj ZIP-fil/mapp manuellt."));
                                            ui.label(crate::i18n::t("3. Dra & Släpp (Drag & Drop): Dra en .zip-fil direkt in i Sonix-fönstret."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🎛 Automatisk Spåridentifiering & Routing:")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• Lead Vocals ➔ 🎙 Sångbuss med lila färgkod."));
                                            ui.label(crate::i18n::t("• Backing Vocals ➔ 🗣 Körbuss."));
                                            ui.label(crate::i18n::t("• Drums / Kick / Snare ➔ 🥁 Trumbuss med cyan färgkod."));
                                            ui.label(crate::i18n::t("• Bass ➔ 🎸 Basbuss med gul färgkod."));
                                            ui.label(crate::i18n::t("• Guitar / Keys / Synth ➔ 🎹 Synthbuss med grön/orange färgkod."));
                                            ui.label(crate::i18n::t("• FX / Other ➔ ✨ Effektsändning."));
                                            ui.separator();
                                            ui.label(crate::i18n::t("Inläsningen sker i bakgrunden med en förloppsindikator utan att programmet hänger sig."));
                                        });
                                    }

                                    4 => {
                                        // 5. STÄMEDITOR & 3-BANDS EQ
                                        ui.heading(egui::RichText::new(crate::i18n::t("🔍 5. Dedikerad Stämeditor & Ljudfokus")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new(crate::i18n::t("Fokusera på en enda stämma med högprecisionskontroller, decibelnivåer, grafisk EQ och fading.")).size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🎧 1. Isolera stämma (Solo On):")).strong().color(Theme::FL_ORANGE));
                                            ui.label(crate::i18n::t("Klicka på den stora knappen '🎧 ISOLERA STÄMMA (SOLO)' högst upp i editorn för att direkt tysta alla andra spår och lyssna enbart på den valda stämman."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("🎚 2. Volym, Pan & Dynamik (Flik 1):")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• Volymreglage med exakt dB-visning och '±0.1 dB' finjusteringsknappar."));
                                            ui.label(crate::i18n::t("• Stereopanorering med snabbcentrering ('Center')."));
                                            ui.label(crate::i18n::t("• Kompressor: Justerbar Threshold (-30 dB till 0 dB) och Ratio (1:1 till 8:1)."));
                                            ui.label(crate::i18n::t("• Pitch Shifter: Transponera stämman ±12 halvtoner med bevarade formanter (1-centprecision, Shift = finjustering, dubbelklick = nollställ)."));
                                            ui.label(crate::i18n::t("• Reverb & Delay sends."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("📈 3. 3-Bands Grafisk Parametrisk EQ (Flik 2):")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• Interaktiv frekvenskurva i realtid (20 Hz – 20 kHz) med dB-skala."));
                                            ui.label(crate::i18n::t("• Low Shelf (Bas): Gain ±12 dB, brytfrekvens 40–400 Hz."));
                                            ui.label(crate::i18n::t("• Mid Peak (Mellanregister): Gain ±12 dB, frekvens 200 Hz – 6 kHz, Q-faktor 0.5–3.0."));
                                            ui.label(crate::i18n::t("• High Shelf (Diskant): Gain ±12 dB, frekvens 3 kHz – 16 kHz."));
                                            ui.label(crate::i18n::t("• Snabbpresets för Sång, Trummor, Gitarr, Bas och Flat nollställning."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("⏱ 4. Fading & Envelope i hundradelar (Flik 3):")).strong().color(Color32::from_rgb(180, 110, 255)));
                                            ui.label(crate::i18n::t("• Fade In och Fade Out med 0.01s precision."));
                                            ui.label(crate::i18n::t("• Stegknappar för '±0.01s' och '±0.10s'. Snabbval för 20ms, 50ms, 100ms, 500ms, 1.00s."));
                                            ui.label(crate::i18n::t("• Slå på alla: Sätter samma fading på alla klipp i just den stämman."));
                                        });
                                    }

                                    5 => {
                                        // 6. CHANNEL RACK & BEATS
                                        ui.heading(egui::RichText::new(crate::i18n::t("🥁 6. Sonix Channel Rack & Stegsequencer (F4)")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("16-Stegs Trummaskin & Mönster:")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• 8 Klassiska trumkanaler: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, 303 Bass och Lead."));
                                            ui.label(crate::i18n::t("• Mönster P1–P4: Skapa variationer för vers, refräng och stick."));
                                            ui.label(crate::i18n::t("• Swing: Skjutreglage för att ge trummorna ett naturligt sväng."));
                                            ui.label(crate::i18n::t("• ⚡ Slumpa Beats: Genererar omedelbart nya inspirerande trumkomp."));
                                            ui.label(crate::i18n::t("• Velocity-redigering: Justera anslagskraften per steg i den nedre velocity-raden."));
                                        });
                                    }

                                    6 => {
                                        // 7. PIANO ROLL & SYNTH
                                        ui.heading(egui::RichText::new(crate::i18n::t("🎹 7. Piano Roll & Analog Synthesizer (F5 / F7)")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Piano Roll (F5):")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• Grafiskt 3-oktavigt notinmatningsfönster (C3 till B5)."));
                                            ui.label(crate::i18n::t("• Klicka på klaviaturet till vänster för att provlyssna toner i realtid."));
                                            ui.label(crate::i18n::t("• ⚡ Slumpa Melodi: Skapar harmoniska melodislingor automatiskt."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Analog Alchemy Synthesizer (F7):")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• 4 Vågformer: Sawtooth (sågtand), Square (fyrkant), Sine (sinus) och Noise (brus)."));
                                            ui.label(crate::i18n::t("• ADSR Envelope: Attack, Decay, Sustain och Release."));
                                            ui.label(crate::i18n::t("• Resonant SVF-filter (12 dB): Cutoff (20Hz–18kHz) och Resonans."));
                                            ui.label(crate::i18n::t("• Saturation & Drive: Analog rörvärme och distortion."));
                                        });
                                    }

                                    7 => {
                                        // 8. VOCAL STUDIO & PITCH
                                        ui.heading(egui::RichText::new(crate::i18n::t("🎤 8. Vocal Studio, Melodyne & Harmonizer (F8)")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Funktioner i Vocal Studio:")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• Melodyne ARA2 Editor: Interaktiva tonhöjds-blobs för att justera sångens toner och timing."));
                                            ui.label(crate::i18n::t("• 4-Voice Harmonizer: Skapar fylliga sångarrangemang med kör, oktavdubbling eller vocoder."));
                                            ui.label(crate::i18n::t("• Comping & Tagningar: Spela in flera sångtagningar och klipp ihop den bästa versionen."));
                                            ui.label(crate::i18n::t("• Sampling & Recorder: Spela in egna ljud, klappar och instrument med din mikrofon."));
                                        });
                                    }

                                    8 => {
                                        // 9. MIXER & WAV-EXPORT
                                        ui.heading(egui::RichText::new(crate::i18n::t("🎛 9. Mixer Console, Effekter & WAV-Export (F6 / Ctrl+E)")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Mixerbord & Effektrack (F6):")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• 8 Stereokanaler + Master Bus med analoga faders och VU peak meters."));
                                            ui.label(crate::i18n::t("• 3-Bands Parametrisk EQ per mixerkanal."));
                                            ui.label(crate::i18n::t("• Master Limiter & Maximizer för kommersiell ljudstyrka utan digital distorsion."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Export & Rendering (Ctrl+E):")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• Export Song: Renderar hela tidslinjen till en sammanslagen masterfil."));
                                            ui.label(crate::i18n::t("• Export Pattern: Exporterar det aktiva Channel Rack-mönstret."));
                                            ui.label(crate::i18n::t("• 📤 BATCH EXPORT: Exporterar alla aktiva stämmor som separata WAV-filer till ./exports/."));
                                            ui.label(crate::i18n::t("• Format: 32-bit float / 24-bit PCM WAV i 44.1 kHz eller 48 kHz."));
                                        });
                                    }

                                    9 => {
                                        // 10. PIPEWIRE & WAYLAND SETUP
                                        ui.heading(egui::RichText::new(crate::i18n::t("⚙ 10. Ljudmotor, PipeWire & Hyprland / Wayland")).color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor (PipeWire / ALSA / JACK):")).strong().color(Theme::FL_CYAN));
                                            ui.label(crate::i18n::t("• Sonix kommunicerar direkt med Linux professionella ljudserver i realtid."));
                                            ui.label(crate::i18n::t("• Buffertstorlek: 128/256 samples för noll latens vid live-spelning, 512/1024 samples för tunga projekt."));
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(crate::i18n::t("Linux Wayland & Hyprland Säkerhet:")).strong().color(Theme::FL_GREEN));
                                            ui.label(crate::i18n::t("• Sonix är helt anpassat för Hyprland, GNOME Wayland och KDE."));
                                            ui.label(crate::i18n::t("• När fönstret täcks av ett annat fönster eller minimeras fortsätter ljudmotorn att spela utan avbrott samtidigt som GUI-uppritningen vilar för att förhindra krascher."));
                                            ui.label(crate::i18n::t("• Inbyggd Crash Logger sparar automatiskt eventuella problem till /tmp/sonix_crash.log."));
                                        });
                                    }

                                    _ => {}
                                }
                            });
                    });
                });
            });

        if close || !open {
            self.show_help_guide = false;
        }
    }
}

fn rand_simple(seed: usize) -> f32 {
    let mut x = (seed as u32).wrapping_mul(1103515245).wrapping_add(12345);
    x = (x >> 16) & 0x7FFF;
    x as f32 / 32767.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal projektfil — fälten utanför `serde(default)` måste anges.
    fn minimal_project_json(name: &str) -> String {
        format!(
            r#"{{"name":"{name}","bpm":128.0,"swing":0.0,"master_volume":1.0,"master_pan":0.0,"tracks":[]}}"#
        )
    }

    fn isolated_paths(tag: &str) -> crate::paths::Paths {
        let root = std::env::temp_dir().join(format!(
            "sonix_app_test_{}_{}_{}",
            tag,
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("tmpdir");
        crate::paths::Paths::with_home(root)
    }

    /// Kärnan i Fas 6.1: vid start ska bara arbete som *inte* finns i den
    /// manuella filen erbjudas — annars blir varningen brus.
    /// Mixer-undon (Fas 6.2): varje fält som motorn tar emot måste ingå i
    /// sammanfattningen, annars kan en mixerändring ske helt utan att en
    /// ångringspunkt skapas. Fälten räknas upp i samma ordning som
    /// `sync_track_audio_state` och `sync_group_state` skickar dem.
    /// Testpattern med åtta kanaler, som appen bygger dem.
    fn test_pattern() -> Pattern {
        Pattern {
            name: "Testpattern".to_string(),
            color: Color32::WHITE,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        }
    }

    #[test]
    fn pattern_bar_notes_maps_drums_synth_and_bass() {
        use crate::audio::smf::TICKS_PER_STEP_16TH;
        let mut pat = test_pattern();
        // Bastrumma på steg 0 och 8 (kanal 0), virvel på steg 4 (kanal 1).
        pat.channel_steps[0][0] = true;
        pat.channel_steps[0][8] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_notes[0][8] = 36;
        pat.channel_steps[1][4] = true;
        pat.channel_notes[1][4] = 38;
        // Synth: rutnätet, rad 12 = MIDI 60, på steg 2.
        pat.piano_roll_grid[12][2] = true;
        // Bas: kanal 7, MIDI 40, på steg 3.
        pat.channel_steps[7][3] = true;
        pat.channel_notes[7][3] = 40;

        let vels = [100u8; 16];
        let drums = pattern_bar_notes(&pat, TrackKind::Drums, 0, 0, &vels);
        assert_eq!(drums.len(), 3, "tre trumslag: {drums:?}");
        assert!(drums.iter().all(|n| n.channel == 9), "trummor ska ligga på kanal 9");
        assert!(drums.iter().any(|n| n.key == 36 && n.start == 0));
        assert!(drums.iter().any(|n| n.key == 36 && n.start == 8 * TICKS_PER_STEP_16TH));
        assert!(drums.iter().any(|n| n.key == 38 && n.start == 4 * TICKS_PER_STEP_16TH));

        let synth = pattern_bar_notes(&pat, TrackKind::SynthLead, 3, 0, &vels);
        assert_eq!(synth.len(), 1);
        assert_eq!(synth[0].key, 60);
        assert_eq!(synth[0].channel, 3);
        assert_eq!(synth[0].start, 2 * TICKS_PER_STEP_16TH);

        let bass = pattern_bar_notes(&pat, TrackKind::Bassline, 4, 0, &vels);
        assert_eq!(bass.len(), 1);
        assert_eq!(bass[0].key, 40);
        assert_eq!(bass[0].start, 3 * TICKS_PER_STEP_16TH);

        // En takt längre fram flyttar noterna i tid.
        let bar2 = pattern_bar_notes(&pat, TrackKind::Drums, 0, 16, &vels);
        assert!(bar2.iter().all(|n| n.start >= 16 * TICKS_PER_STEP_16TH));
    }

    #[test]
    fn exported_midi_can_be_imported_back_with_the_same_notes() {
        // "Klart när": en fil exporterad från Sonix ska kunna läsas tillbaka
        // till samma noter och längder. Här hela vägen genom kodningen.
        use crate::audio::smf;
        let mut pat = test_pattern();
        pat.channel_steps[0][0] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_steps[1][2] = true;
        pat.channel_notes[1][2] = 38;
        pat.piano_roll_grid[0][5] = true; // MIDI 48
        pat.piano_roll_grid[23][15] = true; // MIDI 71
        pat.channel_steps[7][9] = true;
        pat.channel_notes[7][9] = 43;

        let vels = [100u8; 16];
        let mut notes = pattern_bar_notes(&pat, TrackKind::Drums, 9, 0, &vels);
        notes.extend(pattern_bar_notes(&pat, TrackKind::SynthLead, 0, 0, &vels));
        notes.extend(pattern_bar_notes(&pat, TrackKind::Bassline, 1, 0, &vels));
        let exported = smf::write_midi(120.0, &[smf::MidiTrack {
            name: "Allt".to_string(),
            notes: notes.clone(),
        }]);

        let parsed = smf::parse_midi(&exported).expect("egen fil ska gå att läsa");
        let back: Vec<smf::MidiNote> = parsed
            .notes_with_track()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect();
        assert_eq!(parsed.bpm.map(|b| b.round()), Some(120.0));
        assert_eq!(back.len(), notes.len(), "samma antal noter tillbaka");

        // Tillbaka in i ett tomt pattern: samma rutor ska tändas igen.
        let mut target = test_pattern();
        let report = apply_bar_to_pattern(&back, parsed.ppq, &mut target);
        assert_eq!(report.dropped_beyond_arrangement, 0);
        assert_eq!(report.dropped_out_of_range, 0);
        assert_eq!(report.notes_placed, notes.len());
        assert!(target.channel_steps[0][0], "bastrumman tillbaka på steg 0");
        assert_eq!(target.channel_notes[0][0], 36);
        assert!(target.channel_steps[1][2], "virveln tillbaka på steg 2");
        assert!(target.piano_roll_grid[0][5], "MIDI 48 → rad 0");
        assert!(target.piano_roll_grid[23][15], "MIDI 71 → rad 23");
        assert!(target.channel_steps[7][9], "basen tillbaka på steg 9");
        assert_eq!(target.channel_notes[7][9], 43);
    }

    #[test]
    /// En not i en *annan* takt kastas inte längre — den hör till sin egen takt
    /// och blir ett eget mönster (se runtgångstestet). Det som fortfarande
    /// redovisas som tappat är det som inte får plats i rutnätet alls.
    fn midi_import_reports_notes_outside_the_grid() {
        use crate::audio::smf::MidiNote;
        let notes = vec![
            // Går bra: trumma på steg 0.
            MidiNote { start: 0, length: 10, channel: 9, key: 36, velocity: 100 },
            // Okänd trumtangent (claves 75).
            MidiNote { start: 0, length: 10, channel: 9, key: 75, velocity: 100 },
            // Ton under piano-rollen men spelbar av basspåret.
            MidiNote { start: 0, length: 10, channel: 0, key: 30, velocity: 100 },
            // Ton ovanför piano-rollens 48–71: tappas, och räknas.
            MidiNote { start: 0, length: 10, channel: 0, key: 90, velocity: 100 },
        ];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, crate::audio::smf::PPQ, &mut pat);
        assert_eq!(rep.notes_placed, 2, "trumman och den låga bastonen");
        assert_eq!(rep.dropped_out_of_range, 2, "trumtangenten 75 och tonen på 90");
        assert_eq!(rep.dropped_beyond_arrangement, 0, "allt låg i den här takten");
        assert!(pat.channel_steps[0][0]);
        assert!(
            pat.channel_steps[7][0] && pat.channel_notes[7][0] == 30,
            "tonen under 48 ska hamna på baskanalen"
        );
    }

    #[test]
    fn midi_import_follows_the_files_own_resolution() {
        // En fil med 96 PPQ (vanligt från andra DAW:er) ska hamna på rätt steg,
        // inte skalas fel.
        use crate::audio::smf::MidiNote;
        let notes = vec![MidiNote {
            start: 96, // = fjärdedelsnot = steg 4 vid 96 PPQ
            length: 24,
            channel: 0,
            key: 60,
            velocity: 100,
        }];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, 96, &mut pat);
        assert_eq!(rep.notes_placed, 1);
        assert!(!pat.piano_roll_grid[12][0]);
        assert!(pat.piano_roll_grid[12][4], "96 tick vid 96 PPQ är steg 4");
    }

    #[test]
    fn midi_channels_skip_percussion() {
        // Kanal 9 är percussion, så melodiska spår får aldrig den kanalen.
        assert_eq!(midi_channel_for_track(0), 0);
        assert_eq!(midi_channel_for_track(8), 8);
        assert_eq!(midi_channel_for_track(9), 10);
        for t in 0..16 {
            assert_ne!(midi_channel_for_track(t), 9, "spår {t} fick percussionkanalen");
        }
        assert_eq!(drum_channel_for_key(36), Some(0));
        assert_eq!(drum_channel_for_key(38), Some(1));
        assert_eq!(drum_channel_for_key(42), Some(3));
        assert_eq!(drum_channel_for_key(49), Some(5));
        assert_eq!(drum_channel_for_key(75), None);
    }

    /// Kanal med varje fält satt till ett omisskännligt värde, så att ett
    /// bortglömt fält i sparandet syns direkt.
    fn test_channel() -> ChannelStrip {
        ChannelStrip {
            name: "Virvel".to_string(),
            icon: "🥁".to_string(),
            color: Color32::from_rgba_premultiplied(10, 20, 30, 255),
            volume: 0.42,
            pan: -0.25,
            muted: true,
            solo: true,
            steps: [true; 16],
            notes: [42; 16],
            pitch_semitones: -7,
            pitch_fine_cents: 12.5,
            sample_start: 0.125,
            sample_end: 0.875,
            attack_decay: 0.375,
            is_reverse: true,
            waveform_preview: vec![0.5; 4],
            sample_path: Some("/tmp/sonix-finns-inte.wav".to_string()),
            pcm_audio: None,
            sample_base_note: 43,
        }
    }

    #[test]
    fn saved_channel_round_trip_keeps_every_field() {
        // Fullständighetsvakt (Fas 6.7): läggs ett fält till i ChannelStrip utan
        // att följa med i sparandet, failar det här testet i stället för att
        // tyst tappa inställningen när projektet öppnas igen.
        let ch = test_channel();
        let json = serde_json::to_string(&channel_to_saved(&ch)).unwrap();
        let back: SavedChannel = serde_json::from_str(&json).unwrap();
        let r = saved_to_channel(&back);

        assert_eq!(r.name, ch.name);
        assert_eq!(r.icon, ch.icon);
        assert_eq!(r.color.to_array(), ch.color.to_array());
        assert_eq!(r.volume, ch.volume);
        assert_eq!(r.pan, ch.pan);
        assert_eq!(r.muted, ch.muted);
        assert_eq!(r.solo, ch.solo);
        assert_eq!(r.steps, ch.steps);
        assert_eq!(r.notes, ch.notes);
        assert_eq!(r.pitch_semitones, ch.pitch_semitones);
        assert_eq!(r.pitch_fine_cents, ch.pitch_fine_cents);
        assert_eq!(r.sample_start, ch.sample_start);
        assert_eq!(r.sample_end, ch.sample_end);
        assert_eq!(r.attack_decay, ch.attack_decay);
        assert_eq!(r.is_reverse, ch.is_reverse);
        assert_eq!(r.sample_path, ch.sample_path);
        assert_eq!(r.sample_base_note, ch.sample_base_note);
        // Ljudet sparas inte utan läses från disk: sökvägen finns inte här, så
        // både PCM och vågform ska vara tomma — men sökvägen själv ska med.
        assert!(r.pcm_audio.is_none());
        assert!(r.waveform_preview.is_empty());
    }

    #[test]
    fn project_file_carries_the_notes_and_the_drum_rack() {
        // "Klart när" för Fas 6.7: en sparad fil ska ge tillbaka noterna, inte
        // bara spåren. Före den här ändringen fanns varken `patterns` eller
        // `channels` i projektformatet — musiken tappades vid varje sparning.
        let mut pat = test_pattern();
        pat.channel_steps[0][0] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_steps[3][7] = true;
        pat.piano_roll_grid[12][5] = true;
        pat.piano_roll_grid[0][15] = true;

        let mut ch = test_channel();
        ch.steps[2] = true;
        ch.notes[2] = 42;

        let mut step_velocities = [1.0f32; 16];
        step_velocities[3] = 0.25;

        let data = SonixProjectData {
            name: "Testprojekt".to_string(),
            bpm: 133.0,
            tempo_points: Vec::new(),
            swing: 0.2,
            master_volume: 0.8,
            master_pan: 0.0,
            tracks: Vec::new(),
            plugin_slots: Vec::new(),
            bus_volume: default_bus_volume(),
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: default_vca_volume(),
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
            patterns: vec![pattern_to_saved(&pat)],
            selected_pattern: 0,
            step_velocities: Some(step_velocities),
            channels: vec![channel_to_saved(&ch)],
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(
            json.contains("piano_roll_grid"),
            "noterna ska stå i filen, inte bara i minnet"
        );
        assert!(json.contains("channel_steps"));

        let back: SonixProjectData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patterns.len(), 1);
        let restored = saved_to_pattern(&back.patterns[0]);
        assert_eq!(restored.channel_steps, pat.channel_steps, "trumstegen tillbaka");
        assert_eq!(restored.channel_notes, pat.channel_notes, "trumtonerna tillbaka");
        assert_eq!(restored.piano_roll_grid, pat.piano_roll_grid, "piano-rollen tillbaka");
        assert_eq!(restored.name, pat.name);
        assert_eq!(restored.color.to_array(), pat.color.to_array());
        assert_eq!(back.channels.len(), 1);
        assert_eq!(back.channels[0].steps, ch.steps);
        assert_eq!(back.channels[0].notes, ch.notes);
        assert_eq!(back.step_velocities.unwrap()[3], 0.25, "stegvolymen tillbaka");
        assert_eq!(back.selected_pattern, 0);
    }

    #[test]
    fn old_project_files_without_patterns_still_load() {
        // Bakåtkompatibilitet (Fas 6.7): en fil skriven före den här ändringen
        // har varken patterns, channels eller step_velocities. Den ska fortfarande
        // läsas, och då lämnas standardpatterns och standardracket orörda.
        let old = r#"{
            "name": "Gammalt",
            "bpm": 120.0,
            "swing": 0.0,
            "master_volume": 0.9,
            "master_pan": 0.0,
            "tracks": []
        }"#;
        let data: SonixProjectData = serde_json::from_str(old).expect("gammal fil ska gå att läsa");
        assert!(data.patterns.is_empty());
        assert!(data.channels.is_empty());
        assert_eq!(data.selected_pattern, 0);
        assert!(
            data.step_velocities.is_none(),
            "en gammal fil ska inte påstå något om stegvolymer"
        );
        assert_eq!(data.bpm, 120.0);
    }

    #[test]
    fn restoring_a_saved_project_puts_the_notes_back() {
        // Inläsningsvägen (Fas 6.7): appen står med tomma standardpatterns och
        // ett tomt rack, filen har noterna — efter återställningen ska arbetet
        // finnas i appen igen.
        let mut app_patterns = vec![test_pattern(), test_pattern()];
        let mut app_channels = vec![test_channel(), test_channel()];
        let mut app_selected = 0usize;
        let mut app_velocities = [1.0f32; 16];

        let mut saved_pattern = test_pattern();
        saved_pattern.name = "Beat".to_string();
        saved_pattern.channel_steps[2][3] = true; // bastrumma på steg 3
        saved_pattern.channel_notes[2][3] = 36;
        saved_pattern.piano_roll_grid[4][11] = true;
        let mut saved_channel = test_channel();
        saved_channel.steps[5] = true;
        saved_channel.notes[5] = 49;
        let mut velocities = [1.0f32; 16];
        velocities[6] = 0.4;

        let saved = SavedMusic {
            patterns: &[pattern_to_saved(&saved_pattern)],
            // Filen pekar på ett patternnummer som inte finns här: ska klämmas
            // till ett giltigt index i stället för att lämna appen utanför listan.
            selected_pattern: 99,
            step_velocities: Some(velocities),
            channels: &[channel_to_saved(&saved_channel)],
        };
        restore_saved_music(
            &saved,
            &mut app_patterns,
            &mut app_channels,
            &mut app_selected,
            &mut app_velocities,
        );

        assert_eq!(app_patterns[0].name, "Beat");
        assert!(app_patterns[0].channel_steps[2][3], "trumslaget tillbaka");
        assert_eq!(app_patterns[0].channel_notes[2][3], 36);
        assert!(app_patterns[0].piano_roll_grid[4][11], "piano-rollen tillbaka");
        assert_eq!(app_selected, app_patterns.len() - 1, "valt pattern kläms till listan");
        assert_eq!(app_velocities[6], 0.4, "stegvolymen tillbaka");
        assert!(app_channels[0].steps[5], "kanalens steg tillbaka");
        assert_eq!(app_channels[0].notes[5], 49);
    }

    #[test]
    fn restoring_an_old_project_leaves_the_defaults_alone() {
        // En fil skriven före Fas 6.7 har inga mönster. Då ska appens egna
        // pattern och rack stå kvar — inte nollställas.
        let mut app_patterns = vec![test_pattern()];
        app_patterns[0].name = "Mitt eget".to_string();
        app_patterns[0].channel_steps[1][1] = true;
        let mut app_channels = vec![test_channel()];
        app_channels[0].name = "Min kanal".to_string();
        let mut app_selected = 0usize;
        let mut app_velocities = [0.5f32; 16];

        let saved = SavedMusic {
            patterns: &[],
            selected_pattern: 0,
            step_velocities: None,
            channels: &[],
        };
        restore_saved_music(
            &saved,
            &mut app_patterns,
            &mut app_channels,
            &mut app_selected,
            &mut app_velocities,
        );

        assert_eq!(app_patterns.len(), 1);
        assert_eq!(app_patterns[0].name, "Mitt eget");
        assert!(app_patterns[0].channel_steps[1][1]);
        assert_eq!(app_channels[0].name, "Min kanal");
        assert_eq!(app_velocities[0], 0.5, "stegvolymerna rörs inte");
    }

    #[test]
    fn the_recorded_take_survives_the_project_round_trip() {
        // Fas 6.4: tagningens tajming är också arbete — den ska med i filen.
        let mut pat = test_pattern();
        pat.take.push(2.75, 60, 0.8);
        pat.take.push(6.1, 64, 0.6);
        assert!(pat.take.tightness() > 0.0);

        let json = serde_json::to_string(&pattern_to_saved(&pat)).unwrap();
        assert!(json.contains("take"), "tagningen ska stå i filen");
        let back: SavedPattern = serde_json::from_str(&json).unwrap();
        let restored = saved_to_pattern(&back);

        assert_eq!(restored.take, pat.take, "samma tagning tillbaka");
        assert!((restored.take.tightness() - pat.take.tightness()).abs() < 1e-6);
    }

    #[test]
    fn an_old_pattern_file_loads_with_an_empty_take() {
        // Bakåtkompatibilitet: ett pattern sparat före Fas 6.4 har ingen tagning.
        let old = r#"{
            "name": "Gammalt pattern",
            "channel_steps": [[true, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]],
            "channel_notes": [[36, 38, 39, 42, 46, 49, 36, 38, 39, 42, 46, 49, 36, 38, 39, 42]],
            "piano_roll_grid": [[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]]
        }"#;
        // piano_roll_grid är 24 rader i appen; testet nedan använder rätt form.
        let old = old.replace(
            r#""piano_roll_grid": [[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]]"#,
            &format!(
                r#""piano_roll_grid": [{}]"#,
                vec![
                    "[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]";
                    24
                ]
                .join(",")
            ),
        );
        let back: SavedPattern = serde_json::from_str(&old).expect("gammalt pattern ska gå att läsa");
        assert!(back.take.is_empty(), "ingen tagning i en gammal fil");
        let restored = saved_to_pattern(&back);
        assert_eq!(restored.name, "Gammalt pattern");
        assert!(restored.channel_steps[0][0], "stegen läses som förut");
        assert!(restored.take.is_empty());
    }

    #[test]
    fn a_late_scan_does_not_throw_away_a_fresh_import() {
        // Fas 7.4: skanningen tar minuter. Importerar användaren ett eget sample
        // under tiden får den färdiga skanningen inte skriva över den.
        let scanned = vec![
            test_library_item(0, "Kick", "/samples/kick.wav"),
            test_library_item(1, "Snare", "/samples/snare.wav"),
        ];
        let existing = vec![
            test_library_item(9, "Mitt eget", "/home/alex/Music/Sonix/Samples/mitt.wav"),
            // En post som skanningen också hittade: ska inte bli dubbel.
            test_library_item(1, "Snare", "/samples/snare.wav"),
        ];
        let merged = merge_library(scanned.clone(), &existing);

        assert_eq!(merged.len(), 3, "två skannade + en egen import");
        assert!(merged.iter().any(|i| i.name == "Mitt eget"));
        assert_eq!(
            merged
                .iter()
                .filter(|i| i.file_path.as_deref() == Some("/samples/snare.wav"))
                .count(),
            1,
            "ingen dubblett av det skanningen hittade"
        );
    }

    fn test_library_item(id: usize, name: &str, path: &str) -> LibrarySampleItem {
        LibrarySampleItem {
            id,
            name: name.to_string(),
            category: "Test".to_string(),
            icon: "🎵".to_string(),
            default_note: 60,
            color: Color32::WHITE,
            waveform: Vec::new(),
            file_path: Some(path.to_string()),
        }
    }

    #[test]
    fn mixer_digest_covers_every_mixed_field() {
        let base_track = PlaylistTrack::new(
            "Testkanal".to_string(),
            "🎹",
            TrackKind::SynthLead,
            Color32::WHITE,
        );
        let bus_volume = [1.0_f32; crate::audio::synth::NUM_BUSES];
        let bus_muted = [false; crate::audio::synth::NUM_BUSES];
        let bus_solo = [false; crate::audio::synth::NUM_BUSES];
        let vca_faders = [1.0_f32; crate::audio::synth::NUM_VCAS];
        let vca_muted = [false; crate::audio::synth::NUM_VCAS];
        let vca_solos = [false; crate::audio::synth::NUM_VCAS];

        let dig = |tracks: &[PlaylistTrack]| {
            mixer_digest(
                tracks,
                &bus_volume,
                &bus_muted,
                &bus_solo,
                &vca_faders,
                &vca_muted,
                &vca_solos,
            )
        };
        let base = dig(&[base_track.clone()]);

        let probe = |name: &str, mutate: &dyn Fn(&mut PlaylistTrack)| {
            let mut t = base_track.clone();
            mutate(&mut t);
            assert_ne!(dig(&[t]), base, "fältet '{name}' saknas i mixer-digesten");
        };

        probe("volume", &|t| t.volume += 0.1);
        probe("pan", &|t| t.pan -= 0.2);
        probe("muted", &|t| t.muted = !t.muted);
        probe("solo", &|t| t.solo = !t.solo);
        probe("comp_threshold_db", &|t| t.comp_threshold_db += 1.0);
        probe("comp_ratio", &|t| t.comp_ratio += 0.5);
        probe("reverb_send", &|t| t.reverb_send += 0.1);
        probe("delay_send", &|t| t.delay_send += 0.1);
        probe("pitch_semitones", &|t| t.pitch_semitones += 1.0);
        probe("bus", &|t| t.bus += 1);
        probe("vca", &|t| t.vca = t.vca.map(|v| v + 1).or(Some(0)));
        probe("eq.low_gain_db", &|t| t.eq.low_gain_db += 1.0);
        probe("eq.low_freq", &|t| t.eq.low_freq += 10.0);
        probe("eq.mid_gain_db", &|t| t.eq.mid_gain_db += 1.0);
        probe("eq.mid_freq", &|t| t.eq.mid_freq += 10.0);
        probe("eq.high_gain_db", &|t| t.eq.high_gain_db += 1.0);
        probe("eq.high_freq", &|t| t.eq.high_freq += 10.0);

        // Buss- och VCA-nivåerna ligger utanför spåren och måste också fångas,
        // annars går en bussändring att göra utan ångringspunkt.
        let digest_with = |bv: &[f32], bm: &[bool], bs: &[bool], vf: &[f32], vm: &[bool], vs: &[bool]| {
            mixer_digest(&[base_track.clone()], bv, bm, bs, vf, vm, vs)
        };
        let mut bv = bus_volume;
        bv[0] = 0.5;
        assert_ne!(digest_with(&bv, &bus_muted, &bus_solo, &vca_faders, &vca_muted, &vca_solos), base, "bussvolym saknas i mixer-digesten");
        let mut bm = bus_muted;
        bm[0] = true;
        assert_ne!(digest_with(&bus_volume, &bm, &bus_solo, &vca_faders, &vca_muted, &vca_solos), base, "buss-mute saknas i mixer-digesten");
        let mut bs = bus_solo;
        bs[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bs, &vca_faders, &vca_muted, &vca_solos), base, "buss-solo saknas i mixer-digesten");
        let mut vf = vca_faders;
        vf[0] = 0.25;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vf, &vca_muted, &vca_solos), base, "VCA-volym saknas i mixer-digesten");
        let mut vm = vca_muted;
        vm[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vca_faders, &vm, &vca_solos), base, "VCA-mute saknas i mixer-digesten");
        let mut vs = vca_solos;
        vs[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vca_faders, &vca_muted, &vs), base, "VCA-solo saknas i mixer-digesten");

        // Och det viktigaste: ångringspunkten bär hela ljudbilden. Glöms ett fält
        // i `current_snapshot`/`restore_snapshot` går ändringen att göra men inte
        // att ångra — då fångar jämförelsen här det.
        let snapshot = TimelineUndoSnapshot {
            playlist_tracks: vec![base_track.clone()],
            patterns: Vec::new(),
            bus_volume,
            bus_muted,
            bus_solo,
            vca_faders,
            vca_muted,
            vca_solos,
            selected_timeline_track: 0,
            selected_audio_region: None,
            song_time: 0.0,
            description: "test".to_string(),
        };
        assert_eq!(
            mixer_digest(
                &snapshot.playlist_tracks,
                &snapshot.bus_volume,
                &snapshot.bus_muted,
                &snapshot.bus_solo,
                &snapshot.vca_faders,
                &snapshot.vca_muted,
                &snapshot.vca_solos,
            ),
            base,
            "ångringspunktens ljudbild skiljer sig från den levande state:n"
        );
    }

    #[test]
    fn recovery_offers_newer_autosaves_and_skips_already_saved_work() {
        let paths = isolated_paths("recovery");
        let dir = paths.autosave_dir();
        let now = crate::autosave::now_stamp();

        // (a) Autosave som är nyare än sin manuella fil → erbjuds.
        let fresh = minimal_project_json("Färsk");
        let fresh_file = paths.project_file("Färsk");
        std::fs::create_dir_all(fresh_file.parent().unwrap()).unwrap();
        std::fs::write(&fresh_file, &fresh).expect("manuell fil");
        crate::autosave::save(&dir, "Färsk", fresh.as_bytes(), now + 5).expect("autosave");

        // (b) Autosave som är äldre än sin manuella fil → inget att rädda.
        let saved = minimal_project_json("Redan sparad");
        let saved_file = paths.project_file("Redan sparad");
        std::fs::create_dir_all(saved_file.parent().unwrap()).unwrap();
        std::fs::write(&saved_file, &saved).expect("manuell fil");
        crate::autosave::save(&dir, "Redan sparad", saved.as_bytes(), now.saturating_sub(600))
            .expect("autosave");

        // (c) Autosave utan manuell fil (krasch före första sparningen) → erbjuds.
        let unsaved = minimal_project_json("Aldrig sparad");
        crate::autosave::save(&dir, "Aldrig sparad", unsaved.as_bytes(), now).expect("autosave");

        let found = collect_recovery_candidates_in(&paths);
        let names: Vec<String> = found.iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"Färsk".to_string()), "saknas: {names:?}");
        assert!(
            names.contains(&"Aldrig sparad".to_string()),
            "saknas: {names:?}"
        );
        assert!(
            !names.contains(&"Redan sparad".to_string()),
            "redan sparad ska inte erbjudas: {names:?}"
        );
        // Namnet kommer ur filens JSON, inte ur filnamnets slug.
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Färsk", "nyaste först");

        // Prenumererad (återställd) autosave försvinner ur listan men finns kvar.
        let restored = found[0].entry.path.clone();
        let retired = crate::autosave::retire(&restored).expect("pensionera");
        assert!(retired.exists() && !restored.exists());
        let after = collect_recovery_candidates_in(&paths);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "Aldrig sparad");

        let _ = std::fs::remove_dir_all(paths.state_dir());
    }

    /// Läslistan ska hålla sig till senaste projekt, tåla en trasig fil och
    /// aldrig peka på något som städats bort utanför appen.
    #[test]
    fn recent_list_keeps_the_newest_projects_and_drops_missing_files() {
        let paths = isolated_paths("recent");
        let mut entries: Vec<RecentProject> = Vec::new();

        // Tolv projekt, bara åtta ska minnas — och det senaste först.
        for i in 0..12 {
            let file = paths.project_file(&format!("Projekt {i}"));
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, minimal_project_json(&format!("Projekt {i}"))).unwrap();
            push_recent_project(&mut entries, &format!("Projekt {i}"), &file.to_string_lossy());
        }
        store_recent_projects_in(&paths, &entries);

        let loaded = load_recent_projects_in(&paths);
        assert_eq!(loaded.len(), RECENT_MAX);
        assert_eq!(loaded[0].name, "Projekt 11", "senaste först");
        assert!(
            !loaded.iter().any(|e| e.name == "Projekt 0"),
            "äldsta ska ha ramlat ur listan"
        );

        // Samma projekt igen: flyttas upp, blir inte dubbelt.
        let last_path = loaded[3].path.clone();
        let last_name = loaded[3].name.clone();
        push_recent_project(&mut entries, &last_name, &last_path);
        assert_eq!(entries.len(), RECENT_MAX, "ingen dubblett");
        assert_eq!(entries[0].path, last_path);

        // En fil som raderats utanför appen ska inte erbjudas.
        std::fs::remove_file(&last_path).unwrap();
        store_recent_projects_in(&paths, &entries);
        let reloaded = load_recent_projects_in(&paths);
        assert!(
            !reloaded.iter().any(|e| e.path == last_path),
            "borttagen fil ska filtreras bort"
        );

        // Trasig JSON får inte krascha starten — bara ge en tom lista.
        std::fs::write(paths.recent_file(), b"{ inte json").unwrap();
        assert!(load_recent_projects_in(&paths).is_empty());

        let _ = std::fs::remove_dir_all(paths.state_dir());
    }

    #[test]
    fn test_classify_all_stem_names_distinct_colors() {
        let stems = [
            ("0 Lead Vocals", "🎙", Color32::from_rgb(0, 225, 245)),
            ("1 Backing Vocals", "🗣", Color32::from_rgb(225, 75, 235)),
            ("2 Drums", "🥁", Color32::from_rgb(255, 140, 25)),
            ("3 Bass", "🎸", Color32::from_rgb(45, 225, 105)),
            ("4 Guitar", "🎸", Color32::from_rgb(255, 195, 30)),
            ("5 Keyboard", "🎹", Color32::from_rgb(65, 155, 255)),
            ("6 Percussion", "🪘", Color32::from_rgb(255, 105, 50)),
            ("7 Strings", "🎻", Color32::from_rgb(40, 220, 185)),
            ("8 Synth", "🎛", Color32::from_rgb(175, 95, 255)),
            ("9 Other", "✨", Color32::from_rgb(255, 80, 150)),
            (crate::i18n::t("🎤 Mic (Voice & Sång)"), "🎤", Color32::WHITE),
        ];

        let mut seen_colors = std::collections::HashSet::new();
        for (name, expected_icon, expected_color) in stems {
            let (_kind, icon, color) = classify_track_style(name);
            assert_eq!(icon, expected_icon, "Icon mismatch for stem: {}", name);
            assert_eq!(color, expected_color, "Color mismatch for stem: {}", name);
            assert!(seen_colors.insert((color.r(), color.g(), color.b())), "Duplicate color found for: {}", name);
        }
    }

    #[test]
    fn test_vagen_hit_project_loads_with_colors_and_mic_track() {
        // Maskinoberoende: läser projektet ur den kanoniska projektmappen om det
        // finns (hoppar över annars).
        let path = crate::paths::paths().project_file("Vägen hit");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let data: SonixProjectData = serde_json::from_str(&content).expect("Valid JSON");
            assert_eq!(data.tracks.len(), 10, "Original file has 10 tracks");

            let mut app_tracks = Vec::new();
            for st in data.tracks {
                let (kind, icon, color) = classify_track_style(&st.name);
                let mut t = PlaylistTrack::new(st.name, icon, kind, color);
                t.regions = st.regions;
                app_tracks.push(t);
            }

            // Simulate SonixApp state
            let mut app_tracks_copy = app_tracks;
            // 1. Check ensure_mic_track_exists logic
            let mic_pos = app_tracks_copy.iter().position(|t| {
                let n = t.name.to_lowercase();
                n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
            });
            assert!(mic_pos.is_none(), "Original file had no mic track");

            let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
            mic_track.volume = 1.0;
            mic_track.is_rec_armed = true;
            app_tracks_copy.push(mic_track);

            assert_eq!(app_tracks_copy.len(), 11, "Now has 11 tracks including Mic");
            assert_eq!(app_tracks_copy.last().unwrap().name, crate::i18n::t("🎤 Mic (Voice & Sång)"));
            assert_eq!(app_tracks_copy.last().unwrap().color, Color32::WHITE);

            // Re-classify colors
            for track in &mut app_tracks_copy {
                let is_mic = track.name.contains("Mic");
                if !is_mic {
                    let (_k, _icon, col) = classify_track_style(&track.name);
                    track.color = col;
                    for r in &mut track.regions {
                        r.color = col;
                    }
                }
            }

            // Verify lead vocals is Cyan
            assert_eq!(app_tracks_copy[0].color, Color32::from_rgb(0, 225, 245));
            assert_eq!(app_tracks_copy[0].regions[0].color, Color32::from_rgb(0, 225, 245));

            // Verify backing vocals is Magenta
            assert_eq!(app_tracks_copy[1].color, Color32::from_rgb(225, 75, 235));
            assert_eq!(app_tracks_copy[1].regions[0].color, Color32::from_rgb(225, 75, 235));

            // Verify drums is Electric Orange
            assert_eq!(app_tracks_copy[2].color, Color32::from_rgb(255, 140, 25));
            assert_eq!(app_tracks_copy[2].regions[0].color, Color32::from_rgb(255, 140, 25));
        }
    }

    #[test]
    fn test_sample_drag_and_drop_to_timeline_track() {
        let mut tracks = vec![
            PlaylistTrack::new(crate::i18n::t("🥁 Trummor").to_string(), "🥁", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            PlaylistTrack::new(crate::i18n::t("🎸 Bas").to_string(), "🎸", TrackKind::Bassline, Color32::from_rgb(45, 225, 105)),
        ];

        let sample_item = LibrarySampleItem {
            id: 42,
            name: "⚡ 808 Kick Impact".to_string(),
            category: "Kick".to_string(),
            icon: "🥁".to_string(),
            default_note: 36,
            color: Color32::from_rgb(255, 80, 50),
            waveform: vec![0.8, 0.6, 0.4, 0.2, 0.05],
            file_path: None,
        };

        // Add to track 0 at bar 4.5
        let region = AudioRegion {
            id: 101,
            name: sample_item.name.clone(),
            start_bar: 4.5,
            length_bars: 1.0,
            sample_offset_sec: 0.0,
            source_path: sample_item.file_path.clone(),
            waveform_peaks: sample_item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: sample_item.color,
            loop_length_bars: 0.0,
        };
        tracks[0].regions.push(region);

        assert_eq!(tracks[0].regions.len(), 1);
        assert_eq!(tracks[0].regions[0].name, "⚡ 808 Kick Impact");
        assert_eq!(tracks[0].regions[0].start_bar, 4.5);
        assert_eq!(tracks[0].regions[0].waveform_peaks.len(), 5);

        // Add to a brand new track (track index 2)
        let mut new_track = PlaylistTrack::new(
            format!("{} {}", sample_item.icon, sample_item.name),
            "🥁",
            TrackKind::CustomAudio,
            sample_item.color,
        );
        let new_reg = AudioRegion {
            id: 102,
            name: sample_item.name.clone(),
            start_bar: 8.0,
            length_bars: 1.0,
            sample_offset_sec: 0.0,
            source_path: sample_item.file_path.clone(),
            waveform_peaks: sample_item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: sample_item.color,
            loop_length_bars: 0.0,
        };
        new_track.regions.push(new_reg);
        tracks.push(new_track);

        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[2].regions.len(), 1);
        assert_eq!(tracks[2].regions[0].start_bar, 8.0);
    }

    #[test]
    fn test_vocal_studio_sample_take_isolated_properties() {
        let mut vocal_track = crate::audio::recorder::VocalStudioTrack::default();
        vocal_track.takes.clear();
        let dummy_pcm = vec![0.5, -0.5, 0.8, -0.8, 0.3, -0.3];
        let take_idx = vocal_track.load_sample_or_region_as_take("Lead Hook", dummy_pcm, 44100, Color32::from_rgb(0, 200, 240));

        assert_eq!(take_idx, 0);
        assert_eq!(vocal_track.takes.len(), 1);
        assert_eq!(vocal_track.takes[0].name, "Lead Hook");
        assert_eq!(vocal_track.takes[0].gain_linear, 1.0);
        assert_eq!(vocal_track.takes[0].time_stretch, 1.0);
        assert_eq!(vocal_track.takes[0].pitch_semitones, 0.0);
        assert_eq!(vocal_track.takes[0].is_reverse, false);

        // Modify sound shaping parameters
        vocal_track.takes[0].gain_linear = 1.25;
        vocal_track.takes[0].time_stretch = 1.5;
        vocal_track.takes[0].pitch_semitones = 3.0;
        vocal_track.takes[0].is_reverse = true;

        assert_eq!(vocal_track.takes[0].gain_linear, 1.25);
        assert_eq!(vocal_track.takes[0].time_stretch, 1.5);
        assert_eq!(vocal_track.takes[0].pitch_semitones, 3.0);
        assert_eq!(vocal_track.takes[0].is_reverse, true);
    }

    #[test]
    fn test_automation_value_at_interpolates() {
        let lane = AutomationLane {
            param: AutomationParam::Volume,
            enabled: true,
            points: vec![
                AutomationPoint { time_secs: 0.0, value: 0.0 },
                AutomationPoint { time_secs: 10.0, value: 1.0 },
            ],
        };
        assert_eq!(lane.value_at(-1.0), Some(0.0));
        assert_eq!(lane.value_at(0.0), Some(0.0));
        assert_eq!(lane.value_at(5.0), Some(0.5));
        assert_eq!(lane.value_at(10.0), Some(1.0));
        assert_eq!(lane.value_at(20.0), Some(1.0));

        let empty = AutomationLane {
            param: AutomationParam::Pan,
            enabled: true,
            points: Vec::new(),
        };
        assert_eq!(empty.value_at(1.0), None);
    }

    #[test]
    fn test_automation_lane_serde_roundtrip() {
        let lane = AutomationLane {
            param: AutomationParam::ReverbSend,
            enabled: true,
            points: vec![
                AutomationPoint { time_secs: 1.5, value: 0.25 },
                AutomationPoint { time_secs: 4.0, value: 0.9 },
            ],
        };
        let json = serde_json::to_string(&lane).unwrap();
        let back: AutomationLane = serde_json::from_str(&json).unwrap();
        assert_eq!(back.param, AutomationParam::ReverbSend);
        assert!(back.enabled);
        assert_eq!(back.points.len(), 2);
        assert!((back.value_at(2.0).unwrap() - 0.38).abs() < 1e-4);
    }

    #[test]
    fn test_automation_param_ranges_and_index() {
        assert_eq!(AutomationParam::Volume.index(), 0);
        assert_eq!(AutomationParam::Pan.index(), 1);
        assert_eq!(AutomationParam::ReverbSend.index(), 2);
        assert_eq!(AutomationParam::DelaySend.index(), 3);
        assert_eq!(AutomationParam::Volume.range(), (0.0, 1.5));
        assert_eq!(AutomationParam::Pan.range(), (-1.0, 1.0));
        assert_eq!(AutomationParam::DelaySend.range(), (0.0, 1.0));
    }

    #[test]
    fn test_plugin_slots_survive_project_json_roundtrip() {
        let data = SonixProjectData {
            name: "Plugin Test".into(),
            bpm: 120.0,
            tempo_points: Vec::new(),
            swing: 0.0,
            master_volume: 1.0,
            master_pan: 0.0,
            tracks: Vec::new(),
            plugin_slots: vec![
                None,
                Some(SavedPluginData {
                    path: "/plugins/Gain.clap".into(),
                    name: "Gain".into(),
                    state: vec![0, 1, 2, 250, 255],
                    sandboxed: false,
                }),
            ],
            bus_volume: default_bus_volume(),
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: default_vca_volume(),
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
            patterns: Vec::new(),
            selected_pattern: 0,
            step_velocities: None,
            channels: Vec::new(),
        };
        let json = serde_json::to_string(&data).unwrap();
        let back: SonixProjectData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plugin_slots.len(), 2);
        assert!(back.plugin_slots[0].is_none());
        let slot = back.plugin_slots[1].as_ref().unwrap();
        assert_eq!(slot.path, "/plugins/Gain.clap");
        assert_eq!(slot.name, "Gain");
        assert_eq!(slot.state, vec![0, 1, 2, 250, 255]);
    }

    /// Beviset för att hålet i 6.3 är stängt: en låt på fyra takter skrivs ut,
    /// läses tillbaka och delas upp per takt — och **varje not finns kvar i rätt
    /// takt**. Före det här var importen en första-takt-import som räknade in
    /// resten och kastade den.
    #[test]
    fn an_exported_song_comes_back_bar_by_bar_without_losing_a_note() {
        use crate::audio::smf::{parse_midi, write_midi, MidiNote, MidiTrack};
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        let bar_ticks = step * STEPS_PER_BAR as u32;

        // Fyra takter: en ton på steg 0 och ett trumslag på steg 8 i varje.
        let mut notes = Vec::new();
        for bar in 0..4u32 {
            notes.push(MidiNote {
                start: bar * bar_ticks,
                length: step / 2,
                channel: 0,
                key: 60 + bar as u8,
                velocity: 100,
            });
            notes.push(MidiNote {
                start: bar * bar_ticks + 8 * step,
                length: step / 2,
                channel: 9,
                key: 36,
                velocity: 96,
            });
        }
        let bytes = write_midi(
            120.0,
            &[MidiTrack {
                name: "Fyra takter".to_string(),
                notes: notes.clone(),
            }],
        );
        let parsed = parse_midi(&bytes).expect("egen fil ska gå att läsa");
        let back: Vec<MidiNote> = parsed
            .notes_with_track()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect();
        assert_eq!(back.len(), notes.len(), "alla noter ska komma tillbaka ur filen");

        let groups = notes_by_bar(&back, parsed.ppq);
        assert_eq!(groups.len(), 4, "fyra takter ska bli fyra grupper, inte en");
        assert_eq!(
            groups.iter().map(|(_, n)| n.len()).sum::<usize>(),
            notes.len(),
            "ingen not får tappas i uppdelningen"
        );

        for (bar, bar_notes) in &groups {
            let mut pat = test_pattern();
            let rep = apply_bar_to_pattern(bar_notes, parsed.ppq, &mut pat);
            assert_eq!(
                rep.notes_placed + rep.dropped_out_of_range,
                bar_notes.len(),
                "takt {bar} ska redovisas helt"
            );
            assert_eq!(rep.dropped_out_of_range, 0, "takt {bar} ska inte tappa något");
            let row = (60 + *bar as u8 - 48) as usize;
            assert!(
                pat.piano_roll_grid[row][0],
                "tonen i takt {bar} ska ligga på steg 0"
            );
            assert!(pat.channel_steps[0][8], "bastrumman i takt {bar} ska ligga på steg 8");
            assert_eq!(pat.channel_notes[0][8], 36);
        }
    }

    /// Notlängden följer med in (den var ett eget hål i 6.3): en ton som håller
    /// över ett steg tänder alla steg den klingar igenom, medan ett trumslag
    /// bara tänder sitt eget — ett slag är ett slag.
    #[test]
    fn a_held_note_fills_the_steps_it_sounds_but_a_drum_hit_does_not() {
        use crate::audio::smf::MidiNote;
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        let notes = vec![
            // Ton som håller i tre steg från steg 4.
            MidiNote { start: 4 * step, length: 3 * step, channel: 0, key: 64, velocity: 100 },
            // Trumslag med (orimligt) lång not: ska ändå bara tända sitt steg.
            MidiNote { start: 2 * step, length: 8 * step, channel: 9, key: 38, velocity: 100 },
        ];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, crate::audio::smf::PPQ, &mut pat);
        assert_eq!(rep.notes_placed, 2);
        let row = (64 - 48) as usize;
        for s in 4..7 {
            assert!(pat.piano_roll_grid[row][s], "tonen ska tona i steg {s}");
        }
        assert!(!pat.piano_roll_grid[row][7], "och tystna efter sin längd");
        assert!(pat.channel_steps[1][2], "virveln ska ligga på steg 2");
        assert!(
            !pat.channel_steps[1][3],
            "ett slag får inte bli flera för att noten är lång"
        );
    }

    /// Uppdelningen är ren och ska tåla kanter: inga noter, en not, och noter
    /// långt bortom arrangemanget.
    #[test]
    fn notes_by_bar_handles_the_edges() {
        use crate::audio::smf::MidiNote;
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        assert!(notes_by_bar(&[], crate::audio::smf::PPQ).is_empty());

        let notes = vec![
            MidiNote { start: 0, length: step, channel: 0, key: 60, velocity: 100 },
            // Takt 40 — utanför arrangemangets 32 takter.
            MidiNote {
                start: 40 * STEPS_PER_BAR as u32 * step,
                length: step,
                channel: 0,
                key: 62,
                velocity: 100,
            },
        ];
        let groups = notes_by_bar(&notes, crate::audio::smf::PPQ);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, 0);
        assert_eq!(groups[1].0, 40);
        assert!(
            groups[1].0 >= ARRANGEMENT_BARS,
            "takten utanför arrangemanget ska gå att upptäcka och redovisas"
        );
    }

    /// Tempobyten ska överleva en tur genom projektfilen — och en gammal fil utan
    /// fältet ska läsas som ett enda tempo, exakt som den skrevs.
    ///
    /// Det andra är det viktigaste: `#[serde(default)]` är hela skälet till att
    /// fältet kan läggas till utan att röra en enda befintlig projektfil.
    #[test]
    fn tempo_points_survive_the_project_file() {
        let with_points = r#"{"name":"x","bpm":120.0,"swing":0.0,"master_volume":1.0,
            "master_pan":0.0,"tracks":[],
            "tempo_points":[{"start_bar":0,"bpm":120.0},{"start_bar":8,"bpm":90.0}]}"#;
        let data: SonixProjectData =
            serde_json::from_str(with_points).expect("fil med tempobyten ska gå att läsa");
        assert_eq!(data.tempo_points.len(), 2, "båda punkterna ska med");
        assert_eq!(data.tempo_points[1].start_bar, 8);

        // Kartan som byggs ur dem svarar rätt på båda sidor om bytet.
        let map = crate::audio::tempo::TempoMap::from_points(data.tempo_points.clone());
        assert!((map.bpm_at(4.0) - 120.0).abs() < 0.01, "före bytet");
        assert!((map.bpm_at(9.0) - 90.0).abs() < 0.01, "efter bytet");

        // En gammal projektfil har inte fältet alls.
        let old = r#"{"name":"x","bpm":128.0,"swing":0.0,"master_volume":1.0,
            "master_pan":0.0,"tracks":[]}"#;
        let data: SonixProjectData =
            serde_json::from_str(old).expect("gammal projektfil ska fortfarande gå att läsa");
        assert!(
            data.tempo_points.is_empty(),
            "en gammal fil har inga byten — då gäller bpm som förut"
        );
        assert!((data.bpm - 128.0).abs() < 0.01, "och tempot ska vara kvar");
    }

    #[test]
    fn a_frozen_track_plays_its_audio_and_not_its_patterns() {
        let mut track =
            PlaylistTrack::new("Trummor".to_string(), "🥁", TrackKind::Drums, Color32::BLACK);
        track.clips[0] = Some(1);
        track.clips[4] = Some(1);
        assert_eq!(
            render_clips_for(&track)[0],
            Some(1),
            "ett ofruset spår ska trigga sina klipp"
        );

        track.frozen = Some(FrozenTrack {
            path: "/tmp/trummor-0.wav".to_string(),
            digest: 7,
            stamp: 1_700_000_000,
        });
        assert!(
            render_clips_for(&track).iter().all(|c| c.is_none()),
            "ett fruset spår ska vara tyst i pattern-vägen — annars hörs det två gånger"
        );
    }

    #[test]
    fn a_frozen_track_is_left_out_of_a_pattern_render() {
        let mut frozen =
            PlaylistTrack::new("Trummor".to_string(), "🥁", TrackKind::Drums, Color32::BLACK);
        frozen.frozen = Some(FrozenTrack {
            path: "/tmp/x.wav".to_string(),
            digest: 1,
            stamp: 2,
        });
        frozen.frozen_pcm = Some((std::sync::Arc::new(vec![0.0; 8]), std::sync::Arc::new(vec![0.0; 8]), 44_100));

        // Sång-läget: spåret hörs som ljud, och det ska med.
        assert!(frozen_audio_in_render(&frozen, false));
        // Pattern-läget renderar kanalracket — spårets frysta ljud ska inte med.
        assert!(
            !frozen_audio_in_render(&frozen, true),
            "ett fruset spår får inte hamna i en pattern-export"
        );
        // Ett ofruset spår påverkas inte av läget (det har inget fryst ljud).
        let plain = PlaylistTrack::new("Bas".to_string(), "🎸", TrackKind::Bassline, Color32::BLACK);
        assert!(frozen_audio_in_render(&plain, true));
        assert!(frozen_audio_in_render(&plain, false));
    }

    #[test]
    fn only_pattern_tracks_can_be_frozen() {
        for kind in [TrackKind::Drums, TrackKind::SynthLead, TrackKind::Bassline] {
            let track = PlaylistTrack::new("Spår".to_string(), "🎛", kind, Color32::BLACK);
            assert!(track.can_freeze(), "{kind:?} spelas av patterns och kan frysas");
        }
        // Ett ljudspår ligger redan som färdigt ljud — det finns inget att tjäna.
        let audio = PlaylistTrack::new("Sång".to_string(), "🎤", TrackKind::VocalAudio, Color32::BLACK);
        assert!(!audio.can_freeze());
        assert!(!audio.is_frozen());
    }

    #[test]
    fn the_frozen_region_covers_the_whole_file() {
        // 1,5 sekunder vid 44100 Hz.
        let left = vec![0.25f32; 66_150];
        let region = frozen_region(&left, 44_100);
        assert!(
            (region.length_secs - 1.5).abs() < 1e-3,
            "längden ska räknas ur bufferten, fick {}",
            region.length_secs
        );
        assert_eq!(region.start_time_secs, 0.0);
        assert_eq!(region.gain, 1.0);
        assert!(!region.muted);
        assert!((frozen_region(&left, 0).length_secs - 66_150.0).abs() < 1.0, "noll samplingsfrekvens får inte ge oändlig längd");
    }

    #[test]
    fn a_frozen_track_survives_the_project_file() {
        let saved = SavedFrozenTrack {
            path: "/home/x/Music/Sonix/Projects/Beat/Frozen/trummor-0.wav".to_string(),
            digest: 42,
            stamp: 1_700_000_000,
        };
        let json = serde_json::to_string(&saved).unwrap();
        let back: SavedFrozenTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(back, saved);
    }

    #[test]
    fn an_old_project_file_without_a_frozen_field_loads_as_unfrozen() {
        // Exakt den form en fil hade innan fältet fanns: allt annat har defaults.
        let json = r#"{
            "name": "Trummor",
            "volume": 0.8,
            "pan": 0.0,
            "muted": false,
            "solo": false,
            "clips": [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,
                      null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],
            "regions": []
        }"#;
        let track: SavedTrackData = serde_json::from_str(json).expect("äldre fil ska läsas");
        assert!(track.frozen.is_none(), "utan fältet är spåret ofrusat");
        assert_eq!(track.name, "Trummor");
    }

    #[test]
    fn test_old_projects_without_plugin_slots_still_load() {
        let legacy = r#"{
            "name": "Legacy",
            "bpm": 100.0,
            "swing": 0.0,
            "master_volume": 1.0,
            "master_pan": 0.0,
            "tracks": []
        }"#;
        let data: SonixProjectData = serde_json::from_str(legacy).unwrap();
        assert!(data.plugin_slots.is_empty());
    }
}
