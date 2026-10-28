//! Kurva: en överföringsfunktion med åtta punkter — waveshapern från REAPERs `graphdist`,
//! men ritad med rattar i stället för en Canvas. Punkterna är hela gränssnittet: de bor i
//! `Rattar`-rutan som alla andra parametrar, och kurvan är det man hör.

use super::{Dsp, EffectDef, ParamDef};

pub static CURVE: EffectDef = EffectDef {
    id: "curve",
    name: "Kurva",
    description: "Överföringskurva med åtta punkter: mjuk klippning, invertering eller egen färg",
    category: crate::audio::plugin_host::PluginCategory::Distortion,
    params: &[
        ParamDef::knob("Punkt 1", -1.0, 1.0, -1.0),
        ParamDef::knob("Punkt 2", -1.0, 1.0, -1.0 + 2.0 / 7.0),
        ParamDef::knob("Punkt 3", -1.0, 1.0, -1.0 + 4.0 / 7.0),
        ParamDef::knob("Punkt 4", -1.0, 1.0, -1.0 + 6.0 / 7.0),
        ParamDef::knob("Punkt 5", -1.0, 1.0, -1.0 + 8.0 / 7.0),
        ParamDef::knob("Punkt 6", -1.0, 1.0, -1.0 + 10.0 / 7.0),
        ParamDef::knob("Punkt 7", -1.0, 1.0, -1.0 + 12.0 / 7.0),
        ParamDef::knob("Punkt 8", -1.0, 1.0, 1.0),
        ParamDef::knob("Drive dB", 0.0, 24.0, 0.0),
        ParamDef::knob("Utgång dB", -24.0, 12.0, 0.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
    ],
    make: |sr| Box::new(Curve::new(sr)),
};

struct Curve {
    points: [f32; 8],
    drive: f32,
    out: f32,
    mix: f32,
}

impl Curve {
    fn new(_sr: f32) -> Self {
        Self {
            // Exakt linjär: avvik en tusendel och "rak kurva" färgar ljudet.
            points: std::array::from_fn(|i| -1.0 + i as f32 * (2.0 / 7.0)),
            drive: 1.0,
            out: 1.0,
            mix: 1.0,
        }
    }

    /// Linjär interpolering mellan punkterna. `x` kläms till −1..1: kurvan är definierad där,
    /// och en insignal över full skala ska klippas av kurvans ändar, inte av en tabbe utanför.
    #[inline]
    fn shape(&self, x: f32) -> f32 {
        let x = x.clamp(-1.0, 1.0);
        let pos = (x + 1.0) * 3.5;
        let i = pos.floor() as usize;
        if i >= 7 {
            return self.points[7];
        }
        let frac = pos - i as f32;
        self.points[i] + (self.points[i + 1] - self.points[i]) * frac
    }
}

impl Dsp for Curve {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for x in left.iter_mut() {
            let dry = *x;
            *x = dry + (self.shape(dry * self.drive) * self.out - dry) * self.mix;
        }
        for x in right.iter_mut() {
            let dry = *x;
            *x = dry + (self.shape(dry * self.drive) * self.out - dry) * self.mix;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0..=7 => self.points[index] = value as f32,
            8 => self.drive = 10.0_f32.powf(value as f32 / 20.0),
            9 => self.out = 10.0_f32.powf(value as f32 / 20.0),
            10 => self.mix = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn run(dsp: &mut dyn Dsp, input: &[f32]) -> Vec<f32> {
        let mut l = input.to_vec();
        let mut r = input.to_vec();
        dsp.process(&mut l, &mut r);
        l
    }

    /// Den raka kurvan är passthrough — en shaper som färgar redan vid noll är en bugg.
    #[test]
    fn the_straight_curve_is_passthrough() {
        let sr = 48_000.0;
        let input: Vec<f32> = (0..4_800)
            .map(|i| 0.5 * (2.0 * PI * 220.0 * i as f32 / sr).sin())
            .collect();
        let out = run(&mut Curve::new(sr), &input);
        let err = out.iter().zip(&input).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(err < 1.0e-5, "största fel {err}");
    }

    /// En fallande kurva inverterar signalen — punkterna är kurvan, inte en dekoration.
    #[test]
    fn an_inverted_curve_inverts_the_signal() {
        let sr = 48_000.0;
        let input: Vec<f32> = (0..4_800)
            .map(|i| 0.5 * (2.0 * PI * 220.0 * i as f32 / sr).sin())
            .collect();
        let mut c = Curve::new(sr);
        for i in 0..8 {
            let v = 1.0 - i as f64 * (2.0 / 7.0);
            c.set(i, v);
        }
        let out = run(&mut c, &input);
        let err = out.iter().zip(&input).map(|(a, b)| (a + b).abs()).fold(0.0f32, f32::max);
        assert!(err < 1.0e-5, "största fel {err}");
    }

    /// En mättande kurva sänker toppfaktorn: det är vad en shaper gör med en sinus.
    #[test]
    fn a_saturating_curve_lowers_the_crest_factor() {
        let sr = 48_000.0;
        let input: Vec<f32> = (0..24_000)
            .map(|i| 0.8 * (2.0 * PI * 220.0 * i as f32 / sr).sin())
            .collect();
        let crest_in = input.iter().fold(0.0f32, |a, v| a.max(v.abs())) / rms(&input);
        let mut c = Curve::new(sr);
        // Mjuk S-kurva: brantare i mitten, platt mot ändarna.
        for (i, v) in [-0.85, -0.7, -0.45, -0.15, 0.15, 0.45, 0.7, 0.85].iter().enumerate() {
            c.set(i, *v);
        }
        c.set(8, 6.0);
        let out = run(&mut c, &input);
        let crest_out = out.iter().fold(0.0f32, |a, v| a.max(v.abs())) / rms(&out);
        assert!(crest_out < crest_in * 0.95, "toppfaktor {crest_out:.3} mot {crest_in:.3}");
    }
}
