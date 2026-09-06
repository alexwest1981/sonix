use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use eframe::egui::Color32;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use super::wav_writer::write_pcm_f32_to_wav;

#[derive(Debug, Clone)]
pub struct AudioTake {
    pub take_number: usize,
    pub name: String,
    pub pcm_samples: Vec<f32>,
    pub waveform_data: Vec<f32>,
    pub duration_secs: f32,
    pub is_selected: bool,
    pub color: Color32,
    pub start_bar: usize,
    pub length_bars: usize,
    pub trim_start_norm: f32,
    pub trim_end_norm: f32,
    pub gain_linear: f32,
    pub is_muted: bool,
    pub is_playing: bool,
}

#[derive(Debug, Clone)]
pub struct CustomSoundClip {
    pub id: usize,
    pub name: String,
    pub category: String,
    pub pcm_samples: Vec<f32>,
    pub waveform_data: Vec<f32>,
    pub duration_secs: f32,
    pub color: Color32,
}

#[derive(Debug, Clone)]
pub struct CompRegion {
    pub start_norm: f32, // 0.0 .. 1.0
    pub end_norm: f32,
    pub from_take: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    LeadVocals,
    CustomSounds,
}

pub struct LiveMicrophoneCapture {
    pub is_recording: Arc<AtomicBool>,
    pub is_paused: Arc<AtomicBool>,
    pub input_gain: Arc<Mutex<f32>>,
    pub peak_vu: Arc<AtomicU32>,
    pub recorded_samples: Arc<Mutex<Vec<f32>>>,
    pub live_peaks: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub device_name: String,
    _stream: Option<Stream>,
}

impl LiveMicrophoneCapture {
    pub fn new() -> Self {
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let input_gain = Arc::new(Mutex::new(1.0));
        let peak_vu = Arc::new(AtomicU32::new(0));
        let recorded_samples = Arc::new(Mutex::new(Vec::with_capacity(44100 * 30)));
        let live_peaks = Arc::new(Mutex::new(Vec::with_capacity(1000)));

        let host = cpal::default_host();
        let (device_name, stream, sample_rate) = if let Some(device) = host.default_input_device() {
            let name = device.name().unwrap_or_else(|_| "PipeWire / ALSA Standardmikrofon".to_string());
            if let Ok(default_config) = device.default_input_config() {
                let sr = default_config.sample_rate().0;
                let sample_format = default_config.sample_format();
                let config: StreamConfig = default_config.into();

                let is_rec = Arc::clone(&is_recording);
                let is_p = Arc::clone(&is_paused);
                let gain_ref = Arc::clone(&input_gain);
                let vu_ref = Arc::clone(&peak_vu);
                let samples_ref = Arc::clone(&recorded_samples);
                let peaks_ref = Arc::clone(&live_peaks);

                let stream_res = match sample_format {
                    SampleFormat::F32 => device.build_input_stream(
                        &config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let mut block_max: f32 = 0.0;
                            for &s in data {
                                let val = (s * gain).abs();
                                if val > block_max { block_max = val; }
                            }
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);

                            if is_rec.load(Ordering::Relaxed) && !is_p.load(Ordering::Relaxed) {
                                let mut b = samples_ref.lock().unwrap_or_else(|e| e.into_inner());
                                let mut p = peaks_ref.lock().unwrap_or_else(|e| e.into_inner());
                                for &s in data {
                                    let sample = (s * gain).clamp(-1.0, 1.0);
                                    b.push(sample);
                                    if b.len() % 512 == 0 {
                                        p.push(block_max.clamp(0.02, 1.0));
                                    }
                                }
                            }
                        },
                        |err| eprintln!("[Sonix Mic Input Error] {}", err),
                        None,
                    ),
                    SampleFormat::I16 => device.build_input_stream(
                        &config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            let gain = *gain_ref.lock().unwrap_or_else(|e| e.into_inner());
                            let mut block_max: f32 = 0.0;
                            for &s in data {
                                let s_f32 = (s as f32 / 32768.0) * gain;
                                let val = s_f32.abs();
                                if val > block_max { block_max = val; }
                            }
                            vu_ref.store(block_max.to_bits(), Ordering::Relaxed);

                            if is_rec.load(Ordering::Relaxed) && !is_p.load(Ordering::Relaxed) {
                                let mut b = samples_ref.lock().unwrap_or_else(|e| e.into_inner());
                                let mut p = peaks_ref.lock().unwrap_or_else(|e| e.into_inner());
                                for &s in data {
                                    let s_f32 = ((s as f32 / 32768.0) * gain).clamp(-1.0, 1.0);
                                    b.push(s_f32);
                                    if b.len() % 512 == 0 {
                                        p.push(block_max.clamp(0.02, 1.0));
                                    }
                                }
                            }
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
                ("Standard Mikrofon (PipeWire/ALSA)".to_string(), None, 44100)
            }
        } else {
            ("Mikrofon (Simulerad / Standby)".to_string(), None, 44100)
        };

        Self {
            is_recording,
            is_paused,
            input_gain,
            peak_vu,
            recorded_samples,
            live_peaks,
            sample_rate,
            device_name,
            _stream: stream,
        }
    }
}

pub struct VocalStudioTrack {
    pub name: String,
    pub input_channel: String,
    pub is_armed: bool,
    pub is_recording: bool,
    pub is_paused: bool,
    pub recording_mode: RecordingMode,
    pub recording_elapsed_secs: f32,
    pub monitoring_on: bool,
    pub input_gain: f32,
    pub mic_vu_level: f32,
    pub custom_sample_name_input: String,
    pub takes: Vec<AudioTake>,
    pub custom_sounds: Vec<CustomSoundClip>,
    pub comp_regions: Vec<CompRegion>,
    pub active_comp_take: usize,
    pub live_recording_peaks: Vec<f32>,
    pub selected_crop_start_norm: f32,
    pub selected_crop_end_norm: f32,
    pub mic_capture: Option<LiveMicrophoneCapture>,
}

impl Default for VocalStudioTrack {
    fn default() -> Self {
        let mic = LiveMicrophoneCapture::new();
        let dev_name = mic.device_name.clone();

        let mut track = Self {
            name: "🎤 Lead Sång (Studio Recorder)".to_string(),
            input_channel: dev_name,
            is_armed: true,
            is_recording: false,
            is_paused: false,
            recording_mode: RecordingMode::LeadVocals,
            recording_elapsed_secs: 0.0,
            monitoring_on: true,
            input_gain: 1.0,
            mic_vu_level: 0.0,
            custom_sample_name_input: "Mitt Akustiska Ljud 1".to_string(),
            takes: Vec::new(),
            custom_sounds: Vec::new(),
            comp_regions: Vec::new(),
            active_comp_take: 0,
            live_recording_peaks: Vec::new(),
            selected_crop_start_norm: 0.0,
            selected_crop_end_norm: 1.0,
            mic_capture: Some(mic),
        };
        track.populate_demo_takes();
        track.populate_demo_custom_sounds();
        track
    }
}

impl VocalStudioTrack {
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

                // Sync live peaks
                if let Ok(peaks) = mic.live_peaks.lock() {
                    self.live_recording_peaks = peaks.clone();
                }

                // If hardware mic isn't streaming, generate graceful simulated mic wave
                if self.live_recording_peaks.is_empty() || mic._stream.is_none() {
                    let t = self.recording_elapsed_secs;
                    let sim_peak = ((t * 8.0).sin().abs() * 0.7 + (t * 22.0).cos().abs() * 0.25).clamp(0.05, 0.95);
                    self.live_recording_peaks.push(sim_peak);
                    if self.live_recording_peaks.len() > 300 {
                        self.live_recording_peaks.remove(0);
                    }
                    self.mic_vu_level = sim_peak;
                }
            }
        }
    }

    pub fn start_recording(&mut self) {
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

    pub fn stop_recording(&mut self, bpm: f32) -> Option<usize> {
        self.is_recording = false;
        self.is_paused = false;

        let mut samples = Vec::new();
        if let Some(ref mic) = self.mic_capture {
            mic.is_recording.store(false, Ordering::Relaxed);
            mic.is_paused.store(false, Ordering::Relaxed);
            if let Ok(s) = mic.recorded_samples.lock() {
                samples = s.clone();
            }
        }

        // If simulated or empty, generate sample data
        if samples.is_empty() {
            let total_s = ((self.recording_elapsed_secs.max(1.5)) * 44100.0) as usize;
            samples = (0..total_s).map(|i| {
                let t = i as f32 / 44100.0;
                (t * 330.0 * std::f32::consts::TAU).sin() * 0.6 + (t * 660.0 * std::f32::consts::TAU).sin() * 0.2
            }).collect();
        }

        let duration = self.recording_elapsed_secs.max(samples.len() as f32 / 44100.0);
        let sec_per_bar = (60.0 / bpm.max(40.0)) * 4.0;
        let length_bars = (duration / sec_per_bar).ceil().max(1.0) as usize;

        let peaks_count = 120;
        let step = (samples.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in samples.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.04, 0.98));
        }

        let count = self.takes.len() + 1;
        let color = match count % 4 {
            0 => Color32::from_rgb(0, 200, 240),
            1 => Color32::from_rgb(255, 140, 0),
            2 => Color32::from_rgb(180, 100, 255),
            _ => Color32::from_rgb(80, 240, 160),
        };

        let new_take = AudioTake {
            take_number: count,
            name: format!("Tagning {} ({:.1}s)", count, duration),
            pcm_samples: samples,
            waveform_data: visual_peaks,
            duration_secs: duration,
            is_selected: true,
            color,
            start_bar: 0,
            length_bars,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            is_muted: false,
            is_playing: false,
        };

        for t in &mut self.takes { t.is_selected = false; }
        self.takes.push(new_take);
        self.active_comp_take = self.takes.len() - 1;
        self.selected_crop_start_norm = 0.0;
        self.selected_crop_end_norm = 1.0;

        Some(self.active_comp_take)
    }

    pub fn crop_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err("Ingen tagning är vald att beskära".to_string());
        }

        let start_n = self.selected_crop_start_norm.clamp(0.0, 0.99);
        let end_n = self.selected_crop_end_norm.clamp(start_n + 0.01, 1.0);

        let take = &mut self.takes[self.active_comp_take];
        let total_samples = take.pcm_samples.len();
        if total_samples < 100 {
            return Err("Tagningen innehåller för lite data för att beskäras".to_string());
        }

        let s_idx = (total_samples as f32 * start_n) as usize;
        let e_idx = ((total_samples as f32 * end_n) as usize).min(total_samples);

        take.pcm_samples = take.pcm_samples[s_idx..e_idx].to_vec();
        take.duration_secs = take.pcm_samples.len() as f32 / 44100.0;

        // Recompute peaks
        let peaks_count = 120;
        let step = (take.pcm_samples.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in take.pcm_samples.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.04, 0.98));
        }
        take.waveform_data = visual_peaks;
        take.trim_start_norm = 0.0;
        take.trim_end_norm = 1.0;

        self.selected_crop_start_norm = 0.0;
        self.selected_crop_end_norm = 1.0;

        Ok(format!("✔ Beskärde {} till {:.2} sekunder!", take.name, take.duration_secs))
    }

    pub fn slice_selected_take_at(&mut self, split_norm: f32) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err("Ingen tagning är vald att klippa".to_string());
        }

        let split_n = split_norm.clamp(0.05, 0.95);
        let orig_take = self.takes[self.active_comp_take].clone();
        let total = orig_take.pcm_samples.len();
        let split_idx = (total as f32 * split_n) as usize;

        let part1_samples = orig_take.pcm_samples[..split_idx].to_vec();
        let part2_samples = orig_take.pcm_samples[split_idx..].to_vec();

        let make_peaks = |samples: &[f32]| -> Vec<f32> {
            let count = 100;
            let step = (samples.len() / count).max(1);
            samples.chunks(step).map(|c| c.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs())).clamp(0.04, 0.98)).collect()
        };

        let t1_peaks = make_peaks(&part1_samples);
        let t2_peaks = make_peaks(&part2_samples);

        // Update Part 1
        self.takes[self.active_comp_take].name = format!("{} (Del 1)", orig_take.name);
        self.takes[self.active_comp_take].pcm_samples = part1_samples.clone();
        self.takes[self.active_comp_take].waveform_data = t1_peaks;
        self.takes[self.active_comp_take].duration_secs = part1_samples.len() as f32 / 44100.0;

        // Insert Part 2 as new take
        let count = self.takes.len() + 1;
        self.takes.push(AudioTake {
            take_number: count,
            name: format!("{} (Del 2)", orig_take.name),
            pcm_samples: part2_samples.clone(),
            waveform_data: t2_peaks,
            duration_secs: part2_samples.len() as f32 / 44100.0,
            is_selected: true,
            color: Color32::from_rgb(255, 100, 180),
            start_bar: 0,
            length_bars: 4,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            is_muted: false,
            is_playing: false,
        });

        self.active_comp_take = self.takes.len() - 1;
        Ok("✔ Klippte tagningen i två separata delar!".to_string())
    }

    pub fn export_take_to_wav(&self, take_idx: usize, path: &str) -> Result<String, String> {
        if let Some(take) = self.takes.get(take_idx) {
            let sr = self.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            write_pcm_f32_to_wav(path, &take.pcm_samples, sr, 1)
                .map_err(|e| format!("Kunde inte skriva WAV-fil: {}", e))?;
            Ok(format!("✔ Sparade tagning '{}' till {}", take.name, path))
        } else {
            Err("Tagningen hittades inte".to_string())
        }
    }

    pub fn normalize_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err("Ingen tagning är vald att normalisera".to_string());
        }
        let take = &mut self.takes[self.active_comp_take];
        let max_amp = take.pcm_samples.iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        if max_amp < 0.0001 {
            return Err("Ljudspåret är helt tyst".to_string());
        }
        let mult = 0.98 / max_amp;
        for s in &mut take.pcm_samples {
            *s = (*s * mult).clamp(-1.0, 1.0);
        }
        let peaks_count = 120;
        let step = (take.pcm_samples.len() / peaks_count).max(1);
        let mut visual_peaks = Vec::with_capacity(peaks_count);
        for chunk in take.pcm_samples.chunks(step) {
            let peak = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            visual_peaks.push(peak.clamp(0.04, 0.98));
        }
        take.waveform_data = visual_peaks;
        Ok(format!("✔ Normaliserade {} (Peak förstärkt med {:.1}x)!", take.name, mult))
    }

    pub fn delete_selected_take(&mut self) -> Result<String, String> {
        if self.takes.is_empty() || self.active_comp_take >= self.takes.len() {
            return Err("Ingen tagning är vald att ta bort".to_string());
        }
        let removed = self.takes.remove(self.active_comp_take);
        if !self.takes.is_empty() {
            self.active_comp_take = self.active_comp_take.min(self.takes.len() - 1);
            self.takes[self.active_comp_take].is_selected = true;
        } else {
            self.active_comp_take = 0;
        }
        Ok(format!("✔ Tog bort {}", removed.name))
    }

    pub fn populate_demo_takes(&mut self) {
        self.takes.clear();
        let samples_count = 44100 * 4;

        // Take 1: First run-through
        let mut take1_pcm = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let t = i as f32 / 44100.0;
            let val = if (0.5..1.8).contains(&t) || (2.2..3.5).contains(&t) {
                (t * 220.0 * std::f32::consts::TAU).sin() * 0.65 + (t * 440.0 * std::f32::consts::TAU).cos() * 0.15
            } else {
                0.02
            };
            take1_pcm.push(val);
        }

        let peaks1: Vec<f32> = (0..100).map(|i| {
            let t = i as f32 / 100.0;
            if (0.1..0.45).contains(&t) || (0.55..0.9).contains(&t) {
                (t * 30.0).sin().abs() * 0.75 + 0.1
            } else {
                0.05
            }
        }).collect();

        self.takes.push(AudioTake {
            take_number: 1,
            name: "Tagning 1 (Intro & Vers)".to_string(),
            pcm_samples: take1_pcm,
            waveform_data: peaks1,
            duration_secs: 4.0,
            is_selected: false,
            color: Color32::from_rgb(0, 200, 240),
            start_bar: 0,
            length_bars: 8,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            is_muted: false,
            is_playing: false,
        });

        // Take 2: Strong chorus
        let mut take2_pcm = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let t = i as f32 / 44100.0;
            let val = if (0.4..3.6).contains(&t) {
                (t * 260.0 * std::f32::consts::TAU).sin() * 0.85 + (t * 520.0 * std::f32::consts::TAU).cos() * 0.2
            } else {
                0.02
            };
            take2_pcm.push(val);
        }

        let peaks2: Vec<f32> = (0..100).map(|i| {
            let t = i as f32 / 100.0;
            if (0.15..0.92).contains(&t) {
                (t * 35.0).sin().abs() * 0.88 + 0.08
            } else {
                0.04
            }
        }).collect();

        self.takes.push(AudioTake {
            take_number: 2,
            name: "Tagning 2 (Stark Refräng)".to_string(),
            pcm_samples: take2_pcm,
            waveform_data: peaks2,
            duration_secs: 4.0,
            is_selected: true,
            color: Color32::from_rgb(255, 140, 0),
            start_bar: 8,
            length_bars: 8,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            is_muted: false,
            is_playing: false,
        });

        self.comp_regions = vec![
            CompRegion { start_norm: 0.0, end_norm: 0.45, from_take: 0 },
            CompRegion { start_norm: 0.45, end_norm: 1.0, from_take: 1 },
        ];
    }

    pub fn populate_demo_custom_sounds(&mut self) {
        self.custom_sounds.clear();
        let samples = 44100;

        let s1_pcm: Vec<f32> = (0..samples).map(|i| {
            let t = i as f32 / 44100.0;
            (1.0 - t).powi(2) * (t * 800.0 * std::f32::consts::TAU).sin() * 0.8
        }).collect();

        let s1_wave: Vec<f32> = (0..60).map(|i| {
            let t = i as f32 / 60.0;
            ((1.0 - t).powi(2) * (t * 50.0).sin().abs()).clamp(0.05, 0.9)
        }).collect();

        self.custom_sounds.push(CustomSoundClip {
            id: 1,
            name: "👏 Akustisk Handklapp".to_string(),
            category: "Perkussion".to_string(),
            pcm_samples: s1_pcm,
            waveform_data: s1_wave,
            duration_secs: 1.0,
            color: Color32::from_rgb(255, 120, 80),
        });
    }

    pub fn add_new_take(&mut self, name: &str, start_bar: usize, length_bars: usize) {
        let count = self.takes.len() + 1;
        let mut new_wave = Vec::with_capacity(100);
        for i in 0..100 {
            let t = i as f32 / 100.0;
            let val = (t * 24.0).sin().abs() * 0.8 + (t * 48.0).cos().abs() * 0.15;
            new_wave.push(val.clamp(0.03, 0.95));
        }
        let color = match count % 4 {
            0 => Color32::from_rgb(0, 200, 240),
            1 => Color32::from_rgb(255, 140, 0),
            2 => Color32::from_rgb(180, 100, 255),
            _ => Color32::from_rgb(80, 240, 160),
        };
        self.takes.push(AudioTake {
            take_number: count,
            name: if name.is_empty() { format!("Tagning {}", count) } else { name.to_string() },
            pcm_samples: vec![0.0; 44100 * 4],
            waveform_data: new_wave,
            duration_secs: 4.0,
            is_selected: true,
            color,
            start_bar,
            length_bars,
            trim_start_norm: 0.0,
            trim_end_norm: 1.0,
            gain_linear: 1.0,
            is_muted: false,
            is_playing: false,
        });
        self.active_comp_take = self.takes.len() - 1;
    }

    pub fn add_custom_sound(&mut self, name: &str, category: &str) {
        let count = self.custom_sounds.len() + 1;
        let mut new_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = ((1.0 - t) * (t * 36.0).sin().abs()).clamp(0.05, 0.95);
            new_wave.push(val);
        }
        self.custom_sounds.push(CustomSoundClip {
            id: count,
            name: if name.is_empty() { format!("Eget Ljud {}", count) } else { name.to_string() },
            category: if category.is_empty() { "Eget Ljud".to_string() } else { category.to_string() },
            pcm_samples: vec![0.0; 44100 * 2],
            waveform_data: new_wave,
            duration_secs: 1.5,
            color: Color32::from_rgb(255, 180, 60),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vocal_studio_recording_workflow() {
        let mut track = VocalStudioTrack::default();
        let initial_takes = track.takes.len();

        // 1. Start recording
        track.start_recording();
        assert!(track.is_recording);
        assert!(!track.is_paused);

        // 2. Pause and Resume
        track.pause_recording();
        assert!(track.is_paused);
        track.resume_recording();
        assert!(!track.is_paused);

        // 3. Update stream
        track.update_live_stream();

        // 4. Stop recording and create take
        let new_idx = track.stop_recording(120.0).expect("Should create a take");
        assert_eq!(track.takes.len(), initial_takes + 1);
        assert_eq!(track.active_comp_take, new_idx);

        let take = &track.takes[new_idx];
        assert!(!take.pcm_samples.is_empty());
        assert!(!take.waveform_data.is_empty());
    }

    #[test]
    fn test_vocal_studio_crop_and_slice() {
        let mut track = VocalStudioTrack::default();
        track.populate_demo_takes();
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

