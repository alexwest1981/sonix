use std::f32::consts::PI;
use super::command::{AudioCommand, Preset, StemRegionPlayback, Waveform};
use super::drum::{DrumType, DrumVoice};
use super::effects::{DelayParams, ReverbParams, SimpleReverb, StereoDelay};
use super::envelope::{AdsrParams, AdsrVoice};
use super::filter::{FilterParams, StateVariableFilter};

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
        AudioCommand::SeekSongPosition(s) => format!("SeekSongPosition({:.2}s)", s),
        other => format!("{}", variant_name(other)),
    }
}

fn variant_name(cmd: &AudioCommand) -> &'static str {
    match cmd {
        AudioCommand::NoteOn { .. } => "NoteOn",
        AudioCommand::NoteOff { .. } => "NoteOff",
        AudioCommand::TriggerDrum(_) => "TriggerDrum",
        AudioCommand::SetWaveform(_) => "SetWaveform",
        AudioCommand::SetAdsr(_) => "SetAdsr",
        AudioCommand::SetFilter(_) => "SetFilter",
        AudioCommand::SetDelay(_) => "SetDelay",
        AudioCommand::SetReverb(_) => "SetReverb",
        AudioCommand::SetDrive(_) => "SetDrive",
        AudioCommand::LoadPreset(_) => "LoadPreset",
        AudioCommand::SetMasterVolume(_) => "SetMasterVolume",
        AudioCommand::StopAll => "StopAll",
        AudioCommand::LoadStemTrack { .. } => "LoadStemTrack",
        AudioCommand::ClearAllStemTracks => "ClearAllStemTracks",
        AudioCommand::SetStemTrackState { .. } => "SetStemTrackState",
        AudioCommand::SetStemTrackRegions { .. } => "SetStemTrackRegions",
        AudioCommand::SeekSongPosition(_) => "SeekSongPosition",
        AudioCommand::SetSongPlayback(_) => "SetSongPlayback",
        AudioCommand::PlayAudition { .. } => "PlayAudition",
        AudioCommand::StopAudition => "StopAudition",
        AudioCommand::SetAuditionParams { .. } => "SetAuditionParams",
        AudioCommand::TriggerSampleVoice { .. } => "TriggerSampleVoice",
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
    pub pan_l: f32,
    pub pan_r: f32,
    pub muted: bool,
    pub solo: bool,
    pub start_time_secs: f32,
    pub regions: Vec<StemRegionPlayback>,
}

impl StemVoiceTrack {
    pub fn new(
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pan: f32,
        start_time_secs: f32,
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
    pub has_stem_solo: bool,
    pub song_playing: bool,
    pub song_time_samples: usize,
    // Isolated audition for Vocal Studio & Sound Browser
    pub audition: Option<AuditionVoice>,
    // Polyphonic WAV one-shot voices for the Channel Rack sample player
    pub sample_voices: [SampleVoice; MAX_SAMPLE_VOICES],
    pub sample_voice_cursor: usize,
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
            has_stem_solo: false,
            song_playing: false,
            song_time_samples: 0,
            audition: None,
            sample_voices: std::array::from_fn(|_| SampleVoice::new()),
            sample_voice_cursor: 0,
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
                for sv in &mut self.sample_voices {
                    sv.active = false;
                }
                if let Some(ref mut aud) = self.audition {
                    aud.is_playing = false;
                }
                self.song_playing = false;
            }
            AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
                let track = StemVoiceTrack::new(left, right, sample_rate, volume, pan, start_time_secs);
                if track_index < self.stem_tracks.len() {
                    self.stem_tracks[track_index] = track;
                } else {
                    while self.stem_tracks.len() < track_index {
                        self.stem_tracks.push(StemVoiceTrack::new(
                            Arc::new(Vec::new()),
                            Arc::new(Vec::new()),
                            self.sample_rate,
                            1.0,
                            0.0,
                            0.0,
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

        // 7. Multi-Track Stem Audio Streaming (with Region Slicing & Fades)
        let mut stem_mix_l = 0.0;
        let mut stem_mix_r = 0.0;

        if self.song_playing && !self.stem_tracks.is_empty() {
            let has_solo = self.has_stem_solo;
            let current_time_sec = self.song_time_samples as f32 / self.sample_rate;

            for track in &self.stem_tracks {
                let audible = if has_solo { track.solo } else { !track.muted };
                if !audible || track.left.is_empty() {
                    continue;
                }

                let pan_l = track.pan_l;
                let pan_r = track.pan_r;

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
                                stem_mix_l += raw_l * g * pan_l;
                                stem_mix_r += raw_r * g * pan_r;
                            } else if idx0 < track.left.len() {
                                let raw_l = track.left[idx0];
                                let raw_r = if idx0 < track.right.len() { track.right[idx0] } else { raw_l };
                                let g = track.volume * region.gain * env;
                                stem_mix_l += raw_l * g * pan_l;
                                stem_mix_r += raw_r * g * pan_r;
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
                            stem_mix_l += raw_l * track.volume * pan_l;
                            stem_mix_r += raw_r * track.volume * pan_r;
                        } else if idx0 < track.left.len() {
                            let raw_l = track.left[idx0];
                            let raw_r = if idx0 < track.right.len() { track.right[idx0] } else { raw_l };
                            stem_mix_l += raw_l * track.volume * pan_l;
                            stem_mix_r += raw_r * track.volume * pan_r;
                        }
                    }
                }
            }

            self.song_time_samples += 1;
        }

        // 8. Isolated Audition Audio Playback (Vocal Studio & Sample Previews)
        if let Some(ref mut aud) = self.audition {
            if aud.is_playing && !aud.left.is_empty() {
                let len = aud.left.len();
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

                    let speed = (aud.sample_rate / self.sample_rate) * aud.pitch_ratio * aud.time_stretch_ratio;
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

        let out_l = ((del_l + stem_mix_l) * self.master_volume).tanh();
        let out_r = ((del_r + stem_mix_r) * self.master_volume).tanh();

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

    #[allow(dead_code)]
    #[inline(always)]
    pub fn process_sample(&mut self) -> f32 {
        let (l, r) = self.process_stereo();
        (l + r) * 0.5
    }
}
