//! Status: byggs — den stora ytan (UI + tillstånd). Kvar här: 8.7:s slice-UI, 8.10-vyns sträckt-märke, 8.4-samplern, 8.2:s fyra visningsställen
//! Rör inte: två sessioner har krockat i den här filen; kolla `git status` och mtime före varje skrivning
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

/// Assigns a library sample to a Channel Rack channel strip. Loads the real
/// PCM into memory so step playback triggers the actual WAV instead of the
/// built-in synthesizer drum voices.
///
/// Lämnar `false` och rör **ingenting** när filen inte går att läsa. Förut tog
/// kanalen samplen ändå: namn, färg, steg och bibliotekets grova vågform flyttades
/// in medan `pcm_audio` blev `None`, så kanalen såg ut att ha ett eget sample och
/// spelade den inbyggda synten i stället. Det är samma familj som de tysta
/// klippen, i kanalracket i stället för på tidslinjen.
fn assign_library_sample_to_channel(ch: &mut ChannelStrip, item: &LibrarySampleItem) -> bool {
    // Ljudet läses FÖRST. En guard som ligger efter ändringarna lämnar en
    // halvflyttad kanal efter sig — den ser ut att ha ett sample den inte har.
    let pcm = match item.file_path.as_deref() {
        Some(path) => match load_sample_pcm_arcs(path) {
            Some(pcm) => Some(pcm),
            None => return false,
        },
        None => None,
    };
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
    ch.sample_path = item.file_path.clone();
    ch.pcm_audio = pcm;
    true
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
            // Går filen inte att läsa tas samplen inte alls (se funktionen) — då
            // behåller kanalen sin inbyggda röst i stället för ett namn utan ljud.
            let _ = assign_library_sample_to_channel(&mut channels[ch_idx], item);
        }
    }
}

/// Builds the audio command that plays a channel's loaded WAV sample for one
/// sequencer step. Returns None when the channel has no PCM loaded, in which
/// case the caller falls back to the built-in synthesizer.
fn channel_sample_trigger_command(
    ch: &ChannelStrip,
    channel: usize,
    note: u8,
    velocity: f32,
    hold_secs: f32,
) -> Option<AudioCommand> {
    let (start01, end01) = crate::audio::onset::window_for_note(
        &ch.slices,
        ch.sample_base_note,
        note,
        (ch.sample_start, ch.sample_end),
    );
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
        // Slicekartan (8.7): noten **är** adressen — `bas + i` spelar slice `i`,
        // och samma regel används i exporten. Utan den hade en kanal med slicar
        // låtit olika i filen och i högtalarna.
        start01,
        end01,
        // Samplern (Fas 8.4): kanalen (för not-av), loopläget med sina punkter,
        // riktningen, envelopen och notens längd — stegets egen längd, så att en
        // lopande not håller lika länge som steget varar.
        channel: channel as u32,
        loop_mode: ch.loop_mode,
        loop_start01: ch.sample_loop_start,
        loop_end01: ch.sample_loop_end,
        ping_pong: ch.ping_pong,
        amp_env: ch.amp_env,
        hold_secs,
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

    /// Öppnar ett bibliotekssample i Sångstudion som en tagning.
    ///
    /// **Ingen påhittad ton.** Vägen byggde förut en syntetisk sinuston (två
    /// sekunder, ur samplens `default_note`) när filen inte gick att läsa, och
    /// öppnade Sångstudion med den. En mp3 — som appen inte kan avkoda — blev
    /// alltså en påkittad tagning i stället för ett besked, och den som lyssnade
    /// hörde något som varken var samplen eller tystnad. Nu gäller samma regel som
    /// `import_audio_file_as_track`: säg det och avbryt.
    ///
    /// Provspelningen får också **filens egen** samplerate. Förut sades 44100
    /// oavsett vad filen innehöll, så en tagning ur ett 48 kHz-sample spelades i
    /// fel hastighet.
    pub fn open_sample_in_vocal_studio(&mut self, item: &LibrarySampleItem) {
        let Some(ref path) = item.file_path else {
            self.status_message = crate::tstatus!(
                "⚠ '{}' har ingen ljudfil — kan inte öppnas i Sångstudion",
                item.name
            );
            return;
        };
        let Some((pcm, _, sample_rate)) = load_audio_or_report(path) else {
            self.status_message = crate::tstatus!(
                "⚠ Kunde inte läsa '{}' — öppnas inte i Sångstudion (mp3 stöds inte, konvertera till wav)",
                item.name
            );
            return;
        };
        let new_take_idx = self.vocal_studio.load_sample_or_region_as_take(&item.name, pcm, sample_rate, item.color);
        self.view_mode = ViewMode::VocalStudio;
        self.status_message = crate::tstatus!("🎙 Öppnade sample '{}' i Sångstudion (Tagning {}) för isolerad provspelning & formning!", item.name, new_take_idx + 1);
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
        // Tempot i filen används bara som förslag när projektet står kvar på sin
        // ursprungs-BPM; annars vore en import en tyst tempoändring. Samma regel
        // gäller filens **tempobyten** (Fas 8.2 steg 3), och regeln ligger som en
        // ren funktion i `tempo`, så att den kan prövas utan fönster.
        let from_file = self.bpm_source_is_file();
        let imported_points = if from_file {
            crate::audio::tempo::tempo_points_for_import(&parsed.tempo_events, self.bpm)
        } else {
            None
        };
        let mut tempo_note = String::new();
        match imported_points {
            Some(points) => {
                let count = points.len();
                self.tempo_points = points;
                // `bpm` speglar kartans första punkt (Fas 8.2) — samma regel som
                // när ett tempobyte sätts för hand.
                self.bpm = self.tempo_map().bpm_at(0.0);
                if count > 1 {
                    tempo_note = crate::tstatus!(" — och {} tempopunkter ur filen", count);
                }
            }
            None => {
                if from_file {
                    if let Some(bpm) = parsed.bpm {
                        self.bpm = bpm.clamp(40.0, 260.0);
                    }
                } else if parsed.tempo_events.len() > 1 {
                    // Filens byten togs inte in. Att tiga om det vore att tappa
                    // dem utan att säga till.
                    tempo_note = crate::tstatus!(
                        " — filens {} tempobyten togs inte in (projektet har eget tempo)",
                        parsed.tempo_events.len()
                    );
                }
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
        if !tempo_note.is_empty() {
            self.status_message = format!("{}{}", self.status_message, tempo_note);
        }
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

    const SCOPE_HISTORY_MAX: usize = 2048;

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

    /// Logotypen som textur, laddad **första gången den behövs**. `None` när bilden inte gick
    /// att avkoda — anroparen ritar då ingen bild, aldrig en tom ruta.
    fn logo_texture_for(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.logo_texture.is_none()
            && let Some(img) = sonix_logo()
        {
            self.logo_texture = Some(ctx.load_texture(
                "sonix-logo",
                img.clone(),
                egui::TextureOptions::LINEAR,
            ));
        }
        self.logo_texture.clone()
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

    /// Lägger de separerade stämmorna som riktiga tidslinjespår.
    ///
    /// **Stämmorna skrivs till disk först.** Klippet får inte skapas utan sitt
    /// ljud — samma regel som `import_audio_file_as_track` och de övriga
    /// 8.5-vägarna. Förut pekade varje stämregion på **originalfilen** medan
    /// vågformen kom från stämman i minnet. Var originalet en mp3, som appen inte
    /// kan avkoda, blev klippet tyst men ritades som om det hade ljud — och
    /// tystnaden kom tillbaka varje gång projektet öppnades igen. Nu skrivs varje
    /// stämma som en 32-bitars WAV i projektets egen materialmapp, **läses
    /// tillbaka** (så att en trasig skrivning upptäcks här i stället för i en tyst
    /// uppspelning), och regionens `source_path` och vågform kommer båda ur den
    /// filen.
    pub fn export_separated_stems(&mut self) {
        if self.stem_project.stem_audio.is_empty() {
            self.status_message = crate::i18n::t("⚠ Ingen separerad mix att exportera.").to_string();
            return;
        }
        // Antalet punkter den grova översikten ritades med förut (512).
        const WAVEFORM_POINTS: usize = 512;
        let dir = crate::paths::paths()
            .project_assets_dir(&self.project_name)
            .join("Stems");
        let base = stem_base_name(
            self.stem_project.source_path.as_deref(),
            &self.stem_project.track_title,
        );
        let paths = match crate::audio::stem_separator::write_stems_and_read_envelopes(
            &dir,
            &base,
            &self.stem_project.stem_audio,
            self.stem_project.sample_rate,
            WAVEFORM_POINTS,
        ) {
            Ok(written) => written,
            Err(e) => {
                self.status_message = crate::tstatus!(
                    "⚠ Kunde inte skriva stämmorna till disk: {} — inga klipp skapas",
                    e
                );
                return;
            }
        };
        // Vågformen kommer ur filerna, i samma ordning som stämmorna. Det är
        // filen som är sanningen om vad klippet kommer att spela — inte den
        // grova översikt som räckte förut.
        let mut envelopes: Vec<Vec<f32>> = Vec::with_capacity(paths.len());
        let mut files: Vec<String> = Vec::with_capacity(paths.len());
        for (path, envelope) in paths {
            files.push(path.to_string_lossy().into_owned());
            envelopes.push(envelope);
        }

        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let bars =
            (tempo.bars_for_secs_at(0.0, self.stem_project.duration_seconds as f64) as f32)
                .max(1.0);
        let kinds = [TrackKind::VocalAudio, TrackKind::Drums, TrackKind::Bassline, TrackKind::CustomAudio];
        let mut new_tracks = Vec::new();
        // Stämman, kanalens inställningar och filen hör ihop tre och tre — gå
        // aldrig utanför någon av listorna.
        let count = self
            .stem_project
            .stems
            .len()
            .min(self.stem_project.stem_audio.len())
            .min(files.len());
        for i in 0..count {
            let ch = &self.stem_project.stems[i];
            let audio = &self.stem_project.stem_audio[i];
            let region = AudioRegion {
                source_bpm: self.bpm, // stämmorna byggdes mot projektets tempo (8.10)
                tape: false,
                id: i + 1,
                name: ch.stem_type.name().to_string(),
                start_bar: 0.0,
                length_bars: bars,
                sample_offset_sec: 0.0,
                source_path: Some(files[i].clone()),
                waveform_peaks: envelopes[i].clone(),
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
        let added = new_tracks.len();
        self.playlist_tracks.extend(new_tracks);
        self.sync_all_stems_to_engine();
        self.status_message = crate::tstatus!(
            "📥 Lade in {} stämspår i arrangeraren (som wav-filer i {})",
            added,
            dir.display()
        );
    }

    /// Tak för hur många steg en och samma bildruta får ta igen (Fas 8.13b).
    ///
    /// En lång paus ska inte avfyra femtio noter på en bildruta; att tappa steg är
    /// mindre illa än att spränga låten.
    const MAX_STEPS_PER_FRAME: usize = 8;

    pub fn export_wav(&mut self) {
        // Snabbexport öppnar exportdialogen (fullt projekt: format, mapp &
        // ren metadata – ingen hårdkodad sökväg kvar).
        self.open_export_modal();
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
                                    // Samma tre fall som `audition_library_sample`: ingen fil →
                                    // appens egen syntröst; fil som inte går att läsa → ett besked,
                                    // aldrig en syntetisk trumma i samplens ställe.
                                    let mut handled = false;
                                    if let Some(ref path) = item.file_path {
                                        match load_audio_or_report(path) {
                                            Some((l, r, sr)) => {
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
                                                handled = true;
                                            }
                                            None => {
                                                self.status_message = crate::tstatus!(
                                                    "⚠ Kunde inte provspela '{}' — filen går inte att läsa: {} (mp3 stöds inte, konvertera till wav)",
                                                    item.name,
                                                    path
                                                );
                                                handled = true;
                                            }
                                        }
                                    }
                                    if !handled {
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
                        let (bpm_rect, _) =
                            ui.allocate_exact_size(Vec2::new(72.0, 20.0), Sense::hover());
                        ui.painter().rect_filled(
                            bpm_rect,
                            Rounding::same(2.0),
                            Color32::from_rgb(26, 32, 42),
                        );
                        // Skrivbart OCH dragbart — samma kontroll som tempodialogen
                        // använder. Att bara kunna dra tvingade fram skrubbande tills
                        // det blev "nästan" rätt; nu går det att skriva 48.2 exakt.
                        // DragValue visar en textmarkör vid klick och tar siffror,
                        // Enter, och Escape (avbryter) — beteendet kommer från egui,
                        // inte från en egen tolkning av tangenttryck.
                        // Hjälptexten säger vad som händer med LJUDET när tempot
                        // ändras (Fas 8.10) — annars är det en kontroll som ser ut
                        // att bara styra klockan. Antalet räknas, inte cachas.
                        let with_tempo = self.clips_with_source_tempo();
                        let tempo_hover = if with_tempo == 0 {
                            crate::i18n::t(
                                "Klippens tempo: inga klipp har ett känt inspelningstempo ännu, så ljudet rörs inte när du ändrar tempot.",
                            )
                            .to_string()
                        } else {
                            crate::tstatus!(
                                "Klippens tempo: {} klipp har ett känt inspelningstempo och följer med när du ändrar tempot — med bevarad tonhöjd (Fas 8.10 steg 2). Enstaka klipp kan i stället sättas i bandspelarläge i klippmenyn. Klipp med okänt tempo rörs inte.",
                                with_tempo
                            )
                        };
                        let tempo_field = ui.put(
                            bpm_rect.shrink2(Vec2::new(4.0, 2.0)),
                            egui::DragValue::new(&mut self.bpm)
                                .speed(0.5)
                                .range(40.0..=280.0)
                                .suffix(" BPM"),
                        );
                        tempo_field.on_hover_text(tempo_hover);

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
                        // Tonarten (Fas 8.11): samma tabell och samma index som piano
                        // rollen. Förut hade de här menyerna egna listor, och "Dorian"
                        // här blev "Moll" där; fyra av tolv grundtoner visade dessutom
                        // fel namn (en tretton-namnlista för tolv tonhöjder).
                        let cur_root = crate::audio::scale::root_name(self.song_key_root);
                        egui::ComboBox::from_id_salt("arr_key_root")
                            .selected_text(cur_root)
                            .width(40.0)
                            .show_ui(ui, |ui| {
                                for (idx, &r_name) in crate::audio::scale::ROOT_NAMES.iter().enumerate() {
                                    if ui.selectable_label(self.song_key_root as usize == idx, r_name).clicked() {
                                        self.song_key_root = idx as u8;
                                        self.status_message = crate::tstatus!(
                                            "🎼 Tonart: {}",
                                            crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                                        );
                                    }
                                }
                            });

                        let cur_sc = crate::i18n::t(crate::audio::scale::scale_at(self.song_key_scale).name);
                        egui::ComboBox::from_id_salt("arr_key_scale")
                            .selected_text(cur_sc)
                            .width(104.0)
                            .show_ui(ui, |ui| {
                                for (s_idx, sc) in crate::audio::scale::SCALES.iter().enumerate() {
                                    if ui.selectable_label(self.song_key_scale == s_idx, crate::i18n::t(sc.name)).clicked() {
                                        self.song_key_scale = s_idx;
                                        self.status_message = crate::tstatus!(
                                            "🎼 Tonart: {}",
                                            crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                                        );
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
            // **Tidsaxeln går genom tempokartan** (Fas 8.2 steg 3). X är fortfarande takter
            // (`bar_w` per takt); varje gång en *sekund* behövs frågas kartan i stället för att
            // multiplicera med ett enda tempo. Med ett tempo ger det samma tal som förut, och
            // efter ett tempobyte är det här siffran är rätt — annars är varje tid efter bytet
            // tyst fel, och ett fel i visningen syns (en kloss på fel plats).
            let tempo = self.tempo_map();

            // **De fyra frågorna `sec_per_bar` svarade på — men med fel svar efter ett
            // tempobyte** (Fas 8.2 steg 3). En *plats* i tiden, en *längd*, en *omvändning*
            // och en *lokal* sekunder-per-takt är olika frågor; en enda skalär kunde bara
            // svara rätt på dem så länge tempot var konstant. Med ett tempo ger de samma tal
            // som förut, bit för bit.
            let secs_at = |bar: f32| tempo.secs_at_bar(bar as f64) as f32;
            let secs_len = |from_bar: f32, bars: f32| tempo.secs_for_bars_at(from_bar as f64, bars as f64) as f32;
            let bars_at = |secs: f32| tempo.bar_at_secs(secs as f64) as f32;
            let sec_per_bar_at = |bar: f32| tempo.secs_per_bar_at(bar as f64) as f32;
            let bars_len = |from_bar: f32, secs: f32| tempo.bars_for_secs_at(from_bar as f64, secs as f64) as f32;

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
                                self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(tempo.secs_at_bar(reg.start_bar as f64) as f32), format_time_hundredths(tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32));
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
                                let bar_time_sec = tempo.secs_at_bar(bar_idx as f64) as f32;

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
                                    // Inne i en takt är tempot konstant, så delstrecken räknas
                                    // med **den taktens** sekunder per takt — inte projektets.
                                    let bar_secs = tempo.secs_per_bar_at(bar_idx as f64) as f32;
                                    let tenths = (bar_secs * 10.0) as usize;
                                    for t in 1..tenths {
                                        let tx = bar_start_x + (t as f32 * 0.10) * (bar_w / bar_secs);
                                        if tx < bar_start_x + bar_w - 2.0 {
                                            ui.painter().line_segment(
                                                [Pos2::new(tx, ruler_rect.min.y + 25.0), Pos2::new(tx, ruler_rect.max.y)],
                                                Stroke::new(0.5_f32, Color32::from_rgb(70, 80, 100)),
                                            );
                                        }
                                    }
                                }
                            }

                            // Tempobyten i linjalen (Fas 8.2 — synliga sedan 8.12).
                            //
                            // Bytet fanns bara i ⏱ Tempokarta-listan; i linjalen syntes
                            // ingenting, så en låt med tre tempon såg ut som en med ett.
                            // Strecket gäller från sin takt och framåt, och BPM står
                            // strax till VÄNSTER om strecket — taktnumret står till höger
                            // om samma linje, så de två krockar inte.
                            //
                            // Ritas efter tick-varvet: en taktlinje ligger på samma x,
                            // och ritades bytet först skulle den grå linjen lägga sig
                            // över markeringen.
                            for point in &self.tempo_points {
                                let px = ruler_rect.min.x + point.start_bar as f32 * bar_w;
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(px, ruler_rect.min.y),
                                        Pos2::new(px, ruler_rect.max.y),
                                    ],
                                    Stroke::new(2.0_f32, Theme::FL_YELLOW),
                                );
                                ui.painter().text(
                                    Pos2::new(px - 3.0, ruler_rect.min.y + 6.0),
                                    egui::Align2::RIGHT_CENTER,
                                    format!("⏱{:.0}", point.bpm),
                                    egui::FontId::proportional(9.5),
                                    Theme::FL_YELLOW,
                                );
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
                                            // **Snäppet sker i takter** (Fas 8.2): ett
                                            // sextondelssteg är en *plats i takten*, inte ett antal
                                            // sekunder — och sekunder-per-takt är inte ett tal över
                                            // ett tempobyte. Hundradelarna är undantaget: där *är*
                                            // sekunden enheten, och då får kartan svara.
                                            let raw_drop_bar = ((mouse_pos.x - lane_rect.min.x) / bar_w).max(0.0);
                                            let drop_bar = if self.timeline_snap_mode == TimeSnapMode::FreeHundredth {
                                                // Hundradelarna är undantaget: där är sekunden enheten.
                                                let raw_sec = tempo.secs_at_bar(raw_drop_bar as f64) as f32;
                                                let sec = (raw_sec * 100.0).round() / 100.0;
                                                tempo.bar_at_secs(sec as f64) as f32
                                            } else {
                                                snap_bar(self.timeline_snap_mode, raw_drop_bar)
                                            };
                                            let item_dur_sec: f32 = 2.0;
                                            let item_dur_bars =
                                                (item_dur_sec / tempo.secs_per_bar_at(drop_bar as f64) as f32).max(0.25);
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
                                                let r_start_str = format_time_hundredths(tempo.secs_at_bar(region.start_bar as f64) as f32);
                                                let r_len_str = format_time_hundredths(tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32);
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
                                                    secs_len(region.start_bar, region.loop_length_bars)
                                                } else {
                                                    if region.length_bars > 0.001 { secs_len(region.start_bar, region.length_bars) } else { 1.0 }
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
                                                    // Klippet följer tempot (Fas 8.10): källan en
                                                    // region täcker är längre än dess tid på
                                                    // tidslinjen när den spelades in i ett lägre
                                                    // tempo. Utan faktorn här skulle vågformen visa
                                                    // ett annat stycke än det som hörs — samma sorts
                                                    // lögn som 8.3 stängde.
                                                    // Tempot **där klippet ligger** — samma regel
                                                    // som de två andra ställena (8.10 punkt 1), så
                                                    // vyn och ljudet inte kan visa olika tempobyten.
                                                    let region_rate = stretch_ratio_for(
                                                        region.source_bpm,
                                                        self.tempo_map()
                                                            .bpm_at(region.start_bar as f64),
                                                    );
                                                    let total_samples = region_source_span_samples(
                                                        region_secs,
                                                        region_rate,
                                                        sr,
                                                    );
                                                    let cols =
                                                        ((draw_end_x - draw_start_x).max(1.0)).ceil() as usize;
                                                    // En slingad kloss upprepar ett kortare
                                                    // stycke: då följer bildpunkterna
                                                    // upprepningen, inte en sammanhängande
                                                    // sampelmängd.
                                                    let loop_samples = region_source_span_samples(
                                                        loop_sec,
                                                        region_rate,
                                                        sr,
                                                    );
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
                                                    // dB-skalan (Fas 8.4): utan den är
                                                    // svaga partier i praktiken osynliga.
                                                    // Audacity: örat behöver −18 dB för
                                                    // halva styrkan, linjärt räcker −6.
                                                    const WAVE_SCALE: crate::audio::waveform::AmplitudeScale =
                                                        crate::audio::waveform::AmplitudeScale::Decibel;
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
                                                                let top = mid_y
                                                                    - (crate::audio::waveform::amplitude_curve(*hi, WAVE_SCALE)
                                                                        * half
                                                                        * vol)
                                                                        .min(half);
                                                                let bot = mid_y
                                                                    - (crate::audio::waveform::amplitude_curve(*lo, WAVE_SCALE)
                                                                        * half
                                                                        * vol)
                                                                        .max(-half);
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
                                                                    - (crate::audio::waveform::amplitude_curve(v, WAVE_SCALE)
                                                                        * half
                                                                        * vol)
                                                                        .clamp(-half, half);
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
                                                        let rel_time = secs_at(region.start_bar + rel_x / bar_w);

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
                                                        let reg_start_sec = tempo.secs_at_bar(region.start_bar as f64) as f32;
                                                        let total_reg_sec =
                                                            tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32;
                                                        while div_sec < total_reg_sec - 0.05 {
                                                            // Strecket ligger på en *tid* i regionen; x räknas
                                                            // genom kartan, samma väg som allt annat här.
                                                            let div_bar = tempo.bar_at_secs((reg_start_sec + div_sec) as f64) as f32
                                                                - region.start_bar as f32;
                                                            let div_x = rx_start + div_bar * bar_w;
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
                                            if let Some(mouse_pos) = mouse_pos_opt
                                                && let Some(r_i) = region_under_x(
                                                    &regions_snapshot,
                                                    mouse_pos.x,
                                                    lane_rect.min.x,
                                                    bar_w,
                                                )
                                            {
                                                let r = &regions_snapshot[r_i];
                                                let rx_start = lane_rect.min.x + r.start_bar * bar_w;
                                                let rx_end = rx_start + r.length_bars * bar_w;
                                                let init_loop_bars = if r.loop_length_bars > 0.001 {
                                                    r.loop_length_bars
                                                } else {
                                                    r.length_bars
                                                };
                                                let mode = if mouse_pos.x <= rx_start + 16.0
                                                    || (mouse_pos.x - rx_start).abs() <= 10.0
                                                {
                                                    RegionDragMode::TrimStart
                                                } else if mouse_pos.x >= rx_end - 16.0
                                                    || (mouse_pos.x - rx_end).abs() <= 10.0
                                                {
                                                    RegionDragMode::TrimEnd
                                                } else {
                                                    RegionDragMode::Move
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
                                                        TimeSnapMode::FreeHundredth => (0.01 / sec_per_bar_at(drag.initial_start_bar)).max(0.001),
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
                                                                self.status_message = crate::tstatus!("↔ Flyttar '{}' till takt {:.2} (⏱ {})", reg.name, new_start + 1.0, format_time_hundredths(secs_at(new_start)));
                                                            }
                                                            RegionDragMode::TrimStart => {
                                                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                                let right_edge = drag.initial_start_bar + drag.initial_length_bars;
                                                                let max_expand_sec = drag.initial_sample_offset_sec;
                                                                let orig_sample_start = (drag.initial_start_bar - (max_expand_sec / sec_per_bar_at(drag.initial_start_bar))).max(0.0);
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
                                                                let new_offset = (drag.initial_sample_offset_sec + secs_len(drag.initial_start_bar, new_start - drag.initial_start_bar)).max(0.0);
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
                                                                        self.status_message = crate::tstatus!("🧲 Loop-snap: '{}' loopad exakt {:.0}x ({} takter, ⏱ {})", reg.name, reps.round(), new_len, format_time_hundredths(secs_len(reg.start_bar, new_len)));
                                                                    } else {
                                                                        self.status_message = crate::tstatus!("▶ Loopar '{}': {:.2} takter ({:.1}x repetitioner, ⏱ {})", reg.name, new_len, reps, format_time_hundredths(secs_len(reg.start_bar, new_len)));
                                                                    }
                                                                } else {
                                                                    self.status_message = crate::tstatus!("▶ Längd för '{}': {:.2} takter (⏱ {})", reg.name, new_len, format_time_hundredths(secs_len(reg.start_bar, new_len)));
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

                                // **Högerklick väljer klippet under pekaren** (Fas 8.15).
                                //
                                // Menyn nedan gäller det *valda* klippet, och utan det här
                                // kunde "Radera region" träffa ett annat klipp än det man
                                // pekade på: valet var det man senast vänsterklickade. Samma
                                // regel som dragstarten (`region_under_x`), så de två dörrarna
                                // inte kan välja olika klipp.
                                //
                                // Klickar man utanför ett klipp lämnas valet orört — rubriken
                                // i menyn (`🎵 namn`) visar vilket klipp posterna gäller.
                                if lane_resp.secondary_clicked()
                                    && let Some(mouse_pos) = lane_resp.hover_pos()
                                {
                                    // Samma lista som menyn nedan läser — en källa.
                                    let hit = region_under_x(
                                        &self.playlist_tracks[t_idx].regions,
                                        mouse_pos.x,
                                        lane_rect.min.x,
                                        bar_w,
                                    );
                                    if let Some(r_i) = hit {
                                        self.selected_audio_region = Some((t_idx, r_i));
                                        self.selected_timeline_track = t_idx;
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
                                            if ui
                                                .button(crate::i18n::t(
                                                    "🔍 Hitta första slaget (mät i filen)",
                                                ))
                                                .on_hover_text(crate::i18n::t(
                                                    "Mäter var ljudet börjar i klippets fil och sätter det som klippets början — samma regel som \"Sätt takt 1 här\", men talet kommer ur en mätning i stället för ur spelhuvudets position. **Hela stämgruppen följer med**: alla klipp som börjar på samma takt flyttas lika mycket i tid, så att stämmorna inte hamnar ur fas (Ctrl+Z tar tillbaka allt i ett steg). Ett tyst parti i början (eller en stämma som inte hörs inom 20 s) ger inget svar; då händer ingenting.",
                                                ))
                                                .clicked()
                                            {
                                                self.align_clip_to_first_beat();
                                                ui.close_menu();
                                            }
                                            if ui
                                                .button(crate::i18n::t(
                                                    "🎯 Sätt takt 1 här (ljudet under spelhuvudet blir klippets början)",
                                                ))
                                                .on_hover_text(crate::i18n::t(
                                                    "Kapar början av klippet så att slaget under spelhuvudet hamnar på rutnätet. Högerkanten står still, och **hela stämgruppen följer med** — alla klipp som börjar på samma takt flyttas lika mycket i tid, annars hamnar stämman ur fas med sina syskon (Ctrl+Z tar tillbaka allt i ett steg). Klippet flyttas inte.",
                                                ))
                                                .clicked()
                                            {
                                                self.set_beat_one_at_playhead();
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
                                    let live_e_bar = bars_at(self.song_time).max(live_s_bar + 0.05);
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
                                        format!("🔴 SPELAR IN... [⏱ {}]", format_time_hundredths(secs_len(live_s_bar, live_e_bar - live_s_bar))),
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
                                        self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(tempo.secs_at_bar(reg.start_bar as f64) as f32), format_time_hundredths(tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32));
                                    }
                                }

                                // Execute Slice with Hundredth-Second Accuracy
                                if let Some((r_idx, click_bar)) = split_action {
                                    let mut performed_split = false;
                                    let mut cut_feedback_msg = String::new();

                                    if r_idx < self.playlist_tracks[t_idx].regions.len() {
                                        let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
                                        let raw_cut_sec = secs_at(click_bar);

                                        // **Snäppet sker i takter** (Fas 8.2), samma regel som
                                        // i tidlinjen: klippet delas på en *plats i takten*, och
                                        // sekunden hämtas ur kartan efteråt.
                                        let cut_bar = if self.timeline_snap_mode == TimeSnapMode::FreeHundredth {
                                            let sec = (raw_cut_sec * 100.0).round() / 100.0;
                                            bars_at(sec)
                                        } else {
                                            snap_bar(self.timeline_snap_mode, click_bar)
                                        };
                                        let cut_sec = secs_at(cut_bar);

                                        let orig_start_sec = secs_at(orig.start_bar);
                                        let orig_len_sec = secs_len(orig.start_bar, orig.length_bars);
                                        let split_offset_sec = cut_sec - orig_start_sec;

                                        if split_offset_sec > 0.05 && split_offset_sec < orig_len_sec - 0.05 {
                                            let split_offset_bar = bars_len(orig.start_bar, split_offset_sec);
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
                                                source_bpm: orig.source_bpm, // halvan ärver klippets källa
                                                tape: false,
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
                                                source_bpm: orig.source_bpm, // halvan ärver klippets källa
                                                tape: false,
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
                                        let raw_drop_sec = secs_at(raw_drop_bar);
                                        let drop_sec = match self.timeline_snap_mode {
                                            TimeSnapMode::FreeHundredth => (raw_drop_sec * 100.0).round() / 100.0,
                                            TimeSnapMode::Snap16th => {
                                                let step_sec = sec_per_bar_at(raw_drop_bar) / 16.0;
                                                (raw_drop_sec / step_sec).round() * step_sec
                                            }
                                            TimeSnapMode::SnapBeat => {
                                                let beat_sec = sec_per_bar_at(raw_drop_bar) / 4.0;
                                                (raw_drop_sec / beat_sec).round() * beat_sec
                                            }
                                            TimeSnapMode::SnapBar => (raw_drop_sec / sec_per_bar_at(raw_drop_bar)).round() * sec_per_bar_at(raw_drop_bar),
                                        };
                                        let drop_bar = bars_at(drop_sec).max(0.0);
                                        let item_dur_sec: f32 = 2.0;
                                        let item_dur_bars = (item_dur_sec / sec_per_bar_at(drop_bar)).max(0.25);
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
                                self.render_automation_lane(ui, auto_rect, &auto_resp, bar_w, sel);
                                auto_bottom = auto_rect.max.y;
                            }

                            // ====================================================
                            // 2.3 VERTICAL PLAYHEAD NEEDLE ACROSS ALL TRACKS
                            // ====================================================
                            let playhead_bar = bars_at(self.song_time);
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
                    let mut do_tape = false;
                    let mut do_open_focus = false;
                    let mut do_open_vocal_studio = false;
                    let mut do_save_sample = false;
                    let has_copied = self.copied_region.is_some();
                    let copied_name = self.copied_region.as_ref().map(|c| c.name.clone()).unwrap_or_default();
                    let r = &mut self.playlist_tracks[t_idx].regions[r_idx];
                    let r_start_sec = secs_at(r.start_bar);
                    let r_len_sec = secs_len(r.start_bar, r.length_bars);
                    let r_name = r.name.clone();
                    let is_rev = r.is_reverse;
                    let is_tape = r.tape;

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
                                r.start_bar = bars_at((r_start_sec - 0.1).max(0.0));
                            }
                            if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Flytta 0.1s framåt")).clicked() {
                                r.start_bar = bars_at(r_start_sec + 0.1);
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
                                r.length_bars = bars_len(r.start_bar, (r_len_sec - 0.1).max(0.05));
                            }
                            if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Förläng 0.1s")).clicked() {
                                r.length_bars = bars_len(r.start_bar, r_len_sec + 0.1);
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
                                format!("{:.2}s", secs_at(v as f32))
                            }));

                            ui.separator();

                            // Fade Out Slider
                            ui.label(crate::i18n::t("📉 Ut:"));
                            ui.add(egui::Slider::new(&mut r.fade_out_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                                format!("{:.2}s", secs_at(v as f32))
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

                            // Klippets temoläge (Fas 8.10 steg 2): sträcks med bevarad
                            // tonhöjd (standard) eller bandspelaren (undantaget).
                            let tape_label = if is_tape {
                                crate::i18n::t("📼 Bandspelare")
                            } else {
                                crate::i18n::t("🎚 Sträcks (bevarad tonhöjd)")
                            };
                            let tape_bg = if is_tape { Theme::FL_ORANGE } else { Color32::from_rgb(30, 48, 44) };
                            if ui
                                .add(egui::Button::new(egui::RichText::new(tape_label).strong().size(10.5).color(Color32::WHITE)).fill(tape_bg))
                                .on_hover_text(crate::i18n::t(
                                    "Standard: klippet sträcks med bevarad tonhöjd när tempot ändras (tonhöjden står still). Bandspelarläget låter tonhöjden följa med — det är en effekt, inte standarden.",
                                ))
                                .clicked()
                            {
                                do_tape = true;
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
                    } else if do_tape {
                        self.toggle_region_tape(t_idx, r_idx);
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

    /// Tonerna i projektets tonart. Regeln bor i `crate::audio::scale` — här finns
    /// bara projektets grundton och skala (Fas 8.11).
    pub fn get_scale_notes(&self) -> Vec<u8> {
        crate::audio::scale::scale_notes(self.song_key_root, self.song_key_scale)
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
                let cur_root = crate::audio::scale::root_name(self.song_key_root);
                egui::ComboBox::from_id_salt("pr_root_combo")
                    .selected_text(cur_root)
                    .width(45.0)
                    .show_ui(ui, |ui| {
                        for (idx, &r_name) in crate::audio::scale::ROOT_NAMES.iter().enumerate() {
                            if ui.selectable_label(self.song_key_root as usize == idx, r_name).clicked() {
                                self.song_key_root = idx as u8;
                            }
                        }
                    });

                // Scale Snapping Selector
                ui.label(egui::RichText::new(crate::i18n::t("Skala:")).size(11.0).color(Theme::TEXT_MUTED));
                let cur_scale = crate::i18n::t(crate::audio::scale::scale_at(self.song_key_scale).name);
                egui::ComboBox::from_id_salt("pr_scale_combo")
                    .selected_text(cur_scale)
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for (s_idx, sc) in crate::audio::scale::SCALES.iter().enumerate() {
                            if ui.selectable_label(self.song_key_scale == s_idx, crate::i18n::t(sc.name)).clicked() {
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

                // **Till tonarten** (Fas 8.11): flyttar hela mönstret till det skift som sätter
                // flest toner i projektets skala. Regeln räknar bara ut *skiftet*; själva
                // flytten går genom `transpose_active_pattern` — samma provade väg som
                // oktavknapparna, alltså en väg och inte två.
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎵 Till tonarten")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Flytta hela mönstret till tonarten — det kortaste skiftet som sätter flest toner i skalan")).clicked() {
                    let rows: Vec<usize> = (0..crate::audio::scale::PIANO_ROLL_ROWS)
                        .filter(|&o| self.piano_roll_grid[o].iter().any(|&b| b))
                        .collect();
                    let shift = crate::audio::scale::key_transpose(
                        &rows,
                        crate::audio::scale::PIANO_ROLL_BASE_MIDI,
                        self.song_key_root,
                        self.song_key_scale,
                    );
                    if shift == 0 {
                        self.status_message =
                            crate::i18n::t("🎵 Mönstret står redan i tonarten").to_string();
                    } else {
                        self.transpose_active_pattern(shift);
                        self.status_message = crate::tstatus!(
                            "🎵 {} halvtoner till {}",
                            shift,
                            crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                        );
                    }
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

            let base_midi = crate::audio::scale::PIANO_ROLL_BASE_MIDI; // C3
            let semitones_count = 24;
            let key_root = self.song_key_root;
            let key_scale = self.song_key_scale;
            let root_note_val = key_root % 12;

            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                for note_offset in (0..semitones_count).rev() {
                    let midi_note = base_midi + note_offset as u8;
                    let note_val = midi_note % 12;
                    let note_in_scale = crate::audio::scale::in_scale(note_val, key_root, key_scale);
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
                                    // Skal-låset (Fas 8.11). Knappen fanns förut men
                                    // lästes aldrig — den gjorde ingenting. Nu flyttas
                                    // en klickad rad utanför skalan till **närmaste** rad
                                    // som är i den, och markeringen i rutnätet visar
                                    // vilken rad det blev. Att tysta klicket vore att
                                    // låtsas att tangenten fanns.
                                    let row = if self.piano_roll_snap_to_scale {
                                        crate::audio::scale::snap_row(
                                            note_offset,
                                            base_midi,
                                            semitones_count,
                                            self.song_key_root,
                                            self.song_key_scale,
                                        )
                                    } else {
                                        note_offset
                                    };
                                    let row_midi = base_midi + row as u8;
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
                                        let target_off = row + c_off;
                                        if target_off < 24 {
                                            self.piano_roll_grid[target_off][step] = true;
                                        }
                                    }
                                    self.channels[6].steps[step] = true;
                                    self.channels[6].notes[step] = row_midi;
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
                        } else if ui.button(crate::i18n::t("🔌 Anslut MIDI")).clicked() {
                            match MidiKeyboardInput::connect(self.control_tx.clone()) {
                                Ok(m) => {
                                    self.midi_input = Some(m);
                                    self.status_message = crate::i18n::t("✔ MIDI-in öppnad — klaviaturen ansluts automatiskt.").to_string();
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
                    ui.label(egui::RichText::new(crate::i18n::t("Tips: klaviaturen ansluts automatiskt. Hittas den inte: koppla in den och anslut igen.")).size(9.5).color(Theme::TEXT_MUTED));
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
                // Samplern (Fas 8.4) följer med till filen — samma ljud i exporten
                // som i högtalarna, samma krav som för sidokedjan och sendarna.
                loop_mode: ch.loop_mode,
                loop_start: ch.sample_loop_start,
                loop_end: ch.sample_loop_end,
                ping_pong: ch.ping_pong,
                amp_env: ch.amp_env,
            });
            RackChannel {
                voice,
                slices: ch.slices.clone(),
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
            // Sends (Fas 8.13 bussar, Fas 8.3 spår) hör till samma väg: ett fruset spår
            // spelar sin färdigrenderade fil, och den renderades utan sends — alltså faller
            // båda slagen bort. Att *ta emot* en send är däremot som förut: mottagarens
            // kedja är densamma, och senden går in i den precis som sitt eget ljud.
            let sends = if t.frozen_pcm.is_some() {
                Vec::new()
            } else {
                t.sends.clone()
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
                sidechain_from: t.sidechain_from,
                sidechain_amount_db: t.sidechain_amount_db,
                sidechain_threshold_db: t.sidechain_threshold_db,
                sends,
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

    /// Frågan om mp3 eller wav (Alex' förslag, Fas 8.5).
    ///
    /// **Varför en fråga och inte en regel som bara kör.** Appen kan inte spela
    /// mp3 — den konverteras först — så en mp3 vars stämma redan finns som wav är
    /// ett extra varv utan vinst. Men den som importerar kan ha skäl att vilja ha
    /// mp3-filerna med (de finns i projektet, de kan användas utanför Sonix), så
    /// valet är användarens. Standardvalet är regeln: hoppa över dem.
    fn render_wav_question_modal(&mut self, ctx: &egui::Context) {
        let Some(question) = self.pending_wav_question.clone() else {
            return;
        };
        let mut keep_mp3: Option<bool> = None;
        let mut separate_sibling = false;
        let mut separate_chosen = false;
        let mut cancel = false;
        egui::Window::new(crate::i18n::t("🎧 mp3 eller wav?"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_size(Vec2::new(620.0, 200.0))
            .show(ctx, |ui| match &question {
                WavQuestion::StemImport { duplicates, title, .. } => {
                    ui.label(
                        egui::RichText::new(crate::tstatus!(
                            "{} mp3-filer i '{}' har redan sin stämma som wav-fil.",
                            duplicates,
                            title
                        ))
                        .size(12.5)
                        .color(Theme::TEXT_BRIGHT),
                    );
                    ui.label(
                        egui::RichText::new(crate::i18n::t(
                            "Sonix kan inte spela mp3 — den konverteras först, vilket är ett extra varv när wav-filen redan finns. En stämma som bara finns som mp3 tas alltid med.",
                        ))
                        .size(11.0)
                        .color(Theme::TEXT_MUTED),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(crate::i18n::t("Hoppa över mp3:erna (rekommenderas)"))
                            .clicked()
                        {
                            keep_mp3 = Some(false);
                        }
                        if ui.button(crate::i18n::t("Ta med mp3:erna också")).clicked() {
                            keep_mp3 = Some(true);
                        }
                        if ui.button(crate::i18n::t("Avbryt importen")).clicked() {
                            cancel = true;
                        }
                    });
                }
                WavQuestion::Separate { chosen, sibling } => {
                    ui.label(
                        egui::RichText::new(crate::i18n::t(
                            "Du valde en mp3 — och wav-filen finns bredvid.",
                        ))
                        .size(12.5)
                        .color(Theme::TEXT_BRIGHT),
                    );
                    ui.label(
                        egui::RichText::new(crate::tstatus!("mp3:  {}", chosen))
                            .size(10.5)
                            .color(Theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(crate::tstatus!("wav:  {}", sibling))
                            .size(10.5)
                            .color(Theme::TEXT_MUTED),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(crate::i18n::t("Separera wav-filen (rekommenderas)"))
                            .clicked()
                        {
                            separate_sibling = true;
                        }
                        if ui.button(crate::i18n::t("Separera mp3-filen")).clicked() {
                            separate_chosen = true;
                        }
                        if ui.button(crate::i18n::t("Avbryt")).clicked() {
                            cancel = true;
                        }
                    });
                }
            });

        if cancel {
            self.pending_wav_question = None;
            self.status_message = crate::i18n::t("Avbrutet — inget lästes in").to_string();
            return;
        }
        match question {
            WavQuestion::StemImport { source, title, bpm, .. } => {
                let Some(keep) = keep_mp3 else {
                    return;
                };
                self.pending_wav_question = None;
                match source {
                    StemImportSource::Folder(dir) => {
                        self.start_suno_folder_import(&dir, &title, bpm, keep)
                    }
                    StemImportSource::Zip(zip) => self.start_suno_zip_import_choice(&zip, keep),
                }
            }
            WavQuestion::Separate { chosen, sibling } => {
                if separate_sibling {
                    self.pending_wav_question = None;
                    self.start_stem_separation_with(&sibling);
                } else if separate_chosen {
                    self.pending_wav_question = None;
                    self.start_stem_separation_with(&chosen);
                }
            }
        }
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
            self.import_suno_stems_from_folder(&folder, &title, bpm);
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
                    // Samma logga, större — och samma fallback.
                    match self.logo_texture_for(ctx) {
                        Some(tex) => {
                            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::Vec2::splat(96.0)));
                        }
                        None => {
                            ui.label(egui::RichText::new(crate::i18n::t("SONIX")).size(28.0).color(Theme::FL_CYAN));
                        }
                    }
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

        // Samma karta som tidlinjen (Fas 8.2 steg 3): panelen visar tider, och en tid efter
        // ett tempobyte är bara rätt om den kommer från kartan.
        let tempo = self.tempo_map();

        // **De fyra frågorna `sec_per_bar` svarade på — men med fel svar efter ett
        // tempobyte** (Fas 8.2 steg 3). En *plats* i tiden, en *längd*, en *omvändning*
        // och en *lokal* sekunder-per-takt är olika frågor; en enda skalär kunde bara
        // svara rätt på dem så länge tempot var konstant. Med ett tempo ger de samma tal
        // som förut, bit för bit.
        let secs_at = |bar: f32| tempo.secs_at_bar(bar as f64) as f32;
        let secs_len = |from_bar: f32, bars: f32| tempo.secs_for_bars_at(from_bar as f64, bars as f64) as f32;
        let bars_len = |from_bar: f32, secs: f32| tempo.bars_for_secs_at(from_bar as f64, secs as f64) as f32;

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
                                global_fade_in_sec = secs_len(track.regions[0].start_bar, track.regions[0].fade_in_bars);
                                global_fade_out_sec = secs_len(track.regions[0].start_bar, track.regions[0].fade_out_bars);
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
                                    r.fade_in_bars = bars_len(r.start_bar, global_fade_in_sec);
                                    r.fade_out_bars = bars_len(r.start_bar, global_fade_out_sec);
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
                                    let target_sec = norm * secs_len(0.0, 140.0);
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

                                        let r_start_sec = secs_at(r.start_bar);
                                        let r_len_sec = secs_len(r.start_bar, r.length_bars);
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

