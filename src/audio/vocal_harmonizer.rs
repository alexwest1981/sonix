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

/// In-place iterative radix-2 Cooley–Tukey FFT. `re`/`im` must have a power-of-two
/// length. When `invert` is true this performs the inverse transform including
/// the `1/N` scaling.
fn fft_radix2(re: &mut [f32], im: &mut [f32], invert: bool) {
    let n = re.len();
    if n <= 1 {
        return;
    }
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let ang = 2.0 * std::f32::consts::PI / len as f32 * if invert { 1.0 } else { -1.0 };
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0usize;
        while i < n {
            let mut cur_r = 1.0_f32;
            let mut cur_i = 0.0_f32;
            for k in 0..len / 2 {
                let u_r = re[i + k];
                let u_i = im[i + k];
                let v_r = re[i + k + len / 2] * cur_r - im[i + k + len / 2] * cur_i;
                let v_i = re[i + k + len / 2] * cur_i + im[i + k + len / 2] * cur_r;
                re[i + k] = u_r + v_r;
                im[i + k] = u_i + v_i;
                re[i + k + len / 2] = u_r - v_r;
                im[i + k + len / 2] = u_i - v_i;
                let n_r = cur_r * wr - cur_i * wi;
                cur_i = cur_r * wi + cur_i * wr;
                cur_r = n_r;
            }
            i += len;
        }
        len <<= 1;
    }
    if invert {
        let inv = 1.0 / n as f32;
        for i in 0..n {
            re[i] *= inv;
            im[i] *= inv;
        }
    }
}

/// Estimates the smooth spectral envelope (formant structure) of a magnitude
/// spectrum by real-cepstral liftering: keep only the low quefrency terms of the
/// log-magnitude, which removes the fine harmonic structure and leaves the
/// vocal-tract envelope.
fn spectral_envelope(mag: &[f32], lifter: usize) -> Vec<f32> {
    let n = mag.len();
    let mut re: Vec<f32> = mag.iter().map(|&m| (m + 1e-9).ln()).collect();
    let mut im = vec![0.0_f32; n];
    fft_radix2(&mut re, &mut im, true);
    for k in lifter..(n - lifter + 1).min(n) {
        re[k] = 0.0;
        im[k] = 0.0;
    }
    fft_radix2(&mut re, &mut im, false);
    re.iter().map(|&v| v.exp()).collect()
}

/// Formant-preserving pitch shift.
///
/// The input is first pitch-shifted with the granular shifter (which also drags
/// the formants along, causing the "chipmunk" effect). An STFT then estimates the
/// cepstral spectral envelope of both the original and the shifted signal and
/// rescales each shifted frame so its envelope matches the original, optionally
/// warped by `formant_semitones` (0 = keep the original formants, +12 = an octave
/// up). Only the magnitude is corrected; the shifted phase is kept.
pub fn formant_preserving_shift(
    input: &[f32],
    sample_rate: f32,
    pitch_semitones: f32,
    formant_semitones: f32,
) -> Vec<f32> {
    let n = input.len();
    if n == 0 {
        return Vec::new();
    }
    let shifted = pitch_shift(input, sample_rate, pitch_semitones);
    if formant_semitones.abs() > 24.0 {
        return shifted;
    }

    const FRAME: usize = 1024;
    const HOP: usize = 256;
    const LIFTER: usize = 32;
    let two_pi = std::f32::consts::TAU;
    let window: Vec<f32> = (0..FRAME)
        .map(|i| 0.5 - 0.5 * (two_pi * i as f32 / (FRAME - 1) as f32).cos())
        .collect();

    let mut out = vec![0.0_f32; n + FRAME];
    let mut wsum = vec![0.0_f32; n + FRAME];
    let beta = 2f32.powf(formant_semitones / 12.0);

    let mut pos = 0usize;
    while pos < n {
        let mut orig_re = vec![0.0_f32; FRAME];
        let mut orig_im = vec![0.0_f32; FRAME];
        let mut shift_re = vec![0.0_f32; FRAME];
        let mut shift_im = vec![0.0_f32; FRAME];
        for i in 0..FRAME {
            let s = pos + i;
            if s < n {
                orig_re[i] = input[s] * window[i];
                shift_re[i] = shifted[s] * window[i];
            }
        }
        fft_radix2(&mut orig_re, &mut orig_im, false);
        fft_radix2(&mut shift_re, &mut shift_im, false);

        let mag_o: Vec<f32> = (0..FRAME)
            .map(|k| (orig_re[k] * orig_re[k] + orig_im[k] * orig_im[k]).sqrt())
            .collect();
        let mag_s: Vec<f32> = (0..FRAME)
            .map(|k| (shift_re[k] * shift_re[k] + shift_im[k] * shift_im[k]).sqrt())
            .collect();
        let env_o = spectral_envelope(&mag_o, LIFTER);
        let env_s = spectral_envelope(&mag_s, LIFTER);

        for k in 0..FRAME {
            let idx = ((k as f32) / beta).round() as isize;
            let idx = idx.rem_euclid(FRAME as isize) as usize;
            let idx = if idx > FRAME / 2 { FRAME - idx } else { idx };
            let target = env_o[idx.min(FRAME - 1)];
            let corr = (target / (env_s[k] + 1e-9)).clamp(0.1, 10.0);
            shift_re[k] *= corr;
            shift_im[k] *= corr;
        }
        fft_radix2(&mut shift_re, &mut shift_im, true);
        for i in 0..FRAME {
            let s = pos + i;
            if s < n + FRAME {
                out[s] += shift_re[i] * window[i];
                wsum[s] += window[i] * window[i];
            }
        }
        pos += HOP;
    }

    for i in 0..n {
        if wsum[i] > 1e-4 {
            out[i] /= wsum[i];
        }
    }
    out.truncate(n);
    out
}

/// WSOLA (Waveform Similarity Overlap-Add) time-stretcher.
///
/// Produces a signal whose duration is scaled by [`Wsola::set_ratio`] while
/// keeping the pitch intact (unlike varispeed resampling). It runs in frames,
/// overlap-adding Hann-windowed grains; before each grain the best-matching
/// analysis offset near the nominal position is found by normalised
/// cross-correlation with the previous grain's tail, which keeps successive
/// grains phase-aligned.
#[derive(Clone)]
pub struct Wsola {
    frame: usize,
    hop: usize,
    search: usize,
    window: Vec<f32>,
    prev_tail: Vec<f32>,
    ola_l: Vec<f32>,
    ola_r: Vec<f32>,
    fifo_l: Vec<f32>,
    fifo_r: Vec<f32>,
    fifo_read: f64,
    ana_pos: f64,
    started: bool,
    finished: bool,
    ratio: f32,
}

#[inline]
fn read_at(src: &[f32], idx: usize, looping: bool) -> f32 {
    if idx < src.len() {
        src[idx]
    } else if looping && !src.is_empty() {
        src[idx % src.len()]
    } else {
        0.0
    }
}

impl Wsola {
    pub fn new(_sample_rate: f32) -> Self {
        let frame = 1024usize;
        // 50 % overlap: overlapping Hann windows sum to unity, and the coarser
        // hop keeps the (otherwise O(n·search)) similarity search affordable.
        let hop = 512usize;
        let two_pi = std::f32::consts::TAU;
        let window: Vec<f32> = (0..frame)
            .map(|i| 0.5 - 0.5 * (two_pi * i as f32 / (frame - 1) as f32).cos())
            .collect();
        Self {
            frame,
            hop,
            search: 128,
            window,
            prev_tail: vec![0.0; frame - hop],
            ola_l: vec![0.0; frame],
            ola_r: vec![0.0; frame],
            fifo_l: Vec::new(),
            fifo_r: Vec::new(),
            fifo_read: 0.0,
            ana_pos: 0.0,
            started: false,
            finished: false,
            ratio: 1.0,
        }
    }

    /// `ratio` is the output/input duration factor (2.0 = twice as long).
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(0.1, 10.0);
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn available(&self) -> f64 {
        self.fifo_l.len() as f64 - self.fifo_read
    }

    fn best_start(&self, src: &[f32], nominal: isize, looping: bool) -> usize {
        let ov = self.frame - self.hop;
        let len = src.len();
        let energy: f32 = self.prev_tail.iter().map(|v| v * v).sum();
        if energy < 1e-9 {
            return nominal.max(0) as usize;
        }
        let score = |cand: isize| -> Option<f32> {
            if cand < 0 {
                return None;
            }
            let cand = cand as usize;
            if !looping && cand + ov > len {
                return None;
            }
            let mut dot = 0.0_f32;
            let mut na = 0.0_f32;
            for i in 0..ov {
                let a = read_at(src, cand + i, looping);
                let b = self.prev_tail[i];
                dot += a * b;
                na += a * a;
            }
            Some(dot / (na.sqrt() + 1e-9))
        };

        // Coarse-to-fine search keeps the cost low on long buffers.
        let search = self.search as isize;
        let coarse = (search / 8).max(1);
        let mut best = nominal.max(0);
        let mut best_score = f32::NEG_INFINITY;
        let mut d = -search;
        while d <= search {
            if let Some(s) = score(nominal + d) {
                if s > best_score {
                    best_score = s;
                    best = nominal + d;
                }
            }
            d += coarse;
        }
        let refine_lo = (best - coarse).max(-search);
        let refine_hi = (best + coarse).min(search);
        let mut d = refine_lo;
        while d <= refine_hi {
            if let Some(s) = score(nominal + d) {
                if s > best_score {
                    best_score = s;
                    best = nominal + d;
                }
            }
            d += 1;
        }
        best.max(0) as usize
    }

    fn gen_frame(&mut self, sl: &[f32], sr: &[f32], looping: bool) {
        let len = sl.len().max(sr.len());
        let nominal = if self.started {
            self.ana_pos.round() as isize
        } else {
            0
        };
        let start = if self.started {
            self.best_start(sl, nominal, looping)
        } else {
            0
        };
        for i in 0..self.frame {
            let w = self.window[i];
            self.ola_l[i] += read_at(sl, start + i, looping) * w;
            self.ola_r[i] += read_at(sr, start + i, looping) * w;
        }
        for i in 0..self.hop {
            self.fifo_l.push(self.ola_l[i]);
            self.fifo_r.push(self.ola_r[i]);
        }
        self.ola_l.copy_within(self.hop..self.frame, 0);
        self.ola_r.copy_within(self.hop..self.frame, 0);
        for i in (self.frame - self.hop)..self.frame {
            self.ola_l[i] = 0.0;
            self.ola_r[i] = 0.0;
        }
        let ov = self.frame - self.hop;
        for i in 0..ov {
            self.prev_tail[i] = read_at(sl, start + self.hop + i, looping);
        }
        self.started = true;
        self.ana_pos += self.hop as f64 / self.ratio.max(0.01) as f64;
        if !looping && self.ana_pos as usize >= len {
            self.finished = true;
        }
    }

    /// Ensures at least `ahead` output samples are buffered.
    pub fn ensure(&mut self, sl: &[f32], sr: &[f32], looping: bool, ahead: usize) {
        if self.fifo_read > 8192.0 {
            let drop = self.fifo_read.floor() as usize;
            self.fifo_l.drain(0..drop);
            self.fifo_r.drain(0..drop);
            self.fifo_read -= drop as f64;
        }
        while !self.finished && self.available() < ahead as f64 {
            self.gen_frame(sl, sr, looping);
        }
    }

    /// Pops one output sample, resampling the buffered WSOLA stream by `step`
    /// (output frames consumed per emitted frame). `step > 1` raises the pitch.
    pub fn next_resampled(
        &mut self,
        sl: &[f32],
        sr: &[f32],
        looping: bool,
        step: f32,
    ) -> (f32, f32) {
        self.ensure(sl, sr, looping, 2);
        if self.available() <= 0.0 {
            return (0.0, 0.0);
        }
        let i0 = self.fifo_read.floor() as usize;
        let frac = (self.fifo_read - i0 as f64) as f32;
        let i0 = i0.min(self.fifo_l.len() - 1);
        let i1 = (i0 + 1).min(self.fifo_l.len() - 1);
        let l = self.fifo_l[i0] + (self.fifo_l[i1] - self.fifo_l[i0]) * frac;
        let r = self.fifo_r[i0] + (self.fifo_r[i1] - self.fifo_r[i0]) * frac;
        self.fifo_read += step.max(0.01) as f64;
        (l, r)
    }
}

/// Offline pitch-preserving time-stretch. `ratio` is the output/input duration
/// factor (2.0 = twice as long). Used for whole-buffer stretching such as the
/// stem SPEED control.
pub fn time_stretch(input: &[f32], sample_rate: f32, ratio: f32) -> Vec<f32> {
    if input.is_empty() || (ratio - 1.0).abs() < 1e-4 {
        return input.to_vec();
    }
    let mut ws = Wsola::new(sample_rate);
    ws.set_ratio(ratio);
    let out_len = (input.len() as f32 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    while out.len() < out_len {
        let (l, _) = ws.next_resampled(input, input, false, 1.0);
        if ws.available() <= 0.0 && ws.is_finished() {
            break;
        }
        out.push(l);
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
            let shifted = formant_preserving_shift(
                pcm,
                sample_rate,
                v.interval_semitones as f32,
                v.formant_shift,
            );
            let scaled: Vec<f32> = shifted.iter().map(|s| (s * v.volume).clamp(-1.0, 1.0)).collect();
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

    /// A synthetic vowel: a harmonic stack at `f0` shaped by a Gaussian formant
    /// centred at `formant_hz`.
    fn vowel(f0: f32, formant_hz: f32, sr: f32, n: usize) -> Vec<f32> {
        let mut out = vec![0.0_f32; n];
        let mut h = 1;
        while (h as f32) * f0 < sr * 0.45 {
            let f = h as f32 * f0;
            let amp = (-((f - formant_hz).powi(2)) / (2.0 * 150.0 * 150.0)).exp();
            for (i, s) in out.iter_mut().enumerate() {
                *s += amp * (std::f32::consts::TAU * f * i as f32 / sr).sin();
            }
            h += 1;
        }
        let peak = out.iter().fold(0.0_f32, |m, &v| m.max(v.abs())).max(1e-6);
        out.iter().map(|&v| v / peak * 0.8).collect()
    }

    /// Averages the magnitude spectrum over frames and returns the dominant
    /// formant frequency via the same cepstral envelope used by the shifter.
    fn dominant_formant(sig: &[f32], sr: f32) -> f32 {
        const FRAME: usize = 1024;
        const HOP: usize = 512;
        let mut avg = vec![0.0_f32; FRAME];
        let mut frames = 0.0_f32;
        let mut pos = 0usize;
        while pos + FRAME <= sig.len() {
            let mut re: Vec<f32> = sig[pos..pos + FRAME].to_vec();
            let mut im = vec![0.0_f32; FRAME];
            fft_radix2(&mut re, &mut im, false);
            for k in 0..FRAME {
                avg[k] += (re[k] * re[k] + im[k] * im[k]).sqrt();
            }
            frames += 1.0;
            pos += HOP;
        }
        if frames == 0.0 {
            return 0.0;
        }
        for v in avg.iter_mut() {
            *v /= frames;
        }
        let env = spectral_envelope(&avg, 32);
        let mut best_k = 1usize;
        let mut best = 0.0_f32;
        for k in 1..FRAME / 2 {
            let f = k as f32 * sr / FRAME as f32;
            if !(150.0..=3500.0).contains(&f) {
                continue;
            }
            if env[k] > best {
                best = env[k];
                best_k = k;
            }
        }
        best_k as f32 * sr / FRAME as f32
    }

    #[test]
    fn formant_preserving_shift_keeps_formant_while_shifting_pitch() {
        let sr = 44100.0;
        let src = vowel(120.0, 700.0, sr, 44100);

        let plain = pitch_shift(&src, sr, 12.0);
        let preserved = formant_preserving_shift(&src, sr, 12.0, 0.0);

        // Pitch is shifted in both cases (fundamental ~240 Hz).
        let f_plain = detect_pitch_hz(&plain[..8192], sr).expect("plain pitched");
        let f_pres = detect_pitch_hz(&preserved[..8192], sr).expect("preserved pitched");
        assert!((f_plain - 240.0).abs() / 240.0 < 0.05, "plain f0 {f_plain}");
        assert!((f_pres - 240.0).abs() / 240.0 < 0.05, "preserved f0 {f_pres}");

        // The plain shift drags the formant up; preservation keeps it near 700 Hz.
        let formant_plain = dominant_formant(&plain, sr);
        let formant_pres = dominant_formant(&preserved, sr);
        assert!(
            formant_plain > 1050.0,
            "plain shift should move the formant up, got {formant_plain}"
        );
        assert!(
            formant_pres < 950.0,
            "preserved shift should keep the formant near 700 Hz, got {formant_pres}"
        );
        assert!(
            preserved.iter().all(|s| s.is_finite()),
            "preserved output must be finite"
        );
    }

    /// Drives the streaming WSOLA state over a whole buffer, as the audition
    /// playback path does, and returns the stretched mono output.
    fn wsola_stretch(src: &[f32], sr: f32, ratio: f32) -> Vec<f32> {
        let mut ws = Wsola::new(sr);
        ws.set_ratio(ratio);
        let out_len = (src.len() as f32 * ratio).round() as usize;
        let mut out = Vec::with_capacity(out_len);
        while out.len() < out_len {
            let (l, _) = ws.next_resampled(src, src, false, 1.0);
            if ws.available() <= 0.0 && ws.is_finished() {
                break;
            }
            out.push(l);
        }
        out
    }

    #[test]
    fn wsola_time_stretch_preserves_pitch_and_scales_duration() {
        let sr = 44100.0;
        let src = sine(220.0, sr, 44100); // 1 second

        let slow = wsola_stretch(&src, sr, 2.0);
        assert!(
            (slow.len() as f32 - 88200.0).abs() < 4000.0,
            "2.0x should roughly double the length, got {}",
            slow.len()
        );
        let f_slow = detect_pitch_hz(&slow[10000..18192], sr).expect("slow pitched");
        assert!(
            (f_slow - 220.0).abs() / 220.0 < 0.06,
            "pitch must stay at 220 Hz when stretching, got {f_slow}"
        );

        let fast = time_stretch(&src, sr, 0.5);
        assert!(
            (fast.len() as f32 - 22050.0).abs() < 4000.0,
            "0.5x should roughly halve the length, got {}",
            fast.len()
        );
        let f_fast = detect_pitch_hz(&fast[4000..12192], sr).expect("fast pitched");
        assert!(
            (f_fast - 220.0).abs() / 220.0 < 0.06,
            "pitch must stay at 220 Hz when compressing, got {f_fast}"
        );
    }
}
