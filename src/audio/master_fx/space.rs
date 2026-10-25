//! **Luxeverb**: ett modulerat feedback-delay-nät (FDN) med fyra linjer.
//!
//! Skillnaden mot `SimpleReverb` (Schroeder) är att svansen är **modulerad** och att
//! dämpningen sitter i återkopplingen i stället för efteråt: varje varv tappar energin
//! både i diskanten och i tid, vilket är det som gör en stor sal mjuk i stället för
//! metallisk. Hadamard-mixen är normaliserad (×0,5), så nätet är energibevarande och
//! kan inte självsvänga så länge förstärkningen per varv är under 1.

const LINES: usize = 4;

/// Linjelängder i millisekunder, valda så att de inte står i enkla förhållanden till
/// varandra — annars hörs en tonhöjd i svansen. Skalas av SIZE (0,5–2,0).
const BASE_MS: [f32; LINES] = [31.7, 33.1, 29.7, 28.9];

/// LFO-hastigheter i hertz, en per linje. Långsamma: moduleringen ska bredda svansen,
/// inte låta som ett chorus.
const LFO_HZ: [f32; LINES] = [0.13, 0.17, 0.11, 0.19];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LuxeverbParams {
    /// RT60 i sekunder — hur lång tid svansen tar att falla 60 dB.
    pub decay_s: f32,
    /// Skalar linjelängderna: 0,5 = litet rum, 2,0 = stor sal.
    pub size: f32,
    /// 0 = öppen diskant, 1 = mörk svans (dämpning i återkopplingen).
    pub damping: f32,
    /// Djupet på delaymoduleringen, 0–1 (upp till ±2 ms).
    pub modulation: f32,
    pub predelay_ms: f32,
    /// 0 = mono svans, 1 = full bredd.
    pub width: f32,
    pub mix: f32,
    pub enabled: bool,
}

impl Default for LuxeverbParams {
    fn default() -> Self {
        Self {
            decay_s: 2.4,
            size: 1.1,
            damping: 0.35,
            modulation: 0.35,
            predelay_ms: 20.0,
            width: 1.0,
            mix: 0.3,
            enabled: false,
        }
    }
}

pub struct Luxeverb {
    sample_rate: f32,
    lines: [Vec<f32>; LINES],
    write: [usize; LINES],
    damped: [f32; LINES],
    phase: [f32; LINES],
    predelay: Vec<(f32, f32)>,
    pre_write: usize,
}

/// Linjär interpolering bakåt i ringbufferten. `delay` är i sampel och får vara bråkdel:
/// moduleringen flyttar läsningen mindre än ett sampel per bildruta.
#[inline(always)]
fn read_delayed(buf: &[f32], write: usize, delay: f32) -> f32 {
    let len = buf.len() as f32;
    let mut pos = write as f32 - delay;
    while pos < 0.0 {
        pos += len;
    }
    let i = pos as usize % buf.len();
    let frac = pos - pos.floor();
    let j = (i + 1) % buf.len();
    buf[i] * (1.0 - frac) + buf[j] * frac
}

impl Luxeverb {
    pub fn new(sample_rate: f32) -> Self {
        // 2,2× den längsta linjen + moduleringen: SIZE kan inte göra bufferten för kort.
        let line_len = (0.080 * sample_rate) as usize;
        let pre_len = (0.200 * sample_rate) as usize + 1;
        Self {
            sample_rate,
            lines: std::array::from_fn(|_| vec![0.0; line_len]),
            write: [0; LINES],
            damped: [0.0; LINES],
            phase: [0.0; LINES],
            predelay: vec![(0.0, 0.0); pre_len],
            pre_write: 0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &LuxeverbParams) -> (f32, f32) {
        if !p.enabled {
            return (l, r);
        }
        let sr = self.sample_rate;

        // Predelay: torrsignalen skrivs först och läses `d` sampel bakåt, så d = 0 ger
        // dagens sampel och inget hack i början.
        let d = (p.predelay_ms.clamp(0.0, 200.0) * 0.001 * sr) as usize;
        let pre_len = self.predelay.len();
        self.predelay[self.pre_write] = (l, r);
        let read_i = (self.pre_write + pre_len - d.min(pre_len - 1)) % pre_len;
        let (dl, dr) = self.predelay[read_i];
        self.pre_write = (self.pre_write + 1) % pre_len;

        // Dämpningen är en enpolig lågpass i återkopplingen: 1,0 = öppen, 0,02 = mörk.
        let damp = (1.0 - p.damping.clamp(0.0, 1.0)).powi(3) * 0.98 + 0.02;
        let size = p.size.clamp(0.5, 2.0);
        let mod_samples = p.modulation.clamp(0.0, 1.0) * 0.002 * sr;
        let decay = p.decay_s.clamp(0.2, 12.0);

        let input = [dl * 0.5, dr * 0.5, dl * 0.5, dr * 0.5];
        let mut out = [0.0_f32; LINES];
        for i in 0..LINES {
            let base = BASE_MS[i] * size * 0.001 * sr;
            let lfo = (self.phase[i]).sin();
            self.phase[i] += std::f32::consts::TAU * LFO_HZ[i] / sr;
            if self.phase[i] > std::f32::consts::TAU {
                self.phase[i] -= std::f32::consts::TAU;
            }
            let delay = (base + mod_samples * lfo).clamp(8.0, self.lines[i].len() as f32 - 2.0);
            let y = read_delayed(&self.lines[i], self.write[i], delay);
            self.damped[i] += (y - self.damped[i]) * damp;
            // Förstärkningen per varv ur RT60 för just den här linjens längd.
            let g = 10.0_f32.powf(-3.0 * (base / sr) / decay).min(0.999);
            out[i] = self.damped[i] * g;
        }

        // Hadamard 4×4, normaliserad: varje linje får en blandning av alla fyra.
        let fb = [
            (out[0] + out[1] + out[2] + out[3]) * 0.5,
            (out[0] - out[1] + out[2] - out[3]) * 0.5,
            (out[0] + out[1] - out[2] - out[3]) * 0.5,
            (out[0] - out[1] - out[2] + out[3]) * 0.5,
        ];
        for i in 0..LINES {
            self.lines[i][self.write[i]] = input[i] + fb[i];
            self.write[i] = (self.write[i] + 1) % self.lines[i].len();
        }

        // Vänster ur linje 0 och 2, höger ur 1 och 3 — samma nät, olika uttag.
        let mut wet_l = (out[0] + out[2]) * 0.5;
        let mut wet_r = (out[1] + out[3]) * 0.5;
        let width = p.width.clamp(0.0, 1.0);
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5 * width;
        wet_l = mid + side;
        wet_r = mid - side;

        let mix = p.mix.clamp(0.0, 1.0);
        (l * (1.0 - mix) + wet_l * mix, r * (1.0 - mix) + wet_r * mix)
    }
}
