//! Loudness measurement and normalization (ITU-R BS.1770 / EBU R128).
//!
//! This is a real, self-contained implementation used by the offline exporter
//! to hit a target integrated loudness and a true-peak ceiling:
//!
//! - **K-weighting** (a high-shelf followed by a high-pass biquad) is designed
//!   for any sample rate from the reference coefficients.
//! - **Gated integrated loudness** follows BS.1770: 400 ms blocks with 75 %
//!   overlap, an absolute gate at -70 LUFS and a relative gate at -10 LU.
//! - **True peak** is estimated with 4× oversampling (polyphase windowed-sinc
//!   FIR), so the ceiling reflects inter-sample peaks rather than raw samples.

use std::f32::consts::PI;

/// Polyphase oversampling factor for true-peak estimation.
const OVERSAMPLE: usize = 4;
/// FIR taps used per polyphase branch.
const TAPS_PER_PHASE: usize = 12;
const TAPS: usize = OVERSAMPLE * TAPS_PER_PHASE;

/// A biquad as `[b0, b1, b2, a1, a2]` (a0 normalized to 1).
type Biquad = [f32; 5];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoudnessPreset {
    Off,
    Streaming,
    AppleMusic,
    Broadcast,
    Loud,
}

impl LoudnessPreset {
    pub const ALL: [LoudnessPreset; 5] = [
        LoudnessPreset::Off,
        LoudnessPreset::Streaming,
        LoudnessPreset::AppleMusic,
        LoudnessPreset::Broadcast,
        LoudnessPreset::Loud,
    ];

    pub fn enabled(self) -> bool {
        !matches!(self, LoudnessPreset::Off)
    }

    /// i18n key (resolved with `crate::i18n::t` in the UI).
    pub fn label(self) -> &'static str {
        match self {
            LoudnessPreset::Off => "Av (ingen normalisering)",
            LoudnessPreset::Streaming => "Streaming (−14 LUFS, −1 dBTP)",
            LoudnessPreset::AppleMusic => "Apple Music (−16 LUFS, −1 dBTP)",
            LoudnessPreset::Broadcast => "Broadcast EBU R128 (−23 LUFS, −1 dBTP)",
            LoudnessPreset::Loud => "Klubb/Loud (−9 LUFS, −0.3 dBTP)",
        }
    }

    pub fn target_lufs(self) -> f32 {
        match self {
            LoudnessPreset::Off => 0.0,
            LoudnessPreset::Streaming => -14.0,
            LoudnessPreset::AppleMusic => -16.0,
            LoudnessPreset::Broadcast => -23.0,
            LoudnessPreset::Loud => -9.0,
        }
    }

    pub fn ceiling_dbtp(self) -> f32 {
        match self {
            LoudnessPreset::Loud => -0.3,
            _ => -1.0,
        }
    }
}

fn sinc(x: f32) -> f32 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let px = PI * x;
        px.sin() / px
    }
}

fn high_shelf_coeffs(sample_rate: u32) -> Biquad {
    let g = 3.999_843_9_f32;
    let q = 0.707_175_24_f32;
    let fc = 1681.974_5_f32;
    let k = (PI * fc / sample_rate as f32).tan();
    let vh = 10.0_f32.powf(g / 20.0);
    let vb = vh.powf(0.499_666_77_f32);
    let a0 = 1.0 + k / q + k * k;
    [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ]
}

fn high_pass_coeffs(sample_rate: u32) -> Biquad {
    let q = 0.500_327_04_f32;
    let fc = 38.135_47_f32;
    let k = (PI * fc / sample_rate as f32).tan();
    let a0 = 1.0 + k / q + k * k;
    [
        1.0 / a0,
        -2.0 / a0,
        1.0 / a0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ]
}

fn biquad_inplace(buf: &mut [f32], c: &Biquad) {
    let (b0, b1, b2, a1, a2) = (c[0], c[1], c[2], c[3], c[4]);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for s in buf.iter_mut() {
        let x0 = *s;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *s = y0;
    }
}

fn mean_square(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    let sum: f32 = x.iter().map(|v| v * v).sum();
    sum / x.len() as f32
}

/// Gated integrated loudness in LUFS (ITU-R BS.1770). Returns
/// `f32::NEG_INFINITY` for silence.
pub fn integrated_lufs(samples_lr: &[f32], sample_rate: u32) -> f32 {
    let frames = samples_lr.len() / 2;
    if frames == 0 || sample_rate == 0 {
        return f32::NEG_INFINITY;
    }

    let mut left = vec![0.0f32; frames];
    let mut right = vec![0.0f32; frames];
    for i in 0..frames {
        left[i] = samples_lr[i * 2];
        right[i] = samples_lr[i * 2 + 1];
    }
    let shelf = high_shelf_coeffs(sample_rate);
    let hp = high_pass_coeffs(sample_rate);
    biquad_inplace(&mut left, &shelf);
    biquad_inplace(&mut left, &hp);
    biquad_inplace(&mut right, &shelf);
    biquad_inplace(&mut right, &hp);

    let block = ((sample_rate as f32 * 0.4) as usize).max(1);
    let step = ((sample_rate as f32 * 0.1) as usize).max(1);

    let mut loudness: Vec<f32> = Vec::new();
    let mut means: Vec<(f32, f32)> = Vec::new();

    if frames < block {
        let (ml, mr) = (mean_square(&left), mean_square(&right));
        loudness.push(-0.691 + 10.0 * (ml + mr).max(1e-12).log10());
        means.push((ml, mr));
    } else {
        let mut start = 0;
        while start + block <= frames {
            let ml = mean_square(&left[start..start + block]);
            let mr = mean_square(&right[start..start + block]);
            loudness.push(-0.691 + 10.0 * (ml + mr).max(1e-12).log10());
            means.push((ml, mr));
            start += step;
        }
    }

    let absolute_gate = -70.0f32;
    let (mut sl, mut sr, mut n) = (0.0f32, 0.0f32, 0usize);
    for (i, &l) in loudness.iter().enumerate() {
        if l > absolute_gate {
            sl += means[i].0;
            sr += means[i].1;
            n += 1;
        }
    }
    if n == 0 {
        return f32::NEG_INFINITY;
    }
    let relative_gate = -0.691 + 10.0 * (sl / n as f32 + sr / n as f32).max(1e-12).log10() - 10.0;
    let gate = absolute_gate.max(relative_gate);

    let (mut sl, mut sr, mut n) = (0.0f32, 0.0f32, 0usize);
    for (i, &l) in loudness.iter().enumerate() {
        if l > gate {
            sl += means[i].0;
            sr += means[i].1;
            n += 1;
        }
    }
    if n == 0 {
        return f32::NEG_INFINITY;
    }
    -0.691 + 10.0 * (sl / n as f32 + sr / n as f32).max(1e-12).log10()
}

fn oversample_fir() -> [f32; TAPS] {
    let mut h = [0.0f32; TAPS];
    let m = (TAPS - 1) as f32;
    let two_fc = 0.5 / OVERSAMPLE as f32;
    for n in 0..TAPS {
        let x = n as f32 - m / 2.0;
        let w = 0.42 - 0.5 * (2.0 * PI * n as f32 / m).cos() + 0.08 * (4.0 * PI * n as f32 / m).cos();
        h[n] = 2.0 * two_fc * sinc(2.0 * two_fc * x) * w;
    }
    for p in 0..OVERSAMPLE {
        let sum: f32 = (0..TAPS_PER_PHASE).map(|t| h[p + OVERSAMPLE * t]).sum();
        if sum.abs() > 1e-9 {
            for t in 0..TAPS_PER_PHASE {
                h[p + OVERSAMPLE * t] /= sum;
            }
        }
    }
    h
}

/// Estimated true peak in dBTP using 4× polyphase oversampling.
pub fn true_peak_db(samples_lr: &[f32]) -> f32 {
    let frames = samples_lr.len() / 2;
    if frames == 0 {
        return f32::NEG_INFINITY;
    }
    let h = oversample_fir();
    let mut peak = 0.0f32;
    let mut x = vec![0.0f32; frames];
    for ch in 0..2 {
        for i in 0..frames {
            x[i] = samples_lr[i * 2 + ch];
        }
        for p in 0..OVERSAMPLE {
            for n in 0..frames {
                let mut acc = 0.0f32;
                for t in 0..TAPS_PER_PHASE {
                    if n >= t {
                        acc += h[p + OVERSAMPLE * t] * x[n - t];
                    }
                }
                peak = peak.max(acc.abs());
            }
        }
    }
    if peak <= 1e-12 {
        f32::NEG_INFINITY
    } else {
        20.0 * peak.log10()
    }
}

fn apply_gain(buf: &mut [f32], db: f32) {
    let g = 10.0_f32.powf(db / 20.0);
    for s in buf.iter_mut() {
        *s *= g;
    }
}

/// Normalizes a stereo buffer to `target_lufs`, then lowers the gain further if
/// needed so the true peak stays at or below `ceiling_dbtp`. Returns the
/// measured integrated loudness **before** normalization (LUFS).
pub fn normalize_loudness(
    buf: &mut [f32],
    sample_rate: u32,
    target_lufs: f32,
    ceiling_dbtp: f32,
) -> f32 {
    let measured = integrated_lufs(buf, sample_rate);
    if !measured.is_finite() {
        return measured;
    }
    apply_gain(buf, target_lufs - measured);

    let tp = true_peak_db(buf);
    if tp.is_finite() && tp > ceiling_dbtp {
        apply_gain(buf, ceiling_dbtp - tp);
    }
    measured
}

/// Convenience wrapper applying a preset. No-op for [`LoudnessPreset::Off`].
pub fn normalize_to_preset(buf: &mut [f32], sample_rate: u32, preset: LoudnessPreset) -> Option<f32> {
    if !preset.enabled() {
        return None;
    }
    Some(normalize_loudness(
        buf,
        sample_rate,
        preset.target_lufs(),
        preset.ceiling_dbtp(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, secs: f32, freq: f32, amp: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        let mut v = Vec::with_capacity(n * 2);
        for i in 0..n {
            let s = (2.0 * PI * freq * i as f32 / rate as f32).sin() * amp;
            v.push(s);
            v.push(s);
        }
        v
    }

    #[test]
    fn silence_has_no_loudness() {
        let buf = vec![0.0f32; 48000 * 2];
        assert!(!integrated_lufs(&buf, 48000).is_finite());
        assert!(!true_peak_db(&buf).is_finite());
    }

    #[test]
    fn normalizing_reaches_target_loudness() {
        let mut buf = sine(48000, 3.0, 997.0, 0.1);
        let before = integrated_lufs(&buf, 48000);
        assert!(before.is_finite() && before < -10.0, "measured {}", before);
        let measured = normalize_loudness(&mut buf, 48000, -14.0, -1.0);
        assert!((measured - before).abs() < 0.05, "{} vs {}", measured, before);
        let after = integrated_lufs(&buf, 48000);
        assert!((after - (-14.0)).abs() < 0.2, "after {} LUFS", after);
    }

    #[test]
    fn true_peak_of_full_scale_sine_is_near_zero() {
        let buf = sine(48000, 1.0, 997.0, 1.0);
        let tp = true_peak_db(&buf);
        assert!(tp.abs() < 0.5, "true peak {} dBTP", tp);
    }

    #[test]
    fn ceiling_is_respected() {
        let mut buf = sine(48000, 3.0, 997.0, 0.9);
        normalize_loudness(&mut buf, 48000, -9.0, -1.0);
        let tp = true_peak_db(&buf);
        assert!(tp <= -0.95, "true peak {} dBTP exceeds ceiling", tp);
    }

    #[test]
    fn preset_off_is_a_noop() {
        let mut buf = sine(48000, 0.5, 440.0, 0.3);
        let original = buf.clone();
        assert!(normalize_to_preset(&mut buf, 48000, LoudnessPreset::Off).is_none());
        assert_eq!(buf, original);
    }
}
