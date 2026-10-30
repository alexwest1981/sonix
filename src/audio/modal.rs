//! **Modal resonator — the struck bar/membrane model.** A bank of up to 64 two-pole
//! resonators, `y[n] = a1·y[n-1] + a2·y[n-2] + g·x[n]`, excited by a short raised-cosine mallet
//! impulse: exciters plus resonant bodies, the shape Ableton's Collision and Tension use.
//!
//! Three laws carry the knobs. All three are analytic, so a measurement can be compared against
//! them instead of against a feeling:
//!
//! * partials sit at `f_n = f0 · n · sqrt(1 + B·n²)` — `B` (inharmonicity 0..0.1) stretches a
//!   harmonic series into a bar's or a membrane's;
//! * mode `n` rings for `T60_n = T60_1 / (1 + material·(n-1)/8)` — a hard body damps the top
//!   first;
//! * the mallet is a raised cosine of 20 ms (soft) down to 1 ms (hard), geometric in the knob.
//!
//! ## Level
//!
//! A resonator's envelope is its impulse gain over `sin(w)`: at 220 Hz and 48 kHz that factor
//! is 35, so the gains are normalised by the summed envelope, not by the impulse. One strike at
//! velocity 1 peaks at `STRIKE_LEVEL` in the worst case, and five stacked strikes therefore
//! stay under 2.0.

use std::f32::consts::TAU;

/// Resonators in one bank.
pub const MAX_MODES: usize = 64;
/// Summed envelope of one strike at velocity 1 — the bound on its peak.
const STRIKE_LEVEL: f32 = 0.35;
/// -60 dB in nepers, `ln(1000)`: T60 from a decay rate.
const LN_1000: f32 = 6.907_755;
/// A mode at or above this fraction of the sample rate is left out (nothing above Nyquist).
const NYQUIST_GUARD: f32 = 0.45;

/// The four knobs plus the mode count.
#[derive(Clone, Copy, Debug)]
pub struct ModalParams {
    /// T60 of partial 1, in seconds.
    pub t60: f32,
    /// 0 = harmonic, 0.1 = strongly stretched.
    pub inharmonicity: f32,
    /// 0 = soft body (the whole bank rings), 1 = hard (the top dies first).
    pub material: f32,
    /// 0 = soft mallet (20 ms), 1 = hard (1 ms).
    pub hardness: f32,
    /// Modes in the bank, 1..=`MAX_MODES`.
    pub modes: usize,
}

impl Default for ModalParams {
    fn default() -> Self {
        Self { t60: 1.0, inharmonicity: 0.0, material: 0.5, hardness: 0.5, modes: MAX_MODES }
    }
}

/// Frequency of partial `n` (1-based) for a fundamental and a stiffness.
#[inline]
pub fn partial_freq(f0: f32, inharmonicity: f32, n: usize) -> f32 {
    let n = n as f32;
    f0 * n * (1.0 + inharmonicity * n * n).sqrt()
}

/// T60 of partial `n` (1-based) for a fundamental decay and a material.
#[inline]
pub fn partial_t60(t60: f32, material: f32, n: usize) -> f32 {
    t60 / (1.0 + material * (n as f32 - 1.0) / 8.0)
}

/// Mallet length in seconds: 20 ms soft, 1 ms hard.
#[inline]
pub fn mallet_seconds(hardness: f32) -> f32 {
    0.020 * 0.050f32.powf(hardness.clamp(0.0, 1.0))
}

#[derive(Clone, Copy, Default)]
struct Mode {
    a1: f32,
    a2: f32,
    gain: f32,
    y1: f32,
    y2: f32,
}

/// The bank: one resonator per mode plus the mallet exciter in front of it.
pub struct ModalBank {
    sample_rate: f32,
    modes: [Mode; MAX_MODES],
    count: usize,
    /// Exciter: a raised cosine of `exc_len` samples. `exc_pos >= exc_len` = no strike running.
    exc_len: usize,
    exc_pos: usize,
    exc_amp: f32,
}

impl ModalBank {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            modes: [Mode::default(); MAX_MODES],
            count: 0,
            exc_len: 0,
            exc_pos: 0,
            exc_amp: 0.0,
        }
    }

    /// Recomputes every mode from the note frequency and the knobs. The ringing state is kept:
    /// turning a knob changes the note that is sounding, it does not stop it.
    pub fn tune(&mut self, f0: f32, params: ModalParams) {
        let sr = self.sample_rate;
        let n_modes = params.modes.clamp(1, MAX_MODES);
        self.count = 0;
        // Relative envelope of mode n: the 1/n mallet spectrum over its resonant gain.
        let envelopes: Vec<f32> = (1..=n_modes)
            .map(|n| {
                let f = partial_freq(f0, params.inharmonicity, n);
                if f <= 0.0 || f >= NYQUIST_GUARD * sr {
                    return 0.0;
                }
                let s = (TAU * f / sr).sin().abs().max(1e-6);
                1.0 / (n as f32 * s)
            })
            .collect();
        let norm = envelopes.iter().sum::<f32>().max(1e-9);

        for n in 1..=n_modes {
            let rel = envelopes[n - 1];
            if rel <= 0.0 {
                break;
            }
            let f = partial_freq(f0, params.inharmonicity, n);
            let w = TAU * f / sr;
            let t60 = partial_t60(params.t60, params.material, n).max(1e-3);
            let r = (-LN_1000 / (t60 * sr)).exp();
            let gain = STRIKE_LEVEL * rel / norm * w.sin();
            let mut mode = self.modes[n - 1];
            mode.a1 = 2.0 * r * w.cos();
            mode.a2 = -r * r;
            mode.gain = gain;
            self.modes[n - 1] = mode;
            self.count = n;
        }
    }

    /// Starts a mallet strike. It adds to whatever is already ringing — a struck body does not
    /// stop when it is struck again. The pulse is scaled to unit area, so `STRIKE_LEVEL` is a
    /// bound on the peak for a soft mallet as well as a hard one; without it the 20 ms pulse's
    /// 480-sample area would be 24× the hard one at the fundamental.
    pub fn strike(&mut self, velocity: f32, hardness: f32) {
        self.exc_len = ((mallet_seconds(hardness) * self.sample_rate) as usize).max(2);
        self.exc_pos = 0;
        let area = self.exc_len as f32 / 2.0;
        self.exc_amp = velocity.clamp(0.0, 1.0) / area;
    }

    /// One mono sample.
    #[inline]
    pub fn next(&mut self) -> f32 {
        let x = if self.exc_pos < self.exc_len {
            let t = self.exc_pos as f32 / self.exc_len as f32;
            self.exc_pos += 1;
            0.5 * (1.0 - (TAU * t).cos()) * self.exc_amp
        } else {
            0.0
        };
        let mut out = 0.0;
        for mode in &mut self.modes[..self.count] {
            let y = mode.a1 * mode.y1 + mode.a2 * mode.y2 + mode.gain * x;
            mode.y2 = mode.y1;
            mode.y1 = y;
            out += y;
        }
        out
    }

    pub fn reset(&mut self) {
        for mode in &mut self.modes {
            mode.y1 = 0.0;
            mode.y2 = 0.0;
        }
        self.exc_pos = self.exc_len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::Complex32;
    use rustfft::FftPlanner;

    const SR: f32 = 48_000.0;
    const F0: f32 = 220.0;
    /// 40 ms analysis block: a Hann main lobe 100 Hz wide keeps the neighbouring partial out.
    const BLOCK: usize = 1_920;

    fn render(params: ModalParams, seconds: f32) -> Vec<f32> {
        let mut bank = ModalBank::new(SR);
        bank.tune(F0, params);
        bank.strike(1.0, params.hardness);
        (0..(seconds * SR) as usize).map(|_| bank.next()).collect()
    }

    fn spectrum(x: &[f32]) -> Vec<f32> {
        let mut buf: Vec<Complex32> = x.iter().map(|s| Complex32::new(*s, 0.0)).collect();
        FftPlanner::<f32>::new().plan_fft_forward(x.len()).process(&mut buf);
        buf.iter().map(|c| c.norm()).collect()
    }

    /// Strongest DFT bin inside ±`width` (a fraction) of `center`.
    fn peak_near(x: &[f32], center: f32, width: f32) -> f32 {
        let spec = spectrum(x);
        let bin = SR / x.len() as f32;
        let lo = ((center * (1.0 - width)) / bin).round() as usize;
        let hi = (((center * (1.0 + width)) / bin).round() as usize).min(spec.len() - 1);
        let k = (lo..=hi).max_by(|a, b| spec[*a].total_cmp(&spec[*b])).unwrap();
        k as f32 * bin
    }

    /// Amplitude of `freq` in one Hann-windowed block, normalised by the window sum.
    fn amplitude(block: &[f32], freq: f32) -> f32 {
        let step = -TAU * freq / SR;
        let (mut re, mut im, mut wsum) = (0.0f32, 0.0f32, 0.0f32);
        for (i, s) in block.iter().enumerate() {
            let w = 0.5 - 0.5 * (TAU * i as f32 / block.len() as f32).cos();
            re += s * w * (step * i as f32).cos();
            im += s * w * (step * i as f32).sin();
            wsum += w;
        }
        2.0 * (re * re + im * im).sqrt() / wsum
    }

    /// T60 in seconds from a log-linear fit of `freq`'s amplitude over the blocks in
    /// `from..to` seconds.
    fn t60(x: &[f32], freq: f32, from: f32, to: f32) -> f32 {
        let mut pts: Vec<(f32, f32)> = Vec::new();
        for (i, block) in x.chunks_exact(BLOCK).enumerate() {
            let t = (i * BLOCK) as f32 / SR;
            if t < from || t >= to {
                continue;
            }
            let a = amplitude(block, freq);
            if a > 1e-7 {
                pts.push((t, a.ln()));
            }
        }
        assert!(pts.len() >= 4, "only {} blocks to fit a decay", pts.len());
        let n = pts.len() as f32;
        let mt = pts.iter().map(|p| p.0).sum::<f32>() / n;
        let ma = pts.iter().map(|p| p.1).sum::<f32>() / n;
        let num: f32 = pts.iter().map(|p| (p.0 - mt) * (p.1 - ma)).sum();
        let den: f32 = pts.iter().map(|p| (p.0 - mt).powi(2)).sum();
        LN_1000 / -(num / den)
    }

    /// Share of the energy above 2 kHz in the first 20 ms.
    fn high_share(params: ModalParams) -> f32 {
        let x = render(params, 0.020);
        let spec = spectrum(&x);
        let split = (2_000.0 / (SR / x.len() as f32)).round() as usize;
        let energy = |r: std::ops::Range<usize>| spec[r].iter().map(|m| m * m).sum::<f32>();
        energy(split..spec.len() / 2) / energy(1..spec.len() / 2).max(1e-30)
    }

    /// (a) Pitch: with no stiffness the strike is a harmonic series and partial 1 wins.
    #[test]
    fn a_strike_at_220_rings_at_220() {
        let params = ModalParams { material: 0.0, hardness: 1.0, ..Default::default() };
        let x = render(params, 1.0);
        let spec = spectrum(&x);
        let bin = SR / x.len() as f32;
        let k = (20..(5_000.0 / bin) as usize)
            .max_by(|a, b| spec[*a].total_cmp(&spec[*b]))
            .unwrap();
        let measured = k as f32 * bin;
        println!(
            "(a) strongest partial of a 220 Hz strike: {measured:.1} Hz (DFT bin {bin:.1} Hz, 1 s window)"
        );
        assert!((measured - F0).abs() <= 3.0, "pitch {measured} Hz is not 220 +/- 3 Hz");
    }

    /// (b) Stiffness: partial 8 moves sharp as soon as the inharmonicity is up.
    #[test]
    fn b_inharmonicity_stretches_the_partials() {
        let base =
            ModalParams { t60: 2.0, material: 0.0, hardness: 1.0, ..Default::default() };
        let harmonic = peak_near(&render(base, 1.0), partial_freq(F0, 0.0, 8), 0.08);
        let stiff_params = ModalParams { inharmonicity: 0.05, ..base };
        let stiff = peak_near(&render(stiff_params, 1.0), partial_freq(F0, 0.05, 8), 0.08);
        let shift = (stiff - harmonic) / harmonic;
        println!(
            "(b) partial 8: B=0 measured {harmonic:.1} Hz (law {:.1}), B=0.05 measured {stiff:.1} Hz \
             (law {:.1}), shift {:+.1} %",
            partial_freq(F0, 0.0, 8),
            partial_freq(F0, 0.05, 8),
            100.0 * shift
        );
        assert!(shift > 0.01, "partial 8 only moved {:+.1} % sharp", 100.0 * shift);
    }

    /// (c) Decay: the promised T60 is the measured T60, and a hard material shortens the top.
    #[test]
    fn c_decay_matches_t60_and_material_damps_the_top() {
        let soft = ModalParams { t60: 1.0, material: 0.0, hardness: 1.0, ..Default::default() };
        let measured = t60(&render(soft, 1.0), F0, 0.05, 0.80);
        println!(
            "(c) partial 1 T60: promised {:.2} s, measured {measured:.3} s ({:+.1} %)",
            soft.t60,
            100.0 * (measured / soft.t60 - 1.0)
        );
        assert!(
            (measured - soft.t60).abs() <= 0.2 * soft.t60,
            "T60 {measured} s is outside 1.0 s +/- 20 %"
        );

        let hard = ModalParams { material: 1.0, ..soft };
        let x = render(hard, 0.8);
        let p1 = t60(&x, F0, 0.02, 0.50);
        let p8 = t60(&x, partial_freq(F0, 0.0, 8), 0.02, 0.25);
        println!(
            "(c) material 1.0: partial 1 T60 {p1:.3} s (law {:.3}), partial 8 T60 {p8:.3} s (law {:.3})",
            partial_t60(1.0, 1.0, 1),
            partial_t60(1.0, 1.0, 8)
        );
        assert!(p8 < p1, "material did not damp partial 8 (p1 {p1:.3}, p8 {p8:.3})");
    }

    /// (d) Mallet: a hard mallet puts more of the first 20 ms above 2 kHz.
    #[test]
    fn d_hard_mallet_is_brighter() {
        let base = ModalParams { t60: 1.0, material: 0.0, ..Default::default() };
        let hard = high_share(ModalParams { hardness: 1.0, ..base });
        let soft = high_share(ModalParams { hardness: 0.0, ..base });
        println!(
            "(d) energy above 2 kHz in the first 20 ms: hard mallet {:.3} % ({:.2} ms pulse), \
             soft mallet {:.3} % ({:.2} ms pulse)",
            100.0 * hard,
            1e3 * mallet_seconds(1.0),
            100.0 * soft,
            1e3 * mallet_seconds(0.0)
        );
        assert!(hard > soft, "hard mallet {hard} is not brighter than soft {soft}");
    }

    /// (e) Five strikes on top of each other stay finite and under 2.0.
    #[test]
    fn e_five_strikes_stay_below_two() {
        let params =
            ModalParams { t60: 2.0, inharmonicity: 0.05, material: 1.0, hardness: 1.0, ..Default::default() };
        let mut bank = ModalBank::new(SR);
        bank.tune(F0, params);
        let mut out = Vec::new();
        for _ in 0..5 {
            bank.strike(1.0, params.hardness);
            out.extend((0..4_800).map(|_| bank.next()));
        }
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let finite = out.iter().all(|s| s.is_finite());
        println!("(e) 5 strikes 0.1 s apart at velocity 1: peak {peak:.3}, finite {finite}");
        assert!(finite, "NaN or inf in the modal bank");
        assert!(peak <= 2.0, "peak {peak} is above 2.0");
    }
}
