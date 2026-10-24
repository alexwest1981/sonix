use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Det en skriptmotor behöver veta om en sampling utan att läsa själva ljudet: bara
/// chunk-tabellen i huvudet. `frames` är antalet sampelramar (per kanal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub frames: u64,
}

impl WavInfo {
    pub fn seconds(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.frames as f64 / self.sample_rate as f64
    }
}

/// Läser **bara** huvudet på en RIFF/WAVE-fil: `fmt `-blocket och `data`-längden. Hela filen
/// läses aldrig, så en 200 MB-stem kostar några hundra byte.
pub fn wav_info(path: &str) -> Result<WavInfo, String> {
    let mut file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head).map_err(|e| format!("{path}: {e}"))?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err(format!("{path}: inte RIFF/WAVE"));
    }

    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits = 0u16;
    let mut frames = 0u64;
    let mut pos = 12u64;
    loop {
        let mut hdr = [0u8; 8];
        if file.seek(SeekFrom::Start(pos)).is_err() || file.read_exact(&mut hdr).is_err() {
            break;
        }
        let size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as u64;
        let body = pos + 8;
        if &hdr[0..4] == b"fmt " {
            let mut fmt = [0u8; 16];
            if file.read_exact(&mut fmt).is_ok() {
                channels = u16::from_le_bytes([fmt[2], fmt[3]]);
                sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                bits = u16::from_le_bytes([fmt[14], fmt[15]]).max(8);
            }
        } else if &hdr[0..4] == b"data" {
            // 0xFFFFFFFF = storleken står i `ds64` (RF64) — då räknas resten av filen.
            let bytes = if size == u32::MAX as u64 {
                file.metadata().map(|m| m.len().saturating_sub(body)).unwrap_or(0)
            } else {
                size
            };
            let block = channels.max(1) as u64 * (bits / 8).max(1) as u64;
            frames = bytes / block;
            break;
        }
        if size == u32::MAX as u64 {
            break;
        }
        pos = body + size + (size & 1);
    }
    if sample_rate == 0 {
        return Err(format!("{path}: inget fmt-block"));
    }
    Ok(WavInfo { sample_rate, channels, bits, frames })
}

/// Reads a standard 16-bit, 24-bit or 32-bit float RIFF/WAVE file and generates a normalized peak amplitude waveform envelope.
#[allow(unused_variables, unused_assignments, dead_code)]
pub fn read_wav_envelope(path: &str, points_count: usize) -> Result<Vec<f32>, String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {}", crate::i18n::t("Kunde inte öppna fil"), e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| crate::tstatus!("Kunde inte läsa fil: {}", e))?;

    if buffer.len() < 44 {
        return Err(crate::i18n::t("Filen är för kort för att vara en giltig WAV-fil").to_string());
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
            if audio_format == 0xFFFE && chunk_data_start + 26 <= buffer.len() {
                let sub = u16::from_le_bytes([buffer[chunk_data_start + 24], buffer[chunk_data_start + 25]]);
                if sub == 1 || sub == 3 {
                    audio_format = sub;
                }
            }
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
        return Ok(vec![0.0; points_count]);
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
    let mut file = File::open(path).map_err(|e| format!("{}: {}", crate::i18n::t("Kunde inte öppna fil"), e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| crate::tstatus!("Kunde inte läsa fil: {}", e))?;

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
            if audio_format == 0xFFFE && chunk_data_start + 26 <= buffer.len() {
                let sub = u16::from_le_bytes([buffer[chunk_data_start + 24], buffer[chunk_data_start + 25]]);
                if sub == 1 || sub == 3 {
                    audio_format = sub;
                }
            }
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
                (3, 64) => {
                    f64::from_le_bytes([
                        buffer[byte_idx], buffer[byte_idx + 1], buffer[byte_idx + 2], buffer[byte_idx + 3],
                        buffer[byte_idx + 4], buffer[byte_idx + 5], buffer[byte_idx + 6], buffer[byte_idx + 7],
                    ]) as f32
                }
                (1, 32) => {
                    let raw = i32::from_le_bytes([
                        buffer[byte_idx], buffer[byte_idx + 1], buffer[byte_idx + 2], buffer[byte_idx + 3],
                    ]);
                    raw as f32 / 2_147_483_648.0
                }
                (1, 8) => {
                    // 8-bitars WAV är **osignerad** (0–255, mitten på 128).
                    buffer[byte_idx] as f32 / 128.0 - 1.0
                }
                _ => 0.0,
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

/// Loads decoded stereo PCM [-1.0, 1.0] from any supported audio file.
/// WAV is decoded natively; compressed formats (MP3, FLAC, OGG, M4A, AIFF…)
/// are decoded through the external `ffmpeg` binary (f32le, stereo, 44.1 kHz).
pub fn load_audio_pcm(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".wav") || lower.ends_with(".wave") {
        return load_wav_pcm(path);
    }
    load_via_ffmpeg(path)
}

fn load_via_ffmpeg(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-i", path,
            "-f", "f32le",
            "-acodec", "pcm_f32le",
            "-ac", "2",
            "-ar", "44100",
            "-",
        ])
        .output()
        .map_err(|e| crate::tstatus!("ffmpeg kunde inte startas (är det installerat?): {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(crate::tstatus!("ffmpeg-avkodning misslyckades: {}", err.trim()));
    }

    let bytes = output.stdout;
    let frame_bytes = 8usize; // 2 channels * 4 bytes (f32)
    let total_frames = bytes.len() / frame_bytes;
    let mut left = Vec::with_capacity(total_frames);
    let mut right = Vec::with_capacity(total_frames);
    for f in 0..total_frames {
        let base = f * frame_bytes;
        let l = f32::from_le_bytes([bytes[base], bytes[base + 1], bytes[base + 2], bytes[base + 3]]);
        let r = f32::from_le_bytes([bytes[base + 4], bytes[base + 5], bytes[base + 6], bytes[base + 7]]);
        left.push(l);
        right.push(r);
    }
    if left.is_empty() {
        return Err("Avkodningen gav inget ljud".to_string());
    }
    Ok((left, right, 44100))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Huvudet räcker.** 1 sekund stereo vid 48 kHz ska gå att mäta utan att läsa ljudet.
    #[test]
    fn wav_info_reads_the_header_only() {
        let path = std::env::temp_dir().join("sonix_wav_info_probe.wav");
        let path = path.to_string_lossy().to_string();
        let frames = 48_000usize;
        let samples: Vec<f32> = (0..frames * 2).map(|i| (i % 7) as f32 * 0.01).collect();
        crate::audio::wav_writer::write_pcm_f32_to_wav(&path, &samples, 48_000, 2).unwrap();

        let info = wav_info(&path).unwrap();
        assert_eq!(info.sample_rate, 48_000);
        assert_eq!(info.channels, 2);
        assert_eq!(info.frames, frames as u64);
        assert!((info.seconds() - 1.0).abs() < 1e-3, "{}", info.seconds());
        let _ = std::fs::remove_file(&path);

        assert!(wav_info("/finns/inte.wav").is_err());
    }

    #[test]
    fn test_read_real_wav_envelope() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let test_path = format!("{}/imported_stems/A_Box_of_You/0 Lead Vocals.wav", manifest_dir);
        if std::path::Path::new(&test_path).exists() {
            let res = read_wav_envelope(&test_path, 100);
            assert!(res.is_ok());
            let env = res.unwrap();
            assert_eq!(env.len(), 100);
            assert!(env.iter().any(|&v| v > 0.05));
        }
    }

    #[test]
    fn test_load_audio_pcm_delegates_for_wav() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let test_path = format!("{}/imported_stems/A_Box_of_You/0 Lead Vocals.wav", manifest_dir);
        if std::path::Path::new(&test_path).exists() {
            let res = load_audio_pcm(&test_path);
            assert!(res.is_ok());
            let (l, r, sr) = res.unwrap();
            assert!(!l.is_empty());
            assert_eq!(l.len(), r.len());
            assert!(sr > 0);
        }
    }

    #[test]
    fn test_load_audio_pcm_missing_file_errors() {
        let res = load_audio_pcm("/nonexistent/sonix_missing.wav");
        assert!(res.is_err());
    }
    /// Bygger en WAV i minnet: `format` 1 = PCM, 3 = float, 0xFFFE = extensible.
    fn build_wav(format: u16, bits: u16, channels: u16, sr: u32, frames: &[f32]) -> Vec<u8> {
        let bytes_per = (bits / 8) as usize;
        let data_len = frames.len() * channels as usize * bytes_per;
        let mut out = Vec::with_capacity(44 + data_len);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        // Extensible kräver 40 byte fmt med subformat; övriga klarar 16.
        let fmt_len: u32 = if format == 0xFFFE { 40 } else { 16 };
        out.extend_from_slice(&fmt_len.to_le_bytes());
        out.extend_from_slice(&format.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sr.to_le_bytes());
        out.extend_from_slice(&(sr * channels as u32 * bytes_per as u32).to_le_bytes());
        out.extend_from_slice(&(channels * bytes_per as u16).to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        if format == 0xFFFE {
            out.extend_from_slice(&22u16.to_le_bytes()); // cbSize
            out.extend_from_slice(&bits.to_le_bytes()); // valid bits
            out.extend_from_slice(&0u32.to_le_bytes()); // channel mask
            // KSUBSFMT: PCM = 1, Float = 3
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&[0u8; 14]);
        }
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data_len as u32).to_le_bytes());
        for &f in frames {
            for _ch in 0..channels {
            let v = f.clamp(-1.0, 1.0);
            match (format, bits) {
                (3, 32) | (0xFFFE, 32) if format == 3 => out.extend_from_slice(&v.to_le_bytes()),
                (1, 16) => out.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes()),
                (1, 8) => out.push(((v * 127.0) + 128.0) as u8),
                (1, 24) | (0xFFFE, 24) => {
                    let raw = (v * 8_388_607.0) as i32;
                    out.extend_from_slice(&raw.to_le_bytes()[0..3]);
                }
                (1, 32) | (0xFFFE, 32) => {
                    let raw = (v * 2_147_483_647.0) as i32;
                    out.extend_from_slice(&raw.to_le_bytes());
                }
                _ => out.extend_from_slice(&v.to_le_bytes()),
            }
            }
        }
        out
    }

    /// Laddar en WAV från en temporär fil och jämför med den sinus som skrevs.
    fn roundtrip(format: u16, bits: u16, label: &str) -> (f32, f32) {
        let sr = 44_100u32;
        let n = 2_048usize;
        let sine: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin())
            .collect();
        let bytes = build_wav(format, bits, 2, sr, &sine);
        let path = std::env::temp_dir().join(format!("sonix_roundtrip_{label}.wav"));
        std::fs::write(&path, &bytes).expect("tempfilen ska gå att skriva");
        let (l, _, got_sr) = load_wav_pcm(path.to_string_lossy().as_ref()).expect("läsaren ska klara filen");
        assert_eq!(got_sr, sr, "{label}: samplingsfrekvensen ändrades");

        // Topp och korrelation mot sinusen: brus ger låg korrelation, rätt avkodning ger ~1.
        let peak = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let expected_peak = if bits == 8 { 0.5 } else { 0.5 };
        let sum_xy: f32 = l.iter().zip(&sine).map(|(a, b)| a * b).sum();
        let sum_xx: f32 = l.iter().map(|a| a * a).sum::<f32>().sqrt();
        let sum_yy: f32 = sine.iter().map(|b| b * b).sum::<f32>().sqrt();
        let corr = if sum_xx > 0.0 && sum_yy > 0.0 { sum_xy / (sum_xx * sum_yy) } else { 0.0 };
        let _ = expected_peak;
        (peak, corr)
    }

    /// Mot en fil skriven av **ett annat program** (ffmpeg), när en sådan pekas ut.
    ///
    /// En 1 kHz sinus har ~0,045 nollgenomgångar per sampel vid 44,1 kHz. Brus har ~0,5.
    /// Talet skiljer ton från brus utan att kräva att fasen stämmer — det var precis vad
    /// som gick fel när 24- och 32-bitarsfiler avkodades som 8-bitars.
    #[test]
    fn a_wav_from_another_encoder_is_a_tone_not_noise() {
        let Ok(path) = std::env::var("SONIX_WAV") else {
            return;
        };
        let (l, _r, sr) = load_wav_pcm(&path).expect("filen ska gå att läsa");
        assert!(l.len() > 1000, "filen är för kort för mätningen");
        let zc = l
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count() as f32
            / l.len() as f32;
        let expected = 2.0 * 1000.0 / sr as f32;
        println!("{path}: {sr} Hz, nollgenomgångar {zc:.4} (väntat ~{expected:.4})");
        assert!(
            (zc - expected).abs() < expected * 0.25,
            "{path}: nollgenomgångar {zc:.4} mot väntat {expected:.4} — brus avkodat som ton?"
        );
    }

    #[test]
    fn every_wav_variant_decodes_to_the_sine_that_was_written() {
        for (format, bits, label) in [
            (1u16, 16u16, "pcm16"),
            (1, 24, "pcm24"),
            (1, 32, "pcm32"),
            (3, 32, "float32"),
            (0xFFFE, 24, "ext24"),
        ] {
            let (peak, corr) = roundtrip(format, bits, label);
            assert!(
                (peak - 0.5).abs() < 0.02,
                "{label}: toppen blev {peak:.3}, väntade 0,5 — fel avkodning ger brus"
            );
            assert!(
                corr > 0.99,
                "{label}: korrelationen mot sinusen är {corr:.3} — under 0,99 är det brus, inte ton"
            );
        }
    }

}
