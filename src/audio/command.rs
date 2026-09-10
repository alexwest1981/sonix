use super::drum::DrumType;
use super::effects::{DelayParams, ReverbParams};
use super::envelope::AdsrParams;
use super::filter::FilterParams;
use super::master_fx::{MasterFxParams, TrackEqSettings};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[allow(dead_code)]
impl Waveform {
    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Sine => "Sinus (Sine)",
            Waveform::Saw => "Sågtand (Saw)",
            Waveform::Square => "Fyrkant (Square)",
            Waveform::Triangle => "Triangel (Triangle)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preset {
    CleanPluck,
    WarmPad,
    AcidBass,
    ChiptuneLead,
    CosmicBrass,
}

#[allow(dead_code)]
impl Preset {
    pub fn settings(&self) -> (Waveform, AdsrParams, FilterParams) {
        match self {
            Preset::CleanPluck => (
                Waveform::Saw,
                AdsrParams { attack: 0.005, decay: 0.35, sustain: 0.1, release: 0.25 },
                FilterParams { cutoff: 3800.0, resonance: 1.8 },
            ),
            Preset::WarmPad => (
                Waveform::Saw,
                AdsrParams { attack: 0.45, decay: 0.5, sustain: 0.75, release: 0.9 },
                FilterParams { cutoff: 1400.0, resonance: 0.9 },
            ),
            Preset::AcidBass => (
                Waveform::Saw,
                AdsrParams { attack: 0.005, decay: 0.18, sustain: 0.15, release: 0.1 },
                FilterParams { cutoff: 750.0, resonance: 5.0 },
            ),
            Preset::ChiptuneLead => (
                Waveform::Square,
                AdsrParams { attack: 0.002, decay: 0.08, sustain: 0.7, release: 0.08 },
                FilterParams { cutoff: 12000.0, resonance: 0.7 },
            ),
            Preset::CosmicBrass => (
                Waveform::Triangle,
                AdsrParams { attack: 0.08, decay: 0.3, sustain: 0.65, release: 0.45 },
                FilterParams { cutoff: 4500.0, resonance: 2.5 },
            ),
        }
    }
}

use std::sync::Arc;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AudioCommand {
    NoteOn {
        note: u8,
        freq: f32,
        velocity: f32,
    },
    NoteOff {
        note: u8,
    },
    /// Schedules a chord/arpeggio: each note in `notes` is triggered after
    /// `spread_samples * i` samples. `mode`: 0 = Block, 1 = Up, 2 = Down,
    /// 3 = Random. Used by the Chord Matrix audition/strum.
    StrumChord {
        notes: Vec<u8>,
        velocity: f32,
        start_samples: u32,
        spread_samples: u32,
        mode: u8,
    },
    TriggerDrum(DrumType),
    SetWaveform(Waveform),
    SetAdsr(AdsrParams),
    SetFilter(FilterParams),
    /// Per-voice filter envelope: `amount` is the cutoff modulation depth in
    /// octaves (positive = brighter, negative = darker); `adsr` shapes the
    /// per-note sweep. Default amount 0 disables it.
    SetFilterEnv {
        amount: f32,
        adsr: AdsrParams,
    },
    SetDelay(DelayParams),
    SetReverb(ReverbParams),
    SetDrive(f32),
    SetMasterFx(MasterFxParams),
    SetTrackEq {
        track_index: usize,
        settings: TrackEqSettings,
    },
    /// Per-track dynamics, aux sends and pitch for the selected channel strip.
    SetTrackMix {
        track_index: usize,
        comp_threshold_db: f32,
        comp_ratio: f32,
        reverb_send: f32,
        delay_send: f32,
        pitch_semitones: f32,
    },
    SetRemixFx {
        mode: u8,
        bpm: f32,
    },
    SetTapeStop {
        active: bool,
    },
    LoadPreset(Preset),
    SetMasterVolume(f32),
    StopAll,
    // Multi-Track Stem Audio Streaming
    LoadStemTrack {
        track_index: usize,
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pan: f32,
        start_time_secs: f32,
    },
    ClearAllStemTracks,
    SetStemTrackState {
        track_index: usize,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    },
    SetStemTrackRegions {
        track_index: usize,
        regions: Vec<StemRegionPlayback>,
    },
    SeekSongPosition(f32),
    SetSongPlayback(bool),
    // Isolated Audition for Vocal Studio & Sample Preview (does not affect song timeline)
    PlayAudition {
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pitch_ratio: f32,
        time_stretch_ratio: f32,
        is_reverse: bool,
        loop_playback: bool,
    },
    StopAudition,
    SetAuditionParams {
        volume: f32,
        pitch_ratio: f32,
        time_stretch_ratio: f32,
    },
    // Polyphonic WAV one-shot playback (Channel Rack sample player)
    TriggerSampleVoice {
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: u32,
        base_note: u8,
        note: u8,
        pitch_semitones: i8,
        pitch_cents: f32,
        velocity: f32,
        volume: f32,
        reverse: bool,
        start01: f32,
        end01: f32,
    },
    // Modular Patcher (node graph) — real DSP graph evaluated per sample
    SetPatcherGraph(crate::audio::patcher::PatchSpec),
    SetPatcherEnabled(bool),
    PatcherNoteOn {
        freq: f32,
        velocity: f32,
    },
    PatcherNoteOff,
}

#[derive(Debug, Clone)]
pub struct StemRegionPlayback {
    pub start_time_secs: f32,
    pub length_secs: f32,
    pub sample_offset_sec: f32,
    pub gain: f32,
    pub fade_in_sec: f32,
    pub fade_out_sec: f32,
    pub muted: bool,
    pub is_reverse: bool,
    pub loop_length_secs: f32,
}
