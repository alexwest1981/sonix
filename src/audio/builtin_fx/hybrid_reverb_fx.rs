//! **Hybrid Reverb as a chain effect** — the definition lives here for the same reason
//! as `roar_fx`: `ParamDef` belongs to this module. The DSP is `master_fx::hybrid_reverb`.

use super::{Dsp, EffectDef, ParamDef};
use crate::audio::master_fx::{HybridReverb, HybridReverbParams};
use crate::audio::plugin_host::PluginCategory;

pub static HYBRID_REVERB_FX: EffectDef = EffectDef {
    id: "hybrid_reverb",
    name: "Hybrid Reverb",
    category: PluginCategory::Reverb,
    description: "Diffusion network and synthetic-IR convolution in parallel - size, decay, blend, predelay, damping",
    params: &[
        ParamDef::knob("Size", 0.0, 1.0, 0.5),
        ParamDef::knob("Decay (s)", 0.2, 12.0, 2.0),
        ParamDef::knob("Mix", 0.0, 1.0, 0.35),
        ParamDef::knob("Blend (0=algo, 1=conv)", 0.0, 1.0, 0.35),
        ParamDef::knob("Predelay (ms)", 0.0, 200.0, 20.0),
        ParamDef::knob("Damp Low (Hz)", 20.0, 1_000.0, 20.0),
        ParamDef::knob("Damp High (Hz)", 1_000.0, 20_000.0, 16_000.0),
    ],
    make: |sr| Box::new(HybridReverbFx::new(sr)),
};

pub struct HybridReverbFx {
    reverb: HybridReverb,
    sample_rate: f32,
    params: HybridReverbParams,
}

impl HybridReverbFx {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            reverb: HybridReverb::new(sample_rate),
            sample_rate,
            params: HybridReverbParams::default(),
        }
    }
}

impl Dsp for HybridReverbFx {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for i in 0..left.len().min(right.len()) {
            let (l, r) = self.reverb.process(left[i], right[i], &self.params);
            left[i] = l;
            right[i] = r;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        let v = value as f32;
        let p = &mut self.params;
        match index {
            0 => p.size = v.clamp(0.0, 1.0),
            1 => p.decay = v.clamp(0.2, 12.0),
            2 => p.mix = v.clamp(0.0, 1.0),
            3 => p.blend = v.clamp(0.0, 1.0),
            4 => p.predelay_ms = v.clamp(0.0, 200.0),
            5 => p.damp_low = v.clamp(20.0, 1_000.0),
            6 => p.damp_high = v.clamp(1_000.0, 20_000.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.reverb = HybridReverb::new(self.sample_rate);
    }
}
