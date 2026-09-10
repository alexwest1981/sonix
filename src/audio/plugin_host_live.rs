//! In-process **CLAP** plugin host.
//!
//! This is the first real plugin host in Sonix. It speaks the CLAP 1.x C ABI
//! directly (no heavyweight host crate) and uses `dlopen` through the
//! `libloading` crate. The whole loader is **opt-in** behind the
//! `plugin-host` feature so the default build stays dependency-free:
//!
//! ```text
//! cargo build --features plugin-host
//! ```
//!
//! Scope of this module (Fas 4.1): discover a CLAP shared object, `dlopen` it,
//! read the exported `clap_entry`, instantiate a plugin through the
//! plugin-factory, and report its descriptor plus its parameters (via the
//! `clap.params` extension). Audio/MIDI processing, GUI and sandboxing are
//! later phases.
//!
//! Everything that does not touch the ABI (the data types and `inspect`) is
//! compiled in both configurations, so the UI can always show an honest status.

use std::collections::VecDeque;
use std::sync::Arc;

/// CLAP parameter flag: the value is a discrete/stepped value.
pub const PARAM_IS_STEPPED: u32 = 1 << 0;
/// CLAP parameter flag: periodic (e.g. a phase).
pub const PARAM_IS_PERIODIC: u32 = 1 << 1;
/// CLAP parameter flag: hidden from the host UI.
pub const PARAM_IS_HIDDEN: u32 = 1 << 2;
/// CLAP parameter flag: read-only.
pub const PARAM_IS_READONLY: u32 = 1 << 3;
/// CLAP parameter flag: this is the plugin's bypass parameter.
pub const PARAM_IS_BYPASS: u32 = 1 << 4;
/// CLAP parameter flag: can be automated.
pub const PARAM_IS_AUTOMATABLE: u32 = 1 << 5;
/// CLAP parameter flag: can be modulated.
pub const PARAM_IS_MODULATABLE: u32 = 1 << 10;

/// A single automatable parameter reported by a plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginParameter {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub flags: u32,
}

#[allow(dead_code)]
impl PluginParameter {
    pub fn is_stepped(&self) -> bool {
        self.flags & PARAM_IS_STEPPED != 0
    }
    pub fn is_periodic(&self) -> bool {
        self.flags & PARAM_IS_PERIODIC != 0
    }
    pub fn is_hidden(&self) -> bool {
        self.flags & PARAM_IS_HIDDEN != 0
    }
    pub fn is_readonly(&self) -> bool {
        self.flags & PARAM_IS_READONLY != 0
    }
    pub fn is_bypass(&self) -> bool {
        self.flags & PARAM_IS_BYPASS != 0
    }
    pub fn is_automatable(&self) -> bool {
        self.flags & PARAM_IS_AUTOMATABLE != 0
    }
    pub fn is_modulatable(&self) -> bool {
        self.flags & PARAM_IS_MODULATABLE != 0
    }
}

/// Identity/descriptor information reported by a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub description: String,
    pub features: Vec<String>,
}

/// What a plugin's `clap.gui` extension supports, discovered during
/// inspection. `None` (on [`PluginInspection::gui`]) means the plugin ships no
/// GUI at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginGuiCapability {
    /// Window API the plugin can be embedded into, e.g. `"x11"`.
    pub api: String,
    /// Whether the plugin prefers a floating window rather than an embedded one.
    pub floating: bool,
    pub width: u32,
    pub height: u32,
    pub can_resize: bool,
}

/// Backend-agnostic handle to a live plugin instance.
///
/// Future backends (VST3, LV2) will implement the same trait.
#[allow(dead_code)]
pub trait PluginInstance {
    /// Human-readable backend name, e.g. `"CLAP"`.
    fn backend(&self) -> &'static str;
    fn info(&self) -> &PluginInfo;
    fn parameters(&self) -> &[PluginParameter];
    /// Probes the plugin's GUI support. Defaults to "no GUI".
    fn gui_capability(&self) -> Option<PluginGuiCapability> {
        None
    }
}

/// Serializable snapshot of an inspection, safe to keep in the UI state.
#[derive(Debug, Clone)]
pub struct PluginInspection {
    pub path: String,
    pub backend: String,
    pub info: Option<PluginInfo>,
    pub parameters: Vec<PluginParameter>,
    /// GUI capability reported by `clap.gui`, if any.
    pub gui: Option<PluginGuiCapability>,
    pub error: Option<String>,
}

impl PluginInspection {
    fn failed(path: &str, backend: &str, error: String) -> Self {
        Self {
            path: path.to_string(),
            backend: backend.to_string(),
            info: None,
            parameters: Vec::new(),
            gui: None,
            error: Some(error),
        }
    }
}

/// True when Sonix was compiled with the `plugin-host` feature.
pub fn is_available() -> bool {
    cfg!(feature = "plugin-host")
}

/// Loads a CLAP plugin at `path` and returns a snapshot of its descriptor and
/// parameters. Never panics: any failure is reported in `error`.
pub fn inspect(path: &str) -> PluginInspection {
    #[cfg(feature = "plugin-host")]
    {
        if crate::audio::plugin_vst3::is_vst3_path(path) {
            return crate::audio::plugin_vst3::inspect(path);
        }
        if crate::audio::plugin_vst2::is_vst2_path(path) {
            return crate::audio::plugin_vst2::inspect(path);
        }
        match imp::load(path) {
            Ok(instance) => PluginInspection {
                path: path.to_string(),
                backend: instance.backend().to_string(),
                info: Some(instance.info().clone()),
                parameters: instance.parameters().to_vec(),
                gui: instance.gui_capability(),
                error: None,
            },
            Err(err) => PluginInspection::failed(path, "CLAP", err),
        }
    }
    #[cfg(not(feature = "plugin-host"))]
    {
        PluginInspection::failed(
            path,
            "—",
            crate::i18n::t(
                "Plugin-hosting är inte inbyggt i den här builden (bygg med --features plugin-host)",
            )
            .to_string(),
        )
    }
}

// ===========================================================================
// Backend-agnostic audio processing (Fas 4.2)
// ===========================================================================

/// Host-side block size used to drive block-oriented plugin APIs from Sonix's
/// per-sample engine. All inserts share this value so their buffering latency
/// is identical and only the plugin-reported latency differs.
pub const DEFAULT_BLOCK_FRAMES: usize = 128;

/// A live, *processing* plugin instance. Backend-agnostic; the CLAP backend
/// implements it in `imp`. Future VST3/LV2 backends will implement the same.
#[allow(dead_code)]
pub trait PluginProcessor: Send {
    /// Human-readable backend name, e.g. `"CLAP"`.
    fn backend(&self) -> &'static str;
    fn info(&self) -> &PluginInfo;
    fn parameters(&self) -> &[PluginParameter];
    /// Frames of latency the plugin *itself* introduces (not counting the
    /// host's block buffering).
    fn latency_frames(&self) -> u32;
    /// Processes `left`/`right` in place. Both slices must be the same length
    /// and must not exceed the maximum frame count the processor was created
    /// with.
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]);
    /// Applies a parameter value. Returns false when the id is unknown.
    fn set_parameter(&mut self, id: u32, value: f64) -> bool;
    /// Clears internal state (delay lines, voices, …).
    fn reset(&mut self);
    /// Saves the plugin's opaque state via `clap.state`. Empty when unsupported.
    fn save_state(&mut self) -> Vec<u8> {
        Vec::new()
    }
    /// Restores state previously produced by [`Self::save_state`]. Returns false
    /// when unsupported or when the blob is rejected.
    fn load_state(&mut self, data: &[u8]) -> bool {
        let _ = data;
        false
    }
    /// Loads one of the plugin's own presets from `location` via
    /// `clap.preset-load/2`. Returns false when unsupported.
    fn preset_load(&mut self, location: &str) -> bool {
        let _ = location;
        false
    }

    /// `clap.gui`: whether the plugin can embed into `api` (e.g. `"x11"`).
    fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool {
        let _ = (api, is_floating);
        false
    }
    /// `clap.gui`: the plugin's preferred window API and floating preference.
    fn gui_preferred_api(&self) -> Option<(String, bool)> {
        None
    }
    /// `clap.gui`: the GUI's current size. Only valid after [`Self::gui_create`].
    fn gui_get_size(&self) -> Option<(u32, u32)> {
        None
    }
    /// `clap.gui`: whether the user may resize the window.
    fn gui_can_resize(&self) -> bool {
        false
    }
    /// Whether the GUI has been created and not yet destroyed.
    fn gui_is_created(&self) -> bool {
        false
    }
    /// `clap.gui`: creates the GUI for `api`. Main-thread only.
    fn gui_create(&mut self, api: &str, is_floating: bool) -> bool {
        let _ = (api, is_floating);
        false
    }
    /// `clap.gui`: attaches the GUI to a host-provided X11 window. Main-thread.
    fn gui_set_parent(&mut self, x11_window: u64) -> bool {
        let _ = x11_window;
        false
    }
    /// `clap.gui`: requests a new window size. Main-thread only.
    fn gui_set_size(&mut self, width: u32, height: u32) -> bool {
        let _ = (width, height);
        false
    }
    /// `clap.gui`: makes the GUI visible. Main-thread only.
    fn gui_show(&mut self) -> bool {
        false
    }
    /// `clap.gui`: hides the GUI without destroying it. Main-thread only.
    fn gui_hide(&mut self) -> bool {
        false
    }
    /// `clap.gui`: destroys the GUI. Main-thread only.
    fn gui_destroy(&mut self) {}
    /// A cloneable handle to the underlying instance, when the backend supports
    /// sharing it between the audio thread and the main thread (Fas 4.4b).
    ///
    /// The handle keeps the plugin alive: hold one for as long as any GUI
    /// session or deferred state access may still touch the instance.
    fn core_handle(&self) -> Option<PluginHandle> {
        None
    }
}

/// A cloneable, thread-safe handle to a live plugin instance.
///
/// CLAP explicitly allows the host to call `process` on the audio thread while
/// calling GUI/state methods on the main thread, so the instance can be shared
/// behind an `Arc`. The handle is what makes the GUI operate on the *same*
/// instance that is processing audio — no duplicated/parallel plugin.
#[allow(dead_code)]
pub trait PluginCore: Send + Sync {
    fn info(&self) -> &PluginInfo;
    fn latency_frames(&self) -> u32;
    fn save_state(&self) -> Vec<u8>;
    fn load_state(&self, data: &[u8]) -> bool;
    fn set_parameter(&self, id: u32, value: f64) -> bool;
    fn reset(&self);
    fn preset_load(&self, location: &str) -> bool;
    fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool;
    fn gui_preferred_api(&self) -> Option<(String, bool)>;
    fn gui_get_size(&self) -> Option<(u32, u32)>;
    fn gui_can_resize(&self) -> bool;
    fn gui_is_created(&self) -> bool;
    fn gui_create(&self, api: &str, is_floating: bool) -> bool;
    fn gui_set_parent(&self, x11_window: u64) -> bool;
    fn gui_set_size(&self, width: u32, height: u32) -> bool;
    fn gui_show(&self) -> bool;
    fn gui_hide(&self) -> bool;
    fn gui_destroy(&self);
}

/// Shared ownership wrapper around a [`PluginCore`].
#[derive(Clone)]
#[allow(dead_code)]
pub struct PluginHandle(Arc<dyn PluginCore>);

#[allow(dead_code)]
impl PluginHandle {
    pub fn new(core: Arc<dyn PluginCore>) -> Self {
        Self(core)
    }
    pub fn info(&self) -> &PluginInfo {
        self.0.info()
    }
    pub fn latency_frames(&self) -> u32 {
        self.0.latency_frames()
    }
    pub fn save_state(&self) -> Vec<u8> {
        self.0.save_state()
    }
    pub fn load_state(&self, data: &[u8]) -> bool {
        self.0.load_state(data)
    }
    pub fn set_parameter(&self, id: u32, value: f64) -> bool {
        self.0.set_parameter(id, value)
    }
    pub fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool {
        self.0.gui_is_api_supported(api, is_floating)
    }
    pub fn gui_preferred_api(&self) -> Option<(String, bool)> {
        self.0.gui_preferred_api()
    }
    pub fn gui_get_size(&self) -> Option<(u32, u32)> {
        self.0.gui_get_size()
    }
    pub fn gui_can_resize(&self) -> bool {
        self.0.gui_can_resize()
    }
    pub fn gui_is_created(&self) -> bool {
        self.0.gui_is_created()
    }
    pub fn gui_create(&self, api: &str, is_floating: bool) -> bool {
        self.0.gui_create(api, is_floating)
    }
    pub fn gui_set_parent(&self, x11_window: u64) -> bool {
        self.0.gui_set_parent(x11_window)
    }
    pub fn gui_set_size(&self, width: u32, height: u32) -> bool {
        self.0.gui_set_size(width, height)
    }
    pub fn gui_show(&self) -> bool {
        self.0.gui_show()
    }
    pub fn gui_hide(&self) -> bool {
        self.0.gui_hide()
    }
    pub fn gui_destroy(&self) {
        self.0.gui_destroy()
    }
}

/// Creates a live processing instance for `path`, ready to be wrapped in a
/// [`PluginInsert`]. `max_frames` is the largest block that will be processed.
pub fn load_processor(
    path: &str,
    sample_rate: f32,
    max_frames: u32,
) -> Result<Box<dyn PluginProcessor>, String> {
    #[cfg(feature = "plugin-host")]
    {
        if crate::audio::plugin_vst3::is_vst3_path(path) {
            return crate::audio::plugin_vst3::load_processor(path, sample_rate, max_frames);
        }
        if crate::audio::plugin_vst2::is_vst2_path(path) {
            return crate::audio::plugin_vst2::load_processor(path, sample_rate, max_frames);
        }
        imp::load_processor(path, sample_rate, max_frames)
    }
    #[cfg(not(feature = "plugin-host"))]
    {
        let _ = (path, sample_rate, max_frames);
        Err(crate::i18n::t(
            "Plugin-hosting är inte inbyggt i den här builden (bygg med --features plugin-host)",
        )
        .to_string())
    }
}

/// A per-track plugin insert.
///
/// Sonix's engine renders one sample at a time while CLAP processes blocks, so
/// this type buffers `block_frames` input frames, runs the plugin on the whole
/// block and then plays the result back out one sample at a time. It therefore
/// introduces `block_frames + plugin_latency` frames of latency, which the
/// engine compensates for with [`PdcDelay`] (plug-in delay compensation).
#[allow(dead_code)]
pub struct PluginInsert {
    processor: Box<dyn PluginProcessor>,
    block_frames: usize,
    filled: usize,
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    out_l: VecDeque<f32>,
    out_r: VecDeque<f32>,
    /// Frames processed since the last reset, for cheap diagnostics/tests.
    frames_processed: u64,
}

#[allow(dead_code)]
impl PluginInsert {
    pub fn new(processor: Box<dyn PluginProcessor>, block_frames: usize) -> Self {
        let block_frames = block_frames.max(1);
        Self {
            processor,
            block_frames,
            filled: 0,
            in_l: vec![0.0; block_frames],
            in_r: vec![0.0; block_frames],
            out_l: VecDeque::with_capacity(block_frames * 2),
            out_r: VecDeque::with_capacity(block_frames * 2),
            frames_processed: 0,
        }
    }

    pub fn backend(&self) -> &'static str {
        self.processor.backend()
    }

    pub fn info(&self) -> &PluginInfo {
        self.processor.info()
    }

    pub fn parameters(&self) -> &[PluginParameter] {
        self.processor.parameters()
    }

    pub fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        self.processor.set_parameter(id, value)
    }

    /// Saves the plugin state (main-thread only, per the CLAP contract).
    pub fn save_state(&mut self) -> Vec<u8> {
        self.processor.save_state()
    }

    /// Restores plugin state (main-thread only, per the CLAP contract).
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        self.processor.load_state(data)
    }

    /// Loads a native preset from a location (main-thread only).
    pub fn preset_load(&mut self, location: &str) -> bool {
        self.processor.preset_load(location)
    }

    /// `clap.gui` helpers — all main-thread only, delegated to the processor.
    pub fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool {
        self.processor.gui_is_api_supported(api, is_floating)
    }
    pub fn gui_preferred_api(&self) -> Option<(String, bool)> {
        self.processor.gui_preferred_api()
    }
    pub fn gui_get_size(&self) -> Option<(u32, u32)> {
        self.processor.gui_get_size()
    }
    pub fn gui_can_resize(&self) -> bool {
        self.processor.gui_can_resize()
    }
    pub fn gui_is_created(&self) -> bool {
        self.processor.gui_is_created()
    }
    pub fn gui_create(&mut self, api: &str, is_floating: bool) -> bool {
        self.processor.gui_create(api, is_floating)
    }
    pub fn gui_set_parent(&mut self, x11_window: u64) -> bool {
        self.processor.gui_set_parent(x11_window)
    }
    pub fn gui_set_size(&mut self, width: u32, height: u32) -> bool {
        self.processor.gui_set_size(width, height)
    }
    pub fn gui_show(&mut self) -> bool {
        self.processor.gui_show()
    }
    pub fn gui_hide(&mut self) -> bool {
        self.processor.gui_hide()
    }
    pub fn gui_destroy(&mut self) {
        self.processor.gui_destroy()
    }

    /// Shared handle to the plugin instance, when the backend supports it.
    pub fn core_handle(&self) -> Option<PluginHandle> {
        self.processor.core_handle()
    }

    pub fn block_frames(&self) -> usize {
        self.block_frames
    }

    /// Latency introduced by the plugin itself.
    pub fn plugin_latency_frames(&self) -> usize {
        self.processor.latency_frames() as usize
    }

    /// Total latency (host block buffering + plugin latency).
    pub fn latency_frames(&self) -> usize {
        // The first output sample is emitted on the call that completes the
        // first block, so the host contributes `block_frames - 1` frames.
        self.block_frames.saturating_sub(1) + self.plugin_latency_frames()
    }

    pub fn frames_processed(&self) -> u64 {
        self.frames_processed
    }

    pub fn reset(&mut self) {
        self.processor.reset();
        self.filled = 0;
        self.out_l.clear();
        self.out_r.clear();
    }

    /// Feeds one input sample and returns the next output sample. The first
    /// `latency_frames()` calls return silence while the pipeline fills.
    #[inline]
    pub fn process_sample(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.in_l[self.filled] = l;
        self.in_r[self.filled] = r;
        self.filled += 1;
        if self.filled == self.block_frames {
            self.processor.process_stereo(&mut self.in_l, &mut self.in_r);
            for i in 0..self.block_frames {
                self.out_l.push_back(self.in_l[i]);
                self.out_r.push_back(self.in_r[i]);
            }
            self.filled = 0;
            self.frames_processed += self.block_frames as u64;
        }
        let ol = self.out_l.pop_front().unwrap_or(0.0);
        let or = self.out_r.pop_front().unwrap_or(0.0);
        (ol, or)
    }
}

/// A simple fixed-length stereo delay used for plug-in delay compensation.
/// With `delay == 0` it is a zero-cost pass-through.
#[allow(dead_code)]
pub struct PdcDelay {
    l: VecDeque<f32>,
    r: VecDeque<f32>,
    delay: usize,
}

#[allow(dead_code)]
impl PdcDelay {
    pub fn new() -> Self {
        Self {
            l: VecDeque::new(),
            r: VecDeque::new(),
            delay: 0,
        }
    }

    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Changes the delay, clearing the line when the length changes so stale
    /// samples never leak through.
    pub fn set_delay(&mut self, frames: usize) {
        if frames != self.delay {
            self.l.clear();
            self.r.clear();
            self.delay = frames;
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.delay == 0 {
            return (l, r);
        }
        self.l.push_back(l);
        self.r.push_back(r);
        if self.l.len() > self.delay {
            (
                self.l.pop_front().unwrap_or(0.0),
                self.r.pop_front().unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0)
        }
    }
}

impl Default for PdcDelay {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Real CLAP ABI + loader (only when the feature is enabled)
// ===========================================================================
#[cfg(feature = "plugin-host")]
mod imp {
    use super::{PluginInfo, PluginInstance, PluginParameter};
    use libloading::Library;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub const CLAP_VERSION_MAJOR: u32 = 1;

    pub const CLAP_EXT_PARAMS: &CStr = c"clap.params";
    pub const CLAP_EXT_AUDIO_PORTS: &CStr = c"clap.audio-ports";
    pub const CLAP_EXT_NOTE_PORTS: &CStr = c"clap.note-ports";
    pub const CLAP_EXT_LATENCY: &CStr = c"clap.latency";
    pub const CLAP_EXT_STATE: &CStr = c"clap.state";
    /// Current `clap.preset-load` revision. The draft id is the same ABI and is
    /// still advertised by some plugins.
    pub const CLAP_EXT_PRESET_LOAD: &CStr = c"clap.preset-load/2";
    pub const CLAP_EXT_PRESET_LOAD_DRAFT: &CStr = c"clap.preset-load.draft/2";
    pub const CLAP_EXT_GUI: &CStr = c"clap.gui";
    pub const CLAP_PLUGIN_FACTORY_ID: &CStr = c"clap.plugin-factory";

    /// `CLAP_WINDOW_API_X11` — the only window API this host embeds into.
    pub const CLAP_WINDOW_API_X11: &CStr = c"x11";

    /// `clap_event_param_value` space id/type (CLAP core event space 0).
    const CLAP_CORE_EVENT_SPACE_ID: u16 = 0;
    const CLAP_EVENT_PARAM_VALUE: u16 = 5;

    const HOST_NAME: &CStr = c"Sonix";
    const HOST_VENDOR: &CStr = c"Sonix Studio";
    const HOST_URL: &CStr = c"https://github.com/alexwest1981/sonix";
    const HOST_VERSION: &CStr = c"0.9.0";

    const CLAP_NAME_SIZE: usize = 256;
    const CLAP_PATH_SIZE: usize = 1024;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ClapVersion {
        pub major: u32,
        pub minor: u32,
        pub revision: u32,
    }

    #[repr(C)]
    pub struct ClapPluginDescriptor {
        pub clap_version: ClapVersion,
        pub id: *const c_char,
        pub name: *const c_char,
        pub vendor: *const c_char,
        pub version: *const c_char,
        pub description: *const c_char,
        pub features: *const *const c_char,
    }

    #[repr(C)]
    pub struct ClapPlugin {
        pub desc: *const ClapPluginDescriptor,
        pub plugin_data: *mut c_void,
        pub init: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
        pub destroy: Option<unsafe extern "C" fn(*const ClapPlugin)>,
        pub activate:
            Option<unsafe extern "C" fn(*const ClapPlugin, f64, u32, u32) -> bool>,
        pub deactivate: Option<unsafe extern "C" fn(*const ClapPlugin)>,
        pub start_processing: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
        pub stop_processing: Option<unsafe extern "C" fn(*const ClapPlugin)>,
        pub reset: Option<unsafe extern "C" fn(*const ClapPlugin)>,
        pub process:
            Option<unsafe extern "C" fn(*const ClapPlugin, *const c_void) -> i32>,
        pub get_extension:
            Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char) -> *const c_void>,
        pub on_main_thread: Option<unsafe extern "C" fn(*const ClapPlugin)>,
    }

    #[repr(C)]
    pub struct ClapPluginFactory {
        pub get_plugin_count: Option<unsafe extern "C" fn(*const ClapPluginFactory) -> u32>,
        pub get_plugin_descriptor:
            Option<unsafe extern "C" fn(*const ClapPluginFactory, u32) -> *const ClapPluginDescriptor>,
        pub create_plugin: Option<
            unsafe extern "C" fn(
                *const ClapPluginFactory,
                *const ClapHost,
                *const c_char,
            ) -> *const ClapPlugin,
        >,
    }

    #[repr(C)]
    pub struct ClapPluginEntry {
        pub clap_version: ClapVersion,
        pub init: Option<unsafe extern "C" fn(*const c_char) -> bool>,
        pub deinit: Option<unsafe extern "C" fn()>,
        pub get_factory: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
    }

    #[repr(C)]
    pub struct ClapHost {
        pub clap_version: ClapVersion,
        pub host_data: *mut c_void,
        pub name: *const c_char,
        pub vendor: *const c_char,
        pub url: *const c_char,
        pub version: *const c_char,
        pub get_extension:
            Option<unsafe extern "C" fn(*const ClapHost, *const c_char) -> *const c_void>,
        pub request_restart: Option<unsafe extern "C" fn(*const ClapHost)>,
        pub request_process: Option<unsafe extern "C" fn(*const ClapHost)>,
        pub request_callback: Option<unsafe extern "C" fn(*const ClapHost)>,
    }

    #[repr(C)]
    pub struct ClapParamInfo {
        pub id: u32,
        pub flags: u32,
        pub name: [c_char; CLAP_NAME_SIZE],
        pub module: [c_char; CLAP_PATH_SIZE],
        pub min_value: f64,
        pub max_value: f64,
        pub default_value: f64,
    }

    #[repr(C)]
    pub struct ClapPluginParams {
        pub count: Option<unsafe extern "C" fn(*const ClapPlugin) -> u32>,
        pub get_info:
            Option<unsafe extern "C" fn(*const ClapPlugin, u32, *mut ClapParamInfo) -> bool>,
        pub get_value: Option<unsafe extern "C" fn(*const ClapPlugin, u32, *mut f64) -> bool>,
        pub value_to_text: Option<
            unsafe extern "C" fn(*const ClapPlugin, u32, f64, *mut c_char, u32) -> bool,
        >,
        pub text_to_value:
            Option<unsafe extern "C" fn(*const ClapPlugin, u32, *const c_char, *mut f64) -> bool>,
        pub flush: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_void, *const c_void)>,
    }

    #[repr(C)]
    pub struct ClapAudioPortInfo {
        pub id: u32,
        pub name: [c_char; CLAP_NAME_SIZE],
        pub flags: u32,
        pub channel_count: u32,
        pub port_type: *const c_char,
        pub in_place_pair: u32,
    }

    #[repr(C)]
    pub struct ClapPluginAudioPorts {
        pub count: Option<unsafe extern "C" fn(*const ClapPlugin, bool) -> u32>,
        pub get: Option<
            unsafe extern "C" fn(*const ClapPlugin, u32, bool, *mut ClapAudioPortInfo) -> bool,
        >,
    }

    #[repr(C)]
    pub struct ClapNotePortInfo {
        pub id: u32,
        pub name: [c_char; CLAP_NAME_SIZE],
        pub supported_dialects: u32,
        pub preferred_dialect: u32,
    }

    #[repr(C)]
    pub struct ClapPluginNotePorts {
        pub count: Option<unsafe extern "C" fn(*const ClapPlugin, bool) -> u32>,
        pub get: Option<
            unsafe extern "C" fn(*const ClapPlugin, u32, bool, *mut ClapNotePortInfo) -> bool,
        >,
    }

    #[repr(C)]
    pub struct ClapPluginLatency {
        pub get: Option<unsafe extern "C" fn(*const ClapPlugin) -> u32>,
    }

    /// `clap_ostream_t`: the host hands the plugin a `write` sink so it can
    /// serialise its state. Returns the number of bytes written, `0` on EOF
    /// (stop), or `-1` on error.
    #[repr(C)]
    pub struct ClapOstream {
        pub ctx: *mut c_void,
        pub write:
            Option<unsafe extern "C" fn(*const ClapOstream, *const c_void, u64) -> i64>,
    }

    /// `clap_istream_t`: the host hands the plugin a `read` source so it can
    /// deserialise its state.
    #[repr(C)]
    pub struct ClapIstream {
        pub ctx: *mut c_void,
        pub read: Option<unsafe extern "C" fn(*const ClapIstream, *mut c_void, u64) -> i64>,
    }

    #[repr(C)]
    pub struct ClapPluginState {
        pub save: Option<unsafe extern "C" fn(*const ClapPlugin, *const ClapOstream) -> bool>,
        pub load: Option<unsafe extern "C" fn(*const ClapPlugin, *const ClapIstream) -> bool>,
    }

    /// `clap.preset-load/2`: loads one of the plugin's own presets. `location`
    /// is a filesystem path (`location_kind == 0`), `load_key` may be null.
    #[repr(C)]
    pub struct ClapPluginPresetLoad {
        pub from_location: Option<
            unsafe extern "C" fn(*const ClapPlugin, u32, *const c_char, *const c_char) -> bool,
        >,
    }

    /// `CLAP_PRESET_DISCOVERY_LOCATION_FILE`
    const CLAP_PRESET_LOCATION_FILE: u32 = 0;

    /// `clap_window_t`: identifies the host window the GUI is embedded into.
    /// The `api` string selects the union member; only the X11 handle is used,
    /// and it is pointer-sized, so a single `usize` mirrors the union.
    #[repr(C)]
    #[allow(dead_code)]
    pub struct ClapWindow {
        pub api: *const c_char,
        pub x11: usize,
    }

    #[repr(C)]
    #[allow(dead_code)]
    pub struct ClapGuiResizeHints {
        pub can_resize: bool,
        pub preserve_aspect_ratio: bool,
        pub aspect_ratio_width: u32,
        pub aspect_ratio_height: u32,
    }

    /// `clap_plugin_gui_t` — the `clap.gui` extension vtable. Every field must
    /// be declared, in order, even the ones this host does not call, or the
    /// function-pointer offsets would be wrong.
    #[repr(C)]
    #[allow(dead_code)]
    pub struct ClapPluginGui {
        pub is_api_supported:
            Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char, bool) -> bool>,
        pub get_preferred_api:
            Option<unsafe extern "C" fn(*const ClapPlugin, *mut *const c_char, *mut bool) -> bool>,
        pub create: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char, bool) -> bool>,
        pub destroy: Option<unsafe extern "C" fn(*const ClapPlugin)>,
        pub set_scale: Option<unsafe extern "C" fn(*const ClapPlugin, f64) -> bool>,
        pub get_size: Option<unsafe extern "C" fn(*const ClapPlugin, *mut u32, *mut u32) -> bool>,
        pub can_resize: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
        pub get_resize_hints:
            Option<unsafe extern "C" fn(*const ClapPlugin, *mut ClapGuiResizeHints) -> bool>,
        pub adjust_size: Option<unsafe extern "C" fn(*const ClapPlugin, *mut u32, *mut u32) -> bool>,
        pub set_size: Option<unsafe extern "C" fn(*const ClapPlugin, u32, u32) -> bool>,
        pub set_parent: Option<unsafe extern "C" fn(*const ClapPlugin, *const ClapWindow) -> bool>,
        pub set_transient:
            Option<unsafe extern "C" fn(*const ClapPlugin, *const ClapWindow) -> bool>,
        pub suggest_title: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_char) -> bool>,
        pub show: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
        pub hide: Option<unsafe extern "C" fn(*const ClapPlugin) -> bool>,
    }

    #[repr(C)]
    pub struct ClapEventHeader {
        pub size: u32,
        pub time: u32,
        pub space_id: u16,
        pub event_type: u16,
        pub flags: u32,
    }

    #[repr(C)]
    pub struct ClapEventParamValue {
        pub header: ClapEventHeader,
        pub param_id: u32,
        pub cookie: *mut c_void,
        pub note_id: i32,
        pub port_index: i16,
        pub channel: i16,
        pub key: i16,
        pub value: f64,
    }

    #[repr(C)]
    pub struct ClapInputEvents {
        pub ctx: *mut c_void,
        pub size: Option<unsafe extern "C" fn(*const ClapInputEvents) -> u32>,
        pub get: Option<
            unsafe extern "C" fn(*const ClapInputEvents, u32) -> *const ClapEventHeader,
        >,
    }

    #[repr(C)]
    pub struct ClapOutputEvents {
        pub ctx: *mut c_void,
        pub try_push:
            Option<unsafe extern "C" fn(*const ClapOutputEvents, *const ClapEventHeader) -> bool>,
    }

    #[repr(C)]
    pub struct ClapAudioBuffer {
        pub data32: *mut *mut f32,
        pub data64: *mut *mut f64,
        pub channel_count: u32,
        pub latency: u32,
        pub constant_mask: u64,
    }

    #[repr(C)]
    pub struct ClapProcess {
        pub steady_time: i64,
        pub frames_count: u32,
        pub transport: *const c_void,
        pub audio_inputs: *const ClapAudioBuffer,
        pub audio_outputs: *mut ClapAudioBuffer,
        pub audio_inputs_count: u32,
        pub audio_outputs_count: u32,
        pub in_events: *const ClapInputEvents,
        pub out_events: *const ClapOutputEvents,
    }

    unsafe extern "C" fn host_get_extension(
        _host: *const ClapHost,
        _id: *const c_char,
    ) -> *const c_void {
        std::ptr::null()
    }
    unsafe extern "C" fn host_request_restart(_host: *const ClapHost) {}
    unsafe extern "C" fn host_request_process(_host: *const ClapHost) {}
    unsafe extern "C" fn host_request_callback(_host: *const ClapHost) {}

    /// Owns the `dlopen` handle and guarantees `deinit()` runs *before* the
    /// library is unloaded.
    struct LibraryHandle {
        _lib: Library,
        entry: *const ClapPluginEntry,
        initialized: bool,
    }

    impl Drop for LibraryHandle {
        fn drop(&mut self) {
            if self.initialized {
                unsafe {
                    if let Some(deinit) = (*self.entry).deinit {
                        deinit();
                    }
                }
            }
        }
    }

    struct ClapInstance {
        handle: LibraryHandle,
        plugin: *const ClapPlugin,
        _host: Box<ClapHost>,
        info: PluginInfo,
        params: Vec<PluginParameter>,
    }

    impl Drop for ClapInstance {
        fn drop(&mut self) {
            unsafe {
                if !self.plugin.is_null()
                    && let Some(destroy) = (*self.plugin).destroy
                {
                    destroy(self.plugin);
                }
            }
        }
    }

    impl PluginInstance for ClapInstance {
        fn backend(&self) -> &'static str {
            "CLAP"
        }
        fn info(&self) -> &PluginInfo {
            &self.info
        }
        fn parameters(&self) -> &[PluginParameter] {
            &self.params
        }
        fn gui_capability(&self) -> Option<super::PluginGuiCapability> {
            gui_capability(self.plugin)
        }
    }

    /// Fetches a plugin extension by id, or null when the plugin has none.
    fn plugin_extension(plugin: *const ClapPlugin, id: &CStr) -> *const c_void {
        match unsafe { &*plugin }.get_extension {
            Some(f) => unsafe { f(plugin, id.as_ptr()) },
            None => std::ptr::null(),
        }
    }

    /// Probes `clap.gui` for X11 support and the preferred size, creating and
    /// destroying the GUI once so `get_size` is valid (per the CLAP contract).
    fn gui_capability(plugin: *const ClapPlugin) -> Option<super::PluginGuiCapability> {
        let ext = plugin_extension(plugin, CLAP_EXT_GUI) as *const ClapPluginGui;
        if ext.is_null() {
            return None;
        }
        let gui = unsafe { &*ext };
        let is_supported = gui.is_api_supported?;
        if !unsafe { is_supported(plugin, CLAP_WINDOW_API_X11.as_ptr(), false) } {
            return None;
        }
        let create = gui.create?;
        if !unsafe { create(plugin, CLAP_WINDOW_API_X11.as_ptr(), false) } {
            return None;
        }
        let mut width = 0u32;
        let mut height = 0u32;
        let size_ok = gui
            .get_size
            .map(|f| unsafe { f(plugin, &mut width, &mut height) })
            .unwrap_or(false);
        let can_resize = gui
            .can_resize
            .map(|f| unsafe { f(plugin) })
            .unwrap_or(false);
        if let Some(destroy) = gui.destroy {
            unsafe { destroy(plugin) };
        }
        if !size_ok {
            return None;
        }
        Some(super::PluginGuiCapability {
            api: CLAP_WINDOW_API_X11.to_string_lossy().into_owned(),
            floating: false,
            width,
            height,
            can_resize,
        })
    }

    /// Turns a `.clap` file or bundle directory into the shared object to load.
    fn resolve_shared_object(path: &str) -> Result<PathBuf, String> {
        let p = PathBuf::from(path);
        if p.is_dir() {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let mut fallback: Option<PathBuf> = None;
            let entries = std::fs::read_dir(&p)
                .map_err(|e| crate::tstatus!("Kunde inte läsa CLAP-paketet '{}': {}", p.display(), e))?;
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate.extension().and_then(|x| x.to_str()) != Some("so") {
                    continue;
                }
                if candidate.file_stem().and_then(|s| s.to_str()) == Some(stem) {
                    return Ok(candidate);
                }
                if fallback.is_none() {
                    fallback = Some(candidate);
                }
            }
            fallback.ok_or_else(|| {
                crate::tstatus!("CLAP-paketet '{}' saknar en .so-binär", p.display())
            })
        } else if p.exists() {
            Ok(p)
        } else {
            Err(crate::tstatus!("Filen finns inte: {}", p.display()))
        }
    }

    unsafe fn cstr(ptr: *const c_char) -> String {
        if ptr.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    unsafe fn read_features(mut ptr: *const *const c_char) -> Vec<String> {
        let mut out = Vec::new();
        if ptr.is_null() {
            return out;
        }
        // The array is NULL-terminated; guard against a runaway loop.
        for _ in 0..1024 {
            let item = unsafe { *ptr };
            if item.is_null() {
                break;
            }
            out.push(unsafe { cstr(item) });
            ptr = unsafe { ptr.add(1) };
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

    unsafe fn gather_parameters(plugin: *const ClapPlugin) -> Vec<PluginParameter> {
        let mut out = Vec::new();
        let Some(get_extension) = (unsafe { &*plugin }).get_extension else {
            return out;
        };
        let ext = unsafe { get_extension(plugin, CLAP_EXT_PARAMS.as_ptr()) };
        if ext.is_null() {
            return out;
        }
        let params = unsafe { &*(ext as *const ClapPluginParams) };
        let Some(count) = params.count else {
            return out;
        };
        let Some(get_info) = params.get_info else {
            return out;
        };
        let total = unsafe { count(plugin) };
        for index in 0..total {
            let mut info: ClapParamInfo = unsafe { std::mem::zeroed() };
            if !unsafe { get_info(plugin, index, &mut info) } {
                continue;
            }
            out.push(PluginParameter {
                id: info.id,
                name: fixed_cstr(&info.name),
                module: fixed_cstr(&info.module),
                min_value: info.min_value,
                max_value: info.max_value,
                default_value: info.default_value,
                flags: info.flags,
            });
        }
        out
    }

    /// Loads and inspects the first plugin exposed by a CLAP bundle.
    pub fn load(path: &str) -> Result<Box<dyn PluginInstance>, String> {
        Ok(Box::new(create_instance(path)?))
    }

    /// Loads the first plugin in a CLAP bundle, keeping the live instance
    /// (handle, vtable, host and descriptor) alive.
    fn create_instance(path: &str) -> Result<ClapInstance, String> {
        let so = resolve_shared_object(path)?;

        let lib = unsafe { Library::new(&so) }
            .map_err(|e| crate::tstatus!("Kunde inte öppna '{}': {}", so.display(), e.to_string()))?;

        let entry_ptr: *const ClapPluginEntry = unsafe {
            // `dlsym` returns the address of the `clap_entry` struct, so we
            // fetch it as a raw pointer (pointer-sized) and cast.
            let symbol = lib
                .get::<*mut c_void>(b"clap_entry\0")
                .map_err(|e| crate::tstatus!("'{}' exporterar ingen 'clap_entry': {}", so.display(), e.to_string()))?;
            *symbol as *const ClapPluginEntry
        };
        let entry = unsafe { &*entry_ptr };

        if entry.clap_version.major != CLAP_VERSION_MAJOR {
            return Err(crate::tstatus!(
                "'{}' använder CLAP {}.x som inte stöds (hosten kör 1.x)",
                so.display(),
                entry.clap_version.major
            ));
        }
        let Some(init) = entry.init else {
            return Err(crate::tstatus!("'{}' saknar entry.init()", so.display()));
        };
        let path_c = CString::new(so.to_string_lossy().as_bytes())
            .map_err(|_| crate::i18n::t("Ogiltig sökväg (innehåller NUL)").to_string())?;
        if !unsafe { init(path_c.as_ptr()) } {
            return Err(crate::tstatus!("'{}' entry.init() misslyckades", so.display()));
        }

        // From here on the library is initialised: keep it in a guard so any
        // early return still calls deinit() before unloading.
        let handle = LibraryHandle {
            _lib: lib,
            entry: entry_ptr,
            initialized: true,
        };

        let host = Box::new(ClapHost {
            clap_version: ClapVersion {
                major: 1,
                minor: 2,
                revision: 0,
            },
            host_data: std::ptr::null_mut(),
            name: HOST_NAME.as_ptr(),
            vendor: HOST_VENDOR.as_ptr(),
            url: HOST_URL.as_ptr(),
            version: HOST_VERSION.as_ptr(),
            get_extension: Some(host_get_extension),
            request_restart: Some(host_request_restart),
            request_process: Some(host_request_process),
            request_callback: Some(host_request_callback),
        });

        let result = load_from_entry(entry, &host, &so);
        match result {
            Ok((plugin, info, params)) => Ok(ClapInstance {
                handle,
                plugin,
                _host: host,
                info,
                params,
            }),
            Err(err) => {
                // `handle` drops here, calling deinit() while the lib is loaded.
                drop(handle);
                Err(err)
            }
        }
    }

    fn load_from_entry(
        entry: &ClapPluginEntry,
        host: &ClapHost,
        so: &Path,
    ) -> Result<(*const ClapPlugin, PluginInfo, Vec<PluginParameter>), String> {
        let Some(get_factory) = entry.get_factory else {
            return Err(crate::tstatus!("'{}' saknar entry.get_factory()", so.display()));
        };
        let factory =
            unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) } as *const ClapPluginFactory;
        if factory.is_null() {
            return Err(crate::tstatus!(
                "'{}' exponerar ingen '{}'",
                so.display(),
                CLAP_PLUGIN_FACTORY_ID.to_string_lossy().into_owned()
            ));
        }
        let factory_ref = unsafe { &*factory };

        let Some(get_count) = factory_ref.get_plugin_count else {
            return Err(crate::i18n::t("CLAP-fabriken saknar get_plugin_count()").to_string());
        };
        let count = unsafe { get_count(factory) };
        if count == 0 {
            return Err(crate::tstatus!("'{}' innehåller inga plugins", so.display()));
        }

        let Some(get_descriptor) = factory_ref.get_plugin_descriptor else {
            return Err(
                crate::i18n::t("CLAP-fabriken saknar get_plugin_descriptor()").to_string(),
            );
        };
        let desc_ptr = unsafe { get_descriptor(factory, 0) };
        if desc_ptr.is_null() {
            return Err(crate::tstatus!("'{}' gav ingen plugin-beskrivning", so.display()));
        }
        let desc = unsafe { &*desc_ptr };
        let info = PluginInfo {
            id: unsafe { cstr(desc.id) },
            name: unsafe { cstr(desc.name) },
            vendor: unsafe { cstr(desc.vendor) },
            version: unsafe { cstr(desc.version) },
            description: unsafe { cstr(desc.description) },
            features: unsafe { read_features(desc.features) },
        };

        let Some(create) = factory_ref.create_plugin else {
            return Err(crate::i18n::t("CLAP-fabriken saknar create_plugin()").to_string());
        };
        let plugin = unsafe { create(factory, host as *const ClapHost, desc.id) };
        if plugin.is_null() {
            return Err(crate::tstatus!(
                "Kunde inte skapa plugin-instansen '{}'",
                info.id
            ));
        }

        if let Some(init_plugin) = unsafe { &*plugin }.init
            && !unsafe { init_plugin(plugin) }
        {
            if let Some(destroy) = unsafe { &*plugin }.destroy {
                unsafe { destroy(plugin) };
            }
            return Err(crate::tstatus!("'{}' init() misslyckades", info.name));
        }

        let params = unsafe { gather_parameters(plugin) };
        Ok((plugin, info, params))
    }

    // =======================================================================
    // Live processing (Fas 4.2)
    // =======================================================================

    unsafe extern "C" fn empty_events_size(_list: *const ClapInputEvents) -> u32 {
        0
    }
    unsafe extern "C" fn empty_events_get(
        _list: *const ClapInputEvents,
        _index: u32,
    ) -> *const ClapEventHeader {
        std::ptr::null()
    }

    unsafe extern "C" fn param_events_size(list: *const ClapInputEvents) -> u32 {
        let events = unsafe { &*((*list).ctx as *const Vec<ClapEventParamValue>) };
        events.len() as u32
    }
    unsafe extern "C" fn param_events_get(
        list: *const ClapInputEvents,
        index: u32,
    ) -> *const ClapEventHeader {
        let events = unsafe { &*((*list).ctx as *const Vec<ClapEventParamValue>) };
        match events.get(index as usize) {
            Some(e) => e as *const ClapEventParamValue as *const ClapEventHeader,
            None => std::ptr::null(),
        }
    }

    unsafe extern "C" fn output_events_try_push(
        _list: *const ClapOutputEvents,
        _event: *const ClapEventHeader,
    ) -> bool {
        false
    }

    fn empty_input_events() -> ClapInputEvents {
        ClapInputEvents {
            ctx: std::ptr::null_mut(),
            size: Some(empty_events_size),
            get: Some(empty_events_get),
        }
    }

    fn empty_output_events() -> ClapOutputEvents {
        ClapOutputEvents {
            ctx: std::ptr::null_mut(),
            try_push: Some(output_events_try_push),
        }
    }

    /// Channel counts of the plugin's audio ports (input, output). Falls back
    /// to a single stereo pair when the extension is missing.
    unsafe fn audio_port_channels(plugin: *const ClapPlugin) -> (Vec<u32>, Vec<u32>) {
        let get_extension = unsafe { &*plugin }.get_extension;
        let Some(get_extension) = get_extension else {
            return (vec![2], vec![2]);
        };
        let ext = unsafe { get_extension(plugin, CLAP_EXT_AUDIO_PORTS.as_ptr()) };
        if ext.is_null() {
            return (vec![2], vec![2]);
        }
        let ports = unsafe { &*(ext as *const ClapPluginAudioPorts) };
        let (Some(count), Some(get)) = (ports.count, ports.get) else {
            return (vec![2], vec![2]);
        };
        let read = |is_input: bool| -> Vec<u32> {
            let total = unsafe { count(plugin, is_input) };
            let mut out = Vec::new();
            for index in 0..total {
                let mut info: ClapAudioPortInfo = unsafe { std::mem::zeroed() };
                if unsafe { get(plugin, index, is_input, &mut info) } {
                    out.push(info.channel_count.max(1));
                }
            }
            out
        };
        let inputs = read(true);
        let outputs = read(false);
        (inputs, outputs)
    }

    fn allocate_audio(
        counts: &[u32],
        max_frames: u32,
    ) -> (Vec<Vec<Vec<f32>>>, Vec<Vec<*mut f32>>, Vec<ClapAudioBuffer>) {
        let mut bufs: Vec<Vec<Vec<f32>>> = counts
            .iter()
            .map(|&c| {
                (0..c.max(1))
                    .map(|_| vec![0.0_f32; max_frames as usize])
                    .collect()
            })
            .collect();
        let mut ptrs: Vec<Vec<*mut f32>> = bufs
            .iter_mut()
            .map(|port| port.iter_mut().map(|ch| ch.as_mut_ptr()).collect())
            .collect();
        let audio: Vec<ClapAudioBuffer> = ptrs
            .iter_mut()
            .map(|port| ClapAudioBuffer {
                data32: port.as_mut_ptr(),
                data64: std::ptr::null_mut(),
                channel_count: port.len() as u32,
                latency: 0,
                constant_mask: 0,
            })
            .collect();
        (bufs, ptrs, audio)
    }

    /// Shared, thread-safe plugin instance. The audio thread calls
    /// `process`/`set_parameter`; the main thread calls state/GUI methods.
    /// CLAP's contract explicitly allows that split, so the instance is shared
    /// behind an `Arc` instead of being duplicated.
    #[allow(dead_code)]
    struct ClapCore {
        instance: ClapInstance,
        sample_rate: f64,
        max_frames: u32,
        activated: bool,
        processing: bool,
        latency_frames: u32,
        params_ext: *const ClapPluginParams,
        state_ext: *const ClapPluginState,
        preset_load_ext: *const ClapPluginPresetLoad,
        gui_ext: *const ClapPluginGui,
        /// Whether `clap.gui` create() succeeded and destroy() has not run.
        gui_created: AtomicBool,
    }

    // CLAP hosts may call `process` from the audio thread while calling
    // GUI/state methods from the main thread; plugins must be safe under that
    // split. The only host-side shared state is the atomic `gui_created` flag.
    unsafe impl Send for ClapCore {}
    unsafe impl Sync for ClapCore {}

    impl ClapCore {
        fn plugin(&self) -> *const ClapPlugin {
            self.instance.plugin
        }
    }

    impl Drop for ClapCore {
        fn drop(&mut self) {
            // Tear the GUI down first; the CLAP contract wants destroy() before
            // the plugin itself is destroyed.
            super::PluginCore::gui_destroy(self);
            unsafe {
                let plugin = self.plugin();
                if self.processing
                    && let Some(stop) = (*plugin).stop_processing
                {
                    stop(plugin);
                }
                if self.activated
                    && let Some(deactivate) = (*plugin).deactivate
                {
                    deactivate(plugin);
                }
            }
            // `instance` drops afterwards, calling destroy() + deinit().
        }
    }

    impl super::PluginCore for ClapCore {
        fn info(&self) -> &PluginInfo {
            &self.instance.info
        }
        fn latency_frames(&self) -> u32 {
            self.latency_frames
        }
        fn set_parameter(&self, id: u32, value: f64) -> bool {
            if self.params_ext.is_null() {
                return false;
            }
            let params = unsafe { &*self.params_ext };
            let Some(flush) = params.flush else {
                return false;
            };
            let mut events: Vec<ClapEventParamValue> = vec![ClapEventParamValue {
                header: ClapEventHeader {
                    size: std::mem::size_of::<ClapEventParamValue>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    event_type: CLAP_EVENT_PARAM_VALUE,
                    flags: 0,
                },
                param_id: id,
                cookie: std::ptr::null_mut(),
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value,
            }];
            let in_events = ClapInputEvents {
                ctx: &mut events as *mut Vec<ClapEventParamValue> as *mut c_void,
                size: Some(param_events_size),
                get: Some(param_events_get),
            };
            let out_events = empty_output_events();
            unsafe {
                flush(
                    self.plugin(),
                    &in_events as *const ClapInputEvents as *const c_void,
                    &out_events as *const ClapOutputEvents as *const c_void,
                );
            }
            true
        }
        fn reset(&self) {
            unsafe {
                if let Some(reset) = (*self.plugin()).reset {
                    reset(self.plugin());
                }
            }
        }
        fn save_state(&self) -> Vec<u8> {
            if self.state_ext.is_null() {
                return Vec::new();
            }
            let state = unsafe { &*self.state_ext };
            let Some(save) = state.save else {
                return Vec::new();
            };
            let mut out: Vec<u8> = Vec::new();
            let ostream = ClapOstream {
                ctx: &mut out as *mut Vec<u8> as *mut c_void,
                write: Some(ostream_vec_write),
            };
            let ok = unsafe { save(self.plugin(), &ostream) };
            if ok { out } else { Vec::new() }
        }
        fn load_state(&self, data: &[u8]) -> bool {
            if self.state_ext.is_null() {
                return false;
            }
            let state = unsafe { &*self.state_ext };
            let Some(load) = state.load else {
                return false;
            };
            let mut reader = StateReader {
                data,
                pos: 0,
            };
            let istream = ClapIstream {
                ctx: &mut reader as *mut StateReader as *mut c_void,
                read: Some(istream_slice_read),
            };
            unsafe { load(self.plugin(), &istream) }
        }
        fn preset_load(&self, location: &str) -> bool {
            if self.preset_load_ext.is_null() {
                return false;
            }
            let ext = unsafe { &*self.preset_load_ext };
            let Some(from_location) = ext.from_location else {
                return false;
            };
            let Ok(loc) = CString::new(location) else {
                return false;
            };
            unsafe {
                from_location(
                    self.plugin(),
                    CLAP_PRESET_LOCATION_FILE,
                    loc.as_ptr(),
                    std::ptr::null(),
                )
            }
        }
        fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool {
            if self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            let Some(is_supported) = gui.is_api_supported else {
                return false;
            };
            let Ok(api) = CString::new(api) else {
                return false;
            };
            unsafe { is_supported(self.plugin(), api.as_ptr(), is_floating) }
        }
        fn gui_preferred_api(&self) -> Option<(String, bool)> {
            if self.gui_ext.is_null() {
                return None;
            }
            let gui = unsafe { &*self.gui_ext };
            let get = gui.get_preferred_api?;
            let mut api_ptr: *const c_char = std::ptr::null();
            let mut floating = false;
            if !unsafe { get(self.plugin(), &mut api_ptr, &mut floating) } || api_ptr.is_null() {
                return None;
            }
            let api = unsafe { CStr::from_ptr(api_ptr) }.to_string_lossy().into_owned();
            Some((api, floating))
        }
        fn gui_get_size(&self) -> Option<(u32, u32)> {
            if self.gui_ext.is_null() {
                return None;
            }
            let gui = unsafe { &*self.gui_ext };
            let get = gui.get_size?;
            let mut width = 0u32;
            let mut height = 0u32;
            if unsafe { get(self.plugin(), &mut width, &mut height) } {
                Some((width, height))
            } else {
                None
            }
        }
        fn gui_can_resize(&self) -> bool {
            if self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            gui.can_resize
                .map(|f| unsafe { f(self.plugin()) })
                .unwrap_or(false)
        }
        fn gui_is_created(&self) -> bool {
            self.gui_created.load(Ordering::SeqCst)
        }
        fn gui_create(&self, api: &str, is_floating: bool) -> bool {
            if self.gui_created.load(Ordering::SeqCst) {
                return true;
            }
            if self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            let Some(create) = gui.create else {
                return false;
            };
            let Ok(api) = CString::new(api) else {
                return false;
            };
            let ok = unsafe { create(self.plugin(), api.as_ptr(), is_floating) };
            if ok {
                self.gui_created.store(true, Ordering::SeqCst);
            }
            ok
        }
        fn gui_set_parent(&self, x11_window: u64) -> bool {
            if !self.gui_created.load(Ordering::SeqCst) || self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            let Some(set_parent) = gui.set_parent else {
                return false;
            };
            let window = ClapWindow {
                api: CLAP_WINDOW_API_X11.as_ptr(),
                x11: x11_window as usize,
            };
            unsafe { set_parent(self.plugin(), &window) }
        }
        fn gui_set_size(&self, width: u32, height: u32) -> bool {
            if !self.gui_created.load(Ordering::SeqCst) || self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            let Some(set_size) = gui.set_size else {
                return false;
            };
            unsafe { set_size(self.plugin(), width, height) }
        }
        fn gui_show(&self) -> bool {
            if !self.gui_created.load(Ordering::SeqCst) || self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            gui.show
                .map(|f| unsafe { f(self.plugin()) })
                .unwrap_or(false)
        }
        fn gui_hide(&self) -> bool {
            if !self.gui_created.load(Ordering::SeqCst) || self.gui_ext.is_null() {
                return false;
            }
            let gui = unsafe { &*self.gui_ext };
            gui.hide
                .map(|f| unsafe { f(self.plugin()) })
                .unwrap_or(false)
        }
        fn gui_destroy(&self) {
            if !self.gui_created.swap(false, Ordering::SeqCst) {
                return;
            }
            if self.gui_ext.is_null() {
                return;
            }
            let gui = unsafe { &*self.gui_ext };
            if let Some(destroy) = gui.destroy {
                unsafe { destroy(self.plugin()) };
            }
        }
    }

    #[allow(dead_code)]
    pub struct ClapProcessor {
        /// Shared with the main thread (GUI/state); see [`ClapCore`].
        core: Arc<ClapCore>,
        input_channels: Vec<u32>,
        output_channels: Vec<u32>,
        in_bufs: Vec<Vec<Vec<f32>>>,
        out_bufs: Vec<Vec<Vec<f32>>>,
        /// Owns the per-port channel-pointer arrays that `in_audio`/`out_audio`
        /// point into; must outlive them.
        in_ptrs: Vec<Vec<*mut f32>>,
        out_ptrs: Vec<Vec<*mut f32>>,
        in_audio: Vec<ClapAudioBuffer>,
        out_audio: Vec<ClapAudioBuffer>,
    }

    // The audio buffers are only ever touched from the audio thread; the shared
    // `Arc<ClapCore>` is `Send + Sync`.
    unsafe impl Send for ClapProcessor {}

    impl super::PluginProcessor for ClapProcessor {
        fn backend(&self) -> &'static str {
            "CLAP"
        }
        fn info(&self) -> &PluginInfo {
            super::PluginCore::info(&*self.core)
        }
        fn parameters(&self) -> &[PluginParameter] {
            &self.core.instance.params
        }
        fn latency_frames(&self) -> u32 {
            super::PluginCore::latency_frames(&*self.core)
        }
        fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
            let frames = left.len().min(right.len());
            if frames == 0 {
                return;
            }
            let frames = frames.min(self.core.max_frames as usize);

            // Fill input port 0 (and silence any additional input ports).
            for (port, bufs) in self.in_bufs.iter_mut().enumerate() {
                if port == 0 {
                    for (ch, buf) in bufs.iter_mut().enumerate() {
                        for (i, s) in buf.iter_mut().enumerate().take(frames) {
                            *s = match ch {
                                0 => left[i],
                                1 => right[i],
                                _ => 0.0,
                            };
                        }
                    }
                } else {
                    for buf in bufs.iter_mut() {
                        for s in buf.iter_mut().take(frames) {
                            *s = 0.0;
                        }
                    }
                }
            }

            let in_events = empty_input_events();
            let out_events = empty_output_events();
            let process = ClapProcess {
                steady_time: -1,
                frames_count: frames as u32,
                transport: std::ptr::null(),
                audio_inputs: self.in_audio.as_ptr(),
                audio_outputs: self.out_audio.as_mut_ptr(),
                audio_inputs_count: self.in_audio.len() as u32,
                audio_outputs_count: self.out_audio.len() as u32,
                in_events: &in_events,
                out_events: &out_events,
            };

            unsafe {
                if let Some(process_fn) = (*self.core.plugin()).process {
                    process_fn(
                        self.core.plugin(),
                        &process as *const ClapProcess as *const c_void,
                    );
                }
            }

            // Read back output port 0; duplicate channel 0 for mono outputs.
            if let Some(bufs) = self.out_bufs.first() {
                let (l_buf, r_buf) = match bufs.len() {
                    0 => (None, None),
                    1 => (Some(&bufs[0]), None),
                    _ => (Some(&bufs[0]), Some(&bufs[1])),
                };
                for i in 0..frames {
                    left[i] = l_buf.map(|b| b[i]).unwrap_or(0.0);
                    right[i] = r_buf
                        .map(|b| b[i])
                        .unwrap_or_else(|| l_buf.map(|b| b[i]).unwrap_or(0.0));
                }
            } else {
                for i in 0..frames {
                    left[i] = 0.0;
                    right[i] = 0.0;
                }
            }
        }
        fn set_parameter(&mut self, id: u32, value: f64) -> bool {
            super::PluginCore::set_parameter(&*self.core, id, value)
        }
        fn reset(&mut self) {
            super::PluginCore::reset(&*self.core);
        }
        fn save_state(&mut self) -> Vec<u8> {
            super::PluginCore::save_state(&*self.core)
        }
        fn load_state(&mut self, data: &[u8]) -> bool {
            super::PluginCore::load_state(&*self.core, data)
        }
        fn preset_load(&mut self, location: &str) -> bool {
            super::PluginCore::preset_load(&*self.core, location)
        }
        fn gui_is_api_supported(&self, api: &str, is_floating: bool) -> bool {
            super::PluginCore::gui_is_api_supported(&*self.core, api, is_floating)
        }
        fn gui_preferred_api(&self) -> Option<(String, bool)> {
            super::PluginCore::gui_preferred_api(&*self.core)
        }
        fn gui_get_size(&self) -> Option<(u32, u32)> {
            super::PluginCore::gui_get_size(&*self.core)
        }
        fn gui_can_resize(&self) -> bool {
            super::PluginCore::gui_can_resize(&*self.core)
        }
        fn gui_is_created(&self) -> bool {
            super::PluginCore::gui_is_created(&*self.core)
        }
        fn gui_create(&mut self, api: &str, is_floating: bool) -> bool {
            super::PluginCore::gui_create(&*self.core, api, is_floating)
        }
        fn gui_set_parent(&mut self, x11_window: u64) -> bool {
            super::PluginCore::gui_set_parent(&*self.core, x11_window)
        }
        fn gui_set_size(&mut self, width: u32, height: u32) -> bool {
            super::PluginCore::gui_set_size(&*self.core, width, height)
        }
        fn gui_show(&mut self) -> bool {
            super::PluginCore::gui_show(&*self.core)
        }
        fn gui_hide(&mut self) -> bool {
            super::PluginCore::gui_hide(&*self.core)
        }
        fn gui_destroy(&mut self) {
            super::PluginCore::gui_destroy(&*self.core)
        }
        fn core_handle(&self) -> Option<super::PluginHandle> {
            Some(super::PluginHandle::new(self.core.clone()))
        }
    }

    /// Cursor over a state blob handed to `clap.state.load`.
    struct StateReader<'a> {
        data: &'a [u8],
        pos: usize,
    }

    unsafe extern "C" fn ostream_vec_write(
        stream: *const ClapOstream,
        buffer: *const c_void,
        size: u64,
    ) -> i64 {
        if stream.is_null() || (buffer.is_null() && size > 0) {
            return -1;
        }
        let stream = unsafe { &*stream };
        if stream.ctx.is_null() {
            return -1;
        }
        let out = unsafe { &mut *(stream.ctx as *mut Vec<u8>) };
        let bytes = unsafe { std::slice::from_raw_parts(buffer as *const u8, size as usize) };
        out.extend_from_slice(bytes);
        size as i64
    }

    unsafe extern "C" fn istream_slice_read(
        stream: *const ClapIstream,
        buffer: *mut c_void,
        size: u64,
    ) -> i64 {
        if stream.is_null() || (buffer.is_null() && size > 0) {
            return -1;
        }
        let stream = unsafe { &*stream };
        if stream.ctx.is_null() {
            return -1;
        }
        let reader = unsafe { &mut *(stream.ctx as *mut StateReader) };
        let remaining = reader.data.len().saturating_sub(reader.pos);
        let n = (size as usize).min(remaining);
        if n > 0 {
            let src = &reader.data[reader.pos..reader.pos + n];
            unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), buffer as *mut u8, n) };
            reader.pos += n;
        }
        n as i64
    }

    /// Loads `path` and returns a live, activated processor ready to render
    /// blocks of up to `max_frames` at `sample_rate`.
    pub fn load_processor(
        path: &str,
        sample_rate: f32,
        max_frames: u32,
    ) -> Result<Box<dyn super::PluginProcessor>, String> {
        let max_frames = max_frames.max(1);
        let instance = create_instance(path)?;
        let plugin = instance.plugin;

        let get_extension = unsafe { &*plugin }.get_extension;
        let params_ext = match get_extension {
            Some(f) => (unsafe { f(plugin, CLAP_EXT_PARAMS.as_ptr()) }) as *const ClapPluginParams,
            None => std::ptr::null(),
        };

        let state_ext = match get_extension {
            Some(f) => (unsafe { f(plugin, CLAP_EXT_STATE.as_ptr()) }) as *const ClapPluginState,
            None => std::ptr::null(),
        };

        let preset_load_ext = match get_extension {
            Some(f) => {
                let mut ext = unsafe { f(plugin, CLAP_EXT_PRESET_LOAD.as_ptr()) };
                if ext.is_null() {
                    ext = unsafe { f(plugin, CLAP_EXT_PRESET_LOAD_DRAFT.as_ptr()) };
                }
                ext as *const ClapPluginPresetLoad
            }
            None => std::ptr::null(),
        };

        let gui_ext = plugin_extension(plugin, CLAP_EXT_GUI) as *const ClapPluginGui;

        let latency_frames = match get_extension {
            Some(f) => {
                let ext = unsafe { f(plugin, CLAP_EXT_LATENCY.as_ptr()) };
                if ext.is_null() {
                    0
                } else {
                    let latency = unsafe { &*(ext as *const ClapPluginLatency) };
                    latency.get.map(|g| unsafe { g(plugin) }).unwrap_or(0)
                }
            }
            None => 0,
        };

        let (input_channels, output_channels) = unsafe { audio_port_channels(plugin) };
        let (in_bufs, in_ptrs, in_audio) = allocate_audio(&input_channels, max_frames);
        let (out_bufs, out_ptrs, out_audio) = allocate_audio(&output_channels, max_frames);

        let sample_rate_f = sample_rate.max(1.0) as f64;
        let activated = unsafe {
            match (*plugin).activate {
                Some(activate) => activate(plugin, sample_rate_f, 1, max_frames),
                None => false,
            }
        };
        if !activated {
            return Err(crate::tstatus!(
                "Kunde inte aktivera plugin-instansen '{}'",
                instance.info.name
            ));
        }
        let processing = unsafe {
            match (*plugin).start_processing {
                Some(start) => start(plugin),
                None => false,
            }
        };
        if !processing {
            unsafe {
                if let Some(deactivate) = (*plugin).deactivate {
                    deactivate(plugin);
                }
            }
            return Err(crate::tstatus!(
                "Kunde inte starta processning för '{}'",
                instance.info.name
            ));
        }

        Ok(Box::new(ClapProcessor {
            core: Arc::new(ClapCore {
                instance,
                sample_rate: sample_rate_f,
                max_frames,
                activated,
                processing,
                latency_frames,
                params_ext,
                state_ext,
                preset_load_ext,
                gui_ext,
                gui_created: AtomicBool::new(false),
            }),
            input_channels,
            output_channels,
            in_bufs,
            out_bufs,
            in_ptrs,
            out_ptrs,
            in_audio,
            out_audio,
        }))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn temp_dir(name: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(name);
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        #[test]
        fn resolves_so_inside_clap_bundle() {
            let dir = temp_dir("sonix_clap_bundle_test");
            let bundle = dir.join("SuperSynth.clap");
            std::fs::create_dir_all(&bundle).unwrap();
            std::fs::write(bundle.join("SuperSynth.so"), b"\x7fELF").unwrap();
            std::fs::write(bundle.join("readme.txt"), b"hi").unwrap();

            let resolved = resolve_shared_object(bundle.to_str().unwrap()).unwrap();
            assert_eq!(resolved.file_name().unwrap(), "SuperSynth.so");

            std::fs::remove_dir_all(&dir).unwrap();
        }

        #[test]
        fn resolves_plain_shared_object_file() {
            let dir = temp_dir("sonix_clap_file_test");
            let file = dir.join("plugin.clap");
            std::fs::write(&file, b"\x7fELF").unwrap();
            let resolved = resolve_shared_object(file.to_str().unwrap()).unwrap();
            assert_eq!(resolved, file);
            std::fs::remove_dir_all(&dir).unwrap();
        }

        #[test]
        fn rejects_missing_path() {
            let err = resolve_shared_object("/nonexistent/thing.clap").unwrap_err();
            assert!(err.contains("finns inte"));
        }

        #[test]
        fn rejects_bundle_without_binary() {
            let dir = temp_dir("sonix_clap_empty_bundle");
            let bundle = dir.join("Empty.clap");
            std::fs::create_dir_all(&bundle).unwrap();
            let err = resolve_shared_object(bundle.to_str().unwrap()).unwrap_err();
            assert!(err.contains("saknar"));
            std::fs::remove_dir_all(&dir).unwrap();
        }

        #[test]
        fn rejects_non_clap_library() {
            let Some(empty) = option_env!("SONIX_EMPTY_SO") else {
                return; // no C compiler at build time
            };
            let err = match load(empty) {
                Ok(_) => panic!("loading a non-CLAP library must fail"),
                Err(e) => e,
            };
            assert!(
                err.contains("clap_entry"),
                "expected a clap_entry error, got: {err}"
            );
        }

        #[test]
        fn loads_mock_plugin_and_reports_parameters() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return; // no C compiler at build time
            };
            let instance = load(mock).expect("mock CLAP plugin should load");

            assert_eq!(instance.backend(), "CLAP");
            let info = instance.info();
            assert_eq!(info.id, "com.sonix.mock-gain");
            assert_eq!(info.name, "Sonix Mock Gain");
            assert_eq!(info.vendor, "Sonix Test");
            assert_eq!(info.version, "1.0.0");
            assert!(info.features.iter().any(|f| f == "audio-effect"));

            let params = instance.parameters();
            assert_eq!(params.len(), 2, "mock plugin reports two parameters");
            assert_eq!(params[0].name, "Gain");
            assert_eq!(params[0].default_value, 1.0);
            assert!(params[0].is_automatable());
            assert!(!params[0].is_stepped());
            assert_eq!(params[1].name, "Mix");
            assert!(params[1].is_stepped());
            // Dropping the instance must destroy + deinit + unload cleanly.
            drop(instance);
        }

        #[test]
        fn inspect_snapshot_matches_load() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let snapshot = super::super::inspect(mock);
            assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
            assert_eq!(snapshot.backend, "CLAP");
            assert_eq!(snapshot.parameters.len(), 2);
            assert_eq!(snapshot.info.unwrap().name, "Sonix Mock Gain");
        }

        #[test]
        fn loads_processor_and_reports_ports_and_latency() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let processor = super::super::load_processor(mock, 48_000.0, 512)
                .expect("mock processor should load");
            assert_eq!(processor.backend(), "CLAP");
            assert_eq!(processor.info().name, "Sonix Mock Gain");
            assert_eq!(processor.latency_frames(), 0);
            assert_eq!(processor.parameters().len(), 2);
        }

        #[test]
        fn shared_core_outlives_the_processor() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");
            let insert = super::super::PluginInsert::new(processor, 128);
            let handle = insert
                .core_handle()
                .expect("CLAP inserts expose a shared core");
            assert_eq!(handle.info().name, "Sonix Mock Gain");
            drop(insert);
            // The audio-thread processor is gone, but the main-thread handle
            // keeps the shared ClapCore alive (Fas 4.4b invariant).
            assert_eq!(handle.info().name, "Sonix Mock Gain");
            assert_eq!(handle.latency_frames(), 0);
        }

        #[test]
        fn mock_plugin_processes_audio_through_the_abi() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let mut processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");

            // Default settings are transparent: output == input.
            let mut l = vec![0.5_f32; 256];
            let mut r = vec![-0.25_f32; 256];
            processor.process_stereo(&mut l, &mut r);
            assert!((l[0] - 0.5).abs() < 1e-6, "unity gain expected, got {}", l[0]);
            assert!((r[0] + 0.25).abs() < 1e-6, "unity gain expected, got {}", r[0]);

            // "Gain" (id 0) at 0.5 must halve the signal.
            assert!(processor.set_parameter(0, 0.5));
            let mut l = vec![0.8_f32; 128];
            let mut r = vec![0.4_f32; 128];
            processor.process_stereo(&mut l, &mut r);
            assert!((l[0] - 0.4).abs() < 1e-5, "expected 0.4, got {}", l[0]);
            assert!((r[0] - 0.2).abs() < 1e-5, "expected 0.2, got {}", r[0]);
        }

        #[test]
        fn mock_plugin_state_round_trips() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let mut processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");
            assert!(processor.set_parameter(0, 0.25));
            assert!(processor.set_parameter(1, 1.0));
            let blob = processor.save_state();
            assert_eq!(blob.len(), 16, "two f64 parameters");

            // Mutate the live state, then restore the saved blob.
            assert!(processor.set_parameter(0, 1.0));
            assert!(processor.load_state(&blob));

            let mut l = vec![1.0_f32; 64];
            let mut r = vec![1.0_f32; 64];
            processor.process_stereo(&mut l, &mut r);
            assert!(
                (l[0] - 0.25).abs() < 1e-5,
                "restored gain should be 0.25, got {}",
                l[0]
            );
        }

        #[test]
        fn mock_plugin_loads_native_preset() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let mut processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");
            assert!(processor.preset_load("gain=0.5;mix=1.0"));

            let mut l = vec![1.0_f32; 64];
            let mut r = vec![1.0_f32; 64];
            processor.process_stereo(&mut l, &mut r);
            assert!(
                (l[0] - 0.5).abs() < 1e-5,
                "preset gain 0.5 expected, got {}",
                l[0]
            );
        }

        #[test]
        fn mock_plugin_reports_x11_gui_capability() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");

            assert!(processor.gui_is_api_supported("x11", false));
            assert!(!processor.gui_is_api_supported("win32", false));
            assert_eq!(processor.gui_preferred_api(), Some(("x11".to_string(), false)));
            assert!(processor.gui_can_resize());
            assert!(!processor.gui_is_created());
            // `get_size` is only valid once the GUI has been created.
            assert_eq!(processor.gui_get_size(), None);
        }

        #[test]
        fn mock_plugin_gui_lifecycle_round_trips() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let mut processor =
                super::super::load_processor(mock, 48_000.0, 512).expect("mock processor");

            assert!(processor.gui_create("x11", false));
            assert!(processor.gui_is_created());
            assert_eq!(processor.gui_get_size(), Some((320, 240)));

            assert!(processor.gui_set_size(400, 300));
            assert_eq!(processor.gui_get_size(), Some((400, 300)));

            // A real X11 window id is accepted; the null window is rejected.
            assert!(processor.gui_set_parent(0x1234_5678));
            assert!(!processor.gui_set_parent(0));

            assert!(processor.gui_show());
            assert!(processor.gui_hide());

            processor.gui_destroy();
            assert!(!processor.gui_is_created());
            assert_eq!(processor.gui_get_size(), None);
        }

        #[test]
        fn inspect_reports_gui_capability() {
            let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
                return;
            };
            let snapshot = super::super::inspect(mock);
            assert!(snapshot.error.is_none(), "{:?}", snapshot.error);
            let gui = snapshot.gui.expect("mock plugin advertises clap.gui");
            assert_eq!(gui.api, "x11");
            assert!(!gui.floating);
            assert_eq!((gui.width, gui.height), (320, 240));
            assert!(gui.can_resize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_flag_helpers_read_the_right_bits() {
        let p = PluginParameter {
            id: 7,
            name: "Cutoff".into(),
            module: "Filter".into(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.5,
            flags: PARAM_IS_STEPPED | PARAM_IS_AUTOMATABLE,
        };
        assert!(p.is_stepped());
        assert!(p.is_automatable());
        assert!(!p.is_bypass());
        assert!(!p.is_readonly());
    }

    #[test]
    fn inspect_missing_file_is_reported_not_panicked() {
        let snapshot = inspect("/definitely/not/here.clap");
        // Either the feature is off (build message) or the file is missing.
        assert!(snapshot.error.is_some());
    }

    struct FakeProcessor {
        info: PluginInfo,
        params: Vec<PluginParameter>,
        gain: f32,
        latency: u32,
    }

    impl FakeProcessor {
        fn new(gain: f32, latency: u32) -> Self {
            Self {
                info: PluginInfo {
                    id: "fake".into(),
                    name: "Fake".into(),
                    ..Default::default()
                },
                params: Vec::new(),
                gain,
                latency,
            }
        }
    }

    impl PluginProcessor for FakeProcessor {
        fn backend(&self) -> &'static str {
            "Fake"
        }
        fn info(&self) -> &PluginInfo {
            &self.info
        }
        fn parameters(&self) -> &[PluginParameter] {
            &self.params
        }
        fn latency_frames(&self) -> u32 {
            self.latency
        }
        fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
            for s in left.iter_mut() {
                *s *= self.gain;
            }
            for s in right.iter_mut() {
                *s *= self.gain;
            }
        }
        fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
            true
        }
        fn reset(&mut self) {}
    }

    #[test]
    fn insert_delays_audio_by_its_reported_latency() {
        let insert = PluginInsert::new(Box::new(FakeProcessor::new(1.0, 0)), 4);
        assert_eq!(insert.latency_frames(), 3);
        let mut insert = insert;
        let mut out = Vec::new();
        for _ in 0..8 {
            out.push(insert.process_sample(1.0, 1.0).0);
        }
        // First three samples are silence while the first block fills.
        assert_eq!(&out[0..3], &[0.0, 0.0, 0.0]);
        assert!((out[3] - 1.0).abs() < 1e-6);
        assert!((out[7] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn insert_applies_processor_gain() {
        let mut insert = PluginInsert::new(Box::new(FakeProcessor::new(2.0, 0)), 4);
        let mut last = 0.0;
        for _ in 0..8 {
            last = insert.process_sample(1.0, 1.0).0;
        }
        assert!((last - 2.0).abs() < 1e-6, "expected 2.0, got {last}");
    }

    #[test]
    fn insert_reports_plugin_latency_on_top_of_buffering() {
        let insert = PluginInsert::new(Box::new(FakeProcessor::new(1.0, 10)), 4);
        assert_eq!(insert.plugin_latency_frames(), 10);
        assert_eq!(insert.latency_frames(), 13);
    }

    #[test]
    fn pdc_delay_shifts_by_the_requested_frames() {
        let mut delay = PdcDelay::new();
        delay.set_delay(2);
        let out: Vec<f32> = (1..=4)
            .map(|i| delay.process(i as f32, i as f32).0)
            .collect();
        assert_eq!(out, vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn pdc_delay_of_zero_is_a_passthrough() {
        let mut delay = PdcDelay::new();
        assert_eq!(delay.process(0.5, -0.5), (0.5, -0.5));
    }

    #[test]
    fn insert_reset_clears_pending_output() {
        let mut insert = PluginInsert::new(Box::new(FakeProcessor::new(1.0, 0)), 4);
        for _ in 0..4 {
            insert.process_sample(1.0, 1.0);
        }
        insert.reset();
        // After a reset the pipeline is empty again: silence until it refills.
        assert_eq!(insert.process_sample(1.0, 1.0), (0.0, 0.0));
    }

    #[test]
    #[cfg(not(feature = "plugin-host"))]
    fn load_processor_without_feature_is_an_honest_error() {
        let err = match load_processor("whatever.clap", 48_000.0, 128) {
            Ok(_) => panic!("expected an error without the plugin-host feature"),
            Err(e) => e,
        };
        assert!(err.contains("plugin-host"));
    }
}
