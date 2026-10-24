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
use crate::audio::stem_separator::{SeparationResult, StemProject};
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
pub mod script;
pub mod state;
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

mod input_autoconfig;
mod midi;

// Inte kopplad till mixern än — tas bort när panelen anropar den.
#[allow(dead_code)]
mod compare;

mod piano_roll;

mod browser;
pub(crate) use browser::*;

mod export;
mod launcher;
mod macros;

mod modals;

mod arranger;

mod frame;

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
    /// Har den automatiska igenkänningen av instrumentingångar körts? (Sprint 1, punkt 1.)
    pub input_autodetect_done: bool,
    /// Är en frekvensskillnad mellan ingång och motor redan berättad? (Sprint 1, punkt 6.)
    pub rate_mismatch_reported: bool,
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
    /// **Capture MIDI**: det som spelades nyss, med tid sedan första noten.
    pub note_capture: crate::note_capture::NoteCapture,
    /// **Score-loggen** (FL:s Auto dump score): allt spelat sedan förra dumpen, utan att
    /// vikas in i en takt. Egen klocka, för capture-bufferten nollställs vid sin dump.
    pub score_log: crate::note_capture::NoteCapture,
    pub score_clock: Option<std::time::Instant>,
    /// Klockan som tiden räknas från; `None` tills första noten fångas.
    pub capture_clock: Option<std::time::Instant>,
    pub song_bar: usize,
    pub song_step_in_bar: usize,
    pub loop_start_bar: usize,
    pub loop_end_bar: usize,
    /// Slinga på/av. Av som standard: en låt ska spelas igenom, inte snurra.
    pub loop_enabled: bool,
    pub song_markers: Vec<SongMarker>,
    pub last_step_time: Instant,
    pub song_time: f32,
    pub pattern_mode: bool,
    /// Follow action för den valda slingan i mönsterläget. `None` = av.
    pub follow: Option<crate::follow::FollowAction>,
    /// Takter den valda slingan spelat sedan senaste bytet.
    pub follow_bars: f32,
    /// Villkoret för urval i pianorullen (Cubase): anslag, ton, position.
    pub note_filter: crate::midi_take::NoteFilter,
    /// Senaste urvalsräkningen, så gränssnittet kan visa hur många villkoret fångar.
    pub note_filter_hits: Option<usize>,
    /// Notsannolikheten som knappen sätter på urvalet (0–1).
    pub note_probability: f32,
    /// A/B-jämförelsen av mixen: två fångade mixar och vilken som hörs.
    pub compare: crate::ui::app::compare::Compare,
    /// Spåret som väntar på en ny ljudkälla från filväljaren ("byta källa").
    pub pending_swap_source: Option<usize>,
    /// Väntar på en fil till BeepMap (bild → deltoner).
    pub pending_beepmap: bool,
    /// Projektets anteckningar (FL:s NoteBook), sparas i projektfilen.
    pub project_notes: String,
    /// Om anteckningsfönstret är uppe.
    pub show_notes_modal: bool,
    /// Skalan riff-generatorn använder.
    pub riff_scale: crate::riff::Scale,
    /// Grupper i spellistan.
    pub track_groups: Vec<crate::ui::app::state::TrackGroup>,
    pub view_mode: ViewMode,
    /// Groovemallens styrka när den läggs på tagningen (0 = rak, 1 = full förskjutning).
    pub take_groove_amount: f32,
    /// Panel som ligger bredvid arrangemanget (Ctrl+1…5). `None` = ingen panel.
    pub docked_panel: Option<ViewMode>,
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
    /// **Steggrinden** (Gross Beats volymkurva): 16 steg, 1 = öppet, 0 = stängt, och om
    /// grinden är på. Mönstret skickas till motorn vid varje ändring (med projektets BPM).
    pub gate_gains: [f32; 16],
    pub gate_on: bool,
    pub gate_smooth: f32,
    /// **FM** (DX10/Sytrus): ratio + index. Standard `index = 0` = oscillatorn utan FM.
    pub fm: crate::audio::command::FmParams,
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
    pub piano_roll_grid: [[bool; crate::audio::tempo::MAX_STEPS]; 24],
    /// Markerade celler i rutnätet (samma form som rutnätet). Shift-klick växlar.
    pub piano_roll_sel: [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    pub piano_roll_snap_to_scale: bool,
    pub piano_roll_poly_mode: bool,
    pub chord_stamp: usize,
    pub step_velocities: [f32; crate::audio::tempo::MAX_STEPS],
    /// Nivåerna för den additiva oscillatorns övertoner.
    pub harmonics: [f32; crate::audio::synth::HARMONIC_COUNT],
    /// **Sytrus 6-op:** matrisen som UI:t redigerar. Skickas som ett block till motorn.
    pub six_op: crate::audio::synth::SixOpParams,
    /// Vilken operators envelop sliderserna redigerar (0–5).
    pub six_env_target: usize,
    /// Spektral envelop: attack, decay och sustain för den additiva oscillatorn, och hur
    /// mycket snabbare höga partialer dör (lutningen). Av = dragstängerna är statiska.
    pub partial_env: [f32; 3],
    pub partial_env_tilt: f32,
    pub partial_env_on: bool,
    /// 0 = reglagen gäller alla partialer, k = bara partial k (1–64).
    pub partial_env_target: usize,
    /// Egna konturer per partial, (0-baserat index, kontur). Mallen gäller de andra.
    pub partial_env_overrides: Vec<(usize, [f32; 3])>,
    /// Steget som ✂-knappen i tagningfönstret kompar ifrån (0–15).
    pub comp_step: f32,
    /// 0 = kompningen gäller alla takter, k = bara takt k.
    pub comp_bar: usize,
    /// Spektralt filter över partialindex: center (partialnummer), bredd och mängd.
    pub formant: crate::audio::synth::SpectralFormant,
    /// Tidsaxeln: klangformen glider från formantens läge till `to` under `seconds`.
    pub sweep: crate::audio::synth::SpectralSweep,
    /// Suboscillatorns nivå, 0–1.
    pub sub_level: f32,
    /// Filtrets drivning och key tracking, 0–1.
    pub filter_drive: f32,
    pub filter_key_track: f32,
    /// Portamento: tid i sekunder för tonhöjden att glida till nästa not.
    pub glide_time: f32,
    /// Unison: röster per not och isärstämning i cent.
    pub unison_voices: u8,
    pub unison_detune_cents: f32,
    /// Warp-läge för sträckningen (Beats/Tones/Texture/Complex).
    pub warp_mode: crate::audio::warp::WarpMode,
    /// Groove-poolen: flera mallar som läggs på tagningen samtidigt.
    pub groove_pool: Vec<crate::groove::GrooveEntry>,
    /// Mallar som lästs in från en mapp (groove-biblioteket). Poolen är de man valt;
    /// biblioteket är de som finns att välja bland.
    pub groove_library: crate::groove::GrooveLibrary,
    /// Mappen mallarna lästes från — så att 🔄 kan läsa om **samma** mapp.
    pub groove_folder: Option<std::path::PathBuf>,
    /// **FLEX**: användarens sparade ljud, inlästa vid start.
    pub flex_presets: Vec<crate::audio::flex::FlexPreset>,
    /// Modulationsmatrisen: källor → mål.
    pub mod_slots: Vec<crate::audio::modulation::ModSlot>,
    /// Om modulationsfönstret är öppet.
    pub show_mod_matrix: bool,
    /// Matrisen har ändrats och ska skickas till motorn.
    pub mod_dirty: bool,
    /// Nedladdning av neural-modellen: (pågår, andel, meddelande).
    pub model_download: std::sync::Arc<std::sync::Mutex<(bool, f32, String)>>,
    /// Utgångsbussar (kanalpar på ljudenheten), laddade ur `audio.json`.
    pub audio_output_buses: Vec<crate::audio::engine::OutputBus>,
    /// Ackordspåret: ackord per takt, sorterade på takt.
    pub song_chords: Vec<crate::ui::app::state::ChordEvent>,
    /// Om ackordspårsfönstret är öppet.
    pub show_chord_track: bool,
    /// Sektionskedja: markörindex i spelordning (Cubases Arranger Track). Tom = linjärt.
    pub arranger_chain: Vec<usize>,
    /// Om uppspelningen ska följa kedjan.
    pub arranger_chain_active: bool,
    /// Om kedjefönstret är öppet.
    pub show_arranger_chain: bool,
    /// Tröskel för "markera svaga": steg med lägre anslag markeras.
    pub piano_roll_quiet_threshold: f32,
    /// Logical Editor-lite: vilket fält regeln jämför (0 anslag, 1 tonhöjd, 2 steg).
    pub rule_field: usize,
    /// 0 `<`, 1 `>`, 2 `=`.
    pub rule_cmp: usize,
    /// Jämförelsevärde — anslag 0–1, tonhöjd som MIDI-nummer, steg 1–16.
    pub rule_value: f32,
    /// 0 radera, 1 transponera, 2 sätt anslag.
    pub rule_action: usize,
    /// Åtgärdens tal: rader vid transponering, anslag 0–1 vid sätt anslag.
    pub rule_action_value: f32,
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
    pub midi_input: Option<MidiKeyboardInput>,
    /// MIDI-kartan: FL:s splitter, key mapper och level scaler i en tabell.
    pub midi_map: crate::audio::key_mapper::KeyMap,
    /// Fraslyssning: klingar en fras innan den infogas.
    pub phrase_preview: Option<crate::audio::phrase_preview::PhrasePreview>,
    /// **MIDI ut** (Sprint 1, punkt 4): en öppen utgång, om någon är vald.
    pub midi_out: Option<crate::audio::midi_output::MidiOut>,
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
    /// Hur många steg isär ett ackords noter strummas.
    /// Text för pianorullens skriptfält.
    /// Videospåret: referensfilm synkad till tidslinjen.
    pub video: crate::ui::video_view::VideoState,
    /// Sex stämmor i stället för fyra (kräver 6s-modellen).
    pub stem_six: bool,
    pub script_text: String,
    pub take_strum: f32,
    /// Nedstrum: högsta tonen först.
    pub take_strum_down: bool,
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
    pub automation_target: AutomationTarget,
    /// **Plugin-insertarnas egna kurvor** (Fas 8.8). Egna lanes, inte inblandade i
    /// `PlaylistTrack::automation`: en plugins parametrar är en runtime-lista per instans
    /// och får inte plats i den statiska `AutomationParam`.
    pub plugin_automation: Vec<PluginAutomationLane>,
    /// (spår, mål, punkt) som just nu dras. Målet behövs: kurvan kan höra till spårets egen
    /// ratt eller till en plugin-parameter, och de två listorna har var sitt indexrum.
    pub automation_drag: Option<(usize, AutomationTarget, usize)>,
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
    /// Stor klocka (FL:s Big Clock).
    pub show_big_clock: bool,
    /// **Visualizer** (ZGameEditor-lite): vågform + spektrum i ett eget fönster.
    pub show_visualizer: bool,
    /// **VFX Script (lite)**: formel som skrivs in i en automationsbana.
    pub show_script_modal: bool,
    /// **Fruity Dance**: en figur som dansar i takt.
    pub show_dance: bool,
    /// **Take lanes** (Ableton-kompning): välj vilken tagning som spelas.
    pub show_takes_modal: bool,
    /// **Control Room**: monitorläget (nivå, dim, mono, tyst).
    pub control_room: crate::audio::control_room::ControlRoom,
    /// Vad monitorerna lyssnar på. Monitorläge — sparas inte i projektet.
    pub cue_listen: crate::audio::control_room::CueListen,
    /// **Talkback:** micken till monitorerna i stället för in i mastern.
    pub talkback: bool,
    /// **Bärvåg:** hur mycket vocoder som blandas in i mastern (0 = av).
    pub vocoder_amount: f32,
    pub script_source: String,
    pub script_param: usize,
    /// Toppmarkörerna i visualizern, en per band.
    pub visualizer_peaks: Vec<f32>,
    /// Färgkartan: noterna i rutnätet färgas efter tonhöjd (FL:s VFX Color Mapper).
    pub color_notes: bool,
    /// Zoom i pianorullens rutnät (1,0 = standard). Ctrl + +/− ändrar, Ctrl+0 nollställer.
    pub roll_zoom: f32,
    pub tuner_state: TunerState,
    pub show_dice_generator_modal: bool,
    pub dice_generator_state: DiceGeneratorState,
    pub show_fx_rack_modal: bool,
    pub fx_rack_state: FxRackState,
    pub show_song_structure_modal: bool,
    /// Scenrutnätet på skärmen (Sprint 1, punkt 5): modellen, kvantiseringen och rutan.
    pub launcher: crate::audio::launcher::Launcher,
    pub launch_quantize: crate::audio::launcher::LaunchQuantize,
    pub launcher_section_count: usize,
    pub show_launcher: bool,
    pub song_structure_state: SongStructureState,
    /// Makron (Fas 8.9 steg 2): kedjan över filer, och tillståndet för dess dialog.
    pub show_macros_modal: bool,
    pub macro_state: crate::ui::macros_modal::MacroModalState,
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
    /// Frasbanken (FL): en mapp med .mid-fraser, listade i en modal.
    pub show_phrase_bank: bool,
    pub phrase_folder: Option<std::path::PathBuf>,
    pub phrases: Vec<(String, std::path::PathBuf)>,
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
        let s = current_preset.settings();
        let (waveform, adsr, filter) = (s.waveform, s.adsr, s.filter);
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
            name: "808 Kick".to_string(), icon: "💥".to_string(), color: crate::ui::theme::palette().orange(),
            volume: 0.95, pan: 0.0, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [36; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.3, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(20.0, 0.8),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let snare = ChannelStrip {
            name: "909 Snare".to_string(), icon: "🥁".to_string(), color: crate::ui::theme::palette().cyan(),
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [38; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(45.0, 0.9),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let clap = ChannelStrip {
            name: "Electro Clap".to_string(), icon: "👏".to_string(), color: crate::ui::theme::palette().yellow(),
            volume: 0.80, pan: -0.1, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [39; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(35.0, 0.85),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let hat = ChannelStrip {
            name: "Crisp Hat".to_string(), icon: "⚡".to_string(), color: crate::ui::theme::palette().purple(),
            volume: 0.75, pan: 0.15, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [42; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.2, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(70.0, 0.95),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let open_hat = ChannelStrip {
            name: "Open Hat".to_string(), icon: "🌊".to_string(), color: crate::ui::theme::palette().cyan(),
            volume: 0.70, pan: -0.2, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [46; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.6, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(50.0, 0.5),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let crash = ChannelStrip {
            name: "Cyber Crash".to_string(), icon: "✨".to_string(), color: crate::ui::theme::palette().amber(),
            volume: 0.75, pan: 0.25, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [49; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.7, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(30.0, 0.4),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let synth_lead = ChannelStrip {
            name: "303 Lead".to_string(), icon: "🎹".to_string(), color: crate::ui::theme::palette().green(),
            volume: 0.85, pan: 0.0, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [60; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.5, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
            waveform_preview: make_wave(60.0, 0.3),
            slices: Vec::new(), sample_path: None, pcm_audio: None, sample_base_note: 60,
        };
        let sub_bass = ChannelStrip {
            name: "Sub Bass".to_string(), icon: "🎸".to_string(), color: crate::ui::theme::palette().track_rose(),
            volume: 0.90, pan: 0.0, muted: false, solo: false, steps: [false; crate::ui::app::state::MAX_STEPS], notes: [36; crate::ui::app::state::MAX_STEPS],
            pitch_semitones: 0, pitch_fine_cents: 0.0, sample_start: 0.0, sample_end: 1.0, attack_decay: 0.4, loop_mode: crate::audio::LoopMode::Off, sample_loop_start: 0.0, sample_loop_end: 1.0, ping_pong: false, amp_env: crate::audio::envelope::AdsrParams::identity(), velocity_sensitivity: 1.0, velocity_curve: crate::audio::envelope::VelocityCurve::Linear, filter: crate::audio::filter::SamplerFilter::default(), zones: Vec::new(), is_reverse: false,
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
            color: crate::ui::theme::palette().orange(),
            bars: 1,
            bends: Vec::new(),
            alt_takes: Vec::new(),
            active_take: 0,
            comp: Vec::new(),
            channel_steps: vec![[false; crate::ui::app::state::MAX_STEPS]; 8],
            channel_notes: vec![[60; crate::ui::app::state::MAX_STEPS]; 8],
            piano_roll_grid: [[false; crate::audio::tempo::MAX_STEPS]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat2 = Pattern {
            name: "Pat 2: Lead & Synthesizer".to_string(),
            color: crate::ui::theme::palette().cyan(),
            bars: 1,
            bends: Vec::new(),
            alt_takes: Vec::new(),
            active_take: 0,
            comp: Vec::new(),
            channel_steps: vec![[false; crate::ui::app::state::MAX_STEPS]; 8],
            channel_notes: vec![[60; crate::ui::app::state::MAX_STEPS]; 8],
            piano_roll_grid: [[false; crate::audio::tempo::MAX_STEPS]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat3 = Pattern {
            name: "Pat 3: Synth Arp".to_string(),
            color: crate::ui::theme::palette().green(),
            bars: 1,
            bends: Vec::new(),
            alt_takes: Vec::new(),
            active_take: 0,
            comp: Vec::new(),
            channel_steps: vec![[false; crate::ui::app::state::MAX_STEPS]; 8],
            channel_notes: vec![[60; crate::ui::app::state::MAX_STEPS]; 8],
            piano_roll_grid: [[false; crate::audio::tempo::MAX_STEPS]; 24],
            take: crate::midi_take::Take::default(),
        };

        let pat4 = Pattern {
            name: "Pat 4: Deep Bass".to_string(),
            color: crate::ui::theme::palette().track_rose(),
            bars: 1,
            bends: Vec::new(),
            alt_takes: Vec::new(),
            active_take: 0,
            comp: Vec::new(),
            channel_steps: vec![[false; crate::ui::app::state::MAX_STEPS]; 8],
            channel_notes: vec![[60; crate::ui::app::state::MAX_STEPS]; 8],
            piano_roll_grid: [[false; crate::audio::tempo::MAX_STEPS]; 24],
            take: crate::midi_take::Take::default(),
        };

        let patterns = vec![pat1, pat2, pat3, pat4];

        // 1. Drums & Beat Track
        let mut t_drums = PlaylistTrack::new(crate::i18n::t("🥁 Trummor & Beat").to_string(), "🥁", TrackKind::Drums, crate::ui::theme::palette().orange());
        t_drums.volume = 0.90;

        // 2. Synth & Melody Track
        let mut t_synth = PlaylistTrack::new(crate::i18n::t("🎹 Synt & Ackord").to_string(), "🎹", TrackKind::SynthLead, crate::ui::theme::palette().track_blue());
        t_synth.volume = 0.85;

        // 3. Bassline Track
        let mut t_bass = PlaylistTrack::new(crate::i18n::t("🎸 Baslinje").to_string(), "🎸", TrackKind::Bassline, crate::ui::theme::palette().green());
        t_bass.volume = 0.90;

        // 4. FX & Transitions
        let mut t_fx = PlaylistTrack::new(crate::i18n::t("✨ FX & Ljudeffekter").to_string(), "✨", TrackKind::Fx, crate::ui::theme::palette().track_rose());
        t_fx.volume = 0.80;

        // 5. Mic Vocal Track (Armed and Ready for Recording - placed at the END)
        let mut t_mic = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, crate::ui::theme::palette().text());
        t_mic.volume = 1.0;
        t_mic.is_rec_armed = true;

        let playlist_tracks = vec![t_drums, t_synth, t_bass, t_fx, t_mic];
        let selected_pattern = 0;
        let initial_channels = channels;
        let initial_grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];

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
            input_autodetect_done: false,
            rate_mismatch_reported: false,
            monitor_level_sent: None,
            stem_import_progress: std::sync::Arc::new(std::sync::Mutex::new(StemImportProgress::default())),
            project_load_progress: std::sync::Arc::new(std::sync::Mutex::new(ProjectLoadProgress::default())),
            is_playing: false,
            bpm: 120.0,
            tempo_points: Vec::new(),
            swing: 0.0,
            current_step: 0,
            note_capture: crate::note_capture::NoteCapture::default(),
            score_log: crate::note_capture::NoteCapture::with_limit(2048),
            score_clock: None,
            capture_clock: None,
            song_bar: 0,
            song_step_in_bar: 0,
            loop_start_bar: 0,
            loop_enabled: false,
            loop_end_bar: 16,
            song_markers: Vec::new(),
            last_step_time: Instant::now(),
            song_time: 0.0,
            pattern_mode: false,
            follow: None,
            follow_bars: 0.0,
            note_filter: crate::midi_take::NoteFilter::default(),
            note_filter_hits: None,
            note_probability: 1.0,
            compare: crate::ui::app::compare::Compare::default(),
            pending_swap_source: None,
            pending_beepmap: false,
            project_notes: String::new(),
            show_notes_modal: false,
            riff_scale: crate::riff::Scale::Minor,
            track_groups: Vec::new(),
            view_mode: ViewMode::PlaylistArranger,
            docked_panel: None,
            take_groove_amount: 1.0,
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
            gate_gains: [1.0; 16],
            gate_on: false,
            gate_smooth: 0.0,
            fm: crate::audio::command::FmParams::default(),
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
            piano_roll_sel: [[false; crate::ui::app::state::MAX_STEPS]; 24],
            piano_roll_snap_to_scale: false,
            piano_roll_poly_mode: false,
            chord_stamp: 0,
            step_velocities: [0.85; crate::audio::tempo::MAX_STEPS],
            piano_roll_quiet_threshold: 0.3,
            harmonics: crate::audio::synth::HARMONICS_DEFAULT,
            six_op: crate::audio::synth::SixOpParams::default(),
            six_env_target: 0,
            partial_env: [0.002, 0.6, 0.0],
            partial_env_tilt: 1.0,
            partial_env_on: false,
            partial_env_target: 0,
            partial_env_overrides: Vec::new(),
            comp_step: 0.0,
            comp_bar: 0,
            formant: crate::audio::synth::SpectralFormant::default(),
            sweep: crate::audio::synth::SpectralSweep::default(),
            sub_level: 0.0,
            filter_drive: 0.0,
            filter_key_track: 0.0,
            glide_time: 0.0,
            unison_voices: 1,
            unison_detune_cents: 12.0,
            warp_mode: crate::audio::warp::WarpMode::Complex,
            groove_pool: Vec::new(),
            groove_library: crate::groove::GrooveLibrary::default(),
            groove_folder: None,
            flex_presets: crate::audio::flex::store_path()
                .map(|p| crate::audio::flex::load_all(&p))
                .unwrap_or_default(),
            mod_slots: Vec::new(),
            show_mod_matrix: false,
            mod_dirty: false,
            model_download: std::sync::Arc::new(std::sync::Mutex::new((false, 0.0, String::new()))),
            audio_output_buses: crate::audio::engine::AudioSettings::load().output_buses,
            song_chords: Vec::new(),
            show_chord_track: false,
            arranger_chain: Vec::new(),
            arranger_chain_active: false,
            show_arranger_chain: false,
            rule_field: 0,
            rule_cmp: 0,
            rule_value: 0.3,
            rule_action: 0,
            rule_action_value: 12.0,
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
            // **Med databasen från förra gången** (2026-09-15): undviker en full skanning vid
            // varje start, och berättar i statusraden om filen inte gick att läsa.
            plugin_manager: PluginManager::with_cached_database(),
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
            midi_map: crate::audio::key_mapper::KeyMap::default(),
            phrase_preview: None,
            midi_out: None,
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
            video: crate::ui::video_view::VideoState::default(),
            stem_six: false,
            script_text: String::new(),
            take_strum: 0.08,
            take_strum_down: false,
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
            automation_target: AutomationTarget::Track(AutomationParam::Volume),
            plugin_automation: Vec::new(),
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
            show_big_clock: false,
            show_visualizer: false,
            show_script_modal: false,
            show_dance: false,
            show_takes_modal: false,
            control_room: crate::audio::control_room::ControlRoom::default(),
            cue_listen: crate::audio::control_room::CueListen::Main,
            talkback: false,
            vocoder_amount: 0.0,
            script_source: crate::ui::app::script::DEFAULT_SOURCE.to_string(),
            script_param: 0,
            visualizer_peaks: Vec::new(),
            color_notes: false,
            roll_zoom: 1.0,
            tuner_state: TunerState::default(),
            show_dice_generator_modal: false,
            dice_generator_state: DiceGeneratorState::default(),
            show_fx_rack_modal: false,
            fx_rack_state: FxRackState::default(),
            show_song_structure_modal: false,
            launcher: crate::audio::launcher::Launcher::new(1, 4, 4),
            launch_quantize: crate::audio::launcher::LaunchQuantize::Bar,
            launcher_section_count: 0,
            show_launcher: false,
            song_structure_state: SongStructureState::default(),
            show_macros_modal: false,
            macro_state: crate::ui::macros_modal::MacroModalState::default(),
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
            show_phrase_bank: false,
            phrase_folder: None,
            phrases: Vec::new(),
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

        self.poll_background_work(ctx);

        self.advance_phrase_preview();

        self.tick_frame(ctx);

        self.handle_input(ctx);

        self.handle_dropped_files(ctx);

        self.draw_chrome(ctx);

        self.draw_workspace(ctx);

        self.render_all_modals(ctx);

        self.screenshot_tick(ctx);

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

