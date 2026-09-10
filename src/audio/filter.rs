use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct FilterParams {
    pub cutoff: f32,    // Hz (20.0 .. 20000.0)
    pub resonance: f32, // Q (0.1 .. 10.0)
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff: 8000.0,
            resonance: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StateVariableFilter {
    pub sample_rate: f32,
    ic1eq: f32,
    ic2eq: f32,
}

impl StateVariableFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    #[inline(always)]
    pub fn process_lowpass(&mut self, input: f32, params: &FilterParams) -> f32 {
        let cutoff = params.cutoff.clamp(20.0, self.sample_rate * 0.48);
        let q = params.resonance.clamp(0.1, 10.0);

        let g = (PI * cutoff / self.sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        v2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_of_sine(cutoff: f32, freq: f32) -> f32 {
        let sr = 48000.0;
        let mut f = StateVariableFilter::new(sr);
        let params = FilterParams { cutoff, resonance: 0.707 };
        let warmup = 4800;
        let measure = 4800;
        let mut sum = 0.0f32;
        for i in 0..(warmup + measure) {
            let x = (2.0 * PI * freq * i as f32 / sr).sin();
            let y = f.process_lowpass(x, &params);
            if i >= warmup {
                sum += y * y;
            }
        }
        (sum / measure as f32).sqrt()
    }

    #[test]
    fn lowpass_attenuates_high_frequencies() {
        let low = rms_of_sine(500.0, 100.0);
        let high = rms_of_sine(500.0, 15000.0);
        assert!(low > 0.5, "passband should survive, got {}", low);
        assert!(high < low * 0.1, "stopband {} should be far below {}", high, low);
    }

    #[test]
    fn reset_clears_filter_state() {
        let sr = 48000.0;
        let mut f = StateVariableFilter::new(sr);
        let params = FilterParams { cutoff: 200.0, resonance: 5.0 };
        for i in 0..1000 {
            let x = (2.0 * PI * 100.0 * i as f32 / sr).sin();
            f.process_lowpass(x, &params);
        }
        f.reset();
        let tail = f.process_lowpass(0.0, &params);
        assert!(tail.abs() < 1e-6, "state should be cleared, got {}", tail);
    }
}
