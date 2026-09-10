use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use eframe::egui::Color32;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use super::wav_writer::write_pcm_f32_to_wav;

#[derive(Debug, Clone)]
pub struct AudioTake {
    pub name: String,
    pub pcm_samples: Vec<f32>,
    pub waveform_data: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: f32,
    pub is_selected: bool,
    pub color: Color32,
    pub trim_start_norm: f32,
    pub trim_end_norm: f32,
    pub gain_linear: f32,
    pub pitch_semitones: f32,
    pub pitch_cents: f32,
    pub time_stretch: f32,
    pub is_reverse: bool,
    pub loop_audition: bool,
    pub is_playing: bool,
}

#[derive(Debug, Clone)]
pub struct CustomSoundClip {
    pub name: String,
    pub category: String,
    pub pcm_samples: Vec<f32>,
    pub waveform_data: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: f32,
    pub color: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    LeadVocals,
    CustomSounds,
}

#[derive(Clone, Debug)]
pub struct MicrophoneSettings {
    pub selected_device_idx: usize,
    pub available_devices: Vec<String>,
    pub input_gain: f32,
    pub noise_gate_thresh: f32,
    pub direct_monitoring: bool,
    pub feedback_reduction: bool,
    pub low_cut_80hz: bool,
    pub vocal_reverb: f32,
    pub de_esser_amount: f32,
    pub compressor_amount: f32,
}

impl Default for MicrophoneSettings {
    fn default() -> Self {
        let devs = LiveMicrophoneCapture::list_devices();
        let default_idx = devs.iter().position(|d| {
            let l = d.to_lowercase();
            (l.contains("samson") || l.contains("usb") || l.contains("mic")) && !l.contains("monitor")
        }).unwrap_or(0);
        Self {
            selected_device_idx: default_idx,
            available_devices: devs,
            input_gain: 1.50,
            noise_gate_thresh: 0.012,
            direct_monitoring: false,
            feedback_reduction: true,
            low_cut_80hz: true,
            vocal_reverb: 0.20,
            de_esser_amount: 0.35,
            compressor_amount: 0.45,
        }
    }
}

pub struct LiveMicrophoneCapture {
    pub is_recording: Arc<AtomicBool>,
    pub is_paused: Arc<AtomicBool>,
    pub input_gain: Arc<Mutex<f32>>,
    pub noise_gate_thresh: Arc<Mutex<f32>>,
    pub peak_vu: Arc<AtomicU32>,
    pub recorded_samples: Arc<Mutex<Vec<f32>>>,
    pub live_peaks: Arc<Mutex<Vec<f32>>>,
    pub analysis_buffer: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub device_name: String,
    _stream: Option<Stream>,
}

/// How many recent mono samples are kept for always-on pitch analysis.
const ANALYSIS_MAX: usize = 16384;

/// Shared microphone callback body: computes per-block peak, records to
/// `recorded` while armed, and always feeds the rolling `analysis` buffer used
/// by the strobe tuner and harmonizer.
#[allow(clippy::too_many_arguments)]
fn process_mic_block<T: Copy>(
    data: &[T],
    channels: usize,
    gain: f32,
    gate: f32,
    is_recording: bool,
    is_paused: bool,
    recorded: &Arc<Mutex<Vec<f32>>>,
    peaks: &Arc<Mutex<Vec<f32>>>,
    analysis: &Arc<Mutex<Vec<f32>>>,
    convert: impl Fn(T) -> f32,
) -> f32 {
    let ch = channels.max(1);
    let recording = is_recording && !is_paused;
    let mut block_max = 0.0_f32;
    let mut rec = if recording {
        Some(recorded.lock().unwrap_or_else(|e| e.into_inner()))
    } else {
        None
    };
    let mut pk = if recording {
        Some(peaks.lock().unwrap_or_else(|e| e.into_inner()))
    } else {
        None
    };
    let mut an = analysis.lock().unwrap_or_else(|e| e.into_inner());

    for frame in data.chunks(ch) {
        let mut mono = (frame.iter().map(|&s| convert(s)).sum::<f32>() / ch as f32) * gain;
        let raw_abs = mono.abs();
        if raw_abs < gate {
            mono = 0.0;
        }
        if raw_abs > block_max {
            block_max = raw_abs;
        }
        if let Some(b) = rec.as_mut() {
            b.push(mono.clamp(-1.0, 1.0));
            if b.len() % 256 == 0 {
                if let Some(p) = pk.as_mut() {
                    p.push(block_max.clamp(0.04, 1.0));
                }
            }
        }
        an.push(mono.clamp(-1.0, 1.0));
    }

    if an.len() > ANALYSIS_MAX {
        let drop = an.len() - ANALYSIS_MAX;
        an.drain(0..drop);
    }
    block_max
}

/// Normalized autocorrelation pitch detector. Returns the fundamental
/// frequency in Hz for the most recent `samples`, or `None` if the signal is
/// too quiet / unpitched. Suitable for guitar, bass and voice. Uses a fixed
/// analysis window so it stays cheap enough to run per UI frame.
pub fn detect_pitch_hz(samples: &[f32], sample_rate: f32) -> Option<f32> {
    let n = samples.len();
    if n < 1024 || sample_rate <= 0.0 {
        return None;
    }

    // Use the most recent samples, but cap the correlation window for speed.
    let min_lag = (sample_rate / 1200.0).max(2.0) as usize;
    let max_lag = (sample_rate / 55.0) as usize;
    if max_lag <= min_lag || n <= max_lag + 512 {
        return None;
    }
    let window = (n - max_lag).min(2048);
    let buf = &samples[n - window..];
    let mean = buf.iter().sum::<f32>() / window as f32;

    let energy0: f32 = buf
        .iter()
        .map(|s| {
            let d = s - mean;
            d * d
        })
        .sum();
    if energy0 <= 1e-6 {
        return None;
    }
    let rms = (energy0 / window as f32).sqrt();
    if rms < 0.004 {
        return None;
    }

    let mut corrs: Vec<f32> = Vec::with_capacity(max_lag - min_lag);
    for lag in min_lag..max_lag {
        let mut corr = 0.0_f32;
        for i in 0..window {
            corr += (buf[i] - mean) * (samples[n - window + i - lag] - mean);
        }
        corrs.push(corr / energy0);
    }

    let peak = corrs.iter().cloned().fold(f32::MIN, f32::max);
    if peak < 0.5 {
        return None;
    }

    // Pick the *first* strong local peak (smallest lag) to avoid octave errors
    // where 2x the true period also correlates strongly.
    let threshold = peak * 0.9;
    let mut best_idx = None;
    for i in 0..corrs.len() {
        let c = corrs[i];
        let prev = if i == 0 { f32::MIN } else { corrs[i - 1] };
        let next = if i + 1 >= corrs.len() { f32::MIN } else { corrs[i + 1] };
        if c >= threshold && c >= prev && c >= next {
            best_idx = Some(i);
            break;
        }
    }
    let best_idx = best_idx.unwrap_or_else(|| {
        corrs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0)
    });
    let best_lag = min_lag + best_idx;
    let best_corr = corrs[best_idx];

    if best_lag == 0 {
        return None;
    }

    let corr_at = |lag: usize| -> f32 {
        if lag < min_lag || lag >= max_lag {
            return 0.0;
        }
        corrs[lag - min_lag]
    };

    let lag = best_lag;
    if lag > min_lag && lag + 1 < max_lag {
        let y0 = corr_at(lag - 1);
        let y1 = best_corr;
        let y2 = corr_at(lag + 1);
        let denom = y0 - 2.0 * y1 + y2;
        if denom.abs() > 1e-9 {
            let shift = (0.5 * (y0 - y2) / denom).clamp(-0.5, 0.5);
            return Some(sample_rate / (lag as f32 + shift));
        }
    }
    Some(sample_rate / lag as f32)
}

/// Downsamples PCM to a fixed peak envelope for waveform drawing.
pub fn visual_peaks_from(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let count = 512.min(samples.len().max(64));
    let step = (samples.len() / count).max(1);
    samples
        .chunks(step)
        .map(|c| c.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs())).clamp(0.0, 1.0))
        .collect()
}

impl LiveMicrophoneCapture {
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        if let Ok(dev_iter) = host.input_devices() {
            for dev in dev_iter {
                if let Ok(name) = dev.name() {
                    devices.push(name);
                }
            }
        }
        devices
    }

    pub fn new() -> Self {
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let input_gain = Arc::new(Mutex::new(1.50));
        let noise_gate_thresh = Arc::new(Mutex::new(0.012));
        let peak_vu = Arc::new(AtomicU32::new(0));
        let recorded_samples = Arc::new(Mutex::new(Vec::with_capacity(44100 * 30)));
        let live_peaks = Arc::new(Mutex::new(Vec::with_capacity(1000)));
        let analysis_buffer = Arc::new(Mutex::new(Vec::with_capacity(ANALYSIS_MAX)));

        let host = cpal::default_host();
        let mut chosen_device = None;
        if let Ok(dev_iter) = host.input_devices() {
            let devs: Vec<_> = dev_iter.collect();
            // Prioritize dedicated USB mics like Samson Q2U over monitor devices
            if let Some(preferred) = devs.iter().find(|d| {
                if let Ok(n) = d.name() {
                    let l = n.to_lowercase();
                    (l.contains("samson") || l.contains("usb") || l.contains("mic")) && !l.contains("monitor")
                } else {
                    false
                }
            }) {
                chosen_device = Some((*preferred).clone());
            } else if let Some(first) = devs.into_iter().next() {
                chosen_device = Some(first);
            }
        }
        if chosen_device.is_none() {
            chosen_device = host.default_input_device();
        }

        let (device_name, stream, sample_rate) = if let Some(device) = chosen_device {
            let name = device.name().unwrap_or_else(|_| "PipeWire / ALSA Mikrofon".to_string());
            if let Ok(default_config) = device.default_input_config() {
                let sr = default_config.sample_rate().0;
                let sample_format = default_config.sample_format();
                let config: StreamConfig = default_config.into();

                let is_rec = Arc::clone(&is_recording);
                let is_p = Arc::clone(&is_paused);
                let gain_ref = Arc::clone(&input_gain);
                let gate_ref = Arc::clone(&noise_gate_thresh);
                let vu_ref = Arc::clone(&peak_vu);
                let samples_ref = Arc::clone(&recorded_samples);
                let peaks_ref = Arc::clone(&live_peaks);
                let analysis_ref = Arc::clone(&analysis_buffer);

                let num_channels = config.channels as usize;

                let stream_res = match sample_format {
                    SampleFormat::F32 => device.build_input_stream(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let gate = *gate_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let block_max = process_mic_block(
                                data,
                                num_channels,
                                gain,
                                gate,
                                is_rec.load(Ordering::Relaxed),
                                is_p.load(Ordering::Relaxed),
                                &samples_ref,
                                &peaks_ref,
                                &analysis_ref,
                                |s| s,
                            );
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);
                        },
                        |err| eprintln!("[Sonix Mic Input Error] {}", err),
                        None,
                    ),
                    SampleFormat::I16 => device.build_input_stream(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let gate = *gate_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let block_max = process_mic_block(
                                data,
                                num_channels,
                                gain,
                                gate,
                                is_rec.load(Ordering::Relaxed),
                                is_p.load(Ordering::Relaxed),
                                &samples_ref,
                                &peaks_ref,
                                &analysis_ref,
                                |s| s as f32 / 32768.0,
                            );
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);
                        },
                        |err| eprintln!("[Sonix Mic Input Error] {}", err),
                        None,
                    ),
                    _ => Err(cpal::BuildStreamError::DeviceNotAvailable),
                };

                let active_stream = match stream_res {
                    Ok(s) => {
                        let _ = s.play();
                        Some(s)
                    }
                    Err(_) => None,
                };
                (name, active_stream, sr)
            } else {
                ("Ingen mikrofon hittad (standby)".to_string(), None, 44100)
            }
        } else {
            ("Ingen mikrofon hittad (standby)".to_string(), None, 44100)
        };

        Self {
            is_recording,
            is_paused,
            input_gain,
            noise_gate_thresh,
            peak_vu,
            recorded_samples,
            live_peaks,
            analysis_buffer,
            sample_rate,
            device_name,
            _stream: stream,
        }
    }

    pub fn open_device_by_index(&mut self, device_index: usize) -> bool {
        let host = cpal::default_host();
        let mut chosen_device = None;
        if let Ok(dev_iter) = host.input_devices() {
            let devs: Vec<_> = dev_iter.collect();
            if let Some(dev) = devs.into_iter().nth(device_index) {
                chosen_device = Some(dev);
            }
        }
        if chosen_device.is_none() {
            chosen_device = host.default_input_device();
        }

        if let Some(device) = chosen_device {
            let name = device.name().unwrap_or_else(|_| "Mikrofon".to_string());
            if let Ok(default_config) = device.default_input_config() {
                let sr = default_config.sample_rate().0;
                let sample_format = default_config.sample_format();
                let config: StreamConfig = default_config.into();

                let is_rec = Arc::clone(&self.is_recording);
                let is_p = Arc::clone(&self.is_paused);
                let gain_ref = Arc::clone(&self.input_gain);
                let gate_ref = Arc::clone(&self.noise_gate_thresh);
                let vu_ref = Arc::clone(&self.peak_vu);
                let samples_ref = Arc::clone(&self.recorded_samples);
                let peaks_ref = Arc::clone(&self.live_peaks);
                let analysis_ref = Arc::clone(&self.analysis_buffer);

                let num_channels = config.channels as usize;

                let stream_res = match sample_format {
                    SampleFormat::F32 => device.build_input_stream(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let gate = *gate_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let block_max = process_mic_block(
                                data,
                                num_channels,
                                gain,
                                gate,
                                is_rec.load(Ordering::Relaxed),
                                is_p.load(Ordering::Relaxed),
                                &samples_ref,
                                &peaks_ref,
                                &analysis_ref,
                                |s| s,
                            );
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);
                        },
                        |err| eprintln!("[Sonix Mic Input Error] {}", err),
                        None,
                    ),
                    SampleFormat::I16 => device.build_input_stream(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let gate = *gate_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let block_max = process_mic_block(
                                data,
                                num_channels,
                                gain,
                                gate,
                                is_rec.load(Ordering::Relaxed),
                                is_p.load(Ordering::Relaxed),
                                &samples_ref,
                                &peaks_ref,
                                &analysis_ref,
                                |s| s as f32 / 32768.0,
                            );
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);
                        },
                        |err| eprintln!("[Sonix Mic Input Error] {}", err),
                        None,
                    ),
                    _ => Err(cpal::BuildStreamError::DeviceNotAvailable),
                };

                if let Ok(s) = stream_res {
                    let _ = s.play();
                    self._stream = Some(s);
                    self.device_name = name;
                    self.sample_rate = sr;
                    return true;
                }
            }
        }
        false
    }
}

pub struct VocalStudioTrack {
    pub input_channel: String,
    pub is_armed: bool,
    pub is_recording: bool,
    pub is_paused: bool,
    pub is_recording_custom: bool,
    pub custom_recording_elapsed_secs: f32,
    pub recording_mode: RecordingMode,
    pub recording_elapsed_secs: f32,
    pub monitoring_on: bool,
    pub input_gain: f32,
    pub mic_vu_level: f32,
    pub custom_sample_name_input: String,
    pub takes: Vec<AudioTake>,
    pub custom_sounds: Vec<CustomSoundClip>,
    pub active_comp_take: usize,
    pub live_recording_peaks: Vec<f32>,
    pub selected_crop_start_norm: f32,
    pub selected_crop_end_norm: f32,
    pub mic_capture: Option<LiveMicrophoneCapture>,
    pub mic_settings: MicrophoneSettings,
}

impl Default for VocalStudioTrack {
    fn default() -> Self {
        let mic = LiveMicrophoneCapture::new();
        let dev_name = mic.device_name.clone();
        let settings = MicrophoneSettings::default();

        Self {
            input_channel: dev_name,
            is_armed: true,
            is_recording: false,
            is_paused: false,
            is_recording_custom: false,
            custom_recording_elapsed_secs: 0.0,
            recording_mode: RecordingMode::LeadVocals,
            recording_elapsed_secs: 0.0,
            monitoring_on: true,
            input_gain: settings.input_gain,
            mic_vu_level: 0.0,
            custom_sample_name_input: crate::i18n::t("Mitt Akustiska Ljud 1").to_string(),
            takes: Vec::new(),
            custom_sounds: Vec::new(),
            active_comp_take: 0,
            live_recording_peaks: Vec::new(),
            selected_crop_start_norm: 0.0,
            selected_crop_end_norm: 1.0,
            mic_capture: Some(mic),
            mic_settings: settings,
        }
    }
}

impl VocalStudioTrack {
    pub fn select_microphone(&mut self, device_index: usize) -> bool {
        self.mic_settings.selected_device_idx = device_index;
        if let Some(ref mut mic) = self.mic_capture {
            if mic.open_device_by_index(device_index) {
                self.input_channel = mic.device_name.clone();
                return true;
            }
        }
        false
    }

    pub fn set_input_gain(&mut self, gain: f32) {
        self.mic_settings.input_gain = gain;
        self.input_gain = gain;
        if let Some(ref mic) = self.mic_capture {
            if let Ok(mut g) = mic.input_gain.lock() {
                *g = gain;
            }
        }
    }

    pub fn set_noise_gate(&mut self, threshold: f32) {
        self.mic_settings.noise_gate_thresh = threshold;
        if let Some(ref mic) = self.mic_capture {
            if let Ok(mut gate) = mic.noise_gate_thresh.lock() {
                *gate = threshold;
            }
        }
    }
    pub fn update_live_stream(&mut self) {
        if let Some(ref mic) = self.mic_capture {
            let vu_bits = mic.peak_vu.load(Ordering::Relaxed);
            let target_vu = f32::from_bits(vu_bits);
            // Smooth VU falloff
            if target_vu > self.mic_vu_level {
                self.mic_vu_level = target_vu;
            } else {
                self.mic_vu_level = (self.mic_vu_level * 0.88).max(0.0);
            }

            if self.is_recording && !self.is_paused {
                self.recording_elapsed_secs += 0.033; // ~30 fps frame delta

                // Sync the real captured peaks (empty while no mic is streaming).
                if let Ok(peaks) = mic.live_peaks.lock() {
                    self.live_recording_peaks = peaks.clone();
                }
            }

            if self.is_recording_custom {
                self.custom_recording_elapsed_secs += 0.033;
            }
        }
    }

    /// Detects the live fundamental frequency from the always-on microphone
    /// analysis buffer. Returns `None` when silent or unpitched.
    pub fn detect_live_pitch_hz(&self) -> Option<f32> {
        let mic = self.mic_capture.as_ref()?;
        let buf = mic.analysis_buffer.lock().ok()?;
        if buf.len() < 1024 {
            return None;
        }
        detect_pitch_hz(&buf, mic.sample_rate as f32)
    }

    /// Replaces the PCM of the currently selected take (used by Auto-Tune).
    pub fn replace_active_take_pcm(&mut self, name_suffix: &str, pcm: Vec<f32>) {
        if self.takes.is_empty() {
            return;
        }
        let idx = self.active_comp_take.min(self.takes.len() - 1);
        let sr = self.takes[idx].sample_rate.max(8000) as f32;
        let take = &mut self.takes[idx];
        take.pcm_samples = pcm;
        take.duration_secs = take.pcm_samples.len() as f32 / sr;
        take.waveform_data = visual_peaks_from(&take.pcm_samples);
        if !take.name.contains(name_suffix) {
            take.name = format!("{} {}", take.name, name_suffix);
        }
    }

    /// Appends a new take built from a PCM buffer (used by the harmonizer).
    pub fn add_take_from_pcm(
        &mut self,
        name: String,
        pcm: Vec<f32>,
        sample_rate: u32,
        color: Color32,
    ) -> usize {
        let sr = sample_rate.max(8000);
        let waveform_data = visual_peaks_from(&pcm);
        let duration_secs = pcm.len() as f32 / sr as f32;
        for t in &mut self.takes {
            t.is_selected = false;
        }
        self.takes.push(AudioTake {
            name,
            pcm_samples: pcm,
            waveform_data,
            sample_rate: sr,
            duration_secs,
            is_selected: true,
            color,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            pitch_semitones: 0.0,
            pitch_cents: 0.0,
            time_stretch: 1.0,
            is_reverse: false,
            loop_audition: false,
            is_playing: false,
        });
        self.active_comp_take = self.takes.len() - 1;
        self.active_comp_take
    }

    pub fn start_recording(&mut self) -> Result<(), String> {
        if !self.is_armed {
            return Err(crate::i18n::t("Mikrofonen är inte armerad – kryssa i 'Armera' först").to_string());
        }
        if self.mic_capture.as_ref().map(|m| m._stream.is_none()).unwrap_or(true) {
            return Err(crate::i18n::t("Ingen mikrofon är tillgänglig för inspelning").to_string());
        }
        self.is_recording = true;
        self.is_paused = false;
        self.recording_elapsed_secs = 0.0;
        self.live_recording_peaks.clear();

        if let Some(ref mic) = self.mic_capture {
            if let Ok(mut s) = mic.recorded_samples.lock() { s.clear(); }
            if let Ok(mut p) = mic.live_peaks.lock() { p.clear(); }
            mic.is_recording.store(true, Ordering::Relaxed);
            mic.is_paused.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn pause_recording(&mut self) {
        self.is_paused = true;
        if let Some(ref mic) = self.mic_capture {
            mic.is_paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn resume_recording(&mut self) {
        self.is_paused = false;
        if let Some(ref mic) = self.mic_capture {
            mic.is_paused.store(false, Ordering::Relaxed);
        }
    }

    /// Starts capturing microphone audio for a custom sound / one-shot sample.
    pub fn start_custom_recording(&mut self) -> Result<(), String> {
        if self.mic_capture.as_ref().map(|m| m._stream.is_none()).unwrap_or(true) {
            return Err(crate::i18n::t("Ingen mikrofon är tillgänglig för inspelning").to_string());
        }
        if let Some(ref mic) = self.mic_capture {
            if let Ok(mut s) = mic.recorded_samples.lock() { s.clear(); }
            if let Ok(mut p) = mic.live_peaks.lock() { p.clear(); }
            mic.is_recording.store(true, Ordering::Relaxed);
            mic.is_paused.store(false, Ordering::Relaxed);
        }
        self.is_recording_custom = true;
        self.custom_recording_elapsed_secs = 0.0;
        Ok(())
    }

    /// Stops custom-sound capture and stores the real recorded PCM in the
    /// library. Returns the new clip index.
    pub fn stop_custom_recording(&mut self) -> Result<usize, String> {
        self.is_recording_custom = false;
        let mic_sr = self.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
        let mut samples = Vec::new();
        if let Some(ref mic) = self.mic_capture {
            mic.is_recording.store(false, Ordering::Relaxed);
            if let Ok(s) = mic.recorded_samples.lock() {
                samples = s.clone();
            }
        }
        if samples.is_empty() {
            return Err(crate::i18n::t("Ingen ljuddata fångades – kontrollera mikrofonen").to_string());
        }

        let name = if self.custom_sample_name_input.trim().is_empty() {
            format!("{} {}", crate::i18n::t("Eget Ljud"), self.custom_sounds.len() + 1)
        } else {
            self.custom_sample_name_input.trim().to_string()
        };
        let duration = samples.len() as f32 / mic_sr.max(8000) as f32;
        let waveform_data = visual_peaks_from(&samples);
        self.custom_sounds.push(CustomSoundClip {
            name,
            category: crate::i18n::t("Eget Ljud").to_string(),
            pcm_samples: samples,
            waveform_data,
            sample_rate: mic_sr.max(8000),
            duration_secs: duration,
            color: Color32::from_rgb(255, 180, 60),
        });
        Ok(self.custom_sounds.len() - 1)
    }

    pub fn stop_recording(&mut self) -> Result<usize, String> {
        self.is_recording = false;
        self.is_paused = false;

        let mic_sr = self.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
        let mut samples = Vec::new();
        if let Some(ref mic) = self.mic_capture {
            mic.is_recording.store(false, Ordering::Relaxed);
            mic.is_paused.store(false, Ordering::Relaxed);
            if let Ok(s) = mic.recorded_samples.lock() {
                samples = s.clone();
            }
        }

        if samples.is_empty() {
            return Err(crate::i18n::t("Ingen ljuddata fångades – kontrollera mikrofonen").to_string());
        }

        let duration = (samples.len() as f32 / mic_sr as f32).max(0.1);
        let visual_peaks = visual_peaks_from(&samples);

        let count = self.takes.len() + 1;
        let color = match count % 4 {
            0 => Color32::from_rgb(0, 200, 240),
            1 => Color32::from_rgb(255, 140, 0),
            2 => Color32::from_rgb(180, 100, 255),
            _ => Color32::from_rgb(80, 240, 160),
        };

        let new_take = AudioTake {
            name: format!("{} {} ({:.1}s)", crate::i18n::t("Tagning"), count, duration),
            pcm_samples: samples,
            waveform_data: visual_peaks,
            sample_rate: mic_sr,
            duration_secs: duration,
            is_selected: true,
            color,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            pitch_semitones: 0.0,
            pitch_cents: 0.0,
            time_stretch: 1.0,
            is_reverse: false,
            loop_audition: false,
            is_playing: false,
        };

        for t in &mut self.takes { t.is_selected = false; }
        self.takes.push(new_take);
        self.active_comp_take = self.takes.len() - 1;
        self.selected_crop_start_norm = 0.0;
        self.selected_crop_end_norm = 1.0;

        Ok(self.active_comp_take)
    }

    pub fn crop_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err(crate::i18n::t("Ingen tagning är vald att beskära").to_string());
        }

        let start_n = self.selected_crop_start_norm.clamp(0.0, 0.99);
        let end_n = self.selected_crop_end_norm.clamp(start_n + 0.01, 1.0);

        let take = &mut self.takes[self.active_comp_take];
        let total_samples = take.pcm_samples.len();
        if total_samples < 100 {
            return Err(crate::i18n::t("Tagningen innehåller för lite data för att beskäras").to_string());
        }

        let s_idx = (total_samples as f32 * start_n) as usize;
        let e_idx = ((total_samples as f32 * end_n) as usize).min(total_samples);

        take.pcm_samples = take.pcm_samples[s_idx..e_idx].to_vec();
        take.duration_secs = take.pcm_samples.len() as f32 / take.sample_rate.max(8000) as f32;

        // Recompute peaks
        let peaks_count = 512.min(take.pcm_samples.len().max(64));
        let step = (take.pcm_samples.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in take.pcm_samples.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.0, 1.0));
        }
        take.waveform_data = visual_peaks;
        take.trim_start_norm = 0.0;
        take.trim_end_norm = 1.0;

        self.selected_crop_start_norm = 0.0;
        self.selected_crop_end_norm = 1.0;

        Ok(crate::tstatus!("✔ Beskärde {} till {:.2} sekunder!", take.name, take.duration_secs))
    }

    pub fn slice_selected_take_at(&mut self, split_norm: f32) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err(crate::i18n::t("Ingen tagning är vald att klippa").to_string());
        }

        let split_n = split_norm.clamp(0.05, 0.95);
        let orig_take = self.takes[self.active_comp_take].clone();
        let total = orig_take.pcm_samples.len();
        let split_idx = (total as f32 * split_n) as usize;

        let part1_samples = orig_take.pcm_samples[..split_idx].to_vec();
        let part2_samples = orig_take.pcm_samples[split_idx..].to_vec();

        let make_peaks = |samples: &[f32]| -> Vec<f32> {
            let count = 512.min(samples.len().max(64));
            let step = (samples.len() / count).max(1);
            samples.chunks(step).map(|c| c.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs())).clamp(0.0, 1.0)).collect()
        };

        let t1_peaks = make_peaks(&part1_samples);
        let t2_peaks = make_peaks(&part2_samples);
        let sr = orig_take.sample_rate.max(8000);

        // Update Part 1
        self.takes[self.active_comp_take].name = format!("{} (Del 1)", orig_take.name);
        self.takes[self.active_comp_take].pcm_samples = part1_samples.clone();
        self.takes[self.active_comp_take].waveform_data = t1_peaks;
        self.takes[self.active_comp_take].duration_secs = part1_samples.len() as f32 / sr as f32;

        // Insert Part 2 as new take
        self.takes.push(AudioTake {
            name: format!("{} (Del 2)", orig_take.name),
            pcm_samples: part2_samples.clone(),
            waveform_data: t2_peaks,
            sample_rate: sr,
            duration_secs: part2_samples.len() as f32 / sr as f32,
            is_selected: true,
            color: Color32::from_rgb(255, 100, 180),
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            pitch_semitones: 0.0,
            pitch_cents: 0.0,
            time_stretch: 1.0,
            is_reverse: false,
            loop_audition: false,
            is_playing: false,
        });

        self.active_comp_take = self.takes.len() - 1;
        Ok(crate::i18n::t("✔ Klippte tagningen i två separata delar!").to_string())
    }

    pub fn export_take_to_wav(&self, take_idx: usize, path: &str) -> Result<String, String> {
        if let Some(take) = self.takes.get(take_idx) {
            let sr = take.sample_rate;
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            write_pcm_f32_to_wav(path, &take.pcm_samples, sr, 1)
                .map_err(|e| format!("Kunde inte skriva WAV-fil: {}", e))?;
            Ok(crate::tstatus!("✔ Sparade tagning '{}' till {}", take.name, path))
        } else {
            Err(crate::i18n::t("Tagningen hittades inte").to_string())
        }
    }

    pub fn normalize_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err(crate::i18n::t("Ingen tagning är vald att normalisera").to_string());
        }
        let take = &mut self.takes[self.active_comp_take];
        let max_amp = take.pcm_samples.iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        if max_amp < 0.0001 {
            return Err(crate::i18n::t("Ljudspåret är helt tyst").to_string());
        }
        let mult = 0.98 / max_amp;
        for s in &mut take.pcm_samples {
            *s = (*s * mult).clamp(-1.0, 1.0);
        }
        let peaks_count = 512.min(take.pcm_samples.len().max(64));
        let step = (take.pcm_samples.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in take.pcm_samples.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.0, 1.0));
        }
        take.waveform_data = visual_peaks;
        Ok(crate::tstatus!("✔ Normaliserade {} (Peak förstärkt med {:.1}x)!", take.name, mult))
    }

    pub fn delete_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err(crate::i18n::t("Ingen tagning är vald att ta bort").to_string());
        }
        let removed = self.takes.remove(self.active_comp_take);
        if !self.takes.is_empty() {
            self.active_comp_take = self.active_comp_take.min(self.takes.len() - 1);
            self.takes[self.active_comp_take].is_selected = true;
        } else {
            self.active_comp_take = 0;
        }
        Ok(crate::tstatus!("✔ Tog bort {}", removed.name))
    }

    pub fn add_new_take(&mut self, name: &str) {
        let count = self.takes.len() + 1;
        let sr = 44100u32;
        let dur = 2.0f32;
        let freq = 220.0f32;
        let n = (sr as f32 * dur) as usize;
        // A real, audible reference tone with an attack/release envelope; the
        // waveform is derived from the actual PCM so what you see is what you hear.
        let pcm: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let env = (t * 8.0).min(1.0) * ((dur - t) * 4.0).clamp(0.0, 1.0);
                (t * freq * std::f32::consts::TAU).sin() * 0.5 * env
            })
            .collect();
        let new_wave = visual_peaks_from(&pcm);
        let color = match count % 4 {
            0 => Color32::from_rgb(0, 200, 240),
            1 => Color32::from_rgb(255, 140, 0),
            2 => Color32::from_rgb(180, 100, 255),
            _ => Color32::from_rgb(80, 240, 160),
        };
        self.takes.push(AudioTake {
            name: if name.is_empty() { format!("{} {} ({})", crate::i18n::t("Tagning"), count, crate::i18n::t("Testton")) } else { name.to_string() },
            pcm_samples: pcm,
            waveform_data: new_wave,
            sample_rate: sr,
            duration_secs: dur,
            is_selected: true,
            color,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            pitch_semitones: 0.0,
            pitch_cents: 0.0,
            time_stretch: 1.0,
            is_reverse: false,
            loop_audition: false,
            is_playing: false,
        });
        for t in &mut self.takes { t.is_selected = false; }
        self.takes.last_mut().unwrap().is_selected = true;
        self.active_comp_take = self.takes.len() - 1;
    }

    pub fn load_sample_or_region_as_take(&mut self, name: &str, pcm: Vec<f32>, sample_rate: u32, color: Color32) -> usize {
        let duration = (pcm.len() as f32 / sample_rate.max(8000) as f32).max(0.1);

        let peaks_count = 512.min(pcm.len().max(64));
        let step = (pcm.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in pcm.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.0, 1.0));
        }

        let new_take = AudioTake {
            name: name.to_string(),
            pcm_samples: pcm,
            waveform_data: visual_peaks,
            sample_rate,
            duration_secs: duration,
            is_selected: true,
            color,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            pitch_semitones: 0.0,
            pitch_cents: 0.0,
            time_stretch: 1.0,
            is_reverse: false,
            loop_audition: false,
            is_playing: false,
        };

        for t in &mut self.takes { t.is_selected = false; }
        self.takes.push(new_take);
        self.active_comp_take = self.takes.len() - 1;
        self.selected_crop_start_norm = 0.0;
        self.selected_crop_end_norm = 1.0;
        self.active_comp_take
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pitch_hz_sine() {
        let sr = 44100.0_f32;
        for &freq in &[82.41_f32, 110.0, 196.0, 329.63, 440.0] {
            let buf: Vec<f32> = (0..8192)
                .map(|i| 0.6 * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
                .collect();
            let detected = detect_pitch_hz(&buf, sr).expect("should detect a pitch");
            assert!(
                (detected - freq).abs() / freq < 0.02,
                "expected ~{freq} Hz, got {detected} Hz"
            );
        }
    }

    #[test]
    fn test_detect_pitch_hz_silence_is_none() {
        let sr = 44100.0_f32;
        let silence = vec![0.0_f32; 8192];
        assert!(detect_pitch_hz(&silence, sr).is_none());
    }

    #[test]
    fn test_vocal_studio_recording_workflow() {
        let mut track = VocalStudioTrack::default();
        let initial_takes = track.takes.len();

        // Inject real captured samples (as the mic callback would) and mark the
        // track as recording; this exercises the take-creation path without
        // depending on physical audio hardware being present in CI.
        if let Some(ref mic) = track.mic_capture {
            let mut s = mic.recorded_samples.lock().unwrap();
            for i in 0..44100 {
                s.push((i as f32 * 0.01).sin() * 0.5);
            }
        }
        track.is_recording = true;
        track.is_paused = false;
        track.update_live_stream();

        let new_idx = track.stop_recording().expect("Should create a take");
        assert_eq!(track.takes.len(), initial_takes + 1);
        assert_eq!(track.active_comp_take, new_idx);

        let take = &track.takes[new_idx];
        assert!(!take.pcm_samples.is_empty());
        assert!(!take.waveform_data.is_empty());
        assert!(take.duration_secs > 0.0);
    }

    #[test]
    fn test_start_recording_requires_arm() {
        let mut track = VocalStudioTrack::default();
        track.is_armed = false;
        assert!(track.start_recording().is_err());
        assert!(!track.is_recording);
    }

    #[test]
    fn test_custom_sound_recording_stores_pcm() {
        let mut track = VocalStudioTrack::default();
        if let Some(ref mic) = track.mic_capture {
            let mut s = mic.recorded_samples.lock().unwrap();
            for i in 0..22050 {
                s.push((i as f32 * 0.05).sin() * 0.4);
            }
        }
        track.custom_sample_name_input = "Testklapp".to_string();
        let idx = track.stop_custom_recording().expect("clip");
        assert_eq!(idx, 0);
        assert_eq!(track.custom_sounds.len(), 1);
        assert!(!track.custom_sounds[0].pcm_samples.is_empty());
        assert!(!track.custom_sounds[0].waveform_data.is_empty());
    }

    #[test]
    fn test_add_new_take_generates_audible_pcm() {
        let mut track = VocalStudioTrack::default();
        track.add_new_take("Testton");
        let take = track.takes.last().unwrap();
        assert_eq!(take.pcm_samples.len(), 88200);
        let peak = take.pcm_samples.iter().fold(0.0_f32, |a, &s| a.max(s.abs()));
        assert!(peak > 0.1, "test tone should be audible, peak was {peak}");
    }

    #[test]
    fn test_vocal_studio_crop_and_slice() {
        let mut track = VocalStudioTrack::default();
        let pcm: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.02).sin() * 0.5).collect();
        track.load_sample_or_region_as_take("Test", pcm, 44100, Color32::from_rgb(0, 200, 240));
        track.active_comp_take = 0;

        let orig_len = track.takes[0].pcm_samples.len();

        // Crop take
        track.selected_crop_start_norm = 0.25;
        track.selected_crop_end_norm = 0.75;
        let crop_res = track.crop_selected_take();
        assert!(crop_res.is_ok());
        assert!(track.takes[0].pcm_samples.len() < orig_len);

        // Slice take
        let takes_count = track.takes.len();
        let slice_res = track.slice_selected_take_at(0.5);
        assert!(slice_res.is_ok());
        assert_eq!(track.takes.len(), takes_count + 1);

        // Normalize
        let norm_res = track.normalize_selected_take();
        assert!(norm_res.is_ok());
    }
}

