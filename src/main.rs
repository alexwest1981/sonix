mod audio;
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
    println!("🔊 Startar PipeWire / ALSA ljudmotor...");

    let engine = AudioEngine::new(512)?;

    println!("  • Ljudkort:    {}", engine.device_name);
    println!("  • Samplingsfrekvens: {} Hz", engine.sample_rate);
    println!("  • Kanaler:     Stereo");
    println!("🚀 Startar grafiskt gränssnitt (GUI)...");

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([780.0, 540.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw"),
        ..Default::default()
    };

    let cli_arg = std::env::args().nth(1);

    eframe::run_native(
        "Sonix DAW",
        options,
        Box::new(move |_cc| {
            let mut app = SonixApp::new(engine);
            if let Some(ref arg) = cli_arg {
                if arg.to_lowercase().ends_with(".zip") {
                    app.import_suno_zip(arg);
                } else if std::path::Path::new(arg).is_dir() {
                    let (title, bpm) = SonixApp::parse_suno_zip_info(arg);
                    app.import_suno_stems_from_folder(arg, &title, bpm);
                }
            }
            Ok(Box::new(app))
        }),
    ).map_err(|e| format!("Kunde inte starta fönstret: {}", e).into())
}
