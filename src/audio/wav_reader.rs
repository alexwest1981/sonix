use std::fs::File;
use std::io::Read;

/// Reads a standard 16-bit, 24-bit or 32-bit float RIFF/WAVE file and generates a normalized peak amplitude waveform envelope.
#[allow(unused_variables, unused_assignments)]
pub fn read_wav_envelope(path: &str, points_count: usize) -> Result<Vec<f32>, String> {
    let mut file = File::open(path).map_err(|e| format!("Kunde inte öppna fil: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| format!("Kunde inte läsa fil: {}", e))?;

    if buffer.len() < 44 {
        return Err("Filen är för kort för att vara en giltig WAV-fil".to_string());
    }

    if &buffer[0..4] != b"RIFF" || &buffer[8..12] != b"WAVE" {
        return Err("Ogiltigt RIFF/WAVE header-format".to_string());
    }

    // Find 'fmt ' and 'data' chunks
    let mut pos = 12;
    let mut channels = 2u16;
    #[allow(unused_variables, unused_assignments)]
    let mut sample_rate = 44100u32;
    let mut bits_per_sample = 16u16;
    let mut audio_format = 1u16; // 1 = PCM, 3 = IEEE Float
    let mut data_offset = 0;
    let mut data_size = 0;

    while pos + 8 <= buffer.len() {
        let chunk_id = &buffer[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([buffer[pos + 4], buffer[pos + 5], buffer[pos + 6], buffer[pos + 7]]) as usize;
        let chunk_data_start = pos + 8;

        if chunk_id == b"fmt " && chunk_data_start + 16 <= buffer.len() {
            audio_format = u16::from_le_bytes([buffer[chunk_data_start], buffer[chunk_data_start + 1]]);
            channels = u16::from_le_bytes([buffer[chunk_data_start + 2], buffer[chunk_data_start + 3]]);
            sample_rate = u32::from_le_bytes([buffer[chunk_data_start + 4], buffer[chunk_data_start + 5], buffer[chunk_data_start + 6], buffer[chunk_data_start + 7]]);
            bits_per_sample = u16::from_le_bytes([buffer[chunk_data_start + 14], buffer[chunk_data_start + 15]]);
        } else if chunk_id == b"data" {
            data_offset = chunk_data_start;
            data_size = chunk_size.min(buffer.len().saturating_sub(data_offset));
            break;
        }

        pos = chunk_data_start + chunk_size;
    }

    if data_offset == 0 || data_size == 0 {
        data_offset = 44.min(buffer.len());
        data_size = buffer.len().saturating_sub(data_offset);
    }

    let bytes_per_sample = (bits_per_sample / 8).max(1) as usize;
    let block_align = channels.max(1) as usize * bytes_per_sample;
    let total_samples = data_size / block_align;

    if total_samples == 0 {
        return Ok(vec![0.1; points_count]);
    }

    let mut envelope = Vec::with_capacity(points_count);
    let chunk_samples = (total_samples / points_count).max(1);

    for point_idx in 0..points_count {
        let start_sample = point_idx * chunk_samples;
        let end_sample = (start_sample + chunk_samples).min(total_samples);
        let mut peak: f32 = 0.0;

        let step = ((end_sample - start_sample) / 128).max(1);
        let mut s_idx = start_sample;
        while s_idx < end_sample {
            let byte_pos = data_offset + s_idx * block_align;
            if byte_pos + bytes_per_sample <= buffer.len() {
                let sample_val = match (audio_format, bits_per_sample) {
                    (1, 16) => {
                        let raw = i16::from_le_bytes([buffer[byte_pos], buffer[byte_pos + 1]]);
                        (raw as f32 / 32768.0).abs()
                    }
                    (1, 24) => {
                        let raw = ((buffer[byte_pos + 2] as i32) << 24 | (buffer[byte_pos + 1] as i32) << 16 | (buffer[byte_pos] as i32) << 8) >> 8;
                        (raw as f32 / 8388608.0).abs()
                    }
                    (3, 32) => {
                        let raw = f32::from_le_bytes([buffer[byte_pos], buffer[byte_pos + 1], buffer[byte_pos + 2], buffer[byte_pos + 3]]);
                        raw.abs()
                    }
                    _ => {
                        let raw = buffer[byte_pos] as f32 / 128.0 - 1.0;
                        raw.abs()
                    }
                };
                if sample_val > peak {
                    peak = sample_val;
                }
            }
            s_idx += step;
        }

        envelope.push(peak.clamp(0.04, 0.98));
    }

    Ok(envelope)
}

/// Loads full decoded stereo PCM audio samples [-1.0, 1.0] from a WAV file.
#[allow(unused_variables, unused_assignments)]
pub fn load_wav_pcm(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let mut file = File::open(path).map_err(|e| format!("Kunde inte öppna fil: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| format!("Kunde inte läsa fil: {}", e))?;

    if buffer.len() < 44 || &buffer[0..4] != b"RIFF" || &buffer[8..12] != b"WAVE" {
        return Err("Ogiltigt WAV-format".to_string());
    }

    let mut pos = 12;
    let mut channels = 2u16;
    let mut sample_rate = 44100u32;
    let mut bits_per_sample = 16u16;
    let mut audio_format = 1u16;
    let mut data_offset = 0;
    let mut data_size = 0;

    while pos + 8 <= buffer.len() {
        let chunk_id = &buffer[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([buffer[pos + 4], buffer[pos + 5], buffer[pos + 6], buffer[pos + 7]]) as usize;
        let chunk_data_start = pos + 8;

        if chunk_id == b"fmt " && chunk_data_start + 16 <= buffer.len() {
            audio_format = u16::from_le_bytes([buffer[chunk_data_start], buffer[chunk_data_start + 1]]);
            channels = u16::from_le_bytes([buffer[chunk_data_start + 2], buffer[chunk_data_start + 3]]);
            sample_rate = u32::from_le_bytes([buffer[chunk_data_start + 4], buffer[chunk_data_start + 5], buffer[chunk_data_start + 6], buffer[chunk_data_start + 7]]);
            bits_per_sample = u16::from_le_bytes([buffer[chunk_data_start + 14], buffer[chunk_data_start + 15]]);
        } else if chunk_id == b"data" {
            data_offset = chunk_data_start;
            data_size = chunk_size.min(buffer.len().saturating_sub(data_offset));
            break;
        }

        pos = chunk_data_start + chunk_size;
    }

    if data_offset == 0 || data_size == 0 {
        data_offset = 44.min(buffer.len());
        data_size = buffer.len().saturating_sub(data_offset);
    }

    let bytes_per_sample = (bits_per_sample / 8).max(1) as usize;
    let num_ch = channels.max(1) as usize;
    let block_align = num_ch * bytes_per_sample;
    let total_frames = data_size / block_align;

    let mut left = Vec::with_capacity(total_frames);
    let mut right = Vec::with_capacity(total_frames);

    for f_idx in 0..total_frames {
        let base = data_offset + f_idx * block_align;
        if base + block_align > buffer.len() {
            break;
        }

        let read_sample = |byte_idx: usize| -> f32 {
            if byte_idx + bytes_per_sample > buffer.len() {
                return 0.0;
            }
            match (audio_format, bits_per_sample) {
                (1, 16) => {
                    let raw = i16::from_le_bytes([buffer[byte_idx], buffer[byte_idx + 1]]);
                    raw as f32 / 32768.0
                }
                (1, 24) => {
                    let raw = ((buffer[byte_idx + 2] as i32) << 24 | (buffer[byte_idx + 1] as i32) << 16 | (buffer[byte_idx] as i32) << 8) >> 8;
                    raw as f32 / 8388608.0
                }
                (3, 32) => {
                    f32::from_le_bytes([buffer[byte_idx], buffer[byte_idx + 1], buffer[byte_idx + 2], buffer[byte_idx + 3]])
                }
                _ => {
                    buffer[byte_idx] as f32 / 128.0 - 1.0
                }
            }
        };

        let l_sample = read_sample(base);
        let r_sample = if num_ch >= 2 {
            read_sample(base + bytes_per_sample)
        } else {
            l_sample
        };

        left.push(l_sample);
        right.push(r_sample);
    }

    Ok((left, right, sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_real_wav_envelope() {
        let test_path = "/home/alex/Projects/sonix/imported_stems/A_Box_of_You/0 Lead Vocals.wav";
        if std::path::Path::new(test_path).exists() {
            let res = read_wav_envelope(test_path, 100);
            assert!(res.is_ok());
            let env = res.unwrap();
            assert_eq!(env.len(), 100);
            assert!(env.iter().any(|&v| v > 0.05));
        }
    }
}
