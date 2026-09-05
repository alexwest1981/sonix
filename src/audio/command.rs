use super::drum::DrumType;
use super::effects::{DelayParams, ReverbParams};
use super::envelope::AdsrParams;
use super::filter::FilterParams;

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
    pub fn name(&self) -> &'static str {
        match self {
            Preset::CleanPluck => "1. Clean Pluck / Piano",
            Preset::WarmPad => "2. Warm Synth Pad (Mjuk)",
            Preset::AcidBass => "3. Fat Acid Bass (303)",
            Preset::ChiptuneLead => "4. 8-Bit Chiptune Lead",
            Preset::CosmicBrass => "5. Cosmic Synth Brass",
        }
    }

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
    TriggerDrum(DrumType),
    SetWaveform(Waveform),
    SetAdsr(AdsrParams),
    SetFilter(FilterParams),
    SetDelay(DelayParams),
    SetReverb(ReverbParams),
    SetDrive(f32),
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
}
