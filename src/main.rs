mod audio;
mod i18n;
mod ui;

use audio::AudioEngine;
use ui::SonixApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let engine = AudioEngine::new(512)?;

    println!("  • Audio device: {}", engine.device_name);
    println!("  • Sample rate:       {} Hz", engine.sample_rate);
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

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Sonix DAW",
        options,
        Box::new(move |_cc| {
            let mut app = SonixApp::new(engine);
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
