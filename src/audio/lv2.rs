//! Minimal in-process **LV2** host.
//!
//! Speaks the LV2 core C ABI directly (`lv2_descriptor` + `LV2_Descriptor`) and
//! `dlopen`s the module through `libloading` — the same approach the CLAP, VST3
//! and VST2 hosts in this directory take. The [`livi`](https://crates.io/crates/livi)
//! crate was tried first and rejected: it links `liblilv`/`libserd` through
//! `pkg-config` at build time and at run time, which neither CI nor the
//! packaging declares (`packaging/arch-bin/PKGBUILD` depends on alsa-lib only),
//! so `--features plugin-host` would stop building on a clean machine.
//!
//! Scope: discover bundles (walk for `*.ttl`, read `lv2:binary` + URI + name +
//! author), instantiate a stereo effect and run audio through it. No presets,
//! no state, no MIDI, no GUIs, no `buf-size`/URID features: a plugin that
//! requires any of those is refused at load with its own name in the error.
//!
//! Port metadata comes from the bundle's Turtle, because LV2 has no runtime
//! introspection API — the host has to read `lv2:index`/`lv2:AudioPort`/… .
//!
//! shortcut: no LV2 features are passed to `instantiate` (no `buf-size`, no
//! URID map), so synths and plugins with `lv2:requiredFeature` cannot load;
//! upgrade when hosting instruments.

#![allow(dead_code)]

use crate::audio::plugin_host_live::{
    PARAM_IS_AUTOMATABLE, PluginInfo, PluginInspection, PluginParameter, PluginProcessor,
};
use libloading::Library;
use std::ffi::{CStr, c_char, c_void};
use std::path::{Path, PathBuf};

/// `LV2_Descriptor` from `lv2.h`, in declaration order.
#[repr(C)]
pub struct Lv2Descriptor {
    uri: *const c_char,
    instantiate: Option<
        unsafe extern "C" fn(
            descriptor: *const Lv2Descriptor,
            sample_rate: f64,
            features: *const *const Lv2Feature,
        ) -> *mut c_void,
    >,
    connect_port: Option<unsafe extern "C" fn(handle: *mut c_void, port: u32, data: *mut c_void)>,
    activate: Option<unsafe extern "C" fn(handle: *mut c_void)>,
    run: Option<unsafe extern "C" fn(handle: *mut c_void, sample_count: u32)>,
    deactivate: Option<unsafe extern "C" fn(handle: *mut c_void)>,
    cleanup: Option<unsafe extern "C" fn(handle: *mut c_void)>,
    extension_data: Option<unsafe extern "C" fn(uri: *const c_char) -> *const c_void>,
}

/// `LV2_Feature` from `lv2.h`.
#[repr(C)]
pub struct Lv2Feature {
    uri: *const c_char,
    data: *mut c_void,
}

/// One plugin found in a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lv2Plugin {
    /// Plugin URI — the bundle's stable identity, and what `instantiate` needs.
    pub id: String,
    pub name: String,
    pub author: String,
    pub bundle: PathBuf,
}

/// One row from the bundle's Turtle: `lv2:index`, port type, and range.
#[derive(Debug, Clone, PartialEq)]
struct PortDecl {
    index: u32,
    audio: bool,
    input: bool,
    symbol: String,
    min: f32,
    max: f32,
    default: f32,
}

// ------------------------------------------------------------------- turtle

/// Splits Turtle into statement-sized chunks: on `[`, `]` and on a `.` that ends
/// a statement (preceded by whitespace or `>`, followed by whitespace/EOF).
///
/// Deliberately crude: it is a *locator* for `lv2:index`/`lv2:binary` chunks,
/// not a parser. Garbage in a file yields chunks that match nothing.
fn chunks(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        let boundary = *b == b'['
            || *b == b']'
            || (*b == b'.'
                && i > 0
                && bytes[i - 1].is_ascii_whitespace()
                && bytes.get(i + 1).is_none_or(u8::is_ascii_whitespace));
        if boundary {
            if start < i {
                out.push(&text[start..i]);
            }
            start = i + 1;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// The `<uri>` that starts a chunk, i.e. the statement's subject.
fn subject(chunk: &str) -> Option<&str> {
    let open = chunk.find('<')?;
    let rest = &chunk[open + 1..];
    let close = rest.find('>')?;
    Some(&rest[..close])
}

/// The object written after `key`, as `<uri>` or `"literal"` or a bare token.
fn value_after(chunk: &str, key: &str) -> Option<String> {
    let at = chunk.find(key)? + key.len();
    let rest = chunk[at..].trim_start();
    if let Some(body) = rest.strip_prefix('<') {
        return Some(body[..body.find('>')?].to_string());
    }
    if let Some(body) = rest.strip_prefix('"') {
        return Some(body[..body.find('"')?].to_string());
    }
    let token: String = rest
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ';' && *c != '.')
        .collect();
    (!token.is_empty()).then_some(token)
}

fn first_of(chunk: &str, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| value_after(chunk, k))
}

fn number(chunk: &str, key: &str) -> Option<f32> {
    value_after(chunk, key)?.parse().ok()
}

/// Every `*.ttl` under `dir`, so a bundle (`X.lv2/manifest.ttl`) is reached too.
fn collect_ttl(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ttl(&path, depth + 1, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("ttl") {
            out.push(path);
        }
    }
}

/// `(subject, chunk)` for every statement in every `*.ttl` of one bundle.
///
/// Port chunks inside `[ … ]` have no subject of their own, so they are kept
/// with an empty subject — the port reader needs them, the plugin reader ignores
/// them (a plugin statement always carries `lv2:binary`).
fn bundle_statements(bundle: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    collect_ttl(bundle, 0, &mut files);
    let mut out = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for chunk in chunks(&text) {
            let subject = subject(chunk).unwrap_or("");
            if subject.is_empty() && !chunk.contains("lv2:index") {
                continue;
            }
            out.push((subject.to_string(), chunk.to_string()));
        }
    }
    out
}

// --------------------------------------------------------------------- scan

/// Walks `dir` for `*.ttl` manifests and returns one entry per plugin URI.
///
/// Never panics: an unreadable or malformed file is skipped, and a plugin whose
/// name cannot be read keeps its URI's last segment as a name.
pub fn scan(dir: &Path) -> Vec<Lv2Plugin> {
    let mut out: Vec<Lv2Plugin> = Vec::new();
    let mut files = Vec::new();
    collect_ttl(dir, 0, &mut files);
    for file in files {
        let Some(bundle) = file.parent() else {
            continue;
        };
        let statements = bundle_statements(bundle);
        for (uri, chunk) in &statements {
            if !chunk.contains("lv2:binary") {
                continue;
            }
            if out.iter().any(|p| &p.id == uri) {
                continue;
            }
            let name = statements
                .iter()
                .filter(|(s, _)| s == uri)
                .find_map(|(_, c)| first_of(c, &["doap:name", "lv2:name", "rdfs:label"]))
                .unwrap_or_else(|| uri.rsplit('/').next().unwrap_or(uri).to_string());
            let author = statements
                .iter()
                .filter(|(s, _)| s == uri)
                .find_map(|(_, c)| {
                    first_of(c, &["doap:maintainer", "doap:developer", "foaf:name"])
                })
                .unwrap_or_default();
            out.push(Lv2Plugin {
                id: uri.clone(),
                name,
                author,
                bundle: bundle.to_path_buf(),
            });
        }
    }
    out
}

// ----------------------------------------------------------------- instance

/// A live stereo LV2 instance.
pub struct Lv2Instance {
    /// Kept alive for as long as the instance borrows code from it.
    _lib: Library,
    descriptor: *const Lv2Descriptor,
    handle: *mut c_void,
    uri: String,
    block: usize,
    controls: Vec<f32>,
    control_ports: Vec<PortDecl>,
    audio_in: [u32; 2],
    audio_out: [u32; 2],
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
}

// The audio thread owns the instance; the raw handle is only ever touched there.
unsafe impl Send for Lv2Instance {}

impl Lv2Instance {
    /// Instantiates `uri` from `bundle` for stereo block processing.
    ///
    /// Fails when the bundle has no such URI, when the binary does not export a
    /// matching descriptor, or when the plugin's ports are not exactly two audio
    /// inputs and two audio outputs — the error names the plugin either way.
    pub fn new(
        bundle: &Path,
        uri: &str,
        sample_rate: f64,
        block_size: usize,
    ) -> Result<Self, String> {
        if !bundle.is_dir() {
            return Err(format!("LV2 bundle {} is not a directory", bundle.display()));
        }
        let statements = bundle_statements(bundle);
        let binary = statements
            .iter()
            .filter(|(s, _)| s == uri)
            .find_map(|(_, c)| value_after(c, "lv2:binary"))
            .or_else(|| {
                statements
                    .iter()
                    .find_map(|(_, c)| value_after(c, "lv2:binary"))
            })
            .ok_or_else(|| {
                format!(
                    "LV2 bundle {} has no lv2:binary for {uri}",
                    bundle.display()
                )
            })?;
        let library_path = bundle.join(&binary);
        let lib = unsafe { Library::new(&library_path) }.map_err(|e| {
            format!(
                "could not open LV2 binary {} ({e})",
                library_path.display()
            )
        })?;
        let symbol = unsafe { lib.get::<unsafe extern "C" fn(u32) -> *const Lv2Descriptor>(b"lv2_descriptor\0") }
            .map_err(|e| format!("LV2 binary {} exports no lv2_descriptor ({e})", library_path.display()))?;
        let descriptor_fn = *symbol;
        drop(symbol);

        let mut descriptor: *const Lv2Descriptor = std::ptr::null();
        for index in 0..64u32 {
            let candidate = unsafe { descriptor_fn(index) };
            if candidate.is_null() {
                break;
            }
            let found = unsafe { CStr::from_ptr((*candidate).uri) }.to_string_lossy();
            if found == uri {
                descriptor = candidate;
                break;
            }
        }
        if descriptor.is_null() {
            return Err(format!(
                "LV2 binary {} has no plugin with URI {uri}",
                library_path.display()
            ));
        }

        let ports: Vec<PortDecl> = statements
            .iter()
            .filter_map(|(_, chunk)| {
                chunk.contains("lv2:index").then(|| PortDecl {
                    index: number(chunk, "lv2:index").unwrap_or(0.0) as u32,
                    audio: chunk.contains("lv2:AudioPort"),
                    input: chunk.contains("InputPort"),
                    symbol: value_after(chunk, "lv2:symbol").unwrap_or_default(),
                    min: number(chunk, "lv2:minimum").unwrap_or(f32::NEG_INFINITY),
                    max: number(chunk, "lv2:maximum").unwrap_or(f32::INFINITY),
                    default: number(chunk, "lv2:default").unwrap_or(0.0),
                })
            })
            .collect();

        let mut audio_in: Vec<u32> = ports
            .iter()
            .filter(|p| p.audio && p.input)
            .map(|p| p.index)
            .collect();
        let mut audio_out: Vec<u32> = ports
            .iter()
            .filter(|p| p.audio && !p.input)
            .map(|p| p.index)
            .collect();
        audio_in.sort_unstable();
        audio_out.sort_unstable();
        if audio_in.len() != 2 || audio_out.len() != 2 {
            return Err(format!(
                "LV2 plugin {uri} is not stereo in/out ({} audio in, {} audio out)",
                audio_in.len(),
                audio_out.len()
            ));
        }

        let mut control_ports: Vec<PortDecl> = ports
            .iter()
            .filter(|p| !p.audio && p.input)
            .cloned()
            .collect();
        control_ports.sort_by_key(|p| p.index);

        let block = block_size.max(1);
        let mut instance = Lv2Instance {
            _lib: lib,
            descriptor,
            handle: std::ptr::null_mut(),
            uri: uri.to_string(),
            block,
            controls: control_ports.iter().map(|p| p.default).collect(),
            control_ports,
            audio_in: [audio_in[0], audio_in[1]],
            audio_out: [audio_out[0], audio_out[1]],
            in_l: vec![0.0; block],
            in_r: vec![0.0; block],
            out_l: vec![0.0; block],
            out_r: vec![0.0; block],
        };

        let features: [*const Lv2Feature; 1] = [std::ptr::null()];
        let instantiate = unsafe { (*descriptor).instantiate }
            .ok_or_else(|| format!("LV2 plugin {uri} has no instantiate callback"))?;
        let handle = unsafe { instantiate(descriptor, sample_rate, features.as_ptr()) };
        if handle.is_null() {
            return Err(format!("LV2 plugin {uri} could not be instantiated"));
        }
        instance.handle = handle;

        let connect = unsafe { (*descriptor).connect_port }
            .ok_or_else(|| format!("LV2 plugin {uri} has no connect_port callback"))?;
        unsafe {
            connect(handle, instance.audio_in[0], instance.in_l.as_mut_ptr().cast());
            connect(handle, instance.audio_in[1], instance.in_r.as_mut_ptr().cast());
            connect(handle, instance.audio_out[0], instance.out_l.as_mut_ptr().cast());
            connect(handle, instance.audio_out[1], instance.out_r.as_mut_ptr().cast());
            for i in 0..instance.controls.len() {
                connect(
                    handle,
                    instance.control_ports[i].index,
                    std::ptr::addr_of_mut!(instance.controls[i]).cast(),
                );
            }
            if let Some(activate) = (*descriptor).activate {
                activate(handle);
            }
        }
        Ok(instance)
    }

    /// Runs the plugin over one block. Both slices must be at most the block
    /// size given to [`Lv2Instance::new`]; the shorter of the two lengths wins.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len()).min(self.block);
        if frames == 0 || self.handle.is_null() {
            return;
        }
        self.in_l[..frames].copy_from_slice(&left[..frames]);
        self.in_r[..frames].copy_from_slice(&right[..frames]);
        self.out_l[..frames].fill(0.0);
        self.out_r[..frames].fill(0.0);
        if let Some(run) = unsafe { (*self.descriptor).run } {
            unsafe { run(self.handle, frames as u32) };
        }
        left[..frames].copy_from_slice(&self.out_l[..frames]);
        right[..frames].copy_from_slice(&self.out_r[..frames]);
    }

    /// Sets the `index`-th control input (in port-index order). Returns false
    /// when the plugin has no such control port.
    pub fn set_param(&mut self, index: u32, value: f32) -> bool {
        let Some(port) = self.control_ports.get(index as usize) else {
            return false;
        };
        let value = value.clamp(port.min, port.max);
        self.controls[index as usize] = value;
        true
    }

    /// Deactivates and reactivates the plugin, which is LV2's reset idiom.
    pub fn reset(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe {
            if let Some(deactivate) = (*self.descriptor).deactivate {
                deactivate(self.handle);
            }
            if let Some(activate) = (*self.descriptor).activate {
                activate(self.handle);
            }
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    fn parameters(&self) -> Vec<PluginParameter> {
        self.control_ports
            .iter()
            .enumerate()
            .map(|(i, p)| PluginParameter {
                id: i as u32,
                name: if p.symbol.is_empty() {
                    format!("control {i}")
                } else {
                    p.symbol.clone()
                },
                module: String::new(),
                min_value: p.min as f64,
                max_value: p.max as f64,
                default_value: p.default as f64,
                flags: PARAM_IS_AUTOMATABLE,
            })
            .collect()
    }
}

impl Drop for Lv2Instance {
    fn drop(&mut self) {
        if let Some(cleanup) = unsafe { (*self.descriptor).cleanup } {
            unsafe { cleanup(self.handle) };
        }
    }
}

// ----------------------------------------------------------------- processor

/// An LV2 plugin wrapped for the engine's backend-agnostic insert path.
pub struct Lv2Processor {
    instance: Lv2Instance,
    info: PluginInfo,
    params: Vec<PluginParameter>,
}

impl PluginProcessor for Lv2Processor {
    fn backend(&self) -> &'static str {
        "LV2"
    }
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &self.params
    }
    fn latency_frames(&self) -> u32 {
        // LV2 reports latency through lv2:port latency ports; not hosted yet.
        0
    }
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.instance.process(left, right);
    }
    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        self.instance.set_param(id, value as f32)
    }
    fn reset(&mut self) {
        self.instance.reset();
    }
}

/// The plugin `path` points at; `path` may be the bundle itself, a file inside
/// it, or the plugin URI. A file inside a multi-plugin bundle resolves to the
/// bundle's first plugin (the `.so` name is not the URI).
fn resolve(path: &str) -> Result<Lv2Plugin, String> {
    let given = PathBuf::from(path);
    if !given.exists() {
        return Err(format!("LV2 path {path} does not exist"));
    }
    let bundle = if given.is_dir() {
        given
    } else {
        given
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("LV2 path {path} has no bundle directory"))?
    };
    let plugins = scan(&bundle);
    plugins
        .iter()
        .find(|p| p.id == path)
        .or_else(|| plugins.first())
        .cloned()
        .ok_or_else(|| format!("no LV2 plugin in {}", bundle.display()))
}

/// Loads the plugin `path` points at as a live processor.
pub fn load_processor(
    path: &str,
    sample_rate: f32,
    max_frames: u32,
) -> Result<Box<dyn PluginProcessor>, String> {
    let plugin = resolve(path)?;
    let instance = Lv2Instance::new(
        &plugin.bundle,
        &plugin.id,
        sample_rate as f64,
        max_frames as usize,
    )?;
    let info = PluginInfo {
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        vendor: plugin.author.clone(),
        ..PluginInfo::default()
    };
    let params = instance.parameters();
    Ok(Box::new(Lv2Processor {
        instance,
        info,
        params,
    }))
}

/// Snapshot of a bundle's plugins for `--inspect-plugin` and the UI. Never
/// panics; failures end up in `error`.
pub fn inspect(path: &str) -> PluginInspection {
    match resolve(path) {
        Ok(plugin) => {
            let parameters = match Lv2Instance::new(&plugin.bundle, &plugin.id, 48_000.0, 512) {
                Ok(instance) => instance.parameters(),
                Err(_) => Vec::new(),
            };
            PluginInspection {
                path: path.to_string(),
                backend: "LV2".to_string(),
                info: Some(PluginInfo {
                    id: plugin.id,
                    name: plugin.name,
                    vendor: plugin.author,
                    ..PluginInfo::default()
                }),
                parameters,
                gui: None,
                error: None,
            }
        }
        Err(err) => PluginInspection {
            path: path.to_string(),
            backend: "LV2".to_string(),
            info: None,
            parameters: Vec::new(),
            gui: None,
            error: Some(err),
        },
    }
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// The fixture bundle, compiled once per test process. `None` only when
    /// `cc` cannot run — the tests then say so instead of passing silently.
    fn fixture() -> Option<&'static PathBuf> {
        static BUNDLE: OnceLock<Option<PathBuf>> = OnceLock::new();
        BUNDLE.get_or_init(build_fixture).as_ref()
    }

    fn build_fixture() -> Option<PathBuf> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lv2_amp");
        let bundle = std::env::temp_dir().join(format!("sonix-lv2-{}", std::process::id()));
        let bundle = bundle.join("lv2_amp.lv2");
        std::fs::create_dir_all(&bundle).ok()?;
        for file in ["manifest.ttl", "amp.ttl"] {
            std::fs::copy(source.join(file), bundle.join(file)).ok()?;
        }
        let library = bundle.join("lv2_amp.so");
        let built = std::process::Command::new("cc")
            .arg("-shared")
            .arg("-fPIC")
            .arg("-O2")
            .arg("-o")
            .arg(&library)
            .arg(source.join("lv2_amp.c"))
            .status();
        match built {
            Ok(status) if status.success() && library.exists() => Some(bundle),
            Ok(status) => {
                println!("cc exited with {status}; cannot build the LV2 fixture");
                None
            }
            Err(err) => {
                println!("cc is unavailable ({err}); cannot build the LV2 fixture");
                None
            }
        }
    }

    fn sine(amplitude: f32, frames: usize, sample_rate: f32, hz: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                amplitude * (std::f32::consts::TAU * hz * i as f32 / sample_rate).sin()
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    /// Measured output/input level ratio — the gain, not a bit comparison.
    fn measured_gain(input: &[f32], output: &[f32]) -> f32 {
        rms(output) / rms(input).max(f32::MIN_POSITIVE)
    }

    #[test]
    fn scan_finds_the_fixture_plugin() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED scan_finds_the_fixture_plugin: no C compiler");
            return;
        };
        let plugins = scan(bundle.parent().expect("temp root"));
        let plugin = plugins
            .iter()
            .find(|p| p.id == "http://sonix.test/lv2/amp")
            .unwrap_or_else(|| panic!("fixture not found, scan returned: {plugins:?}"));
        println!(
            "scan: id={} name={} author={}",
            plugin.id, plugin.name, plugin.author
        );
        assert_eq!(plugin.name, "Sonix Test Amp");
        assert_eq!(plugin.author, "Sonix Test Suite");
    }

    #[test]
    fn scan_skips_garbage_turtle() {
        let dir = std::env::temp_dir().join(format!("sonix-lv2-garbage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("broken.ttl"), "not turtle {{{ <unclosed\n . . .")
            .expect("write");
        assert!(scan(&dir).is_empty(), "a garbage manifest must yield no plugins");
    }

    #[test]
    fn unity_gain_passes_audio_through() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED unity_gain_passes_audio_through: no C compiler");
            return;
        };
        let mut instance = Lv2Instance::new(bundle, "http://sonix.test/lv2/amp", 48_000.0, 256)
            .expect("instantiate fixture");
        let input = sine(0.5, 256, 48_000.0, 440.0);
        let (mut left, mut right) = (input.clone(), input.clone());
        instance.process(&mut left, &mut right);
        let gain = measured_gain(&input, &left);
        println!("unity gain: measured {gain:.6} (expected 1.0)");
        assert!((gain - 1.0).abs() < 0.01, "measured gain {gain}");
        assert!((measured_gain(&input, &right) - 1.0).abs() < 0.01);
    }

    #[test]
    fn control_gain_halves_the_output() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED control_gain_halves_the_output: no C compiler");
            return;
        };
        let mut instance = Lv2Instance::new(bundle, "http://sonix.test/lv2/amp", 48_000.0, 256)
            .expect("instantiate fixture");
        assert!(instance.set_param(0, 0.5), "gain is the first control port");
        assert!(!instance.set_param(7, 0.5), "unknown control must be refused");
        let input = sine(0.5, 256, 48_000.0, 440.0);
        let (mut left, mut right) = (input.clone(), input.clone());
        instance.process(&mut left, &mut right);
        let gain = measured_gain(&input, &left);
        println!("gain=0.5: measured {gain:.6} (expected 0.5)");
        assert!((gain - 0.5).abs() < 0.005, "measured gain {gain}");
        assert!((measured_gain(&input, &right) - 0.5).abs() < 0.005);
    }

    #[test]
    fn reset_then_process_again_still_works() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED reset_then_process_again_still_works: no C compiler");
            return;
        };
        let mut instance = Lv2Instance::new(bundle, "http://sonix.test/lv2/amp", 48_000.0, 128)
            .expect("instantiate fixture");
        let input = sine(0.25, 128, 48_000.0, 220.0);
        let (mut left, mut right) = (input.clone(), input.clone());
        instance.process(&mut left, &mut right);
        instance.reset();
        assert!(instance.set_param(0, 1.0));
        let (mut left, mut right) = (input.clone(), input.clone());
        instance.process(&mut left, &mut right);
        let gain = measured_gain(&input, &left);
        println!("after reset: measured {gain:.6} (expected 1.0)");
        assert!((gain - 1.0).abs() < 0.01, "measured gain after reset {gain}");
    }

    #[test]
    fn unknown_uri_is_named_in_the_error() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED unknown_uri_is_named_in_the_error: no C compiler");
            return;
        };
        let err = Lv2Instance::new(bundle, "http://sonix.test/lv2/nope", 48_000.0, 128)
            .err()
            .expect("an unknown URI must fail");
        println!("unknown URI error: {err}");
        assert!(err.contains("http://sonix.test/lv2/nope"), "{err}");
    }

    /// `.lv2` paths reach this host instead of being refused — the assertion the
    /// module-level probe in `plugin_host.rs` used to make in the opposite
    /// direction, back when Sonix had no LV2 host.
    #[test]
    fn lv2_paths_route_here_through_the_plugin_host() {
        let Some(bundle) = fixture() else {
            println!("SKIPPED lv2_paths_route_here_through_the_plugin_host: no C compiler");
            return;
        };
        let path = bundle.to_string_lossy().to_string();
        let processor = crate::audio::plugin_host_live::load_processor(&path, 48_000.0, 256)
            .expect("routed LV2 processor");
        println!(
            "routed backend={} name={} params={}",
            processor.backend(),
            processor.info().name,
            processor.parameters().len()
        );
        assert_eq!(processor.backend(), "LV2");
        assert_eq!(processor.info().name, "Sonix Test Amp");
        assert_eq!(processor.parameters().len(), 1);
    }
}
