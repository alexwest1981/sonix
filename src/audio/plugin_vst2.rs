//! Minimal in-process **VST2** host (Fas 4.6c).
//!
//! This is the third real plugin backend in Sonix. Like the CLAP and VST3
//! hosts it speaks the vendor C ABI directly (no Steinberg SDK, no heavyweight
//! crate) and `dlopen`s the module through `libloading`. It is compiled only
//! with the `plugin-host` feature.
//!
//! Scope of Fas 4.6c: load a VST 2.4 shared object, read its `AEffect`, call
//! `VSTPluginMain`, and implement the `dispatcher` opcodes needed to describe
//! and run an effect: descriptor, parameters, sample-rate/block-size setup,
//! `processReplacing`, program chunks for state and `initialDelay` for PDC.
//! That is exactly what makes a yabridge-produced VST2 bridge (`~/.vst/*.so`)
//! *loadable and playable* in Sonix.
//!
//! ABI notes: `AEffect` is declared in the exact field order of `aeffect.h`
//! (192 bytes on 64-bit), the magic is `'VstP'` and all dispatch values are
//! `intptr_t` (`isize`). The plugin receives a host `audioMasterCallback`
//! whose sample-rate/block-size answers come from a small registry keyed by
//! the effect pointer.

#![allow(dead_code)]

use crate::audio::plugin_host_live::{
    PARAM_IS_AUTOMATABLE, PARAM_IS_STEPPED, PluginInfo, PluginInstance, PluginInspection,
    PluginParameter, PluginProcessor,
};
use libloading::Library;
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// ------------------------------------------------------------------ constants

/// `kEffectMagic` from `aeffect.h` (`'VstP'`).
const K_EFFECT_MAGIC: i32 = 0x56737450;
/// `kVstVersion` advertised to the plugin via `audioMasterVersion`.
const K_VST_VERSION: i32 = 2400;
/// `VstProcessPrecision::kVstProcessPrecision32`.
const K_PROCESS_PRECISION_32: isize = 0;

// Base opcodes (`aeffect.h`).
const EFF_OPEN: i32 = 0;
const EFF_CLOSE: i32 = 1;
const EFF_GET_PARAM_LABEL: i32 = 6;
const EFF_GET_PARAM_NAME: i32 = 8;
const EFF_SET_SAMPLE_RATE: i32 = 10;
const EFF_SET_BLOCK_SIZE: i32 = 11;
const EFF_MAINS_CHANGED: i32 = 12;
const EFF_GET_CHUNK: i32 = 23;
const EFF_SET_CHUNK: i32 = 24;
// Extended opcodes (`aeffectx.h`).
const EFF_GET_PLUG_CATEGORY: i32 = 35;
const EFF_GET_EFFECT_NAME: i32 = 45;
const EFF_GET_VENDOR_STRING: i32 = 47;
const EFF_GET_PRODUCT_STRING: i32 = 48;
const EFF_GET_VENDOR_VERSION: i32 = 49;
const EFF_GET_PARAMETER_PROPERTIES: i32 = 56;
const EFF_SET_PROCESS_PRECISION: i32 = 77;

// `AEffect` flags.
const EFF_FLAGS_IS_SYNTH: i32 = 256;
const EFF_FLAGS_CAN_REPLACING: i32 = 16;
const EFF_FLAGS_PROGRAM_CHUNKS: i32 = 32;

// `VstPlugCategory`.
const K_PLUG_CATEG_SYNTH: i32 = 2;
const K_PLUG_CATEG_ANALYSIS: i32 = 3;
const K_PLUG_CATEG_MASTERING: i32 = 4;
const K_PLUG_CATEG_SPATIAL: i32 = 5;
const K_PLUG_CATEG_RESTORATION: i32 = 6;
const K_PLUG_CATEG_SURROUND: i32 = 7;

// `VstParameterProperties::flags`.
const KVST_PARAM_IS_SWITCH: i32 = 1;

// `audioMaster` opcodes (`aeffect.h` + `aeffectx.h`).
const AM_VERSION: i32 = 1;
const AM_IDLE: i32 = 3;
const AM_GET_SAMPLE_RATE: i32 = 16;
const AM_GET_BLOCK_SIZE: i32 = 17;
const AM_GET_CURRENT_PROCESS_LEVEL: i32 = 23;
const AM_GET_VENDOR_STRING: i32 = 32;
const AM_GET_PRODUCT_STRING: i32 = 33;
const AM_GET_VENDOR_VERSION: i32 = 34;
const AM_GET_LANGUAGE: i32 = 38;

// ---------------------------------------------------------------------- types

#[repr(C)]
struct AEffect {
    magic: i32,
    dispatcher: Option<DispatcherProc>,
    process: Option<ProcessProc>,
    set_parameter: Option<SetParameterProc>,
    get_parameter: Option<GetParameterProc>,
    num_programs: i32,
    num_params: i32,
    num_inputs: i32,
    num_outputs: i32,
    flags: i32,
    resvd1: isize,
    resvd2: isize,
    initial_delay: i32,
    real_qualities: i32,
    off_qualities: i32,
    io_ratio: f32,
    object: *mut c_void,
    user: *mut c_void,
    unique_id: i32,
    version: i32,
    process_replacing: Option<ProcessProc>,
    process_double_replacing: Option<ProcessDoubleProc>,
    future: [c_char; 56],
}

type AudioMasterCallback =
    unsafe extern "C" fn(*mut AEffect, i32, i32, isize, *mut c_void, f32) -> isize;
type DispatcherProc =
    unsafe extern "C" fn(*mut AEffect, i32, i32, isize, *mut c_void, f32) -> isize;
type ProcessProc = unsafe extern "C" fn(*mut AEffect, *mut *mut f32, *mut *mut f32, i32);
type ProcessDoubleProc = unsafe extern "C" fn(*mut AEffect, *mut *mut f64, *mut *mut f64, i32);
type SetParameterProc = unsafe extern "C" fn(*mut AEffect, i32, f32);
type GetParameterProc = unsafe extern "C" fn(*mut AEffect, i32) -> f32;
type VstPluginMain = unsafe extern "C" fn(AudioMasterCallback) -> *mut AEffect;

/// Layout must match `aeffectx.h`'s `VstParameterProperties`.
#[repr(C)]
struct VstParameterProperties {
    step_float: f32,
    small_step_float: f32,
    large_step_float: f32,
    label: [c_char; 64],
    flags: i32,
    min_integer: i32,
    max_integer: i32,
    step_integer: i32,
    large_step_integer: i32,
    short_label: [c_char; 8],
    display_index: i16,
    category: i16,
    num_parameters_in_category: i16,
    reserved: i16,
    category_label: [c_char; 24],
    future: [c_char; 16],
}

// -------------------------------------------------------------- host callback

/// Values a plugin may query through `audioMaster` while it is alive.
struct HostContext {
    sample_rate: f32,
    block_size: i32,
}

static HOST_CONTEXTS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<usize, usize>> {
    HOST_CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_context(effect: *mut AEffect, ctx: &HostContext) {
    if let Ok(mut map) = registry().lock() {
        map.insert(effect as usize, ctx as *const HostContext as usize);
    }
}

fn unregister_context(effect: *mut AEffect) {
    if let Ok(mut map) = registry().lock() {
        map.remove(&(effect as usize));
    }
}

fn with_context<R>(effect: *mut AEffect, f: impl FnOnce(&HostContext) -> R, fallback: R) -> R {
    if effect.is_null() {
        return fallback;
    }
    let addr = match registry().lock() {
        Ok(map) => map.get(&(effect as usize)).copied(),
        Err(_) => None,
    };
    match addr {
        Some(addr) => f(unsafe { &*(addr as *const HostContext) }),
        None => fallback,
    }
}

unsafe fn write_c_string(ptr: *mut c_void, s: &str) {
    if ptr.is_null() {
        return;
    }
    let bytes = s.as_bytes();
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
        *(ptr as *mut u8).add(bytes.len()) = 0;
    }
}

/// The host-side `audioMasterCallback`. Never panics: unknown opcodes answer 0.
unsafe extern "C" fn audio_master(
    effect: *mut AEffect,
    opcode: i32,
    _index: i32,
    _value: isize,
    ptr: *mut c_void,
    _opt: f32,
) -> isize {
    match opcode {
        AM_VERSION => K_VST_VERSION as isize,
        AM_IDLE => 0,
        AM_GET_SAMPLE_RATE => with_context(effect, |c| c.sample_rate as isize, 0),
        AM_GET_BLOCK_SIZE => with_context(effect, |c| c.block_size as isize, 0),
        // kVstProcessLevelRealtime.
        AM_GET_CURRENT_PROCESS_LEVEL => 2,
        // kVstLangEnglish.
        AM_GET_LANGUAGE => 1,
        AM_GET_VENDOR_STRING | AM_GET_PRODUCT_STRING => {
            unsafe { write_c_string(ptr, "Sonix") };
            1
        }
        AM_GET_VENDOR_VERSION => 1,
        _ => 0,
    }
}

// ------------------------------------------------------------------- helpers

unsafe fn dispatch(
    effect: *mut AEffect,
    opcode: i32,
    index: i32,
    value: isize,
    ptr: *mut c_void,
    opt: f32,
) -> isize {
    if effect.is_null() {
        return 0;
    }
    match unsafe { (*effect).dispatcher } {
        Some(f) => unsafe { f(effect, opcode, index, value, ptr, opt) },
        None => 0,
    }
}

fn fixed_cstr(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

unsafe fn read_string(effect: *mut AEffect, opcode: i32, len: usize) -> String {
    let mut buf = vec![0 as c_char; len.max(1)];
    unsafe { dispatch(effect, opcode, 0, 0, buf.as_mut_ptr() as *mut c_void, 0.0) };
    fixed_cstr(&buf)
}

fn category_name(category: i32) -> &'static str {
    match category {
        K_PLUG_CATEG_SYNTH => "instrument",
        K_PLUG_CATEG_ANALYSIS => "analyzer",
        K_PLUG_CATEG_MASTERING => "mastering",
        K_PLUG_CATEG_SPATIAL => "spatial",
        K_PLUG_CATEG_RESTORATION => "restoration",
        K_PLUG_CATEG_SURROUND => "surround",
        _ => "effect",
    }
}

unsafe fn gather_info(effect: *mut AEffect) -> PluginInfo {
    let name = unsafe { read_string(effect, EFF_GET_EFFECT_NAME, 256) };
    let vendor = unsafe { read_string(effect, EFF_GET_VENDOR_STRING, 64) };
    let product = unsafe { read_string(effect, EFF_GET_PRODUCT_STRING, 64) };
    let version = unsafe { dispatch(effect, EFF_GET_VENDOR_VERSION, 0, 0, std::ptr::null_mut(), 0.0) };
    let unique_id = unsafe { (*effect).unique_id };
    let flags = unsafe { (*effect).flags };
    let category =
        unsafe { dispatch(effect, EFF_GET_PLUG_CATEGORY, 0, 0, std::ptr::null_mut(), 0.0) } as i32;

    let mut features = vec![category_name(category).to_string()];
    if flags & EFF_FLAGS_IS_SYNTH != 0 {
        features.push("instrument".to_string());
    }

    PluginInfo {
        id: format!("{unique_id:08X}"),
        name,
        vendor,
        version: version.to_string(),
        description: product,
        features,
    }
}

unsafe fn gather_parameters(effect: *mut AEffect) -> Vec<PluginParameter> {
    let mut out = Vec::new();
    let count = unsafe { (*effect).num_params }.max(0);
    for index in 0..count {
        let mut name = vec![0 as c_char; 64];
        unsafe {
            dispatch(
                effect,
                EFF_GET_PARAM_NAME,
                index,
                0,
                name.as_mut_ptr() as *mut c_void,
                0.0,
            )
        };
        let mut label = vec![0 as c_char; 64];
        unsafe {
            dispatch(
                effect,
                EFF_GET_PARAM_LABEL,
                index,
                0,
                label.as_mut_ptr() as *mut c_void,
                0.0,
            )
        };
        let mut props: VstParameterProperties = unsafe { std::mem::zeroed() };
        let has_props = unsafe {
            dispatch(
                effect,
                EFF_GET_PARAMETER_PROPERTIES,
                index,
                0,
                &mut props as *mut VstParameterProperties as *mut c_void,
                0.0,
            )
        } != 0;
        let default = unsafe {
            match (*effect).get_parameter {
                Some(get) => get(effect, index) as f64,
                None => 0.0,
            }
        };

        let mut flags = PARAM_IS_AUTOMATABLE;
        if has_props && props.flags & KVST_PARAM_IS_SWITCH != 0 {
            flags |= PARAM_IS_STEPPED;
        }

        out.push(PluginParameter {
            id: index as u32,
            name: fixed_cstr(&name),
            module: fixed_cstr(&label),
            min_value: 0.0,
            max_value: 1.0,
            default_value: default,
            flags,
        });
    }
    out
}

/// True when `path` looks like a native VST2 shared object (`.so`).
pub fn is_vst2_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("so"))
        .unwrap_or(false)
}

// ------------------------------------------------------------------- instance

/// A loaded VST2 plugin instance (inspection + ownership of the module).
pub struct Vst2Instance {
    effect: *mut AEffect,
    info: PluginInfo,
    params: Vec<PluginParameter>,
    /// Registered as the `audioMaster` context; dropped after `effClose`.
    host: Box<HostContext>,
    /// Declared last so the library outlives the effect.
    _lib: Library,
}

impl Drop for Vst2Instance {
    fn drop(&mut self) {
        unsafe { dispatch(self.effect, EFF_CLOSE, 0, 0, std::ptr::null_mut(), 0.0) };
        unregister_context(self.effect);
    }
}

impl PluginInstance for Vst2Instance {
    fn backend(&self) -> &'static str {
        "VST2"
    }
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &self.params
    }
}

/// Loads the first plugin exported by a VST2 shared object.
pub fn load(path: &str) -> Result<Box<dyn PluginInstance>, String> {
    Ok(Box::new(open(path)?))
}

fn open(path: &str) -> Result<Vst2Instance, String> {
    let so = PathBuf::from(path);
    if !so.exists() {
        return Err(crate::tstatus!("Filen finns inte: {}", so.display()));
    }

    let lib = unsafe { Library::new(&so) }.map_err(|e| {
        crate::tstatus!("Kunde inte öppna '{}': {}", so.display(), e.to_string())
    })?;

    let symbol = unsafe {
        lib.get::<*mut c_void>(b"VSTPluginMain\0")
            .or_else(|_| lib.get::<*mut c_void>(b"main\0"))
    };
    let main: VstPluginMain = match symbol {
        Ok(sym) => unsafe { std::mem::transmute(*sym) },
        Err(e) => {
            return Err(crate::tstatus!(
                "'{}' exporterar ingen 'VSTPluginMain': {}",
                so.display(),
                e.to_string()
            ));
        }
    };

    let effect = unsafe { main(audio_master) };
    if effect.is_null() {
        return Err(crate::tstatus!("'{}' returnerade en null-effect", so.display()));
    }
    let magic = unsafe { (*effect).magic };
    if magic != K_EFFECT_MAGIC {
        return Err(crate::tstatus!(
            "'{}' är inte en giltig VST2-plugin (fel magi 0x{:08X})",
            so.display(),
            magic as u32
        ));
    }

    let host = Box::new(HostContext {
        sample_rate: 44100.0,
        block_size: 0,
    });
    register_context(effect, &host);

    unsafe { dispatch(effect, EFF_OPEN, 0, 0, std::ptr::null_mut(), 0.0) };

    let info = unsafe { gather_info(effect) };
    let params = unsafe { gather_parameters(effect) };

    Ok(Vst2Instance {
        effect,
        info,
        params,
        host,
        _lib: lib,
    })
}

// ------------------------------------------------------------------ processor

/// A live VST2 processor. Implements the backend-agnostic [`PluginProcessor`]
/// trait so the engine can drive it like any other insert.
pub struct Vst2Processor {
    instance: Vst2Instance,
    max_frames: usize,
    latency: u32,
    has_replacing: bool,
    in_bufs: Vec<Vec<f32>>,
    out_bufs: Vec<Vec<f32>>,
    in_ptrs: Vec<*mut f32>,
    out_ptrs: Vec<*mut f32>,
}

// The audio thread owns the processor; the raw `AEffect`/buffer pointers are
// only ever touched there.
unsafe impl Send for Vst2Processor {}

impl Drop for Vst2Processor {
    fn drop(&mut self) {
        let effect = self.instance.effect;
        // Suspend before the instance's `Drop` calls `effClose`.
        unsafe { dispatch(effect, EFF_MAINS_CHANGED, 0, 0, std::ptr::null_mut(), 0.0) };
    }
}

impl Vst2Processor {
    fn effect(&self) -> *mut AEffect {
        self.instance.effect
    }
}

impl PluginProcessor for Vst2Processor {
    fn backend(&self) -> &'static str {
        "VST2"
    }
    fn info(&self) -> &PluginInfo {
        &self.instance.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &self.instance.params
    }
    fn latency_frames(&self) -> u32 {
        self.latency
    }
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len()).min(self.max_frames);
        if frames == 0 {
            return;
        }

        if !self.in_bufs.is_empty() {
            self.in_bufs[0][..frames].copy_from_slice(&left[..frames]);
        }
        if self.in_bufs.len() > 1 {
            self.in_bufs[1][..frames].copy_from_slice(&right[..frames]);
        }
        for buf in self.in_bufs.iter_mut().skip(2) {
            buf[..frames].fill(0.0);
        }
        for buf in self.out_bufs.iter_mut() {
            buf[..frames].fill(0.0);
        }

        let inputs = if self.in_ptrs.is_empty() {
            std::ptr::null_mut()
        } else {
            self.in_ptrs.as_mut_ptr()
        };
        let outputs = self.out_ptrs.as_mut_ptr();
        let effect = self.effect();

        unsafe {
            if self.has_replacing {
                if let Some(process) = (*effect).process_replacing {
                    process(effect, inputs, outputs, frames as i32);
                }
            } else if let Some(process) = (*effect).process {
                process(effect, inputs, outputs, frames as i32);
            }
        }

        left[..frames].copy_from_slice(&self.out_bufs[0][..frames]);
        let right_src = if self.out_bufs.len() > 1 { 1 } else { 0 };
        right[..frames].copy_from_slice(&self.out_bufs[right_src][..frames]);
    }
    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        let effect = self.effect();
        if id as i32 >= unsafe { (*effect).num_params } {
            return false;
        }
        match unsafe { (*effect).set_parameter } {
            Some(set) => {
                unsafe { set(effect, id as i32, value as f32) };
                true
            }
            None => false,
        }
    }
    fn reset(&mut self) {
        let effect = self.effect();
        unsafe {
            dispatch(effect, EFF_MAINS_CHANGED, 0, 0, std::ptr::null_mut(), 0.0);
            dispatch(effect, EFF_MAINS_CHANGED, 0, 1, std::ptr::null_mut(), 0.0);
        }
    }
    fn save_state(&mut self) -> Vec<u8> {
        let effect = self.effect();
        if unsafe { (*effect).flags } & EFF_FLAGS_PROGRAM_CHUNKS != 0 {
            let mut chunk: *mut c_void = std::ptr::null_mut();
            let size = unsafe {
                dispatch(
                    effect,
                    EFF_GET_CHUNK,
                    1,
                    0,
                    &mut chunk as *mut *mut c_void as *mut c_void,
                    0.0,
                )
            };
            if size > 0 && !chunk.is_null() {
                let bytes = unsafe {
                    std::slice::from_raw_parts(chunk as *const u8, size as usize).to_vec()
                };
                // VST2 convention: the host frees the returned chunk.
                unsafe { libc::free(chunk) };
                return bytes;
            }
            return Vec::new();
        }

        // Fallback for plugins without program chunks: tag + f32 parameters.
        let count = unsafe { (*effect).num_params }.max(0);
        let mut out = Vec::with_capacity(8 + count as usize * 4);
        out.extend_from_slice(b"V2P1");
        out.extend_from_slice(&(count as u32).to_le_bytes());
        for index in 0..count {
            let value = unsafe {
                match (*effect).get_parameter {
                    Some(get) => get(effect, index),
                    None => 0.0,
                }
            };
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        let effect = self.effect();
        if unsafe { (*effect).flags } & EFF_FLAGS_PROGRAM_CHUNKS != 0 {
            let ok = unsafe {
                dispatch(
                    effect,
                    EFF_SET_CHUNK,
                    1,
                    data.len() as isize,
                    data.as_ptr() as *mut c_void,
                    0.0,
                )
            };
            return ok != 0;
        }

        if data.len() < 8 || &data[..4] != b"V2P1" {
            return false;
        }
        let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        if data.len() < 8 + count * 4 {
            return false;
        }
        for index in 0..count {
            let off = 8 + index * 4;
            let value = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            unsafe {
                if let Some(set) = (*effect).set_parameter {
                    set(effect, index as i32, value);
                }
            }
        }
        true
    }
}

/// Loads `path` and returns a live, activated VST2 processor ready to render
/// blocks of up to `max_frames` at `sample_rate`.
pub fn load_processor(
    path: &str,
    sample_rate: f32,
    max_frames: u32,
) -> Result<Box<dyn PluginProcessor>, String> {
    let mut instance = open(path)?;
    let max_frames = max_frames.max(1) as usize;
    let sample_rate = sample_rate.max(1.0);
    let effect = instance.effect;

    let can_process = unsafe {
        (*effect).process_replacing.is_some() || (*effect).process.is_some()
    };
    if !can_process {
        return Err(crate::tstatus!(
            "VST2-pluginen '{}' saknar processReplacing/process",
            path
        ));
    }
    let has_replacing = unsafe { (*effect).flags } & EFF_FLAGS_CAN_REPLACING != 0;

    let num_in = unsafe { (*effect).num_inputs }.clamp(0, 8) as usize;
    let num_out = unsafe { (*effect).num_outputs }.clamp(1, 8) as usize;

    unsafe {
        dispatch(
            effect,
            EFF_SET_SAMPLE_RATE,
            0,
            0,
            std::ptr::null_mut(),
            sample_rate,
        );
        dispatch(
            effect,
            EFF_SET_BLOCK_SIZE,
            0,
            max_frames as isize,
            std::ptr::null_mut(),
            0.0,
        );
        dispatch(
            effect,
            EFF_SET_PROCESS_PRECISION,
            0,
            K_PROCESS_PRECISION_32,
            std::ptr::null_mut(),
            0.0,
        );
        dispatch(
            effect,
            EFF_MAINS_CHANGED,
            0,
            1,
            std::ptr::null_mut(),
            0.0,
        );
    }
    instance.host.sample_rate = sample_rate;
    instance.host.block_size = max_frames as i32;

    let latency = unsafe { (*effect).initial_delay }.max(0) as u32;

    let mut in_bufs: Vec<Vec<f32>> = (0..num_in).map(|_| vec![0.0; max_frames]).collect();
    let mut out_bufs: Vec<Vec<f32>> = (0..num_out).map(|_| vec![0.0; max_frames]).collect();
    let in_ptrs = in_bufs.iter_mut().map(|b| b.as_mut_ptr()).collect();
    let out_ptrs = out_bufs.iter_mut().map(|b| b.as_mut_ptr()).collect();

    Ok(Box::new(Vst2Processor {
        instance,
        max_frames,
        latency,
        has_replacing,
        in_bufs,
        out_bufs,
        in_ptrs,
        out_ptrs,
    }))
}

/// Loads a VST2 module and returns an inspection snapshot. Never panics: any
/// failure is reported in `error`.
pub fn inspect(path: &str) -> PluginInspection {
    match load(path) {
        Ok(instance) => PluginInspection {
            path: path.to_string(),
            backend: instance.backend().to_string(),
            info: Some(instance.info().clone()),
            parameters: instance.parameters().to_vec(),
            gui: None,
            error: None,
        },
        Err(error) => PluginInspection {
            path: path.to_string(),
            backend: "VST2".to_string(),
            info: None,
            parameters: Vec::new(),
            gui: None,
            error: Some(error),
        },
    }
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vst2_extension_case_insensitively() {
        assert!(is_vst2_path("/tmp/Plugin.so"));
        assert!(is_vst2_path("/tmp/Plugin.SO"));
        assert!(!is_vst2_path("/tmp/Plugin.clap"));
        assert!(!is_vst2_path("/tmp/Plugin.vst3"));
    }

    #[test]
    fn rejects_non_vst2_library() {
        let Some(empty) = option_env!("SONIX_EMPTY_SO") else {
            return; // no C compiler at build time
        };
        let err = match load(empty) {
            Ok(_) => panic!("loading a non-VST2 library must fail"),
            Err(e) => e,
        };
        assert!(
            err.contains("VSTPluginMain"),
            "expected a VSTPluginMain error, got: {err}"
        );
    }

    #[test]
    fn loads_mock_module_and_reports_parameters() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return; // no C compiler at build time
        };
        let instance = load(mock).expect("mock VST2 module should load");

        assert_eq!(instance.backend(), "VST2");
        let info = instance.info();
        assert_eq!(info.id, "534E5832");
        assert_eq!(info.name, "Sonix Mock VST2");
        assert_eq!(info.vendor, "Sonix Test");
        assert_eq!(info.version, "100");
        assert_eq!(info.description, "Sonix Mock VST2");
        assert!(info.features.iter().any(|f| f == "effect"));

        let params = instance.parameters();
        assert_eq!(params.len(), 2, "mock reports two parameters");
        assert_eq!(params[0].id, 0);
        assert_eq!(params[0].name, "Gain");
        assert_eq!(params[0].default_value, 1.0);
        assert!(params[0].is_automatable());
        assert!(!params[0].is_stepped());
        assert_eq!(params[1].id, 1);
        assert_eq!(params[1].name, "Mix");
        assert!(params[1].is_stepped());
        // Dropping must close the effect and unload cleanly.
        drop(instance);
    }

    #[test]
    fn inspect_snapshot_matches_load() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return;
        };
        let snapshot = inspect(mock);
        assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
        assert_eq!(snapshot.backend, "VST2");
        assert_eq!(snapshot.parameters.len(), 2);
        assert_eq!(snapshot.info.unwrap().name, "Sonix Mock VST2");
    }

    #[test]
    fn missing_file_is_an_honest_error() {
        let err = match load("/nonexistent/thing.so") {
            Ok(_) => panic!("loading a missing file must fail"),
            Err(e) => e,
        };
        assert!(err.contains("finns inte"), "unexpected error: {err}");
    }

    #[test]
    fn loads_processor_and_reports_latency() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return;
        };
        let processor = load_processor(mock, 48_000.0, 512).expect("mock processor");
        assert_eq!(processor.backend(), "VST2");
        assert_eq!(processor.info().name, "Sonix Mock VST2");
        assert_eq!(processor.parameters().len(), 2);
        assert_eq!(processor.latency_frames(), 8, "mock reports 8 frames");
    }

    #[test]
    fn processor_applies_parameters_and_latency() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return;
        };
        let mut processor = load_processor(mock, 48_000.0, 512).expect("mock processor");

        // Unknown ids are rejected.
        assert!(!processor.set_parameter(999, 0.5));

        // Default gain/mix are unity; the mock delays by 8 frames.
        let mut l = vec![0.5_f32; 32];
        let mut r = vec![0.5_f32; 32];
        processor.process_stereo(&mut l, &mut r);
        assert!(l[..8].iter().all(|&s| s == 0.0), "latency must emit silence");
        assert!((l[8] - 0.5).abs() < 1e-6, "unity gain expected, got {}", l[8]);

        // Gain 0.5 with unity mix halves the (delayed) signal.
        assert!(processor.set_parameter(0, 0.5));
        let mut l = vec![1.0_f32; 32];
        let mut r = vec![1.0_f32; 32];
        processor.process_stereo(&mut l, &mut r);
        assert!((l[16] - 0.5).abs() < 1e-5, "expected 0.5, got {}", l[16]);
    }

    #[test]
    fn processor_state_round_trips() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return;
        };
        let mut processor = load_processor(mock, 48_000.0, 512).expect("mock processor");
        assert!(processor.set_parameter(0, 0.25));
        let blob = processor.save_state();
        assert_eq!(blob.len(), 16, "two f64 parameters");

        // A fresh instance restored from the blob must render the saved gain.
        let mut restored = load_processor(mock, 48_000.0, 512).expect("mock processor");
        assert!(restored.load_state(&blob));
        let mut l = vec![1.0_f32; 32];
        let mut r = vec![1.0_f32; 32];
        restored.process_stereo(&mut l, &mut r);
        assert!((l[31] - 0.25).abs() < 1e-5, "expected 0.25, got {}", l[31]);
    }

    #[test]
    fn processor_reset_clears_the_latency_buffer() {
        let Some(mock) = option_env!("SONIX_MOCK_VST2") else {
            return;
        };
        let mut processor = load_processor(mock, 48_000.0, 512).expect("mock processor");
        let mut l = vec![1.0_f32; 16];
        let mut r = vec![1.0_f32; 16];
        processor.process_stereo(&mut l, &mut r);
        processor.reset();
        let mut l = vec![1.0_f32; 16];
        let mut r = vec![1.0_f32; 16];
        processor.process_stereo(&mut l, &mut r);
        assert_eq!(l[0], 0.0, "reset must clear the delay line");
        assert!((l[8] - 1.0).abs() < 1e-6, "expected 1.0, got {}", l[8]);
    }
}
