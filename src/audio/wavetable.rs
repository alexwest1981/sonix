//! **Wavetable oscillator:** 2048-sample frames, one band-limited mip per octave, morphing
//! between frames, and up to seven detuned copies.
//!
//! ## File format (`Wavetable::from_text`)
//!
//! ASCII, one table per file, tokens separated by whitespace:
//!
//! ```text
//! SONIXWT 1
//! frames 3
//! len 2048
//! <2048 floats>          frame 0
//! <2048 floats>          frame 1
//! <2048 floats>          frame 2
//! ```
//!
//! `SONIXWT` is the magic, `1` the version. `len` must be `FRAME_LEN` and `frames` 1..=`MAX_FRAMES`.
//! The parser is strict: a missing or extra sample is an error, so a truncated file cannot
//! silently become a shorter table — a short frame would play the wrong pitch instead of not
//! at all.
//!
//! ## Mipmaps
//!
//! Level `m` keeps harmonics up to `mip_limit(m)` = 1023 >> m, i.e. one halving per level, and
//! is built by zeroing the bins above that limit in the frame's FFT and inverting — not by
//! averaging neighbouring samples, which would also dull the harmonics that do fit. `mip_for`
//! picks the **first** level whose limit fits under Nyquist for the note's frequency, so no
//! note can get a harmonic that folds back into the audible range. The price is that a note at
//! the bottom of an octave gets up to twice the harmonics it needs — audible as slightly
//! duller top end, never as aliasing.
//!
//! ## Detune law (unison)
//!
//! Voice `i` of `n` is detuned by `cents = spread * UNISON_CENTS * (2i/(n-1) - 1)`, symmetric
//! and evenly spaced, the outer pair at ±`UNISON_CENTS` cents when `spread` is 1.0. 25 cents is
//! 1.45 %, so every voice of a seven-voice unison stays inside ±2 % of the fundamental.

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;
use std::sync::OnceLock;

/// Samples per frame. Fixed: the file format and the mip builder both assume it.
pub const FRAME_LEN: usize = 2048;
/// Frame limit for one table (Ableton's wavetables top out around eight).
pub const MAX_FRAMES: usize = 8;
/// Voice limit for one unison.
pub const MAX_UNISON: usize = 7;
/// One mip per octave: 1023 harmonics down to 1.
pub const MIP_LEVELS: usize = 11;
/// Highest harmonic in the untouched frame (everything under the table's own Nyquist).
const BASE_HARMONICS: usize = FRAME_LEN / 2 - 1;
/// Detune of the outer voices at `spread` 1.0, in cents.
pub const UNISON_CENTS: f32 = 25.0;
/// Attack and release ramp in seconds — a gate that switches without one clicks.
const GATE_SECONDS: f32 = 0.005;

/// Names of the built-in tables, in `builtin` order.
pub const TABLE_NAMES: [&str; 3] = ["Sine to saw", "Square to pulse", "Double formant"];

/// Harmonic ceiling of a mip level. Level 0 is untouched, each level below halves.
#[inline]
pub fn mip_limit(level: usize) -> usize {
    (BASE_HARMONICS >> level.min(MIP_LEVELS - 1)).max(1)
}

/// Mip level for a frequency: the first level whose harmonic ceiling fits under Nyquist.
#[inline]
pub fn mip_for(freq: f32, sample_rate: f32) -> usize {
    let allowed = (0.5 * sample_rate / freq.max(1.0)) as usize;
    (0..MIP_LEVELS).find(|m| mip_limit(*m) <= allowed).unwrap_or(MIP_LEVELS - 1)
}

/// One table: 1..=8 frames, each in `MIP_LEVELS` band-limited versions.
pub struct Wavetable {
    /// `frames[frame][mip_level]` — `FRAME_LEN` samples, one cycle.
    frames: Vec<[Vec<f32>; MIP_LEVELS]>,
}

impl Wavetable {
    /// Builds a table from finished frames (one cycle of samples each) and adds the mips.
    fn with_mips(mut raw: Vec<Vec<f32>>) -> Self {
        assert!(!raw.is_empty() && raw.len() <= MAX_FRAMES, "wavetable: 1..=8 frames");
        assert!(raw.iter().all(|f| f.len() == FRAME_LEN), "wavetable: frame length");
        // One normalisation for the whole table: normalising frames separately makes the level
        // jump as the morph crosses from one frame to the next.
        let peak = raw
            .iter()
            .flat_map(|f| f.iter())
            .fold(0.0f32, |m, s| m.max(s.abs()))
            .max(1e-9);
        for frame in &mut raw {
            for s in frame {
                *s /= peak;
            }
        }

        let mut planner = FftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(FRAME_LEN);
        let inv = planner.plan_fft_inverse(FRAME_LEN);
        let frames = raw
            .into_iter()
            .map(|frame| {
                std::array::from_fn(|level| {
                    if level == 0 {
                        return frame.clone();
                    }
                    let limit = mip_limit(level);
                    let mut buf: Vec<Complex32> =
                        frame.iter().map(|s| Complex32::new(*s, 0.0)).collect();
                    fwd.process(&mut buf);
                    // Zero the harmonics above the limit and their mirror bins; DC and the
                    // Nyquist bin carry no table content and stay as they are.
                    for k in limit + 1..=FRAME_LEN / 2 {
                        buf[k] = Complex32::default();
                        buf[FRAME_LEN - k] = Complex32::default();
                    }
                    inv.process(&mut buf);
                    buf.iter().map(|c| c.re / FRAME_LEN as f32).collect()
                })
            })
            .collect();
        Self { frames }
    }

    /// One frame from a list of harmonic amplitudes (harmonic 1 first) — built as an
    /// inverse FFT.
    fn frame_from_harmonics(amps: &[f32]) -> Vec<f32> {
        let mut buf = vec![Complex32::default(); FRAME_LEN];
        for (i, a) in amps.iter().take(BASE_HARMONICS).enumerate() {
            let k = i + 1;
            // A sine as X[k] = -i·a/2 with its conjugate at X[N-k]; the scale is irrelevant,
            // `with_mips` normalises.
            buf[k] = Complex32::new(0.0, -0.5 * a);
            buf[FRAME_LEN - k] = Complex32::new(0.0, 0.5 * a);
        }
        FftPlanner::<f32>::new().plan_fft_inverse(FRAME_LEN).process(&mut buf);
        buf.iter().map(|c| c.re).collect()
    }

    /// Builds a table from one harmonic amplitude list per frame.
    pub fn from_harmonics(frames: &[Vec<f32>]) -> Self {
        Self::with_mips(frames.iter().map(|a| Self::frame_from_harmonics(a)).collect())
    }

    /// Parses the file format documented at the top of this module. Strict: anything that does
    /// not match is an error.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut tokens = text.split_whitespace();
        let mut next = |what: &str| tokens.next().ok_or_else(|| format!("wavetable: missing {what}"));
        if next("header")? != "SONIXWT" {
            return Err("wavetable: missing SONIXWT header".to_string());
        }
        let version: u32 = next("version")?.parse().map_err(|_| "wavetable: bad version")?;
        if version != 1 {
            return Err(format!("wavetable: unsupported version {version}"));
        }
        if next("frames header")? != "frames" {
            return Err("wavetable: expected 'frames'".to_string());
        }
        let frames: usize =
            next("frame count")?.parse().map_err(|_| "wavetable: bad frame count")?;
        if next("len header")? != "len" {
            return Err("wavetable: expected 'len'".to_string());
        }
        let len: usize = next("frame length")?.parse().map_err(|_| "wavetable: bad length")?;
        if len != FRAME_LEN {
            return Err(format!("wavetable: length {len}, expected {FRAME_LEN}"));
        }
        if frames == 0 || frames > MAX_FRAMES {
            return Err(format!("wavetable: {frames} frames, expected 1..={MAX_FRAMES}"));
        }
        let mut raw = Vec::with_capacity(frames);
        for f in 0..frames {
            let mut frame = Vec::with_capacity(FRAME_LEN);
            for i in 0..FRAME_LEN {
                let s: f32 = next(&format!("sample {i} of frame {f}"))?
                    .parse()
                    .map_err(|_| format!("wavetable: bad sample {i} of frame {f}"))?;
                if !s.is_finite() {
                    return Err(format!("wavetable: sample {i} of frame {f} is not finite"));
                }
                frame.push(s);
            }
            raw.push(frame);
        }
        if tokens.next().is_some() {
            return Err("wavetable: trailing samples".to_string());
        }
        Ok(Self::with_mips(raw))
    }

    /// Writes the untouched frame (mip 0) in the format `from_text` reads.
    pub fn to_text(&self) -> String {
        let mut out = format!("SONIXWT 1\nframes {}\nlen {}\n", self.frames.len(), FRAME_LEN);
        for frame in &self.frames {
            for (i, s) in frame[0].iter().enumerate() {
                out.push_str(&format!("{s:.6}"));
                out.push(if i % 16 == 15 { '\n' } else { ' ' });
            }
        }
        out
    }

    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    /// Reads the table at a position (0..1) and a phase in [0, 1). The morph is linear between
    /// the two frames the position falls between.
    #[inline]
    pub fn read(&self, position: f32, mip: usize, phase: f32) -> f32 {
        let page = position.clamp(0.0, 1.0) * (self.frames.len() - 1) as f32;
        let page_i = (page as usize).min(self.frames.len() - 1);
        let mip = mip.min(MIP_LEVELS - 1);
        let a = Self::read_frame(&self.frames[page_i][mip], phase);
        if page_i + 1 >= self.frames.len() {
            return a;
        }
        let b = Self::read_frame(&self.frames[page_i + 1][mip], phase);
        a + (b - a) * (page - page_i as f32)
    }

    #[inline]
    fn read_frame(frame: &[f32], phase: f32) -> f32 {
        let x = phase * FRAME_LEN as f32;
        let i = (x as usize).min(FRAME_LEN - 1);
        let a = frame[i];
        a + (frame[(i + 1) & (FRAME_LEN - 1)] - a) * (x - i as f32)
    }
}

static BUILTIN_0: OnceLock<Wavetable> = OnceLock::new();
static BUILTIN_1: OnceLock<Wavetable> = OnceLock::new();
static BUILTIN_2: OnceLock<Wavetable> = OnceLock::new();

/// The tables that ship in code — no binary asset in the repo. Built once, on first use (the
/// mip build is 11 FFTs per frame, too expensive to do per note).
pub fn builtin(index: usize) -> &'static Wavetable {
    match index.min(TABLE_NAMES.len() - 1) {
        0 => BUILTIN_0.get_or_init(|| Wavetable::from_harmonics(&sine_to_saw())),
        1 => BUILTIN_1.get_or_init(|| Wavetable::from_harmonics(&square_to_pulse())),
        _ => BUILTIN_2.get_or_init(|| Wavetable::from_harmonics(&double_formant())),
    }
}

/// Sine to saw: frame `f` brings in the saw's 1/k with `w = f/4`, so position 0 is a pure sine
/// and position 1 a saw.
fn sine_to_saw() -> Vec<Vec<f32>> {
    (0..5)
        .map(|f| {
            let w = f as f32 / 4.0;
            (1..=BASE_HARMONICS)
                .map(|k| {
                    let k = k as f32;
                    (1.0 - w) * if k == 1.0 { 1.0 } else { 0.0 } + w / k
                })
                .collect()
        })
        .collect()
}

/// Square to pulse: a pulse train with duty 0.5 down to 0.08. Harmonic `k` weighs
/// `2·sin(πkd)/(πk)`, which at d = 0.5 gives the square's odd 1/k.
fn square_to_pulse() -> Vec<Vec<f32>> {
    [0.5, 0.35, 0.22, 0.14, 0.08]
        .iter()
        .map(|d| {
            (1..=BASE_HARMONICS)
                .map(|k| {
                    let k = k as f32;
                    2.0 * (std::f32::consts::PI * k * d).sin() / (std::f32::consts::PI * k)
                })
                .collect()
        })
        .collect()
}

/// Double formant: two Gaussian bumps in the harmonic plane, the second one climbing through
/// the frames. No saw underneath — the sound is the bumps.
fn double_formant() -> Vec<Vec<f32>> {
    (0..3)
        .map(|f| {
            let c2 = 7.0 + 2.0 * f as f32;
            (1..=BASE_HARMONICS)
                .map(|k| {
                    let k = k as f32;
                    let a = (-((k - 3.0) / 1.2).powi(2)).exp();
                    let b = 0.7 * (-((k - c2) / 1.8).powi(2)).exp();
                    a + b
                })
                .collect()
        })
        .collect()
}

/// The oscillator: one table, one position, one unison setting, its own phase per voice.
pub struct WavetableOsc {
    table: &'static Wavetable,
    position: f32,
    unison: usize,
    spread: f32,
    /// Frequency ratio per voice, computed when spread/unison change — not per sample.
    ratios: [f32; MAX_UNISON],
    /// Pan per voice, -1..1 (left..right).
    pans: [f32; MAX_UNISON],
    /// Own phase per voice, in cycles.
    phases: [f32; MAX_UNISON],
    level: f32,
    ramp: f32,
    sample_rate: f32,
}

impl WavetableOsc {
    pub fn new(sample_rate: f32) -> Self {
        let mut osc = Self {
            table: builtin(0),
            position: 0.0,
            unison: 1,
            spread: 0.0,
            ratios: [1.0; MAX_UNISON],
            pans: [0.0; MAX_UNISON],
            // Spread start phases so seven voices do not begin in phase and sum into a single
            // hard transient.
            phases: std::array::from_fn(|i| i as f32 / MAX_UNISON as f32),
            level: 0.0,
            ramp: 1.0 / (GATE_SECONDS * sample_rate).max(1.0),
            sample_rate,
        };
        osc.update_voices();
        osc
    }

    pub fn set_table(&mut self, index: usize) {
        self.table = builtin(index);
    }

    pub fn set_position(&mut self, position: f32) {
        self.position = position.clamp(0.0, 1.0);
    }

    pub fn set_unison(&mut self, voices: usize) {
        self.unison = voices.clamp(1, MAX_UNISON);
        self.update_voices();
    }

    pub fn set_spread(&mut self, spread: f32) {
        self.spread = spread.clamp(0.0, 1.0);
        self.update_voices();
    }

    fn update_voices(&mut self) {
        let n = self.unison;
        for i in 0..MAX_UNISON {
            let t = if n > 1 && i < n { 2.0 * i as f32 / (n - 1) as f32 - 1.0 } else { 0.0 };
            let cents = self.spread * UNISON_CENTS * t;
            self.ratios[i] = 2f32.powf(cents / 1200.0);
            self.pans[i] = t;
        }
    }

    /// One sample. `mip` forces a mip level (`None` = pick by frequency); the forced level
    /// exists so the aliasing measurement can compare against a naive single-mip playback.
    #[inline]
    pub fn next_stereo_mip(&mut self, freq: f32, gate: bool, mip: Option<usize>) -> (f32, f32) {
        let target = if gate { 1.0 } else { 0.0 };
        self.level = if self.level < target {
            (self.level + self.ramp).min(target)
        } else {
            (self.level - self.ramp).max(target)
        };

        let mip = mip.unwrap_or_else(|| mip_for(freq, self.sample_rate));
        let n = self.unison;
        let inv_step = 1.0 / self.sample_rate;
        let (mut l, mut r) = (0.0, 0.0);
        for i in 0..n {
            let phase = self.phases[i];
            let s = self.table.read(self.position, mip, phase);
            let pan = self.pans[i];
            l += s * (1.0 - pan) * 0.5;
            r += s * (1.0 + pan) * 0.5;
            self.phases[i] = (phase + freq * self.ratios[i] * inv_step).fract();
        }
        // Divided by the square root of the voice count: seven voices should sound wider, not
        // seven times louder.
        let gain = self.level / (n as f32).sqrt();
        (l * gain, r * gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wavetable_text_format_round_trips() {
        let table = builtin(2);
        let text = table.to_text();
        let loaded = Wavetable::from_text(&text).expect("round trip");
        assert_eq!(loaded.frames(), table.frames());
        for i in 0..FRAME_LEN {
            let diff = (loaded.frames[0][0][i] - table.frames[0][0][i]).abs();
            assert!(diff < 1e-5, "sample {i} differs by {diff}");
        }
        // A truncated file must be an error, not a shorter table.
        let short = format!("SONIXWT 1\nframes 1\nlen {FRAME_LEN}\n{}", "0 ".repeat(10));
        assert!(Wavetable::from_text(&short).is_err());
        // An extra sample must be an error too.
        assert!(Wavetable::from_text(&format!("{text} 0.5")).is_err());
    }

    #[test]
    fn mip_levels_halve_the_harmonic_limit() {
        assert_eq!(mip_limit(0), BASE_HARMONICS);
        assert_eq!(mip_limit(1), BASE_HARMONICS / 2);
        assert_eq!(mip_limit(MIP_LEVELS + 3), 1);
        assert_eq!(mip_for(8_000.0, 48_000.0), 8, "3 harmonics fit under Nyquist at 8 kHz");
        assert_eq!(mip_for(20.0, 48_000.0), 0);
    }
}
