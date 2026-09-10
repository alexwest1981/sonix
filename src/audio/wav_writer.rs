use std::fs::File;
use std::io::{BufWriter, Write};

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

