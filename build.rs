// Build script for Sonix.
//
// When the opt-in `plugin-host` feature is enabled we compile a tiny, real CLAP
// shared library from `tests/fixtures/mock_clap.c` and expose its path to the
// test suite via `SONIX_MOCK_CLAP`. That lets the loader be tested end-to-end
// against a genuine `dlopen`-able CLAP bundle without shipping any plugin.
// If no C compiler is available the mock is simply skipped (the end-to-end test
// detects the missing env var and does nothing).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=tests/fixtures/mock_clap.c");
    println!("cargo:rerun-if-changed=tests/fixtures/empty.c");

    if env::var_os("CARGO_FEATURE_PLUGIN_HOST").is_none() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let cc = env::var("CC").unwrap_or_else(|_| "cc".to_string());

    let compile = |source: &str, output: &str| -> Option<PathBuf> {
        let src = manifest_dir.join(source);
        if !src.exists() {
            println!("cargo:warning=fixture source missing at {}", src.display());
            return None;
        }
        let out = out_dir.join(output);
        let status = Command::new(&cc)
            .args(["-shared", "-fPIC", "-O2", "-o"])
            .arg(&out)
            .arg(&src)
            .status();
        match status {
            Ok(s) if s.success() => Some(out),
            _ => {
                println!(
                    "cargo:warning=could not build {source} (no working C compiler?); \
                     the plugin-host end-to-end tests will be skipped"
                );
                None
            }
        }
    };

    if let Some(mock) = compile("tests/fixtures/mock_clap.c", "libsonix_mock_clap.so") {
        println!("cargo:rustc-env=SONIX_MOCK_CLAP={}", mock.display());
    }
    if let Some(empty) = compile("tests/fixtures/empty.c", "libsonix_empty.so") {
        println!("cargo:rustc-env=SONIX_EMPTY_SO={}", empty.display());
    }
}
