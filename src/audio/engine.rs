use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::command::AudioCommand;
use super::synth::SynthEngine;

#[allow(dead_code)]
pub struct AudioEngine {
    _stream: Stream,
    command_tx: Producer<AudioCommand>,
    peak_level: Arc<AtomicU32>,
    scope_rx: Consumer<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_name: String,
}

impl AudioEngine {
    pub fn new(ring_buffer_size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("Inget standard-ljudkort hittades på systemet")?;

        let device_name = device.name().unwrap_or_else(|_| "Standard Output".to_string());
        let default_config = device.default_output_config()?;
        let sample_format = default_config.sample_format();
        let config: StreamConfig = default_config.into();

        let sample_rate = config.sample_rate.0;
        let channels = config.channels;

        let (command_tx, command_rx) = RingBuffer::<AudioCommand>::new(ring_buffer_size);
        let peak_level = Arc::new(AtomicU32::new(0));
        let peak_level_clone = Arc::clone(&peak_level);

        // Ring buffer carrying the real (mono) output waveform from the audio
        // thread to the UI for the oscilloscope display.
        let (scope_tx, scope_rx) = RingBuffer::<f32>::new(65536);

        let stream = match sample_format {
            SampleFormat::F32 => Self::build_stream::<f32>(&device, &config, command_rx, peak_level_clone, scope_tx)?,
            SampleFormat::I16 => Self::build_stream::<i16>(&device, &config, command_rx, peak_level_clone, scope_tx)?,
            SampleFormat::U16 => Self::build_stream::<u16>(&device, &config, command_rx, peak_level_clone, scope_tx)?,
            _ => return Err("Ljudformatet stöds inte".into()),
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            command_tx,
            peak_level,
            scope_rx,
            sample_rate,
            channels,
            device_name,
        })
    }

    pub fn send_command(&mut self, cmd: AudioCommand) -> Result<(), rtrb::PushError<AudioCommand>> {
        self.command_tx.push(cmd)
    }

    pub fn get_peak_level(&self) -> f32 {
        let bits = self.peak_level.load(Ordering::Relaxed);
        f32::from_bits(bits)
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
