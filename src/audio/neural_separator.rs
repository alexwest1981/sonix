//! Neural stem separation through an ONNX model (HTDemucs / Demucs family).
//!
//! The actual inference is provided by the optional [`ort`] runtime and is only
//! compiled when the crate is built with `--features neural`. When no model is
//! present — or the feature is disabled — [`crate::audio::stem_separator`] falls
//! back to its genuine (but non-neural) spectral DSP separator, and the UI says
//! so honestly.
//!
//! Model discovery (in order):
//!   1. `$SONIX_DEMUCS_ONNX` — explicit path to an `.onnx` file.
//!   2. `$XDG_CONFIG_HOME/sonix/models/` (default `~/.config/sonix/models/`).
//!   3. `$XDG_DATA_HOME/sonix/models/` (default `~/.local/share/sonix/models/`).
//!
//! Expected model contract (the standard Demucs ONNX export): a single float
//! input shaped `[batch, 2, samples]` and a single float output shaped
//! `[batch, 4, 2, samples]` (or `[batch, 8, samples]`), with sources ordered
//! `drums, bass, other, vocals`.

use std::path::PathBuf;

use super::stem_separator::StemAudio;

/// Sample rate Demucs was trained on.
pub const DEMUCS_SAMPLE_RATE: u32 = 44_100;
/// Default processing segment (HTDemucs uses ~7.8 s).
pub const DEFAULT_SEGMENT_SECONDS: f32 = 7.8;
/// Crossfade overlap between segments.
pub const DEFAULT_OVERLAP_SECONDS: f32 = 1.0;
/// Number of stems produced.
pub const NUM_SOURCES: usize = 4;
/// Stereo.
pub const NUM_CHANNELS: usize = 2;

/// Environment variable that points directly at a model file.
pub const MODEL_ENV: &str = "SONIX_DEMUCS_ONNX";

/// File names accepted inside the model directories.
pub const MODEL_FILE_NAMES: [&str; 3] = ["htdemucs.onnx", "demucs.onnx", "htdemucs_ft.onnx"];

/// Candidate directories searched for a model, most specific first.
pub fn model_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    {
        dirs.push(base.join("sonix").join("models"));
    }
    if let Some(base) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    {
        dirs.push(base.join("sonix").join("models"));
    }
    dirs
}

/// Pure helper: first existing model candidate inside `dirs`.
pub fn find_model_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        for name in MODEL_FILE_NAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolves the model to use, honouring [`MODEL_ENV`] first.
pub fn find_model() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(MODEL_ENV).map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    find_model_in(&model_dirs())
}

/// Human-readable status for the UI.
pub fn model_status() -> String {
    match find_model() {
        Some(path) => format!("{}", path.display()),
        None => model_dirs()
            .first()
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "~/.config/sonix/models".to_string()),
    }
}

/// Whether a neural run is possible right now (feature enabled + model found).
pub fn is_available() -> bool {
    cfg!(feature = "neural") && find_model().is_some()
}

/// Linear-interpolation stereo resampler. Deliberately simple — it exists to
/// feed the model when the source is not already 44.1 kHz, not as an
/// audiophile-grade converter.
pub fn resample_linear(left: &[f32], right: &[f32], from: u32, to: u32) -> (Vec<f32>, Vec<f32>) {
    let n = left.len().min(right.len());
    if n == 0 || from == 0 || to == 0 || from == to {
        return (left[..n].to_vec(), right[..n].to_vec());
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((n as f64) * ratio).round() as usize;
    let mut out_l = Vec::with_capacity(out_len);
    let mut out_r = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = (src.floor() as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        let frac = (src - i0 as f64) as f32;
        out_l.push(left[i0] + (left[i1] - left[i0]) * frac);
        out_r.push(right[i0] + (right[i1] - right[i0]) * frac);
    }
    (out_l, out_r)
}

/// Start offsets for overlapping segments covering `total` samples. The final
/// segment always reaches the end of the signal.
pub fn plan_segments(total: usize, segment: usize, hop: usize) -> Vec<usize> {
    if total == 0 || segment == 0 {
        return Vec::new();
    }
    let hop = hop.max(1);
    let mut starts = Vec::new();
    let mut pos = 0usize;
    loop {
        starts.push(pos);
        if pos + segment >= total {
            break;
        }
        pos += hop;
    }
    starts
}

/// Triangular crossfade window for a segment. The first and last segments keep
/// a flat outer edge so no samples are attenuated at the signal boundaries.
pub fn segment_window(len: usize, overlap: usize, first: bool, last: bool) -> Vec<f32> {
    let mut window = vec![1.0f32; len];
    let overlap = overlap.min(len / 2);
    if overlap == 0 {
        return window;
    }
    for i in 0..overlap {
        let gain = (i + 1) as f32 / (overlap + 1) as f32;
        if !first {
            window[i] = gain;
        }
        if !last {
            window[len - 1 - i] = gain;
        }
    }
    window
}

/// Extract one `(source, channel)` sample slice from a model output tensor
/// shaped `[B, S, C, T]`, `[B, S*C, T]` or `[S*C, T]`.
pub fn source_channel_slice<'a>(
    shape: &[i64],
    data: &'a [f32],
    source: usize,
    channel: usize,
    channels: usize,
) -> Result<&'a [f32], String> {
    if channels == 0 {
        return Err("kanaler får inte vara 0".to_string());
    }
    let (source_channels, frames) = match shape {
        [b, s, c, t] if *b == 0 || *b == 1 => (*s as usize * *c as usize, *t as usize),
        [b, sc, t] if *b == 0 || *b == 1 => (*sc as usize, *t as usize),
        [sc, t] => (*sc as usize, *t as usize),
        _ => return Err(format!("oväntad utdata-form {shape:?}")),
    };
    if source_channels % channels != 0 {
        return Err(format!(
            "utdata-form {shape:?} stämmer inte med {channels} kanaler"
        ));
    }
    let index = source * channels + channel;
    if index >= source_channels {
        return Err("källindex utanför modellens utdata".to_string());
    }
    let start = index * frames;
    let end = start + frames;
    if end > data.len() {
        return Err("modellens utdata är kortare än formen anger".to_string());
    }
    Ok(&data[start..end])
}

/// Map a Demucs source index to our stem index
/// (`0` Vocals, `1` Drums, `2` Bass, `3` Instruments).
pub fn demucs_source_to_stem(source: usize) -> usize {
    match source {
        0 => 1, // drums
        1 => 2, // bass
        2 => 3, // other -> instruments
        3 => 0, // vocals
        _ => 3,
    }
}

/// Runs the neural separator. Returns `Err` with a human-readable reason when
/// no model or backend is available; the caller falls back to DSP.
pub fn separate_neural(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    progress: &dyn Fn(f32),
) -> Result<Vec<StemAudio>, String> {
    #[cfg(feature = "neural")]
    {
        backend::separate(left, right, sample_rate, progress)
    }
    #[cfg(not(feature = "neural"))]
    {
        let _ = (left, right, sample_rate, progress);
        Err(crate::i18n::t(
            "Neural separation är inte inbyggd i den här builden (bygg med --features neural)",
        )
        .to_string())
    }
}

#[cfg(feature = "neural")]
mod backend {
    use super::*;
    use ort::session::Session;
    use ort::value::Tensor;

    /// Runs a Demucs-style ONNX model over a stereo pair with overlap-add.
    pub fn separate(
        left: &[f32],
        right: &[f32],
        sample_rate: u32,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<StemAudio>, String> {
        let path = find_model().ok_or_else(|| {
            crate::tstatus!(
                "Ingen neural modell hittades. Lägg en HTDemucs-ONNX i {} (eller sätt {})",
                model_status(),
                MODEL_ENV
            )
        })?;

        let (l, r) = resample_linear(left, right, sample_rate, DEMUCS_SAMPLE_RATE);
        let n = l.len().min(r.len());
        if n == 0 {
            return Err("Ljudet är tomt".to_string());
        }

        let mut session = Session::builder()
            .map_err(|e| format!("Kunde inte skapa ONNX-session: {e}"))?
            .commit_from_file(&path)
            .map_err(|e| format!("Kunde inte ladda modellen {}: {e}", path.display()))?;

        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .ok_or_else(|| "Modellen har inga indata".to_string())?;
        let output_name = session
            .outputs()
            .first()
            .map(|o| o.name().to_string())
            .ok_or_else(|| "Modellen har inga utdata".to_string())?;

        // Global normalisation, matching Demucs' `apply_model`.
        let mut sum = 0.0f64;
        let mut sumsq = 0.0f64;
        for i in 0..n {
            sum += l[i] as f64 + r[i] as f64;
            sumsq += (l[i] as f64 * l[i] as f64) + (r[i] as f64 * r[i] as f64);
        }
        let count = (n * 2) as f64;
        let mean = (sum / count) as f32;
        let var = (sumsq / count) as f32 - mean * mean;
        let std = var.max(1e-8).sqrt();

        let segment = (DEMUCS_SAMPLE_RATE as f32 * DEFAULT_SEGMENT_SECONDS) as usize;
        let overlap = (DEMUCS_SAMPLE_RATE as f32 * DEFAULT_OVERLAP_SECONDS) as usize;
        let hop = segment.saturating_sub(overlap).max(1);
        let starts = plan_segments(n, segment, hop);
        let total = starts.len().max(1);

        let mut acc: Vec<[Vec<f32>; NUM_CHANNELS]> = (0..NUM_SOURCES)
            .map(|_| [vec![0.0f32; n], vec![0.0f32; n]])
            .collect();
        let mut weight = vec![0.0f32; n];

        for (segment_index, &start) in starts.iter().enumerate() {
            let end = (start + segment).min(n);
            let seg_len = end - start;

            // Always feed a full-length segment: HTDemucs has fixed positional
            // embeddings, so the final short chunk is zero-padded.
            let mut input = vec![0.0f32; NUM_CHANNELS * segment];
            for i in 0..seg_len {
                input[i] = (l[start + i] - mean) / std;
                input[segment + i] = (r[start + i] - mean) / std;
            }
            let tensor = Tensor::from_array((vec![1i64, 2, segment as i64], input))
                .map_err(|e| format!("Kunde inte skapa indata-tensor: {e}"))?;
            let outputs = session
                .run(ort::inputs![input_name.as_str() => tensor])
                .map_err(|e| format!("ONNX-körning misslyckades: {e}"))?;
            let value = outputs
                .get(output_name.as_str())
                .ok_or_else(|| format!("Modellens utdata '{output_name}' saknas"))?;
            let (shape, data) = value
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Kunde inte läsa utdata: {e}"))?;
            let shape: Vec<i64> = shape.iter().copied().collect();

            let first = segment_index == 0;
            let last = segment_index + 1 == total;
            let window = segment_window(seg_len, overlap.min(seg_len / 2), first, last);

            for source in 0..NUM_SOURCES {
                let stem_index = demucs_source_to_stem(source);
                for channel in 0..NUM_CHANNELS {
                    let slice = source_channel_slice(&shape, data, source, channel, NUM_CHANNELS)?;
                    let usable = slice.len().min(seg_len);
                    for i in 0..usable {
                        let sample = slice[i] * std + mean;
                        acc[stem_index][channel][start + i] += sample * window[i];
                    }
                }
            }
            for i in 0..seg_len {
                weight[start + i] += window[i];
            }
            progress((segment_index + 1) as f32 / total as f32);
        }

        let mut stems = vec![StemAudio::default(); NUM_SOURCES];
        for stem_index in 0..NUM_SOURCES {
            let mut out_l = vec![0.0f32; n];
            let mut out_r = vec![0.0f32; n];
            for i in 0..n {
                let w = if weight[i] > 1e-6 { weight[i] } else { 1.0 };
                out_l[i] = (acc[stem_index][0][i] / w).clamp(-1.0, 1.0);
                out_r[i] = (acc[stem_index][1][i] / w).clamp(-1.0, 1.0);
            }
            stems[stem_index] = StemAudio {
                left: out_l,
                right: out_r,
            };
        }
        Ok(stems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demucs_source_order_maps_to_sonix_stems() {
        // drums, bass, other, vocals -> Vocals, Drums, Bass, Instruments
        assert_eq!(demucs_source_to_stem(0), 1);
        assert_eq!(demucs_source_to_stem(1), 2);
        assert_eq!(demucs_source_to_stem(2), 3);
        assert_eq!(demucs_source_to_stem(3), 0);
    }

    #[test]
    fn resampling_upsamples_and_preserves_dc() {
        let left = vec![0.5f32; 100];
        let right = vec![-0.5f32; 100];
        let (l, r) = resample_linear(&left, &right, 1000, 2000);
        assert_eq!(l.len(), 200);
        assert_eq!(r.len(), 200);
        assert!(l.iter().all(|v| (*v - 0.5).abs() < 1e-6));
        assert!(r.iter().all(|v| (*v + 0.5).abs() < 1e-6));
    }

    #[test]
    fn resampling_downsample_length_and_identity() {
        let left: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let (l, r) = resample_linear(&left, &left, 48000, 44100);
        let expected = (1000.0_f64 * 44100.0 / 48000.0).round() as usize;
        assert_eq!(l.len(), expected);
        assert_eq!(r.len(), expected);
        // Same rate is an exact copy.
        let (same, _) = resample_linear(&left, &left, 44100, 44100);
        assert_eq!(same, left);
    }

    #[test]
    fn segments_cover_the_whole_signal() {
        let starts = plan_segments(10_000, 4_000, 3_000);
        assert_eq!(starts, vec![0, 3_000, 6_000]);
        assert_eq!(*starts.last().unwrap() + 4_000, 10_000);
        assert!(plan_segments(0, 4_000, 3_000).is_empty());
        // Shorter than one segment -> a single full-length start.
        assert_eq!(plan_segments(500, 4_000, 3_000), vec![0]);
    }

    #[test]
    fn segment_windows_do_not_attenuate_boundaries() {
        let first = segment_window(100, 20, true, false);
        let last = segment_window(100, 20, false, true);
        let middle = segment_window(100, 20, false, false);
        assert_eq!(first[0], 1.0);
        assert_eq!(last[99], 1.0);
        assert_eq!(middle[0], middle[99]);
        assert!(middle[0] < 1.0 && middle[0] > 0.0);
        // Crossfading two overlapping middle windows sums to 1 in the overlap.
        let a = segment_window(100, 40, false, false);
        let b = segment_window(100, 40, false, false);
        for j in 0..40 {
            assert!((a[100 - 40 + j] + b[j] - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn extracts_sources_from_batched_and_flat_layouts() {
        // [1, 4, 2, 4]: source 0 channel 1 should be samples 4..8.
        let data: Vec<f32> = (0..32).map(|i| i as f32).collect();
        let s = source_channel_slice(&[1, 4, 2, 4], &data, 0, 1, 2).unwrap();
        assert_eq!(s, &[4.0, 5.0, 6.0, 7.0]);
        // [1, 8, 4] (S*C flattened): source 3 channel 0 -> index 6.
        let s = source_channel_slice(&[1, 8, 4], &data, 3, 0, 2).unwrap();
        assert_eq!(s, &[24.0, 25.0, 26.0, 27.0]);
        // Unbatched [8, 4].
        let s = source_channel_slice(&[8, 4], &data, 1, 0, 2).unwrap();
        assert_eq!(s, &[8.0, 9.0, 10.0, 11.0]);
    }

    #[test]
    fn rejects_bad_shapes_and_short_buffers() {
        let data = vec![0.0f32; 8];
        assert!(source_channel_slice(&[1, 8, 4], &data, 3, 1, 2).is_err());
        assert!(source_channel_slice(&[1, 4, 2, 4], &data, 1, 0, 2).is_err());
        assert!(source_channel_slice(&[7], &data, 0, 0, 2).is_err());
    }

    #[test]
    fn finds_model_in_priority_order() {
        let base = std::env::temp_dir().join("sonix_model_discovery_test");
        let a = base.join("a");
        let b = base.join("b");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        assert!(find_model_in(&[a.clone(), b.clone()]).is_none());
        std::fs::write(b.join("demucs.onnx"), b"x").unwrap();
        assert_eq!(
            find_model_in(&[a.clone(), b.clone()]),
            Some(b.join("demucs.onnx"))
        );
        std::fs::write(a.join("htdemucs.onnx"), b"x").unwrap();
        assert_eq!(
            find_model_in(&[a.clone(), b.clone()]),
            Some(a.join("htdemucs.onnx"))
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn neural_requires_a_model_to_be_available() {
        // Without a model on disk this must be false regardless of the feature.
        if find_model().is_none() {
            assert!(!is_available());
        }
    }
}
