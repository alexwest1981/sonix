use std::fs::File;
use std::io::{BufWriter, Write};
use super::synth::SynthEngine;
use super::command::AudioCommand;
use super::drum::DrumType;

pub fn render_to_wav(
    path: &str,
    mut synth: SynthEngine,
    pattern_steps: &[[bool; 16]],
    step_notes: &[u8; 16],
    bpm: f32,
    num_bars: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let sample_rate = synth.sample_rate as u32;
    let step_duration_secs = (60.0 / bpm) / 4.0;
    let step_samples = (step_duration_secs * sample_rate as f32) as usize;
    let total_steps = 16 * num_bars;
    let total_samples = total_steps * step_samples;

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Write WAV 44-byte RIFF Header
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = num_channels * (bits_per_sample / 8);
    let data_chunk_size = total_samples as u32 * block_align as u32;
    let riff_chunk_size = 36 + data_chunk_size;

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_chunk_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    writer.write_all(&1_u16.to_le_bytes())?;  // AudioFormat (1 for PCM)
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_chunk_size.to_le_bytes())?;

    // Render Audio Samples
    for step in 0..total_steps {
        let pattern_step = step % 16;

        // Trigger drums & notes across 8 channels
        if !pattern_steps.is_empty() && pattern_steps[0][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Kick));
        }
        if pattern_steps.len() > 1 && pattern_steps[1][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Snare));
        }
        if pattern_steps.len() > 2 && pattern_steps[2][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Clap));
        }
        if pattern_steps.len() > 3 && pattern_steps[3][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
        }
        if pattern_steps.len() > 4 && pattern_steps[4][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
        }
        if pattern_steps.len() > 5 && pattern_steps[5][pattern_step] {
            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Crash));
        }
        if pattern_steps.len() > 6 && pattern_steps[6][pattern_step] {
            let note = step_notes[pattern_step];
            let freq = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
            synth.handle_command(AudioCommand::NoteOn { note, freq, velocity: 0.85 });
        }

        for _ in 0..step_samples {
            let (sample_l, sample_r) = synth.process_stereo();
            let i16_l = (sample_l.clamp(-1.0, 1.0) * 32767.0) as i16;
            let i16_r = (sample_r.clamp(-1.0, 1.0) * 32767.0) as i16;
            writer.write_all(&i16_l.to_le_bytes())?;
            writer.write_all(&i16_r.to_le_bytes())?;
        }
    }

    writer.flush()?;
    Ok(path.to_string())
}

pub struct SongArrangementExport {
    pub pattern_steps: Vec<Vec<[bool; 16]>>,
    pub pattern_notes: Vec<Vec<[u8; 16]>>,
    pub track_clips: Vec<[Option<usize>; 32]>,
    pub track_muted: Vec<bool>,
    pub num_bars: usize,
    pub bpm: f32,
}

pub fn render_song_arrangement_to_wav(
    path: &str,
    mut synth: SynthEngine,
    arrangement: &SongArrangementExport,
) -> Result<String, Box<dyn std::error::Error>> {
    let sample_rate = synth.sample_rate as u32;
    let step_duration_secs = (60.0 / arrangement.bpm) / 4.0;
    let step_samples = (step_duration_secs * sample_rate as f32) as usize;
    let total_steps = 16 * arrangement.num_bars;
    let total_samples = total_steps * step_samples;

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = num_channels * (bits_per_sample / 8);
    let data_chunk_size = total_samples as u32 * block_align as u32;
    let riff_chunk_size = 36 + data_chunk_size;

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_chunk_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?;
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_chunk_size.to_le_bytes())?;

    for bar in 0..arrangement.num_bars {
        for step_in_bar in 0..16 {
            // Check all tracks for active clips in this bar
            for (track_idx, clips) in arrangement.track_clips.iter().enumerate() {
                if arrangement.track_muted.get(track_idx).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(pat_idx) = clips[bar]
                    && let Some(channels) = arrangement.pattern_steps.get(pat_idx) {
                        // Drums
                        if !channels.is_empty() && channels[0][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Kick));
                        }
                        if channels.len() > 1 && channels[1][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Snare));
                        }
                        if channels.len() > 2 && channels[2][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Clap));
                        }
                        if channels.len() > 3 && channels[3][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
                        }
                        if channels.len() > 4 && channels[4][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
                        }
                        if channels.len() > 5 && channels[5][step_in_bar] {
                            synth.handle_command(AudioCommand::TriggerDrum(DrumType::Crash));
                        }
                        // Synth Lead (Channel 6)
                        if channels.len() > 6 && channels[6][step_in_bar] {
                            let note = arrangement.pattern_notes[pat_idx][6][step_in_bar];
                            let freq = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
                            synth.handle_command(AudioCommand::NoteOn { note, freq, velocity: 0.85 });
                        }
                        // Bass (Channel 7)
                        if channels.len() > 7 && channels[7][step_in_bar] {
                            let note = arrangement.pattern_notes[pat_idx][7][step_in_bar];
                            let freq = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
                            synth.handle_command(AudioCommand::NoteOn { note, freq, velocity: 0.9 });
                        }
                    }
            }

            for _ in 0..step_samples {
                let (sample_l, sample_r) = synth.process_stereo();
                let i16_l = (sample_l.clamp(-1.0, 1.0) * 32767.0) as i16;
                let i16_r = (sample_r.clamp(-1.0, 1.0) * 32767.0) as i16;
                writer.write_all(&i16_l.to_le_bytes())?;
                writer.write_all(&i16_r.to_le_bytes())?;
            }
        }
    }

    writer.flush()?;
    Ok(path.to_string())
}

pub fn write_pcm_f32_to_wav(
    path: &str,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    let bits_per_sample: u16 = 16;
    let num_channels = channels.max(1);
    let total_samples = samples.len() as u32;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = num_channels * (bits_per_sample / 8);
    let data_chunk_size = total_samples * (bits_per_sample / 8) as u32;
    let riff_chunk_size = 36 + data_chunk_size;

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_chunk_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    writer.write_all(&1_u16.to_le_bytes())?;  // AudioFormat (1 for PCM)
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_chunk_size.to_le_bytes())?;

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        writer.write_all(&i16_sample.to_le_bytes())?;
    }

    writer.flush()?;
    Ok(())
}

