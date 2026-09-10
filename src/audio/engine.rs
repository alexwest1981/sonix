use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use super::command::AudioCommand;
use super::synth::SynthEngine;

#[allow(dead_code)]
pub struct AudioEngine {
    _stream: Option<Stream>,
    device: cpal::Device,
    sample_format: SampleFormat,
    command_capacity: usize,
    command_tx: Producer<AudioCommand>,
    peak_level: Arc<AtomicU32>,
    audition_active: Arc<AtomicBool>,
    master_gr_db: Arc<AtomicU32>,
    scope_rx: Consumer<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_frames: Option<u32>,
    pub device_name: String,
    pub host_name: String,
}

/// Persisted audio-hardware preferences (`~/.config/sonix/audio.json`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AudioSettings {
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub buffer_frames: Option<u32>,
}

impl AudioSettings {
    pub fn config_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
        Some(base.join("sonix").join("audio.json"))
    }

    pub fn load() -> Self {
        Self::config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<AudioSettings>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<std::path::PathBuf, String> {
        let path = Self::config_path().ok_or("Kunde inte hitta någon konfigurationsmapp")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Kunde inte skapa {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Kunde inte serialisera ljudinställningar: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Kunde inte skriva {}: {e}", path.display()))?;
        Ok(path)
    }
}

impl AudioEngine {
    /// Opens the default output device with the device's default settings.
    #[allow(dead_code)]
    pub fn new(ring_buffer_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with(ring_buffer_size, None, None)
    }

    /// Opens the default output device, optionally overriding the sample rate
    /// and buffer size. Falls back to the device defaults if the requested
    /// configuration cannot be opened.
    pub fn new_with(
        ring_buffer_size: usize,
        preferred_sample_rate: Option<u32>,
        preferred_buffer_frames: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let host_name = host.id().name().to_string();
        let device = host
            .default_output_device()
            .ok_or(crate::i18n::t("Inget standard-ljudkort hittades på systemet"))?;

        let device_name = device.name().unwrap_or_else(|_| "Standard Output".to_string());
        let default_config = device.default_output_config()?;
        let sample_format = default_config.sample_format();
        let default_stream_config: StreamConfig = default_config.into();

        let peak_level = Arc::new(AtomicU32::new(0));
        let audition_active = Arc::new(AtomicBool::new(false));
        let master_gr_db = Arc::new(AtomicU32::new(0));

        let mut config = default_stream_config.clone();
        apply_preferences(&device, &mut config, sample_format, preferred_sample_rate, preferred_buffer_frames);
        let preferences_changed = config.sample_rate != default_stream_config.sample_rate
            || config.buffer_size != default_stream_config.buffer_size;

        let (stream, command_tx, scope_rx) = match Self::open_stream(
            &device,
            sample_format,
            &config,
            ring_buffer_size,
            &peak_level,
            &audition_active,
            &master_gr_db,
        ) {
            Ok(v) => v,
            Err(_) if preferences_changed => {
                config = default_stream_config.clone();
                Self::open_stream(&device, sample_format, &config, ring_buffer_size, &peak_level, &audition_active, &master_gr_db)?
            }
            Err(e) => return Err(e),
        };

        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        let buffer_frames = match config.buffer_size {
            cpal::BufferSize::Fixed(f) => Some(f),
            cpal::BufferSize::Default => None,
        };

        Ok(Self {
            _stream: Some(stream),
            device,
            sample_format,
            command_capacity: ring_buffer_size,
            command_tx,
            peak_level,
            audition_active,
            master_gr_db,
            scope_rx,
            sample_rate,
            channels,
            buffer_frames,
            device_name,
            host_name,
        })
    }

    /// Rebuilds the output stream with a new sample rate / buffer size.
    /// The synth engine is recreated, so callers must re-send their state.
    pub fn reconfigure(
        &mut self,
        sample_rate: Option<u32>,
        buffer_frames: Option<u32>,
    ) -> Result<(), String> {
        let default_config = self.device.default_output_config().map_err(|e| e.to_string())?;
        let default_stream_config: StreamConfig = default_config.into();
        let mut config = default_stream_config.clone();
        apply_preferences(&self.device, &mut config, self.sample_format, sample_rate, buffer_frames);
        let preferences_changed = config.sample_rate != default_stream_config.sample_rate
            || config.buffer_size != default_stream_config.buffer_size;

        // Drop the old stream before opening a new one on the same device.
        self._stream = None;

        let opened = match Self::open_stream(
            &self.device,
            self.sample_format,
            &config,
            self.command_capacity,
            &self.peak_level,
            &self.audition_active,
            &self.master_gr_db,
        ) {
            Ok(v) => Ok((v, config.clone())),
            Err(_) if preferences_changed => {
                let fallback = default_stream_config.clone();
                match Self::open_stream(
                    &self.device,
                    self.sample_format,
                    &fallback,
                    self.command_capacity,
                    &self.peak_level,
                    &self.audition_active,
                    &self.master_gr_db,
                ) {
                    Ok(v) => Ok((v, fallback)),
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(e) => Err(e.to_string()),
        };

        match opened {
            Ok(((stream, command_tx, scope_rx), used)) => {
                self._stream = Some(stream);
                self.command_tx = command_tx;
                self.scope_rx = scope_rx;
                self.sample_rate = used.sample_rate.0;
                self.channels = used.channels;
                self.buffer_frames = match used.buffer_size {
                    cpal::BufferSize::Fixed(f) => Some(f),
                    cpal::BufferSize::Default => None,
                };
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn open_stream(
        device: &cpal::Device,
        sample_format: SampleFormat,
        config: &StreamConfig,
        ring_buffer_size: usize,
        peak_level: &Arc<AtomicU32>,
        audition_active: &Arc<AtomicBool>,
        master_gr_db: &Arc<AtomicU32>,
    ) -> Result<(Stream, Producer<AudioCommand>, Consumer<f32>), Box<dyn std::error::Error>> {
        let (command_tx, command_rx) = RingBuffer::<AudioCommand>::new(ring_buffer_size);
        let (scope_tx, scope_rx) = RingBuffer::<f32>::new(65536);

        let stream = match sample_format {
            SampleFormat::F32 => Self::build_stream::<f32>(device, config, command_rx, Arc::clone(peak_level), Arc::clone(audition_active), Arc::clone(master_gr_db), scope_tx)?,
            SampleFormat::I16 => Self::build_stream::<i16>(device, config, command_rx, Arc::clone(peak_level), Arc::clone(audition_active), Arc::clone(master_gr_db), scope_tx)?,
            SampleFormat::U16 => Self::build_stream::<u16>(device, config, command_rx, Arc::clone(peak_level), Arc::clone(audition_active), Arc::clone(master_gr_db), scope_tx)?,
            _ => return Err(crate::i18n::t("Ljudformatet stöds inte").into()),
        };

        stream.play()?;
        Ok((stream, command_tx, scope_rx))
    }

    pub fn send_command(&mut self, cmd: AudioCommand) -> Result<(), rtrb::PushError<AudioCommand>> {
        self.command_tx.push(cmd)
    }

    pub fn get_peak_level(&self) -> f32 {
        let bits = self.peak_level.load(Ordering::Relaxed);
        f32::from_bits(bits)
    }

    /// True while the isolated audition player (Vocal Studio / sound browser)
    /// is still producing audio. Cleared automatically when playback ends.
    pub fn is_audition_playing(&self) -> bool {
        self.audition_active.load(Ordering::Relaxed)
    }

    /// Current master-bus compressor gain reduction in dB (<= 0.0).
    pub fn master_gain_reduction_db(&self) -> f32 {
        f32::from_bits(self.master_gr_db.load(Ordering::Relaxed))
    }

    /// Pops every waveform sample produced since the last UI frame.
    pub fn drain_scope_samples(&mut self) -> Vec<f32> {
        let mut out = Vec::new();
        while let Ok(s) = self.scope_rx.pop() {
            out.push(s);
        }
        out
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        mut command_rx: Consumer<AudioCommand>,
        peak_level: Arc<AtomicU32>,
        audition_active: Arc<AtomicBool>,
        master_gr_db: Arc<AtomicU32>,
        mut scope_tx: Producer<f32>,
    ) -> Result<Stream, cpal::BuildStreamError>
    where
        T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample,
    {
        let sample_rate = config.sample_rate.0 as f32;
        let channels = config.channels as usize;
        let mut synth = SynthEngine::new(sample_rate);

        let err_fn = |err| {
            eprintln!("[Sonix Audio Error] {}", err);
            if let Ok(log) = std::env::var("HOME") {
                let path = std::path::Path::new(&log).join("Music/Sonix/audio_crash.log");
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = std::io::Write::write_all(
                        &mut f,
                        format!("[{}] [Audio Error] {}\n", std::process::id(), err).as_bytes(),
                    );
                }
            }
        };

        device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // 1. Process all pending commands from UI/Main thread without blocking
                    while let Ok(cmd) = command_rx.pop() {
                        synth.handle_command(cmd);
                    }

                    // 2. Render samples for this buffer block
                    let mut max_peak: f32 = 0.0;

                    for frame in data.chunks_mut(channels) {
                        let (sample_l, sample_r) = synth.process_stereo();
                        let peak_s = sample_l.abs().max(sample_r.abs());
                        if peak_s > max_peak {
                            max_peak = peak_s;
                        }
                        // Feed the real output waveform to the scope (mono mix).
                        // If the ring is full the oldest samples are dropped,
                        // which only makes the scope skip a beat.
                        let _ = scope_tx.push((sample_l + sample_r) * 0.5);
                        if frame.len() >= 2 {
                            frame[0] = cpal::Sample::from_sample(sample_l);
                            frame[1] = cpal::Sample::from_sample(sample_r);
                        } else if !frame.is_empty() {
                            frame[0] = cpal::Sample::from_sample((sample_l + sample_r) * 0.5);
                        }
                    }

                    // 3. Atomically store peak level for UI visualization
                    peak_level.store(max_peak.to_bits(), Ordering::Relaxed);

                    // 4. Mirror the real audition state so the UI can clear its
                    //    "playing" indicator once playback ends naturally.
                    audition_active.store(
                        synth.audition.as_ref().map(|a| a.is_playing).unwrap_or(false),
                        Ordering::Relaxed,
                    );

                    // 5. Mirror real compressor gain reduction for the FX rack meter.
                    master_gr_db.store(synth.master_fx.gain_reduction_db().to_bits(), Ordering::Relaxed);
                }));

                if let Err(panic) = res {
                    let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = panic.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "okänd panik".to_string()
                    };
                    eprintln!("[Sonix] AUDIO-KRASCH i ljudtråd: {}", msg);
                    if let Ok(log) = std::env::var("HOME") {
                        let path = std::path::Path::new(&log)
                            .join("Music/Sonix/audio_crash.log");
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                            let _ = std::io::Write::write_all(
                                &mut f,
                                format!("[{}] {}\n", std::process::id(), msg).as_bytes(),
                            );
                        }
                    }
                }
            },
            err_fn,
            None,
        )
    }
}

/// Applies requested sample rate / buffer size to a stream config when the
/// device reports support for them. Unsupported requests are ignored so the
/// caller falls back to the device defaults.
fn apply_preferences(
    device: &cpal::Device,
    config: &mut StreamConfig,
    sample_format: SampleFormat,
    sample_rate: Option<u32>,
    buffer_frames: Option<u32>,
) {
    if let Some(sr) = sample_rate {
        let supported = device
            .supported_output_configs()
            .map(|mut configs| {
                configs.any(|range| {
                    range.sample_format() == sample_format
                        && range.min_sample_rate().0 <= sr
                        && sr <= range.max_sample_rate().0
                })
            })
            .unwrap_or(true);
        if supported {
            config.sample_rate = cpal::SampleRate(sr);
        }
    }
    if let Some(frames) = buffer_frames
        && frames > 0
    {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
}
