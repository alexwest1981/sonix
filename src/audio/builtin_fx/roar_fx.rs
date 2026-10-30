//! **Roar som kedje-effekt** — definitionen ligger här, i `builtin_fx`, för att `ParamDef::step`/
//! `knob` hör till den modulen. Själva DSPn ligger i `master_fx::roar`.

use super::{Dsp, EffectDef, ParamDef};
use crate::audio::master_fx::ROAR_MODES;
use crate::audio::master_fx::{Roar, RoarParams};
use crate::audio::plugin_host::PluginCategory;

pub static ROAR_FX: EffectDef = EffectDef {
    id: "roar",
    name: "Roar (multibandssaturator)",
    category: PluginCategory::Distortion,
    description: "Tre band, sex kurvor, aterkoppling med grind och 1-3 steg - egen kurva, inte Abletons",
    params: &[
        ParamDef::step("Kurva (0=tanh, 1=klipp, 2=vik, 3=sinusvik, 4=bitcrush, 5=wrap)", 0.0, 5.0, 0.0),
        ParamDef::knob("Drive", 0.0, 32.0, 2.0),
        ParamDef::knob("Mix", 0.0, 1.0, 0.0),
        ParamDef::knob("Aterkoppling", 0.0, 0.95, 0.0),
        ParamDef::knob("Grind dBFS", -90.0, -20.0, -60.0),
        ParamDef::step("Steg", 1.0, 3.0, 1.0),
        ParamDef::knob("Delning lag", 40.0, 1_000.0, 200.0),
        ParamDef::knob("Delning hog", 1_000.0, 12_000.0, 2_000.0),
        ParamDef::knob("Lutning", 0.0, 1.0, 0.5),
    ],
    make: |sr| Box::new(RoarFx::new(sr)),
};

pub struct RoarFx {
    roar: Roar,
    sample_rate: f32,
    params: RoarParams,
}

impl RoarFx {
    pub fn new(sample_rate: f32) -> Self {
        Self { roar: Roar::new(sample_rate), sample_rate, params: RoarParams::default() }
    }
}

impl Dsp for RoarFx {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for i in 0..left.len().min(right.len()) {
            let (l, r) = self.roar.process(left[i], right[i], &self.params);
            left[i] = l;
            right[i] = r;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        let v = value as f32;
        match index {
            0 => {
                let top = (ROAR_MODES.len() - 1) as f32;
                for band in &mut self.params.bands { band.mode = v.clamp(0.0, top) as u8; }
            }
            1 => { for band in &mut self.params.bands { band.drive = v.clamp(0.0, 32.0); } }
            2 => { for band in &mut self.params.bands { band.mix = v.clamp(0.0, 1.0); } }
            3 => self.params.feedback = v.clamp(0.0, 0.95),
            4 => self.params.fb_gate_db = v.clamp(-90.0, -20.0),
            5 => self.params.stages = v.clamp(1.0, 3.0) as u8,
            6 => self.params.xover_low = v.clamp(40.0, 1_000.0),
            7 => self.params.xover_high = v.clamp(1_000.0, 12_000.0),
            8 => { for band in &mut self.params.bands { band.tilt = v.clamp(0.0, 1.0); } }
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.roar = Roar::new(self.sample_rate);
    }
}
