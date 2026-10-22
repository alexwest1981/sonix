//! Real-time master bus FX chain and per-track equalizer DSP.
//!
//! Everything here is allocation-free while processing and designed to run on
//! the audio thread. The parameters are plain `Copy` structs so they can be
//! shipped through the lock-free command ring buffer without cloning buffers.

use std::f32::consts::PI;
use std::sync::Arc;

use super::vocal_harmonizer::fft_radix2;

// ---------------------------------------------------------------------------
// Biquad building blocks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]

struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn bypass() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    #[inline(always)]
    fn process(&mut self, x: f32, c: &BiquadCoeffs) -> f32 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn peaking(sr: f32, freq: f32, gain_db: f32, q: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q.max(0.05));
    let a0 = 1.0 + alpha / a;
    BiquadCoeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: (-2.0 * cos_w0) / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: (-2.0 * cos_w0) / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

fn low_shelf(sr: f32, freq: f32, gain_db: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let sqrt_a = a.sqrt();
    let alpha = sin_w0 / 2.0 * (2.0_f32).sqrt();
    let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
    BiquadCoeffs {
        b0: a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha) / a0,
        b1: 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0) / a0,
        b2: a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
        a1: -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0) / a0,
        a2: ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
    }
}

fn high_shelf(sr: f32, freq: f32, gain_db: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let sqrt_a = a.sqrt();
    let alpha = sin_w0 / 2.0 * (2.0_f32).sqrt();
    let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
    BiquadCoeffs {
        b0: a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha) / a0,
        b1: -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0) / a0,
        b2: a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
        a1: 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0) / a0,
        a2: ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
    }
}



/// **Control Room** (Cubase): lyssningssektionen, efter limitern och därmed efter allt som
/// påverkar ljudet. Monokontroll, dämpning och lyssningsnivå — verktyg för att kontrollera en
/// mix, inte för att forma den. Allt av som standard, så utgången är identisk med förut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlRoomParams {
    pub mono: bool,
    pub dim: bool,
    pub dim_db: f32,
    pub level_db: f32,
}

impl Default for ControlRoomParams {
    fn default() -> Self {
        Self { mono: false, dim: false, dim_db: -20.0, level_db: 0.0 }
    }
}

/// **Multiband Delay** (FL): tre band, tre olika fördröjningar. Delningsfiltret är samma
/// `XoverChan` som den flerbandiga kompressorn använder — samma delning, samma summa.
///
/// Ingen återkoppling: tre tider och en mix är vad pedalen har rattar till, och en delay utan
/// återkoppling är fortfarande en delay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasterFxParams {
    pub eq_bands: [MasterEqBand; 4],
    pub eq_enabled: bool,
    pub comp: CompressorParams,
    pub comp_enabled: bool,
    pub doubler: DoublerParams,
    pub doubler_enabled: bool,
    pub deesser: DeEsserParams,
    pub deesser_enabled: bool,
    pub transient: TransientParams,
    pub transient_enabled: bool,
    pub sat: SaturatorParams,
    pub sat_enabled: bool,
    pub chorus: ModFxParams,
    pub chorus_enabled: bool,
    pub flanger: ModFxParams,
    pub flanger_enabled: bool,
    pub phaser: ModFxParams,
    pub phaser_enabled: bool,
    pub multiband: MultibandParams,
    pub multiband_enabled: bool,
    pub convolver: ConvolverParams,
    pub convolver_enabled: bool,
    pub gate: GateParams,
    pub gate_enabled: bool,
    pub filter: MasterFilterParams,
    pub limiter: LimiterParams,
    pub limiter_enabled: bool,
    pub granular: GranularParams,
    pub granular_enabled: bool,
    pub freq_shift: FreqShiftParams,
    pub freq_shift_enabled: bool,
    pub crush: BitCrushParams,
    pub crush_enabled: bool,
    pub mb_delay: MultibandDelayParams,
    pub mb_delay_enabled: bool,
    pub delay_bank: DelayBankParams,
    pub delay_bank_enabled: bool,
    pub distructor: DistructorParams,
    pub distructor_enabled: bool,
    pub hardcore: HardcoreParams,
    pub hardcore_enabled: bool,
    pub effector: EffectorParams,
    pub effector_enabled: bool,
    pub control: ControlRoomParams,
}

impl Default for MasterFxParams {
    fn default() -> Self {
        Self {
            eq_bands: [MasterEqBand::flat(); 4],
            eq_enabled: true,
            comp: CompressorParams::default(),
            comp_enabled: false,
            doubler: DoublerParams::default(),
            doubler_enabled: false,
            deesser: DeEsserParams::default(),
            deesser_enabled: false,
            transient: TransientParams::default(),
            transient_enabled: false,
            sat: SaturatorParams::default(),
            sat_enabled: false,
            chorus: ModFxParams::default(),
            chorus_enabled: false,
            flanger: ModFxParams::default(),
            flanger_enabled: false,
            phaser: ModFxParams::default(),
            phaser_enabled: false,
            multiband: MultibandParams::default(),
            multiband_enabled: false,
            convolver: ConvolverParams::default(),
            convolver_enabled: false,
            gate: GateParams::default(),
            gate_enabled: false,
            filter: MasterFilterParams::default(),
            granular: GranularParams::default(),
            granular_enabled: false,
            freq_shift: FreqShiftParams::default(),
            freq_shift_enabled: false,
            crush: BitCrushParams::default(),
            crush_enabled: false,
            mb_delay: MultibandDelayParams::default(),
            mb_delay_enabled: false,
            delay_bank: DelayBankParams::default(),
            delay_bank_enabled: false,
            distructor: DistructorParams::default(),
            distructor_enabled: false,
            hardcore: HardcoreParams::default(),
            hardcore_enabled: false,
            effector: EffectorParams::default(),
            effector_enabled: false,
            control: ControlRoomParams::default(),
            limiter: LimiterParams::default(),
            limiter_enabled: false,
        }
    }
}

pub struct MasterFxChain {
    sample_rate: f32,
    eq_coeffs: [BiquadCoeffs; 4],
    eq_l: [BiquadState; 4],
    eq_r: [BiquadState; 4],
    eq_active: bool,
    comp: Compressor,
    doubler: Doubler,
    deesser: DeEsser,
    transient: TransientShaper,
    sat: Saturator,
    chorus: ModDelay,
    flanger: ModDelay,
    phaser: Phaser,
    multiband: MultibandCompressor,
    convolver: Convolver,
    gate: NoiseGate,
    granular: Granular,
    freq_shift: FrequencyShifter,
    crusher: BitCrusher,
    mb_delay: MultibandDelay,
    delay_bank: DelayBank,
    distructor: Distructor,
    hardcore: Hardcore,
    effector: Effector,
    filter: MasterFilter,
    limiter: Limiter,
    params: MasterFxParams,
}

impl MasterFxChain {
    pub fn new(sample_rate: f32) -> Self {
        let mut chain = Self {
            sample_rate,
            eq_coeffs: [BiquadCoeffs::bypass(); 4],
            eq_l: [BiquadState::default(); 4],
            eq_r: [BiquadState::default(); 4],
            eq_active: false,
            comp: Compressor::new(sample_rate),
            doubler: Doubler::new(sample_rate),
            deesser: DeEsser::new(sample_rate),
            transient: TransientShaper::new(sample_rate),
            sat: Saturator::new(sample_rate),
            chorus: ModDelay::new(sample_rate, 12.0, 8.0),
            flanger: ModDelay::new(sample_rate, 2.5, 2.0),
            phaser: Phaser::new(sample_rate),
            multiband: MultibandCompressor::new(sample_rate),
            convolver: Convolver::new(sample_rate),
            gate: NoiseGate::new(sample_rate),
            granular: Granular::new(sample_rate),
            freq_shift: FrequencyShifter::new(sample_rate),
            crusher: BitCrusher::new(sample_rate),
            mb_delay: MultibandDelay::new(sample_rate),
            delay_bank: DelayBank::new(sample_rate),
            distructor: Distructor::new(sample_rate),
            hardcore: Hardcore::new(sample_rate),
            effector: Effector::new(sample_rate),
            filter: MasterFilter::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            params: MasterFxParams::default(),
        };
        chain.set_params(chain.params);
        chain
    }

    pub fn set_params(&mut self, params: MasterFxParams) {
        self.params = params;
        let mut any = false;
        for (i, band) in params.eq_bands.iter().enumerate() {
            if band.active && band.gain_db.abs() > 0.001 {
                self.eq_coeffs[i] = peaking(self.sample_rate, band.freq, band.gain_db, band.q);
                any = true;
            } else {
                self.eq_coeffs[i] = BiquadCoeffs::bypass();
            }
        }
        self.eq_active = params.eq_enabled && any;
        self.multiband.set_params(params.multiband);
    }

    /// Laddar (eller rensar) convolverns impulsrespons. Responsen är för stor för att
    /// följa med `MasterFxParams` (som är `Copy`), så den kommer som ett eget kommando.
    pub fn set_convolver_ir(&mut self, ir: Option<Arc<Vec<f32>>>) {
        self.convolver.set_ir(ir);
    }

    /// Current compressor gain reduction in dB (<= 0.0). Read by the UI meter.
    pub fn gain_reduction_db(&self) -> f32 {
        if self.params.comp_enabled { self.comp.gain_reduction_db } else { 0.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let p = self.params;
        let (mut l, mut r) = (l, r);

        if p.gate_enabled {
            let (nl, nr) = self.gate.process(l, r, &p.gate);
            l = nl;
            r = nr;
        }

        if p.granular_enabled {
            let (nl, nr) = self.granular.process(l, r, &p.granular);
            l = nl;
            r = nr;
        }

        // Anslaget formas före kompressorn: annars hade kompressorn ätit upp den vinst
        // transientreglaget gjorde, och reglaget hade verkat svagare ju hårdare man komprimerar.
        if p.transient_enabled {
            let (nl, nr) = self.transient.process(l, r, &p.transient);
            l = nl;
            r = nr;
        }

        if self.eq_active {
            for i in 0..4 {
                l = self.eq_l[i].process(l, &self.eq_coeffs[i]);
                r = self.eq_r[i].process(r, &self.eq_coeffs[i]);
            }
        }

        if p.comp_enabled {
            let (nl, nr) = self.comp.process(l, r, &p.comp);
            l = nl;
            r = nr;
        }

        if p.deesser_enabled {
            let (nl, nr) = self.deesser.process(l, r, &p.deesser);
            l = nl;
            r = nr;
        }

        if p.filter.enabled {
            let (nl, nr) = self.filter.process(l, r, &p.filter);
            l = nl;
            r = nr;
        }

        // Sist före limitern: en mättnad som ligger efter allt annat färgar hela kedjan,
        // och limitern tar hand om den extra toppen den kan ge.
        if p.multiband_enabled {
            let (nl, nr) = self.multiband.process(l, r);
            l = nl;
            r = nr;
        }

        if p.convolver_enabled {
            let (nl, nr) = self.convolver.process(l, r, &p.convolver);
            l = nl;
            r = nr;
        }

        if p.distructor_enabled {
            let (nl, nr) = self.distructor.process(l, r, &p.distructor);
            l = nl;
            r = nr;
        }

        if p.hardcore_enabled {
            let (nl, nr) = self.hardcore.process(l, r, &p.hardcore);
            l = nl;
            r = nr;
        }

        if p.effector_enabled {
            let (nl, nr) = self.effector.process(l, r, &p.effector);
            l = nl;
            r = nr;
        }

        if p.sat_enabled {
            let (nl, nr) = self.sat.process(l, r, &p.sat);
            l = nl;
            r = nr;
        }

        if p.crush_enabled {
            let (nl, nr) = self.crusher.process(l, r, &p.crush);
            l = nl;
            r = nr;
        }

        // Modulation efter mättnaden: då sveper kammen i den färgade signalen,
        // och limitern sist tar hand om toppen återkopplingen kan ge.
        if p.chorus_enabled {
            let (nl, nr) = self.chorus.process(l, r, &p.chorus);
            l = nl;
            r = nr;
        }

        if p.flanger_enabled {
            let (nl, nr) = self.flanger.process(l, r, &p.flanger);
            l = nl;
            r = nr;
        }

        if p.phaser_enabled {
            let (nl, nr) = self.phaser.process(l, r, &p.phaser);
            l = nl;
            r = nr;
        }

        if p.freq_shift_enabled {
            let (nl, nr) = self.freq_shift.process(l, r, &p.freq_shift);
            l = nl;
            r = nr;
        }

        if p.mb_delay_enabled {
            let (nl, nr) = self.mb_delay.process(l, r, &p.mb_delay);
            l = nl;
            r = nr;
        }

        if p.delay_bank_enabled {
            let (nl, nr) = self.delay_bank.process(l, r, &p.delay_bank);
            l = nl;
            r = nr;
        }

        if p.doubler_enabled {
            let (nl, nr) = self.doubler.process(l, r, &p.doubler);
            l = nl;
            r = nr;
        }

        if p.limiter_enabled {
            let (nl, nr) = self.limiter.process(l, r, &p.limiter);
            l = nl;
            r = nr;
        }

        // Control Room ligger sist: monokontroll och dämpning ska gälla det som faktiskt
        // lämnar mastern, inte påverka något steg före sig.
        let c = p.control;
        if c.mono {
            let m = 0.5 * (l + r);
            l = m;
            r = m;
        }
        let mut gain = 10.0_f32.powf(c.level_db.clamp(-60.0, 12.0) / 20.0);
        if c.dim {
            gain *= 10.0_f32.powf(c.dim_db.clamp(-60.0, 0.0) / 20.0);
        }
        (l * gain, r * gain)
    }
}

mod color;
mod dynamics;
mod eq;
mod modulation;

pub use color::*;
pub use dynamics::*;
pub use eq::*;
pub use modulation::*;

#[cfg(test)]
mod tests;
