use eframe::egui::Color32;

use super::recorder::detect_pitch_hz;

#[derive(Debug, Clone)]
pub struct PitchBlob {
    pub start_step: usize,
    pub length_steps: usize,
    pub midi_note: u8,
    /// Per-note correction in semitones, edited by dragging the blob.
    pub pitch_offset: f32,
    pub color: Color32,
}

#[derive(Debug, Clone)]
pub struct HarmonyVoice {
    pub name: &'static str,
    pub interval_semitones: i8,
    pub volume: f32,
    pub enabled: bool,
    pub formant_shift: f32, // -12 .. +12
}

#[derive(Debug, Clone)]
pub struct VocalHarmonizer {
    pub autotune_speed: f32, // 0.0 (Natural) .. 1.0 (Hard Robot T-Pain)
    pub target_scale: usize, // 0: Chromatic, 1: Major, 2: Minor, etc.
    pub root_note: i32,      // 0 = C, 9 = A
    pub blobs: Vec<PitchBlob>,
    pub selected_blob: Option<usize>,
    pub blob_steps: usize,
    pub voices: Vec<HarmonyVoice>,
}

impl Default for VocalHarmonizer {
    fn default() -> Self {
        Self {
            autotune_speed: 0.75,
            target_scale: 2, // A Minor
            root_note: 9,
            blobs: Vec::new(),
            selected_blob: None,
            blob_steps: 16,
            voices: vec![
                HarmonyVoice { name: crate::i18n::t("🎤 Lead Sång (Center)"), interval_semitones: 0, volume: 1.0, enabled: true, formant_shift: 0.0 },
                HarmonyVoice { name: crate::i18n::t("✨ Hög Ters (+3st)"), interval_semitones: 3, volume: 0.75, enabled: true, formant_shift: 1.5 },
                HarmonyVoice { name: crate::i18n::t("🎶 Låg Kvint (-5st)"), interval_semitones: -5, volume: 0.70, enabled: true, formant_shift: -2.0 },
                HarmonyVoice { name: crate::i18n::t("🌌 Oktav Högre (+12st)"), interval_semitones: 12, volume: 0.55, enabled: false, formant_shift: 3.0 },
            ],
        }
    }
}

/// Semitone mask (index 0 = C) for the built-in target scales.
pub fn scale_mask(scale_idx: usize) -> [bool; 12] {
    match scale_idx {
        1 => [true, false, true, false, true, true, false, true, false, true, false, true], // Major
        2 => [true, false, true, true, false, true, false, true, true, false, true, false], // Natural minor
        3 => [true, false, true, false, true, false, false, true, false, true, false, false], // Major pentatonic
        4 => [true, false, false, true, false, true, false, true, false, false, true, false], // Minor pentatonic
        5 => [true, false, true, true, false, true, false, true, false, true, false, true], // Dorian
        6 => [true, false, true, false, true, true, false, true, false, true, false, true], // Mixolydian
        7 => [true, false, false, true, false, true, true, true, false, false, true, false], // Blues
        _ => [true; 12], // Chromatic
    }
}

pub fn freq_to_midi(freq: f32) -> f32 {
    69.0 + 12.0 * (freq / 440.0).log2()
}

/// Snaps a fractional MIDI note to the nearest degree of the given scale.
pub fn snap_midi_to_scale(midi: f32, root: i32, mask: &[bool; 12]) -> f32 {
    let center = midi.round() as i32;
    let mut best = midi;
    let mut best_dist = f32::MAX;
    for cand in (center - 2)..=(center + 2) {
        let pc = ((cand - root) % 12 + 12) % 12;
        if mask[pc as usize] {
            let d = (cand as f32 - midi).abs();
            if d < best_dist {
                best_dist = d;
                best = cand as f32;
            }
        }
    }
    best
}

/// Real-time style granular pitch shifter. The read head advances faster or
/// slower than the write head (per output sample position) and the overlapping
/// grains are cross-faded with a Hann window, preserving duration.
pub fn pitch_shift_variable(
    input: &[f32],
    sample_rate: f32,
    ratio_at: &dyn Fn(f32) -> f32,
) -> Vec<f32> {
    let n = input.len();
    if n == 0 {
        return Vec::new();
    }
    let grain = ((sample_rate * 0.06) as usize).max(64);
    let hop_out = (grain / 2).max(1);
    let mut out = vec![0.0_f32; n + grain];
    let mut wsum = vec![0.0_f32; n + grain];
    let two_pi = std::f32::consts::TAU;
    let mut out_pos = 0usize;
    let mut in_pos = 0.0_f32;

    while out_pos + grain <= n {
        let ratio = ratio_at(out_pos as f32).clamp(0.25, 4.0);
        let ip = in_pos as usize;
        let last = ip + (grain as f32 * ratio) as usize + 1;
        if last >= n {
            break;
        }
        for i in 0..grain {
            let w = 0.5 - 0.5 * (two_pi * i as f32 / (grain - 1) as f32).cos();
            let read = ip + (i as f32 * ratio) as usize;
            out[out_pos + i] += input[read] * w;
            wsum[out_pos + i] += w;
        }
        out_pos += hop_out;
        in_pos += hop_out as f32 * ratio;
    }

    for i in 0..n {
        if wsum[i] > 1e-4 {
            out[i] /= wsum[i];
        }
    }
    out.truncate(n);
    out
}

pub fn pitch_shift(input: &[f32], sample_rate: f32, semitones: f32) -> Vec<f32> {
    if semitones.abs() < 0.01 {
        return input.to_vec();
    }
    let ratio = 2f32.powf(semitones / 12.0);
    pitch_shift_variable(input, sample_rate, &move |_| ratio)
}

/// Cheap spectral-tilt approximation of formant shifting: positive values
/// brighten (scale the envelope up), negative values darken.
pub fn apply_formant_tilt(input: &[f32], semitones: f32) -> Vec<f32> {
    if semitones.abs() < 0.01 {
        return input.to_vec();
    }
    let g = 2f32.powf(semitones / 12.0);
    let mut low = 0.0_f32;
    let mut out = Vec::with_capacity(input.len());
    for &x in input {
        low += 0.18 * (x - low);
        let high = x - low;
        out.push((low / g + high * g).clamp(-2.0, 2.0));
    }
    out
}

/// Per-frame scale-snapping pitch correction. `strength` 0..1 blends between
/// the original and the fully corrected pitch (the AUTO-TUNE knob).
pub fn auto_tune(
    input: &[f32],
    sample_rate: f32,
    strength: f32,
    root: i32,
    scale_idx: usize,
) -> Vec<f32> {
    let n = input.len();
    if n == 0 || strength <= 0.0 {
        return input.to_vec();
    }
    let mask = scale_mask(scale_idx);
    let frame = 4096usize;
    let hop = 1024usize;
    let mut ratios: Vec<f32> = Vec::new();
    let mut pos = 0usize;
    while pos + frame <= n {
        let f = detect_pitch_hz(&input[pos..pos + frame], sample_rate);
        let ratio = match f {
            Some(f) => {
                let midi = freq_to_midi(f);
                let target = snap_midi_to_scale(midi, root, &mask);
                let correction = (target - midi) * strength.clamp(0.0, 1.0);
                2f32.powf(correction / 12.0)
            }
            None => 1.0,
        };
        ratios.push(ratio);
        pos += hop;
    }
    if ratios.is_empty() {
        return input.to_vec();
    }
    let ratio_at = |out_pos: f32| -> f32 {
        let idx = (out_pos as usize / hop).min(ratios.len() - 1);
        ratios[idx]
    };
    pitch_shift_variable(input, sample_rate, &ratio_at)
}

/// Analyses a PCM buffer and produces Melodyne-style pitch blobs mapped onto a
/// fixed number of musical steps.
pub fn analyze_blobs(
    input: &[f32],
    sample_rate: f32,
    total_steps: usize,
    root: i32,
    scale_idx: usize,
) -> Vec<PitchBlob> {
    let n = input.len();
    if n < 4096 || total_steps == 0 {
        return Vec::new();
    }
    let mask = scale_mask(scale_idx);
    let frame = 4096usize;
    let hop = 1024usize;
    let mut blobs: Vec<PitchBlob> = Vec::new();
    let mut pos = 0usize;
    while pos + frame <= n {
        let f = detect_pitch_hz(&input[pos..pos + frame], sample_rate);
        let step = ((pos as f32 / n as f32) * total_steps as f32) as usize;
        let step = step.min(total_steps.saturating_sub(1));
        if let Some(f) = f {
            let midi_f = freq_to_midi(f);
            let snapped = snap_midi_to_scale(midi_f, root, &mask);
            let note = snapped.round().clamp(0.0, 127.0) as u8;
            let extend = blobs.last_mut().filter(|b| {
                b.midi_note == note && b.start_step + b.length_steps >= step
            });
            if let Some(b) = extend {
                b.length_steps = (step + 1 - b.start_step).max(b.length_steps);
            } else {
                let color = if note % 2 == 0 {
                    Color32::from_rgb(0, 220, 255)
                } else {
                    Color32::from_rgb(255, 140, 0)
                };
                blobs.push(PitchBlob {
                    start_step: step,
                    length_steps: 1,
                    midi_note: note,
                    pitch_offset: 0.0,
                    color,
                });
            }
        }
        pos += hop;
    }
    blobs
}

impl VocalHarmonizer {
    /// Replaces the demo blobs with a real pitch analysis of the given take.
    pub fn analyze_take(&mut self, pcm: &[f32], sample_rate: f32, total_steps: usize) {
        self.blob_steps = total_steps.max(1);
        self.blobs = analyze_blobs(pcm, sample_rate, self.blob_steps, self.root_note, self.target_scale);
        self.selected_blob = None;
    }

    /// Applies the per-note `pitch_offset` edits to the take. Each output
    /// position is mapped to its blob (musical step) and pitch-shifted by the
    /// corresponding correction, so dragging notes up/down retunes the audio.
    pub fn apply_blob_corrections(&self, pcm: &[f32], sample_rate: f32) -> Vec<f32> {
        if self.blobs.is_empty() || self.blobs.iter().all(|b| b.pitch_offset.abs() < 0.001) {
            return pcm.to_vec();
        }
        let n = pcm.len();
        let steps = self.blob_steps.max(1) as f32;
        let blobs = self.blobs.clone();
        let ratio_at = move |out_pos: f32| -> f32 {
            let step = ((out_pos / n.max(1) as f32) * steps) as usize;
            for b in &blobs {
                if step >= b.start_step && step < b.start_step + b.length_steps {
                    return 2f32.powf(b.pitch_offset / 12.0).clamp(0.25, 4.0);
                }
            }
            1.0
        };
        pitch_shift_variable(pcm, sample_rate, &ratio_at)
    }

    /// Applies scale-snapping pitch correction using the current AUTO-TUNE knob.
    pub fn apply_autotune(&self, pcm: &[f32], sample_rate: f32) -> Vec<f32> {
        auto_tune(pcm, sample_rate, self.autotune_speed, self.root_note, self.target_scale)
    }

    /// Renders one pitch-shifted PCM buffer per enabled harmony voice.
    pub fn render_harmony(&self, pcm: &[f32], sample_rate: f32) -> Vec<(String, Vec<f32>)> {
        let mut out = Vec::new();
        for v in &self.voices {
            if !v.enabled || v.interval_semitones == 0 {
                continue;
            }
            let shifted = pitch_shift(pcm, sample_rate, v.interval_semitones as f32);
            let tilted = apply_formant_tilt(&shifted, v.formant_shift);
            let scaled: Vec<f32> = tilted.iter().map(|s| (s * v.volume).clamp(-1.0, 1.0)).collect();
            out.push((v.name.to_string(), scaled));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| 0.6 * (std::f32::consts::TAU * freq * i as f32 / sr).sin())
            .collect()
    }

    #[test]
    fn pitch_shift_moves_peak() {
        let sr = 44100.0;
        let src = sine(220.0, sr, 44100);
        let up = pitch_shift(&src, sr, 12.0);
        let f = detect_pitch_hz(&up[..8192], sr).expect("pitched");
        assert!((f - 440.0).abs() / 440.0 < 0.03, "got {f}");
    }

    #[test]
    fn autotune_snaps_flat_note() {
        let sr = 44100.0;
        // ~C4 but 30 cents flat -> should snap up toward C4 in chromatic mode.
        let flat = 261.63 * 2f32.powf(-0.3 / 12.0);
        let src = sine(flat, sr, 44100);
        let out = auto_tune(&src, sr, 1.0, 0, 0);
        let f = detect_pitch_hz(&out[..8192], sr).expect("pitched");
        assert!((f - 261.63).abs() / 261.63 < 0.01, "got {f}");
    }

    #[test]
    fn analyze_blobs_finds_notes() {
        let sr = 44100.0;
        let src = sine(440.0, sr, 44100);
        let blobs = analyze_blobs(&src, sr, 16, 0, 0);
        assert!(!blobs.is_empty());
        assert!(blobs.iter().any(|b| b.midi_note == 69));
    }
}
