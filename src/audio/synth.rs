use std::f32::consts::PI;
use std::sync::Mutex;
use super::command::{AudioCommand, Preset, StemRegionPlayback, Waveform};
use super::drum::{DrumType, DrumVoice};
use super::effects::{DelayParams, ReverbParams, SimpleReverb, StereoDelay};
use super::envelope::{AdsrParams, AdsrVoice};
use super::filter::{FilterParams, StateVariableFilter};
use super::master_fx::{Compressor, CompressorParams, MasterFxChain, RemixFx, StereoEq, TapeStop, TrackEqSettings};
use super::patcher::{PatchProcessor, PatchSpec};
use super::plugin_host_live::{PdcDelay, PluginInsert};
use super::vocal_harmonizer::{FormantPitchShifter, Wsola};

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
        AudioCommand::SetPatcherGraph(_) => "SetPatcherGraph",
        AudioCommand::SetPatcherEnabled(_) => "SetPatcherEnabled",
        AudioCommand::PatcherNoteOn { .. } => "PatcherNoteOn",
        AudioCommand::PatcherNoteOff => "PatcherNoteOff",
        AudioCommand::SetTrackPlugin { .. } => "SetTrackPlugin",
        AudioCommand::SetPluginParameter { .. } => "SetPluginParameter",
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Voice {
    pub note: u8,
    pub freq: f32,
    pub phase: f32,
    pub phase_inc: f32,
    pub velocity: f32,
    pub envelope: AdsrVoice,
    /// Per-voice filter state so simultaneously sounding notes do not share a
    /// single filter (each note keeps its own resonant state).
    pub filter: StateVariableFilter,
    /// Per-voice filter envelope; its level modulates the cutoff per note.
    pub filter_env: AdsrVoice,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            note: 0,
            freq: 440.0,
            phase: 0.0,
            phase_inc: 0.0,
            velocity: 0.0,
            envelope: AdsrVoice::new(sample_rate),
            filter: StateVariableFilter::new(sample_rate),
            filter_env: AdsrVoice::new(sample_rate),
        }
    }

    pub fn trigger(&mut self, note: u8, freq: f32, velocity: f32, sample_rate: f32) {
        self.note = note;
        self.freq = freq;
        self.velocity = velocity;
        self.phase_inc = (2.0 * PI * freq) / sample_rate;
        self.filter.reset();
        self.envelope.gate_on();
        self.filter_env.gate_on();
    }

    pub fn release(&mut self) {
        self.envelope.gate_off();
        self.filter_env.gate_off();
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.filter_env.reset();
        self.velocity = 0.0;
        self.phase = 0.0;
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.envelope.is_active()
    }

    #[inline(always)]
    pub fn next_sample(
        &mut self,
        waveform: Waveform,
        adsr: &AdsrParams,
        filter_params: &FilterParams,
        filter_env: &AdsrParams,
        filter_env_amount: f32,
    ) -> f32 {
        if !self.envelope.is_active() {
            return 0.0;
        }

        let env_gain = self.envelope.next_sample(adsr);
        if env_gain <= 0.0 {
            return 0.0;
        }

        let raw_sample = match waveform {
            Waveform::Sine => self.phase.sin(),
            Waveform::Saw => (self.phase / PI) - 1.0,
            Waveform::Square => {
                if self.phase < PI {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Triangle => {
                let normalized = self.phase / (2.0 * PI);
                2.0 * (2.0 * (normalized - (normalized + 0.5).floor())).abs() - 1.0
            }
        };

        // Advance phase
        self.phase += self.phase_inc;
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }

        // Each voice runs through its own resonant filter instance, and its own
        // filter envelope modulates the cutoff (in octaves) independently.
        let fenv = self.filter_env.next_sample(filter_env);
        let effective = FilterParams {
            cutoff: filter_params.cutoff * 2.0_f32.powf(filter_env_amount * fenv),
            resonance: filter_params.resonance,
        };
        self.filter.process_lowpass(raw_sample * self.velocity * env_gain, &effective)
    }
}

use std::sync::Arc;

/// Sidokedje-duckare (Fas 8.3).
///
/// **Varför en egen liten DSP och inte en kompressor:** en kompressor med extern
/// nyckel gör samma sak som att köra kompressorn på key-signalen och multiplicera
/// den egna signalen med dess gain reduction. Det här är samma matematik, men
/// utskriven: ett envelopföljande steg på key-signalen och en gain som går mot
/// `-amount_db` när nyckeln är över tröskeln. Tidkonstanterna är fasta (5 ms
/// attack, 120 ms release) — det är de värden en duckare brukar ha, och ett
/// reglage mindre att förklara.
///
/// **Nyckeln är som mest ett sample gammal.** Spårloopen går i indexordning, så
/// ett key-spår med högre index än målet har redan passerat förra samplet när
/// målet räknas. 20 µs vid 48 kHz, mot tidkonstanter i millisekunder — att kräva
/// samma sample hade krävt en omsortering av hela loopen per sample, och det
/// hade varit en mycket dyrare konstruktion för en omätbar skillnad.
#[derive(Clone, Copy, Debug)]
pub struct Ducker {
    /// Utjämnad nyckelnivå (linjär).
    env: f32,
    attack: f32,
    release: f32,
}

impl Ducker {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            env: 0.0,
            attack: 1.0 - (-1.0 / (0.005 * sr)).exp(),
            release: 1.0 - (-1.0 / (0.120 * sr)).exp(),
        }
    }

    /// Lämnar gainen för det här samplet (1.0 = orörd).
    pub fn process(&mut self, key_l: f32, key_r: f32, threshold_db: f32, amount_db: f32) -> f32 {
        let peak = key_l.abs().max(key_r.abs());
        let coeff = if peak > self.env { self.attack } else { self.release };
        self.env += (peak - self.env) * coeff;
        let env_db = 20.0 * self.env.max(1e-6).log10();
        if env_db <= threshold_db {
            return 1.0;
        }
        let target = 10.0f32.powf(-amount_db.clamp(0.0, 60.0) / 20.0);
        // Mjuk övergång: 6 dB över tröskeln ger hela duckningen. Utan den hade
        // gränsen blivit ett hörbart knäpp.
        let over = ((env_db - threshold_db) / 6.0).clamp(0.0, 1.0);
        1.0 - (1.0 - target) * over
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }
}

pub struct StemVoiceTrack {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: f32,
    pub volume: f32,
    pub pan: f32,
    pub pan_l: f32,
    pub pan_r: f32,
    pub muted: bool,
    pub solo: bool,
    pub start_time_secs: f32,
    pub regions: Vec<StemRegionPlayback>,
    pub eq: TrackEqSettings,
    pub eq_proc: StereoEq,
    pub comp: Compressor,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb: SimpleReverb,
    pub reverb_send: f32,
    pub delay: StereoDelay,
    pub delay_send: f32,
    /// Requested transposition in semitones (±12, cents resolution).
    pub pitch_semitones: f32,
    /// Real-time, formant-preserving pitch shifter for this stem.
    pub pitch_shifter: FormantPitchShifter,
    /// Whether the shifter is engaged (false = bit-exact bypass).
    pub pitch_active: bool,
    /// Optional CLAP insert on this track (Fas 4.2).
    pub plugin: Option<PluginInsert>,
    /// Delay line that aligns this track with the project's max plugin latency.
    pub pdc: PdcDelay,
    /// Sub-mix bus this track feeds (`0..NUM_BUSES`) — Fas 5.2.
    pub bus: usize,
    /// Optional VCA control group (`0..NUM_VCAS`) — Fas 5.2.
    pub vca: Option<usize>,
    /// Sidokedja (Fas 8.3): spåret duckas av det här spårets ljud.
    pub sidechain_from: Option<usize>,
    /// Hur mycket spåret sänks när key-signalen är över tröskeln.
    pub sidechain_amount_db: f32,
    /// Tröskeln key-signalen måste över för att ducka.
    pub sidechain_threshold_db: f32,
    /// Duckarens tillstånd (en per spår — annars styr ett spår ett annat).
    pub sidechain_ducker: Ducker,
    /// Spårets senaste utgångssample, för sidokedjor som pekar hit.
    pub last_out_l: f32,
    pub last_out_r: f32,
}

impl StemVoiceTrack {
    pub fn new(
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pan: f32,
        start_time_secs: f32,
        engine_sample_rate: f32,
    ) -> Self {
        let p = pan.clamp(-1.0, 1.0);
        let pan_l = ((1.0 - p) * 0.5).sqrt();
        let pan_r = ((1.0 + p) * 0.5).sqrt();
        Self {
            left,
            right,
            sample_rate,
            volume,
            pan: p,
            pan_l,
            pan_r,
            muted: false,
            solo: false,
            start_time_secs,
            regions: Vec::new(),
            eq: TrackEqSettings::default(),
            eq_proc: StereoEq::new(engine_sample_rate),
            comp: Compressor::new(engine_sample_rate),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb: SimpleReverb::new(engine_sample_rate),
            reverb_send: 0.0,
            delay: StereoDelay::new(engine_sample_rate),
            delay_send: 0.0,
            pitch_semitones: 0.0,
            pitch_shifter: FormantPitchShifter::new(engine_sample_rate),
            pitch_active: false,
            plugin: None,
            pdc: PdcDelay::new(),
            bus: 0,
            vca: None,
            sidechain_from: None,
            sidechain_amount_db: 0.0,
            sidechain_threshold_db: -30.0,
            sidechain_ducker: Ducker::new(engine_sample_rate),
            last_out_l: 0.0,
            last_out_r: 0.0,
        }
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        self.pan_l = ((1.0 - self.pan) * 0.5).sqrt();
        self.pan_r = ((1.0 + self.pan) * 0.5).sqrt();
    }
}

#[derive(Clone)]
pub struct AuditionVoice {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: f32,
    pub volume: f32,
    pub pitch_ratio: f32,
    pub time_stretch_ratio: f32,
    pub is_reverse: bool,
    pub loop_playback: bool,
    pub play_pos_samples: f32,
    pub is_playing: bool,
    /// Pitch-preserving time-stretch state (used when `time_stretch_ratio != 1`
    /// and playback is forward).
    pub wsola: Wsola,
}

/// A polyphonic one-shot voice that streams a preloaded WAV sample
/// (Channel Rack steps, drum machines & melodic sample playback).
pub struct SampleVoice {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub src_sample_rate: f32,
    /// Playback cursor in *source* sample frames.
    pub pos: f32,
    /// Sample-frames advanced per output frame (pitch ratio * resample).
    pub step: f32,
    /// Inclusive source-frame range this voice may play within (slice/chop).
    pub start_frame: f32,
    pub end_frame: f32,
    pub volume: f32,
    pub pan_l: f32,
    pub pan_r: f32,
    pub reverse: bool,
    /// Short anti-click fade-in, measured in output frames.
    pub attack_frames: u32,
    pub frames_done: u32,
    pub active: bool,
}

impl SampleVoice {
    pub fn new() -> Self {
        Self {
            left: Arc::new(Vec::new()),
            right: Arc::new(Vec::new()),
            src_sample_rate: 44100.0,
            pos: 0.0,
            step: 1.0,
            start_frame: 0.0,
            end_frame: 0.0,
            volume: 1.0,
            pan_l: 1.0,
            pan_r: 1.0,
            reverse: false,
            attack_frames: 0,
            frames_done: 0,
            active: false,
        }
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active && !self.left.is_empty()
    }
}

/// A note waiting to be triggered by the scheduler (chord strum/arpeggio).
pub struct ScheduledNote {
    pub samples_until: u32,
    pub note: u8,
    pub freq: f32,
    pub velocity: f32,
}

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

    pub fn handle_command(&mut self, cmd: AudioCommand) {
        let significant = matches!(
            &cmd,
            AudioCommand::StopAll
                | AudioCommand::SetSongPlayback(_)
                | AudioCommand::SetMasterVolume(_)
                | AudioCommand::ClearAllStemTracks
                | AudioCommand::LoadStemTrack { .. }
                | AudioCommand::SetStemTrackState { .. }
                | AudioCommand::SetStemTrackRegions { .. }
                | AudioCommand::SetStemTrackRouting { .. }
                | AudioCommand::SetBusState { .. }
                | AudioCommand::SetVcaState { .. }
                | AudioCommand::SetTrackEq { .. }
                | AudioCommand::SetTrackMix { .. }
                | AudioCommand::SetRemixFx { .. }
                | AudioCommand::SetTapeStop { .. }
                | AudioCommand::SeekSongPosition(_)
        );
        if significant {
            // NOTE: never print `cmd` via {:?} here – LoadStemTrack contains
            // Arc<Vec<f32>> whose Debug dumps every PCM sample (multi-GB logs).
            self.dbg_state("ENGINE", &format!("<< {}", describe_cmd(&cmd)));
        }
        match cmd {
            AudioCommand::NoteOn { note, freq, velocity } => {
                let mut target_idx = None;
                for (i, voice) in self.voices.iter().enumerate() {
                    if voice.is_active() && voice.note == note {
                        target_idx = Some(i);
                        break;
                    }
                }
                if target_idx.is_none() {
                    for (i, voice) in self.voices.iter().enumerate() {
                        if !voice.is_active() {
                            target_idx = Some(i);
                            break;
                        }
                    }
                }
                let idx = target_idx.unwrap_or(0);
                self.voices[idx].trigger(note, freq, velocity, self.sample_rate);
            }
            AudioCommand::NoteOnDelayed { note, freq, velocity, delay_samples } => {
                // Samma kö som StrumChord. Fördröjning 0 betyder "nästa sample",
                // vilket i praktiken är direkt — så en not utan mikro-tajming
                // låter som förut.
                self.scheduled_notes.push(ScheduledNote {
                    samples_until: delay_samples,
                    note,
                    freq,
                    velocity,
                });
                if self.scheduled_notes.len() > 256 {
                    let overflow = self.scheduled_notes.len() - 256;
                    self.scheduled_notes.drain(0..overflow);
                }
            }
            AudioCommand::NoteOff { note } => {
                for voice in &mut self.voices {
                    if voice.is_active() && voice.note == note {
                        voice.release();
                    }
                }
            }
            AudioCommand::StrumChord { notes, velocity, start_samples, spread_samples, mode } => {
                let mut ordered = notes;
                match mode {
                    2 => ordered.reverse(),
                    3 => {
                        let n = ordered.len();
                        if n > 1 {
                            let mut seed = (self.song_time_samples as u32) ^ 0x9E37_79B9;
                            for i in (1..n).rev() {
                                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                                let j = (seed as usize) % (i + 1);
                                ordered.swap(i, j);
                            }
                        }
                    }
                    _ => {}
                }
                let step = if mode == 0 { 0 } else { spread_samples };
                for (i, note) in ordered.into_iter().enumerate() {
                    let freq = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
                    let delay = start_samples.saturating_add(step.saturating_mul(i as u32));
                    self.scheduled_notes.push(ScheduledNote {
                        samples_until: delay,
                        note,
                        freq,
                        velocity,
                    });
                }
                // Hard cap so a stuck stream of commands can never grow forever.
                if self.scheduled_notes.len() > 256 {
                    let overflow = self.scheduled_notes.len() - 256;
                    self.scheduled_notes.drain(0..overflow);
                }
            }
            AudioCommand::TriggerDrum(drum_type) => {
                let slot = match drum_type {
                    DrumType::Kick => 0,
                    DrumType::Snare => 1,
                    DrumType::Clap => 2,
                    DrumType::HiHatClosed => 3,
                    DrumType::HiHatOpen => 4,
                    DrumType::Crash => 5,
                    DrumType::TomLow => 6,
                    DrumType::TomHigh => 7,
                    DrumType::MetronomeHigh => 8,
                    DrumType::MetronomeLow => 9,
                };
                self.drums[slot].trigger(drum_type);
            }
            AudioCommand::SetWaveform(wf) => {
                self.waveform = wf;
            }
            AudioCommand::SetAdsr(adsr) => {
                self.adsr = adsr;
            }
            AudioCommand::SetFilter(flt) => {
                self.filter_params = flt;
            }
            AudioCommand::SetFilterEnv { amount, adsr } => {
                self.filter_env_amount = amount.clamp(-6.0, 6.0);
                self.filter_env = adsr;
            }
            AudioCommand::SetDelay(dly) => {
                self.delay_params = dly;
            }
            AudioCommand::SetReverb(rvb) => {
                self.reverb_params = rvb;
            }
            AudioCommand::SetDrive(drv) => {
                self.drive = drv.clamp(1.0, 10.0);
            }
            AudioCommand::SetMasterFx(params) => {
                self.master_fx.set_params(params);
            }
            AudioCommand::SetTrackEq { track_index, settings } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.eq = settings;
                    track.eq_proc.set_settings(settings);
                }
            }
            AudioCommand::SetTrackMix { track_index, comp_threshold_db, comp_ratio, reverb_send, delay_send, pitch_semitones } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.comp_threshold_db = comp_threshold_db;
                    track.comp_ratio = comp_ratio;
                    track.reverb_send = reverb_send.clamp(0.0, 1.0);
                    track.delay_send = delay_send.clamp(0.0, 1.0);
                    track.pitch_semitones = pitch_semitones.clamp(-24.0, 24.0);
                    track.pitch_shifter.set_ratio(2.0_f32.powf(track.pitch_semitones / 12.0));
                    track.pitch_active = track.pitch_semitones.abs() > 0.005;
                    track.pitch_shifter.set_active(track.pitch_active);
                }
            }
            AudioCommand::SetRemixFx { mode, bpm } => {
                self.remix_fx.set(mode, bpm);
            }
            AudioCommand::SetTapeStop { active } => {
                self.tape_stop.set_active(active);
            }
            AudioCommand::SetPatcherGraph(spec) => {
                self.patcher_spec = Some(spec.clone());
                self.patcher = Some(PatchProcessor::new(&spec, self.sample_rate));
            }
            AudioCommand::SetPatcherEnabled(enabled) => {
                self.patcher_enabled = enabled;
                if enabled && self.patcher.is_none() {
                    if let Some(spec) = &self.patcher_spec {
                        self.patcher = Some(PatchProcessor::new(spec, self.sample_rate));
                    }
                }
            }
            AudioCommand::PatcherNoteOn { freq, velocity } => {
                if let Some(p) = &mut self.patcher {
                    p.note_on(freq, velocity);
                }
            }
            AudioCommand::PatcherNoteOff => {
                if let Some(p) = &mut self.patcher {
                    p.note_off();
                }
            }
            AudioCommand::LoadPreset(preset) => {
                let (wf, adsr, flt) = preset.settings();
                self.waveform = wf;
                self.adsr = adsr;
                self.filter_params = flt;
                for voice in &mut self.voices {
                    voice.filter.reset();
                }
            }
            AudioCommand::SetMasterVolume(vol) => {
                self.master_volume = vol.clamp(0.0, 1.0);
            }
            AudioCommand::StopAll => {
                for voice in &mut self.voices {
                    voice.reset();
                }
                for drum in &mut self.drums {
                    drum.reset();
                }
                for sv in &mut self.sample_voices {
                    sv.active = false;
                }
                if let Some(ref mut aud) = self.audition {
                    aud.is_playing = false;
                }
                self.song_playing = false;
            }
            AudioCommand::SetTrackPlugin { track_index, insert } => {
                if insert.is_some() {
                    while self.stem_tracks.len() <= track_index {
                        self.stem_tracks.push(StemVoiceTrack::new(
                            Arc::new(Vec::new()),
                            Arc::new(Vec::new()),
                            self.sample_rate,
                            1.0,
                            0.0,
                            0.0,
                            self.sample_rate,
                        ));
                    }
                }
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.plugin = insert;
                    track.pdc.set_delay(0);
                }
            }
            AudioCommand::SetPluginParameter { track_index, param_id, value } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index)
                    && let Some(plugin) = &mut track.plugin
                {
                    plugin.set_parameter(param_id, value);
                }
            }
            AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
                let track = StemVoiceTrack::new(left, right, sample_rate, volume, pan, start_time_secs, self.sample_rate);
                if track_index < self.stem_tracks.len() {
                    let eq = self.stem_tracks[track_index].eq;
                    let old = &mut self.stem_tracks[track_index];
                    let (ct, cr, rs, ds, ps, pa) = (old.comp_threshold_db, old.comp_ratio, old.reverb_send, old.delay_send, old.pitch_semitones, old.pitch_active);
                    // Preserve a live plugin insert (and its PDC line) across reloads.
                    let plugin = old.plugin.take();
                    let pdc = std::mem::replace(&mut old.pdc, PdcDelay::new());
                    let mut new_track = track;
                    new_track.eq = eq;
                    new_track.eq_proc.set_settings(eq);
                    new_track.comp_threshold_db = ct;
                    new_track.comp_ratio = cr;
                    new_track.reverb_send = rs;
                    new_track.delay_send = ds;
                    new_track.pitch_semitones = ps;
                    new_track.pitch_shifter.set_ratio(2.0_f32.powf(ps / 12.0));
                    new_track.pitch_shifter.set_active(pa);
                    new_track.pitch_active = pa;
                    new_track.plugin = plugin;
                    new_track.pdc = pdc;
                    self.stem_tracks[track_index] = new_track;
                } else {
                    while self.stem_tracks.len() < track_index {
                        self.stem_tracks.push(StemVoiceTrack::new(
                            Arc::new(Vec::new()),
                            Arc::new(Vec::new()),
                            self.sample_rate,
                            1.0,
                            0.0,
                            0.0,
                            self.sample_rate,
                        ));
                    }
                    self.stem_tracks.push(track);
                }
                self.has_stem_solo = self.stem_tracks.iter().any(|t| t.solo);
            }
            AudioCommand::ClearAllStemTracks => {
                self.stem_tracks.clear();
                self.has_stem_solo = false;
            }
            AudioCommand::SetStemTrackState { track_index, volume, pan, muted, solo } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.volume = volume;
                    track.set_pan(pan);
                    track.muted = muted;
                    track.solo = solo;
                    self.has_stem_solo = self.stem_tracks.iter().any(|t| t.solo);
                }
            }
            AudioCommand::SetStemTrackRegions { track_index, regions } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.regions = regions;
                }
            }
            AudioCommand::SetStemTrackRouting { track_index, bus, vca } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.bus = bus.min(NUM_BUSES - 1);
                    track.vca = vca.filter(|&v| v < NUM_VCAS);
                }
            }
            AudioCommand::SetStemTrackSidechain { track_index, from, amount_db, threshold_db } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    // Ett `from` som pekar på spåret självt är ingen sidokedja —
                    // det är en slinga, och den släpps inte in här. Ett `from`
                    // utanför listan kan bli giltigt senare (spår läggs till), så
                    // det sparas och filtreras vid användning.
                    track.sidechain_from = from.filter(|&k| k != track_index);
                    track.sidechain_amount_db = amount_db.clamp(0.0, 60.0);
                    track.sidechain_threshold_db = threshold_db.clamp(-80.0, 0.0);
                    track.sidechain_ducker.reset();
                }
            }
            AudioCommand::SetBusState { bus, volume, muted, solo } => {
                let b = bus.min(NUM_BUSES - 1);
                self.bus_volume[b] = volume.max(0.0);
                self.bus_muted[b] = muted;
                self.bus_solo[b] = solo;
            }
            AudioCommand::SetVcaState { vca, volume, muted, solo } => {
                let v = vca.min(NUM_VCAS - 1);
                self.vca_volume[v] = volume.max(0.0);
                self.vca_muted[v] = muted;
                self.vca_solo[v] = solo;
            }
            AudioCommand::SeekSongPosition(secs) => {
                self.song_time_samples = (secs.max(0.0) * self.sample_rate) as usize;
                for track in &mut self.stem_tracks {
                    track.pitch_shifter.reset();
                }
            }
            AudioCommand::SetSongPlayback(playing) => {
                self.song_playing = playing;
                for track in &mut self.stem_tracks {
                    track.pitch_shifter.reset();
                }
            }
            AudioCommand::PlayAudition {
                left,
                right,
                sample_rate,
                volume,
                pitch_ratio,
                time_stretch_ratio,
                is_reverse,
                loop_playback,
            } => {
                let start_pos = if is_reverse {
                    (left.len() as f32 - 1.0).max(0.0)
                } else {
                    0.0
                };
                self.audition = Some(AuditionVoice {
                    left,
                    right,
                    sample_rate,
                    volume: volume.max(0.0),
                    pitch_ratio: pitch_ratio.max(0.05),
                    time_stretch_ratio: time_stretch_ratio.max(0.05),
                    is_reverse,
                    loop_playback,
                    play_pos_samples: start_pos,
                    is_playing: true,
                    wsola: Wsola::new(sample_rate),
                });
            }
            AudioCommand::TriggerSampleVoice {
                left,
                right,
                sample_rate,
                base_note,
                note,
                pitch_semitones,
                pitch_cents,
                velocity,
                volume,
                reverse,
                start01,
                end01,
            } => {
                if left.is_empty() {
                    return;
                }
                let len_f = left.len() as f32;
                let start_f = (start01.clamp(0.0, 1.0) * len_f).floor();
                let end_f = ((end01.clamp(0.0, 1.0) * len_f).floor()).max(start_f + 1.0);
                let semis = note as f32 - base_note as f32 + pitch_semitones as f32 + pitch_cents / 100.0;
                let ratio = (semis / 12.0).exp2().clamp(0.1, 8.0);
                let step = (sample_rate as f32 / self.sample_rate) * ratio;

                // Pick the oldest/last used voice slot so retriggers never block.
                let idx = self.sample_voice_cursor % MAX_SAMPLE_VOICES;
                self.sample_voice_cursor = (self.sample_voice_cursor + 1) % MAX_SAMPLE_VOICES;
                let v = &mut self.sample_voices[idx];
                v.left = left;
                v.right = right;
                v.src_sample_rate = sample_rate as f32;
                v.step = step.max(0.0001);
                v.reverse = reverse;
                v.start_frame = start_f;
                v.end_frame = end_f.min(len_f);
                if reverse {
                    v.pos = v.end_frame;
                } else {
                    v.pos = v.start_frame;
                }
                v.volume = (volume * velocity).clamp(0.0, 1.5);
                let p: f32 = 0.0; // pan handled on the channel strip in future
                v.pan_l = ((1.0 - p) * 0.5).sqrt();
                v.pan_r = ((1.0 + p) * 0.5).sqrt();
                v.attack_frames = ((self.sample_rate * 0.0015) as u32).max(1);
                v.frames_done = 0;
                v.active = true;
            }
            AudioCommand::StopAudition => {
                if let Some(ref mut aud) = self.audition {
                    aud.is_playing = false;
                }
            }
            AudioCommand::SetMonitorRing { ring } => {
                self.monitor_ring = Some(ring);
                self.monitor_buf.clear();
                self.monitor_idx = 0;
            }
            AudioCommand::SetMonitorLevel(level) => {
                self.monitor_level = level.clamp(0.0, 1.0);
            }
            AudioCommand::SetAuditionParams {
                volume,
                pitch_ratio,
                time_stretch_ratio,
            } => {
                if let Some(ref mut aud) = self.audition {
                    aud.volume = volume.max(0.0);
                    aud.pitch_ratio = pitch_ratio.max(0.05);
                    aud.time_stretch_ratio = time_stretch_ratio.max(0.05);
                }
            }
        }
        if significant {
            self.dbg_state("ENGINE", ">> handled");
        }
    }

    #[inline(always)]
    pub fn process_stereo(&mut self) -> (f32, f32) {
        // 0. Fire any due scheduled chord/strum notes.
        if !self.scheduled_notes.is_empty() {
            let mut i = 0;
            while i < self.scheduled_notes.len() {
                if self.scheduled_notes[i].samples_until == 0 {
                    let ev = self.scheduled_notes.swap_remove(i);
                    let mut target = None;
                    for (vi, voice) in self.voices.iter().enumerate() {
                        if voice.is_active() && voice.note == ev.note {
                            target = Some(vi);
                            break;
                        }
                    }
                    if target.is_none() {
                        for (vi, voice) in self.voices.iter().enumerate() {
                            if !voice.is_active() {
                                target = Some(vi);
                                break;
                            }
                        }
                    }
                    let idx = target.unwrap_or(0);
                    self.voices[idx].trigger(ev.note, ev.freq, ev.velocity, self.sample_rate);
                } else {
                    self.scheduled_notes[i].samples_until -= 1;
                    i += 1;
                }
            }
        }

        let mut mixed = 0.0;
        let mut active_count = 0;

        // 1. Synthesizer voices (each with its own envelope + filter state)
        for voice in &mut self.voices {
            if voice.is_active() {
                mixed += voice.next_sample(
                    self.waveform,
                    &self.adsr,
                    &self.filter_params,
                    &self.filter_env,
                    self.filter_env_amount,
                );
                active_count += 1;
            }
        }

        if active_count > 1 {
            mixed *= 1.0 / (1.0 + 0.25 * (active_count as f32 - 1.0));
        }

        // 2. Drive / Saturation on Synth
        if self.drive > 1.05 {
            mixed = (mixed * self.drive).tanh() / (self.drive * 0.7 + 0.3);
        }

        // 3. Per-voice resonant lowpass is applied inside each voice; the synth
        //    bus itself is no longer filtered globally.
        let synth_out = mixed;

        // 4. Drum bus
        let mut drum_mix = 0.0;
        for drum in &mut self.drums {
            if drum.active {
                drum_mix += drum.next_sample();
            }
        }

        // 5. WAV one-shot sample voices (Channel Rack real samples)
        let mut sample_mix = 0.0;
        for sv in &mut self.sample_voices {
            if !sv.is_active() {
                continue;
            }
            let len = sv.left.len();
            if len == 0 {
                sv.active = false;
                continue;
            }
            if (sv.reverse && sv.pos <= sv.start_frame) || (!sv.reverse && sv.pos >= sv.end_frame) {
                sv.active = false;
                continue;
            }
            let idx0 = sv.pos.floor() as usize;
            let frac = sv.pos - idx0 as f32;
            if idx0 >= len {
                sv.active = false;
                continue;
            }
            let idx1 = (idx0 + 1).min(len - 1);
            let l0 = sv.left[idx0];
            let l1 = sv.left[idx1];
            let r0 = if idx0 < sv.right.len() { sv.right[idx0] } else { l0 };
            let r1 = if idx1 < sv.right.len() { sv.right[idx1] } else { l1 };
            let mut s_l = l0 + (l1 - l0) * frac;
            let mut s_r = r0 + (r1 - r0) * frac;
            s_l *= sv.pan_l;
            s_r *= sv.pan_r;
            let mut g = sv.volume;
            if sv.frames_done < sv.attack_frames {
                g *= sv.frames_done as f32 / sv.attack_frames as f32;
            }
            sample_mix += (s_l + s_r) * 0.5 * g;

            if sv.reverse {
                sv.pos -= sv.step;
            } else {
                sv.pos += sv.step;
            }
            sv.frames_done += 1;
        }

        let combined = synth_out + drum_mix + sample_mix;

        // 5. Reverb FX
        let rev_out = self.reverb.process(combined, &self.reverb_params);

        // 6. Stereo Ping-Pong Delay FX
        let (del_l, del_r) = self.delay.process(rev_out, rev_out, &self.delay_params);

        // 6b. Plug-in delay compensation: find the largest insert latency so
        //     every bus can be aligned to it (Fas 4.2). Only relevant while the
        //     song is playing, so live monitoring keeps its low latency.
        let max_plugin_latency = if self.song_playing {
            self.stem_tracks
                .iter()
                .map(|t| {
                    let plugin = t
                        .plugin
                        .as_ref()
                        .map(|p| p.latency_frames())
                        .unwrap_or(0);
                    let pitch = if t.pitch_active {
                        t.pitch_shifter.latency_frames()
                    } else {
                        0
                    };
                    plugin + pitch
                })
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        self.pdc_bus.set_delay(max_plugin_latency);
        let (del_l, del_r) = self.pdc_bus.process(del_l, del_r);

        // 7. Multi-Track Stem Audio Streaming (with Region Slicing & Fades)
        let mut stem_mix_l = 0.0;
        let mut stem_mix_r = 0.0;

        if self.song_playing && !self.stem_tracks.is_empty() {
            // Copy the group state so the loop can borrow `stem_tracks` mutably
            // (Fas 5.2). A track is soloed if its own solo, its bus's solo or its
            // VCA's solo is on; a track is silenced if its own, bus or VCA mute
            // is on. Group gain is applied post-fader, just before the master.
            let bus_volume = self.bus_volume;
            let bus_muted = self.bus_muted;
            let bus_solo = self.bus_solo;
            let vca_volume = self.vca_volume;
            let vca_muted = self.vca_muted;
            let vca_solo = self.vca_solo;
            let has_solo = self.has_stem_solo
                || bus_solo.iter().any(|&s| s)
                || vca_solo.iter().any(|&s| s);
            let current_time_sec = self.song_time_samples as f32 / self.sample_rate;
            // Sidokedjor (Fas 8.3): nyckelsignalerna är varje spårs senaste
            // utgångssample. De kopieras hit av samma skäl som gruppläget ovan —
            // loopen lånar `stem_tracks` mutabelt och kan inte läsa ett annat spår.
            let key_taps: Vec<(f32, f32)> = self
                .stem_tracks
                .iter()
                .map(|t| (t.last_out_l, t.last_out_r))
                .collect();

            for (track_idx, track) in self.stem_tracks.iter_mut().enumerate() {
                let bus = track.bus.min(NUM_BUSES - 1);
                let vca = track.vca.filter(|&v| v < NUM_VCAS);
                let group_soloed = bus_solo[bus] || vca.map(|v| vca_solo[v]).unwrap_or(false);
                let group_muted = bus_muted[bus] || vca.map(|v| vca_muted[v]).unwrap_or(false);
                let audible = if has_solo {
                    track.solo || group_soloed
                } else {
                    !track.muted && !group_muted
                };
                if !audible || track.left.is_empty() {
                    // Keep an engaged shifter primed with silence so that it is
                    // continuous (and latency-aligned) the moment the track is
                    // heard again.
                    if track.pitch_active && !track.left.is_empty() {
                        track.pitch_shifter.process(0.0, 0.0);
                    }
                    continue;
                }
                let group_gain =
                    bus_volume[bus] * vca.map(|v| vca_volume[v]).unwrap_or(1.0);

                let pan_l = track.pan_l;
                let pan_r = track.pan_r;
                let mut track_l = 0.0_f32;
                let mut track_r = 0.0_f32;

                if !track.regions.is_empty() {
                    // Play defined audio regions/slices
                    for region in &track.regions {
                        if region.muted {
                            continue;
                        }
                        let r_end = region.start_time_secs + region.length_secs;
                        if current_time_sec >= region.start_time_secs && current_time_sec < r_end {
                            let rel_time = current_time_sec - region.start_time_secs;
                            let mut env = 1.0_f32;
                            if region.fade_in_sec > 0.001 && rel_time < region.fade_in_sec {
                                env *= (rel_time / region.fade_in_sec).clamp(0.0, 1.0);
                            }
                            let time_left = r_end - current_time_sec;
                            if region.fade_out_sec > 0.001 && time_left < region.fade_out_sec {
                                env *= (time_left / region.fade_out_sec).clamp(0.0, 1.0);
                            }

                            let sample_pos_sec = if region.loop_length_secs > 0.02 {
                                let loop_dur = region.loop_length_secs;
                                let eff_time = (region.sample_offset_sec + rel_time) % loop_dur;
                                if region.is_reverse {
                                    (loop_dur - eff_time).max(0.0)
                                } else {
                                    eff_time
                                }
                            } else {
                                if region.is_reverse {
                                    (region.length_secs - (region.sample_offset_sec + rel_time)).max(0.0)
                                } else {
                                    region.sample_offset_sec + rel_time
                                }
                            };
                            let sample_pos = (sample_pos_sec * track.sample_rate).max(0.0);
                            let idx0 = sample_pos.floor() as usize;
                            let frac = sample_pos - idx0 as f32;

                            if idx0 + 1 < track.left.len() {
                                let raw_l = track.left[idx0] + (track.left[idx0 + 1] - track.left[idx0]) * frac;
                                let raw_r = if idx0 + 1 < track.right.len() {
                                    track.right[idx0] + (track.right[idx0 + 1] - track.right[idx0]) * frac
                                } else {
                                    raw_l
                                };
                                let g = track.volume * region.gain * env;
                                track_l += raw_l * g * pan_l;
                                track_r += raw_r * g * pan_r;
                            } else if idx0 < track.left.len() {
                                let raw_l = track.left[idx0];
                                let raw_r = if idx0 < track.right.len() { track.right[idx0] } else { raw_l };
                                let g = track.volume * region.gain * env;
                                track_l += raw_l * g * pan_l;
                                track_r += raw_r * g * pan_r;
                            }
                        }
                    }
                } else {
                    // Fallback to full track streaming
                    let track_rel_time = current_time_sec - track.start_time_secs;
                    if track_rel_time >= 0.0 {
                        let sample_pos = (track_rel_time * track.sample_rate).max(0.0);
                        let idx0 = sample_pos.floor() as usize;
                        let frac = sample_pos - idx0 as f32;

                        if idx0 + 1 < track.left.len() {
                            let raw_l = track.left[idx0] + (track.left[idx0 + 1] - track.left[idx0]) * frac;
                            let raw_r = if idx0 + 1 < track.right.len() {
                                track.right[idx0] + (track.right[idx0 + 1] - track.right[idx0]) * frac
                            } else {
                                raw_l
                            };
                            track_l += raw_l * track.volume * pan_l;
                            track_r += raw_r * track.volume * pan_r;
                        } else if idx0 < track.left.len() {
                            let raw_l = track.left[idx0];
                            let raw_r = if idx0 < track.right.len() { track.right[idx0] } else { raw_l };
                            track_l += raw_l * track.volume * pan_l;
                            track_r += raw_r * track.volume * pan_r;
                        }
                    }
                }

                // Real-time formant-preserving pitch shift (always run when
                // engaged, feeding silence through on empty frames so the
                // grain pipeline never stalls).
                if track.pitch_active {
                    let (pl, pr) = track.pitch_shifter.process(track_l, track_r);
                    track_l = pl;
                    track_r = pr;
                }

                // Per-track 3-band parametric EQ
                let (eq_l, eq_r) = track.eq_proc.process(track_l, track_r);
                let (mut tl, mut tr) = (eq_l, eq_r);

                // Per-track compressor (channel strip dynamics)
                if track.comp_ratio > 1.0 {
                    let params = CompressorParams {
                        threshold_db: track.comp_threshold_db,
                        ratio: track.comp_ratio,
                        attack_ms: 12.0,
                        release_ms: 140.0,
                        makeup_db: 0.0,
                    };
                    let (cl, cr) = track.comp.process(tl, tr, &params);
                    tl = cl;
                    tr = cr;
                }

                // Per-track aux sends: 100% wet reverb/delay scaled by send amount.
                if track.reverb_send > 0.0001 {
                    let wet = track.reverb.process((tl + tr) * 0.5, &ReverbParams {
                        room_size: 0.65,
                        damping: 0.4,
                        mix: 1.0,
                    });
                    tl += wet * track.reverb_send;
                    tr += wet * track.reverb_send;
                }
                if track.delay_send > 0.0001 {
                    let (dl, dr) = track.delay.process(tl, tr, &DelayParams {
                        time_ms: 350.0,
                        feedback: 0.35,
                        mix: 1.0,
                    });
                    tl += dl * track.delay_send;
                    tr += dr * track.delay_send;
                }

                // Optional CLAP insert, then PDC-align this track to the
                // project's maximum plugin latency.
                let mut track_latency = if track.pitch_active {
                    track.pitch_shifter.latency_frames()
                } else {
                    0
                };
                if let Some(plugin) = &mut track.plugin {
                    let (pl, pr) = plugin.process_sample(tl, tr);
                    tl = pl;
                    tr = pr;
                    track_latency += plugin.latency_frames();
                }
                track.pdc.set_delay(max_plugin_latency.saturating_sub(track_latency));
                let (mut tl, mut tr) = track.pdc.process(tl, tr);

                // Sidokedjan duckar spårets **eget** ljud, efter dess kedja: det är
                // där en kompressor med extern nyckel hade suttit, och det är det
                // som hörs. Nyckeln är key-spårets senaste utgång (se `Ducker` för
                // varför den är som mest ett sample gammal).
                if let Some(key) = track
                    .sidechain_from
                    .filter(|&k| k != track_idx && k < key_taps.len())
                {
                    let (kl, kr) = key_taps[key];
                    let gain = track.sidechain_ducker.process(
                        kl,
                        kr,
                        track.sidechain_threshold_db,
                        track.sidechain_amount_db,
                    );
                    tl *= gain;
                    tr *= gain;
                }
                track.last_out_l = tl;
                track.last_out_r = tr;

                stem_mix_l += tl * group_gain;
                stem_mix_r += tr * group_gain;
            }

            self.song_time_samples += 1;
        }

        // 8. Isolated Audition Audio Playback (Vocal Studio & Sample Previews)
        if let Some(ref mut aud) = self.audition {
            if aud.is_playing && !aud.left.is_empty() {
                let len = aud.left.len();
                let sr_ratio = aud.sample_rate / self.sample_rate;
                let pr = aud.pitch_ratio.max(0.05);
                let tsr = aud.time_stretch_ratio.max(0.05);
                let use_wsola = !aud.is_reverse && (tsr - 1.0).abs() > 1e-3;

                if use_wsola {
                    // Pitch-preserving time-stretch: WSOLA at ratio tsr*pr, then
                    // resample by sr_ratio*pr to convert rate and apply pitch.
                    aud.wsola.set_ratio(tsr * sr_ratio * pr);
                    let step = sr_ratio * pr;
                    let (l, r) = aud
                        .wsola
                        .next_resampled(&aud.left, &aud.right, aud.loop_playback, step);
                    stem_mix_l += l * aud.volume;
                    stem_mix_r += r * aud.volume;
                    if aud.wsola.is_finished() && aud.wsola.available() <= 0.0 {
                        aud.is_playing = false;
                    }
                } else {
                    let idx_f = aud.play_pos_samples;
                    let idx0 = idx_f.floor() as usize;
                    let idx1 = (idx0 + 1).min(len.saturating_sub(1));
                    let frac = idx_f - idx0 as f32;

                    if idx0 < len {
                        let l0 = aud.left[idx0];
                        let l1 = aud.left[idx1];
                        let r0 = if idx0 < aud.right.len() { aud.right[idx0] } else { l0 };
                        let r1 = if idx1 < aud.right.len() { aud.right[idx1] } else { l1 };

                        let raw_l = l0 + (l1 - l0) * frac;
                        let raw_r = r0 + (r1 - r0) * frac;

                        stem_mix_l += raw_l * aud.volume;
                        stem_mix_r += raw_r * aud.volume;

                        let speed = sr_ratio * pr * tsr;
                        if aud.is_reverse {
                            aud.play_pos_samples -= speed;
                            if aud.play_pos_samples < 0.0 {
                                if aud.loop_playback {
                                    aud.play_pos_samples = (len as f32 - 1.0).max(0.0);
                                } else {
                                    aud.is_playing = false;
                                }
                            }
                        } else {
                            aud.play_pos_samples += speed;
                            if aud.play_pos_samples >= len as f32 {
                                if aud.loop_playback {
                                    aud.play_pos_samples = 0.0;
                                } else {
                                    aud.is_playing = false;
                                }
                            }
                        }
                    } else {
                        aud.is_playing = false;
                    }
                }
            }
        }

        // 8b. Modular Patcher output (real DSP graph)
        if self.patcher_enabled
            && let Some(patch) = &mut self.patcher
        {
            let (pl, pr) = patch.process();
            stem_mix_l += pl;
            stem_mix_r += pr;
        }

        // 8c. Zero-latency microphone direct monitoring (post auto-tune).
        //     Drains the ring once per buffer and plays the mono signal on both
        //     channels, routed through the master bus so the limiter catches
        //     peaks. Nivån kommer från MONITOR-ratten (`SetMonitorLevel`) — utan
        //     egen nivå vore mastervolymen enda sättet att bryta en rundgång.
        if let Some(ring) = self.monitor_ring.as_ref() {
            if self.monitor_idx >= self.monitor_buf.len() {
                self.monitor_buf.clear();
                self.monitor_idx = 0;
                if let Ok(mut r) = ring.lock() {
                    if !r.is_empty() {
                        self.monitor_buf.append(&mut r);
                    }
                }
            }
            if self.monitor_idx < self.monitor_buf.len() {
                let m = self.monitor_buf[self.monitor_idx] * self.monitor_level;
                self.monitor_idx += 1;
                stem_mix_l += m;
                stem_mix_r += m;
            }
        }

        // 9. Master bus FX chain (EQ, compressor, de-esser, doubler, gate, filter, limiter)
        let (fx_l, fx_r) = self.master_fx.process(del_l + stem_mix_l, del_r + stem_mix_r);

        let out_l = (fx_l * self.master_volume).tanh();
        let out_r = (fx_r * self.master_volume).tanh();

        // DJ performance FX on the final master bus.
        let (out_l, out_r) = if self.remix_fx.is_active() {
            self.remix_fx.process(out_l, out_r)
        } else {
            (out_l, out_r)
        };
        let (out_l, out_r) = self.tape_stop.process(out_l, out_r);

        self.dbg_frames += 1;
        if self.dbg_frames % 48000 == 0 {
            let active_regions = self
                .stem_tracks
                .iter()
                .map(|t| t.regions.iter().filter(|r| !r.muted).count())
                .sum::<usize>();
            dbg_log(
                "HB",
                &format!(
                    "frames={} out=({:.4},{:.4}) song_playing={} t={:.2}s stems={} regions={} voices={}",
                    self.dbg_frames,
                    out_l,
                    out_r,
                    self.song_playing,
                    self.song_time_samples as f32 / self.sample_rate,
                    self.stem_tracks.len(),
                    active_regions,
                    self.voices.iter().filter(|v| v.is_active()).count(),
                ),
            );
        }

        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_voices(synth: &SynthEngine) -> usize {
        synth.voices.iter().filter(|v| v.is_active()).count()
    }

    #[test]
    fn a_delayed_note_fires_after_its_delay() {
        // Fas 6.4 steg 2: noten ska inte låta på steggränsen utan senare.
        let mut synth = SynthEngine::new(48_000.0);
        synth.handle_command(AudioCommand::NoteOnDelayed {
            note: 64,
            freq: 329.63,
            velocity: 0.8,
            delay_samples: 200,
        });
        assert_eq!(synth.scheduled_notes.len(), 1);

        synth.process_stereo(); // räknar ned till 199
        assert_eq!(active_voices(&synth), 0, "noten ska inte ha låtit än");

        for _ in 0..200 {
            synth.process_stereo();
        }
        assert_eq!(active_voices(&synth), 1, "noten ska klinga efter fördröjningen");
        assert!(synth.scheduled_notes.is_empty());
    }

    #[test]
    fn a_zero_delay_note_fires_immediately() {
        let mut synth = SynthEngine::new(48_000.0);
        synth.handle_command(AudioCommand::NoteOnDelayed {
            note: 60,
            freq: 261.63,
            velocity: 0.8,
            delay_samples: 0,
        });
        synth.process_stereo();
        assert_eq!(active_voices(&synth), 1);
    }

    #[test]
    fn strum_chord_schedules_notes_over_time() {
        let mut synth = SynthEngine::new(48_000.0);
        synth.handle_command(AudioCommand::StrumChord {
            notes: vec![60, 64, 67],
            velocity: 0.8,
            start_samples: 0,
            spread_samples: 100,
            mode: 1,
        });
        assert_eq!(synth.scheduled_notes.len(), 3);

        // First note fires on the first processed frame.
        synth.process_stereo();
        assert_eq!(active_voices(&synth), 1);

        // Advance far enough for the remaining strummed notes.
        for _ in 0..250 {
            synth.process_stereo();
        }
        assert_eq!(active_voices(&synth), 3);
        assert!(synth.scheduled_notes.is_empty());
    }

    #[test]
    fn strum_chord_block_mode_fires_together() {
        let mut synth = SynthEngine::new(48_000.0);
        synth.handle_command(AudioCommand::StrumChord {
            notes: vec![60, 64, 67, 71],
            velocity: 0.8,
            start_samples: 0,
            spread_samples: 500,
            mode: 0,
        });
        synth.process_stereo();
        assert_eq!(active_voices(&synth), 4);
    }

    #[test]
    fn filter_env_zero_amount_is_transparent() {
        let adsr = AdsrParams::default();
        let flt = FilterParams { cutoff: 1200.0, resonance: 2.0 };
        let env = AdsrParams { attack: 0.001, decay: 0.05, sustain: 0.0, release: 0.05 };
        let other_env = AdsrParams { attack: 0.2, decay: 0.9, sustain: 0.5, release: 1.0 };
        let mut a = Voice::new(48_000.0);
        let mut b = Voice::new(48_000.0);
        a.trigger(60, 261.63, 1.0, 48_000.0);
        b.trigger(60, 261.63, 1.0, 48_000.0);
        for _ in 0..500 {
            let sa = a.next_sample(Waveform::Saw, &adsr, &flt, &env, 0.0);
            let sb = b.next_sample(Waveform::Saw, &adsr, &flt, &other_env, 0.0);
            assert!(
                (sa - sb).abs() < 1e-6,
                "amount 0 must ignore the filter envelope shape"
            );
        }
    }

    #[test]
    fn filter_env_amount_changes_timbre() {
        let adsr = AdsrParams::default();
        let flt = FilterParams { cutoff: 400.0, resonance: 4.0 };
        let fenv = AdsrParams { attack: 0.001, decay: 0.2, sustain: 0.0, release: 0.2 };
        let mut dry = Voice::new(48_000.0);
        let mut wet = Voice::new(48_000.0);
        dry.trigger(60, 261.63, 1.0, 48_000.0);
        wet.trigger(60, 261.63, 1.0, 48_000.0);
        let mut diff = 0.0_f32;
        for _ in 0..2000 {
            let a = dry.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 0.0);
            let b = wet.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 4.0);
            diff += (a - b).abs();
        }
        assert!(diff > 1.0, "filter envelope should alter the timbre, diff={diff}");
    }

    #[test]
    fn voices_have_independent_filter_envelopes() {
        let fenv = AdsrParams { attack: 0.5, decay: 0.5, sustain: 1.0, release: 0.5 };
        let adsr = AdsrParams::default();
        let flt = FilterParams::default();
        let mut first = Voice::new(48_000.0);
        let mut second = Voice::new(48_000.0);
        first.trigger(60, 261.63, 1.0, 48_000.0);
        for _ in 0..2000 {
            first.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 2.0);
        }
        second.trigger(64, 329.63, 1.0, 48_000.0);
        let l1 = first.filter_env.current_level;
        let l2 = second.filter_env.current_level;
        assert!(
            l1 > l2,
            "earlier note's filter envelope must be further along: {l1} vs {l2}"
        );
    }

    use crate::audio::plugin_host_live::{
        DEFAULT_BLOCK_FRAMES, PluginInfo, PluginParameter, PluginProcessor,
    };

    struct SilentLatencyProcessor {
        info: PluginInfo,
        latency: u32,
    }

    impl PluginProcessor for SilentLatencyProcessor {
        fn backend(&self) -> &'static str {
            "Fake"
        }
        fn info(&self) -> &PluginInfo {
            &self.info
        }
        fn parameters(&self) -> &[PluginParameter] {
            &[]
        }
        fn latency_frames(&self) -> u32 {
            self.latency
        }
        fn process_stereo(&mut self, _left: &mut [f32], _right: &mut [f32]) {}
        fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
            false
        }
        fn reset(&mut self) {}
    }

    fn load_impulse_tracks(synth: &mut SynthEngine) {
        let impulse = Arc::new(vec![1.0_f32; 8]);
        let silent = Arc::new(vec![0.0_f32; 8]);
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: impulse.clone(),
            right: impulse,
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 1,
            left: silent.clone(),
            right: silent,
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        synth.handle_command(AudioCommand::SetSongPlayback(true));
    }

    #[test]
    fn pdc_aligns_non_plugin_tracks_to_plugin_latency() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        let insert = PluginInsert::new(
            Box::new(SilentLatencyProcessor {
                info: PluginInfo::default(),
                latency: 20,
            }),
            DEFAULT_BLOCK_FRAMES,
        );
        let expected = insert.latency_frames();
        synth.handle_command(AudioCommand::SetTrackPlugin {
            track_index: 0,
            insert: Some(insert),
        });
        synth.process_stereo();
        assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
        assert_eq!(synth.stem_tracks[1].pdc.delay(), expected);
    }

    #[test]
    fn pdc_is_a_passthrough_without_plugins() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        synth.process_stereo();
        assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
        assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
    }

    #[test]
    fn clearing_a_track_plugin_removes_its_pdc() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        let insert = PluginInsert::new(
            Box::new(SilentLatencyProcessor {
                info: PluginInfo::default(),
                latency: 12,
            }),
            DEFAULT_BLOCK_FRAMES,
        );
        synth.handle_command(AudioCommand::SetTrackPlugin {
            track_index: 0,
            insert: Some(insert),
        });
        synth.process_stereo();
        assert!(synth.stem_tracks[1].pdc.delay() > 0);
        synth.handle_command(AudioCommand::SetTrackPlugin {
            track_index: 0,
            insert: None,
        });
        synth.process_stereo();
        assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
        assert!(synth.stem_tracks[0].plugin.is_none());
    }

    #[test]
    fn pitch_shift_engages_shifter_and_compensates_latency() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        synth.handle_command(AudioCommand::SetTrackMix {
            track_index: 0,
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: 12.0,
        });
        assert!(synth.stem_tracks[0].pitch_active);
        assert!((synth.stem_tracks[0].pitch_semitones - 12.0).abs() < 1e-6);
        let latency = synth.stem_tracks[0].pitch_shifter.latency_frames();
        assert!(latency > 0, "engaged shifter must report latency");
        synth.process_stereo();
        // The shifted track defines the maximum latency; the other is delayed.
        assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
        assert_eq!(synth.stem_tracks[1].pdc.delay(), latency);
    }

    #[test]
    fn pitch_shift_zero_bypasses_shifter() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        synth.handle_command(AudioCommand::SetTrackMix {
            track_index: 0,
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: 0.0,
        });
        assert!(!synth.stem_tracks[0].pitch_active);
        synth.process_stereo();
        assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
        assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
    }

    #[test]
    fn pitch_shift_survives_stem_reload() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_tracks(&mut synth);
        synth.handle_command(AudioCommand::SetTrackMix {
            track_index: 0,
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: -5.0,
        });
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: Arc::new(vec![1.0_f32; 8]),
            right: Arc::new(vec![1.0_f32; 8]),
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        assert!(synth.stem_tracks[0].pitch_active);
        assert!((synth.stem_tracks[0].pitch_semitones + 5.0).abs() < 1e-6);
    }

    // -- Fas 5.2: sub-mix bussar & VCA-grupper -------------------------------

    fn route_track(synth: &mut SynthEngine, track: usize, bus: usize, vca: Option<usize>) {
        synth.handle_command(AudioCommand::SetStemTrackRouting {
            track_index: track,
            bus,
            vca,
        });
    }

    fn load_impulse_and_silence(synth: &mut SynthEngine) {
        // A 220 Hz tone (not DC): the master chain high-passes DC away, so a
        // constant buffer would read as silence regardless of group gain.
        let tone: Vec<f32> = (0..8192)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.8)
            .collect();
        let impulse = Arc::new(tone);
        let silent = Arc::new(vec![0.0_f32; 8192]);
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: impulse.clone(),
            right: impulse,
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 1,
            left: silent.clone(),
            right: silent,
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        synth.handle_command(AudioCommand::SetSongPlayback(true));
    }

    /// Sums |L|+|R| over `frames` stereo frames.
    fn output_energy(synth: &mut SynthEngine, frames: usize) -> f32 {
        let mut energy = 0.0_f32;
        for _ in 0..frames {
            let (l, r) = synth.process_stereo();
            energy += l.abs() + r.abs();
        }
        energy
    }
    #[test]
    fn bus_volume_scales_group_output() {
        let mut unity = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut unity);
        route_track(&mut unity, 0, 0, None);
        let full = output_energy(&mut unity, 2048);

        let mut halved = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut halved);
        route_track(&mut halved, 0, 0, None);
        halved.handle_command(AudioCommand::SetBusState {
            bus: 0,
            volume: 0.5,
            muted: false,
            solo: false,
        });
        let half = output_energy(&mut halved, 2048);

        assert!(full > 0.05, "expected audible bus output, got {full}");
        assert!(
            half > 0.0 && half < full,
            "bus gain must attenuate: {half} vs {full}"
        );
    }

    #[test]
    fn bus_mute_silences_group() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut synth);
        route_track(&mut synth, 0, 2, None);
        synth.handle_command(AudioCommand::SetBusState {
            bus: 2,
            volume: 1.0,
            muted: true,
            solo: false,
        });
        assert!(output_energy(&mut synth, 2048) < 1e-4, "muted bus must be silent");
    }

    #[test]
    fn bus_solo_isolates_other_buses() {
        let mut reference = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut reference);
        route_track(&mut reference, 0, 0, None);
        route_track(&mut reference, 1, 1, None);
        assert!(output_energy(&mut reference, 2048) > 0.05, "reference should be audible");

        let mut soloed = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut soloed);
        route_track(&mut soloed, 0, 0, None);
        route_track(&mut soloed, 1, 1, None);
        soloed.handle_command(AudioCommand::SetBusState {
            bus: 1,
            volume: 1.0,
            muted: false,
            solo: true,
        });
        assert!(
            output_energy(&mut soloed, 2048) < 1e-4,
            "soloing the silent bus must silence the non-soloed impulse bus"
        );
    }

    #[test]
    fn vca_volume_scales_group_output() {
        let mut unity = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut unity);
        route_track(&mut unity, 0, 0, Some(2));
        let full = output_energy(&mut unity, 2048);

        let mut halved = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut halved);
        route_track(&mut halved, 0, 0, Some(2));
        halved.handle_command(AudioCommand::SetVcaState {
            vca: 2,
            volume: 0.5,
            muted: false,
            solo: false,
        });
        let half = output_energy(&mut halved, 2048);

        assert!(full > 0.05, "expected audible VCA output, got {full}");
        assert!(
            half > 0.0 && half < full,
            "VCA gain must attenuate: {half} vs {full}"
        );
    }

    #[test]
    fn vca_mute_silences_group() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut synth);
        route_track(&mut synth, 0, 0, Some(1));
        synth.handle_command(AudioCommand::SetVcaState {
            vca: 1,
            volume: 1.0,
            muted: true,
            solo: false,
        });
        assert!(output_energy(&mut synth, 2048) < 1e-4, "muted VCA must be silent");
    }

    #[test]
    fn routing_clamps_out_of_range_assignments() {
        let mut synth = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut synth);
        route_track(&mut synth, 0, 99, Some(99));
        assert_eq!(synth.stem_tracks[0].bus, NUM_BUSES - 1);
        assert_eq!(synth.stem_tracks[0].vca, None);
        // Out-of-range bus/VCA state must clamp to the last group, not panic.
        synth.handle_command(AudioCommand::SetBusState {
            bus: 99,
            volume: 0.3,
            muted: false,
            solo: false,
        });
        synth.handle_command(AudioCommand::SetVcaState {
            vca: 99,
            volume: 0.4,
            muted: false,
            solo: false,
        });
        assert!((synth.bus_volume[NUM_BUSES - 1] - 0.3).abs() < 1e-6);
        assert!((synth.vca_volume[NUM_VCAS - 1] - 0.4).abs() < 1e-6);
    }

    // -- Fas 8.3: sidokedjor -------------------------------------------------

    /// Energin vid **en** frekvens (Goertzel). Behövs för sidokedjetestet: nyckeln
    /// och målet ligger på olika toner, och då mäter den här bara målet — annars
    /// hade nyckelns egen ton drunknat i mätningen.
    fn goertzel(buf: &[f32], freq: f32, sr: f32) -> f32 {
        let n = buf.len() as f32;
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in buf {
            let s0 = x + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).max(0.0)).sqrt() / n
    }

    /// Ett spår med en ren ton på `freq` i 3 s — samma form som
    /// `load_impulse_and_silence` använder, men på valfri frekvens (en
    /// likströmston hade high-passats bort) och lång nog för ett test som duckar,
    /// släpper och mäter efteråt.
    fn load_tone_track(synth: &mut SynthEngine, track_index: usize, freq: f32) {
        // 3 s: ett sidokedjetest hinner både ducka, släppa och mäta efteråt.
        let tone: Vec<f32> = (0..144_000)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin() * 0.6)
            .collect();
        let arc = Arc::new(tone);
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index,
            left: arc.clone(),
            right: arc,
            sample_rate: 48_000.0,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
    }

    fn render_left(synth: &mut SynthEngine, frames: usize) -> Vec<f32> {
        (0..frames).map(|_| synth.process_stereo().0).collect()
    }

    /// Två spår: nyckeln på 220 Hz (spår 0) och målet på 880 Hz (spår 1).
    fn sidechain_pair() -> SynthEngine {
        let mut synth = SynthEngine::new(48_000.0);
        load_tone_track(&mut synth, 0, 220.0);
        load_tone_track(&mut synth, 1, 880.0);
        synth.handle_command(AudioCommand::SetSongPlayback(true));
        synth
    }

    /// Duckarens egen matematik: öppen under tröskeln, ned mot `amount_db` över
    /// den, och tillbaka igen när nyckeln tystnar.
    #[test]
    fn a_ducker_stays_open_below_the_threshold_and_closes_above_it() {
        let mut d = Ducker::new(48_000.0);
        for _ in 0..4_800 {
            assert!((d.process(0.0, 0.0, -30.0, 12.0) - 1.0).abs() < 1e-6, "tyst nyckel ska inte ducka");
        }
        let mut gain = 1.0;
        for _ in 0..4_800 {
            gain = d.process(0.9, 0.9, -30.0, 12.0);
        }
        // −12 dB är 10^(−12/20) = 0.251. Mjuk övergång över 6 dB gör att en nyckel
        // 30 dB över tröskeln ligger vid full duckning.
        assert!(gain < 0.30 && gain > 0.20, "gainen ska ned mot −12 dB, fick {gain}");

        // Nyckeln tystnar → duckaren öppnar igen (release 120 ms).
        for _ in 0..96_000 {
            gain = d.process(0.0, 0.0, -30.0, 12.0);
        }
        assert!(gain > 0.99, "duckaren ska ha öppnat igen, fick {gain}");
    }

    /// Noll duckning ska vara samma sak som ingen duckning — gainen ska aldrig
    /// röra signalen.
    #[test]
    fn a_zero_amount_ducker_never_touches_the_signal() {
        let mut d = Ducker::new(48_000.0);
        for _ in 0..4_800 {
            assert!((d.process(1.0, 1.0, -30.0, 0.0) - 1.0).abs() < 1e-4);
        }
    }

    /// **Det som hörs:** spår 1 (880 Hz) duckas av spår 0 (220 Hz). Energin vid
    /// 880 Hz mäts med och utan sidokedja — nyckelns egen ton ligger på en annan
    /// frekvens, så den blandas inte in.
    #[test]
    fn a_sidechain_ducks_the_target_track() {
        let own = {
            let mut synth = sidechain_pair();
            let buf = render_left(&mut synth, 48_000);
            goertzel(&buf[9_600..], 880.0, 48_000.0)
        };
        let ducked = {
            let mut synth = sidechain_pair();
            synth.handle_command(AudioCommand::SetStemTrackSidechain {
                track_index: 1,
                from: Some(0),
                amount_db: 18.0,
                threshold_db: -30.0,
            });
            let buf = render_left(&mut synth, 48_000);
            goertzel(&buf[9_600..], 880.0, 48_000.0)
        };

        assert!(own > 0.0, "målspåret ska höras utan sidokedja");
        assert!(
            ducked < own * 0.5,
            "880 Hz ska ha dämpats av nyckeln: {own} → {ducked}"
        );
        assert!(ducked > 0.0, "dämpat, inte tystat");
        // Och nyckeln själv ska vara kvar på sin frekvens.
        let mut synth = sidechain_pair();
        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: Some(0),
            amount_db: 18.0,
            threshold_db: -30.0,
        });
        let buf = render_left(&mut synth, 48_000);
        assert!(goertzel(&buf[9_600..], 220.0, 48_000.0) > 0.01, "nyckeln ska höras som förut");
    }

    /// Ett `from` som pekar på spåret självt är en slinga och ignoreras — både när
    /// det sätts och om spårlistan ändras efteråt. Ett `from` utanför listan duckar
    /// ingen i stället för att få motorn att gå utanför bufferten.
    #[test]
    fn a_sidechain_never_points_at_the_track_itself_or_outside_the_list() {
        let mut synth = sidechain_pair();
        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 0,
            from: Some(0),
            amount_db: 12.0,
            threshold_db: -30.0,
        });
        assert_eq!(synth.stem_tracks[0].sidechain_from, None, "självreferens ska inte sparas");

        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: Some(99),
            amount_db: 12.0,
            threshold_db: -30.0,
        });
        assert_eq!(synth.stem_tracks[1].sidechain_from, Some(99));
        let buf = render_left(&mut synth, 4_800);
        assert!(buf.iter().all(|s| s.is_finite()), "utanför listan: ingen panik, inget NaN");
    }

    /// En sidokedja som **inte** är kopplad får inte färga signalen alls: samma
    /// motor, samma spår, en med ett uttryckligt `from: None` och en utan kommandot
    /// — utsignalerna ska vara bitvis lika.
    #[test]
    fn an_unset_sidechain_leaves_the_signal_untouched() {
        let mut plain = sidechain_pair();
        let mut unset = sidechain_pair();
        unset.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: None,
            amount_db: 24.0,
            threshold_db: -40.0,
        });

        let mut identical = true;
        for _ in 0..10_000 {
            let (a, _) = plain.process_stereo();
            let (b, _) = unset.process_stereo();
            if (a - b).abs() > 1e-9 {
                identical = false;
                break;
            }
        }
        assert!(identical, "utan kopplad sidokedja ska signalen vara orörd");
    }

    /// Och när en kopplad sidokedja tas bort ska nivån tillbaka. Här mäts **nivå**,
    /// inte bitvis likhet: master-kedjans dynamik har minne av den duckade
    /// perioden, så två motorer med olika historia kan inte jämföras sample för
    /// sample. Nivån är det användaren hör.
    #[test]
    fn removing_a_sidechain_lets_the_level_come_back() {
        // Referensen: samma spår, ingen sidokedja alls, samma mätfönster.
        let mut plain = sidechain_pair();
        let plain_buf = render_left(&mut plain, 48_000);
        let expected = goertzel(&plain_buf[9_600..24_000], 880.0, 48_000.0);

        let mut synth = sidechain_pair();
        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: Some(0),
            amount_db: 24.0,
            threshold_db: -40.0,
        });
        let ducked_buf = render_left(&mut synth, 48_000);
        let ducked = goertzel(&ducked_buf[9_600..24_000], 880.0, 48_000.0);

        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: None,
            amount_db: 24.0,
            threshold_db: -40.0,
        });
        // 0,5 s: duckarens release är 120 ms, och master-kedjan ska hinna med.
        // Räkningen måste stämma med källtonens längd (3 s = 144 000 samples):
        // 48 000 (duckat) + 24 000 (återhämtning) + 24 000 (mätning) = 96 000.
        let _ = render_left(&mut synth, 24_000);
        let after_buf = render_left(&mut synth, 24_000);
        let restored = goertzel(&after_buf, 880.0, 48_000.0);

        assert!(
            ducked < expected * 0.6,
            "före borttagningen ska nivån vara dämpad: {ducked} mot {expected}"
        );
        assert!(
            restored > expected * 0.8,
            "efter borttagningen ska nivån tillbaka: {restored} mot {expected}"
        );
    }
}
