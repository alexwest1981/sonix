//! **Hybrid Reverb** — a diffusion network and a convolution engine in parallel.
//!
//! Ableton's Hybrid Reverb is one convolution half plus one algorithmic half
//! (`OHybridProcessor` in `docs/ABLETON-BINARY-MAP.md`, described in their manual as a
//! diffusion network with size and decay). We keep the split, not the sound: the
//! algorithmic half is four allpass diffusers feeding an eight-line feedback delay
//! network with a Hadamard mix and one-pole damping in the loop, the convolution half is
//! the existing [`Convolver`] fed a synthetic decaying-noise impulse response. No IR
//! files, no new dependency.
//!
//! `mix` is the dry/wet control and `blend` crossfades the algorithmic half against the
//! convolution half. That is one knob more than "a mix control", but without it a dry
//! `mix = 0` and a "the convolution is gone" `mix = 1` could not both hold.

use std::sync::Arc;

use super::*;

/// Lines in the diffusion network.
const LINES: usize = 8;

/// Base line lengths in milliseconds, chosen prime-ish so the modes do not line up.
const BASE_MS: [f32; LINES] = [7.1, 8.3, 9.7, 11.3, 13.1, 15.1, 17.3, 19.9];

/// Schroeder allpass lengths in milliseconds, in front of the network. The FDN alone has
/// discrete echoes; these fill the space between them, which is what `diffusion_is_dense`
/// measures.
const DIFFUSE_MS: [f32; 4] = [0.7, 1.1, 1.7, 2.3];
const DIFFUSE_G: f32 = 0.65;

/// The size knob scales the delay lines over this range (0 → ×0.6, 1 → ×1.4).
const SIZE_LO: f32 = 0.6;
const SIZE_SPAN: f32 = 0.8;

/// Right channel line offset in samples: same network, decorrelated tail.
const STEREO_OFFSET: usize = 7;

/// Loop input calibration. `in_gain = LEVEL * (1 - g²)^0.75`: enough tail energy for the
/// wet to sound like a reverb, and long decays get a quieter injection so a sustained
/// input cannot pile up past the ceiling the tests check.
const LEVEL: f32 = 6.0;

/// Longest impulse response the synthetic IR generator will produce.
pub const MAX_IR_MS: f32 = 250.0;

/// Total energy of the synthetic IR: gain 1.4 for white noise, a touch above the
/// algorithmic half so the convolution side is not the quiet one.
const IR_ENERGY: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridReverbParams {
    /// 0 = small/dense, 1 = large/airy. Scales the delay lines.
    pub size: f32,
    /// RT60 in seconds, 0.2–12: the decay the network promises.
    pub decay: f32,
    /// Dry → wet. 0 is bit-exact dry.
    pub mix: f32,
    /// 0 = algorithmic only, 1 = convolution only.
    pub blend: f32,
    pub predelay_ms: f32,
    /// Highpass corner inside the feedback loop (low damping), Hz.
    pub damp_low: f32,
    /// Lowpass corner inside the feedback loop (high damping), Hz.
    pub damp_high: f32,
}

impl Default for HybridReverbParams {
    fn default() -> Self {
        Self {
            size: 0.5,
            decay: 2.0,
            mix: 0.35,
            blend: 0.35,
            predelay_ms: 20.0,
            damp_low: 20.0,
            damp_high: 16_000.0,
        }
    }
}

/// One delay line: fixed capacity, moving read offset (the size knob moves `len`).
struct Line {
    buf: Vec<f32>,
    pos: usize,
}

impl Line {
    #[inline(always)]
    fn read(&self, len: usize) -> f32 {
        self.buf[(self.pos + self.buf.len() - len) % self.buf.len()]
    }

    #[inline(always)]
    fn write(&mut self, v: f32) {
        self.buf[self.pos] = v;
        self.pos = (self.pos + 1) % self.buf.len();
    }

    /// Schroeder allpass: `v[n] = x + g·v[n-D]`, `y[n] = v[n-D] − g·v[n]`.
    #[inline(always)]
    fn allpass(&mut self, x: f32, len: usize, g: f32) -> f32 {
        let d = self.read(len);
        let v = x + g * d;
        self.write(v);
        d - g * v
    }
}

/// Pre-delay ring. Skips itself at `delay = 0` so `mix = 0` stays bit-exact dry.
struct Predelay {
    buf: Vec<f32>,
    pos: usize,
}

impl Predelay {
    fn new(cap: usize) -> Self {
        Self { buf: vec![0.0; cap.max(2)], pos: 0 }
    }

    #[inline(always)]
    fn tick(&mut self, x: f32, delay: usize) -> f32 {
        if delay == 0 {
            return x;
        }
        let cap = self.buf.len();
        let y = self.buf[(self.pos + cap - delay.min(cap - 1)) % cap];
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % cap;
        y
    }
}

/// One-pole lowpass and one-pole highpass (`x - lowpass(x)`) per line, per channel.
/// The two `t` weights are zero at the neutral knob positions, so undamped is a no-op and
/// the decay knob means exactly what it says.
#[derive(Clone, Copy, Default)]
struct Damp {
    lo: f32,
    hi: f32,
}

impl Damp {
    #[inline(always)]
    fn process(&mut self, x: f32, a_hp: f32, t_lo: f32, a_lp: f32, t_hi: f32) -> f32 {
        self.lo += a_hp * (x - self.lo);
        // highpass(x) = x - lo, so the blend is just `-t·lo`.
        let y = x - t_lo * self.lo;
        self.hi += a_lp * (y - self.hi);
        y + t_hi * (self.hi - y)
    }
}

/// In-place fast Walsh–Hadamard transform, normalised so the network is energy
/// preserving: with feedback below 1 it cannot grow.
#[inline(always)]
fn hadamard8(v: &mut [f32; LINES]) {
    let mut step = 1;
    while step < LINES {
        let mut i = 0;
        while i < LINES {
            for j in i..i + step {
                let (a, b) = (v[j], v[j + step]);
                v[j] = a + b;
                v[j + step] = a - b;
            }
            i += step * 2;
        }
        step *= 2;
    }
    let g = 1.0 / (LINES as f32).sqrt();
    for x in v.iter_mut() {
        *x *= g;
    }
}

/// Exponentially decaying noise, normalised to [`IR_ENERGY`] so the convolution half has
/// the same loudness as the diffusion half whatever its length.
pub fn decaying_noise_ir(sample_rate: f32, ms: f32) -> Vec<f32> {
    let sr = sample_rate.max(1.0);
    let n = ((ms.clamp(10.0, MAX_IR_MS) * 0.001 * sr) as usize).max(64);
    let len_s = n as f32 / sr;
    let mut seed = 0x2545_f491u32;
    let mut ir = Vec::with_capacity(n);
    for i in 0..n {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
        ir.push(noise * 10.0_f32.powf(-3.0 * (i as f32 / sr) / len_s));
    }
    let energy: f32 = ir.iter().map(|v| v * v).sum::<f32>().sqrt();
    if energy > 1e-9 {
        ir.iter_mut().for_each(|v| *v *= IR_ENERGY / energy);
    }
    ir
}

const CONV_PARAMS: ConvolverParams = ConvolverParams { mix: 1.0, gain_db: 0.0 };

pub struct HybridReverb {
    sample_rate: f32,
    lines: [[Line; LINES]; 2],
    diffusers: [[Line; DIFFUSE_MS.len()]; 2],
    damp: [[Damp; LINES]; 2],
    len: [[usize; LINES]; 2],
    diff_len: [[usize; DIFFUSE_MS.len()]; 2],
    gain: [f32; LINES],
    in_gain: [f32; LINES],
    pre_l: Predelay,
    pre_r: Predelay,
    convolver: Convolver,
    a_lp: f32,
    a_hp: f32,
    t_lp: f32,
    t_hp: f32,
    predelay: usize,
    /// The parameter tuple the tables above were built for.
    tuning: (f32, f32, usize, f32, f32),
}

impl HybridReverb {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_ir_ms(sample_rate, MAX_IR_MS)
    }

    /// `ir_ms` is the length of the synthetic impulse response (capped at 250 ms). It is
    /// a construction argument rather than a knob because changing it re-FFTs the
    /// partitions, and `Dsp::set` can run on the audio thread.
    pub fn with_ir_ms(sample_rate: f32, ir_ms: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = |ms: f32| ((ms * 1e-3 * sr) as usize).max(2) + STEREO_OFFSET + 2;
        let lines = std::array::from_fn(|ch| {
            std::array::from_fn(|i| Line {
                buf: vec![0.0; cap(BASE_MS[i] * (SIZE_LO + SIZE_SPAN)) + ch],
                pos: 0,
            })
        });
        let diffusers = std::array::from_fn(|ch| {
            std::array::from_fn(|i| Line { buf: vec![0.0; cap(DIFFUSE_MS[i]) + ch], pos: 0 })
        });
        let pre_cap = (0.2 * sr) as usize + 2;
        let mut convolver = Convolver::new(sr);
        convolver.set_ir(Some(Arc::new(decaying_noise_ir(sr, ir_ms))));
        Self {
            sample_rate: sr,
            lines,
            diffusers,
            damp: [[Damp::default(); LINES]; 2],
            len: [[0; LINES]; 2],
            diff_len: [[0; DIFFUSE_MS.len()]; 2],
            gain: [0.0; LINES],
            in_gain: [0.0; LINES],
            pre_l: Predelay::new(pre_cap),
            pre_r: Predelay::new(pre_cap),
            convolver,
            a_lp: 0.0,
            a_hp: 0.0,
            t_lp: 0.0,
            t_hp: 0.0,
            predelay: 0,
            // Impossible tuple, so the first `process` always builds the tables.
            tuning: (f32::NAN, 0.0, 0, 0.0, 0.0),
        }
    }

    /// Re-derives lengths, feedback gains and damping coefficients. Allocation-free.
    fn update(&mut self, p: &HybridReverbParams) {
        let size = p.size.clamp(0.0, 1.0);
        let decay = p.decay.clamp(0.2, 12.0);
        let predelay = (p.predelay_ms.clamp(0.0, 200.0) * 0.001 * self.sample_rate) as usize;
        let hi = p.damp_high.clamp(1_000.0, 20_000.0);
        let lo = p.damp_low.clamp(20.0, 1_000.0);
        if (size, decay, predelay, hi, lo) == self.tuning {
            return;
        }
        self.tuning = (size, decay, predelay, hi, lo);
        self.predelay = predelay;
        self.a_lp = 1.0 - (-2.0 * PI * hi / self.sample_rate).exp();
        self.a_hp = 1.0 - (-2.0 * PI * lo / self.sample_rate).exp();
        // Damping depth, zero at the neutral positions (20 kHz / 20 Hz).
        self.t_lp = ((20_000.0 - hi) / 19_000.0).clamp(0.0, 1.0);
        self.t_hp = ((lo - 20.0) / 980.0).clamp(0.0, 1.0);

        let scale = SIZE_LO + SIZE_SPAN * size;
        for ch in 0..2 {
            for i in 0..DIFFUSE_MS.len() {
                let len = ((DIFFUSE_MS[i] * scale * 1e-3 * self.sample_rate) as usize
                    + ch * STEREO_OFFSET / 2)
                    .min(self.diffusers[ch][i].buf.len() - 1);
                self.diff_len[ch][i] = len.max(1);
            }
            for i in 0..LINES {
                let len = ((BASE_MS[i] * scale * 1e-3 * self.sample_rate) as usize
                    + ch * STEREO_OFFSET)
                    .min(self.lines[ch][i].buf.len() - 1);
                self.len[ch][i] = len;
                // 60 dB over `decay` seconds: g^(t/len) = 10^(-3t/decay).
                let g = 10.0_f32.powf(-3.0 * (len as f32 / self.sample_rate) / decay);
                if ch == 0 {
                    self.gain[i] = g;
                    self.in_gain[i] = LEVEL * (1.0 - g * g).powf(0.75);
                }
            }
        }
    }

    /// The diffusion half. One sample in, one sample out per channel.
    fn diffuse(&mut self, l: f32, r: f32) -> (f32, f32) {
        let mut out = [0.0f32; 2];
        let mut v = [0.0f32; LINES];
        for ch in 0..2 {
            let mut x = if ch == 0 { l } else { r };
            for i in 0..DIFFUSE_MS.len() {
                x = self.diffusers[ch][i].allpass(x, self.diff_len[ch][i], DIFFUSE_G);
            }
            for i in 0..LINES {
                let y = self.lines[ch][i].read(self.len[ch][i]);
                v[i] = self.damp[ch][i].process(y, self.a_hp, self.t_hp, self.a_lp, self.t_lp);
            }
            out[ch] = if ch == 0 {
                (v[0] - v[2] + v[4] - v[6]) * 0.353_553_4
            } else {
                (v[1] - v[3] + v[5] - v[7]) * 0.353_553_4
            };
            hadamard8(&mut v);
            for i in 0..LINES {
                let w = self.gain[i] * v[i] + x * self.in_gain[i];
                self.lines[ch][i].write(w);
            }
        }
        (out[0], out[1])
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &HybridReverbParams) -> (f32, f32) {
        self.update(p);
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.0 {
            return (l, r);
        }
        let (wl, wr) = {
            let (xl, xr) = (
                self.pre_l.tick(l, self.predelay),
                self.pre_r.tick(r, self.predelay),
            );
            let (mut wl, mut wr) = self.diffuse(xl, xr);
            let blend = p.blend.clamp(0.0, 1.0);
            if blend > 0.0 {
                let (cl, cr) = self.convolver.process(xl, xr, &CONV_PARAMS);
                wl += blend * (cl - wl);
                wr += blend * (cr - wr);
            }
            (wl, wr)
        };
        (l + mix * (wl - l), r + mix * (wr - r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn params(decay: f32, mix: f32, blend: f32) -> HybridReverbParams {
        HybridReverbParams {
            decay,
            mix,
            blend,
            predelay_ms: 0.0,
            damp_low: 20.0,
            damp_high: 20_000.0,
            ..HybridReverbParams::default()
        }
    }

    fn render(rv: &mut HybridReverb, input: &[f32], p: &HybridReverbParams) -> Vec<f32> {
        input.iter().map(|&x| rv.process(x, x, p).0).collect()
    }

    fn impulse(n: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; n];
        v[0] = 1.0;
        v
    }

    fn noise(amp: f32, n: usize) -> Vec<f32> {
        let mut seed = 0x9e37_79b9u32;
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                amp * ((seed >> 8) as f32 / 8_388_608.0 - 1.0)
            })
            .collect()
    }

    /// Schroeder backward integration: RT60 is where the energy decay curve crosses
    /// -60 dB relative to the total energy.
    fn rt60(h: &[f32], sr: f32) -> f32 {
        let mut acc = 0.0f64;
        let mut edc = vec![0.0f64; h.len()];
        for i in (0..h.len()).rev() {
            acc += (h[i] as f64) * (h[i] as f64);
            edc[i] = acc;
        }
        let total = edc[0].max(1e-30);
        for (i, e) in edc.iter().enumerate() {
            if 10.0 * (e / total).log10() <= -60.0 {
                return i as f32 / sr;
            }
        }
        f32::INFINITY
    }

    /// Last time (seconds) the signal is still above -60 dBFS, measured on 10 ms blocks.
    fn audible_end(x: &[f32], sr: f32) -> f32 {
        let block = (0.010 * sr) as usize;
        let floor = 10.0_f32.powf(-60.0 / 20.0);
        let mut last = 0.0;
        for (b, chunk) in x.chunks(block).enumerate() {
            let peak = chunk.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            if peak > floor {
                last = (b + 1) as f32 * block as f32 / sr;
            }
        }
        last
    }

    /// (a) The `decay` knob is the RT60 the network actually delivers.
    #[test]
    fn rt60_matches_the_decay_knob() {
        for decay in [0.8f32, 1.5, 3.0] {
            let p = params(decay, 1.0, 0.0);
            let mut rv = HybridReverb::new(SR);
            let n = ((decay * 1.8 + 0.5) * SR) as usize;
            let h = render(&mut rv, &impulse(n), &p);
            let measured = rt60(&h, SR);
            let err = (measured - decay) / decay * 100.0;
            println!("rt60: promised {decay:.2} s, measured {measured:.2} s ({err:+.1} %)");
            assert!(
                (measured - decay).abs() <= 0.25 * decay,
                "decay {decay:.2} s promised, {measured:.2} s measured"
            );
        }
    }

    /// (b) The response is a dense tail, not a handful of echoes.
    ///
    /// A plain FDN answers an impulse with discrete echoes; the allpass front-end is what
    /// fills the gaps. The convolution half is noise by construction and is checked too.
    #[test]
    fn diffusion_is_dense() {
        let floor = 10.0_f32.powf(-40.0 / 20.0);
        for (label, blend) in [("algorithmic", 0.0f32), ("convolution", 1.0)] {
            let p = params(1.5, 1.0, blend);
            let mut rv = HybridReverb::new(SR);
            let h = render(&mut rv, &impulse((0.5 * SR) as usize), &p);
            let head = &h[..(0.1 * SR) as usize];
            let above = head.iter().filter(|v| v.abs() > floor).count();
            let share = above as f32 / head.len() as f32;
            println!(
                "{label}: {above}/{} samples above -40 dBFS = {:.1} %",
                head.len(),
                share * 100.0
            );
            assert!(share > 0.5, "{label} response is sparse: {share:.3}");
        }
    }

    /// (c) Ten seconds of full-scale-ish input at maximum decay stays bounded.
    #[test]
    fn maximum_decay_is_stable() {
        let p = params(12.0, 1.0, 0.5);
        let mut rv = HybridReverb::new(SR);
        let input = noise(0.9, (10.0 * SR) as usize);
        let mut peak = 0.0f32;
        for &x in &input {
            let (l, r) = rv.process(x, -x, &p);
            assert!(l.is_finite() && r.is_finite(), "non-finite output");
            peak = peak.max(l.abs()).max(r.abs());
        }
        println!("stability: peak {peak:.3} over 10 s at 0.9 in, decay 12 s");
        assert!(peak < 4.0, "output grew unbounded: {peak:.3}");
    }

    /// (d) `mix` is the dry/wet control: 0 is the input path, 1 has no dry left.
    #[test]
    fn mix_moves_from_dry_to_wet_only() {
        let input = noise(0.5, (1.0 * SR) as usize);
        let mut rv = HybridReverb::new(SR);
        let dry = render(&mut rv, &input, &params(2.0, 0.0, 0.5));
        let max_err = dry.iter().zip(&input).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        let scale = input.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        println!(
            "mix 0: max |out-in| {max_err:.3e} ({:.4} % of full scale)",
            max_err / scale * 100.0
        );
        assert!(max_err <= 0.01 * scale, "dry path is not the input: {max_err:.3e}");

        let mut rv = HybridReverb::new(SR);
        let wet = render(&mut rv, &input, &params(2.0, 1.0, 0.5));
        let corr = correlation(&input, &wet);
        println!("mix 1: correlation with input {corr:+.4}");
        assert!(corr.abs() < 0.3, "the dry signal is still in the output: {corr:.3}");
    }

    fn correlation(a: &[f32], b: &[f32]) -> f32 {
        let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
        for (x, y) in a.iter().zip(b) {
            ab += (*x as f64) * (*y as f64);
            aa += (*x as f64) * (*x as f64);
            bb += (*y as f64) * (*y as f64);
        }
        (ab / (aa * bb).sqrt().max(1e-30)) as f32
    }

    /// (e) The tail outlives half the promised decay after the input stops.
    #[test]
    fn tail_outlives_half_the_decay() {
        for decay in [1.0f32, 2.0] {
            let p = params(decay, 1.0, 0.0);
            let mut rv = HybridReverb::new(SR);
            let burst = noise(0.7, (0.25 * SR) as usize);
            let mut input = burst.clone();
            input.extend(std::iter::repeat(0.0).take((4.0 * SR) as usize));
            let out = render(&mut rv, &input, &p);
            let end = audible_end(&out, SR) - burst.len() as f32 / SR;
            println!(
                "tail: decay {decay:.1} s promised, {end:.2} s above -60 dBFS after the input stops"
            );
            assert!(
                end >= 0.5 * decay,
                "tail {end:.2} s is shorter than half the {decay:.1} s decay"
            );
        }
    }
}
