//! **Instrument library — one list of everything playable.**
//!
//! The scan is a folder convention, not a database: drop a `.sf2`, a `.sfz` or a JSFX file
//! into `~/.local/share/sonix/instruments/` and it shows up. The builtin sources and the
//! already-scanned plugin list are added from memory, so there is exactly one plugin
//! scanner in the app ([`crate::audio::plugin_host::PluginManager`]).
//!
//! `sf2_presets` exists so the UI can ask which preset to import instead of silently
//! taking the first instrument in the file.

use std::path::{Path, PathBuf};

use crate::audio::plugin_host::{PluginCategory, PluginDescriptor, PluginFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstrumentKind {
    Builtin,
    JsFx,
    SoundFont,
    Sfz,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub name: String,
    pub kind: InstrumentKind,
    pub path: String,
    pub format: String,
}

/// The folder convention. `paths.rs` is the only module allowed to build paths.
#[allow(dead_code)] // the library view that lists these lands separately; the scan has no caller yet.
pub fn default_dir() -> PathBuf {
    crate::paths::Paths::from_env()
        .data_dir()
        .join("instruments")
}

/// The two Rust sources in the binary. Their pseudo-paths are what `SetInstrument` matches.
fn builtins() -> Vec<LibraryEntry> {
    use crate::audio::builtin_fx::{
        modal_fx::MODAL_SOURCE_PATH, wavetable_fx::WAVETABLE_SOURCE_PATH,
    };
    [
        ("Wavetable", WAVETABLE_SOURCE_PATH),
        ("Modal", MODAL_SOURCE_PATH),
    ]
    .into_iter()
    .map(|(name, path)| LibraryEntry {
        name: name.to_string(),
        kind: InstrumentKind::Builtin,
        path: path.to_string(),
        format: "Builtin".to_string(),
    })
    .collect()
}

/// Scans the default folder (creating it) plus the builtin sources.
#[allow(dead_code)] // no caller until the library view exists.
pub fn scan() -> Vec<LibraryEntry> {
    scan_with_plugins(&[])
}

/// Scans the default folder plus a caller-supplied plugin list.
#[allow(dead_code)] // no caller until the library view exists.
pub fn scan_with_plugins(plugins: &[PluginDescriptor]) -> Vec<LibraryEntry> {
    let dir = default_dir();
    // A missing folder is the normal first run, not an error: the scan just has no files.
    let _ = std::fs::create_dir_all(&dir);
    scan_in(&dir, plugins)
}

/// The testable half: everything but the folder is given by the caller.
pub fn scan_in(dir: &Path, plugins: &[PluginDescriptor]) -> Vec<LibraryEntry> {
    let mut out = builtins();

    collect_files(dir, &mut out, 0);

    for entry in crate::audio::jsfx::list(dir) {
        let stem = entry
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(LibraryEntry {
            name: if entry.desc.is_empty() {
                stem
            } else {
                entry.desc
            },
            kind: InstrumentKind::JsFx,
            path: entry.path.to_string_lossy().to_string(),
            format: "JSFX".to_string(),
        });
    }

    // Instruments are the "Synth" category; Sonix's own effects are already the builtins.
    for p in plugins
        .iter()
        .filter(|p| p.format != PluginFormat::Builtin && p.category == PluginCategory::Synth)
    {
        out.push(LibraryEntry {
            name: p.name.clone(),
            kind: InstrumentKind::Plugin,
            path: p.file_path.clone(),
            format: p.format.name().to_string(),
        });
    }

    out.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    out
}

fn collect_files(dir: &Path, out: &mut Vec<LibraryEntry>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out, depth + 1);
            continue;
        }
        let lower = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let (kind, format) = match lower.as_str() {
            "sf2" => (InstrumentKind::SoundFont, "SoundFont 2"),
            "sfz" => (InstrumentKind::Sfz, "SFZ"),
            _ => continue,
        };
        out.push(LibraryEntry {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            kind,
            path: path.to_string_lossy().to_string(),
            format: format.to_string(),
        });
    }
}

/// The preset names `sf2::parse` already collects, by path.
pub fn sf2_presets(path: &str) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(crate::audio::sf2::parse(&bytes)?.presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sonix-instr-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// (a) one sf2, one sfz, one jsfx in a folder are found and labeled by kind.
    #[test]
    fn scan_finds_each_convention_kind() {
        let dir = temp_dir("kinds");
        std::fs::write(dir.join("piano.sf2"), crate::audio::sf2::tests::tiny_sf2()).unwrap();
        std::fs::write(dir.join("strings.sfz"), "<group>\n").unwrap();
        std::fs::write(
            dir.join("pass.jsfx"),
            "desc:Test Filter\n@init\n@sample\nspl0=spl0;\n",
        )
        .unwrap();

        let entries = scan_in(&dir, &[]);
        for e in &entries {
            println!("{:?} {} | {} | {}", e.kind, e.name, e.format, e.path);
        }

        let kinds: Vec<_> = entries.iter().map(|e| e.kind).collect();
        for kind in [
            InstrumentKind::SoundFont,
            InstrumentKind::Sfz,
            InstrumentKind::JsFx,
        ] {
            assert_eq!(
                kinds.iter().filter(|k| **k == kind).count(),
                1,
                "one {kind:?}"
            );
        }
        assert_eq!(
            entries
                .iter()
                .find(|e| e.kind == InstrumentKind::SoundFont)
                .unwrap()
                .format,
            "SoundFont 2"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (b) a missing folder yields the builtins only — no error, no panic.
    #[test]
    fn missing_folder_returns_builtins_only() {
        let dir = std::env::temp_dir().join(format!("sonix-instr-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let entries = scan_in(&dir, &[]);
        for e in &entries {
            println!("{:?} {} | {} | {}", e.kind, e.name, e.format, e.path);
        }
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.kind == InstrumentKind::Builtin));
    }

    /// (c) `sf2_presets` sees the same presets `parse` does. Fixture: `sf2::tests::tiny_sf2`,
    /// the in-memory minimal font the parser's own tests use (the repo ships no `.sf2`).
    #[test]
    fn sf2_presets_matches_parse() {
        let dir = temp_dir("presets");
        let path = dir.join("tiny.sf2");
        let bytes = crate::audio::sf2::tests::tiny_sf2();
        std::fs::write(&path, &bytes).unwrap();

        let from_file = sf2_presets(&path.to_string_lossy()).expect("presets");
        let parsed = crate::audio::sf2::parse(&bytes).expect("parse");
        println!(
            "file: {from_file:?} parse: {:?} preset→instrument: {:?}",
            parsed.presets, parsed.preset_instruments
        );
        assert_eq!(from_file, parsed.presets);
        assert_eq!(from_file.len(), 1);
        // The fixture's one preset names instrument 0 (pgen operator 41).
        assert_eq!(parsed.preset_instruments, vec![Some(0)]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (d) the builtin ids are exactly the strings `SetInstrument` matches.
    #[test]
    fn builtin_paths_match_set_instrument() {
        use crate::audio::builtin_fx::{
            modal_fx::MODAL_SOURCE_PATH, wavetable_fx::WAVETABLE_SOURCE_PATH,
        };
        println!("wavetable: {WAVETABLE_SOURCE_PATH} modal: {MODAL_SOURCE_PATH}");
        assert_eq!(WAVETABLE_SOURCE_PATH, "builtin:wavetable");
        assert_eq!(MODAL_SOURCE_PATH, "builtin:modal");

        let paths: Vec<_> = builtins().into_iter().map(|e| e.path).collect();
        assert!(paths.contains(&WAVETABLE_SOURCE_PATH.to_string()));
        assert!(paths.contains(&MODAL_SOURCE_PATH.to_string()));
    }
}
