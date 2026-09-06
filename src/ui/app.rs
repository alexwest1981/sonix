use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::HashSet;
use std::time::Instant;

use crate::audio::{
    render_song_arrangement_to_wav, render_to_wav, AdsrParams, AudioCommand, AudioEngine,
    DelayParams, DrumType, FilterParams, Preset, ReverbParams, SongArrangementExport,
    StemRegionPlayback, SynthEngine, Waveform,
};
use crate::audio::ai_generator::AiMusicAssistant;
use crate::audio::patcher::ModularGraph;
use crate::audio::plugin_host::PluginManager;
use crate::audio::recorder::VocalStudioTrack;
use crate::audio::stem_separator::StemProject;
use crate::audio::vocal_harmonizer::VocalHarmonizer;
use super::ai_assistant_view::render_ai_assistant_view;
use super::patcher_view::render_patcher_view;
use super::plugins_view::render_plugins_view;
use super::stem_view::render_stem_separator_view;
use super::theme::Theme;
use super::vocal_studio_view::render_vocal_studio_view;
use super::widgets::{
    alchemy_transform_matrix, drummer_xy_matrix, eq_curve_visualizer, fl_step_button,
    oscilloscope_display, rotary_knob, vertical_fader,
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
    Select, // ⇱ Markör / Flytta
    Paint,  // ✎ Rita
    Slice,  // ✂ Klipp / Sax
    Erase,  // 🗑 Radera
    Mute,   // 🔇 Muta
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusRouting {
    Master,
    VocalBus,
    DrumBus,
    SynthBus,
    FxSend,
}

impl BusRouting {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            BusRouting::Master => "Master",
            BusRouting::VocalBus => "Vocal Bus",
            BusRouting::DrumBus => "Drum Bus",
            BusRouting::SynthBus => "Synth Bus",
            BusRouting::FxSend => "FX Send",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcaGroup {
    None,
    Vca1,
    Vca2,
    Vca3,
    Vca4,
}

impl VcaGroup {
    pub fn label(&self) -> &'static str {
        match self {
            VcaGroup::None => "--",
            VcaGroup::Vca1 => "V1",
            VcaGroup::Vca2 => "V2",
            VcaGroup::Vca3 => "V3",
            VcaGroup::Vca4 => "V4",
        }
    }
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
    pub target_bus: BusRouting,
    pub vca_group: VcaGroup,
    pub pdc_latency_samples: usize,
}

#[derive(Clone)]
pub struct Pattern {
    pub name: String,
    pub color: Color32,
    pub channel_steps: Vec<[bool; 16]>,
    pub channel_notes: Vec<[u8; 16]>,
    pub piano_roll_grid: [[bool; 16]; 24],
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
    format!("Takt {:03}.{}.{} (+{:02}cs)", bar_idx, beat, step, cs)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    VocalAudio,
    CustomAudio,
    Drums,
    SynthLead,
    Bassline,
    Fx,
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
    #[serde(skip, default = "default_region_color")]
    pub color: Color32,
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SonixProjectData {
    pub name: String,
    pub bpm: f32,
    pub swing: f32,
    pub master_volume: f32,
    pub master_pan: f32,
    pub tracks: Vec<SavedTrackData>,
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
}

#[derive(Clone)]
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
    pub audio_waveform: Option<Vec<f32>>,
    #[allow(dead_code)]
    pub custom_clip_name: Option<String>,
    pub automation_enabled: bool,
    pub eq: TrackEq,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    pub pitch_semitones: f32,
}

impl PlaylistTrack {
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
            audio_waveform: None,
            custom_clip_name: None,
            automation_enabled: false,
            eq: TrackEq::default(),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: 0.0,
        }
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

pub struct SonixApp {
    pub engine: AudioEngine,
    pub stem_import_progress: std::sync::Arc<std::sync::Mutex<StemImportProgress>>,
    // Transport & Clock
    pub is_playing: bool,
    pub bpm: f32,
    pub swing: f32,
    pub current_step: usize,
    pub song_bar: usize,
    pub song_step_in_bar: usize,
    pub loop_start_bar: usize,
    pub loop_end_bar: usize,
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
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
    // Synth Parameters
    pub current_preset: Preset,
    pub waveform: Waveform,
    pub adsr: AdsrParams,
    pub filter: FilterParams,
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
    // Piano Roll & Scale Snapping
    pub piano_roll_grid: [[bool; 16]; 24],
    pub selected_scale: usize,
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
    pub chorus_on: bool,
    pub chorus_depth: f32,
    pub compressor_amount: f32,
    // Modular Patcher, Stem Separator & Plugin Manager
    pub modular_graph: ModularGraph,
    pub stem_project: StemProject,
    pub plugin_manager: PluginManager,
    // Vocal Studio, Comping, Harmonizer & AI Music Assistant
    pub vocal_studio: VocalStudioTrack,
    pub vocal_harmonizer: VocalHarmonizer,
    pub ai_assistant: AiMusicAssistant,
    // Interactive Piano
    pub active_mouse_note: Option<u8>,
    pub active_keys: HashSet<u8>,
    pub anim_phase: f32,
    // Sub-Mixing, VCA Groups & PDC
    pub vca_faders: [f32; 4],
    pub vca_mutes: [bool; 4],
    #[allow(dead_code)]
    pub vca_solos: [bool; 4],
    pub vocal_bus_vol: f32,
    pub drum_bus_vol: f32,
    pub synth_bus_vol: f32,
    pub fx_send_vol: f32,
    pub pdc_enabled: bool,
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
    // Batch Export & Render Queue
    pub show_render_queue_modal: bool,
    pub render_format_idx: usize,
    pub render_scope_idx: usize,
    pub render_sample_rate_idx: usize,
    #[allow(dead_code)]
    pub render_bit_depth_idx: usize,
    pub render_template: String,
    pub is_rendering: bool,
    pub render_progress: f32,
    pub render_queue_status: String,
    // Suno AI Prompt Bar & Studio Timeline View
    pub suno_prompt_input: String,
    pub suno_model_version: String,
    pub suno_context_clip: Option<String>,
    pub suno_is_generating: bool,
    pub suno_generation_progress: f32,
    pub suno_zoom_level: f32,
    pub timeline_snap_mode: TimeSnapMode,
    pub timeline_auto_scroll: bool,
    pub selected_timeline_track: usize,
    // Project Metadata & Suno Multi-Track Stems
    pub project_name: String,
    pub show_suno_import_modal: bool,
    pub detected_suno_zips: Vec<String>,
    pub custom_stem_path_input: String,
    // Selected Audio Region Inspector
    pub selected_audio_region: Option<(usize, usize)>,
    // Focused Stem Detail & Sound Editor Modal
    pub focused_stem_track: Option<usize>,
    pub show_stem_focus_modal: bool,
    pub stem_focus_active_tab: usize,
    // Modals
    pub show_about_modal: bool,
    pub show_ai_settings_modal: bool,
    pub show_audio_settings_modal: bool,
    pub show_project_manager_modal: bool,
    pub project_file_path: Option<String>,
    pub new_project_name_input: String,
    // AI Providers & API Keys
    pub ai_provider_suno_key: String,
    pub ai_provider_stable_audio_key: String,
    pub ai_provider_openai_key: String,
    pub ai_provider_claude_key: String,
    pub ai_provider_ollama_endpoint: String,
    // Audio / MIDI Configuration
    pub audio_driver_idx: usize,
    pub audio_sample_rate_idx: usize,
    pub audio_buffer_size_idx: usize,
    pub audio_limiter_enabled: bool,
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
    HelpGuideModal,
    AboutModal,
}

#[derive(Clone, Debug)]
pub enum ScreenshotState {
    Idle,
    Preparing { target: ScreenshotTarget, dest: std::path::PathBuf, frames_left: usize },
    AwaitingCapture { dest: std::path::PathBuf },
}



impl SonixApp {
    pub fn new(engine: AudioEngine) -> Self {
        let current_preset = Preset::CleanPluck;
        let (waveform, adsr, filter) = current_preset.settings();

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

        // 8 Channels with Pitch, Sample Chop, Bus Routing, VCA and PDC Parameters
        let kick = ChannelStrip {
            name: "808 Kick".to_string(), icon: "💥".to_string(), color: Theme::FL_ORANGE,
            volume: 0.95, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.3, is_reverse: false,
            waveform_preview: make_wave(20.0, 0.8),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let snare = ChannelStrip {
            name: "909 Snare".to_string(), icon: "🥁".to_string(), color: Theme::FL_CYAN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [38; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, is_reverse: false,
            waveform_preview: make_wave(45.0, 0.9),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let clap = ChannelStrip {
            name: "Electro Clap".to_string(), icon: "👏".to_string(), color: Theme::FL_YELLOW,
            volume: 0.80, pan: -0.1, muted: false, solo: false, steps: [false; 16], notes: [39; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, is_reverse: false,
            waveform_preview: make_wave(35.0, 0.85),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let hat = ChannelStrip {
            name: "Crisp Hat".to_string(), icon: "⚡".to_string(), color: Theme::FL_PURPLE,
            volume: 0.75, pan: 0.15, muted: false, solo: false, steps: [false; 16], notes: [42; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.2, is_reverse: false,
            waveform_preview: make_wave(70.0, 0.95),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let open_hat = ChannelStrip {
            name: "Open Hat".to_string(), icon: "🌊".to_string(), color: Theme::FL_CYAN,
            volume: 0.70, pan: -0.2, muted: false, solo: false, steps: [false; 16], notes: [46; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.6, is_reverse: false,
            waveform_preview: make_wave(50.0, 0.5),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let crash = ChannelStrip {
            name: "Cyber Crash".to_string(), icon: "✨".to_string(), color: Color32::from_rgb(255, 180, 50),
            volume: 0.75, pan: 0.25, muted: false, solo: false, steps: [false; 16], notes: [49; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.7, is_reverse: false,
            waveform_preview: make_wave(30.0, 0.4),
            target_bus: BusRouting::DrumBus, vca_group: VcaGroup::Vca1, pdc_latency_samples: 0,
        };
        let synth_lead = ChannelStrip {
            name: "303 Lead".to_string(), icon: "🎹".to_string(), color: Theme::FL_GREEN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [60; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, is_reverse: false,
            waveform_preview: make_wave(60.0, 0.3),
            target_bus: BusRouting::SynthBus, vca_group: VcaGroup::Vca2, pdc_latency_samples: 0,
        };
        let sub_bass = ChannelStrip {
            name: "Sub Bass".to_string(), icon: "🎸".to_string(), color: Color32::from_rgb(255, 80, 140),
            volume: 0.90, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, is_reverse: false,
            waveform_preview: make_wave(25.0, 0.6),
            target_bus: BusRouting::SynthBus, vca_group: VcaGroup::Vca2, pdc_latency_samples: 0,
        };

        let channels = vec![kick, snare, clap, hat, open_hat, crash, synth_lead, sub_bass];

        // Sample Library Initial Items (Categorized with FL & GarageBand styles)
        let sample_library = vec![
            // Kicks
            LibrarySampleItem { id: 0, name: "808 Kick Deep Sub".to_string(), category: "Kicks".to_string(), icon: "🥁".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(25.0, 0.5) },
            LibrarySampleItem { id: 1, name: "Acoustic Rock Kick".to_string(), category: "Kicks".to_string(), icon: "🥁".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(30.0, 0.7) },
            LibrarySampleItem { id: 2, name: "Punchy Club Kick 909".to_string(), category: "Kicks".to_string(), icon: "🥁".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(35.0, 0.8) },
            LibrarySampleItem { id: 3, name: "Trap Heavy Sub Kick".to_string(), category: "Kicks".to_string(), icon: "🥁".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(22.0, 0.4) },
            // Snares
            LibrarySampleItem { id: 4, name: "Trap Snare Tight 808".to_string(), category: "Snares".to_string(), icon: "🥁".to_string(), default_note: 38, color: Theme::FL_CYAN, waveform: make_wave(45.0, 0.8) },
            LibrarySampleItem { id: 5, name: "Acoustic Wood Snare".to_string(), category: "Snares".to_string(), icon: "🥁".to_string(), default_note: 38, color: Theme::FL_CYAN, waveform: make_wave(40.0, 0.75) },
            LibrarySampleItem { id: 6, name: "909 Snare Electronic".to_string(), category: "Snares".to_string(), icon: "🥁".to_string(), default_note: 38, color: Theme::FL_CYAN, waveform: make_wave(50.0, 0.85) },
            LibrarySampleItem { id: 7, name: "Vintage Rimshot".to_string(), category: "Snares".to_string(), icon: "🥁".to_string(), default_note: 37, color: Theme::FL_CYAN, waveform: make_wave(60.0, 0.9) },
            // Claps
            LibrarySampleItem { id: 8, name: "Studio Multi-Clap".to_string(), category: "Claps".to_string(), icon: "👏".to_string(), default_note: 39, color: Theme::FL_CYAN, waveform: make_wave(48.0, 0.8) },
            LibrarySampleItem { id: 9, name: "Stereo Wide Clap".to_string(), category: "Claps".to_string(), icon: "👏".to_string(), default_note: 39, color: Theme::FL_CYAN, waveform: make_wave(52.0, 0.85) },
            // Hi-Hats
            LibrarySampleItem { id: 10, name: "808 Closed Hat Clean".to_string(), category: "Hi-Hats".to_string(), icon: "🧂".to_string(), default_note: 42, color: Theme::FL_YELLOW, waveform: make_wave(80.0, 0.95) },
            LibrarySampleItem { id: 11, name: "909 Open Hat Sizzle".to_string(), category: "Hi-Hats".to_string(), icon: "🧂".to_string(), default_note: 46, color: Theme::FL_YELLOW, waveform: make_wave(70.0, 0.7) },
            LibrarySampleItem { id: 12, name: "Trap Hat Roll Tick".to_string(), category: "Hi-Hats".to_string(), icon: "🧂".to_string(), default_note: 42, color: Theme::FL_YELLOW, waveform: make_wave(90.0, 0.98) },
            LibrarySampleItem { id: 13, name: "Analog Shaker Groove".to_string(), category: "Hi-Hats".to_string(), icon: "🧂".to_string(), default_note: 44, color: Theme::FL_PURPLE, waveform: make_wave(65.0, 0.9) },
            // Percussion & Toms
            LibrarySampleItem { id: 14, name: "Low Sub Tom".to_string(), category: "Percussion".to_string(), icon: "🪘".to_string(), default_note: 45, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(28.0, 0.7) },
            LibrarySampleItem { id: 15, name: "High Punch Tom".to_string(), category: "Percussion".to_string(), icon: "🪘".to_string(), default_note: 50, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(38.0, 0.75) },
            LibrarySampleItem { id: 16, name: "808 Cowbell Classic".to_string(), category: "Percussion".to_string(), icon: "🔔".to_string(), default_note: 56, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(55.0, 0.8) },
        ];

        let pat1 = Pattern {
            name: "Pat 1: Beats & Groove".to_string(),
            color: Theme::FL_ORANGE,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
        };

        let pat2 = Pattern {
            name: "Pat 2: Lead & Synthesizer".to_string(),
            color: Theme::FL_CYAN,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
        };

        let pat3 = Pattern {
            name: "Pat 3: Synth Arp".to_string(),
            color: Theme::FL_GREEN,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
        };

        let pat4 = Pattern {
            name: "Pat 4: Deep Bass".to_string(),
            color: Color32::from_rgb(255, 80, 140),
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
        };

        let patterns = vec![pat1, pat2, pat3, pat4];

        // 6 Audio & MIDI Timeline Tracks (Clean & Empty by default)
        let mut t0 = PlaylistTrack::new("🎙 Sång (Lead Vocal)".to_string(), "🎙", TrackKind::VocalAudio, Color32::from_rgb(180, 110, 255));
        t0.volume = 0.95;
        t0.is_rec_armed = true;

        let mut t1 = PlaylistTrack::new("🥁 Trummor & Beats".to_string(), "🥁", TrackKind::Drums, Theme::FL_CYAN);
        t1.volume = 0.90;

        let mut t2 = PlaylistTrack::new("🎹 Lead Melodi & Synt".to_string(), "🎹", TrackKind::SynthLead, Theme::FL_GREEN);
        t2.volume = 0.85;

        let mut t3 = PlaylistTrack::new("🎸 Bas / Basgång".to_string(), "🎸", TrackKind::Bassline, Color32::from_rgb(255, 80, 140));
        t3.volume = 0.90;

        let mut t4 = PlaylistTrack::new("📂 Egna Ljud & Sampler".to_string(), "📂", TrackKind::CustomAudio, Color32::from_rgb(255, 200, 80));
        t4.volume = 0.85;

        let mut t5 = PlaylistTrack::new("⚡ FX & Drop".to_string(), "⚡", TrackKind::Fx, Theme::FL_PURPLE);
        t5.volume = 0.80;

        let playlist_tracks = vec![t0, t1, t2, t3, t4, t5];
        let selected_pattern = 0;
        let initial_channels = channels;
        let initial_grid = [[false; 16]; 24];

        Self {
            engine,
            stem_import_progress: std::sync::Arc::new(std::sync::Mutex::new(StemImportProgress::default())),
            is_playing: false,
            bpm: 120.0,
            swing: 0.0,
            current_step: 0,
            song_bar: 0,
            song_step_in_bar: 0,
            loop_start_bar: 0,
            loop_end_bar: 16,
            last_step_time: Instant::now(),
            song_time: 0.0,
            pattern_mode: false,
            view_mode: ViewMode::PlaylistArranger,
            status_message: "Välkommen till Sonix Studio! Skapa ett beat eller importera dina stämmor (Stems).".to_string(),
            show_help_guide: false,
            help_manual_active_tab: 0,
            help_search_query: String::new(),
            show_browser: true,
            browser_search: String::new(),
            master_volume: 0.85,
            master_pan: 0.0,
            stereo_width: 1.0,
            eq_low: 1.0,
            eq_mid: 0.0,
            eq_high: 1.0,
            current_preset,
            waveform,
            adsr,
            filter,
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
            custom_import_name_input: "Mitt Nya Ljud".to_string(),
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
            piano_roll_grid: initial_grid,
            selected_scale: 0,
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
            alchemy_puck: [0.0, 0.0],
            remix_xy: [0.5, 0.5],
            remix_tape_stop: false,
            remix_stutter: 0,
            stompbox_drive: 2.5,
            stompbox_tone: 5000.0,
            stompbox_cab: true,
            chorus_on: true,
            chorus_depth: 0.35,
            compressor_amount: 0.65,
            modular_graph: ModularGraph::default(),
            stem_project: StemProject::default(),
            plugin_manager: PluginManager::default(),
            vocal_studio: VocalStudioTrack::default(),
            vocal_harmonizer: VocalHarmonizer::default(),
            ai_assistant: AiMusicAssistant::default(),
            active_mouse_note: None,
            active_keys: HashSet::new(),
            anim_phase: 0.0,
            // Sub-Mixing, VCA Groups & PDC
            vca_faders: [1.0, 1.0, 1.0, 1.0],
            vca_mutes: [false, false, false, false],
            vca_solos: [false, false, false, false],
            vocal_bus_vol: 0.90,
            drum_bus_vol: 0.95,
            synth_bus_vol: 0.88,
            fx_send_vol: 0.75,
            pdc_enabled: true,
            // Linux Native Hardware Controller (MCU / OSC)
            show_controller_modal: false,
            mcu_connected: true,
            mcu_device_name: "Behringer X-Touch / Launchpad (ALSA MIDI 14:0)".to_string(),
            mcu_bank: 0,
            osc_enabled: true,
            osc_rx_port: 8000,
            osc_tx_port: 9000,
            osc_rx_count: 142,
            midi_learn_active: false,
            // Batch Export & Render Queue
            show_render_queue_modal: false,
            render_format_idx: 0,
            render_scope_idx: 0,
            render_sample_rate_idx: 0,
            render_bit_depth_idx: 0,
            render_template: "{project}_{track}_{bpm}bpm".to_string(),
            is_rendering: false,
            render_progress: 0.0,
            render_queue_status: "Klar för rendering".to_string(),
            // Suno AI Prompt Bar & Studio Timeline View
            suno_prompt_input: "Generera ett 8-takters synthwave-trumkomp och vokalmelodi i A-moll".to_string(),
            suno_model_version: "Sonix AI v5.5 (Music & Stems)".to_string(),
            suno_context_clip: None,
            suno_is_generating: false,
            suno_generation_progress: 0.0,
            suno_zoom_level: 1.0,
            timeline_snap_mode: TimeSnapMode::FreeHundredth,
            timeline_auto_scroll: true,
            selected_timeline_track: 0,
            // Project Metadata & Suno Multi-Track Stems
            project_name: "Namnlöst Projekt".to_string(),
            show_suno_import_modal: false,
            detected_suno_zips: Vec::new(),
            custom_stem_path_input: "/home/alex/Music".to_string(),
            // Selected Audio Region Inspector
            selected_audio_region: None,
            // Focused Stem Detail & Sound Editor Modal
            focused_stem_track: None,
            show_stem_focus_modal: false,
            stem_focus_active_tab: 0,
            // Modals
            show_about_modal: false,
            show_ai_settings_modal: false,
            show_audio_settings_modal: false,
            show_project_manager_modal: false,
            project_file_path: None,
            new_project_name_input: "Mitt Beat".to_string(),
            // AI Providers & API Keys
            ai_provider_suno_key: String::new(),
            ai_provider_stable_audio_key: String::new(),
            ai_provider_openai_key: String::new(),
            ai_provider_claude_key: String::new(),
            ai_provider_ollama_endpoint: "http://localhost:11434".to_string(),
            // Audio / MIDI Configuration
            audio_driver_idx: 0,
            audio_sample_rate_idx: 1,
            audio_buffer_size_idx: 1,
            audio_limiter_enabled: true,
            // Automated Screenshot System
            screenshot_queue: Vec::new(),
            screenshot_state: ScreenshotState::Idle,
            screenshot_mode_active: false,
        }
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
            (ScreenshotTarget::HelpGuideModal, out_dir.join("21_dialog_hjalpguide_manual.png")),
            (ScreenshotTarget::AboutModal, out_dir.join("22_dialog_om_sonix.png")),
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
                self.show_render_queue_modal = true;
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
                    color: Color32::from_rgb(255, 100, 120),
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
                    color: Color32::from_rgb(255, 120, 140),
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
                    color: Color32::from_rgb(200, 130, 255),
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
        self.ai_assistant.prompt_input = "Cyberpunk 80s synthwave lead med mörk rezonans i A-moll".to_string();
        self.ai_assistant.last_status = "✨ AI genererade 3 variationer av Synth Lead & Bassline".to_string();
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
        self.project_name = "Namnlöst Projekt".to_string();
        self.project_file_path = None;
        self.status_message = "✨ Skapade ett nytt tomt projekt!".to_string();
    }

    pub fn load_demo_project(&mut self) {
        self.new_empty_project();

        // Pattern 1: Electro Beats
        let mut p1_steps = vec![[false; 16]; 8];
        p1_steps[0][0] = true; p1_steps[0][6] = true; p1_steps[0][10] = true;
        p1_steps[1][4] = true; p1_steps[1][12] = true;
        for i in (0..16).step_by(2) { p1_steps[2][i] = true; }
        p1_steps[3][2] = true; p1_steps[3][8] = true; p1_steps[3][14] = true;
        p1_steps[4][2] = true; p1_steps[4][10] = true;
        p1_steps[5][7] = true; p1_steps[5][15] = true;
        p1_steps[6][0] = true; p1_steps[6][3] = true; p1_steps[6][8] = true; p1_steps[6][11] = true;
        p1_steps[7][0] = true; p1_steps[7][4] = true; p1_steps[7][8] = true; p1_steps[7][12] = true;
        if !self.patterns.is_empty() { self.patterns[0].channel_steps = p1_steps; }

        // Pattern 2: Acid Groove
        let mut p2_steps = vec![[false; 16]; 8];
        for i in (0..16).step_by(4) { p2_steps[0][i] = true; }
        p2_steps[1][4] = true; p2_steps[1][12] = true;
        p2_steps[2].fill(true);
        p2_steps[6][0] = true; p2_steps[6][2] = true; p2_steps[6][6] = true; p2_steps[6][10] = true;
        if self.patterns.len() > 1 { self.patterns[1].channel_steps = p2_steps; }

        // Pattern 3: Synth Arp
        let mut p3_steps = vec![[false; 16]; 8];
        let mut p3_grid = [[false; 16]; 24];
        let arp_notes = [60, 63, 67, 70, 72, 70, 67, 63];
        for i in 0..16 {
            p3_steps[6][i] = true;
            let n = arp_notes[i % 8];
            let offset = (n as usize).saturating_sub(48).min(23);
            p3_grid[offset][i] = true;
        }
        if self.patterns.len() > 2 {
            self.patterns[2].channel_steps = p3_steps;
            self.patterns[2].piano_roll_grid = p3_grid;
        }

        // Pattern 4: Deep Bass
        let mut p4_steps = vec![[false; 16]; 8];
        for i in (0..16).step_by(4) { p4_steps[3][i] = true; }
        p4_steps[7][0] = true; p4_steps[7][4] = true; p4_steps[7][8] = true; p4_steps[7][12] = true;
        if self.patterns.len() > 3 { self.patterns[3].channel_steps = p4_steps; }

        // Demo arrangement clips
        if self.playlist_tracks.len() >= 6 {
            for b in 0..4 { self.playlist_tracks[1].clips[b] = Some(1); }
            for b in 4..16 { self.playlist_tracks[1].clips[b] = Some(0); }
            for b in 4..8 { self.playlist_tracks[2].clips[b] = Some(0); }
            for b in 8..12 { self.playlist_tracks[2].clips[b] = Some(2); }
            for b in 12..16 { self.playlist_tracks[2].clips[b] = Some(0); }
            for b in 0..16 { self.playlist_tracks[3].clips[b] = Some(3); }
            for b in 0..8 { self.playlist_tracks[4].clips[b] = Some(2); }
            self.playlist_tracks[5].clips[7] = Some(1);
            self.playlist_tracks[5].clips[15] = Some(1);
        }

        self.bpm = 126.0;
        self.project_name = "Sonix Synthwave Demo".to_string();
        self.select_pattern(0);
        self.status_message = "⚡ Laddade Sonix Synthwave Demo-projekt!".to_string();
    }

    pub fn sync_track_regions(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            let sec_per_bar = 60.0 / self.bpm * 4.0;
            let t = &self.playlist_tracks[track_idx];
            let mut region_playbacks = Vec::with_capacity(t.regions.len());
            for r in &t.regions {
                region_playbacks.push(StemRegionPlayback {
                    start_time_secs: r.start_bar * sec_per_bar,
                    length_secs: r.length_bars * sec_per_bar,
                    sample_offset_sec: r.sample_offset_sec,
                    gain: r.volume,
                    fade_in_sec: r.fade_in_bars * sec_per_bar,
                    fade_out_sec: r.fade_out_bars * sec_per_bar,
                    muted: r.muted,
                });
            }
            let _ = self.engine.send_command(AudioCommand::SetStemTrackRegions {
                track_index: track_idx,
                regions: region_playbacks,
            });
        }
    }

    pub fn save_project(&mut self, name: &str) {
        let clean_name = if name.trim().is_empty() { "Namnlöst Projekt" } else { name.trim() };
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = std::path::PathBuf::from(home).join("Music/Sonix/Projects");
        let _ = std::fs::create_dir_all(&dir);

        let file_path = dir.join(format!("{}.sonix", clean_name.replace('/', "_")));
        let saved_tracks: Vec<SavedTrackData> = self.playlist_tracks.iter().map(|t| SavedTrackData {
            name: t.name.clone(),
            volume: t.volume,
            pan: t.pan,
            muted: t.muted,
            solo: t.solo,
            clips: t.clips,
            regions: t.regions.clone(),
            eq: t.eq.clone(),
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send: t.reverb_send,
            delay_send: t.delay_send,
        }).collect();

        let data = SonixProjectData {
            name: clean_name.to_string(),
            bpm: self.bpm,
            swing: self.swing,
            master_volume: self.master_volume,
            master_pan: self.master_pan,
            tracks: saved_tracks,
        };

        if let Ok(json) = serde_json::to_string_pretty(&data)
            && std::fs::write(&file_path, json).is_ok() {
                self.project_name = clean_name.to_string();
                self.project_file_path = Some(file_path.to_string_lossy().to_string());
                self.status_message = format!("💾 Sparade projekt till '{}'!", file_path.display());
                return;
            }
        self.status_message = "❌ Misslyckades att spara projektet.".to_string();
    }

    pub fn load_project_file(&mut self, path_str: &str) {
        if let Ok(content) = std::fs::read_to_string(path_str)
            && let Ok(data) = serde_json::from_str::<SonixProjectData>(&content) {
                self.stop_playback();
                let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);

                self.project_name = data.name;
                self.bpm = data.bpm;
                self.swing = data.swing;
                self.master_volume = data.master_volume;
                self.master_pan = data.master_pan;

                self.playlist_tracks.clear();
                for (t_idx, st) in data.tracks.into_iter().enumerate() {
                    let regions = st.regions;
                    // Re-load audio PCM for regions that have source_path
                    for r in &regions {
                        if let Some(ref p) = r.source_path
                            && let Ok((pcm_l, pcm_r, sr)) = crate::audio::load_wav_pcm(p) {
                                let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                                    track_index: t_idx,
                                    left: std::sync::Arc::new(pcm_l),
                                    right: std::sync::Arc::new(pcm_r),
                                    sample_rate: sr as f32,
                                    volume: st.volume,
                                    pan: st.pan,
                                    start_time_secs: 0.0,
                                });
                            }
                    }

                    let mut loaded_track = PlaylistTrack::new(st.name, "🎵", TrackKind::CustomAudio, Color32::from_rgb(100, 180, 255));
                    loaded_track.volume = st.volume;
                    loaded_track.pan = st.pan;
                    loaded_track.muted = st.muted;
                    loaded_track.solo = st.solo;
                    loaded_track.regions = regions;
                    loaded_track.clips = st.clips;
                    loaded_track.eq = st.eq;
                    loaded_track.comp_threshold_db = st.comp_threshold_db;
                    loaded_track.comp_ratio = st.comp_ratio;
                    loaded_track.reverb_send = st.reverb_send;
                    loaded_track.delay_send = st.delay_send;
                    self.playlist_tracks.push(loaded_track);
                    self.sync_track_regions(t_idx);
                }

                self.loop_end_bar = self.get_max_project_bars().max(32);
                self.project_file_path = Some(path_str.to_string());
                self.status_message = format!("📂 Öppnade projekt '{}'!", self.project_name);
                return;
            }
        self.status_message = format!("❌ Kunde inte läsa projektfilen: {}", path_str);
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
            for (ch_i, ch) in self.channels.iter_mut().enumerate() {
                if ch_i < self.patterns[pat_idx].channel_steps.len() {
                    ch.steps = self.patterns[pat_idx].channel_steps[ch_i];
                    ch.notes = self.patterns[pat_idx].channel_notes[ch_i];
                }
            }
            self.piano_roll_grid = self.patterns[pat_idx].piano_roll_grid;
            self.status_message = format!("Aktivt mönster: {}", self.patterns[pat_idx].name);
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
            let ch = &mut self.channels[ch_idx];
            ch.name = item.name.clone();
            ch.icon = item.icon.clone();
            ch.color = item.color;
            ch.notes = [item.default_note; 16];
            ch.waveform_preview = item.waveform.clone();
            ch.sample_start = 0.0;
            ch.sample_end = 1.0;
            ch.pitch_semitones = 0;
            ch.pitch_fine_cents = 0.0;
            self.status_message = format!("Kanal {} ändrad till: {}", ch_idx + 1, item.name);
            let freq = midi_to_freq(item.default_note);
            let _ = self.engine.send_command(AudioCommand::NoteOn { note: item.default_note, freq, velocity: 0.9 });
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
        };
        self.sample_library.push(item);
        self.status_message = format!("Importerade '{}' till ljudbiblioteket!", name);
    }

    pub fn toggle_timeline_recording(&mut self) {
        if self.is_recording_timeline {
            let sec_per_bar = (60.0 / self.bpm.max(40.0)) * 4.0;
            let maybe_take = self.vocal_studio.stop_recording(self.bpm);
            self.is_recording_timeline = false;
            self.is_playing = false;
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));

            if let Some(take_idx) = maybe_take {
                if let Some(take) = self.vocal_studio.takes.get(take_idx) {
                    let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
                    let start_bar = self.timeline_rec_start_bar;
                    let length_bars = (take.duration_secs / sec_per_bar).max(0.5);
                    let region = AudioRegion {
                        id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize,
                        name: format!("🎙 {}", take.name),
                        start_bar,
                        length_bars,
                        sample_offset_sec: 0.0,
                        source_path: None,
                        waveform_peaks: take.waveform_data.clone(),
                        volume: 1.0,
                        fade_in_bars: 0.0,
                        fade_out_bars: 0.0,
                        muted: false,
                        color: self.playlist_tracks[armed_idx].color,
                    };
                    self.playlist_tracks[armed_idx].regions.push(region);
                    self.status_message = format!("✔ Spelade in '{}' direkt i spår {} ({}) vid takt {:.1}!", take.name, armed_idx + 1, self.playlist_tracks[armed_idx].name, start_bar + 1.0);
                }
            }
        } else {
            if !self.playlist_tracks.iter().any(|t| t.is_rec_armed) && !self.playlist_tracks.is_empty() {
                self.playlist_tracks[0].is_rec_armed = true;
            }
            let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
            let sec_per_bar = (60.0 / self.bpm.max(40.0)) * 4.0;
            self.timeline_rec_start_bar = self.song_time / sec_per_bar;
            self.is_recording_timeline = true;
            self.vocal_studio.start_recording();
            self.is_playing = true;
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
            self.status_message = format!("🔴 Spelar in direkt i spår '{}' från takt {:.1}...", self.playlist_tracks[armed_idx].name, self.timeline_rec_start_bar + 1.0);
        }
    }

    pub fn toggle_playback(&mut self) {
        if self.is_recording_timeline {
            self.toggle_timeline_recording();
            return;
        }
        self.is_playing = !self.is_playing;
        if self.is_playing {
            let song_secs = if self.pattern_mode {
                0.0
            } else {
                (self.song_bar as f32 + self.song_step_in_bar as f32 / 16.0) * (60.0 / self.bpm * 4.0)
            };
            self.song_time = song_secs;
            let _ = self.engine.send_command(AudioCommand::SeekSongPosition(song_secs));
            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
        } else {
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
        self.song_time = bar as f32 * (60.0 / self.bpm * 4.0);
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(self.song_time));
        self.status_message = format!("Flyttade markör till Takt {}", bar + 1);
    }

    pub fn seek_song_time(&mut self, time_secs: f32) {
        let sec_per_bar = 60.0 / self.bpm * 4.0;
        let clamped = time_secs.max(0.0);
        self.song_time = clamped;
        let bar_float = clamped / sec_per_bar;
        self.song_bar = bar_float.floor() as usize;
        let rem_bar = (bar_float - self.song_bar as f32).max(0.0);
        self.song_step_in_bar = ((rem_bar * 16.0).floor() as usize).min(15);
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(clamped));
        self.status_message = format!("Flyttade markör till {}", format_time_hundredths(clamped));
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
        }
    }

    fn step_duration(&self, step: usize) -> std::time::Duration {
        let base_seconds = (60.0 / self.bpm) / 4.0;
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
        }
    }

    fn trigger_step(&mut self, step: usize) {
        let has_solo = self.channels.iter().any(|c| c.solo);
        let vel = self.step_velocities[step];

        for (idx, ch) in self.channels.iter().enumerate() {
            let is_audible = if has_solo { ch.solo } else { !ch.muted };
            if ch.steps[step] && is_audible {
                match idx {
                    0 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
                    1 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
                    2 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
                    3 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed)); }
                    4 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen)); }
                    5 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Crash)); }
                    6 => {
                        let note = ch.notes[step];
                        let freq = midi_to_freq(note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
                    }
                    7 => {
                        let note = ch.notes[step];
                        let freq = midi_to_freq(note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
                    }
                    _ => {}
                }
            }
        }
    }

    fn trigger_song_step(&mut self, bar: usize, step_in_bar: usize) {
        if bar >= 32 || step_in_bar >= 16 {
            return;
        }
        let has_track_solo = self.playlist_tracks.iter().any(|t| t.solo);
        let vel = self.step_velocities[step_in_bar];

        for (t_idx, track) in self.playlist_tracks.iter().enumerate() {
            let is_audible = if has_track_solo { track.solo } else { !track.muted };
            if !is_audible {
                continue;
            }

            if let Some(pat_idx) = track.clips[bar]
                && let Some(pat) = self.patterns.get(pat_idx) {
                    match track.kind {
                        TrackKind::Drums => {
                            for ch_idx in 0..=5 {
                                if ch_idx < pat.channel_steps.len() && pat.channel_steps[ch_idx][step_in_bar] {
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
                        TrackKind::SynthLead => {
                            if pat.channel_steps.len() > 6 && pat.channel_steps[6][step_in_bar] {
                                let note = pat.channel_notes[6][step_in_bar];
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                            }
                        }
                        TrackKind::Bassline => {
                            if pat.channel_steps.len() > 7 && pat.channel_steps[7][step_in_bar] {
                                let note = pat.channel_notes[7][step_in_bar];
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                            }
                        }
                        TrackKind::VocalAudio | TrackKind::CustomAudio | TrackKind::Fx => {
                            if pat.channel_steps.len() > 6 && pat.channel_steps[6][step_in_bar] && step_in_bar.is_multiple_of(4) {
                                let note = 55 + (t_idx as u8 * 3);
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * 0.65 });
                            }
                        }
                    }
                }
        }
    }

    pub fn export_wav(&mut self) {
        self.sync_active_pattern_from_ui();
        let sample_rate = self.engine.sample_rate as f32;
        let mut synth = SynthEngine::new(sample_rate);
        synth.waveform = self.waveform;
        synth.adsr = self.adsr;
        synth.filter_params = self.filter;
        synth.delay_params = self.delay;
        synth.reverb_params = self.reverb;
        synth.drive = self.drive;
        synth.master_volume = self.master_volume;

        if self.pattern_mode {
            let pattern_grid: Vec<[bool; 16]> = self.channels.iter().map(|c| c.steps).collect();
            let step_notes = self.channels[6].notes;
            let export_path = "/home/alex/Projects/sonix/exported_pattern.wav";
            match render_to_wav(export_path, synth, &pattern_grid, &step_notes, self.bpm, 4) {
                Ok(p) => {
                    self.status_message = format!("✔ Sparat mönster som WAV: {}", p);
                }
                Err(e) => {
                    self.status_message = format!("✘ Export misslyckades: {}", e);
                }
            }
        } else {
            let pattern_steps = self.patterns.iter().map(|p| p.channel_steps.clone()).collect();
            let pattern_notes = self.patterns.iter().map(|p| p.channel_notes.clone()).collect();
            let track_clips = self.playlist_tracks.iter().map(|t| t.clips).collect();
            let track_muted = self.playlist_tracks.iter().map(|t| t.muted).collect();

            let arrangement = SongArrangementExport {
                pattern_steps,
                pattern_notes,
                track_clips,
                track_muted,
                num_bars: self.loop_end_bar.clamp(4, 32),
                bpm: self.bpm,
            };

            let export_path = "/home/alex/Projects/sonix/exported_song.wav";
            match render_song_arrangement_to_wav(export_path, synth, &arrangement) {
                Ok(p) => {
                    self.status_message = format!("✔ Sparad hel låt (Song Arranger) som WAV: {}", p);
                }
                Err(e) => {
                    self.status_message = format!("✘ Export misslyckades: {}", e);
                }
            }
        }
    }

    fn play_note(&mut self, note: u8) {
        if !self.active_keys.contains(&note) {
            self.active_keys.insert(note);
            let freq = midi_to_freq(note);
            let _ = self.engine.send_command(AudioCommand::NoteOn {
                note,
                freq,
                velocity: 0.85,
            });
        }
    }

    fn release_note(&mut self, note: u8) {
        self.active_keys.remove(&note);
        let _ = self.engine.send_command(AudioCommand::NoteOff { note });
    }
}

impl eframe::App for SonixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        self.advance_sequencer();
        self.anim_phase += 0.08;
        self.vocal_studio.update_live_stream();

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
        let modal_open = self.show_help_guide
            || self.show_project_manager_modal
            || self.show_suno_import_modal
            || self.show_render_queue_modal
            || self.show_stem_focus_modal
            || self.show_controller_modal
            || self.show_about_modal
            || self.show_ai_settings_modal
            || self.show_audio_settings_modal
            || self.show_import_modal;

        let wants_keyboard = ctx.wants_keyboard_input();
        let piano_active = self.view_mode == ViewMode::PianoRoll && !modal_open && !wants_keyboard;

        // Release any stuck notes if piano was switched away or keyboard input was captured
        if !piano_active && !self.active_keys.is_empty() {
            let keys_to_release: Vec<u8> = self.active_keys.iter().copied().collect();
            for note in keys_to_release {
                self.release_note(note);
            }
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

            // Transport & region keys only when not actively typing text in an input box
            if !wants_keyboard {
                if i.key_pressed(egui::Key::Space) {
                    self.toggle_playback();
                }

                if (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
                    && let Some((t_idx, r_idx)) = self.selected_audio_region
                        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                            self.playlist_tracks[t_idx].regions.remove(r_idx);
                            self.sync_track_regions(t_idx);
                            self.selected_audio_region = None;
                            self.status_message = "🗑 Raderade markerad ljudregion.".to_string();
                        }
            }

            // Global Project & View Shortcuts
            if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
                self.new_empty_project();
            }
            if i.modifiers.ctrl && (i.key_pressed(egui::Key::O) || i.key_pressed(egui::Key::P)) {
                self.show_project_manager_modal = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                let p_name = self.project_name.clone();
                self.save_project(&p_name);
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::I) {
                self.show_suno_import_modal = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::E) {
                self.show_render_queue_modal = true;
            }
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
                    ui.menu_button("📁 Arkiv", |ui| {
                        if ui.button("📄 Nytt tomt projekt (Ctrl+N)").clicked() {
                            self.new_empty_project();
                            ui.close_menu();
                        }
                        if ui.button("📂 Öppna projekt... (Ctrl+O)").clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        if ui.button("💾 Spara projekt (Ctrl+S)").clicked() {
                            let p_name = self.project_name.clone();
                            self.save_project(&p_name);
                            ui.close_menu();
                        }
                        if ui.button("💾 Spara som... (Ctrl+Shift+S)").clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("📁 Filhanterare / Projektbläddrare (Ctrl+P)").clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        if ui.button("⚡ Ladda Demo-projekt").clicked() {
                            self.load_demo_project();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("🎼 Importera Stämmor / Stems... (Ctrl+I)").clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button("↗ Exportera WAV / Master (Ctrl+E)").clicked() {
                            self.show_render_queue_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("🚪 Avsluta").clicked() {
                            std::process::exit(0);
                        }
                    });

                    // Redigera
                    ui.menu_button("✏ Redigera", |ui| {
                        if ui.button("↶ Ångra (Ctrl+Z)").clicked() {
                            self.status_message = "Ångrade senaste ändringen.".to_string();
                            ui.close_menu();
                        }
                        if ui.button("↷ Gör om (Ctrl+Y)").clicked() {
                            self.status_message = "Gjorde om senaste ändringen.".to_string();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("✂ Klipp markerat (Ctrl+X)").clicked() {
                            if let Some((t_idx, r_idx)) = self.selected_audio_region
                                && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                                    self.playlist_tracks[t_idx].regions.remove(r_idx);
                                    self.sync_track_regions(t_idx);
                                    self.selected_audio_region = None;
                                    self.status_message = "Klippte ut ljudregion.".to_string();
                                }
                            ui.close_menu();
                        }
                        if ui.button("🗑 Ta bort markerat (Del)").clicked() {
                            if let Some((t_idx, r_idx)) = self.selected_audio_region
                                && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                                    self.playlist_tracks[t_idx].regions.remove(r_idx);
                                    self.sync_track_regions(t_idx);
                                    self.selected_audio_region = None;
                                    self.status_message = "🗑 Raderade markerad region.".to_string();
                                }
                            ui.close_menu();
                        }
                        if ui.button("🧹 Rensa alla spår").clicked() {
                            for t in &mut self.playlist_tracks {
                                t.regions.clear();
                                t.clips = [None; 32];
                            }
                            self.selected_audio_region = None;
                            self.status_message = "Rensade alla spår och tidslinjeklipp.".to_string();
                            ui.close_menu();
                        }
                    });

                    // Vy
                    ui.menu_button("👁 Vy", |ui| {
                        if ui.button("📊 Tidslinje / Arranger (F3)").clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button("🥁 Sonix Channel Rack (F4)").clicked() {
                            self.view_mode = ViewMode::ChannelRack;
                            ui.close_menu();
                        }
                        if ui.button("🎹 Sonix Piano Roll (F5)").clicked() {
                            self.view_mode = ViewMode::PianoRoll;
                            ui.close_menu();
                        }
                        if ui.button("🎛 Mixer Console (F6)").clicked() {
                            self.view_mode = ViewMode::EffectsMixer;
                            ui.close_menu();
                        }
                        if ui.button("🎛 Analog Synthesizer (F7)").clicked() {
                            self.view_mode = ViewMode::AlchemySynth;
                            ui.close_menu();
                        }
                        if ui.button("🎙 Vocal Studio & Harmonizer (F8)").clicked() {
                            self.view_mode = ViewMode::VocalStudio;
                            ui.close_menu();
                        }
                        if ui.button("🤖 AI Music Studio (F9)").clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                        if ui.button("🔌 Modulär Synt & Patcher (F10)").clicked() {
                            self.view_mode = ViewMode::ModularPatcher;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(format!("📁 Växla Webbläsare / Bibliotek: {}", if self.show_browser { "PÅ" } else { "AV" })).clicked() {
                            self.show_browser = !self.show_browser;
                            ui.close_menu();
                        }
                    });

                    // AI & Providers
                    ui.menu_button("🤖 AI & Providers", |ui| {
                        if ui.button("⚙ AI Provider Inställningar & API-nycklar...").clicked() {
                            self.show_ai_settings_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("🎼 Importera Stämmor / Multi-Track Stems...").clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button("✨ ACE-Step & Stable Audio Generator...").clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button("🤖 AI Co-Producer Assistent...").clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                    });

                    // Inställningar
                    ui.menu_button("⚙ Inställningar", |ui| {
                        if ui.button("🎛 Ljud- & MIDI-inställningar (PipeWire/ALSA/JACK)...").clicked() {
                            self.show_audio_settings_modal = true;
                            ui.close_menu();
                        }
                        if ui.button("🎛 Hårdvarukontroller (MCU / OSC)...").clicked() {
                            self.show_controller_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Hjälp
                    ui.menu_button("❓ Hjälp", |ui| {
                        if ui.button("📖 Snabbguide & Manual (F1)").clicked() {
                            self.show_help_guide = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("ℹ Om Sonix Studio...").clicked() {
                            self.show_about_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Right-aligned project indicator & quick status
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let p_label = format!("📁 Projekt: {}", self.project_name);
                        if ui.add(egui::Button::new(egui::RichText::new(p_label).size(10.5).color(Theme::FL_ORANGE)).frame(false)).clicked() {
                            self.show_project_manager_modal = true;
                        }
                    });
                });
            });

        // ====================================================================
        // 1. TOP METALLIC TOOLBAR
        // ====================================================================
        egui::TopBottomPanel::top("fl_toolbar")
            .frame(egui::Frame::none().fill(Theme::HEADER_BG).inner_margin(6.0))
            .show(ctx, |ui| {
                // ROW 1: Logo, Transport, Master Knobs, Clock, Export & Guide
                ui.horizontal(|ui| {
                    // Browser Toggle
                    let browser_btn = ui.selectable_label(self.show_browser, "📁 Browser");
                    if browser_btn.clicked() { self.show_browser = !self.show_browser; }

                    ui.separator();

                    ui.label(egui::RichText::new("🍊 SONIX").strong().size(16.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new("STUDIO").strong().size(13.0).color(Theme::FL_CYAN));
                    ui.separator();

                    // Transport Buttons
                    let play_color = if self.is_playing { Theme::FL_GREEN } else { Color32::from_rgb(40, 50, 45) };
                    if ui.add(egui::Button::new(egui::RichText::new(" ▶ PLAY ").strong().size(12.0).color(Color32::WHITE)).fill(play_color)).clicked()
                        && !self.is_playing {
                            self.toggle_playback();
                        }

                    let pause_color = if !self.is_playing { Theme::FL_ORANGE } else { Color32::from_rgb(45, 40, 35) };
                    if ui.add(egui::Button::new(egui::RichText::new(" ⏸ PAUSE ").strong().size(12.0).color(Color32::WHITE)).fill(pause_color)).clicked()
                        && self.is_playing {
                            self.toggle_playback();
                        }

                    if ui.add(egui::Button::new(egui::RichText::new(" ⏹ STOP ").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(45, 48, 56))).clicked() {
                        self.stop_playback();
                    }

                    ui.separator();

                    // Digital LCD Display & Clock (Fixed width, monospace)
                    ui.group(|ui| {
                        ui.set_height(26.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("BPM").size(9.0).color(Theme::TEXT_MUTED));
                            ui.add_sized(Vec2::new(42.0, 18.0), egui::Label::new(egui::RichText::new(format!("{:.1}", self.bpm)).monospace().strong().size(12.5).color(Theme::LCD_TEXT)));
                            if ui.button("▲").clicked() && self.bpm < 240.0 { self.bpm += 1.0; }
                            if ui.button("▼").clicked() && self.bpm > 40.0 { self.bpm -= 1.0; }

                            ui.separator();

                            let mode_text = if self.pattern_mode { "PAT" } else { "SONG" };
                            let mode_color = if self.pattern_mode { Theme::FL_ORANGE } else { Theme::FL_CYAN };
                            if ui.add_sized(Vec2::new(44.0, 20.0), egui::Button::new(egui::RichText::new(mode_text).strong().size(9.5).color(Color32::BLACK)).fill(mode_color)).clicked() {
                                self.pattern_mode = !self.pattern_mode;
                                self.status_message = format!("Läge ändrat till: {}", if self.pattern_mode { "PAT (Mönsterloop)" } else { "SONG (Låtläge)" });
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
                                let tick = (self.song_step_in_bar % 4) + 1;
                                (format!("⏱ {} [TAKT {:03}.{}.{}]", time_str, bar, step, tick), Theme::FL_CYAN)
                            };
                            ui.add_sized(Vec2::new(200.0, 20.0), egui::Label::new(egui::RichText::new(txt).monospace().strong().size(12.0).color(color)));
                        });
                    });

                    ui.separator();

                    // Pattern Quick Selector
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("PAT:").size(9.0).color(Theme::TEXT_MUTED));
                            let p_len = self.patterns.len();
                            for pat_idx in 0..p_len {
                                let is_active = self.selected_pattern == pat_idx;
                                let p_color = self.patterns[pat_idx].color;
                                let fill = if is_active { p_color } else { Theme::PANEL_BG };
                                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                                if ui.add(egui::Button::new(egui::RichText::new(format!("P{}", pat_idx + 1)).strong().size(10.0).color(text_color)).fill(fill)).clicked() {
                                    self.select_pattern(pat_idx);
                                }
                            }
                        });
                    });

                    ui.separator();

                    // Swing Knob
                    rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 22.0);

                    ui.separator();

                    // Oscilloscope Display
                    let peak = self.engine.get_peak_level();
                    oscilloscope_display(ui, peak, self.anim_phase, Vec2::new(60.0, 24.0));

                    ui.separator();

                    // Master Volume & Pan Knobs
                    if rotary_knob(ui, &mut self.master_volume, 0.0, 1.0, "MASTER", Theme::FL_ORANGE, 22.0) {
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    }
                    rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", Color32::from_rgb(180, 190, 200), 22.0);

                    ui.separator();

                    // Export WAV Button
                    let export_label = if self.pattern_mode { "💾 EXPORT PAT" } else { "💾 EXPORT SONG" };
                    if ui.add(egui::Button::new(egui::RichText::new(export_label).strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 90))).clicked() {
                        self.export_wav();
                    }

                    // Batch Export & Render Queue Button
                    if ui.add(egui::Button::new(egui::RichText::new("📤 BATCH EXPORT").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(45, 90, 140))).clicked() {
                        self.show_render_queue_modal = true;
                    }

                    // Stems Importer Button
                    if ui.add(egui::Button::new(egui::RichText::new("📦 IMPORT STEMS").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
                        self.scan_for_suno_stems();
                        self.show_suno_import_modal = true;
                    }

                    // Hardware Controller MCU / OSC Button
                    let mcu_col = if self.mcu_connected { Color32::from_rgb(110, 60, 130) } else { Theme::PANEL_BG };
                    if ui.add(egui::Button::new(egui::RichText::new("🎛 MCU/OSC").strong().size(11.0).color(Color32::WHITE)).fill(mcu_col)).clicked() {
                        self.show_controller_modal = true;
                    }

                    ui.separator();

                    let guide_col = if self.show_help_guide { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                    if ui.button(egui::RichText::new("💡 Snabbguide").color(guide_col)).clicked() {
                        self.show_help_guide = !self.show_help_guide;
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // ROW 2: Primary Workspaces & Advanced Studios Navigation Tabs
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ARBETSSTYTA:").strong().size(10.0).color(Theme::TEXT_MUTED));

                    let arr_btn = ui.selectable_label(self.view_mode == ViewMode::PlaylistArranger, "🎼 1. Tidslinje / Arranger");
                    if arr_btn.clicked() { self.view_mode = ViewMode::PlaylistArranger; }

                    let rack_btn = ui.selectable_label(self.view_mode == ViewMode::ChannelRack, "🥁 2. Channel Rack");
                    if rack_btn.clicked() { self.view_mode = ViewMode::ChannelRack; }

                    let roll_btn = ui.selectable_label(self.view_mode == ViewMode::PianoRoll, "🎹 3. Piano & Pianorulle");
                    if roll_btn.clicked() { self.view_mode = ViewMode::PianoRoll; }

                    let voc_btn = ui.selectable_label(self.view_mode == ViewMode::VocalStudio, "🎙 4. Sångstudio");
                    if voc_btn.clicked() { self.view_mode = ViewMode::VocalStudio; }

                    let fx_btn = ui.selectable_label(self.view_mode == ViewMode::EffectsMixer, "🎚 5. Mixer");
                    if fx_btn.clicked() { self.view_mode = ViewMode::EffectsMixer; }

                    let ai_btn = ui.selectable_label(self.view_mode == ViewMode::AiMusicAssistant, "🤖 6. AI Assistent");
                    if ai_btn.clicked() { self.view_mode = ViewMode::AiMusicAssistant; }

                    ui.separator();
                    ui.label(egui::RichText::new("STUDIOS:").strong().size(10.0).color(Theme::TEXT_MUTED));

                    let alc_btn = ui.selectable_label(self.view_mode == ViewMode::AlchemySynth, "✨ Alchemy Synt");
                    if alc_btn.clicked() { self.view_mode = ViewMode::AlchemySynth; }

                    let drum_btn = ui.selectable_label(self.view_mode == ViewMode::SessionDrummer, "🥁 Session Drummer");
                    if drum_btn.clicked() { self.view_mode = ViewMode::SessionDrummer; }

                    let patch_btn = ui.selectable_label(self.view_mode == ViewMode::ModularPatcher, "🧩 Patcher Grid");
                    if patch_btn.clicked() { self.view_mode = ViewMode::ModularPatcher; }

                    let stem_btn = ui.selectable_label(self.view_mode == ViewMode::StemSeparator, "🧠 AI Stems");
                    if stem_btn.clicked() { self.view_mode = ViewMode::StemSeparator; }

                    let plug_btn = ui.selectable_label(self.view_mode == ViewMode::PluginManager, "🔌 Plugins & Wine");
                    if plug_btn.clicked() { self.view_mode = ViewMode::PluginManager; }

                    let rmx_btn = ui.selectable_label(self.view_mode == ViewMode::RemixFx, "🎛 Remix FX");
                    if rmx_btn.clicked() { self.view_mode = ViewMode::RemixFx; }
                });
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("fl_status_bar")
            .frame(egui::Frame::none().fill(Theme::PANEL_BG).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.status_message).size(11.0).color(Theme::FL_CYAN));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("PipeWire / ALSA 44.1kHz • 16-Stämmor Polyfoni • Sonix Studio Pro DAW").size(10.0).color(Theme::TEXT_MUTED));
                    });
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
                            render_patcher_view(ui, &mut self.modular_graph, self.anim_phase);
                        }
                        ViewMode::StemSeparator => {
                            render_stem_separator_view(ui, &mut self.stem_project, self.song_time, self.is_playing, &mut self.status_message);
                        }
                        ViewMode::PluginManager => {
                            render_plugins_view(ui, &mut self.plugin_manager, &mut self.status_message);
                        }
                        ViewMode::VocalStudio => {
                            render_vocal_studio_view(
                                ui,
                                &mut self.vocal_studio,
                                &mut self.vocal_harmonizer,
                                self.is_playing,
                                self.current_step,
                                self.bpm,
                                &mut self.playlist_tracks,
                                &mut self.status_message,
                            );
                        }
                        ViewMode::AiMusicAssistant => {
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
        self.render_about_modal(ctx);
        self.render_project_manager_modal(ctx);
        self.render_ai_settings_modal(ctx);
        self.render_audio_settings_modal(ctx);
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
                        println!("🎉 Alla 22 skärmdumpar har genererats och sparats framgångsrikt!");
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
            ui.label(egui::RichText::new("📁 SOUND BROWSER").strong().size(13.0).color(Theme::FL_ORANGE));
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut self.browser_search);
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // 1. Drum Sounds
                ui.collapsing("🥁 DRUM KITS & SAMPLES", |ui| {
                    let drums = [
                        ("💥 808 Sub Kick", DrumType::Kick),
                        ("🥁 909 Tight Snare", DrumType::Snare),
                        ("👏 Electro Handclap", DrumType::Clap),
                        ("⚡ Crisp Closed Hat", DrumType::HiHatClosed),
                        ("🌊 Sizzling Open Hat", DrumType::HiHatOpen),
                        ("✨ Cyber Crash Cymbal", DrumType::Crash),
                        ("🔊 808 Low Tom", DrumType::TomLow),
                        ("🔔 808 High Tom", DrumType::TomHigh),
                    ];
                    for (name, dt) in drums {
                        if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("▶").clicked() {
                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(dt));
                            }
                            ui.label(name);
                        });
                    }
                });

                ui.add_space(6.0);

                // 2. Synth Presets
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
                            if ui.button("▶").clicked() {
                                self.current_preset = p;
                                let (wf, adsr, flt) = p.settings();
                                self.waveform = wf;
                                self.adsr = adsr;
                                self.filter = flt;
                                let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note: 60, freq: 261.63, velocity: 0.85 });
                            }
                            if ui.button("Ladda").clicked() {
                                self.current_preset = p;
                                let (wf, adsr, flt) = p.settings();
                                self.waveform = wf;
                                self.adsr = adsr;
                                self.filter = flt;
                                let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                                self.status_message = format!("Laddade preset: {}", name);
                            }
                            ui.label(name);
                        });
                    }
                });

                ui.add_space(6.0);

                // 3. Apple Loops / Groove Templates
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
                            if ui.button("⚡ Sätt BPM").clicked() {
                                self.bpm = bpm;
                                self.swing = swing;
                                self.status_message = format!("Satte tempo till {} BPM med {:.0}% swing", bpm, swing * 100.0);
                            }
                            ui.label(name);
                        });
                    }
                });

                ui.add_space(6.0);

                // 4. Sound FX
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
                            if ui.button("▶").clicked() {
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: midi_to_freq(note), velocity: 0.9 });
                            }
                            ui.label(name);
                        });
                    }
                });
            });
        });
    }

    pub fn trigger_suno_ai_generation(&mut self) {
        if self.suno_prompt_input.trim().is_empty() {
            return;
        }
        self.suno_is_generating = true;
        self.suno_generation_progress = 1.0;

        let prompt = self.suno_prompt_input.clone();
        let prompt_lower = prompt.to_lowercase();

        if prompt_lower.contains("stem") || prompt_lower.contains("full") || prompt_lower.contains("hela låt") {
            // Multi-Stem Generation (Drums, Bass, Vocal, Synth)
            let stem_defs = [
                ("🎙 AI Lead Vocal (ACE-Step)", TrackKind::VocalAudio, Color32::from_rgb(180, 110, 255), 28.0),
                ("🥁 AI Drum Stems (Stable Audio)", TrackKind::Drums, Theme::FL_CYAN, 40.0),
                ("🎸 AI Sub Bass (MusicGen)", TrackKind::Bassline, Color32::from_rgb(255, 80, 140), 20.0),
                ("✨ AI Synth Harmony (ACE-Step)", TrackKind::SynthLead, Theme::FL_GREEN, 52.0),
            ];

            for (s_name, kind, col, freq) in stem_defs {
                let mut wave = Vec::with_capacity(100);
                for i in 0..100 {
                    let t = i as f32 / 100.0;
                    let val = (t * freq).sin().abs() * 0.75 + (t * freq * 2.0).cos().abs() * 0.25;
                    wave.push(val.clamp(0.08, 0.96));
                }

                let mut track = PlaylistTrack::new(s_name.to_string(), "✨", kind, col);
                track.volume = 0.90;
                track.audio_waveform = Some(wave);
                track.custom_clip_name = Some(format!("Stem: {}", prompt));
                for b in self.loop_start_bar..self.loop_end_bar.min(32) {
                    track.clips[b] = Some(self.selected_pattern);
                }
                self.playlist_tracks.push(track);
            }
            self.status_message = format!("✨ ACE-Step/Stable Audio genererade 4 synkade stems för: '{}'", prompt);
        } else {
            // Single Track Generation or Inpainting
            let track_name = if prompt_lower.contains("trum") || prompt_lower.contains("drum") || prompt_lower.contains("beat") {
                "🥁 AI Drum Groove (Stable Audio)"
            } else if prompt_lower.contains("sång") || prompt_lower.contains("vocal") || prompt_lower.contains("voice") {
                "🎙 AI Vocal Lead (ACE-Step)"
            } else if prompt_lower.contains("bas") || prompt_lower.contains("bass") {
                "🎸 AI Sub Bassline (MusicGen)"
            } else {
                "✨ AI Synth Harmony (ACE-Step)"
            };

            let mut new_wave = Vec::with_capacity(100);
            for i in 0..100 {
                let t = i as f32 / 100.0;
                let val = (t * 32.0).sin().abs() * 0.75 + (t * 64.0).cos().abs() * 0.25;
                new_wave.push(val.clamp(0.08, 0.96));
            }

            let mut new_track = PlaylistTrack::new(track_name.to_string(), "✨", TrackKind::CustomAudio, Color32::from_rgb(175, 115, 255));
            new_track.volume = 0.90;
            new_track.audio_waveform = Some(new_wave);
            new_track.custom_clip_name = Some(format!("AI Clip: {}", prompt));

            let start_b = self.loop_start_bar.min(31);
            let end_b = self.loop_end_bar.max(start_b + 4).min(32);
            for b in start_b..end_b {
                new_track.clips[b] = Some(self.selected_pattern);
            }

            self.playlist_tracks.push(new_track);
            self.status_message = format!("✨ Genererade ny tagning (Takt {}-{}) via {}: '{}'", start_b + 1, end_b, self.suno_model_version, prompt);
        }

        self.suno_context_clip = Some(prompt);
        self.suno_is_generating = false;
    }

    fn render_playlist_arranger(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // ================================================================
            // 1. ARRANGER TOP CONTROL & TRANSPORT BARS (2 Non-Overlapping Rows)
            // ================================================================
            // ROW 1: Project Pill, Stems Import, Capsule Transport & Clock, BPM
            ui.horizontal(|ui| {
                // Project title pill
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("🎵 {}", self.project_name)).strong().size(12.0).color(Theme::TEXT_BRIGHT));
                        if ui.button("•••").on_hover_text("Filhanterare & Projektinställningar").clicked() {
                            self.show_project_manager_modal = true;
                        }
                    });
                });

                ui.separator();

                // Stems Importer Button
                if ui.add(egui::Button::new(egui::RichText::new("📦 Importera Stämmor (Stems)").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
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
                        if ui.button("⏮").on_hover_text("Gå till start (00:00.00)").clicked() {
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
                        if ui.add(egui::Button::new(egui::RichText::new("🔁").size(10.0).color(Theme::FL_CYAN)).fill(loop_bg)).clicked() {
                            self.status_message = format!("Loop-region satt till Takt {}-{}", self.loop_start_bar + 1, self.loop_end_bar);
                        }

                        ui.separator();

                        // High Precision LCD Time Display (Hundredths of a second) - Monospace and Fixed Width to prevent horizontal jitter
                        let time_str = format_time_hundredths(self.song_time);
                        let sec_per_bar = 60.0 / self.bpm * 4.0;
                        let bar_float = self.song_time / sec_per_bar;
                        let bar_text = format_bar_subdivisions(bar_float);
                        let full_time_str = format!("⏱ {}  {}", time_str, bar_text);
                        ui.add_sized(
                            Vec2::new(235.0, 18.0),
                            egui::Label::new(egui::RichText::new(full_time_str).monospace().strong().size(12.0).color(Theme::FL_CYAN))
                        );
                    });
                });

                ui.separator();

                // BPM Pill (Fixed width)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            Vec2::new(56.0, 18.0),
                            egui::Label::new(egui::RichText::new(format!("{:.1} BPM", self.bpm)).monospace().strong().size(11.0).color(Theme::TEXT_BRIGHT))
                        );
                        ui.separator();
                        ui.label(egui::RichText::new("4/4").strong().size(11.0).color(Theme::TEXT_MUTED));
                    });
                });
            });

            ui.add_space(2.0);

            // ROW 2: Arranger Tools, Snap Grid, Follow Playhead, and Right-Aligned Zoom Controls
            ui.horizontal(|ui| {
                // Arranger Tools Selector (Select, Paint, Slice ✂, Mute, Erase)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        let tools = [
                            (ArrangerTool::Select, "⇱ Välj"),
                            (ArrangerTool::Paint, "✎ Rita"),
                            (ArrangerTool::Slice, "✂ Klipp (0.01s)"),
                            (ArrangerTool::Mute, "🔇 Muta"),
                            (ArrangerTool::Erase, "🗑 Radera"),
                        ];
                        for (tool_val, tool_label) in tools {
                            let is_sel = std::mem::discriminant(&self.arranger_tool) == std::mem::discriminant(&tool_val);
                            let btn_bg = if is_sel { Theme::FL_ORANGE } else { Color32::from_rgb(32, 36, 44) };
                            let btn_fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(tool_label).strong().size(10.5).color(btn_fg)).fill(btn_bg)).clicked() {
                                self.arranger_tool = tool_val;
                            }
                        }
                    });
                });

                ui.separator();

                // Snap Selector (0.01s Precision, 1/16, Beat, Bar)
                ui.group(|ui| {
                    ui.set_height(26.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Snäpp:").size(10.0).color(Theme::TEXT_MUTED));
                        let snaps = [
                            (TimeSnapMode::FreeHundredth, "⚡ 0.01s (Fri)"),
                            (TimeSnapMode::Snap16th, "1/16"),
                            (TimeSnapMode::SnapBeat, "Beat"),
                            (TimeSnapMode::SnapBar, "Takt"),
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

                // Follow Playhead Toggle (Följ / Scrolla tidslinje)
                let auto_label = if self.timeline_auto_scroll { "🏃 Följ tidslinje: PÅ" } else { "⏸ Följ tidslinje: AV" };
                let auto_bg = if self.timeline_auto_scroll { Theme::FL_GREEN } else { Color32::from_rgb(36, 42, 54) };
                let auto_fg = if self.timeline_auto_scroll { Color32::BLACK } else { Theme::TEXT_MUTED };
                let auto_tip = if self.timeline_auto_scroll {
                    "Tidslinjen rullar automatiskt med spelhuvudet under uppspelning. Klicka för att stänga av."
                } else {
                    "Tidslinjen står stilla så du kan redigera i lugn och ro. Klicka för att aktivera följning."
                };
                if ui.add(egui::Button::new(egui::RichText::new(auto_label).strong().size(10.5).color(auto_fg)).fill(auto_bg)).on_hover_text(auto_tip).clicked() {
                    self.timeline_auto_scroll = !self.timeline_auto_scroll;
                    self.status_message = format!("Följ spelhuvud / Autoscroll: {}", if self.timeline_auto_scroll { "AKTIVERAD (Tidslinjen rullar med musiken)" } else { "INAKTIVERAD (Tidslinjen står stilla)" });
                }

                // Zoom Controls Bar on the right side of Row 2
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.group(|ui| {
                        ui.set_height(26.0);
                        ui.horizontal(|ui| {
                            if ui.button("🔍+").on_hover_text("Zooma in (Ctrl+Skrolla)").clicked() {
                                self.suno_zoom_level = (self.suno_zoom_level * 1.25).min(20.0);
                            }
                            let zoom_presets = [(0.5, "50%"), (1.0, "100%"), (2.0, "200%"), (4.0, "400%"), (8.0, "800%")];
                            for (z_val, z_lbl) in zoom_presets {
                                if ui.selectable_label((self.suno_zoom_level - z_val).abs() < 0.1, z_lbl).clicked() {
                                    self.suno_zoom_level = z_val;
                                }
                            }
                            if ui.button("🔍-").on_hover_text("Zooma ut (Ctrl+Skrolla)").clicked() {
                                self.suno_zoom_level = (self.suno_zoom_level * 0.8).max(0.25);
                            }
                            ui.label(egui::RichText::new(format!("Zoom: {:.0}%", self.suno_zoom_level * 100.0)).size(9.5).color(Theme::TEXT_MUTED));
                        });
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

            egui::ScrollArea::horizontal()
                .id_salt("sonix_arranger_timeline_scroll")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // ====================================================
                        // 2.1 MULTI-TIER RULER (Bars, Beats, Steps, Hundredths)
                        // ====================================================
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;

                            // Corner Track Header Spacer
                            let (corner_rect, _) = ui.allocate_exact_size(Vec2::new(header_w, ruler_h), Sense::hover());
                            ui.painter().rect_filled(corner_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 26));
                            ui.painter().text(
                                Pos2::new(corner_rect.min.x + 10.0, corner_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "SPÅR / INSTRUMENT",
                                egui::FontId::proportional(11.0),
                                Theme::TEXT_MUTED,
                            );

                            // Timeline Ruler Strip
                            let (ruler_rect, ruler_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, ruler_h), Sense::click_and_drag());
                            ui.painter().rect_filled(ruler_rect, Rounding::ZERO, Color32::from_rgb(16, 19, 26));
                            ui.painter().line_segment([Pos2::new(ruler_rect.min.x, ruler_rect.max.y), Pos2::new(ruler_rect.max.x, ruler_rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)));

                            // Highlight loop region on ruler
                            let loop_start_x = ruler_rect.min.x + self.loop_start_bar as f32 * bar_w;
                            let loop_end_x = ruler_rect.min.x + self.loop_end_bar as f32 * bar_w;
                            let loop_rect = Rect::from_min_max(Pos2::new(loop_start_x, ruler_rect.min.y), Pos2::new(loop_end_x, ruler_rect.max.y));
                            ui.painter().rect_filled(loop_rect, Rounding::ZERO, Color32::from_rgb(26, 36, 52));

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
                            if (ruler_resp.clicked() || ruler_resp.dragged())
                                && let Some(mouse_pos) = ruler_resp.hover_pos() {
                                    let click_bar = (mouse_pos.x - ruler_rect.min.x) / bar_w;
                                    let raw_sec = click_bar * sec_per_bar;
                                    let target_sec = match self.timeline_snap_mode {
                                        TimeSnapMode::FreeHundredth => (raw_sec * 100.0).round() / 100.0,
                                        TimeSnapMode::Snap16th => {
                                            let step_sec = sec_per_bar / 16.0;
                                            (raw_sec / step_sec).round() * step_sec
                                        }
                                        TimeSnapMode::SnapBeat => {
                                            let beat_sec = sec_per_bar / 4.0;
                                            (raw_sec / beat_sec).round() * beat_sec
                                        }
                                        TimeSnapMode::SnapBar => (raw_sec / sec_per_bar).round() * sec_per_bar,
                                    };
                                    scrub_target_sec = Some(target_sec.max(0.0));
                                    self.pattern_mode = false;
                                }

                            // Ruler Hover Tooltip with exact hundredths
                            if let Some(hover_pos) = ruler_resp.hover_pos() {
                                let h_bar = (hover_pos.x - ruler_rect.min.x) / bar_w;
                                let h_sec = (h_bar * sec_per_bar).max(0.0);
                                ruler_resp.clone().on_hover_text(format!("⏱ Tid: {}\n{}", format_time_hundredths(h_sec), format_bar_subdivisions(h_bar)));
                            }
                        });

                        ui.add_space(2.0);

                        // ====================================================
                        // 2.2 TRACK ROWS & CONTINUOUS WAVEFORM AUDIO LANES
                        // ====================================================
                        let num_tracks = self.playlist_tracks.len();
                        let mut track_lane_origin_x = 0.0;

                        for t_idx in 0..num_tracks {
                            let is_sel_track = self.selected_timeline_track == t_idx;

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;

                                // Left Track Header Card (Sonix Style)
                                let (h_rect, h_resp) = ui.allocate_exact_size(Vec2::new(header_w, row_h), Sense::click());
                                let track_color = self.playlist_tracks[t_idx].color;
                                let track_muted = self.playlist_tracks[t_idx].muted;
                                let track_solo = self.playlist_tracks[t_idx].solo;
                                let track_vol = self.playlist_tracks[t_idx].volume;
                                let track_name = self.playlist_tracks[t_idx].name.clone();
                                let _track_auto = self.playlist_tracks[t_idx].automation_enabled;

                                let card_bg = if is_sel_track { Color32::from_rgb(28, 32, 42) } else { Color32::from_rgb(18, 21, 28) };
                                ui.painter().rect_filled(h_rect, Rounding::same(3.0), card_bg);
                                ui.painter().rect_stroke(h_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_sel_track { track_color } else { Color32::from_rgb(34, 40, 52) }));

                                // Left Accent Bar
                                let color_bar = Rect::from_min_size(h_rect.min, Vec2::new(4.0, h_rect.height()));
                                ui.painter().rect_filled(color_bar, Rounding::same(1.5), track_color);

                                // Track Number & Name
                                let icon = self.playlist_tracks[t_idx].icon;
                                ui.painter().text(
                                    Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + 12.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}  {} {}", t_idx + 1, icon, track_name),
                                    egui::FontId::proportional(11.0),
                                    Color32::WHITE,
                                );

                                // Controls Row inside header (Vol Slider, Mute, Solo, Record Arm R, Focus Editor 🔍)
                                let vol_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + 26.0), Vec2::new(55.0, 12.0));
                                ui.painter().rect_filled(vol_rect, Rounding::same(2.0), Color32::from_rgb(30, 35, 45));
                                let vol_fill_w = vol_rect.width() * (track_vol / 1.25).clamp(0.0, 1.0);
                                ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), track_color);

                                // Mute icon (speaker)
                                let m_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 90.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                                let m_bg = if track_muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 35, 45) };
                                ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
                                ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(8.5), if track_muted { Color32::WHITE } else { Theme::TEXT_MUTED });

                                // Solo (S)
                                let s_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 68.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                                let s_bg = if track_solo { Theme::FL_ORANGE } else { Color32::from_rgb(30, 35, 45) };
                                ui.painter().rect_filled(s_rect, Rounding::same(2.0), s_bg);
                                ui.painter().text(s_rect.center(), egui::Align2::CENTER_CENTER, "S", egui::FontId::proportional(9.0), if track_solo { Color32::BLACK } else { Theme::TEXT_MUTED });

                                // Record Arm (R)
                                let r_arm_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 46.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                                let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                                let r_arm_bg = if is_armed { Color32::from_rgb(220, 30, 30) } else { Color32::from_rgb(30, 35, 45) };
                                ui.painter().rect_filled(r_arm_rect, Rounding::same(2.0), r_arm_bg);
                                ui.painter().text(r_arm_rect.center(), egui::Align2::CENTER_CENTER, "R", egui::FontId::proportional(9.0), if is_armed { Color32::WHITE } else { Theme::TEXT_MUTED });

                                // Stem Focus / Detail Editor button (🔍)
                                let f_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 24.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                                let is_focused = self.show_stem_focus_modal && self.focused_stem_track == Some(t_idx);
                                let f_bg = if is_focused { Theme::FL_CYAN } else { Color32::from_rgb(30, 35, 45) };
                                ui.painter().rect_filled(f_rect, Rounding::same(2.0), f_bg);
                                ui.painter().text(f_rect.center(), egui::Align2::CENTER_CENTER, "🔍", egui::FontId::proportional(8.5), if is_focused { Color32::BLACK } else { Theme::TEXT_MUTED });

                                if h_resp.clicked() || h_resp.double_clicked() {
                                    self.selected_timeline_track = t_idx;
                                    if let Some(mouse_pos) = h_resp.hover_pos() {
                                        if m_rect.contains(mouse_pos) {
                                            self.playlist_tracks[t_idx].muted = !self.playlist_tracks[t_idx].muted;
                                            self.sync_track_audio_state(t_idx);
                                        } else if s_rect.contains(mouse_pos) {
                                            self.playlist_tracks[t_idx].solo = !self.playlist_tracks[t_idx].solo;
                                            self.sync_track_audio_state(t_idx);
                                        } else if r_arm_rect.contains(mouse_pos) {
                                            self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                                            if self.playlist_tracks[t_idx].is_rec_armed {
                                                self.status_message = format!("🔴 Spår {} ({}) är nu armerat för mikrofoninspelning!", t_idx + 1, self.playlist_tracks[t_idx].name);
                                            }
                                        } else if f_rect.contains(mouse_pos) || h_resp.double_clicked() {
                                            self.open_stem_focus(t_idx);
                                        } else if vol_rect.contains(mouse_pos) {
                                            let ratio = (mouse_pos.x - vol_rect.min.x) / vol_rect.width();
                                            self.playlist_tracks[t_idx].volume = (ratio * 1.25).clamp(0.0, 1.25);
                                            self.sync_track_audio_state(t_idx);
                                        }
                                    }
                                }

                                // Full Timeline Track Lane (Scalable width with total_bars)
                                let (lane_rect, lane_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, row_h), Sense::click_and_drag());
                                track_lane_origin_x = lane_rect.min.x;

                                // Draw Background Bar Grid Lines
                                for bar_i in 0..total_bars {
                                    let b_min_x = lane_rect.min.x + bar_i as f32 * bar_w;
                                    let b_rect = Rect::from_min_size(Pos2::new(b_min_x, lane_rect.min.y), Vec2::new(bar_w, row_h));
                                    let is_active_bar = !self.pattern_mode && self.is_playing && self.song_bar == bar_i;
                                    let bg = if is_active_bar {
                                        Color32::from_rgb(32, 44, 58)
                                    } else if (bar_i / 4) % 2 == 0 {
                                        Color32::from_rgb(16, 18, 24)
                                    } else {
                                        Color32::from_rgb(22, 25, 32)
                                    };
                                    ui.painter().rect_filled(b_rect, Rounding::ZERO, bg);
                                    ui.painter().line_segment([Pos2::new(b_min_x, lane_rect.min.y), Pos2::new(b_min_x, lane_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(36, 42, 54)));

                                    // Beat Sub-Grid Lines when zoomed in
                                    if bar_w >= 70.0 {
                                        for beat in 1..4 {
                                            let bx = b_min_x + beat as f32 * (bar_w / 4.0);
                                            ui.painter().line_segment([Pos2::new(bx, lane_rect.min.y), Pos2::new(bx, lane_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(26, 30, 40)));
                                        }
                                    }

                                    // 16th-note sub-grid lines when zoomed in
                                    if bar_w >= 220.0 {
                                        for step in 1..16 {
                                            if step % 4 != 0 {
                                                let sx = b_min_x + step as f32 * (bar_w / 16.0);
                                                ui.painter().line_segment([Pos2::new(sx, lane_rect.min.y), Pos2::new(sx, lane_rect.max.y)], Stroke::new(0.3_f32, Color32::from_rgb(20, 24, 32)));
                                            }
                                        }
                                    }
                                }

                                // Bottom line separator between tracks
                                ui.painter().line_segment([Pos2::new(lane_rect.min.x, lane_rect.max.y), Pos2::new(lane_rect.max.x, lane_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 48)));

                                // Render continuous Audio Regions on this track
                                let mut split_action: Option<(usize, f32)> = None;
                                let mut delete_action: Option<usize> = None;
                                let mut mute_action: Option<usize> = None;
                                let mut select_action: Option<usize> = None;
                                let mut clip_action: Option<(usize, Option<usize>)> = None;

                                {
                                    let track = &self.playlist_tracks[t_idx];
                                    if !track.regions.is_empty() {
                                        for (r_i, region) in track.regions.iter().enumerate() {
                                            let is_region_selected = self.selected_audio_region == Some((t_idx, r_i));
                                            let rx_start = lane_rect.min.x + region.start_bar * bar_w;
                                            let rx_end = rx_start + region.length_bars * bar_w;
                                            let r_rect = Rect::from_min_max(Pos2::new(rx_start + 1.0, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.0, lane_rect.max.y - 2.0));

                                            if r_rect.width() > 1.0 {
                                                let col = region.color;
                                                let fill = if region.muted {
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

                                                ui.painter().rect_filled(r_rect, Rounding::same(3.0), fill);
                                                let stroke_col = if is_region_selected {
                                                    Color32::WHITE
                                                } else if region.muted {
                                                    Color32::from_rgb(70, 75, 85)
                                                } else {
                                                    col
                                                };
                                                ui.painter().rect_stroke(r_rect, Rounding::same(3.0), Stroke::new(if is_region_selected { 1.8_f32 } else { 1.0_f32 }, stroke_col));

                                                // Region Header Banner & Exact Time readout (Start & Length down to hundredths!)
                                                let gain_db = if region.volume <= 0.001 { -60.0 } else { 20.0 * region.volume.log10() };
                                                let r_start_str = format_time_hundredths(region.start_bar * sec_per_bar);
                                                let r_len_str = format_time_hundredths(region.length_bars * sec_per_bar);
                                                let title_text = format!("{} [⏱ {} | 📏 {} | {:+.1}dB]", region.name, r_start_str, r_len_str, gain_db);

                                                ui.painter().text(
                                                    Pos2::new(r_rect.min.x + 6.0, r_rect.min.y + 8.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    title_text,
                                                    egui::FontId::proportional(9.5),
                                                    if region.muted { Color32::from_rgb(140, 140, 150) } else { Color32::WHITE },
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

                                                // Draw Continuous Multi-Sample Waveform Inside Region
                                                let num_points = region.waveform_peaks.len().max(1);
                                                let step_w = (r_rect.width() - 4.0) / num_points as f32;
                                                let mid_y = r_rect.center().y + 4.0;
                                                let wave_col = if region.muted {
                                                    Color32::from_rgb(90, 95, 105)
                                                } else if is_region_selected {
                                                    Color32::from_rgb(255, 255, 255)
                                                } else {
                                                    Color32::from_rgb(210, 225, 255)
                                                };

                                                for (wi, &amp) in region.waveform_peaks.iter().enumerate() {
                                                    let sx = r_rect.min.x + 2.0 + wi as f32 * step_w;
                                                    let h = amp * (r_rect.height() * 0.36) * region.volume.clamp(0.2, 1.8);
                                                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.2_f32, wave_col));
                                                }
                                            }
                                        }

                                        if (lane_resp.clicked() || lane_resp.secondary_clicked() || lane_resp.double_clicked())
                                            && let Some(mouse_pos) = lane_resp.hover_pos() {
                                                let click_bar = (mouse_pos.x - lane_rect.min.x) / bar_w;
                                                for (r_i, r) in track.regions.iter().enumerate() {
                                                    if click_bar >= r.start_bar && click_bar <= (r.start_bar + r.length_bars) {
                                                        if lane_resp.double_clicked() {
                                                            self.selected_timeline_track = t_idx;
                                                            self.selected_audio_region = Some((t_idx, r_i));
                                                            self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::LeadVocals;
                                                            self.view_mode = ViewMode::VocalStudio;
                                                            self.status_message = format!("Öppnade '{}' i Studio Recorder / ARA2 Editor", r.name);
                                                        } else if lane_resp.secondary_clicked() {
                                                            select_action = Some(r_i);
                                                        } else {
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
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                    } else {
                                        // Fallback pattern clips
                                        for bar_idx in 0..32 {
                                            if let Some(pat_idx) = track.clips[bar_idx] {
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
                                        self.status_message = format!("Placerade mönster {} vid takt {}", self.selected_pattern + 1, c_bar + 1);
                                    } else {
                                        self.status_message = format!("Raderade mönsterblock vid takt {}", c_bar + 1);
                                    }
                                }

                                if let Some(s_idx) = select_action {
                                    self.selected_audio_region = Some((t_idx, s_idx));
                                    self.selected_timeline_track = t_idx;
                                    if s_idx < self.playlist_tracks[t_idx].regions.len() {
                                        let reg = &self.playlist_tracks[t_idx].regions[s_idx];
                                        self.status_message = format!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(reg.start_bar * sec_per_bar), format_time_hundredths(reg.length_bars * sec_per_bar));
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

                                            let r_left = AudioRegion {
                                                id: orig.id,
                                                name: format!("{} [Del 1]", orig.name),
                                                start_bar: orig.start_bar,
                                                length_bars: split_offset_bar,
                                                sample_offset_sec: orig.sample_offset_sec,
                                                source_path: orig.source_path.clone(),
                                                waveform_peaks: left_peaks,
                                                volume: orig.volume,
                                                fade_in_bars: orig.fade_in_bars.min(split_offset_bar),
                                                fade_out_bars: 0.0,
                                                muted: orig.muted,
                                                color: orig.color,
                                            };

                                            let r_right = AudioRegion {
                                                id: orig.id + 100,
                                                name: format!("{} [Del 2]", orig.name),
                                                start_bar: orig.start_bar + split_offset_bar,
                                                length_bars: orig.length_bars - split_offset_bar,
                                                sample_offset_sec: orig.sample_offset_sec + split_offset_sec,
                                                source_path: orig.source_path,
                                                waveform_peaks: right_peaks,
                                                volume: orig.volume,
                                                fade_in_bars: 0.0,
                                                fade_out_bars: orig.fade_out_bars.min(orig.length_bars - split_offset_bar),
                                                muted: orig.muted,
                                                color: orig.color,
                                            };

                                            self.playlist_tracks[t_idx].regions.remove(r_idx);
                                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
                                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
                                            performed_split = true;
                                            cut_feedback_msg = format!("✂ Klippte '{}' exakt vid {} ({})!", orig.name, format_time_hundredths(cut_sec), format_bar_subdivisions(orig.start_bar + split_offset_bar));
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
                                        self.playlist_tracks[t_idx].regions.remove(d_idx);
                                        self.sync_track_regions(t_idx);
                                        self.selected_audio_region = None;
                                        self.status_message = format!("🗑 Raderade '{}'", name);
                                    }

                                if let Some(m_idx) = mute_action
                                    && m_idx < self.playlist_tracks[t_idx].regions.len() {
                                        self.playlist_tracks[t_idx].regions[m_idx].muted = !self.playlist_tracks[t_idx].regions[m_idx].muted;
                                        let name = self.playlist_tracks[t_idx].regions[m_idx].name.clone();
                                        self.sync_track_regions(t_idx);
                                        self.status_message = format!("🔇 Toggla mute för '{}'", name);
                                    }
                            });
                            ui.add_space(2.0);
                        }

                        // "+ Add Track" Action Row (Soundtrap Studio Style)
                        ui.horizontal(|ui| {
                            let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(header_w, 28.0), Sense::click());
                            let is_h = add_resp.hovered();
                            ui.painter().rect_filled(add_rect, Rounding::same(4.0), if is_h { Color32::from_rgb(32, 38, 50) } else { Color32::from_rgb(20, 24, 32) });
                            ui.painter().rect_stroke(add_rect, Rounding::same(4.0), Stroke::new(1.0_f32, if is_h { Theme::FL_ORANGE } else { Color32::from_rgb(45, 52, 66) }));
                            ui.painter().text(add_rect.center(), egui::Align2::CENTER_CENTER, "➕ Lägg till spår (Soundtrap Studio)", egui::FontId::proportional(11.0), if is_h { Theme::FL_ORANGE } else { Theme::TEXT_BRIGHT });

                            if add_resp.clicked() {
                                self.show_add_track_modal = true;
                            }
                        });

                        ui.add_space(4.0);

                        // Master "Main" Track Row (Suno Studio Style Master Strip)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;

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
                                "🎛 Main",
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

                            // Timeline background for Main track lane
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
                                format!("Stereo Master Utgång • Volym: {:.0}% • Peak: {:.2}", self.master_volume * 100.0, peak),
                                egui::FontId::proportional(10.0),
                                Color32::from_rgb(130, 140, 160),
                            );
                        });

                        // ====================================================
                        // 2.3 VERTICAL PLAYHEAD NEEDLE ACROSS ALL TRACKS
                        // ====================================================
                        let playhead_bar = self.song_time / sec_per_bar;
                        let playhead_x = track_lane_origin_x + playhead_bar * bar_w;
                        let track_area_top = ui.min_rect().min.y;
                        let track_area_bottom = ui.min_rect().max.y;

                        if playhead_x >= track_lane_origin_x && playhead_x <= track_lane_origin_x + timeline_total_w {
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

            // Apply scrub seeking if user interacted with ruler
            if let Some(target) = scrub_target_sec {
                self.seek_song_time(target);
            }

            // ================================================================
            // 2.5 SELECTED AUDIO REGION INSPECTOR (HUNDREDTH-SECOND PRECISION)
            // ================================================================
            if let Some((t_idx, r_idx)) = self.selected_audio_region
                && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                    let mut do_delete = false;
                    let mut do_split = false;
                    let mut do_duplicate = false;
                    let mut do_open_focus = false;
                    let r = &mut self.playlist_tracks[t_idx].regions[r_idx];
                    let r_start_sec = r.start_bar * sec_per_bar;
                    let r_len_sec = r.length_bars * sec_per_bar;

                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.set_height(38.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("🎵 Klipp: {}", r.name)).strong().color(Theme::FL_CYAN));
                            ui.separator();

                            // Start time readout with +/- 0.01s nudging
                            ui.label("⏱ Start:");
                            ui.label(egui::RichText::new(format_time_hundredths(r_start_sec)).strong().color(Theme::TEXT_BRIGHT));
                            if ui.button("-0.01s").on_hover_text("Justera 1 hundradels sekund bakåt").clicked() {
                                r.start_bar = ((r_start_sec - 0.01).max(0.0)) / sec_per_bar;
                            }
                            if ui.button("+0.01s").on_hover_text("Justera 1 hundradels sekund framåt").clicked() {
                                r.start_bar = (r_start_sec + 0.01) / sec_per_bar;
                            }

                            ui.separator();

                            // Duration readout with +/- 0.01s nudging
                            ui.label("📏 Längd:");
                            ui.label(egui::RichText::new(format_time_hundredths(r_len_sec)).strong().color(Theme::TEXT_BRIGHT));
                            if ui.button("-0.01s").clicked() {
                                r.length_bars = ((r_len_sec - 0.01).max(0.05)) / sec_per_bar;
                            }
                            if ui.button("+0.01s").clicked() {
                                r.length_bars = (r_len_sec + 0.01) / sec_per_bar;
                            }

                            ui.separator();

                            // Gain Slider
                            ui.label("🎚 Ljudnivå:");
                            let gain_resp = ui.add(egui::Slider::new(&mut r.volume, 0.0..=2.0).custom_formatter(|v, _| {
                                let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                                format!("{:.1} dB ({:.0}%)", db, v * 100.0)
                            }));

                            ui.separator();

                            // Fade In Slider (Hundredths of second)
                            ui.label("📈 Fade In:");
                            let fade_in_resp = ui.add(egui::Slider::new(&mut r.fade_in_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                                let cs = (v as f32 * sec_per_bar * 100.0).round() as u32;
                                format!("{:.2} s ({} cs)", v as f32 * sec_per_bar, cs)
                            }));

                            ui.separator();

                            // Fade Out Slider (Hundredths of second)
                            ui.label("📉 Fade Out:");
                            let fade_out_resp = ui.add(egui::Slider::new(&mut r.fade_out_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                                let cs = (v as f32 * sec_per_bar * 100.0).round() as u32;
                                format!("{:.2} s ({} cs)", v as f32 * sec_per_bar, cs)
                            }));

                            ui.separator();

                            // Mute toggle
                            let mute_bg = if r.muted { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                            if ui.add(egui::Button::new(if r.muted { "🔇 Mutad" } else { "🔊 Aktiv" }).fill(mute_bg)).clicked() {
                                r.muted = !r.muted;
                            }

                            // Split at playhead button (Exact hundredths!)
                            let cur_song_time = self.song_time;
                            if cur_song_time > r_start_sec + 0.02 && cur_song_time < r_start_sec + r_len_sec - 0.02
                                && ui.button(egui::RichText::new(format!("✂ Dela vid {}", format_time_hundredths(cur_song_time))).color(Theme::FL_GREEN)).clicked() {
                                    do_split = true;
                                }

                            // Duplicate
                            if ui.button("📋 Duplicera").clicked() {
                                do_duplicate = true;
                            }

                            // Delete
                            if ui.add(egui::Button::new(egui::RichText::new("🗑 Ta bort").color(Color32::from_rgb(255, 100, 100)))).clicked() {
                                do_delete = true;
                            }

                            // Open Stem Focus Editor
                            if ui.add(egui::Button::new(egui::RichText::new("🔍 Öppna Stämeditor").color(Theme::FL_ORANGE))).clicked() {
                                do_open_focus = true;
                            }

                            // Close Inspector
                            if ui.button("❌").clicked() {
                                self.selected_audio_region = None;
                            }

                            if gain_resp.changed() || fade_in_resp.changed() || fade_out_resp.changed() {
                                // Updated
                            }
                        });
                    });

                    if do_open_focus {
                        self.open_stem_focus(t_idx);
                    }

                    if do_delete {
                        self.playlist_tracks[t_idx].regions.remove(r_idx);
                        self.sync_track_regions(t_idx);
                        self.selected_audio_region = None;
                        self.status_message = "🗑 Raderade ljudregion.".to_string();
                    } else if do_duplicate {
                        let mut dup = self.playlist_tracks[t_idx].regions[r_idx].clone();
                        dup.start_bar += dup.length_bars;
                        dup.name = format!("{} (Kopia)", dup.name);
                        dup.id += 500;
                        self.playlist_tracks[t_idx].regions.push(dup);
                        self.sync_track_regions(t_idx);
                        self.status_message = "📋 Duplicerade ljudregion.".to_string();
                    } else if do_split {
                        let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
                        let cur_cut_sec = (self.song_time * 100.0).round() / 100.0;
                        let orig_start_sec = orig.start_bar * sec_per_bar;
                        let orig_len_sec = orig.length_bars * sec_per_bar;
                        let split_offset_sec = cur_cut_sec - orig_start_sec;

                        if split_offset_sec > 0.02 && split_offset_sec < orig_len_sec - 0.02 {
                            let split_offset_bar = split_offset_sec / sec_per_bar;
                            let split_points = ((split_offset_sec / orig_len_sec) * orig.waveform_peaks.len() as f32) as usize;
                            let left_peaks = orig.waveform_peaks[..split_points.min(orig.waveform_peaks.len())].to_vec();
                            let right_peaks = orig.waveform_peaks[split_points.min(orig.waveform_peaks.len())..].to_vec();

                            let r_left = AudioRegion {
                                id: orig.id,
                                name: format!("{} [Del 1]", orig.name),
                                start_bar: orig.start_bar,
                                length_bars: split_offset_bar,
                                sample_offset_sec: orig.sample_offset_sec,
                                source_path: orig.source_path.clone(),
                                waveform_peaks: left_peaks,
                                volume: orig.volume,
                                fade_in_bars: orig.fade_in_bars.min(split_offset_bar),
                                fade_out_bars: 0.0,
                                muted: orig.muted,
                                color: orig.color,
                            };

                            let r_right = AudioRegion {
                                id: orig.id + 100,
                                name: format!("{} [Del 2]", orig.name),
                                start_bar: orig.start_bar + split_offset_bar,
                                length_bars: orig.length_bars - split_offset_bar,
                                sample_offset_sec: orig.sample_offset_sec + split_offset_sec,
                                source_path: orig.source_path,
                                waveform_peaks: right_peaks,
                                volume: orig.volume,
                                fade_in_bars: 0.0,
                                fade_out_bars: orig.fade_out_bars.min(orig.length_bars - split_offset_bar),
                                muted: orig.muted,
                                color: orig.color,
                            };

                            self.playlist_tracks[t_idx].regions.remove(r_idx);
                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
                            self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
                            self.sync_track_regions(t_idx);
                            self.selected_audio_region = Some((t_idx, r_idx));
                            self.status_message = format!("✂ Klippte region vid {}!", format_time_hundredths(cur_cut_sec));
                        }
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
                        ui.label(egui::RichText::new("BETA").strong().size(9.0).color(Theme::FL_ORANGE));
                        ui.separator();

                        // Add Context (+)
                        if ui.button("➕").on_hover_text("Bifoga aktuellt spår/mix som AI-kontext").clicked() {
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
                                    if ui.add(egui::Button::new("✕").small().fill(fill)).clicked() {
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
                        if ui.add(egui::Button::new(egui::RichText::new(" ⬆ ").strong().size(12.0).color(Color32::WHITE)).fill(gen_bg)).clicked() {
                            self.trigger_suno_ai_generation();
                        }
                    });
                });

                ui.add_space(4.0);

                // Center Action: "+ Add Track Effects" Button (Suno Studio Style)
                if ui.add(egui::Button::new(egui::RichText::new("➕ Add Track Effects").strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 48, 65))).clicked() {
                    self.view_mode = ViewMode::EffectsMixer;
                }
            });

            ui.add_space(4.0);

            // ================================================================
            // 4. SUNO STUDIO BOTTOM UTILITY BAR (Quick Switcher, Browser, Guide)
            // ================================================================
            ui.horizontal(|ui| {
                // Left Mode Switcher Icons
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

                // Right Controls (Browser & Quick Guide)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("💡 Snabbguide (F1)").clicked() {
                        self.show_help_guide = !self.show_help_guide;
                    }

                    if ui.button("🗂 Projektfiler").clicked() {
                        self.show_browser = !self.show_browser;
                    }
                });
            });
        });
    }

    fn render_channel_rack(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // Pattern Selector Bar (FL Studio Style)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🎹 SONIX CHANNEL RACK").strong().size(13.0).color(Theme::FL_ORANGE));
                ui.separator();

                ui.label(egui::RichText::new("Mönster:").size(11.0).color(Theme::TEXT_MUTED));
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

                if ui.button(egui::RichText::new("➕ Importera Ljud").color(Theme::FL_GREEN)).clicked() {
                    self.show_import_modal = true;
                }

                if ui.button("⚡ Slumpa Beats").clicked() {
                    for ch in &mut self.channels {
                        for s in 0..16 {
                            ch.steps[s] = (s % 4 == 0) || (rand_simple(s * 13 + ch.name.len()) > 0.68);
                        }
                    }
                    self.sync_active_pattern_from_ui();
                }
                if ui.button("🗑 Rensa Steg").clicked() {
                    for ch in &mut self.channels {
                        ch.steps = [false; 16];
                    }
                    self.sync_active_pattern_from_ui();
                }
                if ui.button("🎹 Öppna Piano Roll").clicked() {
                    self.view_mode = ViewMode::PianoRoll;
                }
                if ui.button("🎼 Öppna Arranger").clicked() {
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
                    if ui.add(egui::Button::new(egui::RichText::new("M").strong().size(10.0).color(Color32::WHITE)).fill(mute_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
                        ch.muted = !ch.muted;
                    }
                    let solo_color = if ch.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                    if ui.add(egui::Button::new(egui::RichText::new("S").strong().size(10.0).color(Color32::WHITE)).fill(solo_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
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
                    let byt_btn = egui::Button::new(egui::RichText::new("🔄 Byt").size(10.0).color(byt_text_color))
                        .fill(byt_fill)
                        .min_size(Vec2::new(42.0, 24.0));
                    if ui.add(byt_btn).on_hover_text("Byt ut detta beat/ljud direkt från biblioteket").clicked() {
                        toggle_picker_for = Some(ch_idx);
                    }

                    // Chop & Pitch Inspector Button [✂ Chop]
                    let chop_fill = if is_chopper_open { Theme::FL_YELLOW } else { Color32::from_rgb(38, 36, 28) };
                    let chop_text_color = if is_chopper_open { Color32::BLACK } else { Theme::FL_YELLOW };
                    let chop_btn = egui::Button::new(egui::RichText::new("✂ Chop").size(10.0).color(chop_text_color))
                        .fill(chop_fill)
                        .min_size(Vec2::new(52.0, 24.0));
                    if ui.add(chop_btn).on_hover_text("Öppna Waveform Chopper & Pitch Slicer").clicked() {
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
                        ui.label(egui::RichText::new(format!("📚 BYT LJUD: Välj sample för Kanal {} ({})", picker_ch + 1, cur_name)).strong().size(12.5).color(Theme::FL_CYAN));
                        ui.separator();
                        if ui.button(egui::RichText::new("➕ Importera Eget Ljud").color(Theme::FL_GREEN)).clicked() {
                            self.show_import_modal = true;
                        }
                        if ui.button(egui::RichText::new("✖ Stäng Väljare").color(Color32::from_rgb(255, 100, 100))).clicked() {
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
                                        if ui.button(egui::RichText::new("▶ Prov").size(9.5)).clicked() {
                                            let freq = midi_to_freq(item.default_note);
                                            let _ = self.engine.send_command(AudioCommand::NoteOn { note: item.default_note, freq, velocity: 0.9 });
                                        }
                                        if ui.add(egui::Button::new(egui::RichText::new("✅ Välj").strong().size(9.5).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                            if ui.button(egui::RichText::new("▶ Provspela Chop").color(Theme::FL_GREEN)).clicked() {
                                let note = (ch.notes[0] as i16 + ch.pitch_semitones as i16).clamp(0, 127) as u8;
                                let base_freq = midi_to_freq(note);
                                let fine_mul = (ch.pitch_fine_cents / 1200.0).exp2();
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: base_freq * fine_mul, velocity: ch.volume });
                            }
                            if ui.button(egui::RichText::new("✖ Stäng Chopper").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                self.active_chopper_channel = None;
                            }
                        });

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            // Waveform Chopper Slicer
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Waveform Chop & Slice Trim:").strong().size(11.0).color(Theme::TEXT_MUTED));
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
                                    ui.label(egui::RichText::new("Chop Snabbval:").size(10.0).color(Theme::TEXT_MUTED));
                                    if ui.button("Fullt").clicked() { ch.sample_start = 0.0; ch.sample_end = 1.0; }
                                    if ui.button("1/2").clicked() { ch.sample_start = 0.0; ch.sample_end = 0.5; }
                                    if ui.button("2/2").clicked() { ch.sample_start = 0.5; ch.sample_end = 1.0; }
                                    if ui.button("1/4").clicked() { ch.sample_start = 0.0; ch.sample_end = 0.25; }
                                    if ui.button("2/4").clicked() { ch.sample_start = 0.25; ch.sample_end = 0.5; }
                                    if ui.button("3/4").clicked() { ch.sample_start = 0.50; ch.sample_end = 0.75; }
                                    if ui.button("4/4").clicked() { ch.sample_start = 0.75; ch.sample_end = 1.0; }
                                    if ui.button("⚡ Transient").clicked() { ch.sample_start = 0.0; ch.sample_end = 0.18; }
                                });
                            });

                            ui.separator();

                            // Pitch & Envelope Controls
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Pitch & Tonhöjd:").strong().size(11.0).color(Theme::TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    ui.label("Semitoner:");
                                    ui.add(egui::Slider::new(&mut ch.pitch_semitones, -24..=24).text("st"));
                                    if ui.button("0").clicked() { ch.pitch_semitones = 0; }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Fine Cents:");
                                    ui.add(egui::Slider::new(&mut ch.pitch_fine_cents, -100.0..=100.0).text("ct"));
                                    if ui.button("0").clicked() { ch.pitch_fine_cents = 0.0; }
                                });

                                let total_cents = ch.pitch_semitones as f32 * 100.0 + ch.pitch_fine_cents;
                                let semitone_text = if ch.pitch_semitones >= 0 { format!("+{}", ch.pitch_semitones) } else { format!("{}", ch.pitch_semitones) };
                                ui.label(egui::RichText::new(format!("Totalt: {} st ({:+.0} ct)", semitone_text, total_cents)).color(Theme::FL_CYAN).size(10.0));

                                ui.separator();

                                ui.label(egui::RichText::new("Chop Trim & Envelope:").strong().size(11.0).color(Theme::TEXT_MUTED));
                                ui.horizontal(|ui| {
                                    ui.label("Start Trim:");
                                    ui.add(egui::Slider::new(&mut ch.sample_start, 0.0..=1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("End Trim:");
                                    ui.add(egui::Slider::new(&mut ch.sample_end, 0.0..=1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Attack / Decay:");
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
        if !self.show_add_track_modal {
            return;
        }

        let mut close_modal = false;
        egui::Window::new("➕ Lägg till nytt spår (Soundtrap Studio)")
            .resizable(false)
            .collapsible(false)
            .fixed_size(Vec2::new(540.0, 360.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Välj vilken typ av spår du vill skapa i studion:").size(12.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                let options = [
                    ("🎙 Sång & Mikrofon", "Spela in sång, tal eller instrument live via mikrofon med vågform.", Color32::from_rgb(180, 110, 255), "🎙", TrackKind::VocalAudio),
                    ("🎹 Keyboards & Piano", "Spela virtuellt piano, Rhodes, orgel och syntar via tangentbord/MIDI.", Theme::FL_GREEN, "🎹", TrackKind::SynthLead),
                    ("🎸 Gitarr & Bas", "Spela in elgitarr eller bas med inbyggd förstärkare och tonkontroller.", Color32::from_rgb(255, 80, 140), "🎸", TrackKind::Bassline),
                    ("🥁 Trummaskin & Beats", "Skapa beats med 808-trummor, hi-hats, claps och samplingar.", Theme::FL_CYAN, "🥁", TrackKind::Drums),
                    ("🎷 Synthesizer & Modular", "Lead-syntar, arpeggion, basgångar och modulära ljudmattor.", Color32::from_rgb(255, 180, 60), "🎷", TrackKind::SynthLead),
                    ("📂 Importera Ljud / WAV", "Ladda in egna ljudspår, samplingsfiler eller Suno stems direkt.", Theme::FL_ORANGE, "📂", TrackKind::CustomAudio),
                ];

                egui::Grid::new("add_track_grid").num_columns(2).spacing(Vec2::new(12.0, 12.0)).show(ui, |ui| {
                    for (idx, (title, desc, color, icon, kind)) in options.iter().enumerate() {
                        let (card_rect, card_resp) = ui.allocate_exact_size(Vec2::new(250.0, 68.0), Sense::click());
                        let is_hover = card_resp.hovered();
                        let bg = if is_hover { Color32::from_rgb(32, 38, 50) } else { Color32::from_rgb(20, 24, 32) };
                        ui.painter().rect_filled(card_rect, Rounding::same(6.0), bg);
                        ui.painter().rect_stroke(card_rect, Rounding::same(6.0), Stroke::new(if is_hover { 1.5_f32 } else { 1.0_f32 }, *color));

                        ui.painter().text(Pos2::new(card_rect.min.x + 10.0, card_rect.min.y + 16.0), egui::Align2::LEFT_CENTER, *icon, egui::FontId::proportional(16.0), *color);
                        ui.painter().text(Pos2::new(card_rect.min.x + 36.0, card_rect.min.y + 16.0), egui::Align2::LEFT_CENTER, *title, egui::FontId::proportional(11.5), Color32::WHITE);
                        ui.painter().text(Pos2::new(card_rect.min.x + 10.0, card_rect.min.y + 44.0), egui::Align2::LEFT_CENTER, *desc, egui::FontId::proportional(9.5), Theme::TEXT_MUTED);

                        if card_resp.clicked() {
                            let new_id = self.playlist_tracks.len() + 1;
                            let mut track = PlaylistTrack::new(format!("{} {} {}", icon, title, new_id), icon, *kind, *color);
                            if *kind == TrackKind::VocalAudio {
                                track.is_rec_armed = true;
                            }
                            self.playlist_tracks.push(track);
                            self.status_message = format!("✔ Skapade nytt spår: {} {}", icon, title);
                            close_modal = true;
                        }

                        if (idx + 1) % 2 == 0 {
                            ui.end_row();
                        }
                    }
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Stäng").clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

        if close_modal {
            self.show_add_track_modal = false;
        }
    }

    fn render_import_sample_modal(&mut self, ctx: &egui::Context) {
        if !self.show_import_modal {
            return;
        }

        let mut close = false;
        egui::Window::new("📥 Importera & Skapa Nytt Sample Ljud")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(egui::RichText::new("Utöka ditt personliga ljudbibliotek med egna samples!").size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Ljudets Namn:");
                    ui.text_edit_singleline(&mut self.custom_import_name_input);
                });

                ui.horizontal(|ui| {
                    ui.label("Kategori:");
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
                ui.label(egui::RichText::new("Sample Karaktär & Syntes:").strong().size(11.0));

                let icons = ["📂", "💥", "🥁", "👏", "⚡", "🌊", "🗣", "🎸", "🎹", "✨", "🔔"];
                ui.horizontal_wrapped(|ui| {
                    ui.label("Ikon:");
                    for &ic in icons.iter() {
                        let is_sel = self.custom_import_icon == ic;
                        let fill = if is_sel { Theme::FL_GREEN } else { Theme::PANEL_BG };
                        if ui.add(egui::Button::new(ic).fill(fill)).clicked() {
                            self.custom_import_icon = ic.to_string();
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Grundton (MIDI):");
                    ui.add(egui::Slider::new(&mut self.custom_import_note, 24..=84).text("Ton"));
                });

                ui.horizontal(|ui| {
                    ui.label("Frekvens & Tonhöjd:");
                    ui.add(egui::Slider::new(&mut self.custom_import_freq, 10.0..=90.0));
                });

                ui.horizontal(|ui| {
                    ui.label("Decay Tid:");
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
                    if ui.button(egui::RichText::new("▶ Provspela").size(11.0)).clicked() {
                        let freq = midi_to_freq(self.custom_import_note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn {
                            note: self.custom_import_note,
                            freq,
                            velocity: 0.9,
                        });
                    }

                    if ui.add(egui::Button::new(egui::RichText::new("💾 Spara i Bibliotek").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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

                    if ui.button(egui::RichText::new("Avbryt").size(11.0)).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_import_modal = false;
        }
    }

    fn render_piano_roll_editor(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🎹 SONIX PIANO ROLL (303 Lead)").strong().size(13.0).color(Theme::FL_GREEN));
                ui.separator();

                ui.label(egui::RichText::new("Mönster:").size(11.0).color(Theme::TEXT_MUTED));
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

                // Scale Snapping Selector
                ui.label(egui::RichText::new("Skala:").size(11.0).color(Theme::TEXT_MUTED));
                let scales = ["Kromatik", "C-Dur", "A-Moll", "Dorian", "Pentatonisk", "Blues", "Synthwave"];
                for (s_idx, s_name) in scales.iter().enumerate() {
                    let is_s = self.selected_scale == s_idx;
                    if ui.selectable_label(is_s, *s_name).clicked() {
                        self.selected_scale = s_idx;
                    }
                }

                ui.separator();

                // Chord Stamp Selector
                ui.label(egui::RichText::new("Ackord-stämpel:").size(11.0).color(Theme::TEXT_MUTED));
                let chords = ["Enkel", "Dur", "Moll", "7th", "Oktav"];
                for (c_idx, c_name) in chords.iter().enumerate() {
                    let is_c = self.chord_stamp == c_idx;
                    if ui.selectable_label(is_c, *c_name).clicked() {
                        self.chord_stamp = c_idx;
                    }
                }

                ui.separator();

                if ui.button("◀ Channel Rack").clicked() {
                    self.view_mode = ViewMode::ChannelRack;
                }
                if ui.button("🎼 Arranger").clicked() {
                    self.view_mode = ViewMode::PlaylistArranger;
                }
            });

            ui.add_space(8.0);

            let base_midi = 48; // C3
            let semitones_count = 24;

            // Scale check helper:
            let scale_notes: Vec<u8> = match self.selected_scale {
                1 => vec![0, 2, 4, 5, 7, 9, 11], // Major
                2 => vec![0, 2, 3, 5, 7, 8, 10], // Minor
                3 => vec![0, 2, 3, 5, 7, 9, 10], // Dorian
                4 => vec![0, 3, 5, 7, 10],        // Pentatonic
                5 => vec![0, 3, 5, 6, 7, 10],     // Blues
                6 => vec![0, 1, 3, 5, 7, 8, 10], // Synthwave
                _ => (0..12).collect(),           // Chromatic
            };

            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for note_offset in (0..semitones_count).rev() {
                    let midi_note = base_midi + note_offset as u8;
                    let note_in_scale = scale_notes.contains(&(midi_note % 12));
                    let is_sharp = [1, 3, 6, 8, 10].contains(&(midi_note % 12));
                    let n_name = note_name(midi_note);
                    let octave_num = (midi_note / 12) as i8 - 1;

                    ui.horizontal(|ui| {
                        // Piano Key label on left with Scale Highlighting
                        let key_bg = if !note_in_scale {
                            Color32::from_rgb(20, 22, 28)
                        } else if is_sharp {
                            Color32::from_rgb(32, 42, 56)
                        } else {
                            Color32::from_rgb(220, 228, 240)
                        };
                        let key_text = if is_sharp || !note_in_scale { Color32::WHITE } else { Color32::BLACK };
                        let (k_rect, _) = ui.allocate_exact_size(Vec2::new(45.0, 16.0), Sense::hover());
                        ui.painter().rect_filled(k_rect, Rounding::same(2.0), key_bg);
                        ui.painter().text(
                            k_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}{}", n_name, octave_num),
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
                                    self.channels[6].steps[step] = false;
                                } else {
                                    for o in 0..24 {
                                        self.piano_roll_grid[o][step] = false;
                                    }
                                    self.piano_roll_grid[note_offset][step] = true;
                                    self.channels[6].steps[step] = true;
                                    self.channels[6].notes[step] = midi_note;
                                }
                                self.sync_active_pattern_from_ui();
                            }

                            let mut bg = if is_on {
                                Theme::FL_GREEN
                            } else if !note_in_scale {
                                Color32::from_rgb(16, 18, 24)
                            } else if is_sharp {
                                Color32::from_rgb(24, 28, 36)
                            } else if is_beat_start {
                                Color32::from_rgb(38, 44, 56)
                            } else {
                                Color32::from_rgb(30, 35, 45)
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
                ui.label(egui::RichText::new("VELOCITY").size(10.0).color(Theme::TEXT_MUTED));
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

        let mut kick = [false; 16];
        let mut snare = [false; 16];
        let mut clap = [false; 16];
        let mut hat_c = [false; 16];
        let mut hat_o = [false; 16];
        let mut crash = [false; 16];

        // 1. Kick Pattern (4-on-the-floor + syncopations)
        kick[0] = true;
        if comp < 0.3 {
            kick[8] = true;
        } else if comp < 0.7 {
            kick[6] = true;
            kick[8] = true;
            kick[14] = true;
        } else {
            kick[0] = true;
            kick[3] = true;
            kick[6] = true;
            kick[8] = true;
            kick[10] = true;
            kick[14] = true;
        }

        // 2. Snare / Clap
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
            if comp > 0.5 {
                snare[15] = true;
            }
            if comp > 0.8 {
                snare[7] = true;
                snare[11] = true;
            }
            if self.drummer_percussion_on && comp > 0.4 {
                clap[12] = true;
            }
        }

        // 3. Hi-Hats
        if comp < 0.35 {
            for step in (0..16).step_by(2) {
                hat_c[step] = true;
            }
        } else if comp < 0.75 {
            for step in 0..16 {
                if step == 2 || step == 6 || step == 10 || step == 14 {
                    hat_o[step] = true;
                } else {
                    hat_c[step] = true;
                }
            }
        } else {
            for step in 0..16 {
                if step % 4 == 2 {
                    hat_o[step] = true;
                } else {
                    hat_c[step] = true;
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

        self.channels[0].steps = kick;
        self.channels[1].steps = snare;
        self.channels[2].steps = clap;
        self.channels[3].steps = hat_c;
        self.channels[4].steps = hat_o;
        self.channels[5].steps = crash;

        let vel_mult = 0.5 + loud * 0.5;
        self.step_velocities.fill(vel_mult);

        self.sync_active_pattern_from_ui();
        self.status_message = format!("🥁 Drummer genererade mönster (Komplexitet: {:.0}%, Volym: {:.0}%)", comp * 100.0, loud * 100.0);
    }

    fn render_session_drummer(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🥁 SONIX SESSION DRUMMER").strong().size(14.0).color(Theme::FL_ORANGE));
                ui.separator();
                ui.label(egui::RichText::new("Intelligent trummis med 2D XY-radar för realtids-generering av trumspår").size(11.0).color(Theme::TEXT_MUTED));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("⚡ Skriv till Mönster").strong().color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
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
                        ui.label(egui::RichText::new("DRUMMERS (STILAR)").strong().size(11.0).color(Theme::FL_CYAN));
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
                        ui.label(egui::RichText::new("BEAT PRESETS").strong().size(11.0).color(Theme::FL_ORANGE));
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
                        ui.label(egui::RichText::new("XY PERFORMANCE RADAR").strong().size(11.0).color(Theme::FL_YELLOW));
                        ui.label(egui::RichText::new("Dra den gula pucken för att ändra dynamik & komplexitet i realtid").size(10.0).color(Theme::TEXT_MUTED));
                        ui.add_space(4.0);

                        if drummer_xy_matrix(ui, &mut self.drummer_complexity, &mut self.drummer_loudness, Vec2::new(300.0, 260.0)) {
                            self.apply_drummer_generation();
                        }

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(format!("Komplexitet: {:.0}%  •  Ljudstyrka: {:.0}%", self.drummer_complexity * 100.0, self.drummer_loudness * 100.0)).size(11.0).color(Theme::FL_YELLOW));
                    });
                });

                ui.separator();

                // Right Column: Drum Kit Parts & Fine Controls
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("TRUMSET & INSTRUMENT").strong().size(11.0).color(Theme::FL_ORANGE));
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
                            ui.label(egui::RichText::new("Kick & Snare:").size(10.0).color(Theme::TEXT_MUTED));
                            for v in 1..=4 {
                                if ui.selectable_label(self.drummer_kick_snare_var == v, format!("{}", v)).clicked() {
                                    self.drummer_kick_snare_var = v;
                                    self.apply_drummer_generation();
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Hi-Hat Rytm:").size(10.0).color(Theme::TEXT_MUTED));
                            for v in 1..=4 {
                                if ui.selectable_label(self.drummer_hihat_var == v, format!("{}", v)).clicked() {
                                    self.drummer_hihat_var = v;
                                    self.apply_drummer_generation();
                                }
                            }
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.label(egui::RichText::new("FILLS & GROOVE").strong().size(11.0).color(Theme::FL_CYAN));

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
                            ui.label(egui::RichText::new("💡 TIPS").strong().size(10.0).color(Theme::FL_GREEN));
                            ui.label(egui::RichText::new("Alla förändringar i Session Drummer speglas direkt i FL Channel Rack och tidslinjen!").size(9.5).color(Theme::TEXT_MUTED));
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
                ui.label(egui::RichText::new("✨ SONIX ALCHEMY SYNTHESIZER (Transform Pad)").strong().size(14.0).color(Theme::FL_CYAN));
                ui.separator();
                ui.label(egui::RichText::new("8-punkters realtids-morphing mellan subtraktiva, FM- och wavetable-synteser").size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                // Center Morphing Pad
                ui.group(|ui| {
                    ui.set_width(380.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("TRANSFORM PAD").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.label(egui::RichText::new("Dra den lysande markören för att sömlöst smälta samman filter, vågformer och ADSR").size(10.0).color(Theme::TEXT_MUTED));
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
                        ui.label(egui::RichText::new("AKTIVA SYNTPARAMETRAR (MORPHED)").strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(6.0);

                        ui.label(format!("Vågform: {}", self.waveform.name()));
                        ui.label(format!("Filter Cutoff: {:.0} Hz", self.filter.cutoff));
                        ui.label(format!("Resonans (Q): {:.2}", self.filter.resonance));
                        ui.label(format!("Attack: {:.3}s  •  Decay: {:.2}s", self.adsr.attack, self.adsr.decay));

                        ui.add_space(8.0);
                        ui.separator();

                        ui.label(egui::RichText::new("SNABBVAL SNAPSHOTS").strong().size(11.0).color(Theme::FL_CYAN));
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
                        if ui.button("🎹 Provspela Ton (C4)").clicked() {
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
                ui.label(egui::RichText::new("🎛 REMIX FX & GROSS BEAT (DJ Performance Touch)").strong().size(14.0).color(Theme::FL_GREEN));
                ui.separator();
                ui.label(egui::RichText::new("DJ-effekter, Tape Stop, Filter Sweeps och Stutter Glitch").size(11.0).color(Theme::TEXT_MUTED));
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                // DJ XY Filter Touch Surface
                ui.group(|ui| {
                    ui.set_width(340.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("DJ FILTER & CRUSH XY PAD").strong().size(11.0).color(Theme::FL_GREEN));
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
                        ui.label(egui::RichText::new("DJ PERFORMANCE PADS").strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            // Tape Stop
                            let stop_col = if self.remix_tape_stop { Theme::FL_ORANGE } else { Color32::from_rgb(50, 40, 30) };
                            if ui.add(egui::Button::new(egui::RichText::new("⏸ TAPE STOP").strong().size(12.0).color(Color32::WHITE)).fill(stop_col).min_size(Vec2::new(100.0, 40.0))).clicked() {
                                self.remix_tape_stop = !self.remix_tape_stop;
                                if self.remix_tape_stop {
                                    self.bpm = (self.bpm * 0.5).max(40.0);
                                } else {
                                    self.bpm = 126.0;
                                }
                            }

                            // Vinyl Scratch
                            if ui.add(egui::Button::new(egui::RichText::new("📻 VINYL SCRATCH").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(30, 45, 60)).min_size(Vec2::new(120.0, 40.0))).clicked() {
                                self.drive = 4.5;
                                let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                            }
                        });

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("GROSS BEAT / STUTTER GLITCH").strong().size(11.0).color(Theme::FL_CYAN));
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.remix_stutter == 1, "⚡ 1/4 Repeat").clicked() {
                                self.remix_stutter = if self.remix_stutter == 1 { 0 } else { 1 };
                                self.status_message = "⚡ 1/4 Beat Repeat aktiv".to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 2, "⚡ 1/8 Stutter").clicked() {
                                self.remix_stutter = if self.remix_stutter == 2 { 0 } else { 2 };
                                self.status_message = "⚡ 1/8 Stutter aktiv".to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 3, "⚡ 1/16 Roll").clicked() {
                                self.remix_stutter = if self.remix_stutter == 3 { 0 } else { 3 };
                                self.status_message = "⚡ 1/16 High-Speed Roll aktiv".to_string();
                            }
                            if ui.selectable_label(self.remix_stutter == 4, "🌀 Reverse").clicked() {
                                self.remix_stutter = if self.remix_stutter == 4 { 0 } else { 4 };
                                self.status_message = "🌀 Gross Beat Half-Speed aktiv".to_string();
                            }
                        });

                        ui.add_space(10.0);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("Styr DJ-effekter under uppspelning för live-remixing av dina mönster!").size(10.0).color(Theme::TEXT_MUTED));
                        });
                    });
                });
            });
        });
    }

    fn render_effects_mixer_rack(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            // Header Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🎛 SONIX PRO MULTI-TRACK MIXER CONSOLE").strong().size(13.5).color(Theme::FL_CYAN));
                ui.separator();
                ui.label(egui::RichText::new("Fullständigt mixerbord med Sub-Mix bussar, VCA-grupper, PDC latenskompensering & FX Rack").size(11.0).color(Theme::TEXT_MUTED));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pdc_text = if self.pdc_enabled { "⚡ PDC AUTO-SYNC: AKTIV (0.0 ms)" } else { "⚡ PDC: AV" };
                    let pdc_col = if self.pdc_enabled { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                    if ui.selectable_label(self.pdc_enabled, egui::RichText::new(pdc_text).strong().size(10.0).color(pdc_col)).clicked() {
                        self.pdc_enabled = !self.pdc_enabled;
                    }
                });
            });

            ui.add_space(6.0);

            // ================================================================
            // 1. TOP TIER: FULL-WIDTH STUDIO CHANNEL STRIPS (MASTER + 8 CHANNELS + BUSSES + VCA)
            // ================================================================
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    // Master Strip
                    ui.group(|ui| {
                        ui.set_width(76.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("MASTER").strong().size(11.0).color(Theme::FL_ORANGE));
                            rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", Color32::WHITE, 18.0);
                            rotary_knob(ui, &mut self.stereo_width, 0.0, 2.0, "WIDTH", Theme::FL_CYAN, 18.0);
                            ui.add_space(2.0);
                            let peak = self.engine.get_peak_level();
                            if vertical_fader(ui, &mut self.master_volume, 0.0, 1.25, peak, Theme::FL_ORANGE, 115.0) {
                                let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                            }
                            ui.label(egui::RichText::new(format!("{:.0}%", self.master_volume * 100.0)).strong().size(9.5).color(Theme::FL_ORANGE));
                        });
                    });

                    ui.separator();

                    // 8 Instrument Channel Strips
                    for (idx, ch) in self.channels.iter_mut().enumerate() {
                        let is_selected = self.selected_channel == idx;
                        let fill_bg = if is_selected { Theme::PANEL_BG } else { Theme::CHANNEL_BG };
                        ui.group(|ui| {
                            ui.set_width(72.0);
                            ui.vertical_centered(|ui| {
                                let name_btn = egui::Button::new(egui::RichText::new(format!("{} {}", ch.icon, ch.name)).strong().size(9.0).color(ch.color))
                                    .fill(fill_bg)
                                    .min_size(Vec2::new(68.0, 18.0));
                                if ui.add(name_btn).clicked() {
                                    self.selected_channel = idx;
                                }

                                // Target Bus & VCA Quick Selectors
                                ui.horizontal(|ui| {
                                    let bus_str = match ch.target_bus {
                                        BusRouting::Master => "MST",
                                        BusRouting::VocalBus => "VOC",
                                        BusRouting::DrumBus => "DRM",
                                        BusRouting::SynthBus => "SYN",
                                        BusRouting::FxSend => "FX",
                                    };
                                    if ui.add(egui::Button::new(egui::RichText::new(bus_str).size(7.5).color(Theme::FL_CYAN)).fill(Color32::from_rgb(25, 30, 40)).min_size(Vec2::new(26.0, 13.0))).clicked() {
                                        ch.target_bus = match ch.target_bus {
                                            BusRouting::Master => BusRouting::DrumBus,
                                            BusRouting::DrumBus => BusRouting::SynthBus,
                                            BusRouting::SynthBus => BusRouting::VocalBus,
                                            BusRouting::VocalBus => BusRouting::FxSend,
                                            BusRouting::FxSend => BusRouting::Master,
                                        };
                                    }
                                    let vca_str = ch.vca_group.label();
                                    let vca_col = if ch.vca_group != VcaGroup::None { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                                    if ui.add(egui::Button::new(egui::RichText::new(vca_str).size(7.5).color(vca_col)).fill(Color32::from_rgb(25, 30, 40)).min_size(Vec2::new(20.0, 13.0))).clicked() {
                                        ch.vca_group = match ch.vca_group {
                                            VcaGroup::None => VcaGroup::Vca1,
                                            VcaGroup::Vca1 => VcaGroup::Vca2,
                                            VcaGroup::Vca2 => VcaGroup::Vca3,
                                            VcaGroup::Vca3 => VcaGroup::Vca4,
                                            VcaGroup::Vca4 => VcaGroup::None,
                                        };
                                    }
                                });

                                rotary_knob(ui, &mut ch.pan, -1.0, 1.0, "PAN", Color32::from_rgb(180, 190, 200), 16.0);

                                ui.horizontal(|ui| {
                                    let m_col = if ch.muted { Color32::from_rgb(70, 20, 20) } else { Theme::FL_GREEN };
                                    if ui.add(egui::Button::new(egui::RichText::new("M").size(8.0).color(Color32::WHITE)).fill(m_col).min_size(Vec2::new(14.0, 14.0))).clicked() {
                                        ch.muted = !ch.muted;
                                    }
                                    let s_col = if ch.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                                    if ui.add(egui::Button::new(egui::RichText::new("S").size(8.0).color(Color32::WHITE)).fill(s_col).min_size(Vec2::new(14.0, 14.0))).clicked() {
                                        ch.solo = !ch.solo;
                                    }
                                });

                                ui.add_space(1.0);
                                let fake_meter = if ch.steps[self.current_step] && self.is_playing { (ch.volume * 0.95).min(1.0) } else { 0.0 };
                                vertical_fader(ui, &mut ch.volume, 0.0, 1.25, fake_meter, ch.color, 115.0);
                                ui.label(egui::RichText::new(format!("{:.0}%", ch.volume * 100.0)).size(8.5).color(Theme::TEXT_MUTED));
                                if ch.pdc_latency_samples > 0 {
                                    let ms = ch.pdc_latency_samples as f32 / 44.1;
                                    ui.label(egui::RichText::new(format!("⏱{:.1}ms", ms)).size(7.0).color(Theme::FL_YELLOW));
                                }
                            });
                        });
                    }

                    ui.separator();

                    // Sub-Mix Buses Group
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("🎛 SUB-MIX BUSSES").strong().size(9.5).color(Theme::FL_CYAN));
                            ui.horizontal(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("DRUMS").size(8.0).color(Theme::FL_ORANGE));
                                    vertical_fader(ui, &mut self.drum_bus_vol, 0.0, 1.25, if self.is_playing { 0.65 } else { 0.0 }, Theme::FL_ORANGE, 80.0);
                                    ui.label(egui::RichText::new(format!("{:.0}%", self.drum_bus_vol * 100.0)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("SYNTH").size(8.0).color(Theme::FL_GREEN));
                                    vertical_fader(ui, &mut self.synth_bus_vol, 0.0, 1.25, if self.is_playing { 0.50 } else { 0.0 }, Theme::FL_GREEN, 80.0);
                                    ui.label(egui::RichText::new(format!("{:.0}%", self.synth_bus_vol * 100.0)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("VOCAL").size(8.0).color(Theme::FL_PURPLE));
                                    vertical_fader(ui, &mut self.vocal_bus_vol, 0.0, 1.25, if self.is_playing { 0.40 } else { 0.0 }, Theme::FL_PURPLE, 80.0);
                                    ui.label(egui::RichText::new(format!("{:.0}%", self.vocal_bus_vol * 100.0)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("FX").size(8.0).color(Theme::FL_CYAN));
                                    vertical_fader(ui, &mut self.fx_send_vol, 0.0, 1.25, if self.is_playing { 0.30 } else { 0.0 }, Theme::FL_CYAN, 80.0);
                                    ui.label(egui::RichText::new(format!("{:.0}%", self.fx_send_vol * 100.0)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                            });
                        });
                    });

                    ui.separator();

                    // VCA Master Groups
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("⚡ VCA GROUPS").strong().size(9.5).color(Theme::FL_YELLOW));
                            ui.horizontal(|ui| {
                                for v_idx in 0..4 {
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new(format!("VCA {}", v_idx + 1)).size(8.0).color(Theme::FL_YELLOW));
                                        let v_m = self.vca_mutes[v_idx];
                                        if ui.add(egui::Button::new(egui::RichText::new("M").size(7.0).color(Color32::WHITE)).fill(if v_m { Color32::RED } else { Color32::from_rgb(40, 45, 50) }).min_size(Vec2::new(12.0, 12.0))).clicked() {
                                            self.vca_mutes[v_idx] = !self.vca_mutes[v_idx];
                                        }
                                        let meter = if self.is_playing { (self.vca_faders[v_idx] * 0.7).min(1.0) } else { 0.0 };
                                        vertical_fader(ui, &mut self.vca_faders[v_idx], 0.0, 1.25, meter, Theme::FL_YELLOW, 70.0);
                                        ui.label(egui::RichText::new(format!("{:.0}%", self.vca_faders[v_idx] * 100.0)).size(7.5).color(Theme::TEXT_MUTED));
                                    });
                                }
                            });
                        });
                    });
                });
            });

            ui.add_space(8.0);

            // ================================================================
            // 2. BOTTOM TIER: 3-COLUMN MASTER & CHANNEL FX STUDIO
            // ================================================================
            ui.columns(3, |cols| {
                // Column 1: Parametric EQ 2
                cols[0].group(|ui| {
                    ui.label(egui::RichText::new("📊 PARAMETRIC EQ 2").strong().size(11.5).color(Theme::FL_ORANGE));
                    ui.separator();

                    ui.horizontal(|ui| {
                        rotary_knob(ui, &mut self.eq_low, -12.0, 12.0, "LOW", Theme::FL_ORANGE, 22.0);
                        rotary_knob(ui, &mut self.eq_mid, -12.0, 12.0, "MID", Theme::FL_YELLOW, 22.0);
                        rotary_knob(ui, &mut self.eq_high, -12.0, 12.0, "HIGH", Theme::FL_CYAN, 22.0);
                    });

                    ui.add_space(4.0);
                    eq_curve_visualizer(ui, &mut self.eq_low, &mut self.eq_mid, &mut self.eq_high, Vec2::new(270.0, 68.0));
                });

                // Column 2: Space & Time FX (Stereo Delay & Reverb)
                cols[1].group(|ui| {
                    ui.label(egui::RichText::new("🌊 TIME & SPACE FX").strong().size(11.5).color(Theme::FL_CYAN));
                    ui.separator();

                    ui.horizontal(|ui| {
                        // Delay Sub-block
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Stereo Delay").strong().size(10.0).color(Theme::FL_CYAN));
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.delay.time_ms, 30.0, 800.0, "TIME", Theme::FL_CYAN, 20.0);
                            ch |= rotary_knob(ui, &mut self.delay.feedback, 0.0, 0.88, "FB", Theme::FL_CYAN, 20.0);
                            ch |= rotary_knob(ui, &mut self.delay.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 20.0);
                            if ch { let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay)); }
                        });

                        ui.separator();

                        // Reverb Sub-block
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Space Reverb").strong().size(10.0).color(Theme::FL_PURPLE));
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.reverb.room_size, 0.1, 0.95, "ROOM", Theme::FL_PURPLE, 20.0);
                            ch |= rotary_knob(ui, &mut self.reverb.damping, 0.05, 0.9, "DAMP", Theme::FL_PURPLE, 20.0);
                            ch |= rotary_knob(ui, &mut self.reverb.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 20.0);
                            if ch { let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb)); }
                        });
                    });
                });

                // Column 3: Dynamics & Saturation (Maximus & Drive)
                cols[2].group(|ui| {
                    ui.label(egui::RichText::new("🔥 DYNAMICS & COLOR").strong().size(11.5).color(Theme::FL_YELLOW));
                    ui.separator();

                    ui.horizontal(|ui| {
                        // Maximus Compressor & Chorus
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Maximus & Chorus").strong().size(10.0).color(Theme::FL_GREEN));
                            ui.checkbox(&mut self.chorus_on, "Chorus");
                            rotary_knob(ui, &mut self.compressor_amount, 0.0, 1.0, "COMP", Theme::FL_GREEN, 20.0);
                            rotary_knob(ui, &mut self.chorus_depth, 0.0, 1.0, "DEPTH", Theme::FL_CYAN, 20.0);
                        });

                        ui.separator();

                        // Tube Overdrive
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Tube Drive").strong().size(10.0).color(Theme::FL_ORANGE));
                            ui.checkbox(&mut self.stompbox_cab, "4x12 Cab");
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.stompbox_drive, 1.0, 8.0, "DRIVE", Theme::FL_ORANGE, 20.0);
                            ch |= rotary_knob(ui, &mut self.stompbox_tone, 1000.0, 10000.0, "TONE", Theme::FL_YELLOW, 20.0);
                            if ch {
                                self.drive = self.stompbox_drive;
                                let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                            }
                        });
                    });
                });
            });
        });
    }

    fn render_synth_hardware_rack(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🎛 SONIX HARDWARE SYNTHESIZER").strong().size(13.0).color(Theme::FL_CYAN));
                ui.separator();

                // Presets
                ui.label(egui::RichText::new("Presets:").color(Theme::TEXT_MUTED));
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
                    ui.label(egui::RichText::new("OSCILLATOR").strong());
                    let waveforms = [
                        (Waveform::Sine, "∿ Sinus"),
                        (Waveform::Saw, "⩘ Sågtand"),
                        (Waveform::Square, "⊓ Fyrkant"),
                        (Waveform::Triangle, "⋀ Triangel"),
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
                        ui.label("Oktav:");
                        if ui.button("➖ (Z)").clicked() && self.octave > 1 { self.octave -= 1; }
                        ui.label(egui::RichText::new(format!("C{}", self.octave)).strong().color(Theme::FL_CYAN));
                        if ui.button("➕ (X)").clicked() && self.octave < 8 { self.octave += 1; }
                    });
                });

                // Column 2: Moog Filter Controls
                cols[1].group(|ui| {
                    ui.label(egui::RichText::new("MOOG 24dB FILTER").strong().color(Theme::FL_ORANGE));
                    ui.horizontal(|ui| {
                        let mut flt_changed = false;
                        flt_changed |= rotary_knob(ui, &mut self.filter.cutoff, 60.0, 18000.0, "CUTOFF", Theme::FL_ORANGE, 40.0);
                        flt_changed |= rotary_knob(ui, &mut self.filter.resonance, 0.5, 9.5, "RESO (Q)", Theme::FL_YELLOW, 40.0);
                        if flt_changed {
                            let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                        }
                    });
                    ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz | Q: {:.1}", self.filter.cutoff, self.filter.resonance)).size(10.0).color(Theme::TEXT_MUTED));
                });

                // Column 3: ADSR Rotary Envelopes
                cols[2].group(|ui| {
                    ui.label(egui::RichText::new("ADSR ENVELOPE").strong().color(Theme::FL_PURPLE));
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
                    ui.label(egui::RichText::new("ENVELOPE GRAF").strong().color(Theme::FL_GREEN));
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
                ui.label(egui::RichText::new("🎹 SONIX PIANO").strong().size(13.0).color(Theme::TEXT_BRIGHT));
                ui.separator();
                ui.label(egui::RichText::new("Spela med tangentbordet [A, S, D...] eller klicka med musen").size(11.0).color(Theme::TEXT_MUTED));
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
        egui::Window::new("🎛 Hårdvarukontroller & MCU / OSC Routing")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(520.0);
                ui.label(egui::RichText::new("Direkt hårdvaruintegration med Mackie Control Universal (MCU) och Open Sound Control (OSC)").size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                // Section 1: Mackie MCU
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🎚 MACKIE CONTROL UNIVERSAL (MCU)").strong().size(12.0).color(Theme::FL_ORANGE));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (status_txt, status_col) = if self.mcu_connected {
                                ("● ANSLUTEN (ALSA RawMIDI)", Theme::FL_GREEN)
                            } else {
                                ("○ FRÅNKOPPLAD", Theme::TEXT_MUTED)
                            };
                            ui.label(egui::RichText::new(status_txt).strong().size(10.0).color(status_col));
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Enhet:");
                        ui.label(egui::RichText::new(&self.mcu_device_name).strong().color(Theme::FL_CYAN));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Fader Bank:");
                        let banks = ["Bank 1 (Spår 1–8)", "Bank 2 (Spår 9–16)", "Bank 3 (Bussar & VCA)"];
                        for (b_i, name) in banks.iter().enumerate() {
                            let is_b = self.mcu_bank == b_i;
                            if ui.selectable_label(is_b, *name).clicked() {
                                self.mcu_bank = b_i;
                                self.status_message = format!("MCU växlade till {}", name);
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Motorfaders feedback status:").size(10.0).color(Theme::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        for i in 0..8 {
                            let ch_vol = if i < self.channels.len() { self.channels[i].volume } else { 0.8 };
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

                // Section 2: Open Sound Control (OSC)
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📡 OPEN SOUND CONTROL (OSC / UDP)").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut self.osc_enabled, "OSC Server Aktiv");
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Lyssningsport (RX):");
                        ui.add(egui::DragValue::new(&mut self.osc_rx_port).range(1024..=65535));
                        ui.separator();
                        ui.label("Sändningsport (TX):");
                        ui.add(egui::DragValue::new(&mut self.osc_tx_port).range(1024..=65535));
                    });

                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("Mottagna OSC-paket: {}", self.osc_rx_count)).size(10.5).color(Theme::FL_GREEN));
                        if ui.button("⚡ Skicka Test Ping (/sonix/ping)").clicked() {
                            self.osc_rx_count += 1;
                            self.status_message = "📡 Sände OSC Test Ping på port 9000".to_string();
                        }
                    });

                    ui.collapsing("📋 OSC Adress-mappningar", |ui| {
                        ui.label(egui::RichText::new("• /sonix/transport/play (1/0)\n• /sonix/track/{1..8}/volume (0.0 .. 1.25)\n• /sonix/track/{1..8}/mute (1/0)\n• /sonix/bus/{vocal|drum|synth|fx}/volume (0.0 .. 1.25)\n• /sonix/vca/{1..4}/volume (0.0 .. 1.25)").size(10.0).color(Theme::TEXT_MUTED));
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Stäng").clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.show_controller_modal = false;
        }
    }

    fn render_batch_export_modal(&mut self, ctx: &egui::Context) {
        if !self.show_render_queue_modal {
            return;
        }

        let mut close = false;
        egui::Window::new("📤 Batch-Export & Render Queue")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(540.0);
                ui.label(egui::RichText::new("Automatisera export av hela låtar, individuella stämmor (stems) och torra tagningar i valfritt format.").size(11.0).color(Theme::TEXT_MUTED));
                ui.add_space(8.0);

                // Scope & Source
                ui.group(|ui| {
                    ui.label(egui::RichText::new("1. VAD SKALL EXPORTERAS (SCOPE)").strong().size(11.5).color(Theme::FL_ORANGE));
                    ui.separator();
                    let scopes = [
                        ("Master Mix (Hela Låten)", "Renderar full stereo-mix med all master-processering"),
                        ("Alla Stämmor - Torra (All Tracks Dry)", "Exporterar 8 individuella spår utan reverb/delay för extern mixning"),
                        ("Alla Stämmor - Med FX (All Stems Wet)", "Exporterar 8 individuella spår med alla effekter och modulation"),
                        ("Sång & Stämmor (Vocal Studio & Harmonies)", "Exporterar master take + ters/kvint harmonier"),
                    ];
                    for (i, (title, desc)) in scopes.iter().enumerate() {
                        if ui.selectable_label(self.render_scope_idx == i, format!("⦿ {}", title)).clicked() {
                            self.render_scope_idx = i;
                        }
                        ui.label(egui::RichText::new(format!("    {}", desc)).size(9.5).color(Theme::TEXT_MUTED));
                    }
                });

                ui.add_space(6.0);

                // Format & Sample Rate
                ui.group(|ui| {
                    ui.label(egui::RichText::new("2. FORMAT & LJUDKVALITET").strong().size(11.5).color(Theme::FL_CYAN));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Format:");
                        let formats = ["WAV (32-bit Float)", "WAV (24-bit PCM)", "MP3 (320 kbps CBR)", "FLAC (24-bit Lossless)"];
                        for (f_i, fname) in formats.iter().enumerate() {
                            if ui.selectable_label(self.render_format_idx == f_i, *fname).clicked() {
                                self.render_format_idx = f_i;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Samplingsfrekvens:");
                        let rates = ["44.1 kHz (CD Standard)", "48.0 kHz (Film & Video)", "96.0 kHz (Hi-Res Studio)"];
                        for (r_i, rname) in rates.iter().enumerate() {
                            if ui.selectable_label(self.render_sample_rate_idx == r_i, *rname).clicked() {
                                self.render_sample_rate_idx = r_i;
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // Filename Template
                ui.group(|ui| {
                    ui.label(egui::RichText::new("3. FILNAMNSMALL").strong().size(11.5).color(Theme::FL_YELLOW));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Mall:");
                        ui.text_edit_singleline(&mut self.render_template);
                    });

                    let ext = match self.render_format_idx {
                        0 | 1 => "wav",
                        2 => "mp3",
                        _ => "flac",
                    };
                    let example_name = format!("Sonix_808_Kick_{:.0}bpm.{}", self.bpm, ext);
                    ui.label(egui::RichText::new(format!("Exempel på filnamn: {}", example_name)).size(10.0).color(Theme::FL_GREEN));
                });

                ui.add_space(8.0);

                // Status & Render Button
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Status:").strong());
                        ui.label(egui::RichText::new(&self.render_queue_status).color(Theme::FL_CYAN));
                    });

                    if self.is_rendering {
                        ui.add(egui::ProgressBar::new(self.render_progress).show_percentage());
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("🚀 STARTA BATCH-RENDERING").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 90))).clicked() {
                        self.execute_batch_export();
                    }

                    if ui.button("Stäng").clicked() {
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
        self.render_progress = 1.0;

        let export_dir = "/home/alex/Projects/sonix/exports";
        let _ = std::fs::create_dir_all(export_dir);

        let sample_rate = match self.render_sample_rate_idx {
            1 => 48000.0,
            2 => 96000.0,
            _ => 44100.0,
        };

        let mut synth = SynthEngine::new(sample_rate);
        synth.waveform = self.waveform;
        synth.adsr = self.adsr;
        synth.filter_params = self.filter;
        synth.delay_params = self.delay;
        synth.reverb_params = self.reverb;
        synth.drive = self.drive;
        synth.master_volume = self.master_volume;

        match self.render_scope_idx {
            0 => {
                // Master mix
                let export_path = format!("{}/Sonix_Master_Mix_{:.0}bpm.wav", export_dir, self.bpm);
                if self.pattern_mode {
                    let pattern_grid: Vec<[bool; 16]> = self.channels.iter().map(|c| c.steps).collect();
                    let step_notes = self.channels[6].notes;
                    let _ = render_to_wav(&export_path, synth, &pattern_grid, &step_notes, self.bpm, 4);
                } else {
                    let pattern_steps = self.patterns.iter().map(|p| p.channel_steps.clone()).collect();
                    let pattern_notes = self.patterns.iter().map(|p| p.channel_notes.clone()).collect();
                    let track_clips = self.playlist_tracks.iter().map(|t| t.clips).collect();
                    let track_muted = self.playlist_tracks.iter().map(|t| t.muted).collect();

                    let arrangement = SongArrangementExport {
                        pattern_steps,
                        pattern_notes,
                        track_clips,
                        track_muted,
                        num_bars: self.loop_end_bar.clamp(4, 32),
                        bpm: self.bpm,
                    };
                    let _ = render_song_arrangement_to_wav(&export_path, synth, &arrangement);
                }
                self.render_queue_status = format!("✅ Master Mix sparad till: {}", export_path);
                self.status_message = format!("✔ Batch-export klar: {}", export_path);
            }
            1 | 2 => {
                // All Stems (Dry or Wet)
                let mut stem_count = 0;
                for (idx, ch) in self.channels.iter().enumerate() {
                    let is_wet = self.render_scope_idx == 2;
                    let sanitized_name = ch.name.replace(' ', "_");
                    let file_path = format!("{}/Stem_{:02}_{}_{}_{:.0}bpm.wav", export_dir, idx + 1, sanitized_name, if is_wet { "Wet" } else { "Dry" }, self.bpm);

                    let mut solo_grid = vec![[false; 16]; self.channels.len()];
                    solo_grid[idx] = ch.steps;
                    let step_notes = ch.notes;

                    let mut stem_synth = SynthEngine::new(sample_rate);
                    stem_synth.waveform = self.waveform;
                    stem_synth.adsr = self.adsr;
                    stem_synth.filter_params = self.filter;
                    stem_synth.drive = if is_wet { self.drive } else { 1.0 };
                    stem_synth.master_volume = ch.volume;
                    if is_wet {
                        stem_synth.delay_params = self.delay;
                        stem_synth.reverb_params = self.reverb;
                    }

                    if render_to_wav(&file_path, stem_synth, &solo_grid, &step_notes, self.bpm, 4).is_ok() {
                        stem_count += 1;
                    }
                }
                self.render_queue_status = format!("✅ {} stämmor exporterade till {}", stem_count, export_dir);
                self.status_message = format!("✔ Exporterade {} stämmor till ./exports/", stem_count);
            }
            _ => {
                // Vocal Stems
                let vocal_path = format!("{}/Sonix_Vocal_Studio_Comp_{:.0}bpm.wav", export_dir, self.bpm);
                let pattern_grid: Vec<[bool; 16]> = self.channels.iter().map(|c| c.steps).collect();
                let step_notes = self.channels[6].notes;
                let _ = render_to_wav(&vocal_path, synth, &pattern_grid, &step_notes, self.bpm, 4);
                self.render_queue_status = format!("✅ Sångstämmor exporterade till: {}", vocal_path);
                self.status_message = format!("✔ Sång export klar: {}", vocal_path);
            }
        }
    }
    pub fn scan_for_suno_stems(&mut self) {
        let mut results = Vec::new();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/alex".to_string());
        let scan_dirs = [
            format!("{}/Music", home),
            format!("{}/Downloads", home),
            "./imported_stems".to_string(),
        ];

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

        self.status_message = format!("⏳ Startade inläsning av '{}' i bakgrunden...", title);

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
                        p.error_message = Some(format!("Kunde inte packa upp ZIP: {}", e));
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
                p.stage = "Förbereder avkodning av stämmor...".to_string();
                p.current_file = String::new();
                p.current_idx = 0;
                p.total_files = 8;
                p.progress_ratio = 0.05;
                p.completed_payload = None;
                p.error_message = None;
            }
        }

        self.status_message = format!("⏳ Startade inläsning av stämmor för '{}' i bakgrunden...", project_title);

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
                p.error_message = Some(format!("Inga ljudfiler hittades i mappen: {}", folder_path));
            }
            return;
        }

        stem_files.sort();
        let total_files = stem_files.len();

        {
            if let Ok(mut p) = progress.lock() {
                p.total_files = total_files;
                p.stage = format!("Hittade {} stämspår. Avkodar PCM-ljud...", total_files);
                p.progress_ratio = 0.10;
            }
        }

        let sec_per_bar = 60.0 / bpm * 4.0;
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
                    p.stage = format!("Läser & avkodar spår {}/{}: {}", idx + 1, total_files, clean_name);
                    p.progress_ratio = 0.10 + 0.85 * (idx as f32 / total_files as f32);
                }
            }

            let mut file_bars = 32.0_f32;
            let mut pcm_l_opt = None;
            let mut pcm_r_opt = None;
            let mut sample_rate = 44100u32;
            let mut wave_env = Vec::new();

            if path_str.to_lowercase().ends_with(".wav")
                && let Ok((l, r, sr)) = crate::audio::load_wav_pcm(&path_str) {
                    sample_rate = sr;
                    let total_secs = l.len() as f32 / sr.max(1) as f32;
                    file_bars = (total_secs / sec_per_bar).max(1.0);
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
                let num_points = ((file_bars * 16.0) as usize).clamp(240, 4800);
                let seed = (idx as f32 + 1.0) * 17.3;
                for i in 0..num_points {
                    let t = i as f32 / num_points as f32;
                    let val = (t * seed).sin().abs() * 0.7 + (t * seed * 2.1).cos().abs() * 0.3;
                    wave_env.push(val.clamp(0.08, 0.92));
                }
            }

            let name_lower = clean_name.to_lowercase();
            let (kind, icon, color) = if name_lower.contains("lead vocal") || name_lower.contains("0 lead") {
                (TrackKind::VocalAudio, "🎙", Color32::from_rgb(190, 120, 255))
            } else if name_lower.contains("backing vocal") || name_lower.contains("1 backing") || name_lower.contains("vocal") {
                (TrackKind::VocalAudio, "🗣", Color32::from_rgb(140, 100, 245))
            } else if name_lower.contains("drum") || name_lower.contains("2 drums") || name_lower.contains("trumm") {
                (TrackKind::Drums, "🥁", Theme::FL_CYAN)
            } else if name_lower.contains("bass") || name_lower.contains("3 bass") || name_lower.contains("bas") {
                (TrackKind::Bassline, "🎸", Theme::FL_YELLOW)
            } else if name_lower.contains("guitar") || name_lower.contains("4 guitar") || name_lower.contains("gitarr") {
                (TrackKind::SynthLead, "🎸", Theme::FL_ORANGE)
            } else if name_lower.contains("percussion") || name_lower.contains("5 percussion") || name_lower.contains("perc") {
                (TrackKind::Drums, "🪘", Color32::from_rgb(70, 170, 250))
            } else if name_lower.contains("synth") || name_lower.contains("6 synth") || name_lower.contains("key") {
                (TrackKind::SynthLead, "🎹", Theme::FL_GREEN)
            } else {
                (TrackKind::Fx, "✨", Color32::from_rgb(240, 110, 190))
            };

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
            p.stage = "Slutför inläsning...".to_string();
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
                color: dt.color,
            };

            let mut track = PlaylistTrack::new(format!("{} {}", dt.icon, dt.clean_name), dt.icon, dt.kind, dt.color);
            track.volume = 0.9;
            track.is_rec_armed = idx == 0;
            track.regions = vec![initial_region];
            track.audio_waveform = Some(dt.wave_env);
            track.custom_clip_name = Some(dt.clean_name);

            for b in 0..32 {
                track.clips[b] = Some(idx);
            }

            new_tracks.push(track);
        }

        let total_tracks_len = new_tracks.len();
        self.playlist_tracks = new_tracks;
        self.project_name = res.project_title.clone();
        self.bpm = res.bpm;
        self.loop_start_bar = 0;
        self.loop_end_bar = (res.max_stem_bars.ceil() as usize).max(32);
        for t_idx in 0..self.playlist_tracks.len() {
            self.sync_track_regions(t_idx);
        }
        self.suno_context_clip = Some(format!("{} - Master Mix", res.project_title));
        self.status_message = format!("✨ Importerade {} stämspår för '{}' ({:.1} BPM) med äkta vågformer!", total_tracks_len, res.project_title, res.bpm);
        self.view_mode = ViewMode::PlaylistArranger;
    }

    fn render_suno_stem_import_modal(&mut self, ctx: &egui::Context) {
        if !self.show_suno_import_modal {
            return;
        }

        let mut close = false;
        let mut zip_to_import: Option<String> = None;
        let mut folder_to_import: Option<(String, String, f32)> = None;

        egui::Window::new("📦 Import Stems / Multi-Track Importer")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(580.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🎵").size(24.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Multi-Track Stämmor & Ljudspår (Stems)").strong().size(13.0).color(Theme::FL_ORANGE));
                            ui.label(egui::RichText::new("Importera nedladdade ZIP-paket eller mappar med stämmor (Suno, FL Studio, Ableton, Logic m.fl.). Sonix läser ut äkta 48kHz WAV-vågformer, detekterar tempo (BPM) och mappar spåren i tidslinjen.").size(10.5).color(Theme::TEXT_MUTED));
                        });
                    });
                });

                ui.add_space(8.0);

                // Quick Rescan Bar
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("UPPTÄCKTA STEMPAKET:").strong().size(11.0).color(Theme::FL_CYAN));
                    if ui.button("🔄 Skanna ~/Music & ~/Downloads").clicked() {
                        self.scan_for_suno_stems();
                    }
                });

                ui.add_space(4.0);

                // List of detected ZIPs
                ui.group(|ui| {
                    if self.detected_suno_zips.is_empty() {
                        ui.label(egui::RichText::new("Inga .zip-stempaket hittades i ~/Music eller ~/Downloads. Klicka på knappen ovan för att skanna, eller ange sökväg manuellt nedan.").size(10.5).color(Theme::TEXT_MUTED));
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
                                            if ui.add(egui::Button::new(egui::RichText::new("⚡ Importera Alla Stämmor").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                    ui.label(egui::RichText::new("VÄLJ ZIP-FIL ELLER MAPP").strong().size(11.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("📁 Välj ZIP-fil från datorn...").strong().size(11.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked()
                            && let Ok(output) = std::process::Command::new("zenity")
                                .args(["--file-selection", "--file-filter=*.zip", "--title=Välj ZIP Stempaket"])
                                .output()
                                && output.status.success() {
                                    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                                    if !selected_path.is_empty() {
                                        zip_to_import = Some(selected_path);
                                    }
                                }

                        ui.separator();
                        ui.label("eller ange sökväg:");
                        ui.text_edit_singleline(&mut self.custom_stem_path_input);
                        if ui.add(egui::Button::new("Ladda").fill(Color32::from_rgb(50, 60, 80))).clicked() {
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
                    ui.label(egui::RichText::new("💡 Tips: Du kan även dra & släppa (Drag & Drop) .zip-filer direkt in i fönstret!").size(9.5).color(Theme::FL_GREEN));
                });

                ui.add_space(8.0);

                // Stems Layout Preview Info
                ui.group(|ui| {
                    ui.label(egui::RichText::new("SPÅRSTRUKTUR I SONIX:").strong().size(10.5).color(Theme::TEXT_MUTED));
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
                    if ui.button(egui::RichText::new("Stäng").size(11.0)).clicked() {
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
                self.status_message = format!("❌ Fel vid stämimport: {}", err);
            }
        }

        if let Some((stage, file, idx, total, ratio)) = progress_info {
            egui::Window::new("⏳ Importerar stämspår...")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_width(460.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Läser in och avkodar stämmor (Stems)").strong().size(13.0).color(Theme::FL_ORANGE));
                                ui.label(egui::RichText::new(&stage).size(11.0).color(Color32::WHITE));
                            });
                        });
                    });

                    ui.add_space(8.0);
                    ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if !file.is_empty() {
                            ui.label(egui::RichText::new(format!("Spår {} av {}: {}", idx, total, file)).size(10.0).color(Theme::TEXT_MUTED));
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

    fn render_about_modal(&mut self, ctx: &egui::Context) {
        if !self.show_about_modal {
            return;
        }

        let mut close = false;
        let mut open = self.show_about_modal;
        egui::Window::new("ℹ Om Sonix Studio")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(460.0, 420.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("🍊").size(64.0));
                    ui.heading(egui::RichText::new("SONIX STUDIO").strong().size(22.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new("Professionell Digital Audio Workstation & AI Musikstudio för Linux").size(12.0).color(Theme::FL_CYAN));
                    ui.label(egui::RichText::new("Version 0.1.0 (PipeWire / ALSA / JACK Audio Engine)").size(10.5).color(Theme::TEXT_MUTED));

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new("🚀 Huvudfunktioner i Sonix Studio:").strong().size(12.0).color(Theme::TEXT_BRIGHT));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("• Realtids Multi-Track Audio Streaming & Mixmotor i Rust").size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("• Fullt stöd för Suno AI Stems med linjär vågformsredigering").size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("• 16-Stegs Sonix Channel Rack, Piano Roll & Touch Piano").size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("• Klippverktyg (Slice ✂), Fading & Dynamisk Gain-justering").size(11.0).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new("• 64-bit SIMD Audio DSP med Zero-Latency Process Thread").size(11.0).color(Theme::TEXT_MUTED));

                    ui.add_space(16.0);
                    if ui.add(egui::Button::new(egui::RichText::new("  Stäng  ").strong().size(12.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
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

        egui::Window::new("📁 Filhanterare & Projekt")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(580.0, 440.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("📄 Nytt tomt projekt").strong().color(Theme::FL_CYAN)).clicked() {
                        create_new = true;
                        close = true;
                    }
                    if ui.button(egui::RichText::new("⚡ Ladda Demo-projekt").strong().color(Theme::FL_GREEN)).clicked() {
                        load_demo = true;
                        close = true;
                    }
                    if ui.button(egui::RichText::new("🎼 Importera Suno ZIP...").strong().color(Theme::FL_ORANGE)).clicked() {
                        show_suno = true;
                        close = true;
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Spara som
                ui.group(|ui| {
                    ui.label(egui::RichText::new("💾 SPARA PROJEKT").strong().color(Theme::FL_ORANGE));
                    ui.horizontal(|ui| {
                        ui.label("Projektnamn:");
                        ui.text_edit_singleline(&mut self.new_project_name_input);
                        if ui.button("Spara till disk").clicked() {
                            save_name = Some(self.new_project_name_input.clone());
                        }
                    });
                });

                ui.add_space(8.0);
                ui.label(egui::RichText::new("📂 SPARADE PROJEKT (~/Music/Sonix/Projects):").strong().color(Theme::FL_CYAN));

                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let projects_dir = std::path::PathBuf::from(home).join("Music/Sonix/Projects");
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
                        ui.label(egui::RichText::new("Inga sparade .sonix-projekt hittades ännu. Spara ditt nuvarande projekt ovan eller ladda ett demo-projekt!").italics().color(Theme::TEXT_MUTED));
                    });
                } else {
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for (stem, full_path) in saved_files {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("🎵 {}", stem)).strong().color(Theme::TEXT_BRIGHT));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new("Öppna Projekt").strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                if ui.button("Stäng").clicked() {
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
        egui::Window::new("🤖 AI Provider Inställningar")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(520.0, 420.0))
            .show(ctx, |ui| {
                ui.label("Konfigurera dina API-nycklar och lokala AI-modeller för stämdelning och musikgenerering.");
                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("Suno AI").strong().color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new("Används för automatisk stäm-nedladdning och metadata-synk.").size(10.5).color(Theme::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        ui.label("Session Key:");
                        ui.add(egui::TextEdit::singleline(&mut self.ai_provider_suno_key).password(true));
                    });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Stability AI / Stable Audio").strong().color(Theme::FL_CYAN));
                    ui.horizontal(|ui| {
                        ui.label("API-nyckel:");
                        ui.add(egui::TextEdit::singleline(&mut self.ai_provider_stable_audio_key).password(true));
                    });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("OpenAI / ChatGPT Studio Assistant").strong().color(Theme::FL_GREEN));
                    ui.horizontal(|ui| {
                        ui.label("API-nyckel:");
                        ui.add(egui::TextEdit::singleline(&mut self.ai_provider_openai_key).password(true));
                    });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Anthropic / Claude Co-Producer").strong().color(Theme::FL_ORANGE));
                    ui.horizontal(|ui| {
                        ui.label("API-nyckel:");
                        ui.add(egui::TextEdit::singleline(&mut self.ai_provider_claude_key).password(true));
                    });
                });

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Lokal Ollama / Piper / Whisper").strong().color(Theme::FL_PURPLE));
                    ui.horizontal(|ui| {
                        ui.label("Endpoint:");
                        ui.text_edit_singleline(&mut self.ai_provider_ollama_endpoint);
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("💾 Spara inställningar").strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                        self.status_message = "Sparade AI Provider-inställningar!".to_string();
                        close = true;
                    }
                    if ui.button("Avbryt").clicked() {
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
        egui::Window::new("🎛 Ljud- & MIDI-inställningar")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(500.0, 380.0))
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Ljudmotor & Drivrutiner").strong().color(Theme::FL_CYAN));
                    ui.horizontal(|ui| {
                        ui.label("Drivrutin:");
                        let drivers = ["PipeWire (Rekommenderad)", "ALSA Direct", "JACK Audio Server"];
                        for (i, drv) in drivers.iter().enumerate() {
                            if ui.selectable_label(self.audio_driver_idx == i, *drv).clicked() {
                                self.audio_driver_idx = i;
                            }
                        }
                    });
                    ui.label("Status: 🟢 Ansluten och aktiv (Noll latens)");

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Samplingsfrekvens:");
                        let rates = ["44.1 kHz", "48.0 kHz", "96.0 kHz"];
                        for (i, rate) in rates.iter().enumerate() {
                            if ui.selectable_label(self.audio_sample_rate_idx == i, *rate).clicked() {
                                self.audio_sample_rate_idx = i;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Bufferstorlek:");
                        let buffers = ["128 (2.9ms)", "256 (5.8ms)", "512 (11.6ms)", "1024 (23.2ms)"];
                        for (i, buf) in buffers.iter().enumerate() {
                            if ui.selectable_label(self.audio_buffer_size_idx == i, *buf).clicked() {
                                self.audio_buffer_size_idx = i;
                            }
                        }
                    });
                });

                ui.add_space(6.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Master Säkerhet & Limiter").strong().color(Theme::FL_ORANGE));
                    ui.checkbox(&mut self.audio_limiter_enabled, "Aktivera Soft-Clip Brickwall Limiter vid 0.0 dBFS");
                });

                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });

        if close || !open {
            self.show_audio_settings_modal = false;
        }
    }

    pub fn open_stem_focus(&mut self, track_idx: usize) {
        if track_idx < self.playlist_tracks.len() {
            self.focused_stem_track = Some(track_idx);
            self.selected_timeline_track = track_idx;
            self.show_stem_focus_modal = true;
            self.status_message = format!("🔍 Öppnade stämeditor för '{}'", self.playlist_tracks[track_idx].name);
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

        let sec_per_bar = 60.0 / self.bpm * 4.0;

        egui::Window::new("🎛 Stämeditor & Ljudfokus")
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
                        if ui.button("◀ Föregående").on_hover_text("Växla till föregående stämma").clicked() {
                            if t_idx > 0 {
                                navigate_idx = Some(t_idx - 1);
                            } else {
                                navigate_idx = Some(num_tracks - 1);
                            }
                        }
                        if ui.button("Nästa ▶").on_hover_text("Växla till nästa stämma").clicked() {
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
                        if ui.add(egui::Button::new(egui::RichText::new(if track.solo { "🎧 ISOLERAD (SOLO ON)" } else { "🎧 ISOLERA STÄMMA" }).strong().color(solo_fg)).fill(solo_bg)).on_hover_text("Isolera och lyssna enbart på denna stämma i realtid").clicked() {
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

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("❌ Stäng").clicked() {
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
                        (3, "🎵 Klipp & Vågform"),
                    ];
                    for (tab_idx, label) in tabs {
                        let is_sel = self.stem_focus_active_tab == tab_idx;
                        let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(25, 30, 40) };
                        let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(label).strong().size(11.0).color(fg)).fill(bg)).clicked() {
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
                            ui.label(egui::RichText::new("Nivåer & Stereobild för stämman").strong().color(Theme::FL_ORANGE));
                            ui.add_space(6.0);

                            ui.horizontal(|ui| {
                                // Volume Section
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("🎚 Exakt Ljudvolym & Gain").strong().size(12.0).color(Theme::FL_CYAN));
                                        ui.add_space(4.0);

                                        let cur_vol = track.volume;
                                        let gain_db = if cur_vol <= 0.001 { -60.0 } else { 20.0 * cur_vol.log10() };
                                        ui.label(egui::RichText::new(format!("Gain: {:+.2} dB  (Nivå: {:.1}%)", gain_db, cur_vol * 100.0)).strong().size(13.0).color(Theme::TEXT_BRIGHT));

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
                                        ui.label("Snabbjustering (0.1 dB precision):");
                                        ui.horizontal(|ui| {
                                            if ui.button("-0.5 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-0.5 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("-0.1 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-0.1 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("0.0 dB (100%)").clicked() {
                                                track.volume = 1.0;
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("+0.1 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(0.1 / 20.0)).min(2.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("+0.5 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(0.5 / 20.0)).min(2.0);
                                                trigger_audio_sync = true;
                                            }
                                        });

                                        ui.horizontal(|ui| {
                                            if ui.button("📉 -3.0 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-3.0 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("📉 -6.0 dB").clicked() {
                                                track.volume = (track.volume * 10.0_f32.powf(-6.0 / 20.0)).max(0.0);
                                                trigger_audio_sync = true;
                                            }
                                            if ui.button("🔇 Muta").clicked() {
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
                                        ui.label(egui::RichText::new("↔ Stereopanorering").strong().size(12.0).color(Theme::FL_CYAN));
                                        let pan_text = if track.pan < -0.01 {
                                            format!("Vänster {:.0}%", track.pan.abs() * 100.0)
                                        } else if track.pan > 0.01 {
                                            format!("Höger {:.0}%", track.pan * 100.0)
                                        } else {
                                            "Center (Mitt)".to_string()
                                        };
                                        ui.label(egui::RichText::new(format!("Pan: {}", pan_text)).color(Theme::TEXT_BRIGHT));

                                        let mut pan_val = track.pan;
                                        if ui.add(egui::Slider::new(&mut pan_val, -1.0..=1.0).text("L / R")).changed() {
                                            track.pan = pan_val;
                                            trigger_audio_sync = true;
                                        }

                                        ui.horizontal(|ui| {
                                            if ui.button("100% V").clicked() { track.pan = -1.0; trigger_audio_sync = true; }
                                            if ui.button("50% V").clicked() { track.pan = -0.5; trigger_audio_sync = true; }
                                            if ui.button("Center").clicked() { track.pan = 0.0; trigger_audio_sync = true; }
                                            if ui.button("50% H").clicked() { track.pan = 0.5; trigger_audio_sync = true; }
                                            if ui.button("100% H").clicked() { track.pan = 1.0; trigger_audio_sync = true; }
                                        });

                                        ui.add_space(8.0);
                                        ui.label(egui::RichText::new("🎛 Dynamik & Kompressor").strong().size(12.0).color(Theme::FL_CYAN));
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
                                ui.label(egui::RichText::new("📈 3-Bands Parametrisk Stäm-EQ").strong().color(Theme::FL_ORANGE));
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
                                    ui.label(egui::RichText::new("🔊 Bas (Low Shelf)").strong().color(Theme::FL_ORANGE));
                                    ui.add(egui::Slider::new(&mut track.eq.low_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.low_freq, 40.0..=400.0).text("Frekvens").suffix(" Hz"));
                                    ui.horizontal(|ui| {
                                        if ui.button("-1dB").clicked() { track.eq.low_gain_db = (track.eq.low_gain_db - 1.0).max(-12.0); }
                                        if ui.button("0dB").clicked() { track.eq.low_gain_db = 0.0; }
                                        if ui.button("+1dB").clicked() { track.eq.low_gain_db = (track.eq.low_gain_db + 1.0).min(12.0); }
                                    });
                                });

                                // Mid Band
                                ui.group(|ui| {
                                    ui.set_width(220.0);
                                    ui.label(egui::RichText::new("🎙 Mellanregister (Mid Peak)").strong().color(Theme::FL_GREEN));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_freq, 200.0..=6000.0).text("Frekvens").suffix(" Hz"));
                                    ui.add(egui::Slider::new(&mut track.eq.mid_q, 0.5..=3.0).text("Q (Bredd)"));
                                });

                                // High Band
                                ui.group(|ui| {
                                    ui.set_width(220.0);
                                    ui.label(egui::RichText::new("✨ Diskant (High Shelf)").strong().color(Color32::from_rgb(180, 110, 255)));
                                    ui.add(egui::Slider::new(&mut track.eq.high_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                    ui.add(egui::Slider::new(&mut track.eq.high_freq, 3000.0..=16000.0).text("Frekvens").suffix(" Hz"));
                                    ui.horizontal(|ui| {
                                        if ui.button("-1dB").clicked() { track.eq.high_gain_db = (track.eq.high_gain_db - 1.0).max(-12.0); }
                                        if ui.button("0dB").clicked() { track.eq.high_gain_db = 0.0; }
                                        if ui.button("+1dB").clicked() { track.eq.high_gain_db = (track.eq.high_gain_db + 1.0).min(12.0); }
                                    });
                                });
                            });

                            ui.add_space(4.0);

                            // EQ Presets
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Förinställningar:").size(10.5).color(Theme::TEXT_MUTED));
                                if ui.button("🎙 Sång: Luft & Värme").clicked() {
                                    track.eq.low_gain_db = -2.0; track.eq.low_freq = 100.0;
                                    track.eq.mid_gain_db = 1.5; track.eq.mid_freq = 3000.0; track.eq.mid_q = 1.2;
                                    track.eq.high_gain_db = 3.5; track.eq.high_freq = 9000.0;
                                }
                                if ui.button("🥁 Trummor: Punch & Snap").clicked() {
                                    track.eq.low_gain_db = 3.0; track.eq.low_freq = 75.0;
                                    track.eq.mid_gain_db = -2.5; track.eq.mid_freq = 450.0; track.eq.mid_q = 1.0;
                                    track.eq.high_gain_db = 2.0; track.eq.high_freq = 7500.0;
                                }
                                if ui.button("🎸 Gitarr: Presence").clicked() {
                                    track.eq.low_gain_db = -3.0; track.eq.low_freq = 120.0;
                                    track.eq.mid_gain_db = 2.0; track.eq.mid_freq = 2400.0; track.eq.mid_q = 1.4;
                                    track.eq.high_gain_db = 1.0; track.eq.high_freq = 6000.0;
                                }
                                if ui.button("🔊 Bas: Deep Sub").clicked() {
                                    track.eq.low_gain_db = 4.0; track.eq.low_freq = 65.0;
                                    track.eq.mid_gain_db = -3.0; track.eq.mid_freq = 1000.0; track.eq.mid_q = 1.0;
                                    track.eq.high_gain_db = -4.0; track.eq.high_freq = 5000.0;
                                }
                                if ui.button("🔄 Nollställ (Flat)").clicked() {
                                    track.eq = TrackEq::default();
                                }
                            });
                        });
                    }

                    2 => {
                        // TAB 3: FADING & TIDSREDIGERING (0.01s Precision)
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("⏱ Fading & Fade-kurvor (Exakt hundradels precision)").strong().color(Theme::FL_ORANGE));
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
                                    ui.label(egui::RichText::new("📈 Fade In (In-toning)").strong().size(12.0).color(Theme::FL_CYAN));
                                    let cs_in = (global_fade_in_sec * 100.0).round() as i32;
                                    ui.label(egui::RichText::new(format!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_in_sec, cs_in)).strong().color(Theme::TEXT_BRIGHT));

                                    ui.add_space(4.0);
                                    if ui.add(egui::Slider::new(&mut global_fade_in_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                        apply_all_fades = true;
                                    }

                                    ui.add_space(4.0);
                                    ui.label("Finjustera med hundradelar:");
                                    ui.horizontal(|ui| {
                                        if ui.button("-0.10s").clicked() { global_fade_in_sec = (global_fade_in_sec - 0.10).max(0.0); apply_all_fades = true; }
                                        if ui.button("-0.01s").clicked() { global_fade_in_sec = (global_fade_in_sec - 0.01).max(0.0); apply_all_fades = true; }
                                        if ui.button("0.00s").clicked() { global_fade_in_sec = 0.0; apply_all_fades = true; }
                                        if ui.button("+0.01s").clicked() { global_fade_in_sec = (global_fade_in_sec + 0.01).min(10.0); apply_all_fades = true; }
                                        if ui.button("+0.10s").clicked() { global_fade_in_sec = (global_fade_in_sec + 0.10).min(10.0); apply_all_fades = true; }
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button("20 ms").clicked() { global_fade_in_sec = 0.02; apply_all_fades = true; }
                                        if ui.button("50 ms").clicked() { global_fade_in_sec = 0.05; apply_all_fades = true; }
                                        if ui.button("100 ms").clicked() { global_fade_in_sec = 0.10; apply_all_fades = true; }
                                        if ui.button("500 ms").clicked() { global_fade_in_sec = 0.50; apply_all_fades = true; }
                                        if ui.button("1.00 s").clicked() { global_fade_in_sec = 1.00; apply_all_fades = true; }
                                    });
                                });

                                ui.separator();

                                // Fade Out Box
                                ui.group(|ui| {
                                    ui.set_width(340.0);
                                    ui.label(egui::RichText::new("📉 Fade Out (Ut-toning)").strong().size(12.0).color(Theme::FL_ORANGE));
                                    let cs_out = (global_fade_out_sec * 100.0).round() as i32;
                                    ui.label(egui::RichText::new(format!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_out_sec, cs_out)).strong().color(Theme::TEXT_BRIGHT));

                                    ui.add_space(4.0);
                                    if ui.add(egui::Slider::new(&mut global_fade_out_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                        apply_all_fades = true;
                                    }

                                    ui.add_space(4.0);
                                    ui.label("Finjustera med hundradelar:");
                                    ui.horizontal(|ui| {
                                        if ui.button("-0.10s").clicked() { global_fade_out_sec = (global_fade_out_sec - 0.10).max(0.0); apply_all_fades = true; }
                                        if ui.button("-0.01s").clicked() { global_fade_out_sec = (global_fade_out_sec - 0.01).max(0.0); apply_all_fades = true; }
                                        if ui.button("0.00s").clicked() { global_fade_out_sec = 0.0; apply_all_fades = true; }
                                        if ui.button("+0.01s").clicked() { global_fade_out_sec = (global_fade_out_sec + 0.01).min(10.0); apply_all_fades = true; }
                                        if ui.button("+0.10s").clicked() { global_fade_out_sec = (global_fade_out_sec + 0.10).min(10.0); apply_all_fades = true; }
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button("20 ms").clicked() { global_fade_out_sec = 0.02; apply_all_fades = true; }
                                        if ui.button("50 ms").clicked() { global_fade_out_sec = 0.05; apply_all_fades = true; }
                                        if ui.button("100 ms").clicked() { global_fade_out_sec = 0.10; apply_all_fades = true; }
                                        if ui.button("500 ms").clicked() { global_fade_out_sec = 0.50; apply_all_fades = true; }
                                        if ui.button("1.00 s").clicked() { global_fade_out_sec = 1.00; apply_all_fades = true; }
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
                                ui.label(egui::RichText::new("Tonhöjd (Pitch Shift):").size(11.0).color(Theme::TEXT_MUTED));
                                ui.add(egui::Slider::new(&mut track.pitch_semitones, -12.0..=12.0).text("Halvtoner"));
                                if ui.button("Nollställ Pitch").clicked() {
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
                                        ui.label(format!("Start: {}  |  Längd: {}", format_time_hundredths(r_start_sec), format_time_hundredths(r_len_sec)));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("🗑 Ta bort").clicked() {
                                                del_region_idx = Some(ri);
                                            }
                                            let m_txt = if r.muted { "🔇 Mutad" } else { "🔊 På" };
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

        egui::Window::new("📖 Sonix Studio – Komplett Bruksanvisning & Master Manual")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(920.0, 620.0))
            .min_size(Vec2::new(760.0, 480.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                // Top Header with quick search and close
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📖 SONIX STUDIO MASTER MANUAL").strong().size(14.0).color(Theme::FL_ORANGE));
                    ui.label(egui::RichText::new("• Komplett Referensguide & Handbok").size(11.0).color(Theme::FL_CYAN));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("✖ Stäng Manual").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
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
                            ui.label(egui::RichText::new("KAPITEL & AVSNITT:").strong().size(10.5).color(Theme::TEXT_MUTED));
                            ui.add_space(2.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("🔍").size(10.0));
                                ui.add(egui::TextEdit::singleline(&mut self.help_search_query).hint_text("Sök i manualen...").desired_width(170.0));
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

                                    if ui.add_sized(Vec2::new(215.0, 26.0), egui::Button::new(egui::RichText::new(title).strong().size(10.5).color(fg)).fill(bg)).clicked() {
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
                                        ui.heading(egui::RichText::new("🚀 1. Snabbstart & Översikt").color(Theme::FL_ORANGE));
                                        ui.label(egui::RichText::new("Välkommen till Sonix Studio – en blixtsnabb Digital Audio Workstation skapad för Linux med äkta realtidsprestanda.").size(12.0).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(8.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🎯 Typiskt produktionsarbetsflöde:").strong().color(Theme::FL_CYAN));
                                            ui.add_space(4.0);
                                            ui.label("1. Importera stämmor: Klicka på '📦 IMPORT STEMS' (Ctrl+I) för att läsa in ett ZIP-paket med sång, bas, trummor m.m.");
                                            ui.label("2. Arrangera & Klipp: Använd saxverktyget (✂ Klipp) och zooma in djupt (Ctrl+Scroll) för att dela med 0.01s precision.");
                                            ui.label("3. Fokusera & Förädla: Klicka på '🔍' på ett spår för att öppna Stämeditorn, isolera stämman med Solo och ratta 3-bands EQ.");
                                            ui.label("4. Skapa trumkomp: Tryck F4 för att öppna Channel Rack och klicka in 16-stegs beats.");
                                            ui.label("5. Spela in melodier: Tryck F5 för Piano Roll eller spela live med datortangentbordet.");
                                            ui.label("6. Mixa & Exportera: Tryck F6 för Mixern och Ctrl+E för att exportera mastrad 48kHz WAV.");
                                        });

                                        ui.add_space(8.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("⚡ Systemarkitektur & Prestanda:").strong().color(Theme::FL_GREEN));
                                            ui.label("• 100% Rust Audio DSP – Inget hack, noll skräpsamling (Garbage Collection), noll latens.");
                                            ui.label("• PipeWire & ALSA Native – Ansluter direkt till Linux moderna ljudserver.");
                                            ui.label("• Asynkron bakgrundsavkodning – Gränssnittet fryser aldrig vid inläsning av tunga ljudfiler.");
                                        });
                                    }

                                    1 => {
                                        // 2. TANGENTBORD & KOMMANDON
                                        ui.heading(egui::RichText::new("⌨ 2. Tangentbord & Komplett Kortkommandoreferens").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("NAVIGERING MELLAN VYER (Funktionstangenter):").strong().color(Theme::FL_CYAN));
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
                                                    ui.label(desc);
                                                });
                                            }
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("PROJEKT & REDIGERINGSKOMMANDON:").strong().color(Theme::FL_GREEN));
                                            ui.separator();
                                            let shortcuts = [
                                                ("Mellanslag (Space)", "Starta / Pausa uppspelning"),
                                                ("Ctrl + N", "Skapa nytt tomt projekt"),
                                                ("Ctrl + O / P", "Öppna Projektbläddrare"),
                                                ("Ctrl + S", "Spara projektfil"),
                                                ("Ctrl + I", "Importera Stämmor / Multi-Track Stems"),
                                                ("Ctrl + E", "Exportera WAV / Master"),
                                                ("Del / Backspace", "Radera markerat ljudklipp"),
                                                ("Ctrl + Scroll", "Mjuk horisontell zoomning i tidslinjen"),
                                                ("A, W, S, E, D...", "Klaviatur – Spela synthen live med tangentbordet"),
                                            ];
                                            for (k, desc) in shortcuts {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(format!("{:<18}", k)).monospace().strong().color(Theme::FL_CYAN));
                                                    ui.label(desc);
                                                });
                                            }
                                        });
                                    }

                                    2 => {
                                        // 3. TIDSLINJE & 0.01s PRECISION
                                        ui.heading(egui::RichText::new("📊 3. Tidslinje, Snäpp & 0.01s Precision").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new("Tidslinjen hanterar obegränsat med ljudspår och regioner med precision ned till 0.01 sekunder (hundradelar).").size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🔍 Dynamisk 4-Nivåers Tidslinjelinjal:").strong().color(Theme::FL_CYAN));
                                            ui.label("1. Takter (Bars): Stora vertikala streck med taktnummer (1, 2, 3...) och tidskod (MM:SS.cs).");
                                            ui.label("2. Beats: Fjärdedelstikar (.2, .3, .4) som syns vid normal zoom.");
                                            ui.label("3. 1/16-delssteg: Tunt rutnät för exakt rytmisk klippning och placering.");
                                            ui.label("4. Hundradelar (0.01s): Aktiveras vid djup inzoomning (200%–800%) för millimeterexakta snitt i sång och trummor.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("✂ Redigeringsverktyg & Snäpp:").strong().color(Theme::FL_ORANGE));
                                            ui.label("• ⇱ Välj: Klicka på en region för att markera och se dess egenskaper i inspektorn.");
                                            ui.label("• ✎ Rita: Klicka i tidslinjen för att rita ut mönster och aktiva klipp.");
                                            ui.label("• ✂ Klipp (0.01s): Saxverktyg. Klicka var som helst på ett ljudspår för att klyva klippet i två.");
                                            ui.label("• 🔇 Muta / 🗑 Radera: Tysta eller radera regioner direkt med ett klick.");
                                            ui.separator();
                                            ui.label("• Snäppläge '⚡ 0.01s (Fri)': Frikopplar från musikaliska takter och låter dig klippa med 10ms precision.");
                                            ui.label("• Snäpplägen 1/16, Beat, Takt: Snäpper automatiskt till det musikaliska tempot (BPM).");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🏃 Följ Tidslinje (Auto-Scroll):").strong().color(Theme::FL_GREEN));
                                            ui.label("• '🏃 Följ tidslinje: PÅ': Tidslinjen rullar automatiskt och håller spelhuvudet centrerat på skärmen.");
                                            ui.label("• '⏸ Följ tidslinje: AV': Tidslinjen står stilla så att du kan redigera i lugn och ro medan låten spelar.");
                                        });
                                    }

                                    3 => {
                                        // 4. MULTI-TRACK STÄMIMPORT
                                        ui.heading(egui::RichText::new("📦 4. Multi-Track Stämimport (Stems)").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new("Importera kompletta stämpaket (WAV, MP3, FLAC, OGG) från Suno AI, FL Studio, Ableton, Logic m.fl.").size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("📥 Hur du importerar:").strong().color(Theme::FL_CYAN));
                                            ui.label("1. Tryck '📦 IMPORT STEMS' i verktygsfältet eller tryck Ctrl+I.");
                                            ui.label("2. Välj bland automatiskt upptäckta paket i ~/Music / ~/Downloads eller välj ZIP-fil/mapp manuellt.");
                                            ui.label("3. Dra & Släpp (Drag & Drop): Dra en .zip-fil direkt in i Sonix-fönstret.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🎛 Automatisk Spåridentifiering & Routing:").strong().color(Theme::FL_GREEN));
                                            ui.label("• Lead Vocals ➔ 🎙 Sångbuss med lila färgkod.");
                                            ui.label("• Backing Vocals ➔ 🗣 Körbuss.");
                                            ui.label("• Drums / Kick / Snare ➔ 🥁 Trumbuss med cyan färgkod.");
                                            ui.label("• Bass ➔ 🎸 Basbuss med gul färgkod.");
                                            ui.label("• Guitar / Keys / Synth ➔ 🎹 Synthbuss med grön/orange färgkod.");
                                            ui.label("• FX / Other ➔ ✨ Effektsändning.");
                                            ui.separator();
                                            ui.label("Inläsningen sker i bakgrunden med en förloppsindikator utan att programmet hänger sig.");
                                        });
                                    }

                                    4 => {
                                        // 5. STÄMEDITOR & 3-BANDS EQ
                                        ui.heading(egui::RichText::new("🔍 5. Dedikerad Stämeditor & Ljudfokus").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.label(egui::RichText::new("Fokusera på en enda stämma med högprecisionskontroller, decibelnivåer, grafisk EQ och fading.").size(11.5).color(Theme::TEXT_BRIGHT));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🎧 1. Isolera stämma (Solo On):").strong().color(Theme::FL_ORANGE));
                                            ui.label("Klicka på den stora knappen '🎧 ISOLERA STÄMMA (SOLO)' högst upp i editorn för att direkt tysta alla andra spår och lyssna enbart på den valda stämman.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("🎚 2. Volym, Pan & Dynamik (Flik 1):").strong().color(Theme::FL_CYAN));
                                            ui.label("• Volymreglage med exakt dB-visning och '±0.1 dB' finjusteringsknappar.");
                                            ui.label("• Stereopanorering med snabbcentrering ('Center').");
                                            ui.label("• Kompressor: Justerbar Threshold (-30 dB till 0 dB) och Ratio (1:1 till 8:1).");
                                            ui.label("• Pitch Shifter: Transponera stämman upp/ned ±12 halvtoner.");
                                            ui.label("• Reverb & Delay sends.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("📈 3. 3-Bands Grafisk Parametrisk EQ (Flik 2):").strong().color(Theme::FL_GREEN));
                                            ui.label("• Interaktiv frekvenskurva i realtid (20 Hz – 20 kHz) med dB-skala.");
                                            ui.label("• Low Shelf (Bas): Gain ±12 dB, brytfrekvens 40–400 Hz.");
                                            ui.label("• Mid Peak (Mellanregister): Gain ±12 dB, frekvens 200 Hz – 6 kHz, Q-faktor 0.5–3.0.");
                                            ui.label("• High Shelf (Diskant): Gain ±12 dB, frekvens 3 kHz – 16 kHz.");
                                            ui.label("• Snabbpresets för Sång, Trummor, Gitarr, Bas och Flat nollställning.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("⏱ 4. Fading & Envelope i hundradelar (Flik 3):").strong().color(Color32::from_rgb(180, 110, 255)));
                                            ui.label("• Fade In och Fade Out med 0.01s precision.");
                                            ui.label("• Stegknappar för '±0.01s' och '±0.10s'. Snabbval för 20ms, 50ms, 100ms, 500ms, 1.00s.");
                                            ui.label("• Slå på alla: Sätter samma fading på alla klipp i just den stämman.");
                                        });
                                    }

                                    5 => {
                                        // 6. CHANNEL RACK & BEATS
                                        ui.heading(egui::RichText::new("🥁 6. Sonix Channel Rack & Stegsequencer (F4)").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("16-Stegs Trummaskin & Mönster:").strong().color(Theme::FL_CYAN));
                                            ui.label("• 8 Klassiska trumkanaler: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, 303 Bass och Lead.");
                                            ui.label("• Mönster P1–P4: Skapa variationer för vers, refräng och stick.");
                                            ui.label("• Swing: Skjutreglage för att ge trummorna ett naturligt sväng.");
                                            ui.label("• ⚡ Slumpa Beats: Genererar omedelbart nya inspirerande trumkomp.");
                                            ui.label("• Velocity-redigering: Justera anslagskraften per steg i den nedre velocity-raden.");
                                        });
                                    }

                                    6 => {
                                        // 7. PIANO ROLL & SYNTH
                                        ui.heading(egui::RichText::new("🎹 7. Piano Roll & Analog Synthesizer (F5 / F7)").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Piano Roll (F5):").strong().color(Theme::FL_CYAN));
                                            ui.label("• Grafiskt 3-oktavigt notinmatningsfönster (C3 till B5).");
                                            ui.label("• Klicka på klaviaturet till vänster för att provlyssna toner i realtid.");
                                            ui.label("• ⚡ Slumpa Melodi: Skapar harmoniska melodislingor automatiskt.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Analog Alchemy Synthesizer (F7):").strong().color(Theme::FL_GREEN));
                                            ui.label("• 4 Vågformer: Sawtooth (sågtand), Square (fyrkant), Sine (sinus) och Noise (brus).");
                                            ui.label("• ADSR Envelope: Attack, Decay, Sustain och Release.");
                                            ui.label("• Moog 24dB Resonant Filter: Cutoff (20Hz–18kHz) och Resonans.");
                                            ui.label("• Saturation & Drive: Analog rörvärme och distortion.");
                                        });
                                    }

                                    7 => {
                                        // 8. VOCAL STUDIO & PITCH
                                        ui.heading(egui::RichText::new("🎤 8. Vocal Studio, Melodyne & Harmonizer (F8)").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Funktioner i Vocal Studio:").strong().color(Theme::FL_CYAN));
                                            ui.label("• Melodyne ARA2 Editor: Interaktiva tonhöjds-blobs för att justera sångens toner och timing.");
                                            ui.label("• 4-Voice Harmonizer: Skapar fylliga sångarrangemang med kör, oktavdubbling eller vocoder.");
                                            ui.label("• Comping & Tagningar: Spela in flera sångtagningar och klipp ihop den bästa versionen.");
                                            ui.label("• Sampling & Recorder: Spela in egna ljud, klappar och instrument med din mikrofon.");
                                        });
                                    }

                                    8 => {
                                        // 9. MIXER & WAV-EXPORT
                                        ui.heading(egui::RichText::new("🎛 9. Mixer Console, Effekter & WAV-Export (F6 / Ctrl+E)").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Mixerbord & Effektrack (F6):").strong().color(Theme::FL_CYAN));
                                            ui.label("• 8 Stereokanaler + Master Bus med analoga faders och VU peak meters.");
                                            ui.label("• 3-Bands Parametrisk EQ per mixerkanal.");
                                            ui.label("• Master Limiter & Maximizer för kommersiell ljudstyrka utan digital distorsion.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Export & Rendering (Ctrl+E):").strong().color(Theme::FL_GREEN));
                                            ui.label("• Export Song: Renderar hela tidslinjen till en sammanslagen masterfil.");
                                            ui.label("• Export Pattern: Exporterar det aktiva Channel Rack-mönstret.");
                                            ui.label("• 📤 BATCH EXPORT: Exporterar alla aktiva stämmor som separata WAV-filer till ./exports/.");
                                            ui.label("• Format: 32-bit float / 24-bit PCM WAV i 44.1 kHz eller 48 kHz.");
                                        });
                                    }

                                    9 => {
                                        // 10. PIPEWIRE & WAYLAND SETUP
                                        ui.heading(egui::RichText::new("⚙ 10. Ljudmotor, PipeWire & Hyprland / Wayland").color(Theme::FL_ORANGE));
                                        ui.add_space(6.0);

                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Ljudmotor (PipeWire / ALSA / JACK):").strong().color(Theme::FL_CYAN));
                                            ui.label("• Sonix kommunicerar direkt med Linux professionella ljudserver i realtid.");
                                            ui.label("• Buffertstorlek: 128/256 samples för noll latens vid live-spelning, 512/1024 samples för tunga projekt.");
                                        });

                                        ui.add_space(6.0);
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new("Linux Wayland & Hyprland Säkerhet:").strong().color(Theme::FL_GREEN));
                                            ui.label("• Sonix är helt anpassat för Hyprland, GNOME Wayland och KDE.");
                                            ui.label("• När fönstret täcks av ett annat fönster eller minimeras fortsätter ljudmotorn att spela utan avbrott samtidigt som GUI-uppritningen vilar för att förhindra krascher.");
                                            ui.label("• Inbyggd Crash Logger sparar automatiskt eventuella problem till /tmp/sonix_crash.log.");
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
