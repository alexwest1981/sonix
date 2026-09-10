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
    PluginInstance, PluginInspection, PluginParameter, PluginProcessor,
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
const IID_IBSTREAM: [u8; 16] = tuid(0xC3BF_6EA2, 0x3099_4752, 0x9B6B_F990, 0x1EE3_3E9B);

/// `tresult` values (non-COM; only `kResultOk` means success).
const K_RESULT_FALSE: i32 = 1;
const K_NO_INTERFACE: i32 = -1;

// Sample-size / process-mode enum values used in `ProcessSetup`/`ProcessData`.
const K_SAMPLE32: i32 = 0;
const K_REALTIME: i32 = 0;

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

// ------------------------------------------------------- audio processor ABI

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcessSetup {
    process_mode: i32,
    symbolic_sample_size: i32,
    max_samples_per_block: i32,
    sample_rate: f64,
}

#[repr(C)]
struct AudioBusBuffers {
    num_channels: i32,
    silence_flags: u64,
    /// `Sample32** channelBuffers32` — the union member for 32-bit samples.
    channel_buffers32: *mut *mut f32,
}

#[repr(C)]
struct ProcessData {
    process_mode: i32,
    symbolic_sample_size: i32,
    num_samples: i32,
    num_inputs: i32,
    num_outputs: i32,
    inputs: *mut AudioBusBuffers,
    outputs: *mut AudioBusBuffers,
    input_parameter_changes: *mut c_void,
    output_parameter_changes: *mut c_void,
    input_events: *mut c_void,
    output_events: *mut c_void,
    process_context: *mut c_void,
}

#[repr(C)]
struct AudioProcessorVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    set_bus_arrangements:
        Option<unsafe extern "C" fn(*mut c_void, *mut u64, i32, *mut u64, i32) -> i32>,
    get_bus_arrangement: Option<unsafe extern "C" fn(*mut c_void, i32, i32, *mut u64) -> i32>,
    can_process_sample_size: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    get_latency_samples: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    setup_processing: Option<unsafe extern "C" fn(*mut c_void, *mut ProcessSetup) -> i32>,
    set_processing: Option<unsafe extern "C" fn(*mut c_void, u8) -> i32>,
    process: Option<unsafe extern "C" fn(*mut c_void, *mut ProcessData) -> i32>,
    get_tail_samples: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
}

// --------------------------------------------------------- host IBStream ABI
//
// The plugin reads/writes its opaque state through an `IBStream` that *we*
// implement, exactly like a real VST3 host. A single `StreamState` backs both
// directions: `read` walks `data`, `write` appends to it.

#[repr(C)]
struct IBStreamVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    read: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut i32) -> i32>,
    write: Option<unsafe extern "C" fn(*mut c_void, *const c_void, i32, *mut i32) -> i32>,
    seek: Option<unsafe extern "C" fn(*mut c_void, i64, i32, *mut i64) -> i32>,
    tell: Option<unsafe extern "C" fn(*mut c_void, *mut i64) -> i32>,
}

#[repr(C)]
struct StreamState {
    vtbl: *const IBStreamVtbl,
    data: Vec<u8>,
    pos: usize,
}

// ------------------------------------------------- host IParameterChanges
//
// A minimal `IParameterChanges`/`IParamValueQueue` implementation used to hand
// parameter changes to the plugin during `process` (the standard VST3 realtime
// path). Only the read side is used by plugins; `addPoint` is provided for
// completeness so an output-changes object would also work.

#[repr(C)]
#[derive(Clone, Copy)]
struct ParamPoint {
    sample_offset: i32,
    value: f64,
}

#[repr(C)]
struct ParamValueQueue {
    vtbl: *const ParamValueQueueVtbl,
    id: u32,
    points: Vec<ParamPoint>,
}

#[repr(C)]
struct ParamValueQueueVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    get_parameter_id: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    get_point_count: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    get_point: Option<unsafe extern "C" fn(*mut c_void, i32, *mut i32, *mut f64) -> i32>,
    add_point: Option<unsafe extern "C" fn(*mut c_void, i32, f64, *mut i32) -> i32>,
}

#[repr(C)]
struct ParameterChanges {
    vtbl: *const ParameterChangesVtbl,
    queues: Vec<ParamValueQueue>,
}

#[repr(C)]
struct ParameterChangesVtbl {
    query_interface:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> i32>,
    add_ref: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    release: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    get_parameter_count: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    get_parameter_data: Option<unsafe extern "C" fn(*mut c_void, i32) -> *mut c_void>,
    add_parameter_data:
        Option<unsafe extern "C" fn(*mut c_void, u32, *mut i32) -> *mut c_void>,
}

// ------------------------------------------------------- stream host methods

unsafe extern "C" fn stream_query(
    _self: *mut c_void,
    iid: *const c_char,
    obj: *mut *mut c_void,
) -> i32 {
    if obj.is_null() {
        return K_NO_INTERFACE;
    }
    let matches = unsafe {
        !iid.is_null()
            && std::slice::from_raw_parts(iid as *const u8, 16) == &IID_FUNKNOWN[..]
    };
    if matches {
        unsafe { *obj = _self };
        K_RESULT_OK
    } else {
        unsafe { *obj = std::ptr::null_mut() };
        K_NO_INTERFACE
    }
}

unsafe extern "C" fn stream_add_ref(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn stream_release(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn stream_read(
    this: *mut c_void,
    buffer: *mut c_void,
    num_bytes: i32,
    num_read: *mut i32,
) -> i32 {
    let state = unsafe { &mut *(this as *mut StreamState) };
    let want = num_bytes.max(0) as usize;
    let remaining = state.data.len().saturating_sub(state.pos);
    let n = want.min(remaining);
    if n > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                state.data.as_ptr().add(state.pos),
                buffer as *mut u8,
                n,
            );
        }
        state.pos += n;
    }
    if !num_read.is_null() {
        unsafe { *num_read = n as i32 };
    }
    K_RESULT_OK
}

unsafe extern "C" fn stream_write(
    this: *mut c_void,
    buffer: *const c_void,
    num_bytes: i32,
    num_written: *mut i32,
) -> i32 {
    let state = unsafe { &mut *(this as *mut StreamState) };
    let n = num_bytes.max(0) as usize;
    if n > 0 && !buffer.is_null() {
        let bytes = unsafe { std::slice::from_raw_parts(buffer as *const u8, n) };
        state.data.extend_from_slice(bytes);
        state.pos = state.data.len();
    }
    if !num_written.is_null() {
        unsafe { *num_written = n as i32 };
    }
    K_RESULT_OK
}

unsafe extern "C" fn stream_seek(
    this: *mut c_void,
    pos: i64,
    mode: i32,
    result: *mut i64,
) -> i32 {
    let state = unsafe { &mut *(this as *mut StreamState) };
    let base = match mode {
        0 => 0i64,                 // kIBStreamSeekSet
        1 => state.pos as i64,     // kIBStreamSeekCur
        _ => state.data.len() as i64, // kIBStreamSeekEnd
    };
    let target = (base + pos).max(0) as usize;
    state.pos = target.min(state.data.len());
    if !result.is_null() {
        unsafe { *result = state.pos as i64 };
    }
    K_RESULT_OK
}

unsafe extern "C" fn stream_tell(this: *mut c_void, pos: *mut i64) -> i32 {
    let state = unsafe { &*(this as *const StreamState) };
    if !pos.is_null() {
        unsafe { *pos = state.pos as i64 };
    }
    K_RESULT_OK
}

static STREAM_VTBL: IBStreamVtbl = IBStreamVtbl {
    query_interface: Some(stream_query),
    add_ref: Some(stream_add_ref),
    release: Some(stream_release),
    read: Some(stream_read),
    write: Some(stream_write),
    seek: Some(stream_seek),
    tell: Some(stream_tell),
};

// ------------------------------------------------- parameter changes methods

unsafe extern "C" fn pvq_query(
    this: *mut c_void,
    _iid: *const c_char,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = this };
    }
    K_RESULT_OK
}

unsafe extern "C" fn pvq_add_ref(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn pvq_release(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn pvq_parameter_id(this: *mut c_void) -> u32 {
    unsafe { (*(this as *const ParamValueQueue)).id }
}

unsafe extern "C" fn pvq_point_count(this: *mut c_void) -> i32 {
    unsafe { (*(this as *const ParamValueQueue)).points.len() as i32 }
}

unsafe extern "C" fn pvq_get_point(
    this: *mut c_void,
    index: i32,
    sample_offset: *mut i32,
    value: *mut f64,
) -> i32 {
    let queue = unsafe { &*(this as *const ParamValueQueue) };
    match queue.points.get(index.max(0) as usize) {
        Some(point) => {
            if !sample_offset.is_null() {
                unsafe { *sample_offset = point.sample_offset };
            }
            if !value.is_null() {
                unsafe { *value = point.value };
            }
            K_RESULT_OK
        }
        None => K_RESULT_FALSE,
    }
}

unsafe extern "C" fn pvq_add_point(
    this: *mut c_void,
    sample_offset: i32,
    value: f64,
    index: *mut i32,
) -> i32 {
    let queue = unsafe { &mut *(this as *mut ParamValueQueue) };
    queue.points.push(ParamPoint {
        sample_offset,
        value,
    });
    if !index.is_null() {
        unsafe { *index = (queue.points.len() - 1) as i32 };
    }
    K_RESULT_OK
}

static PARAM_VALUE_QUEUE_VTBL: ParamValueQueueVtbl = ParamValueQueueVtbl {
    query_interface: Some(pvq_query),
    add_ref: Some(pvq_add_ref),
    release: Some(pvq_release),
    get_parameter_id: Some(pvq_parameter_id),
    get_point_count: Some(pvq_point_count),
    get_point: Some(pvq_get_point),
    add_point: Some(pvq_add_point),
};

unsafe extern "C" fn pchanges_query(
    this: *mut c_void,
    _iid: *const c_char,
    obj: *mut *mut c_void,
) -> i32 {
    if !obj.is_null() {
        unsafe { *obj = this };
    }
    K_RESULT_OK
}

unsafe extern "C" fn pchanges_add_ref(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn pchanges_release(_self: *mut c_void) -> u32 {
    1
}

unsafe extern "C" fn pchanges_count(this: *mut c_void) -> i32 {
    unsafe { (*(this as *const ParameterChanges)).queues.len() as i32 }
}

unsafe extern "C" fn pchanges_get(this: *mut c_void, index: i32) -> *mut c_void {
    let changes = unsafe { &mut *(this as *mut ParameterChanges) };
    match changes.queues.get_mut(index.max(0) as usize) {
        Some(queue) => queue as *mut ParamValueQueue as *mut c_void,
        None => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn pchanges_add(
    this: *mut c_void,
    id: u32,
    index: *mut i32,
) -> *mut c_void {
    let changes = unsafe { &mut *(this as *mut ParameterChanges) };
    changes.queues.push(ParamValueQueue {
        vtbl: &PARAM_VALUE_QUEUE_VTBL,
        id,
        points: Vec::new(),
    });
    let position = changes.queues.len() - 1;
    if !index.is_null() {
        unsafe { *index = position as i32 };
    }
    match changes.queues.get_mut(position) {
        Some(queue) => queue as *mut ParamValueQueue as *mut c_void,
        None => std::ptr::null_mut(),
    }
}

static PARAMETER_CHANGES_VTBL: ParameterChangesVtbl = ParameterChangesVtbl {
    query_interface: Some(pchanges_query),
    add_ref: Some(pchanges_add_ref),
    release: Some(pchanges_release),
    get_parameter_count: Some(pchanges_count),
    get_parameter_data: Some(pchanges_get),
    add_parameter_data: Some(pchanges_add),
};

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

// ===========================================================================
// Live audio processing + state (Fas 4.6b)
// ===========================================================================

/// A live VST3 processor. Implements the backend-agnostic [`PluginProcessor`]
/// trait so the engine can drive it like any other insert.
pub struct VstProcessor {
    /// Owns the module and the COM objects; released after `Drop` tears down.
    instance: VstInstance,
    component: *mut c_void,
    controller: *mut c_void,
    audio_processor: *mut c_void,
    sample_rate: f64,
    max_frames: usize,
    active: bool,
    processing: bool,
    latency: u32,
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    changes: ParameterChanges,
}

// The audio thread owns the processor; the raw COM pointers are only used
// there. (Sharing the instance with the main thread, e.g. for a GUI, is a
// later phase.)
unsafe impl Send for VstProcessor {}

impl Drop for VstProcessor {
    fn drop(&mut self) {
        unsafe {
            if self.processing
                && let Some(stop) =
                    vtable::<AudioProcessorVtbl>(self.audio_processor).set_processing
            {
                stop(self.audio_processor, 0);
            }
            if self.active
                && let Some(deactivate) = vtable::<ComponentVtbl>(self.component).set_active
            {
                deactivate(self.component, 0);
            }
            // Release the `IAudioProcessor` reference from `queryInterface`.
            if !self.audio_processor.is_null() {
                release_object(self.audio_processor);
            }
        }
        // `instance` drops next, terminating and releasing the objects.
    }
}

impl VstProcessor {
    fn queue_parameter(&mut self, id: u32, value: f64) {
        if let Some(queue) = self.changes.queues.iter_mut().find(|q| q.id == id) {
            queue.points.clear();
            queue.points.push(ParamPoint {
                sample_offset: 0,
                value,
            });
        } else {
            self.changes.queues.push(ParamValueQueue {
                vtbl: &PARAM_VALUE_QUEUE_VTBL,
                id,
                points: vec![ParamPoint {
                    sample_offset: 0,
                    value,
                }],
            });
        }
    }

    fn clear_changes(&mut self) {
        for queue in &mut self.changes.queues {
            queue.points.clear();
        }
    }
}

impl PluginProcessor for VstProcessor {
    fn backend(&self) -> &'static str {
        "VST3"
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
        self.in_l[..frames].copy_from_slice(&left[..frames]);
        self.in_r[..frames].copy_from_slice(&right[..frames]);

        let mut in_channels: [*mut f32; 2] = [self.in_l.as_mut_ptr(), self.in_r.as_mut_ptr()];
        let mut out_channels: [*mut f32; 2] = [self.out_l.as_mut_ptr(), self.out_r.as_mut_ptr()];
        let mut input_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            channel_buffers32: in_channels.as_mut_ptr(),
        };
        let mut output_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            channel_buffers32: out_channels.as_mut_ptr(),
        };
        let mut data = ProcessData {
            process_mode: K_REALTIME,
            symbolic_sample_size: K_SAMPLE32,
            num_samples: frames as i32,
            num_inputs: 1,
            num_outputs: 1,
            inputs: &mut input_bus,
            outputs: &mut output_bus,
            input_parameter_changes: &mut self.changes as *mut ParameterChanges as *mut c_void,
            output_parameter_changes: std::ptr::null_mut(),
            input_events: std::ptr::null_mut(),
            output_events: std::ptr::null_mut(),
            process_context: std::ptr::null_mut(),
        };
        if let Some(process) = unsafe { vtable::<AudioProcessorVtbl>(self.audio_processor) }.process
        {
            unsafe { process(self.audio_processor, &mut data) };
        }
        left[..frames].copy_from_slice(&self.out_l[..frames]);
        right[..frames].copy_from_slice(&self.out_r[..frames]);
        self.clear_changes();
    }
    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        if !self.instance.params.iter().any(|p| p.id == id) {
            return false;
        }
        unsafe {
            if let Some(set) = vtable::<ControllerVtbl>(self.controller).set_param_normalized {
                set(self.controller, id, value);
            }
        }
        self.queue_parameter(id, value);
        true
    }
    fn reset(&mut self) {
        unsafe {
            if let Some(set_active) = vtable::<ComponentVtbl>(self.component).set_active {
                set_active(self.component, 0);
                set_active(self.component, 1);
            }
        }
        self.clear_changes();
    }
    fn save_state(&mut self) -> Vec<u8> {
        let mut stream = StreamState {
            vtbl: &STREAM_VTBL,
            data: Vec::new(),
            pos: 0,
        };
        let ok = unsafe {
            vtable::<ComponentVtbl>(self.component)
                .get_state
                .map(|get| {
                    get(
                        self.component,
                        &mut stream as *mut StreamState as *mut c_void,
                    ) == K_RESULT_OK
                })
                .unwrap_or(false)
        };
        if ok { stream.data } else { Vec::new() }
    }
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        let mut stream = StreamState {
            vtbl: &STREAM_VTBL,
            data: data.to_vec(),
            pos: 0,
        };
        let ok = unsafe {
            vtable::<ComponentVtbl>(self.component)
                .set_state
                .map(|set| {
                    set(
                        self.component,
                        &mut stream as *mut StreamState as *mut c_void,
                    ) == K_RESULT_OK
                })
                .unwrap_or(false)
        };
        if !ok {
            return false;
        }
        // Let the controller observe the restored processor state.
        stream.pos = 0;
        unsafe {
            if let Some(set) = vtable::<ControllerVtbl>(self.controller).set_component_state {
                set(
                    self.controller,
                    &mut stream as *mut StreamState as *mut c_void,
                );
            }
        }
        true
    }
}

/// Loads `path` and returns a live, activated VST3 processor ready to render
/// blocks of up to `max_frames` at `sample_rate`.
pub fn load_processor(
    path: &str,
    sample_rate: f32,
    max_frames: u32,
) -> Result<Box<dyn PluginProcessor>, String> {
    let instance = open(path)?;
    let component = instance.handles.component;
    let controller = instance.handles.controller;
    let max_frames = max_frames.max(1) as usize;
    let sample_rate = sample_rate.max(1.0) as f64;

    let audio_processor = unsafe {
        let query = vtable::<FUnknownVtbl>(component).query_interface;
        let mut obj: *mut c_void = std::ptr::null_mut();
        match query {
            Some(q)
                if q(
                    component,
                    IID_IAUDIO_PROCESSOR.as_ptr() as *const c_char,
                    &mut obj,
                ) == K_RESULT_OK
                    && !obj.is_null() =>
            {
                obj
            }
            _ => {
                return Err(crate::tstatus!(
                    "VST3-komponenten i '{}' exponerar ingen IAudioProcessor",
                    path
                ));
            }
        }
    };

    unsafe {
        if let Some(activate_bus) = vtable::<ComponentVtbl>(component).activate_bus {
            activate_bus(component, 0, 0, 0, 1);
            activate_bus(component, 0, 1, 0, 1);
        }
    }

    let mut setup = ProcessSetup {
        process_mode: K_REALTIME,
        symbolic_sample_size: K_SAMPLE32,
        max_samples_per_block: max_frames as i32,
        sample_rate,
    };
    unsafe {
        if let Some(setup_fn) = vtable::<AudioProcessorVtbl>(audio_processor).setup_processing
            && setup_fn(audio_processor, &mut setup) != K_RESULT_OK
        {
            release_object(audio_processor);
            return Err(crate::tstatus!(
                "Kunde inte konfigurera VST3-processorn i '{}'",
                path
            ));
        }
    }

    let active = unsafe {
        match vtable::<ComponentVtbl>(component).set_active {
            Some(set) => set(component, 1) == K_RESULT_OK,
            None => false,
        }
    };
    if !active {
        unsafe { release_object(audio_processor) };
        return Err(crate::tstatus!(
            "Kunde inte aktivera VST3-komponenten i '{}'",
            path
        ));
    }

    let processing = unsafe {
        match vtable::<AudioProcessorVtbl>(audio_processor).set_processing {
            Some(set) => set(audio_processor, 1) == K_RESULT_OK,
            None => false,
        }
    };
    if !processing {
        unsafe {
            if let Some(set) = vtable::<ComponentVtbl>(component).set_active {
                set(component, 0);
            }
        }
        unsafe { release_object(audio_processor) };
        return Err(crate::tstatus!(
            "Kunde inte starta VST3-processningen i '{}'",
            path
        ));
    }

    let latency = unsafe {
        vtable::<AudioProcessorVtbl>(audio_processor)
            .get_latency_samples
            .map(|get| get(audio_processor))
            .unwrap_or(0)
    };

    Ok(Box::new(VstProcessor {
        instance,
        component,
        controller,
        audio_processor,
        sample_rate,
        max_frames,
        active,
        processing,
        latency,
        in_l: vec![0.0; max_frames],
        in_r: vec![0.0; max_frames],
        out_l: vec![0.0; max_frames],
        out_r: vec![0.0; max_frames],
        changes: ParameterChanges {
            vtbl: &PARAMETER_CHANGES_VTBL,
            queues: Vec::new(),
        },
    }))
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

    #[test]
    fn loads_processor_and_reports_latency() {
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
            return;
        };
        let processor = load_processor(mock, 48_000.0, 512).expect("mock processor");
        assert_eq!(processor.backend(), "VST3");
        assert_eq!(processor.info().name, "Sonix Mock VST3");
        assert_eq!(processor.parameters().len(), 2);
        assert_eq!(processor.latency_frames(), 8, "mock reports 8 frames");
    }

    #[test]
    fn processor_applies_parameter_changes() {
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
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
        assert!(processor.set_parameter(100, 0.5));
        let mut l = vec![1.0_f32; 32];
        let mut r = vec![1.0_f32; 32];
        processor.process_stereo(&mut l, &mut r);
        assert!((l[16] - 0.5).abs() < 1e-5, "expected 0.5, got {}", l[16]);
    }

    #[test]
    fn processor_state_round_trips() {
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
            return;
        };
        let mut processor = load_processor(mock, 48_000.0, 512).expect("mock processor");
        assert!(processor.set_parameter(100, 0.25));
        let mut l = vec![1.0_f32; 32];
        let mut r = vec![1.0_f32; 32];
        processor.process_stereo(&mut l, &mut r);
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
        let Some(mock) = option_env!("SONIX_MOCK_VST3") else {
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
