//! Status: stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends
//! Rör inte: körordningen — `stem_order` och `stem_incoming` byggs i **samma** pass, så sändarens utgång finns när mottagarens kedja kör (8.3; provet `a_track_send_arrives_in_phase`).
//!
//! Här bor bara **roten**: `SynthEngine`-tillståndet, `new` och kommandots namn.
//! Resten är delat efter område (2026-09-14 — modulen var 3 517 rader): `process`
//! (renderingsloopen), `commands` (kommandovägen och körordningen), `voices` (rösterna,
//! spåren och samplingarna), `tests` (motorns prov).
use std::f32::consts::PI;
use std::sync::Mutex;
use super::command::{
    AudioCommand, LoopMode, Preset, SendTarget, StemRegionPlayback, StemSend, Waveform,
    loop_frames,
};
use super::drum::{DrumType, DrumVoice};
use super::effects::{DelayParams, ReverbParams, SimpleReverb, StereoDelay};
use super::envelope::{AdsrParams, AdsrVoice};
use super::filter::{FilterParams, StateVariableFilter};
use super::master_fx::{Compressor, CompressorParams, MasterFxChain, RemixFx, StereoEq, TapeStop, TrackEqSettings};
use super::patcher::{PatchProcessor, PatchSpec};
use super::plugin_host_live::{PdcDelay, PluginInsert};
use super::vocal_harmonizer::{FormantPitchShifter, Wsola};

mod commands;
mod process;
mod voices;

use voices::*;

/// Felsökningsloggning till `~/.local/state/sonix/logs/audio_debug.log`.
/// Påslagen när `SONIX_AUDIO_DEBUG` är satt, eller när markörfilen
/// `~/.local/state/sonix/debug_on` finns (så vanliga skrivbordsstarter kan
/// logga). Den äldre markören `~/Music/Sonix/debug_on` läses fortfarande.
fn dbg_enabled() -> bool {
    if std::env::var("SONIX_AUDIO_DEBUG").is_ok() {
        return true;
    }
    let paths = crate::paths::paths();
    paths.debug_marker_file().exists() || paths.legacy_library_file("debug_on").exists()
}

fn dbg_log(tag: &str, msg: &str) {
    if !dbg_enabled() {
        return;
    }
    let path = crate::paths::paths().log_file("audio_debug.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {}: {}", std::process::id(), tag, msg);
    }
}

const MAX_VOICES: usize = 16;
const MAX_DRUMS: usize = 10;
const MAX_SAMPLE_VOICES: usize = 24;

/// Number of sub-mix buses (Vocal, Drum, Synth, FX) — Fas 5.2.
pub const NUM_BUSES: usize = 4;
/// Number of VCA control groups — Fas 5.2.
pub const NUM_VCAS: usize = 4;

/// Human-readable names for the four sub-mix buses, used by the mixer UI.
pub const BUS_NAMES: [&str; NUM_BUSES] = ["Vocal", "Trummor", "Synth", "FX"];

/// One-line description of a command, WITHOUT dumping Arc/PCM contents.
fn describe_cmd(cmd: &AudioCommand) -> String {
    match cmd {
        AudioCommand::StopAll => "StopAll".to_string(),
        AudioCommand::SetSongPlayback(playing) => format!("SetSongPlayback({})", playing),
        AudioCommand::SetMasterVolume(v) => format!("SetMasterVolume({})", v),
        AudioCommand::ClearAllStemTracks => "ClearAllStemTracks".to_string(),
        AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
            format!(
                "LoadStemTrack(idx={} sr={:.0} vol={} pan={} start={:.2}s lenL={} lenR={})",
                track_index,
                sample_rate,
                volume,
                pan,
                start_time_secs,
                left.len(),
                right.len()
            )
        }
        AudioCommand::SetStemTrackState { track_index, volume, pan, muted, solo } => {
            format!("SetStemTrackState(idx={} vol={} pan={} muted={} solo={})", track_index, volume, pan, muted, solo)
        }
        AudioCommand::SetStemTrackRegions { track_index, regions } => {
            format!("SetStemTrackRegions(idx={} nregions={})", track_index, regions.len())
        }
        AudioCommand::SetStemTrackRouting { track_index, bus, vca } => {
            format!("SetStemTrackRouting(idx={} bus={} vca={:?})", track_index, bus, vca)
        }
        AudioCommand::SetStemTrackSends { track_index, sends } => {
            format!("SetStemTrackSends(idx={} antal={})", track_index, sends.len())
        }
        AudioCommand::SetStemTrackSidechain { track_index, from, amount_db, threshold_db } => {
            format!("SetStemTrackSidechain(idx={} from={:?} amount={} threshold={})", track_index, from, amount_db, threshold_db)
        }
        AudioCommand::SetBusState { bus, volume, muted, solo } => {
            format!("SetBusState(bus={} vol={} muted={} solo={})", bus, volume, muted, solo)
        }
        AudioCommand::SetVcaState { vca, volume, muted, solo } => {
            format!("SetVcaState(vca={} vol={} muted={} solo={})", vca, volume, muted, solo)
        }
        AudioCommand::SeekSongPosition(s) => format!("SeekSongPosition({:.2}s)", s),
        other => format!("{}", variant_name(other)),
    }
}

fn variant_name(cmd: &AudioCommand) -> &'static str {
    match cmd {
        AudioCommand::NoteOn { .. } => "NoteOn",
        AudioCommand::NoteOnDelayed { .. } => "NoteOnDelayed",
        AudioCommand::NoteOff { .. } => "NoteOff",
        AudioCommand::StrumChord { .. } => "StrumChord",
        AudioCommand::TriggerDrum(_) => "TriggerDrum",
        AudioCommand::SetWaveform(_) => "SetWaveform",
        AudioCommand::SetAdsr(_) => "SetAdsr",
        AudioCommand::SetFilter(_) => "SetFilter",
        AudioCommand::SetFilterEnv { .. } => "SetFilterEnv",
        AudioCommand::SetDelay(_) => "SetDelay",
        AudioCommand::SetReverb(_) => "SetReverb",
        AudioCommand::SetDrive(_) => "SetDrive",
        AudioCommand::SetMasterFx(_) => "SetMasterFx",
        AudioCommand::SetTrackEq { .. } => "SetTrackEq",
        AudioCommand::SetTrackMix { .. } => "SetTrackMix",
        AudioCommand::SetRemixFx { .. } => "SetRemixFx",
        AudioCommand::SetTapeStop { .. } => "SetTapeStop",
        AudioCommand::LoadPreset(_) => "LoadPreset",
        AudioCommand::SetMasterVolume(_) => "SetMasterVolume",
        AudioCommand::StopAll => "StopAll",
        AudioCommand::LoadStemTrack { .. } => "LoadStemTrack",
        AudioCommand::ClearAllStemTracks => "ClearAllStemTracks",
        AudioCommand::SetStemTrackSends { .. } => "SetStemTrackSends",
        AudioCommand::SetStemTrackState { .. } => "SetStemTrackState",
        AudioCommand::SetStemTrackRegions { .. } => "SetStemTrackRegions",
        AudioCommand::SetStemTrackRouting { .. } => "SetStemTrackRouting",
        AudioCommand::SetStemTrackSidechain { .. } => "SetStemTrackSidechain",
        AudioCommand::SetBusState { .. } => "SetBusState",
        AudioCommand::SetVcaState { .. } => "SetVcaState",
        AudioCommand::SeekSongPosition(_) => "SeekSongPosition",
        AudioCommand::SetSongPlayback(_) => "SetSongPlayback",
        AudioCommand::PlayAudition { .. } => "PlayAudition",
        AudioCommand::StopAudition => "StopAudition",
        AudioCommand::SetMonitorRing { .. } => "SetMonitorRing",
        AudioCommand::SetMonitorLevel(_) => "SetMonitorLevel",
        AudioCommand::SetAuditionParams { .. } => "SetAuditionParams",
        AudioCommand::TriggerSampleVoice { .. } => "TriggerSampleVoice",
        AudioCommand::ReleaseSampleVoices { .. } => "ReleaseSampleVoices",
        AudioCommand::SetPatcherGraph(_) => "SetPatcherGraph",
        AudioCommand::SetPatcherEnabled(_) => "SetPatcherEnabled",
        AudioCommand::PatcherNoteOn { .. } => "PatcherNoteOn",
        AudioCommand::PatcherNoteOff => "PatcherNoteOff",
        AudioCommand::SetTrackPlugin { .. } => "SetTrackPlugin",
        AudioCommand::SetPluginParameter { .. } => "SetPluginParameter",
    }
}

use std::sync::Arc;

pub struct SynthEngine {
    pub sample_rate: f32,
    pub waveform: Waveform,
    pub adsr: AdsrParams,
    pub filter_params: FilterParams,
    /// Per-voice filter envelope depth in octaves and its shape.
    pub filter_env_amount: f32,
    pub filter_env: AdsrParams,
    pub delay_params: DelayParams,
    pub delay: StereoDelay,
    pub reverb_params: ReverbParams,
    pub reverb: SimpleReverb,
    pub drive: f32,
    pub master_fx: MasterFxChain,
    pub remix_fx: RemixFx,
    pub tape_stop: TapeStop,
    pub master_volume: f32,
    pub voices: [Voice; MAX_VOICES],
    pub drums: [DrumVoice; MAX_DRUMS],
    // Multi-track audio stem streaming
    pub stem_tracks: Vec<StemVoiceTrack>,
    pub has_stem_solo: bool,
    /// Hur många spår som **faktiskt** har en sträckt fil i sin regionlista (Fas 8.10c).
    ///
    /// Räknas om när regioner eller spår byts — aldrig i sample-loopen. Appen läser
    /// talet och kan säga "motorn spelar 8 av 9" i stället för att anta att kommandot
    /// landade. **Mätt 2026-09-13:** `LoadStemTrack` byggde ett nytt spår och tömde
    /// regionerna tyst — Alex hörde "ingen märkbar skillnad på tempo" medan cachen var
    /// full av korrekta filer och vyn ritade som om de spelades.
    pub stretched_region_tracks: u32,
    pub song_playing: bool,
    pub song_time_samples: usize,
    // Isolated audition for Vocal Studio & Sound Browser
    pub audition: Option<AuditionVoice>,
    // Zero-latency microphone direct monitoring (post auto-tune mono samples)
    pub monitor_ring: Option<Arc<Mutex<Vec<f32>>>>,
    monitor_buf: Vec<f32>,
    monitor_idx: usize,
    /// Nivå för direktlyssningen (0.0–1.0), satt med `SetMonitorLevel`.
    monitor_level: f32,
    // Polyphonic WAV one-shot voices for the Channel Rack sample player
    pub sample_voices: [SampleVoice; MAX_SAMPLE_VOICES],
    pub sample_voice_cursor: usize,
    // Scheduled chord/strum notes (Chord Matrix audition)
    pub scheduled_notes: Vec<ScheduledNote>,
    // Modular Patcher DSP graph
    pub patcher: Option<PatchProcessor>,
    pub patcher_spec: Option<PatchSpec>,
    pub patcher_enabled: bool,
    /// PDC delay line for the synth/drum/sample bus (Fas 4.2).
    pub pdc_bus: PdcDelay,
    /// Sub-mix bus group gains / mute / solo (Fas 5.2).
    pub bus_volume: [f32; NUM_BUSES],
    pub bus_muted: [bool; NUM_BUSES],
    pub bus_solo: [bool; NUM_BUSES],
    /// VCA group gains / mute / solo (Fas 5.2).
    pub vca_volume: [f32; NUM_VCAS],
    pub vca_muted: [bool; NUM_VCAS],
    pub vca_solo: [bool; NUM_VCAS],
    /// **Ordningen stämmorna räknas i** (Fas 8.3): topologisk över spår-sends, så att en
    /// mottagare alltid räknas **efter** sina sändare. Räknas om när sends eller spår
    /// ändras — aldrig per sample.
    pub stem_order: Vec<usize>,
    /// Utgångarna för det **pågående samplet** (Fas 8.3). En spår-send läser sändarens
    /// utgång härifrån, så att den kommer fram exakt i fas i stället för en sample sent.
    /// Buffetarna återanvänds — ingen allokering per sample.
    pub track_out_l: Vec<f32>,
    pub track_out_r: Vec<f32>,
    /// **Vilka som skickar till ett spår** (Fas 8.3): för varje spår en lista av
    /// `(sändare, nivå)`. Byggs i samma pass som `stem_order` — samma graf, samma ställe —
    /// så att ordningen och vägarna inte kan driva isär. Sends ändras bara via
    /// `SetStemTrackSends`, som räknar om båda.
    pub stem_incoming: Vec<Vec<(usize, f32)>>,
    // Debug heartbeat counters (only used when SONIX_AUDIO_DEBUG is set)
    pub dbg_frames: u64,
}

impl SynthEngine {
    pub fn new(sample_rate: f32) -> Self {
        let (waveform, adsr, filter_params) = Preset::CleanPluck.settings();
        Self {
            sample_rate,
            waveform,
            adsr,
            filter_params,
            filter_env_amount: 0.0,
            filter_env: AdsrParams::default(),
            delay_params: DelayParams::default(),
            delay: StereoDelay::new(sample_rate),
            reverb_params: ReverbParams::default(),
            reverb: SimpleReverb::new(sample_rate),
            drive: 1.0,
            master_fx: MasterFxChain::new(sample_rate),
            remix_fx: RemixFx::new(sample_rate),
            tape_stop: TapeStop::new(sample_rate),
            master_volume: 0.8,
            voices: [Voice::new(sample_rate); MAX_VOICES],
            drums: [DrumVoice::new(sample_rate); MAX_DRUMS],
            stem_tracks: Vec::new(),
            has_stem_solo: false,
            stretched_region_tracks: 0,
            song_playing: false,
            song_time_samples: 0,
            audition: None,
            monitor_ring: None,
            monitor_buf: Vec::new(),
            monitor_idx: 0,
            monitor_level: 1.0,
            sample_voices: std::array::from_fn(|_| SampleVoice::new()),
            sample_voice_cursor: 0,
            scheduled_notes: Vec::new(),
            patcher: None,
            patcher_spec: None,
            patcher_enabled: false,
            pdc_bus: PdcDelay::new(),
            bus_volume: [1.0; NUM_BUSES],
            bus_muted: [false; NUM_BUSES],
            bus_solo: [false; NUM_BUSES],
            vca_volume: [1.0; NUM_VCAS],
            vca_muted: [false; NUM_VCAS],
            vca_solo: [false; NUM_VCAS],
            stem_order: Vec::new(),
            track_out_l: Vec::new(),
            track_out_r: Vec::new(),
            stem_incoming: Vec::new(),
            dbg_frames: 0,
        }
    }

    fn dbg_state(&self, tag: &str, msg: &str) {
        let voices = self.voices.iter().filter(|v| v.is_active()).count();
        let drums = self.drums.iter().filter(|d| d.active).count();
        let samples = self.sample_voices.iter().filter(|v| v.is_active()).count();
        dbg_log(
            tag,
            &format!(
                "{} | song_playing={} t={:.2}s stems={} voices={} drums={} samples={} audition={} master={:.3}",
                msg,
                self.song_playing,
                self.song_time_samples as f32 / self.sample_rate,
                self.stem_tracks.len(),
                voices,
                drums,
                samples,
                self.audition.as_ref().map(|a| a.is_playing).unwrap_or(false),
                self.master_volume,
            ),
        );
    }

}

#[cfg(test)]
mod tests;
