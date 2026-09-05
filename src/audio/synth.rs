use std::f32::consts::PI;
use super::command::{AudioCommand, Preset, Waveform};
use super::drum::{DrumType, DrumVoice};
use super::effects::{DelayParams, ReverbParams, SimpleReverb, StereoDelay};
use super::envelope::{AdsrParams, AdsrVoice};
use super::filter::{FilterParams, StateVariableFilter};

const MAX_VOICES: usize = 16;
const MAX_DRUMS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Voice {
    pub note: u8,
    pub freq: f32,
    pub phase: f32,
    pub phase_inc: f32,
    pub velocity: f32,
    pub envelope: AdsrVoice,
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
        }
    }

    pub fn trigger(&mut self, note: u8, freq: f32, velocity: f32, sample_rate: f32) {
        self.note = note;
        self.freq = freq;
        self.velocity = velocity;
        self.phase_inc = (2.0 * PI * freq) / sample_rate;
        self.envelope.gate_on();
    }

    pub fn release(&mut self) {
        self.envelope.gate_off();
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.velocity = 0.0;
        self.phase = 0.0;
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.envelope.is_active()
    }

    #[inline(always)]
    pub fn next_sample(&mut self, waveform: Waveform, adsr: &AdsrParams) -> f32 {
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

        raw_sample * self.velocity * env_gain
    }
}

use std::sync::Arc;

#[derive(Clone)]
pub struct StemVoiceTrack {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: f32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub start_time_secs: f32,
}

pub struct SynthEngine {
    pub sample_rate: f32,
    pub waveform: Waveform,
    pub adsr: AdsrParams,
    pub filter_params: FilterParams,
    pub filter: StateVariableFilter,
    pub delay_params: DelayParams,
    pub delay: StereoDelay,
    pub reverb_params: ReverbParams,
    pub reverb: SimpleReverb,
    pub drive: f32,
    pub master_volume: f32,
    pub voices: [Voice; MAX_VOICES],
    pub drums: [DrumVoice; MAX_DRUMS],
    // Multi-track audio stem streaming
    pub stem_tracks: Vec<StemVoiceTrack>,
    pub song_playing: bool,
    pub song_time_samples: usize,
}

impl SynthEngine {
    pub fn new(sample_rate: f32) -> Self {
        let (waveform, adsr, filter_params) = Preset::CleanPluck.settings();
        Self {
            sample_rate,
            waveform,
            adsr,
            filter_params,
            filter: StateVariableFilter::new(sample_rate),
            delay_params: DelayParams::default(),
            delay: StereoDelay::new(sample_rate),
            reverb_params: ReverbParams::default(),
            reverb: SimpleReverb::new(),
            drive: 1.0,
            master_volume: 0.8,
            voices: [Voice::new(sample_rate); MAX_VOICES],
            drums: [DrumVoice::new(sample_rate); MAX_DRUMS],
            stem_tracks: Vec::new(),
            song_playing: false,
            song_time_samples: 0,
        }
    }

    pub fn handle_command(&mut self, cmd: AudioCommand) {
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
            AudioCommand::SetDelay(dly) => {
                self.delay_params = dly;
            }
            AudioCommand::SetReverb(rvb) => {
                self.reverb_params = rvb;
            }
            AudioCommand::SetDrive(drv) => {
                self.drive = drv.clamp(1.0, 10.0);
            }
            AudioCommand::LoadPreset(preset) => {
                let (wf, adsr, flt) = preset.settings();
                self.waveform = wf;
                self.adsr = adsr;
                self.filter_params = flt;
                self.filter.reset();
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
                self.song_playing = false;
            }
            AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
                let track = StemVoiceTrack {
                    left,
                    right,
                    sample_rate,
                    volume,
                    pan,
                    muted: false,
                    solo: false,
                    start_time_secs,
                };
                if track_index < self.stem_tracks.len() {
                    self.stem_tracks[track_index] = track;
                } else {
                    while self.stem_tracks.len() < track_index {
                        self.stem_tracks.push(StemVoiceTrack {
                            left: Arc::new(Vec::new()),
                            right: Arc::new(Vec::new()),
                            sample_rate: self.sample_rate,
                            volume: 1.0,
                            pan: 0.0,
                            muted: false,
                            solo: false,
                            start_time_secs: 0.0,
                        });
                    }
                    self.stem_tracks.push(track);
                }
            }
            AudioCommand::ClearAllStemTracks => {
                self.stem_tracks.clear();
            }
            AudioCommand::SetStemTrackState { track_index, volume, pan, muted, solo } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.volume = volume;
                    track.pan = pan;
                    track.muted = muted;
                    track.solo = solo;
                }
            }
            AudioCommand::SeekSongPosition(secs) => {
                self.song_time_samples = (secs.max(0.0) * self.sample_rate) as usize;
            }
            AudioCommand::SetSongPlayback(playing) => {
                self.song_playing = playing;
            }
        }
    }

    #[inline(always)]
    pub fn process_stereo(&mut self) -> (f32, f32) {
        let mut mixed = 0.0;
        let mut active_count = 0;

        // 1. Synthesizer voices
        for voice in &mut self.voices {
            if voice.is_active() {
                mixed += voice.next_sample(self.waveform, &self.adsr);
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

        // 3. Resonant Lowpass Filter on synth bus
        let synth_out = self.filter.process_lowpass(mixed, &self.filter_params);

        // 4. Drum bus
        let mut drum_mix = 0.0;
        for drum in &mut self.drums {
            if drum.active {
                drum_mix += drum.next_sample();
            }
        }

        let combined = synth_out + drum_mix;

        // 5. Reverb FX
        let rev_out = self.reverb.process(combined, &self.reverb_params);

        // 6. Stereo Ping-Pong Delay FX
        let (del_l, del_r) = self.delay.process(rev_out, rev_out, &self.delay_params);

        // 7. Multi-Track Stem Audio Streaming
        let mut stem_mix_l = 0.0;
        let mut stem_mix_r = 0.0;

        if self.song_playing && !self.stem_tracks.is_empty() {
            let has_solo = self.stem_tracks.iter().any(|t| t.solo);
            let current_time_sec = self.song_time_samples as f32 / self.sample_rate;

            for track in &self.stem_tracks {
                let audible = if has_solo { track.solo } else { !track.muted };
                if !audible || track.left.is_empty() {
                    continue;
                }

                let track_rel_time = current_time_sec - track.start_time_secs;
                if track_rel_time < 0.0 {
                    continue;
                }

                let sample_idx = (track_rel_time * track.sample_rate) as usize;
                if sample_idx < track.left.len() {
                    let raw_l = track.left[sample_idx];
                    let raw_r = if sample_idx < track.right.len() { track.right[sample_idx] } else { raw_l };

                    let pan = track.pan.clamp(-1.0, 1.0);
                    let pan_l = ((1.0 - pan) * 0.5).sqrt();
                    let pan_r = ((1.0 + pan) * 0.5).sqrt();

                    stem_mix_l += raw_l * track.volume * pan_l;
                    stem_mix_r += raw_r * track.volume * pan_r;
                }
            }

            self.song_time_samples += 1;
        }

        let out_l = ((del_l + stem_mix_l) * self.master_volume).tanh();
        let out_r = ((del_r + stem_mix_r) * self.master_volume).tanh();

        (out_l, out_r)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn process_sample(&mut self) -> f32 {
        let (l, r) = self.process_stereo();
        (l + r) * 0.5
    }
}
