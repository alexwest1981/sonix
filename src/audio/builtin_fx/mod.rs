//! **Sonix egna effekter.** Inbyggda, skrivna här, utan fil på disk och utan främmande kod.
//!
//! De laddas på samma väg som en plugin (`builtin:<id>`) och blir en `PluginProcessor`, så de
//! hamnar i varje kanals effektkedja med automatable parametrar — en registrering i stället för
//! ett fält per effekt i mastern.
//!
//! Varje effekt är `Dsp` + en `EffectDef` med sina parametrar. Parametrarna är hela
//! gränssnittet: namn, intervall och standardvärde räcker för att ratta den, och `set` säger
//! vilken ratt som vrids.

mod biquad;
mod dynamics;
mod filters;
mod lowend;
mod modulation;

use crate::audio::plugin_host::PluginCategory;
use crate::audio::plugin_host_live::{
    PluginInfo, PluginInspection, PluginParameter, PluginProcessor, PARAM_IS_AUTOMATABLE,
    PARAM_IS_STEPPED,
};

/// Prefixet som skiljer en inbyggd effekt från en fil på disk.
pub const PREFIX: &str = "builtin:";

pub struct ParamDef {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub stepped: bool,
}

impl ParamDef {
    const fn knob(name: &'static str, min: f64, max: f64, default: f64) -> Self {
        Self { name, min, max, default, stepped: false }
    }
    const fn step(name: &'static str, min: f64, max: f64, default: f64) -> Self {
        Self { name, min, max, default, stepped: true }
    }
}

pub struct EffectDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: PluginCategory,
    pub params: &'static [ParamDef],
    pub make: fn(f32) -> Box<dyn Dsp>,
}

/// Det en effekt måste kunna: behandla ett block och ta emot en parameter.
pub trait Dsp: Send {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);
    fn set(&mut self, index: usize, value: f64);
    /// Nollställ tillstånd (delaylinjer, filter, fönster) men behåll parametrarna.
    fn reset(&mut self);
    /// Fördröjning i sampel som effekten själv inför (PDC).
    fn latency(&self) -> u32 {
        0
    }
}

pub fn is_builtin_path(path: &str) -> bool {
    path.starts_with(PREFIX)
}

pub fn id_of(path: &str) -> &str {
    path.strip_prefix(PREFIX).unwrap_or(path)
}

/// Alla inbyggda effekter. Ordningen är den de visas i.
pub fn defs() -> Vec<&'static EffectDef> {
    vec![
        &filters::FILTER72,
        &filters::TILT,
        &filters::MOOG,
        &filters::PHASE_ROTATE,
        &lowend::BASS_MANAGER,
        &modulation::RING_MOD,
        &modulation::PITCH,
        &dynamics::SPECTRAL_COMP,
        &dynamics::INFLATOR,
    ]
}

pub fn find(id: &str) -> Option<&'static EffectDef> {
    defs().into_iter().find(|d| d.id == id)
}

pub fn parameters(def: &EffectDef) -> Vec<PluginParameter> {
    def.params
        .iter()
        .enumerate()
        .map(|(i, p)| PluginParameter {
            id: i as u32,
            name: p.name.to_string(),
            module: def.name.to_string(),
            min_value: p.min,
            max_value: p.max,
            default_value: p.default,
            flags: PARAM_IS_AUTOMATABLE | if p.stepped { PARAM_IS_STEPPED } else { 0 },
        })
        .collect()
}

pub fn info(def: &EffectDef) -> PluginInfo {
    PluginInfo {
        id: format!("{PREFIX}{}", def.id),
        name: def.name.to_string(),
        vendor: "Sonix".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: def.description.to_string(),
        features: vec!["effect".to_string(), "sonix".to_string()],
    }
}

pub fn inspect(path: &str) -> PluginInspection {
    let id = id_of(path);
    match find(id) {
        Some(def) => PluginInspection {
            path: path.to_string(),
            backend: "Sonix".to_string(),
            info: Some(info(def)),
            parameters: parameters(def),
            gui: None,
            error: None,
        },
        None => PluginInspection {
            path: path.to_string(),
            backend: "Sonix".to_string(),
            info: None,
            parameters: Vec::new(),
            gui: None,
            error: Some(format!("okänd inbyggd effekt: {id}")),
        },
    }
}

pub fn load(path: &str, sample_rate: f32) -> Result<Box<dyn PluginProcessor>, String> {
    let id = id_of(path);
    let def = find(id).ok_or_else(|| format!("okänd inbyggd effekt: {id}"))?;
    let mut dsp = (def.make)(sample_rate);
    let mut values = Vec::with_capacity(def.params.len());
    for (i, p) in def.params.iter().enumerate() {
        dsp.set(i, p.default);
        values.push(p.default);
    }
    Ok(Box::new(BuiltinProcessor {
        info: info(def),
        parameters: parameters(def),
        dsp,
        values,
    }))
}

struct BuiltinProcessor {
    info: PluginInfo,
    parameters: Vec<PluginParameter>,
    dsp: Box<dyn Dsp>,
    values: Vec<f64>,
}

impl PluginProcessor for BuiltinProcessor {
    fn backend(&self) -> &'static str {
        "Sonix"
    }

    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn parameters(&self) -> &[PluginParameter] {
        &self.parameters
    }

    fn latency_frames(&self) -> u32 {
        self.dsp.latency()
    }

    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.dsp.process(left, right);
    }

    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        let Some(p) = self.parameters.get(id as usize) else {
            return false;
        };
        let v = value.clamp(p.min_value, p.max_value);
        self.values[id as usize] = v;
        self.dsp.set(id as usize, v);
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
        // Parametrarna ligger kvar i `self.values`; DSP:n behåller dem via `set`-anropen.
        for (i, v) in self.values.clone().into_iter().enumerate() {
            self.dsp.set(i, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Varje registrerad effekt går att ladda, har unika id och svarar på alla sina parametrar.
    #[test]
    fn every_builtin_loads_and_takes_its_parameters() {
        let mut seen = Vec::new();
        for def in defs() {
            assert!(!seen.contains(&def.id), "dubbelt id: {}", def.id);
            seen.push(def.id);
            let mut p = load(&format!("{PREFIX}{}", def.id), 48_000.0)
                .unwrap_or_else(|e| panic!("{}: {e}", def.id));
            assert_eq!(p.parameters().len(), def.params.len());
            for (i, d) in def.params.iter().enumerate() {
                assert!(p.set_parameter(i as u32, d.max), "{}: param {i}", def.id);
                assert!(!p.set_parameter(9999, 0.0));
            }
            let mut l = vec![0.1f32; 128];
            let mut r = vec![0.1f32; 128];
            p.process_stereo(&mut l, &mut r);
            assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()), "{}: NaN", def.id);
            p.reset();
        }
    }

    #[test]
    fn an_unknown_builtin_is_an_error_not_silence() {
        assert!(load("builtin:finnsinte", 48_000.0).is_err());
        assert!(inspect("builtin:finnsinte").error.is_some());
    }
}
