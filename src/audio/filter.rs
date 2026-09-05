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
