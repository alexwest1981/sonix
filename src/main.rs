// En GUI-app ska inte öppna en konsolruta bredvid fönstret på Windows. Mätt på
// artefakten: utan det här stod det "Subsystem: Windows CUI" i den färdiga
// .exe-filen. I debug behålls konsolen, så att utskrifter och panik går att se.
// Innerattributet måste stå först i filen, före alla moduler.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod autosave;

// CLAP-värden använder X11 för plugin-fönster och memfd för sandlådan — båda
// är Linux-specifika. I stället för att falla på `libc`/X11 med kryptiska fel
// säger vi det rakt ut (Fas 7.1; porten är en egen uppgift).
#[cfg(all(feature = "plugin-host", not(target_os = "linux")))]
compile_error!(
    "plugin-host (CLAP-värd med X11-fönster och memfd-sandlåda) är Linux-only i den här versionen. Bygg utan --features plugin-host, eller se Fas 7.1 i ROADMAP.md."
);

mod i18n;
mod midi_take;
mod rng;
mod paths;
mod ui;

use audio::{AudioEngine, AudioSettings};
use ui::SonixApp;

/// `sonix --paths`: den kanoniska filkartan, utan att starta ljudmotor eller GUI.
fn print_paths() {
    let p = paths::paths();
    println!("Sonix – filkarta (Fas 6.0)");
    println!("  hem: {}", p.home().display());
    println!();
    for (label, path) in paths::table() {
        let mark = if path.exists() { '✓' } else { '·' };
        println!("  {mark} {label:<22} {}", path.display());
    }
    println!();
    println!("  ✓ = finns   · = skapas vid behov");
    println!("  Överstyr med SONIX_PROJECTS_DIR, SONIX_SAMPLES_DIR, SONIX_CONFIG_DIR,");
    println!("  SONIX_DATA_DIR, SONIX_STATE_DIR, SONIX_CACHE_DIR (annars följs XDG).");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Out-of-process plugin sandbox worker (Fas 4.5a): re-executed self with a
    // flag. This must run before any banner output so stdout stays a clean
    // protocol channel.
    #[cfg(feature = "plugin-host")]
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(pos) = args.iter().position(|a| a == audio::plugin_sandbox::WORKER_FLAG) {
            let result = audio::plugin_sandbox::serve_from_args(&args[pos + 1..]);
            if let Err(err) = result {
                eprintln!("{err}");
                std::process::exit(2);
            }
            return Ok(());
        }
    }

    // Filkartan skrivs ut före ljudmotorn så att kommandot fungerar utan ljudkort.
    if std::env::args().any(|a| a == "--paths") {
        print_paths();
        return Ok(());
    }

    // Fas 6.0: flytta äldre platser hit FÖRST (ensure_dirs skapar annars ett tomt
    // mål, vilket skulle blockera flytten), därefter skapa resten.
    let path_notes = paths::migrate();
    for (dir, err) in paths::paths().ensure_dirs() {
        eprintln!("⚠ Kunde inte skapa {}: {err}", dir.display());
    }

    // Install a robust panic hook that logs detailed backtrace to stderr and /tmp/sonix_crash.log
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        eprintln!("💥 [SONIX CRASH CAUGHT] {}", panic_info);
        eprintln!("Stack Backtrace:\n{}", backtrace);
        if let Ok(mut f) = std::fs::File::create("/tmp/sonix_crash.log") {
            use std::io::Write;
            let _ = writeln!(f, "=== SONIX CRASH REPORT ===");
            let _ = writeln!(f, "Panic: {}", panic_info);
            let _ = writeln!(f, "Backtrace:\n{}", backtrace);
        }
    }));

    println!("=========================================================");
    println!("  🎹 SONIX - Native Linux Digital Audio Workstation     ");
    println!("=========================================================");
    println!("🔊 Starting PipeWire / ALSA audio engine...");

    let audio_settings = AudioSettings::load();
    let engine = AudioEngine::new_with(
        512,
        audio_settings.sample_rate,
        audio_settings.buffer_frames,
    )?;

    println!("  • Audio device: {}", engine.device_name);
    println!("  • Sample rate:       {} Hz", engine.sample_rate);
    if let Some(frames) = engine.buffer_frames {
        println!("  • Buffer size:       {} frames", frames);
    }
    println!("  • Channels:    Stereo");
    println!("🚀 Starting graphical interface (GUI)...");

    let args: Vec<String> = std::env::args().collect();
    let screenshot_dir = if let Some(idx) = args.iter().position(|a| a == "--capture-screenshots" || a == "--screenshots" || a == "-s") {
        args.get(idx + 1).cloned().unwrap_or_else(|| "screenshots".to_string())
    } else {
        String::new()
    };

    let cli_arg = if screenshot_dir.is_empty() {
        args.get(1).cloned()
    } else {
        None
    };

    let is_fullscreen = !screenshot_dir.is_empty() || args.contains(&"--fullscreen".to_string());

    let viewport = if is_fullscreen {
        eframe::egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_inner_size([1920.0, 1200.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw")
    } else {
        eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([780.0, 540.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw")
    };

    let viewport = viewport.with_icon(
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/sonix.png"))
            .expect("embedded app icon (assets/sonix.png) must be a valid PNG"),
    );

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Sonix DAW",
        options,
        Box::new(move |_cc| {
            let mut app = SonixApp::new(engine);
            // Fas 6.0: berätta en gång om något flyttades eller läses från en
            // äldre plats — annars undrar användaren var filerna tog vägen.
            if !path_notes.is_empty() {
                for note in &path_notes {
                    println!("{note}");
                }
                app.status_message = path_notes.join("   ");
            }
            if !screenshot_dir.is_empty() {
                println!("📸 Enabling automatic screenshot capture to: {}", screenshot_dir);
                app.enable_screenshot_mode(std::path::Path::new(&screenshot_dir));
            } else if let Some(ref arg) = cli_arg {
                if arg.to_lowercase().ends_with(".zip") {
                    app.import_suno_zip(arg);
                } else if std::path::Path::new(arg).is_dir() {
                    let (title, bpm) = SonixApp::parse_suno_zip_info(arg);
                    app.import_suno_stems_from_folder(arg, &title, bpm);
                }
            }
            Ok(Box::new(app))
        }),
    ).map_err(|e| format!("Could not start the window: {}", e).into())
}
