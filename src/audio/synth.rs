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
use super::vocal_harmonizer::Wsola;

/// Debug logging to ~/Music/Sonix/audio_debug.log. Enabled when the
/// SONIX_AUDIO_DEBUG env var is set OR when the marker file
/// ~/Music/Sonix/debug_on exists (so normal desktop starts can log too).
fn dbg_enabled() -> bool {
    if std::env::var("SONIX_AUDIO_DEBUG").is_ok() {
        return true;
    }
    if let Ok(home) = std::env::var("HOME") {
        let marker = std::path::Path::new(&home).join("Music/Sonix/debug_on");
        return marker.exists();
    }
    false
}

fn dbg_log(tag: &str, msg: &str) {
    if !dbg_enabled() {
        return;
    }
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::Path::new(&home).join("Music/Sonix/audio_debug.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            let _ = writeln!(f, "[{}] {}: {}", std::process::id(), tag, msg);
        }
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
        AudioCommand::SetBusState { .. } => "SetBusState",
        AudioCommand::SetVcaState { .. } => "SetVcaState",
        AudioCommand::SeekSongPosition(_) => "SeekSongPosition",
        AudioCommand::SetSongPlayback(_) => "SetSongPlayback",
        AudioCommand::PlayAudition { .. } => "PlayAudition",
        AudioCommand::StopAudition => "StopAudition",
        AudioCommand::SetMonitorRing { .. } => "SetMonitorRing",
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
    pub pitch_ratio: f32,
    /// Optional CLAP insert on this track (Fas 4.2).
    pub plugin: Option<PluginInsert>,
    /// Delay line that aligns this track with the project's max plugin latency.
    pub pdc: PdcDelay,
    /// Sub-mix bus this track feeds (`0..NUM_BUSES`) — Fas 5.2.
    pub bus: usize,
    /// Optional VCA control group (`0..NUM_VCAS`) — Fas 5.2.
    pub vca: Option<usize>,
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
            pitch_ratio: 1.0,
            plugin: None,
            pdc: PdcDelay::new(),
            bus: 0,
            vca: None,
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
                    track.pitch_ratio = 2.0_f32.powf(pitch_semitones / 12.0);
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
                    let (ct, cr, rs, ds, pr) = (old.comp_threshold_db, old.comp_ratio, old.reverb_send, old.delay_send, old.pitch_ratio);
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
                    new_track.pitch_ratio = pr;
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
            }
            AudioCommand::SetSongPlayback(playing) => {
                self.song_playing = playing;
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
                .filter_map(|t| t.plugin.as_ref().map(|p| p.latency_frames()))
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

            for track in &mut self.stem_tracks {
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
                            let sample_pos = (sample_pos_sec * track.sample_rate * track.pitch_ratio).max(0.0);
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
                        let sample_pos = (track_rel_time * track.sample_rate * track.pitch_ratio).max(0.0);
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
                let track_latency = if let Some(plugin) = &mut track.plugin {
                    let (pl, pr) = plugin.process_sample(tl, tr);
                    tl = pl;
                    tr = pr;
                    plugin.latency_frames()
                } else {
                    0
                };
                track.pdc.set_delay(max_plugin_latency.saturating_sub(track_latency));
                let (tl, tr) = track.pdc.process(tl, tr);

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
        //     peaks and the master volume controls the level.
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
                let m = self.monitor_buf[self.monitor_idx];
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

    /// Sums |L|+|R| over `frames` stereo frames. The master chain ends in the
    /// tape-stop ring buffer, which currently reads a full buffer ahead, so the
    /// stem audio only reaches the output after ~32768 frames. Integrating over
    /// a window past that point is therefore required.
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
        let full = output_energy(&mut unity, 40_000);

        let mut halved = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut halved);
        route_track(&mut halved, 0, 0, None);
        halved.handle_command(AudioCommand::SetBusState {
            bus: 0,
            volume: 0.5,
            muted: false,
            solo: false,
        });
        let half = output_energy(&mut halved, 40_000);

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
        assert!(output_energy(&mut synth, 40_000) < 1e-4, "muted bus must be silent");
    }

    #[test]
    fn bus_solo_isolates_other_buses() {
        let mut reference = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut reference);
        route_track(&mut reference, 0, 0, None);
        route_track(&mut reference, 1, 1, None);
        assert!(output_energy(&mut reference, 40_000) > 0.05, "reference should be audible");

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
            output_energy(&mut soloed, 40_000) < 1e-4,
            "soloing the silent bus must silence the non-soloed impulse bus"
        );
    }

    #[test]
    fn vca_volume_scales_group_output() {
        let mut unity = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut unity);
        route_track(&mut unity, 0, 0, Some(2));
        let full = output_energy(&mut unity, 40_000);

        let mut halved = SynthEngine::new(48_000.0);
        load_impulse_and_silence(&mut halved);
        route_track(&mut halved, 0, 0, Some(2));
        halved.handle_command(AudioCommand::SetVcaState {
            vca: 2,
            volume: 0.5,
            muted: false,
            solo: false,
        });
        let half = output_energy(&mut halved, 40_000);

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
        assert!(output_energy(&mut synth, 40_000) < 1e-4, "muted VCA must be silent");
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
}
