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
mod curve;
mod dynamics;
mod filters;
mod lowend;
mod modulation;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::audio::plugin_host::PluginCategory;
use crate::audio::plugin_host_live::{
    PluginCore, PluginHandle, PluginInfo, PluginInspection, PluginParameter, PluginProcessor,
    PARAM_IS_AUTOMATABLE, PARAM_IS_STEPPED,
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

mod hybrid_reverb_fx;
pub mod modal_fx;
mod roar_fx;
pub mod wavetable_fx;

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
        &roar_fx::ROAR_FX,
        &wavetable_fx::WAVETABLE_FX,
        &hybrid_reverb_fx::HYBRID_REVERB_FX,
        &filters::FILTER72,
        &filters::TILT,
        &filters::MOOG,
        &filters::CYTOMIC,
        &filters::PHASE_ROTATE,
        &lowend::BASS_MANAGER,
        &lowend::ALIGN,
        &modulation::RING_MOD,
        &modulation::PITCH,
        &dynamics::SPECTRAL_COMP,
        &dynamics::INFLATOR,
        &curve::CURVE,
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
        values.push(Arc::new(AtomicU64::new(p.default.to_bits())));
    }
    let parameters = parameters(def);
    let latency = dsp.latency();
    let core = Arc::new(BuiltinCore {
        info: info(def),
        parameters: parameters.clone(),
        values: values.clone(),
        latency,
    });
    Ok(Box::new(BuiltinProcessor {
        info: info(def),
        parameters,
        dsp,
        values,
        applied: vec![f64::NAN; def.params.len()],
        core,
    }))
}

/// Handtaget som gränssnittet rattar: samma värden som processorn läser, utan lås på
/// ljudtråden. Varje parameter är en `AtomicU64` med f64-bitar — en ratt skriver, nästa block
/// läser och ser att något ändrats.
struct BuiltinCore {
    info: PluginInfo,
    parameters: Vec<PluginParameter>,
    values: Vec<Arc<AtomicU64>>,
    latency: u32,
}

impl PluginCore for BuiltinCore {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn parameters(&self) -> &[PluginParameter] {
        &self.parameters
    }

    fn latency_frames(&self) -> u32 {
        self.latency
    }

    fn save_state(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.values.len() * 8);
        for v in &self.values {
            out.extend_from_slice(&f64::from_bits(v.load(Ordering::Relaxed)).to_le_bytes());
        }
        out
    }

    fn load_state(&self, data: &[u8]) -> bool {
        if data.len() != self.values.len() * 8 {
            return false;
        }
        for (i, v) in self.values.iter().enumerate() {
            let bytes: [u8; 8] = data[i * 8..i * 8 + 8].try_into().unwrap_or([0; 8]);
            v.store(f64::from_le_bytes(bytes).to_bits(), Ordering::Relaxed);
        }
        true
    }

    fn set_parameter(&self, id: u32, value: f64) -> bool {
        let Some(p) = self.parameters.get(id as usize) else {
            return false;
        };
        let v = value.clamp(p.min_value, p.max_value);
        self.values[id as usize].store(v.to_bits(), Ordering::Relaxed);
        true
    }

    fn parameter_value(&self, id: u32) -> Option<f64> {
        let v = self.values.get(id as usize)?;
        Some(f64::from_bits(v.load(Ordering::Relaxed)))
    }

    fn reset(&self) {}

    fn preset_load(&self, _location: &str) -> bool {
        false
    }

    fn gui_is_api_supported(&self, _api: &str, _is_floating: bool) -> bool {
        false
    }
    fn gui_preferred_api(&self) -> Option<(String, bool)> {
        None
    }
    fn gui_get_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn gui_can_resize(&self) -> bool {
        false
    }
    fn gui_is_created(&self) -> bool {
        false
    }
    fn gui_create(&self, _api: &str, _is_floating: bool) -> bool {
        false
    }
    fn gui_set_parent(&self, _x11_window: u64) -> bool {
        false
    }
    fn gui_set_size(&self, _width: u32, _height: u32) -> bool {
        false
    }
    fn gui_show(&self) -> bool {
        false
    }
    fn gui_hide(&self) -> bool {
        false
    }
    fn gui_destroy(&self) {}
}

struct BuiltinProcessor {
    info: PluginInfo,
    parameters: Vec<PluginParameter>,
    dsp: Box<dyn Dsp>,
    values: Vec<Arc<AtomicU64>>,
    /// Senast applicerade värdet per parameter, så blocket bara rör det som ändrats.
    applied: Vec<f64>,
    core: Arc<BuiltinCore>,
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
        for (i, v) in self.values.iter().enumerate() {
            let value = f64::from_bits(v.load(Ordering::Relaxed));
            if value.to_bits() != self.applied[i].to_bits() {
                self.dsp.set(i, value);
                self.applied[i] = value;
            }
        }
        self.dsp.process(left, right);
    }

    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        let Some(p) = self.parameters.get(id as usize) else {
            return false;
        };
        let v = value.clamp(p.min_value, p.max_value);
        self.values[id as usize].store(v.to_bits(), Ordering::Relaxed);
        self.dsp.set(id as usize, v);
        self.applied[id as usize] = v;
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
        // Ratten äger värdet: nollställningen tar tillståndet, inte inställningarna.
        for (i, v) in self.values.iter().enumerate() {
            let value = f64::from_bits(v.load(Ordering::Relaxed));
            self.dsp.set(i, value);
            self.applied[i] = value;
        }
    }

    fn core_handle(&self) -> Option<PluginHandle> {
        Some(PluginHandle::new(self.core.clone()))
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

    /// Gränssnittet rattar genom handtaget; parametrarna ska synas där och tillståndet gå att
    /// spara. Utan handtag hade effekten laddats utan rattar.
    #[test]
    fn the_core_handle_exposes_the_same_parameters() {
        let p = load("builtin:tilt", 48_000.0).expect("tilt");
        let core = p.core_handle().expect("inbyggd effekt ska ha ett handtag");
        assert_eq!(core.parameters().len(), 2);
        assert_eq!(core.info().name, "Tilt");
        assert!(core.set_parameter(1, 6.0));
        assert_eq!(core.parameter_value(1), Some(6.0));
        assert_eq!(core.parameter_value(99), None);
        assert!(!core.set_parameter(99, 0.0));
        // Ingen egen GUI: gränssnittet ska rita våra rattar, inte öppna ett tomt X11-fönster.
        assert!(!core.gui_is_api_supported("x11", false));
        let state = core.save_state();
        assert_eq!(state.len(), 16);
        assert!(core.load_state(&state));
        assert!(!core.load_state(&[0u8; 3]), "fel längd ska avvisas");
    }

    #[test]
    fn an_unknown_builtin_is_an_error_not_silence() {
        assert!(load("builtin:finnsinte", 48_000.0).is_err());
        assert!(inspect("builtin:finnsinte").error.is_some());
    }
}
