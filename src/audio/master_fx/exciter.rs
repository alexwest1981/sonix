//! **Emphasis / Emphasizer** — harmonisk exciter.
//!
//! Ett högpassat band (1–10 kHz) körs genom en asymmetrisk mjukklippning. Asymmetrin ger
//! **både** jämna och udda övertoner; en symmetrisk `tanh` hade bara gett udda, och då låter
//! det som en dist, inte som en exciter. Det som läggs till är skillnaden mellan den klippta
//! och den rena bandsignalen, så grundtonen i bandet blir kvar och bara övertonerna tillförs.
//!
//! Det tillförda materialet DC-blockeras: biasen i klippningen ger annars en likspänning som
//! stjäl headroom i limitern.

use crate::audio::filter::{FilterParams, StateVariableFilter};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmphasisParams {
    /// Var övertonerna föds, i Hz.
    pub freq_hz: f32,
    /// Hur hårt bandet klipps. 1 = nästan rent, 10 = rikligt med övertoner.
    pub drive: f32,
    /// Hur mycket av övertonerna som blandas in.
    pub mix: f32,
    pub enabled: bool,
}

impl Default for EmphasisParams {
    fn default() -> Self {
        Self {
            freq_hz: 3_000.0,
            drive: 3.0,
            mix: 0.4,
            enabled: false,
        }
    }
}

pub struct Emphasis {
    sample_rate: f32,
    high: [StateVariableFilter; 2],
    dc: [f32; 2],
}

impl Emphasis {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            sample_rate: sr,
            high: [StateVariableFilter::new(sr), StateVariableFilter::new(sr)],
            dc: [0.0; 2],
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &EmphasisParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if !p.enabled || mix <= 0.0 {
            return (l, r);
        }
        let freq = p.freq_hz.clamp(200.0, self.sample_rate * 0.4);
        let drive = p.drive.clamp(1.0, 20.0);
        let bias = 0.3_f32;
        let dc_coeff = (1.0 - std::f32::consts::TAU * 15.0 / self.sample_rate).clamp(0.9, 0.9999);
        let params = FilterParams { cutoff: freq, resonance: 0.707 };
        let mut out = [0.0_f32; 2];
        for (ch, x) in [l, r].into_iter().enumerate() {
            let high = self.high[ch].process_highpass(x, &params);
            let shaped = (high * drive + bias).tanh() - bias.tanh();
            // Ett enpoligt högpass på det tillförda materialet: biasen ger annars DC.
            self.dc[ch] = self.dc[ch] * dc_coeff + shaped * (1.0 - dc_coeff);
            let added = shaped - self.dc[ch] - high;
            out[ch] = x + added * mix;
        }
        (out[0], out[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(secs: f32, freq: f32, sr: f32) -> Vec<f32> {
        let n = (secs * sr) as usize;
        (0..n)
            .map(|i| (std::f32::consts::TAU * freq * i as f32 / sr).sin() * 0.3)
            .collect()
    }

    /// Goertzels magnitud vid en frekvens — samma mätning som exciter-provet behöver.
    fn magnitude(samples: &[f32], sr: f32, hz: f32) -> f32 {
        let (mut re, mut im) = (0.0_f32, 0.0_f32);
        for (i, v) in samples.iter().enumerate() {
            let ph = std::f32::consts::TAU * hz * i as f32 / sr;
            re += v * ph.cos();
            im -= v * ph.sin();
        }
        (re * re + im * im).sqrt() / samples.len() as f32
    }

    fn render(input: &[f32], params: EmphasisParams, sr: f32) -> Vec<f32> {
        let mut fx = Emphasis::new(sr);
        input
            .iter()
            .map(|&x| {
                let (l, _) = fx.process(x, x, &params);
                l
            })
            .collect()
    }

    #[test]
    fn bypassed_emphasis_is_identity() {
        let sr = 48_000.0;
        let input = sine(0.5, 220.0, sr);
        let mut p = EmphasisParams::default();
        p.enabled = false;
        assert_eq!(render(&input, p, sr), input);
    }

    #[test]
    fn emphasis_adds_both_even_and_odd_harmonics() {
        let sr = 48_000.0;
        let input = sine(1.0, 200.0, sr);
        let mut p = EmphasisParams::default();
        p.enabled = true;
        p.mix = 1.0;
        p.drive = 8.0;
        p.freq_hz = 200.0; // bandet ska innehålla grundtonen, annars finns inget att klippa
        let out = render(&input, p, sr);
        let dry = &input[12_000..36_000];
        let wet = &out[12_000..36_000];
        let f0_dry = magnitude(dry, sr, 200.0);
        let h2_dry = magnitude(dry, sr, 400.0);
        let h3_dry = magnitude(dry, sr, 600.0);
        let h2 = magnitude(wet, sr, 400.0);
        let h3 = magnitude(wet, sr, 600.0);
        assert!(
            h2 > h2_dry * 20.0 + 1e-4,
            "andra övertonen ska växa: {h2_dry:.5} → {h2:.5}"
        );
        assert!(
            h3 > h3_dry * 20.0 + 1e-4,
            "tredje övertonen ska växa: {h3_dry:.5} → {h3:.5}"
        );
        // Grundtonen ska finnas kvar — en exciter lägger till, den byter inte ut.
        assert!(
            magnitude(wet, sr, 200.0) > f0_dry * 0.5,
            "grundtonen försvann"
        );
    }

    #[test]
    fn zero_mix_leaves_the_signal_alone() {
        let sr = 48_000.0;
        let input = sine(0.3, 200.0, sr);
        let mut p = EmphasisParams::default();
        p.enabled = true;
        p.mix = 0.0;
        assert_eq!(render(&input, p, sr), input);
    }
}
