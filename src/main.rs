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
mod platform;
mod selftest;
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
    // `sonix --selftest`: mäter det Windows-kriteriet som CI inte kan svara på — att
    // ljudenheten spelar och vad MIDI-ingången hittar. Fönsterlöst med flit: det som
    // bara en människa kan se står sist i utskriften, inte bland det som är mätt.
    // (Fönster på Windows-substantiv skriver till en omdirigering — se ROADMAP 7.1.)
    if std::env::args().any(|a| a == "--selftest") {
        selftest::run()?;
        return Ok(());
    }
    // `sonix --clean-tags <fil...>`: visar vad filerna bär och tar bort
    // AI-/Sunohärkomsten — inte musiken. Titel, artist, låttext och omslag lämnas
    // (Alex 2026-09-13: den första versionen tog hela taggen, och det var för
    // mycket). Ljudet rörs inte; bara härkomsten försvinner. (Att en kontroll
    // läser LJUDET och inte taggen står i metadata.rs — och är skälet att det här
    // inte är ett verktyg för att dölja något.)
    if let Some(pos) = std::env::args().position(|a| a == "--clean-tags") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --clean-tags <fil> [fler filer]");
            std::process::exit(2);
        }
        let mut cleaned = 0usize;
        for f in &files {
            match audio::metadata::scan(f) {
                Ok(report) => {
                    for k in &report.kept {
                        println!("  {f} — lämnas: {k}");
                    }
                    for n in &report.notes {
                        println!("  {f} — ⚠ {n}");
                    }
                    if report.removable_bytes == 0 {
                        println!("  ren: {f} (ingen härkomst att ta)");
                        continue;
                    }
                    println!(
                        "  {f}: {} byte härkomst (ID3v2={} ID3v1={} RIFF={})",
                        report.removable_bytes,
                        report.has_id3v2,
                        report.has_id3v1,
                        report.has_riff_metadata
                    );
                    for e in &report.excerpts {
                        println!("      tas bort: {e}");
                    }
                    match audio::metadata::strip_tags(f) {
                        Ok(n) => {
                            println!("      ✅ {n} byte borttagna, ljudet orört");
                            cleaned += 1;
                        }
                        Err(e) => println!("      ⚠ {e}"),
                    }
                }
                Err(e) => println!("  ⚠ {e}"),
            }
        }
        println!("Städat: {cleaned} fil(er).");
        std::process::exit(0);
    }

    // `sonix --detect-bpm <fil...>`: uppskattar tempot ur en ljudfil. Verktyget finns
    // för att analysen ska kunna MÄTAS mot riktiga stämmor innan den får styra ett
    // projekt — ett gissat tempo ställer hela låten fel.
    if let Some(pos) = std::env::args().position(|a| a == "--detect-bpm") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --detect-bpm <fil> [fler filer]");
            std::process::exit(2);
        }
        for f in &files {
            match audio::stem_separator::detect_bpm_from_file(f) {
                Ok(bpm) => println!("  {bpm:7.2} BPM  {}", f),
                Err(e) => println!("  ⚠ {f}: {e}"),
            }
        }
        std::process::exit(0);
    }



    // `sonix --inspect-plugin <fil...>`: öppnar en plugin **huvudlöst** och skriver ut vad
    // den rapporterar — identitet, portar, latens och parametrar. Verktyget finns för att
    // Fas 8.6 ska kunna MÄTAS mot en riktig, nedladdad plugin i stället för bara mot vår egen
    // mock: pluginens egna portar (sidokedja in, egna utbussar) och dess latens går inte att
    // gissa, och en gissning hade stämt för mocken och varit fel för allt annat.
    //
    // **Detta är den enda vägen som öppnar en plugin utan fönster** — en krasch tar därför
    // kommandot med sig (och syns som en utgångskod), vilket är ärligare än att en skanning
    // ser ut att ha lyckats. I appen går samma plugin genom sandlådan i stället.
    if let Some(pos) = std::env::args().position(|a| a == "--inspect-plugin") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --inspect-plugin <plugin.clap|plugin.vst3|mapp> [...]");
            std::process::exit(2);
        }
        let mut failed = 0usize;
        for f in &files {
            match audio::plugin_host_live::load_processor(f, 48_000.0, 512) {
                Ok(processor) => {
                    let info = processor.info();
                    println!("📦 {}", f);
                    println!("  backend:    {}", processor.backend());
                    println!("  namn:       {} ({})", info.name, info.id);
                    println!("  tillverkare:{} v{}", info.vendor, info.version);
                    println!("  latens:     {} ramar", processor.latency_frames());
                    // Fas 8.6: det här är siffrorna som avgör om en plugin kan köras i en
                    // sidokedja eller ha egna utbussar — lästa ur pluginens egen beskrivning.
                    println!(
                        "  sidokedja:  {} ingång(ar)",
                        processor.sidechain_inputs()
                    );
                    println!("  egna utbussar: {}", processor.extra_outputs());
                    let params = processor.parameters();
                    println!("  parametrar: {} (automatiserbara: {})", params.len(), params.iter().filter(|p| p.is_automatable()).count());
                    for p in params.iter().take(8) {
                        println!(
                            "    {:>4}  {:<28} {} .. {} (standard {})",
                            p.id, p.name, p.min_value, p.max_value, p.default_value
                        );
                    }
                    if params.len() > 8 {
                        println!("    ... {} fler", params.len() - 8);
                    }
                }
                Err(e) => {
                    println!("⚠ {}: {e}", f);
                    failed += 1;
                }
            }
        }
        std::process::exit(if failed == 0 { 0 } else { 1 });
    }

    // `sonix --macro-example <fil>`: skriver en färdig startkedja. Kedjan är en fil man kan
    // rätta för hand, så kommandot behövs för att komma igång utan att kunna filformen.
    if let Some(pos) = std::env::args().position(|a| a == "--macro-example") {
        let rest: Vec<String> = std::env::args().skip(pos + 1).collect();
        let Some(path) = rest.first() else {
            eprintln!("användning: sonix --macro-example <fil.json>");
            std::process::exit(2);
        };
        let chain = audio::macro_chain::example_chain();
        match audio::macro_chain::save_chain(std::path::Path::new(path), &chain) {
            Ok(()) => {
                println!("Skrev startkedjan \"{}\" till {path}:", chain.name);
                for step in &chain.steps {
                    println!("  - {}", step.label());
                }
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("⚠ {e}");
                std::process::exit(2);
            }
        }
    }

    // `sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]` (Fas 8.9): kör en
    // sparad kedja över filer — samma behandling på många tagningar. Fönsterlöst med flit,
    // så en batch går att köra och MÄTAS från en terminal (och från CI, om vi vill).
    // Felkoderna är en del av kontraktet: 2 = användningsfel, 1 = någon fil föll.
    if let Some(pos) = std::env::args().position(|a| a == "--run-macro") {
        let rest: Vec<String> = std::env::args().skip(pos + 1).collect();
        let mut out_dir: Option<std::path::PathBuf> = None;
        let mut inputs: Vec<String> = Vec::new();
        let mut chain_path: Option<String> = None;
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--out" => {
                    i += 1;
                    if i >= rest.len() {
                        eprintln!("--out behöver en mapp");
                        std::process::exit(2);
                    }
                    out_dir = Some(std::path::PathBuf::from(&rest[i]));
                }
                arg => {
                    if chain_path.is_none() {
                        chain_path = Some(arg.to_string());
                    } else {
                        inputs.push(arg.to_string());
                    }
                }
            }
            i += 1;
        }
        let Some(chain_path) = chain_path else {
            eprintln!("användning: sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]");
            std::process::exit(2);
        };
        let chain = match audio::macro_chain::load_chain(&chain_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("kedjan gick inte att läsa: {e}");
                std::process::exit(2);
            }
        };
        // En mapp blir sina ljudfiler. En fil blir sig själv. En sökväg som inte finns
        // behålls med flit: då blir den ett **fel i loggen** i stället för en tyst tom körning.
        let mut files: Vec<String> = Vec::new();
        for input in &inputs {
            let path = std::path::Path::new(input);
            if path.is_dir() {
                match audio::macro_chain::audio_files_in(path) {
                    Ok(found) => files.extend(found),
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        std::process::exit(2);
                    }
                }
            } else {
                files.push(input.clone());
            }
        }
        if files.is_empty() {
            eprintln!("användning: sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]");
            std::process::exit(2);
        }
        if !chain.has_export() {
            // En kedja utan exportsteg gör ingenting som syns. Att köra den tyst vore att
            // lämna ett pass som ser lyckat ut utan att ha skrivit något.
            eprintln!(
                "kedjan \"{}\" har inget exportsteg — den skriver ingenting. Lägg till ett export-steg.",
                chain.name
            );
            std::process::exit(2);
        }
        println!("Makro: {} ({} steg)", chain.name, chain.steps.len());
        for step in &chain.steps {
            println!("  - {}", step.label());
        }
        println!("Filer: {}", files.len());
        let meta = audio::exporter::ExportMeta::default();
        let runs = audio::macro_chain::run_batch(&chain, &files, out_dir.as_deref(), &meta);
        for run in &runs {
            println!("\n{}", run.input);
            for r in &run.reports {
                println!("  {}: {}", r.step, r.detail);
            }
            if let Some(e) = &run.error {
                println!("  ⚠ {e}");
            }
        }
        let ok = audio::macro_chain::succeeded(&runs);
        let failed = runs.len() - ok;
        match audio::macro_chain::write_batch_log(
            &chain,
            &runs,
            std::path::Path::new(&runs[0].input),
            out_dir.as_deref(),
        ) {
            Ok(path) => println!("\nLogg: {path}"),
            Err(e) => println!("\n⚠ loggen kunde inte skrivas: {e}"),
        }
        println!("Klara: {ok} av {} ({failed} med fel)", runs.len());
        std::process::exit(if failed > 0 { 1 } else { 0 });
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
