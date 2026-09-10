//! Minimal in-process **VST3** host (Fas 4.6a).
//!
//! This is the second real plugin backend in Sonix. Like the CLAP host it
//! speaks the vendor C ABI directly (no Steinberg SDK, no heavyweight crate)
//! and `dlopen`s the module through `libloading`. It is compiled only with the
//! `plugin-host` feature.
//!
//! Scope of Fas 4.6a: load a VST3 module, read its `IPluginFactory`, pick the
//! first `"Audio Module Class"`, instantiate the component and its edit
//! controller, and report the descriptor plus the controller's parameters. This
//! is exactly the information the UI needs for inspection, and it is what makes
//! a yabridge-produced `.vst3` bridge *loadable* (a prerequisite for the
//! Wine/yabridge goal). Audio processing, parameter changes and state are Fas
//! 4.6b.
//!
//! ABI notes: the vtables are declared in the exact order of the VST 3 headers
//! and the interface IDs use the non-COM `INLINE_UID` byte order (each 32-bit
//! word stored big-endian), matching a Linux VST3 build.

#![allow(dead_code)]

use crate::audio::plugin_host_live::{
    PARAM_IS_AUTOMATABLE, PARAM_IS_HIDDEN, PARAM_IS_READONLY, PARAM_IS_STEPPED, PluginInfo,
    PluginInstance, PluginInspection, PluginParameter,
};
use libloading::Library;
use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};

const K_RESULT_OK: i32 = 0;

const VST3_AUDIO_MODULE_CLASS: &str = "Audio Module Class";

/// Non-COM `INLINE_UID`: each 32-bit word is stored big-endian.
const fn tuid(l1: u32, l2: u32, l3: u32, l4: u32) -> [u8; 16] {
    [
        (l1 >> 24) as u8,
        (l1 >> 16) as u8,
        (l1 >> 8) as u8,
        l1 as u8,
        (l2 >> 24) as u8,
        (l2 >> 16) as u8,
        (l2 >> 8) as u8,
        l2 as u8,
        (l3 >> 24) as u8,
        (l3 >> 16) as u8,
        (l3 >> 8) as u8,
        l3 as u8,
        (l4 >> 24) as u8,
        (l4 >> 16) as u8,
        (l4 >> 8) as u8,
        l4 as u8,
    ]
}

const IID_FUNKNOWN: [u8; 16] = tuid(0x0000_0000, 0x0000_0000, 0xC000_0000, 0x0000_0046);
const IID_IPLUGIN_BASE: [u8; 16] = tuid(0x2288_8DDB, 0x156E_45AE, 0x8358_B348, 0x0819_0625);
const IID_ICOMPONENT: [u8; 16] = tuid(0xE831_FF31, 0xF2D5_4301, 0x928E_BBEE, 0x2569_7802);
const IID_IAUDIO_PROCESSOR: [u8; 16] = tuid(0x4204_3F99, 0xB7DA_453C, 0xA569_E79D, 0x9AAE_C33D);
const IID_IEDIT_CONTROLLER: [u8; 16] = tuid(0xDCD7_BBE3, 0x7742_448D, 0xA874_AACC, 0x979C_759E);

// --------------------------------------------------------------- C layouts

#[repr(C)]
#[derive(Clone, Copy)]
struct PFactoryInfo {
    vendor: [c_char; 64],
    url: [c_char; 256],
    email: [c_char; 128],
    flags: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PClassInfo {
    cid: [u8; 16],
    cardinality: i32,
    category: [c_char; 32],
    name: [c_char; 64],
}

#[repr(C)]
struct ParameterInfo {
    id: u32,
    title: [u16; 128],
    short_title: [u16; 128],
    units: [u16; 128],
    step_count: i32,
    default_normalized_value: f64,
    unit_id: i32,
    flags: i32,
}

// ---------------------------------------------------------------- vtables

#[repr(C)]
struct FUnknownVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
}

#[repr(C)]
struct PluginBaseVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    initialize: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    terminate: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
}

#[repr(C)]
struct FactoryVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    get_factory_info: Option<unsafe extern "C" fn(*mut c_void, *mut PFactoryInfo) -> i32>,
    count_classes: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    get_class_info: Option<unsafe extern "C" fn(*mut c_void, i32, *mut PClassInfo) -> i32>,
    create_instance: Option<
        unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *mut *mut c_void) -> i32,
    >,
}

#[repr(C)]
struct ComponentVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    initialize: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    terminate: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    get_controller_class_id: Option<unsafe extern "C" fn(*mut c_void, *mut u8) -> i32>,
    set_io_mode: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    get_bus_count: Option<unsafe extern "C" fn(*mut c_void, i32, i32) -> i32>,
    get_bus_info: Option<unsafe extern "C" fn(*mut c_void, i32, i32, i32, *mut c_void) -> i32>,
    get_routing_info: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32>,
    activate_bus: Option<unsafe extern "C" fn(*mut c_void, i32, i32, i32, u8) -> i32>,
    set_active: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
    set_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    get_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
}

#[repr(C)]
struct ControllerVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    initialize: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    terminate: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    set_component_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    set_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    get_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    get_parameter_count: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    get_parameter_info:
        Option<unsafe extern "C" fn(*mut c_void, i32, *mut ParameterInfo) -> i32>,
    get_param_string_by_value:
        Option<unsafe extern "C" fn(*mut c_void, u32, f64, *mut u16) -> i32>,
    get_param_value_by_string:
        Option<unsafe extern "C" fn(*mut c_void, u32, *mut u16, *mut f64) -> i32>,
    normalized_param_to_plain: Option<unsafe extern "C" fn(*mut c_void, u32, f64) -> f64>,
    plain_param_to_normalized: Option<unsafe extern "C" fn(*mut c_void, u32, f64) -> f64>,
    get_param_normalized: Option<unsafe extern "C" fn(*mut c_void, u32) -> f64>,
    set_param_normalized: Option<unsafe extern "C" fn(*mut c_void, u32, f64) -> i32>,
    set_component_handler: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    create_view: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
}

// ---------------------------------------------------------------- helpers

/// Reads the vtable pointer stored at the start of a COM-style object.
unsafe fn vtable<T>(obj: *mut c_void) -> &'static T {
    unsafe { &**(obj as *const *const T) }
}

unsafe fn release_object(obj: *mut c_void) {
    unsafe {
        let vt = vtable::<FUnknownVtbl>(obj);
        if let Some(release) = vt.release {
            release(obj);
        }
    }
}

unsafe fn terminate_plugin_base(obj: *mut c_void) {
    unsafe {
        let vt = vtable::<PluginBaseVtbl>(obj);
        if let Some(terminate) = vt.terminate {
            terminate(obj);
        }
    }
}

unsafe fn factory_create(
    factory: *mut c_void,
    cid: &[u8; 16],
    iid: &[u8; 16],
) -> Option<*mut c_void> {
    let f = unsafe { vtable::<FactoryVtbl>(factory) };
    let create = f.create_instance?;
    let mut obj: *mut c_void = std::ptr::null_mut();
    let res = unsafe {
        create(
            factory,
            cid.as_ptr() as *const c_char,
            iid.as_ptr() as *const c_char,
            &mut obj,
        )
    };
    if res == K_RESULT_OK && !obj.is_null() {
        Some(obj)
    } else {
        None
    }
}

unsafe fn pick_component_class(factory: *mut c_void) -> Option<PClassInfo> {
    let f = unsafe { vtable::<FactoryVtbl>(factory) };
    let count = f.count_classes?;
    let get = f.get_class_info?;
    let total = unsafe { count(factory) };
    for index in 0..total {
        let mut info: PClassInfo = unsafe { std::mem::zeroed() };
        if unsafe { get(factory, index, &mut info) } == K_RESULT_OK
            && fixed_cstr(&info.category) == VST3_AUDIO_MODULE_CLASS
        {
            return Some(info);
        }
    }
    None
}

unsafe fn gather_parameters(controller: *mut c_void) -> Vec<PluginParameter> {
    let c = unsafe { vtable::<ControllerVtbl>(controller) };
    let (Some(count), Some(get_info)) = (c.get_parameter_count, c.get_parameter_info) else {
        return Vec::new();
    };
    let total = unsafe { count(controller) };
    let mut out = Vec::new();
    for index in 0..total {
        let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
        if unsafe { get_info(controller, index, &mut info) } != K_RESULT_OK {
            continue;
        }
        out.push(PluginParameter {
            id: info.id,
            name: utf16_to_string(&info.title),
            module: String::new(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: info.default_normalized_value,
            flags: map_flags(info.flags, info.step_count),
        });
    }
    out
}

/// VST3 parameter flags are mapped onto the CLAP flag bits the UI already
/// understands, so both backends present a uniform parameter list.
fn map_flags(flags: i32, step_count: i32) -> u32 {
    const VST3_CAN_AUTOMATE: i32 = 1 << 0;
    const VST3_READ_ONLY: i32 = 1 << 1;
    const VST3_HIDDEN: i32 = 1 << 4;
    let mut out = 0u32;
    if flags & VST3_CAN_AUTOMATE != 0 {
        out |= PARAM_IS_AUTOMATABLE;
    }
    if flags & VST3_READ_ONLY != 0 {
        out |= PARAM_IS_READONLY;
    }
    if flags & VST3_HIDDEN != 0 {
        out |= PARAM_IS_HIDDEN;
    }
    if step_count == 1 {
        out |= PARAM_IS_STEPPED;
    }
    out
}

fn fixed_cstr(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn utf16_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn tuid_hex(uid: &[u8; 16]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(32);
    for byte in uid {
        let _ = write!(s, "{byte:02X}");
    }
    s
}

/// True when `path` looks like a VST3 module or bundle (`.vst3`).
pub fn is_vst3_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("vst3"))
        .unwrap_or(false)
}

/// Turns a `.vst3` file or bundle directory into the shared object to load.
fn resolve_vst3_binary(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if p.is_dir() {
        let contents = p.join("Contents");
        let arch = std::env::consts::ARCH;
        let candidates = [
            contents.join(format!("{arch}-linux")),
            contents.join("x86_64-linux"),
            contents.join("aarch64-linux"),
        ];
        for dir in candidates {
            if let Some(so) = find_shared_object(&dir) {
                return Ok(so);
            }
        }
        if let Ok(entries) = std::fs::read_dir(&contents) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate.is_dir()
                    && let Some(so) = find_shared_object(&candidate)
                {
                    return Ok(so);
                }
            }
        }
        Err(crate::tstatus!(
            "VST3-paketet '{}' saknar en .so-binär",
            p.display()
        ))
    } else if p.exists() {
        Ok(p)
    } else {
        Err(crate::tstatus!("Filen finns inte: {}", p.display()))
    }
}

fn find_shared_object(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.extension().and_then(|e| e.to_str()) == Some("so") {
            return Some(candidate);
        }
    }
    None
}

// ------------------------------------------------------------------- module

struct RawHandles {
    factory: *mut c_void,
    component: *mut c_void,
    controller: *mut c_void,
}

impl RawHandles {
    fn new(factory: *mut c_void) -> Self {
        Self {
            factory,
            component: std::ptr::null_mut(),
            controller: std::ptr::null_mut(),
        }
    }

    unsafe fn release_all(&mut self) {
        unsafe {
            if !self.controller.is_null() {
                terminate_plugin_base(self.controller);
                release_object(self.controller);
                self.controller = std::ptr::null_mut();
            }
            if !self.component.is_null() {
                terminate_plugin_base(self.component);
                release_object(self.component);
                self.component = std::ptr::null_mut();
            }
            if !self.factory.is_null() {
                release_object(self.factory);
                self.factory = std::ptr::null_mut();
            }
        }
    }
}

impl Drop for RawHandles {
    fn drop(&mut self) {
        unsafe { self.release_all() };
    }
}

/// A loaded VST3 plugin instance (Fas 4.6a: inspection only).
pub struct VstInstance {
    /// Declared before `_lib` so the objects are released before unload.
    handles: RawHandles,
    exit_dll: Option<unsafe extern "C" fn() -> bool>,
    info: PluginInfo,
    params: Vec<PluginParameter>,
    _lib: Library,
}

impl Drop for VstInstance {
    fn drop(&mut self) {
        unsafe { self.handles.release_all() };
        if let Some(exit) = self.exit_dll {
            unsafe { exit() };
        }
    }
}

impl PluginInstance for VstInstance {
    fn backend(&self) -> &'static str {
        "VST3"
    }
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &self.params
    }
}

/// Loads the first audio class in a VST3 module.
pub fn load(path: &str) -> Result<Box<dyn PluginInstance>, String> {
    Ok(Box::new(open(path)?))
}

fn open(path: &str) -> Result<VstInstance, String> {
    let so = resolve_vst3_binary(path)?;

    let lib = unsafe { Library::new(&so) }.map_err(|e| {
        crate::tstatus!("Kunde inte öppna '{}': {}", so.display(), e.to_string())
    })?;

    let init_dll = unsafe {
        lib.get::<unsafe extern "C" fn() -> bool>(b"InitDll\0")
            .ok()
            .map(|s| *s)
    };
    if let Some(init) = init_dll {
        unsafe { init() };
    }
    let exit_dll = unsafe {
        lib.get::<unsafe extern "C" fn() -> bool>(b"ExitDll\0")
            .ok()
            .map(|s| *s)
    };

    let factory = unsafe {
        let sym = lib.get::<*mut c_void>(b"GetPluginFactory\0").map_err(|e| {
            crate::tstatus!(
                "'{}' exporterar ingen 'GetPluginFactory': {}",
                so.display(),
                e.to_string()
            )
        })?;
        let get_factory: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(*sym);
        get_factory()
    };
    if factory.is_null() {
        return Err(crate::tstatus!(
            "'{}' returnerade en null-factory",
            so.display()
        ));
    }

    let mut handles = RawHandles::new(factory);

    let mut factory_info: PFactoryInfo = unsafe { std::mem::zeroed() };
    if let Some(get_info) = unsafe { vtable::<FactoryVtbl>(factory) }.get_factory_info {
        unsafe { get_info(factory, &mut factory_info) };
    }

    let class_info = unsafe { pick_component_class(factory) }.ok_or_else(|| {
        crate::tstatus!("'{}' exporterar ingen VST3-ljudklass", so.display())
    })?;

    let component = unsafe { factory_create(factory, &class_info.cid, &IID_ICOMPONENT) }
        .ok_or_else(|| {
            crate::tstatus!("Kunde inte skapa VST3-komponenten i '{}'", so.display())
        })?;
    handles.component = component;

    let mut controller_cid = class_info.cid;
    unsafe {
        if let Some(get_cid) = vtable::<ComponentVtbl>(component).get_controller_class_id {
            let mut buf = [0u8; 16];
            if get_cid(component, buf.as_mut_ptr()) == K_RESULT_OK {
                controller_cid = buf;
            }
        }
        if let Some(init) = vtable::<ComponentVtbl>(component).initialize
            && init(component, std::ptr::null_mut()) != K_RESULT_OK
        {
            return Err(crate::tstatus!(
                "Kunde inte initiera VST3-komponenten i '{}'",
                so.display()
            ));
        }
    }

    let controller = unsafe { factory_create(factory, &controller_cid, &IID_IEDIT_CONTROLLER) }
        .ok_or_else(|| {
            crate::tstatus!("Kunde inte skapa VST3-kontrollern i '{}'", so.display())
        })?;
    handles.controller = controller;

    unsafe {
        if let Some(init) = vtable::<ControllerVtbl>(controller).initialize
            && init(controller, std::ptr::null_mut()) != K_RESULT_OK
        {
            return Err(crate::tstatus!(
                "Kunde inte initiera VST3-kontrollern i '{}'",
                so.display()
            ));
        }
    }

    let params = unsafe { gather_parameters(controller) };

    let info = PluginInfo {
        id: tuid_hex(&class_info.cid),
        name: fixed_cstr(&class_info.name),
        vendor: fixed_cstr(&factory_info.vendor),
        version: String::new(),
        description: fixed_cstr(&class_info.category),
        features: Vec::new(),
    };

    Ok(VstInstance {
        handles,
        exit_dll,
        info,
        params,
        _lib: lib,
    })
}

/// Loads a VST3 module and returns an inspection snapshot. Never panics: any
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
            backend: "VST3".to_string(),
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
    fn detects_vst3_extension_case_insensitively() {
        assert!(is_vst3_path("/tmp/Plugin.vst3"));
        assert!(is_vst3_path("/tmp/Plugin.VST3"));
        assert!(!is_vst3_path("/tmp/Plugin.so"));
        assert!(!is_vst3_path("/tmp/Plugin.clap"));
    }

    #[test]
    fn rejects_non_vst3_library() {
        let Some(empty) = option_env!("SONIX_EMPTY_SO") else {
            return; // no C compiler at build time
        };
        let err = match load(empty) {
            Ok(_) => panic!("loading a non-VST3 library must fail"),
            Err(e) => e,
        };
        assert!(
            err.contains("GetPluginFactory"),
            "expected a GetPluginFactory error, got: {err}"
        );
    }

    #[test]
    fn loads_mock_module_and_reports_parameters() {
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
            return; // no C compiler at build time
        };
        let instance = load(mock).expect("mock VST3 module should load");

        assert_eq!(instance.backend(), "VST3");
        let info = instance.info();
        assert_eq!(info.name, "Sonix Mock VST3");
        assert_eq!(info.vendor, "Sonix Test");
        assert_eq!(info.description, "Audio Module Class");
        assert_eq!(info.id.len(), 32);

        let params = instance.parameters();
        assert_eq!(params.len(), 2, "mock reports two parameters");
        assert_eq!(params[0].id, 100);
        assert_eq!(params[0].name, "Gain");
        assert_eq!(params[0].default_value, 1.0);
        assert!(params[0].is_automatable());
        assert!(!params[0].is_stepped());
        assert_eq!(params[1].id, 101);
        assert_eq!(params[1].name, "Mix");
        assert!(params[1].is_stepped());
        // Dropping must terminate + release + unload cleanly.
        drop(instance);
    }

    #[test]
    fn inspect_snapshot_matches_load() {
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
            return;
        };
        let snapshot = inspect(mock);
        assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
        assert_eq!(snapshot.backend, "VST3");
        assert_eq!(snapshot.parameters.len(), 2);
        assert_eq!(snapshot.info.unwrap().name, "Sonix Mock VST3");
    }

    #[test]
    fn missing_file_is_an_honest_error() {
        let err = match load("/nonexistent/thing.vst3") {
            Ok(_) => panic!("loading a missing file must fail"),
            Err(e) => e,
        };
        assert!(err.contains("finns inte"), "unexpected error: {err}");
    }
}
