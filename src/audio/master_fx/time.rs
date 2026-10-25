//! **Time Curve** — Gross Beats tidskurva: varvtalet på masterns egen historik.
//!
//! Signalen skrivs in i en ringbuffert och läses tillbaka på en position som rör sig enligt
//! en kurva i stället för i realtid. Det ger stutter, tape stop och dubbla hastigheten — med
//! tonhöjden följande, som på en bandspelare. Det är skillnaden mot Newtime, som sträcker
//! utan att ändra tonhöjd: här är tonhöjdsändringen hela effekten.
//!
//! Kurvan är sexton punkter i −1…1, där 0 = 1× (spela som vanligt), −1 = stillastående och
//! +1 = 2× (två oktaver upp). Punktlistan är `Copy` för att parametrarna går genom
//! kommandorringen till ljudtråden.

/// Antalet punkter i tidskurvan.
pub const TIME_POINTS: usize = 16;

/// Färdiga kurvor. FL:s Gross Beat har ritbara kurvor; här är fyra former som täcker det
/// man faktiskt gör med dem, och en ritbar variant kan komma ovanpå utan att modellen ändras.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeShape {
    /// 1× hela tiden — effekten är avstängd i praktiken.
    Flat,
    /// Fyra stutter: 1× i tre fjärdedelar, stillastående i en.
    Stutter,
    /// Tape stop: bromsar från 1× till stillastående över en takt.
    TapeStop,
    /// Halva farten rakt igenom — en oktav ner, som att bromsa bandet.
    Half,
}

impl TimeShape {
    pub const ALL: [TimeShape; 4] =
        [TimeShape::Flat, TimeShape::Stutter, TimeShape::TapeStop, TimeShape::Half];


    /// Kurvan i −1…1.
    pub fn curve(self) -> [f32; TIME_POINTS] {
        let mut c = [0.0_f32; TIME_POINTS];
        match self {
            TimeShape::Flat => {}
            TimeShape::Stutter => {
                // Var tolfte del stillastående: fyra stutar per takt.
                for (i, v) in c.iter_mut().enumerate() {
                    if i % 4 == 3 {
                        *v = -1.0;
                    }
                }
            }
            TimeShape::TapeStop => {
                for (i, v) in c.iter_mut().enumerate() {
                    *v = -1.0 + i as f32 / (TIME_POINTS - 1) as f32;
                }
            }
            TimeShape::Half => c.fill(-0.5),
        }
        c
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeCurveParams {
    pub curve: [f32; TIME_POINTS],
    /// Hur snabbt kurvan går runt: 1,0 = en takt på fyra sekunder vid 120 BPM.
    pub bpm: f32,
    pub mix: f32,
    pub enabled: bool,
}

impl Default for TimeCurveParams {
    fn default() -> Self {
        Self {
            curve: TimeShape::Flat.curve(),
            bpm: 120.0,
            mix: 1.0,
            enabled: false,
        }
    }
}

/// Varvtalet för ett kurvvärde: −1 = 0 (stilla), 0 = 1×. Positiva värden kläms till 1×.
///
/// **Bara bromsa, inte skynda.** En insert kan inte läsa in i framtiden: för att spela
/// dubbelt så fort måste den ha en buffert att hämta från, och det är Gross Beats egen
/// inspelning, inte masterns historik. Att låtsas om 2× hade gett ett prov som mätte
/// något annat än det lät som.
pub fn rate_for(value: f32) -> f32 {
    (1.0 + value.clamp(-1.0, 0.0)).clamp(0.0, 1.0)
}

pub struct TimeCurve {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    /// Absolut skrivposition i sampel (går aldrig tillbaka).
    t: f32,
    /// Absolut läsposition i sampel. Rör sig långsammare än skrivhuvudet när kurvan bromsar.
    read_pos: f32,
    phase: f32,
    /// Utjämnat varvtal: ett hopp i kurvan ska låta som en bandspelare, inte som ett klick.
    rate_smooth: f32,
}

impl TimeCurve {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        // Två sekunder historik räcker för en takt i långsamt tempo.
        let cap = (sr * 2.0) as usize + 4;
        Self {
            sample_rate: sr,
            buf_l: vec![0.0; cap],
            buf_r: vec![0.0; cap],
            t: 0.0,
            read_pos: 0.0,
            phase: 0.0,
            rate_smooth: 1.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &TimeCurveParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if !p.enabled || mix <= 0.001 {
            return (l, r);
        }
        let cap = self.buf_l.len();
        let idx = (self.t as usize) % cap;
        self.buf_l[idx] = l;
        self.buf_r[idx] = r;

        // Kurvan går ett varv per takt (fyra fjärdedelar).
        let bars_per_sec = p.bpm.max(20.0) / 60.0 / 4.0;
        self.phase += bars_per_sec / self.sample_rate;
        self.phase -= self.phase.floor();

        let x = self.phase * TIME_POINTS as f32;
        let i = (x.floor() as usize).min(TIME_POINTS - 1);
        let frac = x - i as f32;
        let a = p.curve[i];
        let b = p.curve[(i + 1) % TIME_POINTS];
        let rate = rate_for(a + (b - a) * frac);
        // Utjämnat varvtal: ett hopp i kurvan ska låta som en bandspelare, inte som ett klick.
        self.rate_smooth += (rate - self.rate_smooth) * 0.002;

        self.read_pos += self.rate_smooth;
        // Läsningen får aldrig gå förbi skrivhuvudet, och aldrig längre bak än bufferten.
        let newest = self.t - 1.0;
        let oldest = self.t - (cap - 4) as f32;
        if self.read_pos > newest {
            self.read_pos = newest;
        }
        if self.read_pos < oldest {
            self.read_pos = oldest;
        }

        let read = self.read_pos.max(0.0);
        let i0 = (read.floor() as usize) % cap;
        let i1 = (i0 + 1) % cap;
        let f = read - read.floor();
        let wl = self.buf_l[i0] + (self.buf_l[i1] - self.buf_l[i0]) * f;
        let wr = self.buf_r[i0] + (self.buf_r[i1] - self.buf_r[i0]) * f;

        self.t += 1.0;
        (l * (1.0 - mix) + wl * mix, r * (1.0 - mix) + wr * mix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dominant(samples: &[f32], sr: f32, lo: u32, hi: u32) -> f32 {
        let mut best = (0.0_f32, 0.0_f32);
        for hz in lo..=hi {
            let f = hz as f32;
            let (mut re, mut im) = (0.0_f32, 0.0_f32);
            for (i, v) in samples.iter().enumerate() {
                let ph = std::f32::consts::TAU * f * i as f32 / sr;
                re += v * ph.cos();
                im -= v * ph.sin();
            }
            let mag = (re * re + im * im).sqrt();
            if mag > best.1 {
                best = (f, mag);
            }
        }
        best.0
    }

    fn run(shape: TimeShape, freq: f32) -> Vec<f32> {
        let sr = 48_000.0;
        let mut fx = TimeCurve::new(sr);
        let mut p = TimeCurveParams::default();
        p.enabled = true;
        p.mix = 1.0;
        p.curve = shape.curve();
        let n = sr as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr;
            let x = 0.5 * (std::f32::consts::TAU * freq * t).sin();
            let (l, _) = fx.process(x, x, &p);
            out.push(l);
        }
        out
    }

    /// **Halv fart = halv frekvens.** Det är hela mekanismen: läsningen rör sig ett halvt
    /// sampel per sampel, så bandet spelas upp långsammare och tonhöjden följer med ner.
    #[test]
    fn half_speed_plays_the_buffer_at_half_rate() {
        let out = run(TimeShape::Half, 220.0);
        let f = dominant(&out[24_000..40_000], 48_000.0, 80, 160);
        assert!((f - 110.0).abs() < 12.0, "fick {f:.0} Hz, väntade 110");
    }

    /// Rak kurva ska ge tillbaka signalen oförändrad (med ett sampels fördröjning).
    #[test]
    fn a_flat_curve_is_passthrough() {
        let out = run(TimeShape::Flat, 220.0);
        let f = dominant(&out[24_000..40_000], 48_000.0, 180, 260);
        assert!((f - 220.0).abs() < 10.0, "fick {f:.0} Hz, väntade 220");
        let peak = out[24_000..40_000].iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        assert!((peak - 0.5).abs() < 0.05, "nivån ändrades: {peak:.3}");
    }

    /// Stillastående kurva ska hålla kvar det som redan skrivits in — ingen ny läsning.
    #[test]
    fn a_frozen_curve_holds_the_last_moment() {
        let sr = 48_000.0;
        let mut fx = TimeCurve::new(sr);
        let mut p = TimeCurveParams::default();
        p.enabled = true;
        p.mix = 1.0;
        p.curve = [-1.0; TIME_POINTS];
        let mut out = Vec::new();
        for i in 0..(sr as usize) {
            let t = i as f32 / sr;
            let x = 0.5 * (std::f32::consts::TAU * 220.0 * t).sin();
            let (l, _) = fx.process(x, x, &p);
            out.push(l);
        }
        // Efter en sekund ska nivån inte längre variera med insignalen.
        let tail = &out[40_000..];
        let peak = tail.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
        assert!(peak > 0.0, "fryst ljud ska inte vara tyst");
        assert!((peak - rms).abs() < peak * 0.35, "fryst ljud ska vara stilla: peak {peak:.3} rms {rms:.3}");
    }
}
