//! **App-ytan — modulens rot.** Här bor `SonixApp` (tillståndet), `new` (starten) och
//! `update` (bildrutan). Allt annat ligger i undermodulerna nedan, delade efter område
//! 2026-09-14 — filen var 23 119 rader och är nu modulens rot.
//!
//! Undermodulerna ligger **under** `app` med flit: ett barn ser förälderns privata fält, så
//! `SonixApp` behöver inte göra sina fält publika för att en syskonmodul ska kunna läsa dem.
//! Vilken modul som äger vad står i `SECTIONS.md` (genererad av `tools/sections.py`).
//!
//! Status: byggs — roten: tillståndet, starten och bildrutan.
//! Rör inte: `update` är den enda ingången per bildruta. En ny panel läggs i sin egen modul
//! och anropas härifrån i **en** rad — annars växer filen tillbaka.
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
    mini_track_eq_curve, oscilloscope_display, pitch_knob, register_eq, rotary_knob,
    stereo_meter, vertical_fader,
};

// Undermoduler — den stora ytan är delad efter område (2026-09-14). De ligger **under** `app`
// med flit: ett barn ser förälderns privata fält, så `SonixApp` och de andra typerna behöver
// inte göra sina fält publika för att en syskonmodul ska kunna läsa dem.
mod state;
pub use state::*;

mod project;
pub use project::*;

mod import;
pub use import::*;

mod waveform;
pub(crate) use waveform::*;

mod stretch;
pub use stretch::*;

mod timeline;
pub use timeline::*;

mod mixer;
pub use mixer::*;

mod transport;

mod plugins;

mod midi;

mod piano_roll;

mod browser;
pub(crate) use browser::*;

mod export;

mod modals;

mod arranger;

#[cfg(test)]
mod tests;

/// Regionen ett fruset spår spelar: hela filen från början. Längden räknas ur
/// bufferten i stället för ur takter, så den följer med automatiskt när
/// frysningen görs om eller tempot ändras.
/// Hur mycket snabbare källjudet ska gå för att klippet ska följa projektets
/// tempo (Fas 8.10).
///
/// `rate` = källsekunder per utsekund. Ett klipp inspelat i 120 BPM som spelas i
/// ett projekt på 240 BPM ska hinna igenom dubbelt så mycket ljud på samma takter
/// — alltså 2,0. Vid 120 i ett 120-projekt blir det 1,0, och då är vägen
/// **bit-exakt** den gamla (ingen omsampling alls).
///
/// **Okänt ursprung (`source_bpm <= 0`) ger 1,0.** Att gissa ett tempo där vore att
/// hitta på data, och det är samma regel som 8.5 vilar på: säg hellre att inget
/// ändras än att ändra något ingen bett om.
///
/// Gränserna är desamma som sångstudiens reglage (0,25–4,0): utanför dem är
/// omsamplingen mer artefakt än musik, och en felaktig siffra i ett projektfält
/// ska inte kunna göra ett klipp oanvändbart.
/// **Ett klipp stycke för stycke över tempokartan** (Fas 8.10, sista punkten).
///
/// Ett klipp som spänner över ett tempobyte kan inte ha **en** faktor: tempot före bytet och
/// tempot efter kräver var sitt, och i dag räknas faktorn från tempot vid klippets *start* —
/// allt efter bytet spelas då i fel tempo.
///
/// Regeln delar klippets **ut-tid** vid varje tempobyte som ligger inuti spannet, och ger varje
/// stycke sin egen faktor, sin egen längd och sin egen start i källan.
///
/// Enheten är [`stretch_ratio_for`]: `ratio` = **ut-sekunder per källsekund** — alltså
/// `projektets tempo / källans tempo`, inte den inverterade kvoten. Det var den här kodbasens
/// kända fälla redan i 8.10c ("302,49 s ut av 259,28 s = 1,1667"), och **första versionen av
/// den här funktionen gick i den**: doc-raden påstod motsatsen och provet förväntade 0,8 där
/// 1,25 var rätt. Att en källa i 120 spelas i ett projekt i 150 ger 150/120 = 1,25, och en
/// ut-sekund gör då av med 1,25 källsekunder — filen spelas *kortare*, vilket är vad ett högre
/// tempo betyder.
///
/// **Ett enda tempo ger exakt ett stycke** med samma faktor som i dag. Det är avsiktligt: för
/// projekt utan tempobyten ska ingenting ändras, och då är den här vägen bit-identisk med den
/// gamla.
// Anropas av `ensure_stretched`: en sträckning per tempovärde i klippets spann (8.10 punkt 1).

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
    /// Tonartens grundton (0 = C, tonhöjdsklass) och skala (index i
    /// [`crate::audio::scale::SCALES`]).
    ///
    /// **Ett par, ett ställe** (Alex' kvittens 2026-09-12): piano rollen hade förut
    /// egna kopior (`piano_roll_root_note`, `selected_scale`) med **egna listor**, så
    /// arrangerarens "Dorian" blev piano rollens "Moll" och fyra av tolv grundtoner
    /// visade fel namn. Nu finns bara den här tonarten, och varje meny läser samma
    /// tabell. Fälten sparas i projektfilen (`#[serde(default)]`).
    pub song_key_root: u8,
    pub song_key_scale: usize,
    pub time_signature: (u8, u8),
    // Piano Roll & Scale Snapping
    pub piano_roll_grid: [[bool; 16]; 24],
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
    /// Registrets bandnivåer (0..1) ur `audio::spectrum` (Fas 8.13). Egen cache så
    /// att mätaren kan FALLA när ljudet tar slut i stället för att stå kvar och se
    /// levande ut.
    pub spectrum_levels: Vec<f32>,
    pub language: crate::i18n::Language,
    // Sub-Mixing, VCA Groups & PDC (Fas 5.2)
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    /// **Bussarnas egna kurvor** (Fas 8.8).
    pub bus_automation: Vec<BusAutomationLane>,
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
    /// Texturen för logotypen (samma bild som fönsterikonen). Laddas första gången den ritas.
    pub logo_texture: Option<egui::TextureHandle>,
    pub automation_param: AutomationParam,
    /// (track, lane, point) currently being dragged.
    pub automation_drag: Option<(usize, usize, usize)>,
    // Project Metadata & Suno Multi-Track Stems
    pub project_name: String,
    pub show_suno_import_modal: bool,
    /// Frågan om mp3/wav som väntar på svar (Fas 8.5). `None` = ingen fråga uppe.
    pub pending_wav_question: Option<WavQuestion>,
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
    /// Arbetstrådarna som bygger vågformscachar (Fas 8.4).
    ///
    /// Bygget får ALDRIG ske i en bildruta: en cache för tio minuters ljud tar
    /// 2,5 s, och ett projekt med åtta stems låste hela fönstret tills
    /// window-managern gav upp ("not responding"). Nu byggs de vid sidan om, och
    /// ritningen visar den gamla vägen tills svaret kommer.
    pub waveform_cache_tx: Option<std::sync::mpsc::Sender<(usize, u64, crate::audio::waveform::WaveformCache)>>,
    pub waveform_cache_rx: Option<std::sync::mpsc::Receiver<(usize, u64, crate::audio::waveform::WaveformCache)>>,
    /// Färdigsträckta filer (Fas 8.10 steg 2): en cache med en egen arbetstråd.
    /// Ljudtråden får en färdig buffert och spelar den som vilken fil som helst.
    pub stretch_cache: crate::audio::stretch::StretchCache,
    /// Väntar på att tempot ska stå still innan en sträckning beställs.
    pub tempo_settle: crate::audio::stretch::TempoSettle,
    pub show_tempo_modal: bool,
    /// Tempot som regionerna i motorn senast räknades för (Fas 8.10).
    ///
    /// Klippens sträckning hänger på projektets tempo, och tempot kan ändras från
    /// flera håll (reglaget, TAP, ett tempobyte i kartan, ett inläst projekt).
    /// I stället för en sync i varje sådan dörr jämförs det här talet med `bpm`
    /// en gång per bildruta: en olikhet betyder att motorn ska ha nya regioner.
    /// "Följ tempot" (Fas 8.10 steg 2): ett enda val för hela projektet. Av betyder att
    /// inget klipp följer tempot alls — varken sträckning eller bandspelare.
    pub follow_tempo: bool,
    pub stems_synced_bpm: f32,
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
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.3, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(20.0, 0.8),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let snare = ChannelStrip {
            name: "909 Snare".to_string(), icon: "🥁".to_string(), color: Theme::FL_CYAN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [38; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(45.0, 0.9),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let clap = ChannelStrip {
            name: "Electro Clap".to_string(), icon: "👏".to_string(), color: Theme::FL_YELLOW,
            volume: 0.80, pan: -0.1, muted: false, solo: false, steps: [false; 16], notes: [39; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(35.0, 0.85),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let hat = ChannelStrip {
            name: "Crisp Hat".to_string(), icon: "⚡".to_string(), color: Theme::FL_PURPLE,
            volume: 0.75, pan: 0.15, muted: false, solo: false, steps: [false; 16], notes: [42; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.2, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(70.0, 0.95),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let open_hat = ChannelStrip {
            name: "Open Hat".to_string(), icon: "🌊".to_string(), color: Theme::FL_CYAN,
            volume: 0.70, pan: -0.2, muted: false, solo: false, steps: [false; 16], notes: [46; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.6, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(50.0, 0.5),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let crash = ChannelStrip {
            name: "Cyber Crash".to_string(), icon: "✨".to_string(), color: Color32::from_rgb(255, 180, 50),
            volume: 0.75, pan: 0.25, muted: false, solo: false, steps: [false; 16], notes: [49; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.7, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(30.0, 0.4),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let synth_lead = ChannelStrip {
            name: "303 Lead".to_string(), icon: "🎹".to_string(), color: Theme::FL_GREEN,
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [60; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(60.0, 0.3),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let sub_bass = ChannelStrip {
            name: "Sub Bass".to_string(), icon: "🎸".to_string(), color: Color32::from_rgb(255, 80, 140),
            volume: 0.90, pan: 0.0, muted: false, solo: false, steps: [false; 16], notes: [36; 16],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), is_reverse: false,
            waveform_preview: make_wave(25.0, 0.6),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
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
            spectrum_levels: vec![0.0; crate::audio::spectrum::REGISTERS.len()],
            language,
            // Sub-Mixing, VCA Groups & PDC (Fas 5.2)
            bus_volume: [1.0; crate::audio::synth::NUM_BUSES],
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_automation: Vec::new(),
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
            logo_texture: None,
            automation_param: AutomationParam::Volume,
            automation_drag: None,
            // Project Metadata & Suno Multi-Track Stems
            project_name: crate::i18n::t("Namnlöst Projekt").to_string(),
            show_suno_import_modal: false,
            pending_wav_question: None,
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
            waveform_cache_tx: None,
            waveform_cache_rx: None,
            stretch_cache: crate::audio::stretch::StretchCache::new(),
            tempo_settle: crate::audio::stretch::TempoSettle::new(120.0),
            show_tempo_modal: false,
            follow_tempo: true,
            stems_synced_bpm: 120.0,
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

    const SCOPE_HISTORY_MAX: usize = 2048;

    /// Tak för hur många steg en och samma bildruta får ta igen (Fas 8.13b).
    ///
    /// En lång paus ska inte avfyra femtio noter på en bildruta; att tappa steg är
    /// mindre illa än att spränga låten.
    const MAX_STEPS_PER_FRAME: usize = 8;

}

impl eframe::App for SonixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        // Källfiler som inte gick att läsa: säg det i statusraden i stället för att tiga.
        // En gång per bildruta, på ett ställe — alla elva anrop omfattas, inte bara
        // projektinläsningen. Ett klipp som ser ut att ha ljud men är tyst är värre än
        // ett som är tomt; då ska det stå varför.
        let unreadable = take_unreadable_sources();
        if !unreadable.is_empty() {
            self.status_message = crate::tstatus!(
                "⚠ {} fil(er) kunde inte läsas och är tysta: {}",
                unreadable.len(),
                unreadable.join(", ")
            )
            .to_string();
        }
        // Auto-open the MIDI keyboard input port once, so external keyboards
        // work as soon as the app is opened — alla in-portar ansluts automatiskt (Fas 7.1:
        // den gamla ALSA-vägen krävde `aconnect`, midir-vägen gör det själv).
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
        self.sync_tempo_follow();
        // Sträckningen är offline (Fas 8.10 steg 2): beställ de filer som saknas och
        // ta emot dem som blivit klara. Ligger efter tempoföljningen, så att motorn
        // redan vet vilket tempo klippen ska följa.
        self.ensure_stretched();
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
            if let ScreenshotState::AwaitingCapture { dest, .. } = &self.screenshot_state {
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
                            // Plattformens egen filhanterare: xdg-open på Linux,
                            // explorer/open på Windows/macOS (Fas 7.1). Att titta på
                            // starten och inte på slutkoden är med flit — explorer
                            // avslutar med 1 även när den lyckas.
                            if let Err(e) = crate::platform::open_dir(&dir) {
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

                    // **Logotypen, inte en apelsin** (Alex 2026-09-14): samma bild som
                    // fönsterikonen. Faller tillbaka på ordet om bilden inte kan läsas.
                    let logo = self.logo_texture_for(ui.ctx());
                    if let Some(tex) = &logo {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::Vec2::splat(18.0)));
                    }
                    ui.label(egui::RichText::new(crate::i18n::t("SONIX")).strong().size(15.0).color(Theme::FL_CYAN));
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

                    // Nivå per kanal och registret (Fas 8.13, Alex 2026-09-13):
                    // scopen visar VÅGFORMEN, mätaren visar NIVÅN per kanal, och
                    // registret visar VAR i frekvensbanden energin ligger.
                    let (peak_l, peak_r) = self.engine.get_stereo_peaks();
                    stereo_meter(ui, peak_l, peak_r, Vec2::new(48.0, 22.0));
                    register_eq(
                        ui,
                        &self.spectrum_levels,
                        &crate::audio::spectrum::REGISTERS.map(|r| r.0),
                        Vec2::new(128.0, 22.0),
                    );

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
        self.render_wav_question_modal(ctx);
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
                        // Antalet står inte här: listan växer när en dialog läggs
                        // till, och en siffra i en utskrift blir fel tyst.
                        println!("🎉 Alla skärmdumpar har genererats och sparats framgångsrikt!");
                        self.screenshot_mode_active = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                ScreenshotState::Preparing { target, dest, frames_left } => {
                    if frames_left > 0 {
                        self.screenshot_state = ScreenshotState::Preparing { target, dest, frames_left: frames_left - 1 };
                        ctx.request_repaint();
                    } else {
                        self.screenshot_state = ScreenshotState::AwaitingCapture {
                            dest,
                            frames_left: SCREENSHOT_WAIT_FRAMES,
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                        ctx.request_repaint();
                    }
                }
                ScreenshotState::AwaitingCapture { dest, frames_left } => {
                    if frames_left == 0 {
                        // Svaret kom aldrig. Säg det, städa och stäng — en loop som
                        // väntar för evigt ser ut att arbeta, och det är värre än
                        // ett tydligt fel.
                        eprintln!(
                            "❌ Ingen skärmdump kom tillbaka för {:?} efter {} bildrutor. \
                             Ingen ruta har besvarat begäran — körningen avbryts så att felet syns.",
                            dest, SCREENSHOT_WAIT_FRAMES
                        );
                        self.screenshot_queue.clear();
                        self.screenshot_state = ScreenshotState::Idle;
                        self.screenshot_mode_active = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.screenshot_state =
                            ScreenshotState::AwaitingCapture { dest, frames_left: frames_left - 1 };
                        ctx.request_repaint();
                    }
                }
            }
        }
    }
}

impl SonixApp {

    const EXPORT_FORMATS: [crate::audio::ExportFormat; 7] = [
        crate::audio::ExportFormat::Wav16,
        crate::audio::ExportFormat::Wav24,
        crate::audio::ExportFormat::Wav32,
        crate::audio::ExportFormat::Flac,
        crate::audio::ExportFormat::Mp3,
        crate::audio::ExportFormat::Ogg,
        crate::audio::ExportFormat::Aac,
    ];

}

