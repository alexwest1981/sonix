//! Biquad och Butterworth-kaskad — byggstenarna som gör 24/48/72 dB per oktav möjliga.
//!
//! Koefficienterna är RBJ:s kokbok, normaliserade med `a0` en gång vid inställning så
//! processloopen är fem multiplikationer. Ingen allokering efter `set`.

use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    LowPass,
    HighPass,
    LowShelf,
    HighShelf,
    AllPass,
}

#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }
}

impl Biquad {
    pub fn set(&mut self, kind: Kind, sr: f32, freq: f32, gain_db: f32, q: f32) {
        let sr = sr.max(1.0);
        let f = freq.clamp(10.0, sr * 0.49);
        let w0 = 2.0 * PI * f / sr;
        let (sin, cos) = w0.sin_cos();
        let q = q.max(0.05);
        let alpha = sin / (2.0 * q);
        let a = 10.0_f32.powf(gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match kind {
            Kind::LowPass => ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            Kind::HighPass => ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            Kind::AllPass => (1.0 - alpha, -2.0 * cos, 1.0 + alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha),
            Kind::LowShelf => {
                let sqrt_a = a.sqrt();
                let beta = 2.0 * sqrt_a * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + beta),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - beta),
                    (a + 1.0) + (a - 1.0) * cos + beta,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - beta,
                )
            }
            Kind::HighShelf => {
                let sqrt_a = a.sqrt();
                let beta = 2.0 * sqrt_a * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + beta),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - beta),
                    (a + 1.0) - (a - 1.0) * cos + beta,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - beta,
                )
            }
        };

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Butterworth-kaskad: `order` måste vara jämn. Q-värdena räknas ur ordningen, så 4/8/12
/// poler (24/48/72 dB per oktav) blir samma kod.
#[derive(Clone, Copy)]
pub struct Butterworth {
    stages: [Biquad; 6],
    used: usize,
}

impl Default for Butterworth {
    fn default() -> Self {
        Self { stages: [Biquad::default(); 6], used: 0 }
    }
}

impl Butterworth {
    pub fn set(&mut self, kind: Kind, sr: f32, freq: f32, order: usize) {
        let order = order.clamp(2, 12) & !1;
        self.used = order / 2;
        for k in 0..self.used {
            let q = 1.0 / (2.0 * (PI * (2 * k + 1) as f32 / (2 * order) as f32).sin());
            self.stages[k].set(kind, sr, freq, 0.0, q);
        }
    }

    #[inline]
    pub fn process(&mut self, mut x: f32) -> f32 {
        for i in 0..self.used {
            x = self.stages[i].process(x);
        }
        x
    }

    pub fn reset(&mut self) {
        for s in &mut self.stages {
            s.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain_at(mut b: Biquad, sr: f32, freq: f32) -> f32 {
        let n = 48_000;
        let mut peak = 0.0f32;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f32 / sr).sin();
            let y = b.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn a_lowpass_passes_below_and_stops_above() {
        let sr = 48_000.0;
        let mut lp = Biquad::default();
        lp.set(Kind::LowPass, sr, 1000.0, 0.0, 0.7071);
        assert!(gain_at(lp, sr, 100.0) > 0.95);
        assert!(gain_at(lp, sr, 8000.0) < 0.05);
    }

    /// 24 dB/okt är fyra poler: en oktav över gränsfrekvensen ska ge ~1/16, inte ~1/4.
    #[test]
    fn the_cascade_has_the_slope_its_order_promises() {
        let sr = 48_000.0;
        for (order, db_per_oct) in [(2, 12.0), (4, 24.0), (8, 48.0), (12, 72.0)] {
            let mut bw = Butterworth::default();
            bw.set(Kind::LowPass, sr, 1000.0, order);
            let a = 20.0 * gain_at_butter(&mut bw, sr, 2000.0).log10();
            assert!(
                (a + db_per_oct).abs() < db_per_oct * 0.15,
                "ordning {order}: {a:.1} dB, förväntat -{db_per_oct}"
            );
        }
    }

    fn gain_at_butter(b: &mut Butterworth, sr: f32, freq: f32) -> f32 {
        let n = 48_000;
        let mut peak = 0.0f32;
        for i in 0..n {
            let x = (2.0 * PI * freq * i as f32 / sr).sin();
            let y = b.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }
}
