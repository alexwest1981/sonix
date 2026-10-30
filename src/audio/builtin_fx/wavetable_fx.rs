//! **Wavetable as an effect-instrument.** The oscillator lives in `audio::wavetable`; this file
//! is the registry entry plus the 2-pole filter that follows it.
//!
//! **`process` never synthesizes.** As a channel effect the unit is a pass-through that only
//! filters what comes in, so dropping it on an ordinary track is a plain 2-pole lowpass and the
//! knobs Table/Position/Unison/Spread do nothing there. The oscillator is heard only where a
//! source calls `render_note`, which overwrites the buffers with the note's signal filtered
//! through the same filter state. (The synth's own oscillator path — `synth::voices` — was left
//! alone: it renders per-voice envelopes and needs no effect registry, so a `Waveform` variant
//! would have to be threaded through every match arm for no gain.)

use super::{Dsp, EffectDef, ParamDef};
use crate::audio::filter::{FilterParams, StateVariableFilter};
use crate::audio::plugin_host::PluginCategory;
use crate::audio::wavetable::{WavetableOsc, MAX_UNISON, TABLE_NAMES};

pub static WAVETABLE_FX: EffectDef = EffectDef {
    id: "wavetable",
    name: "Wavetable (oscillator)",
    category: PluginCategory::Synth,
    description: "Morphing wavetable with octave mipmaps, seven voices and a 2-pole filter",
    params: &[
        ParamDef::step("Table (0=sine-saw, 1=square-pulse, 2=double formant)", 0.0, 2.0, 0.0),
        ParamDef::knob("Position", 0.0, 1.0, 0.0),
        ParamDef::step("Unison", 1.0, MAX_UNISON as f64, 1.0),
        ParamDef::knob("Spread", 0.0, 1.0, 0.0),
        ParamDef::knob("Filter cutoff", 20.0, 20_000.0, 20_000.0),
        ParamDef::knob("Resonance", 0.5, 1.25, 0.7),
    ],
    make: |sr| Box::new(WavetableFx::new(sr)),
};

pub struct WavetableFx {
    osc: WavetableOsc,
    params: FilterParams,
    filter_l: StateVariableFilter,
    filter_r: StateVariableFilter,
    sample_rate: f32,
}

impl WavetableFx {
    pub fn new(sample_rate: f32) -> Self {
        let params = FilterParams { cutoff: 20_000.0, resonance: 0.7 };
        Self {
            osc: WavetableOsc::new(sample_rate),
            params,
            filter_l: StateVariableFilter::new(sample_rate),
            filter_r: StateVariableFilter::new(sample_rate),
            sample_rate,
        }
    }

    /// Renders one note into the buffers, **overwriting** what is there. `gate` low releases
    /// with the oscillator's 5 ms ramp, so the tail is cut, not clicked.
    pub fn render_note(&mut self, freq: f32, gate: bool, out_l: &mut [f32], out_r: &mut [f32]) {
        self.render_note_with_mip(freq, gate, None, out_l, out_r)
    }

    /// `mip` forces a mip level for the measurement that compares against a naive single-mip
    /// playback; `None` picks by frequency, which is what `render_note` does.
    pub fn render_note_with_mip(
        &mut self,
        freq: f32,
        gate: bool,
        mip: Option<usize>,
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        let params = self.params;
        for i in 0..out_l.len().min(out_r.len()) {
            let (l, r) = self.osc.next_stereo_mip(freq, gate, mip);
            out_l[i] = self.filter_l.process_lowpass(l, &params);
            out_r[i] = self.filter_r.process_lowpass(r, &params);
        }
    }
}

impl Dsp for WavetableFx {
    /// Pass-through: filters the incoming signal, generates nothing.
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let params = self.params;
        for i in 0..left.len().min(right.len()) {
            left[i] = self.filter_l.process_lowpass(left[i], &params);
            right[i] = self.filter_r.process_lowpass(right[i], &params);
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        let v = value as f32;
        match index {
            0 => self.osc.set_table(v.clamp(0.0, TABLE_NAMES.len() as f32 - 1.0) as usize),
            1 => self.osc.set_position(v),
            2 => self.osc.set_unison(v.clamp(1.0, MAX_UNISON as f32) as usize),
            3 => self.osc.set_spread(v),
            4 => self.params.cutoff = v.clamp(20.0, 20_000.0),
            5 => self.params.resonance = v.clamp(0.5, 1.25),
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.filter_l.reset();
        self.filter_r.reset();
        self.osc = WavetableOsc::new(self.sample_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::Complex32;
    use rustfft::FftPlanner;

    const SR: f32 = 48_000.0;
    /// The gate has settled after 5 ms; skip 0.1 s so the analysis window holds steady state.
    const SKIP: usize = 4_800;

    fn unit() -> WavetableFx {
        let mut fx = WavetableFx::new(SR);
        fx.set(0, 0.0); // sine -> saw
        fx.set(1, 1.0); // saw end of the morph
        fx.set(2, 1.0); // one voice
        fx.set(3, 0.0);
        fx
    }

    /// Mono sum of the note (`n` samples after the skip). The sum, not the left channel: the
    /// unison pans its voices hard, so one channel alone is missing the outermost voice.
    fn render(fx: &mut WavetableFx, freq: f32, n: usize, mip: Option<usize>) -> Vec<f32> {
        let mut l = vec![0.0; n + SKIP];
        let mut r = vec![0.0; n + SKIP];
        fx.render_note_with_mip(freq, true, mip, &mut l, &mut r);
        l[SKIP..].iter().zip(&r[SKIP..]).map(|(a, b)| 0.5 * (a + b)).collect()
    }

    fn magnitudes(x: &[f32]) -> Vec<f32> {
        let mut buf: Vec<Complex32> = x.iter().map(|s| Complex32::new(*s, 0.0)).collect();
        FftPlanner::<f32>::new().plan_fft_forward(x.len()).process(&mut buf);
        buf.iter().map(|c| c.norm()).collect()
    }

    /// Energy in bins above `1.5·f0` that are not within two bins of a harmonic of `f0`,
    /// relative to the fundamental, in dB.
    fn alias_floor_db(spec: &[f32], f0: f32, n: usize) -> f32 {
        let bin_hz = SR / n as f32;
        let fund = (f0 / bin_hz).round() as usize;
        let fundamental = spec[fund].max(1e-20);
        let mut alias = 0.0f64;
        for k in (1.5 * f0 / bin_hz) as usize..n / 2 {
            let hz = k as f32 * bin_hz;
            let nearest = (hz / f0).round() * f0;
            if (hz - nearest).abs() <= 2.0 * bin_hz {
                continue;
            }
            alias += (spec[k] as f64).powi(2);
        }
        (10.0 * (alias / (fundamental as f64).powi(2)).log10()) as f32
    }

    /// (mipmapped, naive single mip) alias floors for one fundamental.
    fn alias_floors(f0: f32, n: usize) -> (f32, f32) {
        let mip = alias_floor_db(&magnitudes(&render(&mut unit(), f0, n, None)), f0, n);
        let naive = alias_floor_db(&magnitudes(&render(&mut unit(), f0, n, Some(0))), f0, n);
        (mip, naive)
    }

    #[test]
    fn mip_selection_suppresses_aliasing() {
        // 3 s at 48 kHz: 14 kHz lands exactly on a bin (bin width 1/3 Hz), so the rectangular
        // window leaks nothing and the floor is real alias energy.
        let n = 3 * 48_000;
        for f0 in [8_000.0f32, 14_000.0] {
            let (mip, naive) = alias_floors(f0, n);
            println!(
                "alias floor at {f0:.0} Hz (saw, 48 kHz): mipmapped {mip:.1} dB, \
                 naive single mip {naive:.1} dB"
            );
            assert!(mip < -40.0, "{f0} Hz: alias floor {mip} dB is not below -40 dB");
        }
        // 8 kHz folds onto the 8 kHz grid (48/8 = 6), so a single-mip playback hides its
        // aliases in the DC and harmonic bins. Print that DC figure too: it is where the
        // folded even harmonics land.
        let n = 48_000;
        let mip_dc = magnitudes(&render(&mut unit(), 8_000.0, n, None))[1];
        let naive = magnitudes(&render(&mut unit(), 8_000.0, n, Some(0)));
        let level = 20.0 * (mip_dc.max(1e-20) / naive[24_000]).log10();
        println!("8 kHz DC relative to the fundamental: mipmapped {level:.1} dB");
        let naive_dc = 20.0 * (naive[1].max(1e-20) / naive[24_000]).log10();
        println!("8 kHz DC relative to the fundamental: naive single mip {naive_dc:.1} dB");
    }

    /// Share of harmonics 2..8 in the energy of harmonics 1..64.
    fn upper_share(position: f32) -> f32 {
        let mut fx = unit();
        fx.set(1, position as f64);
        let n = 48_000; // 375 Hz sits on bin 375 exactly
        let spec = magnitudes(&render(&mut fx, 375.0, n, None));
        let energy = |from: usize, to: usize| -> f64 {
            (from..=to)
                .map(|k| (spec[375 * k] as f64).powi(2))
                .sum::<f64>()
        };
        (energy(2, 8) / energy(1, 64).max(1e-30)) as f32
    }

    #[test]
    fn morph_raises_the_upper_harmonics() {
        let start = upper_share(0.0);
        let end = upper_share(1.0);
        println!(
            "morph H2..H8 share of H1..H64 (sine->saw): position 0 = {start:.4}, position 1 = {end:.4}"
        );
        assert!(end > 4.0 * start + 0.1, "morph did not raise the upper harmonics");
    }

    /// Local maxima inside ±2 % of `f0`, at least 20 dB under the loudest bin there.
    fn peak_count(voices: usize, spread: f32) -> (usize, Vec<f32>) {
        let mut fx = unit();
        fx.set(2, voices as f64);
        fx.set(3, spread as f64);
        let n = 48_000; // 1 Hz bins
        let x = render(&mut fx, 1_000.0, n, None);
        // Hann: the sidelobes of a rectangular window would read as extra peaks between the
        // detuned voices.
        let windowed: Vec<f32> = x
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let w = 0.5
                    - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
                s * w
            })
            .collect();
        let spec = magnitudes(&windowed);
        let lo = (0.98 * 1_000.0) as usize;
        let hi = (1.02 * 1_000.0) as usize;
        let max = spec[lo..=hi].iter().cloned().fold(0.0f32, f32::max);
        let peaks: Vec<f32> = (lo + 1..hi)
            .filter(|&k| spec[k] > 0.1 * max && spec[k] > spec[k - 1] && spec[k] >= spec[k + 1])
            .map(|k| k as f32)
            .collect();
        (peaks.len(), peaks)
    }

    #[test]
    fn unison_widens_the_spectrum() {
        let (one, _) = peak_count(1, 0.0);
        let (seven, at) = peak_count(7, 1.0);
        println!("distinct peaks within ±2 % of 1000 Hz: 1 voice = {one}, 7 voices spread 1.0 = {seven} at {at:?} Hz");
        assert!(seven > one, "unison did not add spectral peaks ({one} -> {seven})");
    }

    #[test]
    fn ten_seconds_are_finite_dc_free_and_below_two() {
        let mut fx = WavetableFx::new(SR);
        fx.set(0, 2.0); // formant table: the asymmetric one
        fx.set(1, 0.5);
        fx.set(2, 7.0);
        fx.set(3, 1.0);
        fx.set(4, 4_000.0);
        fx.set(5, 1.25);
        let n = 10 * 48_000;
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        fx.render_note(220.0, true, &mut l, &mut r);
        let finite = l.iter().chain(r.iter()).all(|s| s.is_finite());
        let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, s| m.max(s.abs()));
        let mean = l.iter().sum::<f32>() / n as f32;
        println!("10 s at 220 Hz (7 voices, spread 1.0): peak {peak:.3}, mean {mean:.6}, finite {finite}");
        assert!(finite, "NaN or inf in the render");
        assert!(peak < 2.0, "peak {peak} is not below 2.0");
        assert!(mean.abs() < 1e-4, "DC {mean} is not negligible");
    }
}
