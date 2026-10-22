use super::*;

// ---------------------------------------------------------------------------
// Per-track 3-band equalizer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackEqSettings {
    pub enabled: bool,
    pub low_freq: f32,
    pub low_gain_db: f32,
    pub mid_freq: f32,
    pub mid_gain_db: f32,
    pub mid_q: f32,
    pub high_freq: f32,
    pub high_gain_db: f32,
}

impl Default for TrackEqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            low_freq: 100.0,
            low_gain_db: 0.0,
            mid_freq: 1500.0,
            mid_gain_db: 0.0,
            mid_q: 1.0,
            high_freq: 8000.0,
            high_gain_db: 0.0,
        }
    }
}

impl TrackEqSettings {
    pub fn is_flat(&self) -> bool {
        !self.enabled
            || (self.low_gain_db.abs() < 0.001
                && self.mid_gain_db.abs() < 0.001
                && self.high_gain_db.abs() < 0.001)
    }
}

#[derive(Clone)]
pub struct StereoEq {
    sample_rate: f32,
    coeffs: [BiquadCoeffs; 3],
    left: [BiquadState; 3],
    right: [BiquadState; 3],
    settings: TrackEqSettings,
}

impl StereoEq {
    pub fn new(sample_rate: f32) -> Self {
        let settings = TrackEqSettings::default();
        let mut eq = Self {
            sample_rate,
            coeffs: [BiquadCoeffs::bypass(); 3],
            left: [BiquadState::default(); 3],
            right: [BiquadState::default(); 3],
            settings,
        };
        eq.set_settings(settings);
        eq
    }

    pub fn set_settings(&mut self, s: TrackEqSettings) {
        self.settings = s;
        self.coeffs[0] = low_shelf(self.sample_rate, s.low_freq, s.low_gain_db);
        self.coeffs[1] = peaking(self.sample_rate, s.mid_freq, s.mid_gain_db, s.mid_q);
        self.coeffs[2] = high_shelf(self.sample_rate, s.high_freq, s.high_gain_db);
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.settings.is_flat() {
            return (l, r);
        }
        let mut l = l;
        let mut r = r;
        for i in 0..3 {
            l = self.left[i].process(l, &self.coeffs[i]);
            r = self.right[i].process(r, &self.coeffs[i]);
        }
        (l, r)
    }
}


// ---------------------------------------------------------------------------
// Full master FX chain
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasterEqBand {
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub active: bool,
}

impl MasterEqBand {
    pub const fn flat() -> Self {
        Self { freq: 1000.0, gain_db: 0.0, q: 1.0, active: false }
    }
}
