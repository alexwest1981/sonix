//! **Spektral målning.** Ett spektrogram man kan måla dämpning på, och en mask som gör
//! målningen till ljud igen. Samma idé som REAPERs `spectro_filter_paint`: välj ett område i
//! tid och frekvens, dämpa det, hör resultatet — RX-reparation utan att lämna DAW:en.
//!
//! Analysen är STFT med Hann-fönster och 75 % överlapp (fyra gånger), så `istft(stft(x))` är
//! identisk med insignalen. Masken är en förstärkning per (bildruta, bin), och målningen ritar
//! i en **vy** som är nedskalad till högst 1024 × 256 rutor — en timmes ljud får inte bli en
//! textur på 600 MB.

use std::f32::consts::PI;

pub const N_FFT: usize = 1024;
pub const HOP: usize = 256;
pub const BINS: usize = N_FFT / 2 + 1;
/// Hur grov vyn får bli: fler rutor än så här ritas inte, och fler behövs inte för en pensel.
pub const VIEW_MAX_W: usize = 1024;
pub const VIEW_MAX_H: usize = 256;
/// Under den här nivån ritas inget — tystnad är inte intressant att måla på.
pub const FLOOR_DB: f32 = -90.0;

/// Magnitudspektrum i dB per (bildruta, bin), med tiden först.
pub struct Spectrogram {
    pub frames: usize,
    pub bins: usize,
    pub data: Vec<f32>,
    pub seconds: f32,
}

impl Spectrogram {
    /// `left`/`right` är kanalerna; analysen är deras medelvärde — målningen gäller båda.
    pub fn analyze(left: &[f32], right: &[f32], sample_rate: f32) -> Self {
        let n = left.len().max(right.len());
        let frames = if n == 0 { 1 } else { n.div_ceil(HOP) };
        let mut data = vec![FLOOR_DB; frames * BINS];
        let window: Vec<f32> = (0..N_FFT)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / N_FFT as f32).cos())
            .collect();
        let mut planner = rustfft::FftPlanner::new();
        let fft = planner.plan_fft_forward(N_FFT);
        let mut buf = vec![rustfft::num_complex::Complex32::new(0.0, 0.0); N_FFT];

        for f in 0..frames {
            for i in 0..N_FFT {
                let idx = f * HOP + i;
                let l = left.get(idx).copied().unwrap_or(0.0);
                let r = right.get(idx).copied().unwrap_or(0.0);
                buf[i] = rustfft::num_complex::Complex32::new(0.5 * (l + r) * window[i], 0.0);
            }
            fft.process(&mut buf);
            for b in 0..BINS {
                // Samma normalisering som den spektrala kompressorn: en sinus med amplitud A
                // får toppbin A·N/4, alltså dBFS och inte "FFT-enheter".
                let mag = buf[b].norm() / (N_FFT as f32 / 4.0);
                data[f * BINS + b] = (20.0 * (mag + 1.0e-9).log10()).max(FLOOR_DB);
            }
        }
        Self { frames, bins: BINS, data, seconds: n as f32 / sample_rate.max(1.0) }
    }

    pub fn at(&self, frame: usize, bin: usize) -> f32 {
        self.data[frame.min(self.frames - 1) * BINS + bin.min(BINS - 1)]
    }

    /// Vyns storlek: högst `VIEW_MAX_W` × `VIEW_MAX_H` rutor, och aldrig fler än analysen.
    pub fn view_size(&self) -> (usize, usize) {
        (self.frames.min(VIEW_MAX_W).max(1), self.bins.min(VIEW_MAX_H).max(1))
    }

    /// Bilden som ska ritas: `(bredd, höjd, rgb)`, hög frekvens överst. Varje ruta är
    /// maxvärdet över de analysrutor den täcker, så en kort transient syns även när vyn är
    /// mycket grövre än analysen.
    pub fn view_rgb(&self) -> (usize, usize, Vec<u8>) {
        let (w, h) = self.view_size();
        let mut rgb = vec![0u8; w * h * 3];
        for vy in 0..h {
            // Rad 0 är högsta frekvensen: bin-index räknas nedifrån och vänds här.
            let row = h - 1 - vy;
            let bin_hi = ((row + 1) * self.bins).div_ceil(h).min(self.bins);
            let bin_lo = (row * self.bins) / h;
            for vx in 0..w {
                let f_hi = ((vx + 1) * self.frames).div_ceil(w).min(self.frames);
                let f_lo = (vx * self.frames) / w;
                let mut peak = FLOOR_DB;
                for f in f_lo..f_hi.max(f_lo + 1) {
                    for b in bin_lo..bin_hi.max(bin_lo + 1) {
                        peak = peak.max(self.at(f, b));
                    }
                }
                let (r, g, b) = heat(peak);
                let o = (vy * w + vx) * 3;
                rgb[o] = r;
                rgb[o + 1] = g;
                rgb[o + 2] = b;
            }
        }
        (w, h, rgb)
    }
}

/// Mörk botten, orange topp — samma familj som paletten, utan att läsa temafil här.
fn heat(db: f32) -> (u8, u8, u8) {
    let t = ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.5 {
        let u = t * 2.0;
        (14.0 + 20.0 * u, 16.0 + 60.0 * u, 26.0 + 70.0 * u)
    } else {
        let u = (t - 0.5) * 2.0;
        (34.0 + 221.0 * u, 76.0 + 100.0 * u, 96.0 - 40.0 * u)
    };
    (r as u8, g as u8, b as u8)
}

/// Förstärkningen per (bildruta, bin), linjär. 1,0 = orörd.
pub struct Mask {
    pub frames: usize,
    pub bins: usize,
    pub gains: Vec<f32>,
}

impl Mask {
    pub fn new(frames: usize, bins: usize) -> Self {
        Self { frames, bins, gains: vec![1.0; frames * bins] }
    }

    /// Målar ett rektangelområde i **analysens** rutnät till `gain_db` (0 = orörd, −60 =
    /// nästan borta). Området kläms till rutnätet.
    pub fn paint(&mut self, frame_lo: usize, frame_hi: usize, bin_lo: usize, bin_hi: usize, gain_db: f32) {
        let g = 10.0_f32.powf(gain_db.clamp(-60.0, 0.0) / 20.0);
        let f_lo = frame_lo.min(self.frames.saturating_sub(1));
        let f_hi = frame_hi.min(self.frames);
        let b_lo = bin_lo.min(self.bins.saturating_sub(1));
        let b_hi = bin_hi.min(self.bins);
        for f in f_lo..f_hi.max(f_lo + 1) {
            for b in b_lo..b_hi.max(b_lo + 1) {
                self.gains[f * self.bins + b] = g;
            }
        }
    }

    /// Hur många rutor som är målade (gain under 1,0) — statusraden kan säga det i stället
    /// för att gissa.
    pub fn painted_cells(&self) -> usize {
        self.gains.iter().filter(|g| **g < 0.999).count()
    }

    /// Tillämpar masken: STFT, binvis förstärkning, ISTFT med överlapp-addering.
    ///
    /// Identisk mask ger tillbaka insignalen (Hann² vid 75 % överlapp summerar till 1,5).
    pub fn apply(&self, left: &[f32], right: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let n = left.len().max(right.len());
        // **Förspel:** överlapp-adderingen är bara exakt där alla fyra fönstren bidrar. Utan
        // en framruta på N sampel blir de första ~768 samplen för svaga — och det hörs som
        // ett klick i början av varje målning.
        let pad = N_FFT;
        let pad_frames = pad / HOP;
        let frames = (pad + n).div_ceil(HOP).max(1);
        let window: Vec<f32> = (0..N_FFT)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / N_FFT as f32).cos())
            .collect();
        let mut planner = rustfft::FftPlanner::new();
        let fwd = planner.plan_fft_forward(N_FFT);
        let inv = planner.plan_fft_inverse(N_FFT);
        let scale = 1.0 / (N_FFT as f32 * 1.5);

        let mut out_l = vec![0.0f32; frames * HOP + N_FFT];
        let mut out_r = vec![0.0f32; frames * HOP + N_FFT];
        let mut buf = vec![rustfft::num_complex::Complex32::new(0.0, 0.0); N_FFT];

        for f in 0..frames {
            for ch in 0..2 {
                let src = if ch == 0 { left } else { right };
                for i in 0..N_FFT {
                    let idx = f * HOP + i;
                    let x = if idx >= pad { src.get(idx - pad).copied().unwrap_or(0.0) } else { 0.0 };
                    buf[i] = rustfft::num_complex::Complex32::new(x * window[i], 0.0);
                }
                fwd.process(&mut buf);
                for b in 0..BINS {
                    let g = if f >= pad_frames { self.gain_at(f - pad_frames, b) } else { 1.0 };
                    buf[b] *= g;
                    if b > 0 && b < N_FFT / 2 {
                        buf[N_FFT - b] *= g;
                    }
                }
                inv.process(&mut buf);
                let dst = if ch == 0 { &mut out_l } else { &mut out_r };
                for i in 0..N_FFT {
                    dst[f * HOP + i] += buf[i].re * scale * window[i];
                }
            }
        }
        let out_l = out_l[pad..pad + n].to_vec();
        let out_r = out_r[pad..pad + n].to_vec();
        (out_l, out_r)
    }

    fn gain_at(&self, frame: usize, bin: usize) -> f32 {
        let f = frame.min(self.frames.saturating_sub(1));
        let b = bin.min(self.bins.saturating_sub(1));
        self.gains[f * self.bins + b]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, amp: f32, n: usize, sr: f32) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * freq * i as f32 / sr).sin()).collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    /// Spektrogrammet hittar tonen på rätt bin: 1 kHz vid 48 kHz ska ligga kring bin 21.
    #[test]
    fn the_spectrogram_puts_a_tone_in_the_right_bin() {
        let sr = 48_000.0;
        let x = tone(1_000.0, 0.5, 24_000, sr);
        let spec = Spectrogram::analyze(&x, &x, sr);
        let frame = 40;
        let (peak_bin, peak_db) = (0..BINS)
            .map(|b| (b, spec.at(frame, b)))
            .fold((0, -200.0f32), |a, v| if v.1 > a.1 { v } else { a });
        let expected = (1_000.0 * N_FFT as f32 / sr).round() as usize;
        assert!((peak_bin as i32 - expected as i32).abs() <= 1, "bin {peak_bin}, väntat {expected}");
        assert!((peak_db - -6.0).abs() < 1.5, "nivå {peak_db:.1} dB");
    }

    /// Identisk mask får inte färga ljudet — annars är varje målning en försämring.
    #[test]
    fn an_empty_mask_reconstructs_the_signal() {
        let sr = 48_000.0;
        let l = tone(440.0, 0.4, 24_000, sr);
        let r = tone(660.0, 0.3, 24_000, sr);
        let mask = Mask::new(24_000usize.div_ceil(HOP), BINS);
        let (ol, or) = mask.apply(&l, &r);
        assert_eq!(ol.len(), l.len());
        let err = ol.iter().zip(&l).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        let err_r = or.iter().zip(&r).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(
            err < 1.0e-3 && err_r < 1.0e-3,
            "fel {err} / {err_r} (rms in {:.4}/{:.4}, ut {:.4}/{:.4})",
            rms(&l), rms(&r), rms(&ol), rms(&or)
        );
    }

    /// Målad dämpning kring 1 kHz tar bort 1 kHz och lämnar 4 kHz.
    #[test]
    fn painting_a_band_removes_that_band_and_leaves_the_rest() {
        let sr = 48_000.0;
        let low = tone(1_000.0, 0.5, 48_000, sr);
        let high = tone(4_000.0, 0.5, 48_000, sr);
        let mixed: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();

        let spec = Spectrogram::analyze(&mixed, &mixed, sr);
        let mut mask = Mask::new(spec.frames, spec.bins);
        // Bin för 1 kHz ± 3.
        let centre = (1_000.0 * N_FFT as f32 / sr).round() as usize;
        mask.paint(0, spec.frames, centre.saturating_sub(3), centre + 4, -60.0);
        let (out, _) = mask.apply(&mixed, &mixed);

        // Mät i tidsdomänen: nollställ de första 2 000 samplen (fönsterkanten).
        let measure = |x: &[f32]| {
            let w = 8_192;
            let seg = &x[2_000..2_000 + w];
            let goertzel = |f: f32| {
                let (mut re, mut im) = (0.0f32, 0.0f32);
                for (i, v) in seg.iter().enumerate() {
                    let a = 2.0 * PI * f * i as f32 / sr;
                    re += v * a.cos();
                    im += v * a.sin();
                }
                (re * re + im * im).sqrt() / w as f32 * 2.0
            };
            (goertzel(1_000.0), goertzel(4_000.0))
        };
        let (l_in, h_in) = measure(&mixed);
        let (l_out, h_out) = measure(&out);
        let cut_db = 20.0 * (l_out / l_in.max(1e-9)).log10();
        let keep_db = 20.0 * (h_out / h_in.max(1e-9)).log10();
        assert!(cut_db < -30.0, "1 kHz dämpades {cut_db:.1} dB");
        assert!(keep_db.abs() < 1.5, "4 kHz ändrades {keep_db:.1} dB");
    }

    /// Vyn är begränsad även för långt ljud, och ritar hög frekvens överst.
    #[test]
    fn the_view_is_bounded_and_puts_high_frequencies_at_the_top() {
        let sr = 48_000.0;
        let mut x = tone(8_000.0, 0.5, 48_000 * 120, sr); // två minuter
        for v in x.iter_mut().take(1_000) {
            *v = 0.0;
        }
        let spec = Spectrogram::analyze(&x, &x, sr);
        let (w, h) = spec.view_size();
        assert!(w <= VIEW_MAX_W && h <= VIEW_MAX_H);
        let (w, h, rgb) = spec.view_rgb();
        assert_eq!(rgb.len(), w * h * 3);
        // 8 kHz ligger ovanför mitten men under toppen: hitta raden med mest energi och
        // jämför med var bin 170 ska hamna (rad 0 = högst frekvens).
        let row_energy = |vy: usize| -> u32 {
            (0..w).map(|vx| rgb[(vy * w + vx) * 3 + 1] as u32).sum()
        };
        let (peak_row, _) = (0..h).map(|vy| (vy, row_energy(vy))).fold((0, 0), |a, v| if v.1 > a.1 { v } else { a });
        let expected_bin = (8_000.0 * N_FFT as f32 / sr).round() as usize;
        let expected_row = (h - 1).saturating_sub(expected_bin * h / BINS);
        assert!(
            peak_row.abs_diff(expected_row) <= 3,
            "rad {peak_row}, väntat {expected_row} (bin {expected_bin})"
        );
    }

    #[test]
    fn painting_clamps_to_the_grid() {
        let mut mask = Mask::new(4, 8);
        mask.paint(3, 99, 7, 99, -12.0);
        assert!(mask.painted_cells() > 0);
        assert!(mask.gains.iter().all(|g| *g > 0.0));
        assert_eq!(mask.painted_cells(), 1, "bara rutan (3,7)");
    }

    #[test]
    fn silence_is_floor_not_minus_infinity() {
        let sr = 48_000.0;
        let x = vec![0.0f32; 4_096];
        let spec = Spectrogram::analyze(&x, &x, sr);
        assert!(spec.data.iter().all(|v| *v >= FLOOR_DB && v.is_finite()));
        assert!((rms(&x)).abs() < 1e-9);
    }
}
