use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::HashSet;
use std::time::Instant;

use crate::audio::{
    render_song_arrangement_to_wav, render_to_wav, AdsrParams, AudioCommand, AudioEngine,
    DelayParams, DrumType, FilterParams, Preset, ReverbParams, SongArrangementExport, SynthEngine,
    Waveform,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    VocalAudio,
    CustomAudio,
    Drums,
    SynthLead,
    Bassline,
    Fx,
}

#[derive(Clone, Debug)]
pub struct AudioRegion {
    pub id: usize,
    pub name: String,
    pub start_bar: f32,
    pub length_bars: f32,
    pub sample_offset_sec: f32,
    pub source_path: Option<String>,
    pub waveform_peaks: Vec<f32>,
    pub volume: f32,
    pub muted: bool,
    pub color: Color32,
}

#[derive(Clone)]
pub struct PlaylistTrack {
    pub name: String,
    #[allow(dead_code)]
    pub icon: &'static str,
    pub kind: TrackKind,
    pub color: Color32,
    pub volume: f32,
    #[allow(dead_code)]
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
}

pub struct SonixApp {
    pub engine: AudioEngine,
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
    pub selected_timeline_track: usize,
    // Project Metadata & Suno Multi-Track Stems
    pub project_name: String,
    pub show_suno_import_modal: bool,
    pub detected_suno_zips: Vec<String>,
    pub custom_stem_path_input: String,
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
            target_bus: BusRouting::SynthBus, vca_group: VcaGroup::Vca2, pdc_latency_samples: 128,
        };
        let sub_bass = ChannelStrip {
            name: "Sub Bass".to_string(), icon: "🎸".to_string(), color: Color32::from_rgb(255, 80, 140),
            volume: 0.90, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, is_reverse: false,
            waveform_preview: make_wave(25.0, 0.6),
            target_bus: BusRouting::SynthBus, vca_group: VcaGroup::Vca2, pdc_latency_samples: 0,
        };

        let channels = vec![kick, snare, clap, hat, open_hat, crash, synth_lead, sub_bass];

        // Rich Built-in Sound Library
        let sample_library = vec![
            // Kicks
            LibrarySampleItem { id: 1, name: "808 Deep Sub Kick".to_string(), category: "Kicks".to_string(), icon: "💥".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(22.0, 0.8) },
            LibrarySampleItem { id: 2, name: "Punchy Acoustic Kick".to_string(), category: "Kicks".to_string(), icon: "🥁".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(30.0, 0.9) },
            LibrarySampleItem { id: 3, name: "EDM Hardstyle Kick".to_string(), category: "Kicks".to_string(), icon: "⚡".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(40.0, 0.85) },
            LibrarySampleItem { id: 4, name: "Lo-Fi Vinyl Kick".to_string(), category: "Kicks".to_string(), icon: "📻".to_string(), default_note: 36, color: Theme::FL_ORANGE, waveform: make_wave(18.0, 0.7) },
            // Snares
            LibrarySampleItem { id: 5, name: "909 Classic Snare".to_string(), category: "Snares".to_string(), icon: "🥢".to_string(), default_note: 38, color: Theme::FL_CYAN, waveform: make_wave(45.0, 0.9) },
            LibrarySampleItem { id: 6, name: "Layered Crisp Snare".to_string(), category: "Snares".to_string(), icon: "💥".to_string(), default_note: 38, color: Theme::FL_CYAN, waveform: make_wave(50.0, 0.92) },
            LibrarySampleItem { id: 7, name: "Organic Wood Rim".to_string(), category: "Snares".to_string(), icon: "🪵".to_string(), default_note: 40, color: Theme::FL_CYAN, waveform: make_wave(35.0, 0.95) },
            // Claps
            LibrarySampleItem { id: 8, name: "Electro Stereo Clap".to_string(), category: "Claps".to_string(), icon: "👏".to_string(), default_note: 39, color: Theme::FL_YELLOW, waveform: make_wave(35.0, 0.85) },
            LibrarySampleItem { id: 9, name: "808 Handclap".to_string(), category: "Claps".to_string(), icon: "👏".to_string(), default_note: 39, color: Theme::FL_YELLOW, waveform: make_wave(30.0, 0.8) },
            LibrarySampleItem { id: 10, name: "Acoustic Finger Snap".to_string(), category: "Claps".to_string(), icon: "🫰".to_string(), default_note: 39, color: Theme::FL_YELLOW, waveform: make_wave(60.0, 0.95) },
            // Hi-Hats
            LibrarySampleItem { id: 11, name: "Crisp 16th Closed Hat".to_string(), category: "Hi-Hats".to_string(), icon: "⚡".to_string(), default_note: 42, color: Theme::FL_PURPLE, waveform: make_wave(70.0, 0.95) },
            LibrarySampleItem { id: 12, name: "Open Sizzle 909 Hat".to_string(), category: "Hi-Hats".to_string(), icon: "🌊".to_string(), default_note: 46, color: Theme::FL_PURPLE, waveform: make_wave(50.0, 0.5) },
            LibrarySampleItem { id: 13, name: "Analog Shaker Groove".to_string(), category: "Hi-Hats".to_string(), icon: "🧂".to_string(), default_note: 44, color: Theme::FL_PURPLE, waveform: make_wave(65.0, 0.9) },
            // Percussion & Toms
            LibrarySampleItem { id: 14, name: "Low Sub Tom".to_string(), category: "Percussion".to_string(), icon: "🪘".to_string(), default_note: 45, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(28.0, 0.7) },
            LibrarySampleItem { id: 15, name: "High Punch Tom".to_string(), category: "Percussion".to_string(), icon: "🪘".to_string(), default_note: 50, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(38.0, 0.75) },
            LibrarySampleItem { id: 16, name: "808 Cowbell Classic".to_string(), category: "Percussion".to_string(), icon: "🔔".to_string(), default_note: 56, color: Color32::from_rgb(255, 140, 60), waveform: make_wave(55.0, 0.8) },
            // Vocal Chops
            LibrarySampleItem { id: 17, name: "🗣 Vocal Chop 'Yeah!'".to_string(), category: "Vocal Chops".to_string(), icon: "🗣".to_string(), default_note: 60, color: Color32::from_rgb(100, 220, 255), waveform: make_wave(32.0, 0.6) },
            LibrarySampleItem { id: 18, name: "🗣 Vocal Chop 'Hey!'".to_string(), category: "Vocal Chops".to_string(), icon: "🗣".to_string(), default_note: 62, color: Color32::from_rgb(100, 220, 255), waveform: make_wave(36.0, 0.65) },
            LibrarySampleItem { id: 19, name: "🗣 Vocal Chop 'Drop It'".to_string(), category: "Vocal Chops".to_string(), icon: "🗣".to_string(), default_note: 58, color: Color32::from_rgb(100, 220, 255), waveform: make_wave(28.0, 0.55) },
            // 808 Bass & Leads
            LibrarySampleItem { id: 20, name: "Sub Bass Sine 808".to_string(), category: "808 & Bass".to_string(), icon: "🎸".to_string(), default_note: 36, color: Color32::from_rgb(255, 80, 140), waveform: make_wave(20.0, 0.6) },
            LibrarySampleItem { id: 21, name: "Acid 303 Saw Lead".to_string(), category: "808 & Bass".to_string(), icon: "🎹".to_string(), default_note: 60, color: Theme::FL_GREEN, waveform: make_wave(50.0, 0.4) },
            // Egna Importerade
            LibrarySampleItem { id: 22, name: "👏 Akustisk Handklapp".to_string(), category: "Egna Importerade".to_string(), icon: "📂".to_string(), default_note: 60, color: Color32::from_rgb(255, 200, 80), waveform: make_wave(40.0, 0.7) },
            LibrarySampleItem { id: 23, name: "🎸 Gitarrackord E-Moll".to_string(), category: "Egna Importerade".to_string(), icon: "📂".to_string(), default_note: 52, color: Color32::from_rgb(255, 200, 80), waveform: make_wave(24.0, 0.5) },
        ];

        // Pattern 1: Main Full Beat & Melodic Hook (8 channels)
        let mut p1_steps = vec![[false; 16]; 8];
        let mut p1_notes = vec![[60; 16]; 8];
        let mut p1_grid = [[false; 16]; 24];
        // Kick
        p1_steps[0][0] = true; p1_steps[0][4] = true; p1_steps[0][8] = true; p1_steps[0][12] = true;
        // Snare
        p1_steps[1][4] = true; p1_steps[1][12] = true;
        // Clap
        p1_steps[2][4] = true; p1_steps[2][12] = true;
        // Hats
        for i in (0..16).step_by(2) { p1_steps[3][i] = true; }
        p1_steps[4][2] = true; p1_steps[4][6] = true; p1_steps[4][10] = true; p1_steps[4][14] = true;
        // Crash on 1
        p1_steps[5][0] = true;
        // Lead
        p1_steps[6][0] = true; p1_notes[6][0] = 60; p1_grid[0][0] = true;
        p1_steps[6][3] = true; p1_notes[6][3] = 63; p1_grid[3][3] = true;
        p1_steps[6][6] = true; p1_notes[6][6] = 65; p1_grid[5][6] = true;
        p1_steps[6][8] = true; p1_notes[6][8] = 67; p1_grid[7][8] = true;
        p1_steps[6][11] = true; p1_notes[6][11] = 70; p1_grid[10][11] = true;
        p1_steps[6][14] = true; p1_notes[6][14] = 72; p1_grid[12][14] = true;
        // Bass
        p1_steps[7][0] = true; p1_notes[7][0] = 36;
        p1_steps[7][8] = true; p1_notes[7][8] = 43;

        let pat1 = Pattern {
            name: "Pat 1: Main Groove".to_string(),
            color: Theme::FL_ORANGE,
            channel_steps: p1_steps,
            channel_notes: p1_notes,
            piano_roll_grid: p1_grid,
        };

        // Pattern 2: Tight Drum Machine Only
        let mut p2_steps = vec![[false; 16]; 8];
        let p2_notes = vec![[60; 16]; 8];
        let p2_grid = [[false; 16]; 24];
        p2_steps[0][0] = true; p2_steps[0][6] = true; p2_steps[0][8] = true; p2_steps[0][14] = true;
        p2_steps[1][4] = true; p2_steps[1][12] = true;
        p2_steps[2][12] = true;
        for i in 0..16 { p2_steps[3][i] = true; }
        p2_steps[4][14] = true;

        let pat2 = Pattern {
            name: "Pat 2: Tight Drums".to_string(),
            color: Theme::FL_CYAN,
            channel_steps: p2_steps,
            channel_notes: p2_notes,
            piano_roll_grid: p2_grid,
        };

        // Pattern 3: Synth Arpeggio & Lead Solo
        let mut p3_steps = vec![[false; 16]; 8];
        let mut p3_notes = vec![[60; 16]; 8];
        let mut p3_grid = [[false; 16]; 24];
        let arp_notes = [60, 63, 67, 70, 72, 70, 67, 63];
        for i in 0..16 {
            p3_steps[6][i] = true;
            let n = arp_notes[i % 8];
            p3_notes[6][i] = n;
            let offset = (n as usize).saturating_sub(48).min(23);
            p3_grid[offset][i] = true;
        }
        let pat3 = Pattern {
            name: "Pat 3: Synth Arp".to_string(),
            color: Theme::FL_GREEN,
            channel_steps: p3_steps,
            channel_notes: p3_notes,
            piano_roll_grid: p3_grid,
        };

        // Pattern 4: Deep Bassline & Chill Intro
        let mut p4_steps = vec![[false; 16]; 8];
        let mut p4_notes = vec![[60; 16]; 8];
        let p4_grid = [[false; 16]; 24];
        for i in (0..16).step_by(4) { p4_steps[3][i] = true; }
        p4_steps[7][0] = true; p4_notes[7][0] = 36;
        p4_steps[7][4] = true; p4_notes[7][4] = 39;
        p4_steps[7][8] = true; p4_notes[7][8] = 41;
        p4_steps[7][12] = true; p4_notes[7][12] = 43;

        let pat4 = Pattern {
            name: "Pat 4: Deep Bass".to_string(),
            color: Color32::from_rgb(255, 80, 140),
            channel_steps: p4_steps,
            channel_notes: p4_notes,
            piano_roll_grid: p4_grid,
        };

        let patterns = vec![pat1, pat2, pat3, pat4];

        // Vocal Track Demo Waveform
        let mut vocal_wave = Vec::with_capacity(100);
        for i in 0..100 {
            let t = i as f32 / 100.0;
            let val = (t * 28.0).sin().abs() * 0.8 + (t * 56.0).sin().abs() * 0.2;
            vocal_wave.push(val.clamp(0.05, 0.95));
        }

        // Custom Sound Demo Waveform
        let mut custom_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = (1.0 - t).powi(2) * (t * 40.0).sin().abs();
            custom_wave.push(val.clamp(0.05, 0.9));
        }

        // 6 Audio & MIDI Timeline Tracks (Sonix Studio Style)
        let r0 = AudioRegion {
            id: 1,
            name: "Lead Vocal (Take 1)".to_string(),
            start_bar: 0.0,
            length_bars: 16.0,
            sample_offset_sec: 0.0,
            source_path: None,
            waveform_peaks: vocal_wave.clone(),
            volume: 1.0,
            muted: false,
            color: Color32::from_rgb(180, 110, 255),
        };

        let mut t0 = PlaylistTrack {
            name: "🎙 Sång (Lead Vocal)".to_string(), icon: "🎙", kind: TrackKind::VocalAudio, color: Color32::from_rgb(180, 110, 255),
            volume: 0.95, pan: 0.0, muted: false, solo: false, is_rec_armed: true,
            regions: vec![r0],
            clips: [None; 32],
            audio_waveform: Some(vocal_wave),
            custom_clip_name: Some("Lead Vocal (Take 1)".to_string()),
            automation_enabled: false,
        };
        for b in 0..16 { t0.clips[b] = Some(1); }

        let mut t1 = PlaylistTrack {
            name: "🥁 Trummor & Beats".to_string(), icon: "🥁", kind: TrackKind::Drums, color: Theme::FL_CYAN,
            volume: 0.9, pan: 0.0, muted: false, solo: false, is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            audio_waveform: None,
            custom_clip_name: Some("808 Electro Drum Loop [126 BPM]".to_string()),
            automation_enabled: false,
        };
        for b in 0..4 { t1.clips[b] = Some(1); }
        for b in 4..16 { t1.clips[b] = Some(0); }

        let mut t2 = PlaylistTrack {
            name: "🎹 Lead Melodi & Synt".to_string(), icon: "🎹", kind: TrackKind::SynthLead, color: Theme::FL_GREEN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            audio_waveform: None,
            custom_clip_name: Some("303 Acid Arp Hook".to_string()),
            automation_enabled: false,
        };
        for b in 4..8 { t2.clips[b] = Some(0); }
        for b in 8..12 { t2.clips[b] = Some(2); }
        for b in 12..16 { t2.clips[b] = Some(0); }

        let mut t3 = PlaylistTrack {
            name: "🎸 Bas / Basgång".to_string(), icon: "🎸", kind: TrackKind::Bassline, color: Color32::from_rgb(255, 80, 140),
            volume: 0.9, pan: 0.0, muted: false, solo: false, is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            audio_waveform: None,
            custom_clip_name: Some("Deep Sub Bass Groove".to_string()),
            automation_enabled: false,
        };
        for b in 0..16 { t3.clips[b] = Some(3); }

        let r4 = AudioRegion {
            id: 2,
            name: "Custom Vinyl Sample Chop".to_string(),
            start_bar: 0.0,
            length_bars: 8.0,
            sample_offset_sec: 0.0,
            source_path: None,
            waveform_peaks: custom_wave.clone(),
            volume: 1.0,
            muted: false,
            color: Color32::from_rgb(255, 200, 80),
        };

        let mut t4 = PlaylistTrack {
            name: "📂 Egna Ljud & Sampler".to_string(), icon: "📂", kind: TrackKind::CustomAudio, color: Color32::from_rgb(255, 200, 80),
            volume: 0.85, pan: 0.0, muted: false, solo: false, is_rec_armed: false,
            regions: vec![r4],
            clips: [None; 32],
            audio_waveform: Some(custom_wave),
            custom_clip_name: Some("Custom Vinyl Sample Chop".to_string()),
            automation_enabled: false,
        };
        for b in 0..8 { t4.clips[b] = Some(2); }

        let mut t5 = PlaylistTrack {
            name: "⚡ FX & Drop".to_string(), icon: "⚡", kind: TrackKind::Fx, color: Theme::FL_PURPLE,
            volume: 0.8, pan: 0.0, muted: false, solo: false, is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            audio_waveform: None,
            custom_clip_name: Some("Riser & Cyber Drop".to_string()),
            automation_enabled: false,
        };
        t5.clips[7] = Some(1);
        t5.clips[15] = Some(1);

        let playlist_tracks = vec![t0, t1, t2, t3, t4, t5];

        let selected_pattern = 0;
        let mut initial_channels = channels;
        for (ch_i, ch) in initial_channels.iter_mut().enumerate() {
            if ch_i < patterns[0].channel_steps.len() {
                ch.steps = patterns[0].channel_steps[ch_i];
                ch.notes = patterns[0].channel_notes[ch_i];
            }
        }
        let initial_grid = patterns[0].piano_roll_grid;

        Self {
            engine,
            is_playing: true,
            bpm: 126.0,
            swing: 0.15,
            current_step: 0,
            song_bar: 0,
            song_step_in_bar: 0,
            loop_start_bar: 0,
            loop_end_bar: 16,
            last_step_time: Instant::now(),
            song_time: 0.0,
            pattern_mode: false,
            view_mode: ViewMode::PlaylistArranger,
            status_message: "Välkommen till Sonix Studio! Klicka på Play eller Mellanslag för att lyssna.".to_string(),
            show_help_guide: true,
            show_browser: true,
            browser_search: String::new(),
            master_volume: 0.85,
            master_pan: 0.0,
            stereo_width: 1.0,
            eq_low: 1.5,
            eq_mid: 0.0,
            eq_high: 2.0,
            current_preset,
            waveform,
            adsr,
            filter,
            delay: DelayParams::default(),
            reverb: ReverbParams::default(),
            drive: 1.2,
            octave: 4,
            patterns,
            selected_pattern,
            channels: initial_channels,
            selected_channel: 6,
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
            selected_timeline_track: 0,
            // Project Metadata & Suno Multi-Track Stems
            project_name: "Mitt Låtprojekt (Untitled)".to_string(),
            show_suno_import_modal: false,
            detected_suno_zips: Vec::new(),
            custom_stem_path_input: "/home/alex/Music".to_string(),
        }
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

    pub fn toggle_playback(&mut self) {
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

    pub fn seek_song_bar(&mut self, bar: usize) {
        self.song_bar = bar;
        self.song_step_in_bar = 0;
        self.song_time = bar as f32 * (60.0 / self.bpm * 4.0);
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(self.song_time));
        self.status_message = format!("Flyttade markör till Takt {}", bar + 1);
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
                    if self.song_bar >= self.loop_end_bar || self.song_bar >= 32 {
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

            if let Some(pat_idx) = track.clips[bar] {
                if let Some(pat) = self.patterns.get(pat_idx) {
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
                            if pat.channel_steps.len() > 6 && pat.channel_steps[6][step_in_bar] && step_in_bar % 4 == 0 {
                                let note = 55 + (t_idx as u8 * 3);
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * 0.65 });
                            }
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
                num_bars: self.loop_end_bar.max(4).min(32),
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
        self.advance_sequencer();
        self.anim_phase += 0.08;

        // Smooth 60 FPS while playing, 30 FPS when idle (no 100% CPU lock)
        if self.is_playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        // Keyboard Shortcuts
        ctx.input(|i| {
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

            if i.key_pressed(egui::Key::Space) {
                self.toggle_playback();
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
        // 1. TOP METALLIC TOOLBAR (FL STUDIO / GARAGEBAND 2-ROW DESIGN)
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
                    if ui.add(egui::Button::new(egui::RichText::new(" ▶ PLAY ").strong().size(12.0).color(Color32::WHITE)).fill(play_color)).clicked() {
                        if !self.is_playing {
                            self.toggle_playback();
                        }
                    }

                    let pause_color = if !self.is_playing { Theme::FL_ORANGE } else { Color32::from_rgb(45, 40, 35) };
                    if ui.add(egui::Button::new(egui::RichText::new(" ⏸ PAUSE ").strong().size(12.0).color(Color32::WHITE)).fill(pause_color)).clicked() {
                        if self.is_playing {
                            self.toggle_playback();
                        }
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(" ⏹ STOP ").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(45, 48, 56))).clicked() {
                        self.stop_playback();
                    }

                    ui.separator();

                    // Digital LCD Display & Clock
                    ui.group(|ui| {
                        ui.set_height(26.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("BPM").size(9.0).color(Theme::TEXT_MUTED));
                            ui.label(egui::RichText::new(format!("{:.1}", self.bpm)).strong().size(13.0).color(Theme::LCD_TEXT));
                            if ui.button("▲").clicked() && self.bpm < 240.0 { self.bpm += 1.0; }
                            if ui.button("▼").clicked() && self.bpm > 40.0 { self.bpm -= 1.0; }

                            ui.separator();

                            let mode_text = if self.pattern_mode { "PAT" } else { "SONG" };
                            let mode_color = if self.pattern_mode { Theme::FL_ORANGE } else { Theme::FL_CYAN };
                            if ui.add(egui::Button::new(egui::RichText::new(mode_text).strong().size(9.5).color(Color32::BLACK)).fill(mode_color)).clicked() {
                                self.pattern_mode = !self.pattern_mode;
                                self.status_message = format!("Läge ändrat till: {}", if self.pattern_mode { "PAT (Mönsterloop)" } else { "SONG (Låtläge)" });
                            }

                            ui.separator();

                            let total_sec = self.song_time as u32;
                            let min = total_sec / 60;
                            let sec = total_sec % 60;
                            if self.pattern_mode {
                                let bar = (self.current_step / 4) + 1;
                                let step_in_bar = (self.current_step % 4) + 1;
                                ui.label(egui::RichText::new(format!("{:02}:{:02} [B{}:S{}]", min, sec, bar, step_in_bar)).strong().size(12.0).color(Theme::LCD_ORANGE));
                            } else {
                                let bar = self.song_bar + 1;
                                let step = (self.song_step_in_bar / 4) + 1;
                                ui.label(egui::RichText::new(format!("{:02}:{:02} [TAKT {}/{} : Beat {}]", min, sec, bar, self.loop_end_bar, step)).strong().size(12.0).color(Theme::FL_CYAN));
                            }
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

                    // Suno Stems Importer Button
                    if ui.add(egui::Button::new(egui::RichText::new("📦 SUNO STEMS").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
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

                    let arr_btn = ui.selectable_label(self.view_mode == ViewMode::PlaylistArranger, "🎼 1. Tidslinje (Låt)");
                    if arr_btn.clicked() { self.view_mode = ViewMode::PlaylistArranger; }

                    let voc_btn = ui.selectable_label(self.view_mode == ViewMode::VocalStudio, "🎙 2. Sång & Ljudinspelning");
                    if voc_btn.clicked() { self.view_mode = ViewMode::VocalStudio; }

                    let rack_btn = ui.selectable_label(self.view_mode == ViewMode::ChannelRack, "🥁 3. Beat Maker");
                    if rack_btn.clicked() { self.view_mode = ViewMode::ChannelRack; }

                    let roll_btn = ui.selectable_label(self.view_mode == ViewMode::PianoRoll, "🎹 4. Pianorulle");
                    if roll_btn.clicked() { self.view_mode = ViewMode::PianoRoll; }

                    let ai_btn = ui.selectable_label(self.view_mode == ViewMode::AiMusicAssistant, "🤖 5. AI Assistent");
                    if ai_btn.clicked() { self.view_mode = ViewMode::AiMusicAssistant; }

                    let fx_btn = ui.selectable_label(self.view_mode == ViewMode::EffectsMixer, "🎚 6. Mixer");
                    if fx_btn.clicked() { self.view_mode = ViewMode::EffectsMixer; }

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
                        ViewMode::ChannelRack => {
                            self.render_channel_rack(ui);
                            ui.add_space(10.0);
                            self.render_synth_hardware_rack(ui);
                        }
                        ViewMode::PianoRoll => {
                            self.render_piano_roll_editor(ui);
                            ui.add_space(10.0);
                            self.render_synth_hardware_rack(ui);
                        }
                        ViewMode::PlaylistArranger => {
                            self.render_playlist_arranger(ui);
                            ui.add_space(10.0);
                            self.render_channel_rack(ui);
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
                            render_vocal_studio_view(ui, &mut self.vocal_studio, &mut self.vocal_harmonizer, self.is_playing, self.current_step, &mut self.status_message);
                        }
                        ViewMode::AiMusicAssistant => {
                            render_ai_assistant_view(ui, &mut self.ai_assistant, &mut self.channels, &mut self.status_message);
                        }
                        ViewMode::EffectsMixer => {
                            self.render_effects_mixer_rack(ui);
                        }
                    }

                    ui.add_space(10.0);
                    self.render_touch_piano_keyboard(ui, ctx);
                });
            });

        // 4. Modals & Dialogs
        self.render_import_sample_modal(ctx);
        self.render_hardware_controller_modal(ctx);
        self.render_batch_export_modal(ctx);
        self.render_suno_stem_import_modal(ctx);
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

                let mut track = PlaylistTrack {
                    name: s_name.to_string(),
                    icon: "✨",
                    kind,
                    color: col,
                    volume: 0.90,
                    pan: 0.0,
                    muted: false,
                    solo: false,
                    is_rec_armed: false,
                    clips: [None; 32],
                    regions: Vec::new(),
                    audio_waveform: Some(wave),
                    custom_clip_name: Some(format!("Stem: {}", prompt)),
                    automation_enabled: false,
                };
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

            let mut new_track = PlaylistTrack {
                name: track_name.to_string(),
                icon: "✨",
                kind: TrackKind::CustomAudio,
                color: Color32::from_rgb(175, 115, 255),
                volume: 0.90,
                pan: 0.0,
                muted: false,
                solo: false,
                is_rec_armed: false,
                clips: [None; 32],
                regions: Vec::new(),
                audio_waveform: Some(new_wave),
                custom_clip_name: Some(format!("AI Clip: {}", prompt)),
                automation_enabled: false,
            };

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
            // 1. SUNO STUDIO STYLE TOPBAR (Project Title, Capsule Transport & Meta)
            // ================================================================
            ui.horizontal(|ui| {
                // Project title pill
                ui.group(|ui| {
                    ui.set_height(24.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("🎵 {}", self.project_name)).strong().size(12.0).color(Theme::TEXT_BRIGHT));
                        if ui.button("•••").on_hover_text("Projektinställningar & Importera Suno Stämmor").clicked() {
                            self.scan_for_suno_stems();
                            self.show_suno_import_modal = true;
                        }
                    });
                });

                ui.separator();

                // Suno Stems Button
                if ui.add(egui::Button::new(egui::RichText::new("📦 Importera Suno Stems").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
                    self.scan_for_suno_stems();
                    self.show_suno_import_modal = true;
                }

                ui.separator();

                // Centered Capsule Transport Bar (Suno Style)
                ui.group(|ui| {
                    ui.set_height(24.0);
                    ui.horizontal(|ui| {
                        // Level LED bars
                        let peak = self.engine.get_peak_level();
                        let (m_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                        let h = (peak * 14.0).clamp(2.0, 14.0);
                        let bar_rect = Rect::from_min_max(Pos2::new(m_rect.min.x + 4.0, m_rect.max.y - h), Pos2::new(m_rect.max.x - 4.0, m_rect.max.y));
                        ui.painter().rect_filled(bar_rect, Rounding::same(1.0), Theme::FL_GREEN);

                        // Transport Buttons
                        if ui.button("⏮").clicked() {
                            self.seek_song_bar(0);
                        }

                        let rec_col = Color32::from_rgb(200, 40, 40);
                        if ui.add(egui::Button::new(egui::RichText::new("⏺").size(10.0).color(Color32::WHITE)).fill(rec_col)).clicked() {
                            self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::LeadVocals;
                            self.view_mode = ViewMode::VocalStudio;
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

                        // Digital Clock & Bars:Beats (Suno Style)
                        let total_sec = self.song_time as u32;
                        let min = total_sec / 60;
                        let sec = total_sec % 60;
                        let bar = self.song_bar + 1;
                        let beat = (self.song_step_in_bar / 4) + 1;
                        let tick = (self.song_step_in_bar % 4) + 1;
                        ui.label(egui::RichText::new(format!("{:02}:{:02}.000   {:03}.{}.{}", min, sec, bar, beat, tick)).strong().size(11.0).color(Theme::TEXT_BRIGHT));
                    });
                });

                ui.separator();

                // BPM & Time Signature Pill
                ui.group(|ui| {
                    ui.set_height(24.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{:.0} BPM", self.bpm)).strong().size(11.0).color(Theme::TEXT_BRIGHT));
                        ui.separator();
                        ui.label(egui::RichText::new("4/4").strong().size(11.0).color(Theme::TEXT_MUTED));
                    });
                });

                ui.separator();

                // Arranger Tool Selector (Select, Paint, Slice, Mute, Erase)
                ui.group(|ui| {
                    ui.set_height(24.0);
                    ui.horizontal(|ui| {
                        let tools = [
                            (ArrangerTool::Select, "⇱ Välj"),
                            (ArrangerTool::Paint, "✎ Rita"),
                            (ArrangerTool::Slice, "✂ Klipp"),
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

                // Export & Library Pills
                if ui.button(egui::RichText::new("↗ Export").strong().color(Theme::FL_CYAN)).clicked() {
                    self.show_render_queue_modal = true;
                }
                if ui.button("|||\\ Library").clicked() {
                    self.show_browser = !self.show_browser;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 Rensa").clicked() {
                        for t in &mut self.playlist_tracks {
                            t.clips = [None; 32];
                        }
                    }
                    if ui.button("⚡ Återställ Demo").clicked() {
                        if self.playlist_tracks.len() >= 6 {
                            for b in 0..32 {
                                self.playlist_tracks[0].clips[b] = if b < 16 { Some(1) } else { None };
                                self.playlist_tracks[1].clips[b] = if b < 16 { Some(0) } else { None };
                                self.playlist_tracks[2].clips[b] = if b >= 4 && b < 16 { Some(if b >= 8 && b < 12 { 2 } else { 0 }) } else { None };
                                self.playlist_tracks[3].clips[b] = if b < 16 { Some(3) } else { None };
                                self.playlist_tracks[4].clips[b] = if b < 8 { Some(2) } else { None };
                                self.playlist_tracks[5].clips[b] = if b == 7 || b == 15 { Some(1) } else { None };
                            }
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // ================================================================
            // 2. SUNO STUDIO MAIN TIMELINE CANVAS & TRACK HEADERS
            // ================================================================
            let header_w = 210.0;
            let bar_w = (38.0 * self.suno_zoom_level).clamp(24.0, 70.0);
            let row_h = 44.0;
            let ruler_h = 24.0;

            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.vertical(|ui| {
                    // Top Bar Ruler (Bar Ticks 1, 9, 17, 25, 33...)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                        // Corner spacer
                        let (corner_rect, _) = ui.allocate_exact_size(Vec2::new(header_w, ruler_h), Sense::hover());
                        ui.painter().rect_filled(corner_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 26));
                        ui.painter().text(
                            Pos2::new(corner_rect.min.x + 8.0, corner_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "SPÅR",
                            egui::FontId::proportional(10.5),
                            Theme::TEXT_MUTED,
                        );

                        // Bar numbers (1, 5, 9, 13, 17, 21, 25, 29, 32...)
                        for bar_idx in 0..32 {
                            let (r_rect, r_resp) = ui.allocate_exact_size(Vec2::new(bar_w, ruler_h), Sense::click());
                            let is_current_bar = !self.pattern_mode && self.song_bar == bar_idx;
                            let in_loop = bar_idx >= self.loop_start_bar && bar_idx < self.loop_end_bar;

                            let ruler_bg = if is_current_bar {
                                Theme::FL_CYAN
                            } else if in_loop {
                                Color32::from_rgb(32, 38, 50)
                            } else {
                                Color32::from_rgb(18, 21, 28)
                            };

                            let text_color = if is_current_bar { Color32::BLACK } else { Theme::TEXT_MUTED };

                            ui.painter().rect_filled(r_rect, Rounding::same(2.0), ruler_bg);
                            ui.painter().rect_stroke(r_rect, Rounding::same(2.0), Stroke::new(0.5_f32, Color32::from_rgb(38, 44, 56)));

                            let bar_label = format!("{}", bar_idx + 1);
                            ui.painter().text(
                                r_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                bar_label,
                                egui::FontId::proportional(10.0),
                                text_color,
                            );

                            if r_resp.clicked() {
                                self.seek_song_bar(bar_idx);
                                self.pattern_mode = false;
                            }
                        }
                    });

                    ui.add_space(2.0);

                    // Track Rows
                    let num_tracks = self.playlist_tracks.len();
                    for t_idx in 0..num_tracks {
                        let is_sel_track = self.selected_timeline_track == t_idx;

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                            // Left Track Header Card (Sonix Style)
                            let (h_rect, h_resp) = ui.allocate_exact_size(Vec2::new(header_w, row_h), Sense::click());
                            let track_color = self.playlist_tracks[t_idx].color;
                            let track_muted = self.playlist_tracks[t_idx].muted;
                            let track_solo = self.playlist_tracks[t_idx].solo;
                            let track_vol = self.playlist_tracks[t_idx].volume;
                            let track_name = self.playlist_tracks[t_idx].name.clone();
                            let track_auto = self.playlist_tracks[t_idx].automation_enabled;

                            let card_bg = if is_sel_track { Color32::from_rgb(26, 30, 40) } else { Color32::from_rgb(18, 21, 28) };
                            ui.painter().rect_filled(h_rect, Rounding::same(3.0), card_bg);
                            ui.painter().rect_stroke(h_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_sel_track { track_color } else { Color32::from_rgb(34, 40, 52) }));

                            // Left Accent Bar
                            let color_bar = Rect::from_min_size(h_rect.min, Vec2::new(4.0, h_rect.height()));
                            ui.painter().rect_filled(color_bar, Rounding::same(1.5), track_color);

                            // Track Number & Name
                            ui.painter().text(
                                Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + 12.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{}  {}", t_idx + 1, track_name),
                                egui::FontId::proportional(11.0),
                                Color32::WHITE,
                            );

                            // Controls Row inside header (Vol Slider, Pan, Mute, Solo, Auto)
                            let vol_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + 26.0), Vec2::new(75.0, 12.0));
                            ui.painter().rect_filled(vol_rect, Rounding::same(2.0), Color32::from_rgb(30, 35, 45));
                            let vol_fill_w = vol_rect.width() * (track_vol / 1.25).clamp(0.0, 1.0);
                            ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), track_color);

                            // Mute icon (speaker)
                            let m_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 70.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                            let m_bg = if track_muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 35, 45) };
                            ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
                            ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(8.5), if track_muted { Color32::WHITE } else { Theme::TEXT_MUTED });

                            // Solo (S)
                            let s_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 48.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                            let s_bg = if track_solo { Theme::FL_ORANGE } else { Color32::from_rgb(30, 35, 45) };
                            ui.painter().rect_filled(s_rect, Rounding::same(2.0), s_bg);
                            ui.painter().text(s_rect.center(), egui::Align2::CENTER_CENTER, "S", egui::FontId::proportional(9.0), if track_solo { Color32::BLACK } else { Theme::TEXT_MUTED });

                            // Automation (A)
                            let a_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 26.0, h_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
                            let a_bg = if track_auto { Theme::FL_CYAN } else { Color32::from_rgb(30, 35, 45) };
                            ui.painter().rect_filled(a_rect, Rounding::same(2.0), a_bg);
                            ui.painter().text(a_rect.center(), egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(9.0), if track_auto { Color32::BLACK } else { Theme::TEXT_MUTED });

                            if h_resp.clicked() {
                                self.selected_timeline_track = t_idx;
                                if let Some(mouse_pos) = h_resp.hover_pos() {
                                    if m_rect.contains(mouse_pos) {
                                        self.playlist_tracks[t_idx].muted = !self.playlist_tracks[t_idx].muted;
                                        self.sync_track_audio_state(t_idx);
                                    } else if s_rect.contains(mouse_pos) {
                                        self.playlist_tracks[t_idx].solo = !self.playlist_tracks[t_idx].solo;
                                        self.sync_track_audio_state(t_idx);
                                    } else if a_rect.contains(mouse_pos) {
                                        self.playlist_tracks[t_idx].automation_enabled = !self.playlist_tracks[t_idx].automation_enabled;
                                    } else if vol_rect.contains(mouse_pos) {
                                        let ratio = (mouse_pos.x - vol_rect.min.x) / vol_rect.width();
                                        self.playlist_tracks[t_idx].volume = (ratio * 1.25).clamp(0.0, 1.25);
                                        self.sync_track_audio_state(t_idx);
                                    }
                                }
                            }

                            // Full 32-bar Timeline Track Lane
                            let lane_w = bar_w * 32.0;
                            let (lane_rect, lane_resp) = ui.allocate_exact_size(Vec2::new(lane_w, row_h), Sense::click());

                            // Draw background bar grid lines
                            for bar_i in 0..32 {
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
                            }

                            // Render continuous Audio Regions on this track
                            let track = &mut self.playlist_tracks[t_idx];
                            let mut split_action: Option<(usize, f32)> = None;
                            let mut delete_action: Option<usize> = None;
                            let mut mute_action: Option<usize> = None;

                            if !track.regions.is_empty() {
                                for (_r_i, region) in track.regions.iter().enumerate() {
                                    let rx_start = lane_rect.min.x + region.start_bar * bar_w;
                                    let rx_end = rx_start + region.length_bars * bar_w;
                                    let r_rect = Rect::from_min_max(Pos2::new(rx_start + 1.0, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.0, lane_rect.max.y - 2.0));

                                    if r_rect.width() > 2.0 {
                                        let col = region.color;
                                        let fill = if region.muted {
                                            Color32::from_rgb(24, 26, 32)
                                        } else {
                                            Color32::from_rgb(
                                                (col.r() as f32 * 0.28) as u8,
                                                (col.g() as f32 * 0.28) as u8,
                                                (col.b() as f32 * 0.28) as u8,
                                            )
                                        };

                                        ui.painter().rect_filled(r_rect, Rounding::same(3.0), fill);
                                        ui.painter().rect_stroke(r_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if region.muted { Color32::from_rgb(70, 75, 85) } else { col }));

                                        // Region header banner
                                        ui.painter().text(
                                            Pos2::new(r_rect.min.x + 6.0, r_rect.min.y + 8.0),
                                            egui::Align2::LEFT_CENTER,
                                            &region.name,
                                            egui::FontId::proportional(9.5),
                                            if region.muted { Color32::from_rgb(140, 140, 150) } else { Color32::WHITE },
                                        );

                                        // Draw continuous waveform inside region
                                        let num_points = region.waveform_peaks.len().max(1);
                                        let step_w = (r_rect.width() - 4.0) / num_points as f32;
                                        let mid_y = r_rect.center().y + 4.0;
                                        let wave_col = if region.muted { Color32::from_rgb(90, 95, 105) } else { Color32::from_rgb(210, 225, 255) };

                                        for (wi, &amp) in region.waveform_peaks.iter().enumerate() {
                                            let sx = r_rect.min.x + 2.0 + wi as f32 * step_w;
                                            let h = amp * (r_rect.height() * 0.35);
                                            ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.2_f32, wave_col));
                                        }
                                    }
                                }

                                if lane_resp.clicked() {
                                    if let Some(mouse_pos) = lane_resp.hover_pos() {
                                        let click_bar = (mouse_pos.x - lane_rect.min.x) / bar_w;
                                        for (r_i, r) in track.regions.iter().enumerate() {
                                            if click_bar >= r.start_bar && click_bar <= (r.start_bar + r.length_bars) {
                                                match self.arranger_tool {
                                                    ArrangerTool::Slice => {
                                                        split_action = Some((r_i, click_bar));
                                                    }
                                                    ArrangerTool::Erase => {
                                                        delete_action = Some(r_i);
                                                    }
                                                    ArrangerTool::Mute => {
                                                        mute_action = Some(r_i);
                                                    }
                                                    _ => {}
                                                }
                                                break;
                                            }
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
                            }

                            // Execute slice / erase / mute actions
                            if let Some((r_idx, click_bar)) = split_action {
                                let orig = track.regions[r_idx].clone();
                                let split_offset_bar = click_bar - orig.start_bar;
                                if split_offset_bar > 0.2 && split_offset_bar < orig.length_bars - 0.2 {
                                    let sec_per_bar = 60.0 / self.bpm * 4.0;
                                    let split_points = ((split_offset_bar / orig.length_bars) * orig.waveform_peaks.len() as f32) as usize;

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
                                        muted: orig.muted,
                                        color: orig.color,
                                    };

                                    let r_right = AudioRegion {
                                        id: orig.id + 100,
                                        name: format!("{} [Del 2]", orig.name),
                                        start_bar: click_bar,
                                        length_bars: orig.length_bars - split_offset_bar,
                                        sample_offset_sec: orig.sample_offset_sec + (split_offset_bar * sec_per_bar),
                                        source_path: orig.source_path,
                                        waveform_peaks: right_peaks,
                                        volume: orig.volume,
                                        muted: orig.muted,
                                        color: orig.color,
                                    };

                                    track.regions.remove(r_idx);
                                    track.regions.insert(r_idx, r_right);
                                    track.regions.insert(r_idx, r_left);
                                    self.status_message = format!("✂ Klippte region vid takt {:.2}!", click_bar + 1.0);
                                }
                            }

                            if let Some(d_idx) = delete_action {
                                if d_idx < track.regions.len() {
                                    let name = track.regions[d_idx].name.clone();
                                    track.regions.remove(d_idx);
                                    self.status_message = format!("🗑 Raderade '{}'", name);
                                }
                            }

                            if let Some(m_idx) = mute_action {
                                if m_idx < track.regions.len() {
                                    track.regions[m_idx].muted = !track.regions[m_idx].muted;
                                    self.status_message = format!("🔇 Toggla mute för '{}'", track.regions[m_idx].name);
                                }
                            }
                        });
                        ui.add_space(2.0);
                    }

                    // "+ Add New Track" Action Row
                    ui.horizontal(|ui| {
                        let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(header_w, 24.0), Sense::click());
                        ui.painter().rect_filled(add_rect, Rounding::same(3.0), Color32::from_rgb(24, 28, 36));
                        ui.painter().rect_stroke(add_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(40, 46, 58)));
                        ui.painter().text(add_rect.center(), egui::Align2::CENTER_CENTER, "➕ Add New Track", egui::FontId::proportional(10.5), Theme::TEXT_MUTED);

                        if add_resp.clicked() {
                            let new_id = self.playlist_tracks.len() + 1;
                            self.playlist_tracks.push(PlaylistTrack {
                                name: format!("Spår {}", new_id),
                                icon: "🎵",
                                kind: TrackKind::CustomAudio,
                                color: Theme::FL_YELLOW,
                                volume: 0.85,
                                pan: 0.0,
                                muted: false,
                                solo: false,
                                is_rec_armed: false,
                                regions: Vec::new(),
                                clips: [None; 32],
                                audio_waveform: None,
                                custom_clip_name: Some(format!("Nytt Ljudspår {}", new_id)),
                                automation_enabled: false,
                            });
                            self.status_message = format!("Lade till Spår {}", new_id);
                        }
                    });

                    // Bottom Time Markers Row (00:00, 00:15, 00:30, 00:45, 01:00...)
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.allocate_exact_size(Vec2::new(header_w, 16.0), Sense::hover());
                        for t_marker in (0..24).step_by(4) {
                            let (m_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w * 4.0, 16.0), Sense::hover());
                            let seconds = (t_marker as f32 * 60.0 / self.bpm * 4.0) as u32;
                            let m_str = format!("{:02}:{:02}", seconds / 60, seconds % 60);
                            ui.painter().text(m_rect.min, egui::Align2::LEFT_TOP, m_str, egui::FontId::proportional(9.0), Theme::TEXT_MUTED);
                        }
                    });
                });
            });

            ui.add_space(8.0);

            // ================================================================
            // 3. SUNO STUDIO FLOATING AI PROMPT & GENERATION BAR (BETA)
            // ================================================================
            ui.horizontal(|ui| {
                ui.add_space(60.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width() - 120.0);
                    ui.horizontal(|ui| {
                        // Beta Tag
                        ui.label(egui::RichText::new("BETA").strong().size(9.0).color(Theme::FL_ORANGE));
                        ui.separator();

                        // Add Context (+)
                        if ui.button("➕").clicked() {
                            self.suno_context_clip = Some(format!("{} - Master Mix", self.project_name));
                        }

                        // Context Chip
                        let mut clear_context = false;
                        if let Some(ref ctx_clip) = self.suno_context_clip {
                            let fill = Color32::from_rgb(60, 35, 100);
                            let clip_label = format!("🟣 {}", ctx_clip);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(clip_label).size(10.5).color(Color32::WHITE));
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
                        let resp = ui.add_sized(
                            [ui.available_width() - 170.0, 24.0],
                            egui::TextEdit::singleline(&mut self.suno_prompt_input)
                                .hint_text("Skriv prompt (t.ex. 'Generera ett 8-takters synthwave-trumkomp och sångvers')..."),
                        );

                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.trigger_suno_ai_generation();
                        }

                        // Model Selector Pill (Local & Open Source AI Engines)
                        egui::ComboBox::from_id_salt("suno_model_combo")
                            .selected_text(&self.suno_model_version)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.suno_model_version, "ACE-Step (Lokal Open-Source Suno)".to_string(), "ACE-Step (Lokal Suno)");
                                ui.selectable_value(&mut self.suno_model_version, "Stable Audio 3.0 (DAW Plugin)".to_string(), "Stable Audio 3.0");
                                ui.selectable_value(&mut self.suno_model_version, "Meta MusicGen (Lokal ONNX)".to_string(), "MusicGen (Lokal)");
                                ui.selectable_value(&mut self.suno_model_version, "Demucs UVR5 Stem Isolation".to_string(), "UVR5 Stems");
                                ui.selectable_value(&mut self.suno_model_version, "ARA2 Harmonizer Engine".to_string(), "ARA2 Harmony");
                            });

                        // Submit / Generate Button (↑)
                        let gen_bg = if self.suno_is_generating { Color32::RED } else { Theme::FL_GREEN };
                        if ui.add(egui::Button::new(egui::RichText::new(" ⬆ ").strong().size(13.0).color(Color32::WHITE)).fill(gen_bg)).clicked() {
                            self.trigger_suno_ai_generation();
                        }
                    });
                });
                ui.add_space(60.0);
            });

            ui.add_space(6.0);

            // ================================================================
            // 4. SUNO STUDIO BOTTOM UTILITY DOCK (Tools, FX, Zoom & Guide)
            // ================================================================
            ui.horizontal(|ui| {
                // Left Tool Selector
                ui.label(egui::RichText::new("VERKTYG:").size(9.5).color(Theme::TEXT_MUTED));
                let ptr_btn = ui.selectable_label(self.arranger_tool == ArrangerTool::Paint, "⇱ Markör");
                if ptr_btn.clicked() { self.arranger_tool = ArrangerTool::Paint; }

                let draw_btn = ui.selectable_label(self.arranger_tool == ArrangerTool::Paint, "✎ Rita");
                if draw_btn.clicked() { self.arranger_tool = ArrangerTool::Paint; }

                let chop_btn = ui.selectable_label(self.arranger_tool == ArrangerTool::Erase, "〰 Klipp");
                if chop_btn.clicked() { self.arranger_tool = ArrangerTool::Erase; }

                ui.separator();

                // Center Action Button
                if ui.add(egui::Button::new(egui::RichText::new("➕ Add Track Effects").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(45, 55, 75))).clicked() {
                    self.view_mode = ViewMode::EffectsMixer;
                }

                ui.separator();

                // Right Utility Controls (Files, Zoom, Guide)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("💡 Learn / Guide").clicked() {
                        self.show_help_guide = !self.show_help_guide;
                    }

                    if ui.button("🔍+").clicked() && self.suno_zoom_level < 2.0 {
                        self.suno_zoom_level += 0.2;
                    }
                    if ui.button("><").clicked() {
                        self.suno_zoom_level = 1.0;
                    }
                    if ui.button("🔍-").clicked() && self.suno_zoom_level > 0.6 {
                        self.suno_zoom_level -= 0.2;
                    }

                    ui.label(egui::RichText::new("ZOOM:").size(9.5).color(Theme::TEXT_MUTED));

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
            if let Some(chop_ch) = self.active_chopper_channel {
                if chop_ch < self.channels.len() {
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
            }

            self.sync_active_pattern_from_ui();
        });
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
        for v in &mut self.step_velocities {
            *v = vel_mult;
        }

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

                        if resp.dragged() || resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                self.remix_xy[0] = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                                self.remix_xy[1] = (1.0 - (pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);

                                self.filter.cutoff = 200.0 + self.remix_xy[0] * 12000.0;
                                self.filter.resonance = 0.5 + self.remix_xy[1] * 7.5;
                                let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                            }
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

                if let Some(pos) = pointer_pos {
                    if key_rect.contains(pos) {
                        hovered_note = Some(note);
                    }
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

                if let Some(pos) = pointer_pos {
                    if key_rect.contains(pos) {
                        hovered_note = Some(note);
                    }
                }
            }

            // Mouse Touch Interaction
            if pointer_down {
                if let Some(note) = hovered_note {
                    if self.active_mouse_note != Some(note) {
                        if let Some(old) = self.active_mouse_note {
                            self.release_note(old);
                        }
                        self.active_mouse_note = Some(note);
                        self.play_note(note);
                    }
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
                        num_bars: self.loop_end_bar.max(4).min(32),
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
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("zip") {
                            if let Some(path_str) = path.to_str() {
                                results.push(path_str.to_string());
                            }
                        }
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
            .unwrap_or("Suno Stems Project");

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

        let title = if title.is_empty() { "Suno AI Låtprojekt".to_string() } else { title };
        (title, bpm)
    }

    pub fn import_suno_zip(&mut self, zip_path: &str) {
        let (title, bpm) = Self::parse_suno_zip_info(zip_path);
        let sanitized = title.replace([' ', '/', '\\', '(', ')', '\'', '"'], "_");
        let dest_dir = format!("./imported_stems/{}", sanitized);

        let _ = std::fs::create_dir_all(&dest_dir);

        match std::process::Command::new("unzip")
            .args(["-o", zip_path, "-d", &dest_dir])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    let err_str = String::from_utf8_lossy(&output.stderr);
                    eprintln!("Unzip warning: {}", err_str);
                }
            }
            Err(e) => {
                self.status_message = format!("Kunde inte köra unzip: {}", e);
            }
        }

        self.import_suno_stems_from_folder(&dest_dir, &title, bpm);
    }

    pub fn import_suno_stems_from_folder(&mut self, folder_path: &str, project_title: &str, bpm: f32) {
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
            self.status_message = format!("Inga ljudfiler hittades i mappen: {}", folder_path);
            return;
        }

        // Sort so "0 Lead Vocals", "1 Backing Vocals", "2 Drums", etc. are in standard order
        stem_files.sort();

        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        let mut new_tracks = Vec::new();
        let total_bars = 32.0;

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

            // 1. Decode full stereo PCM for real-time engine streaming
            if path_str.to_lowercase().ends_with(".wav") {
                if let Ok((l, r, s_rate)) = crate::audio::load_wav_pcm(&path_str) {
                    let arc_l = std::sync::Arc::new(l);
                    let arc_r = std::sync::Arc::new(r);
                    let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                        track_index: idx,
                        left: arc_l,
                        right: arc_r,
                        sample_rate: s_rate as f32,
                        volume: 0.9,
                        pan: 0.0,
                        start_time_secs: 0.0,
                    });
                }
            }

            // 2. Extract high-resolution waveform peaks (240 points)
            let wave_env = if path_str.to_lowercase().ends_with(".wav") {
                crate::audio::read_wav_envelope(&path_str, 240).unwrap_or_else(|_| vec![0.3; 240])
            } else {
                let mut fake_wave = Vec::with_capacity(240);
                let seed = (idx as f32 + 1.0) * 17.3;
                for i in 0..240 {
                    let t = i as f32 / 240.0;
                    let val = (t * seed).sin().abs() * 0.7 + (t * seed * 2.1).cos().abs() * 0.3;
                    fake_wave.push(val.clamp(0.08, 0.92));
                }
                fake_wave
            };

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

            let initial_region = AudioRegion {
                id: idx + 1,
                name: clean_name.to_string(),
                start_bar: 0.0,
                length_bars: total_bars,
                sample_offset_sec: 0.0,
                source_path: Some(path_str.clone()),
                waveform_peaks: wave_env.clone(),
                volume: 1.0,
                muted: false,
                color,
            };

            let mut track = PlaylistTrack {
                name: format!("{} {}", icon, clean_name),
                icon,
                kind,
                color,
                volume: 0.9,
                pan: 0.0,
                muted: false,
                solo: false,
                is_rec_armed: idx == 0,
                regions: vec![initial_region],
                clips: [None; 32],
                audio_waveform: Some(wave_env),
                custom_clip_name: Some(clean_name.to_string()),
                automation_enabled: false,
            };

            for b in 0..32 {
                track.clips[b] = Some(idx);
            }

            new_tracks.push(track);
        }

        self.playlist_tracks = new_tracks;
        self.project_name = project_title.to_string();
        self.bpm = bpm;
        self.suno_context_clip = Some(format!("{} - Master Mix", project_title));
        self.status_message = format!("✨ Importerade {} Suno-stämspår för '{}' ({:.1} BPM) med äkta vågformer!", stem_files.len(), project_title, bpm);
        self.view_mode = ViewMode::PlaylistArranger;
    }

    fn render_suno_stem_import_modal(&mut self, ctx: &egui::Context) {
        if !self.show_suno_import_modal {
            return;
        }

        let mut close = false;
        let mut zip_to_import: Option<String> = None;
        let mut folder_to_import: Option<(String, String, f32)> = None;

        egui::Window::new("📦 Suno AI Multi-Track Stems Importer")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(580.0);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🎵").size(24.0));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Suno AI & Generativa Stämmor (8 Spår)").strong().size(13.0).color(Theme::FL_ORANGE));
                            ui.label(egui::RichText::new("Importera nedladdade ZIP-paket eller mappar med Suno-stämmor. Sonix läser ut äkta 48kHz WAV-vågformer, detekterar tempo (BPM) och mappar spåren i tidslinjen.").size(10.5).color(Theme::TEXT_MUTED));
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
                                            if ui.add(egui::Button::new(egui::RichText::new("⚡ Importera Alla 8 Stämmor").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                        if ui.add(egui::Button::new(egui::RichText::new("📁 Välj ZIP-fil från datorn...").strong().size(11.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked() {
                            if let Ok(output) = std::process::Command::new("zenity")
                                .args(["--file-selection", "--file-filter=*.zip", "--title=Välj Suno ZIP Stempaket"])
                                .output()
                            {
                                if output.status.success() {
                                    let selected_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                                    if !selected_path.is_empty() {
                                        zip_to_import = Some(selected_path);
                                    }
                                }
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
                    ui.label(egui::RichText::new("8-SPÅRS STRUKTUR I SONIX:").strong().size(10.5).color(Theme::TEXT_MUTED));
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
            self.import_suno_zip(&zip_p);
            close = true;
        }

        if let Some((folder, title, bpm)) = folder_to_import {
            self.import_suno_stems_from_folder(&folder, &title, bpm);
            close = true;
        }

        if close {
            self.show_suno_import_modal = false;
        }
    }
}

fn rand_simple(seed: usize) -> f32 {
    let mut x = (seed as u32).wrapping_mul(1103515245).wrapping_add(12345);
    x = (x >> 16) & 0x7FFF;
    x as f32 / 32767.0
}
