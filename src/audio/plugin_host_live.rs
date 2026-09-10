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
    pub id: u64,
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

/// Backend-agnostic handle to a live plugin instance.
///
/// Future backends (VST3, LV2) will implement the same trait.
#[allow(dead_code)]
pub trait PluginInstance {
    /// Human-readable backend name, e.g. `"CLAP"`.
    fn backend(&self) -> &'static str;
    fn info(&self) -> &PluginInfo;
    fn parameters(&self) -> &[PluginParameter];
}

/// Serializable snapshot of an inspection, safe to keep in the UI state.
#[derive(Debug, Clone)]
pub struct PluginInspection {
    pub path: String,
    pub backend: String,
    pub info: Option<PluginInfo>,
    pub parameters: Vec<PluginParameter>,
    pub error: Option<String>,
}

impl PluginInspection {
    fn failed(path: &str, backend: &str, error: String) -> Self {
        Self {
            path: path.to_string(),
            backend: backend.to_string(),
            info: None,
            parameters: Vec::new(),
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
        match imp::load(path) {
            Ok(instance) => PluginInspection {
                path: path.to_string(),
                backend: instance.backend().to_string(),
                info: Some(instance.info().clone()),
                parameters: instance.parameters().to_vec(),
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
// Real CLAP ABI + loader (only when the feature is enabled)
// ===========================================================================
#[cfg(feature = "plugin-host")]
mod imp {
    use super::{PluginInfo, PluginInstance, PluginParameter};
    use libloading::Library;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::path::{Path, PathBuf};

    pub const CLAP_VERSION_MAJOR: u32 = 1;

    pub const CLAP_EXT_PARAMS: &CStr = c"clap.params";
    pub const CLAP_PLUGIN_FACTORY_ID: &CStr = c"clap.plugin-factory";

    const HOST_NAME: &CStr = c"Sonix";
    const HOST_VENDOR: &CStr = c"Sonix Studio";
    const HOST_URL: &CStr = c"https://github.com/alexwest1981/sonix";
    const HOST_VERSION: &CStr = c"0.1.0";

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
        pub id: u64,
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
        pub get_value: Option<unsafe extern "C" fn(*const ClapPlugin, u64, *mut f64) -> bool>,
        pub value_to_text: Option<
            unsafe extern "C" fn(*const ClapPlugin, u64, f64, *mut c_char, u32) -> bool,
        >,
        pub text_to_value:
            Option<unsafe extern "C" fn(*const ClapPlugin, u64, *const c_char, *mut f64) -> bool>,
        pub flush: Option<unsafe extern "C" fn(*const ClapPlugin, *const c_void, *const c_void)>,
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
            Ok((plugin, info, params)) => Ok(Box::new(ClapInstance {
                handle,
                plugin,
                _host: host,
                info,
                params,
            })),
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
            assert_eq!(info.id, "com.sonix.mock-synth");
            assert_eq!(info.name, "Sonix Mock Synth");
            assert_eq!(info.vendor, "Sonix Test");
            assert_eq!(info.version, "1.0.0");
            assert!(info.features.iter().any(|f| f == "instrument"));

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
            assert_eq!(snapshot.info.unwrap().name, "Sonix Mock Synth");
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
}
