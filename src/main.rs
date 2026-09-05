mod audio;
mod ui;

use audio::AudioEngine;
use ui::SonixApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([780.0, 540.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation"),
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
