//! **Självtestet** (Fas 7.1) — den del av Windows-kriteriet som ingen CI-mätning kan svara på.
//!
//! Kriteriet är "startar, spelar upp ljud och tar emot MIDI". CI kan bygga, länka och läsa
//! artefakten, men en runner har varken skärm eller ljudenhet: de två sista delarna kräver en
//! riktig dator. Det här kommandot gör den kvittensen till en **mätning** i stället för ett
//! intryck. Det öppnar ljudenheten med **appens egna inställningar**, spelar en kort ton, läser
//! vad ljudklockan hann gå under tiden och vad MIDI-lagret svarar — och skriver ett utfall i
//! klartext som går att klistra in i ett samtal.
//!
//! Tre saker mäts, och de mäter olika fel:
//!
//! 1. **Enheten öppnas** — annars är resten meningslös, och felet skrivs ut som det är.
//! 2. **Klockan går i realtid.** Ljudtrådens egen position jämförs med väggklockan. Att en
//!    enhet *öppnas* bevisar inte att den *konsumerar*: en tyst callback som aldrig kallas ser
//!    likadan ut ända fram till att inget låter. Ett förhållande nära 1,0 är det som skiljer.
//! 3. **Något hörs.** Toppen på mastern läses medan en trumma slås, så "spelar upp ljud" blir
//!    ett tal och inte en förhoppning. Sedan 2026-09-15 är läget **tyst som standard**:
//!    mixen mäts precis som förut, men ingenting skickas till enheten. `--audible` är
//!    den medvetna vägen till att höra slagen, för en människa som vill kvittera
//!    högtalaren — skälet är att ett autonomt pass körde testet i tid och otid och
//!    spelade trummor i rummet där Alex arbetade.
//!
//! **MIDI-delen mäts på alla plattformar** sedan Fas 7.1 gick över till `midir` (se
//! `audio/midi_input.rs`): samma backend överallt, och listan hämtas färsk så att en klaviatur
//! som kopplas in mitt i en session syns. Att inga portar hittas är **inte** ett underkänt —
//! det kan helt enkelt inte sitta något inkopplat — och raden blir då `➖` i stället för `✅`.
//!
//! **Fönsterlöst med flit:** ingen egui, ingen skärm. Det som bara en människa kan se står sist
//! i utskriften, tydligt åtskilt från det som är mätt.

use std::time::{Duration, Instant};

use crate::audio::command::AudioCommand;
use crate::audio::drum::DrumType;
use crate::audio::midi_input::MidiKeyboardInput;
use crate::audio::{AudioEngine, AudioSettings};

/// En del av testet, med sitt utfall.
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    /// Mätt och godkänt. Texten är beviset, inte berömmet.
    Pass(String),
    /// Mätt och underkänt. Texten säger vad som svarade, inte vad som borde ha svarat.
    Fail(String),
    /// Inte mätbart här, med skälet. Räknas **inte** som godkänt.
    Note(String),
}

/// Vad ljudklockan gjorde under mätfönstret.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockVerdict {
    /// Ljudtråden gick i realtid. Talet är förhållandet mot väggklockan (1,0 = exakt).
    Ran(f32),
    /// Den gick, men långsammare än realtid: bufferten tog slut eller callbacken fick för
    /// lite tid — alltså ett riktigt ljudproblem, inte ett mätfel.
    Behind(f32),
    /// Mätfönstret var för kort för att säga något. Ärligt "inte mätt" i stället för en gissning.
    TooShort,
}

/// Under det här är en topp brus, inte ljud (dither och denormaler ligger långt under, och ett
/// trumslag ligger på helt andra tal).
const AUDIBLE_PEAK: f32 = 0.001;

/// Under den här tiden är mätfönstret för kort för att klockan ska hinna säga något.
const MEASURABLE_SECS: f32 = 0.05;

/// Klockan anses gå i realtid ned till den här andelen av väggklockan. Marginalen finns för att
/// en schemaläggare kan vara upptagen — att hamna under den är inte jitter, det är en buffert
/// som inte hinner fyllas.
const REALTIME_FLOOR: f32 = 0.5;

/// **Regeln hela testet vilar på.** Ren funktion, egen provsvit: att en enhet öppnas är inte
/// samma sak som att den spelar, och skillnaden syns bara som ett förhållande mot väggklockan.
pub fn clock_verdict(start_secs: f32, end_secs: f32, elapsed_secs: f32) -> ClockVerdict {
    if elapsed_secs < MEASURABLE_SECS {
        return ClockVerdict::TooShort;
    }
    let advance = end_secs - start_secs;
    // En klocka som står stilla eller går baklänges är inte "lite långsam".
    if !advance.is_finite() || advance < 0.0 {
        return ClockVerdict::Behind(0.0);
    }
    let ratio = advance / elapsed_secs;
    if ratio >= REALTIME_FLOOR {
        ClockVerdict::Ran(ratio)
    } else {
        ClockVerdict::Behind(ratio)
    }
}

/// Hördes något? Toppen från en slagen trumma är långt över brusgolvet; nollor betyder att
/// ingen signal nådde mastern.
pub fn sound_was_heard(peak: f32) -> bool {
    peak.is_finite() && peak > AUDIBLE_PEAK
}

/// Sammanfattningen: hur många av de **mätbara** delarna som gick igenom, med de underkända
/// namngivna. `Note` räknas inte som godkänd — den räknas inte alls, och det står.
pub fn summary(checks: &[Check]) -> String {
    let passed = checks.iter().filter(|c| matches!(c, Check::Pass(_))).count();
    let measurable = checks
        .iter()
        .filter(|c| !matches!(c, Check::Note(_)))
        .count();
    let failed: Vec<&str> = checks
        .iter()
        .filter_map(|c| match c {
            Check::Fail(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let mut line = format!("SJÄLVTEST: {passed} av {measurable} mätbara delar OK");
    if !failed.is_empty() {
        line.push_str(&format!(" — underkänt: {}", failed.join("; ")));
    }
    let notes = checks.iter().filter(|c| matches!(c, Check::Note(_))).count();
    if notes > 0 {
        line.push_str(&format!(" ({notes} del(ar) inte mätbara här)"));
    }
    line
}

/// Kör mätningen och skriv ut den. `Err` bara när inget alls gick att mäta (enheten gick inte
/// att öppna) — annars bär utskriften utfallet, inklusive underkända delar.
/// Ska självtestet höras i högtalaren? **Standard är nej.** Testet slår kicken med
/// flit (det är så "spelar upp ljud" blir en mätning), och standarden är själva
/// regeln — därför en ren funktion med prov i stället för en rad inuti `run`.
pub fn audible_from_args(args: &[String]) -> bool {
    args.iter().any(|a| a == "--audible")
}

pub fn run(audible: bool) -> Result<(), String> {
    println!("=========================================================");
    println!("  SONIX — SJÄLVTEST (Fas 7.1)");
    println!("=========================================================");
    println!("  Plattform: {}", std::env::consts::OS);
    println!("  Version:   {}", env!("CARGO_PKG_VERSION"));

    // Sökvägarna först: de visar var appen lägger sina filer, vilket är precis vad som skiljer
    // sig mellan plattformarna (och vad roadmapens punkt 3 handlar om).
    let paths = crate::paths::paths();
    println!("\n-- Filvägar --");
    println!("  konfig:    {}", paths.config_dir().display());
    println!("  tillstånd: {}", paths.state_dir().display());
    println!("  cache:     {}", paths.cache_dir().display());
    println!("  bibliotek: {}", paths.music_dir().display());
    println!("  projekt:   {}", paths.projects_dir().display());

    let mut checks: Vec<Check> = Vec::new();

    // -- 1. Ljudenheten, med appens egna inställningar -------------------------------
    let settings = AudioSettings::load();
    println!("\n-- Ljud --");
    let mut engine = match AudioEngine::new_with(
        512,
        settings.sample_rate,
        settings.buffer_frames,
    ) {
        Ok(engine) => {
            println!("  enhet:      {}", engine.device_name);
            println!("  frekvens:   {} Hz", engine.sample_rate);
            match engine.buffer_frames {
                Some(frames) => println!("  buffert:    {frames} frames"),
                None => println!("  buffert:    enhetens standard"),
            }
            let unit = format!("ljudenheten öppnades ({})", engine.device_name);
            checks.push(Check::Pass(unit));
            engine
        }
        Err(e) => {
            println!("  ⚠ enheten gick inte att öppna: {e}");
            checks.push(Check::Fail(format!("ljudenheten gick inte att öppna: {e}")));
            println!("\n{}", summary(&checks));
            println!("\nUtan en ljudenhet går resten inte att mäta.");
            return Ok(());
        }
    };

    // Tyst läge är standard: flaggan sätts efter toppmätningen i callbacken, alltså
    // mäter raden nedan samma mix som förut medan enheten får nollor.
    let silent = !audible;
    if silent {
        engine.set_output_silent(true);
        println!("  läge:       tyst (mixen mäts, inget skickas till enheten)");
    } else {
        println!("  läge:       hörbart (--audible)");
    }

    // -- 2. Klockan i realtid, och 3. att något hörs --------------------------------
    // Trumman slås **under** mätfönstret och toppen läses medan den låter: en topp som läses
    // efteråt har redan klingat av och skulle säga "tyst" om ett fungerande ljud.
    let _ = engine.send_command(AudioCommand::SetSongPlayback(true));
    let start_secs = engine.song_position_secs();
    let wall = Instant::now();
    let mut loudest = 0.0f32;
    let mut hits = 0;
    while wall.elapsed() < Duration::from_millis(1200) {
        if wall.elapsed() >= Duration::from_millis(300 * hits as u64) {
            // Kicken är den kanal som alltid finns i racket — samma väg appen spelar.
            let _ = engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick));
            hits += 1;
        }
        loudest = loudest.max(engine.get_peak_level());
        std::thread::sleep(Duration::from_millis(20));
    }
    // Positionen läses **före** väggklockan: då blir `elapsed` i värsta fall något större än
    // positionen, alltså drar förhållandet nedåt — åt det säkra hållet. En trög ljudtråd är
    // felet vi letar efter, och ett förhållande som ser bättre ut än det är vore farligt.
    let end_secs = engine.song_position_secs();
    let elapsed = wall.elapsed().as_secs_f32();

    println!("\n-- Ljudtråden --");
    match clock_verdict(start_secs, end_secs, elapsed) {
        ClockVerdict::Ran(ratio) => {
            println!(
                "  klockan:    gick {:.2} s på {:.2} s väggklocka ({:.0} % av realtid)",
                end_secs - start_secs,
                elapsed,
                ratio * 100.0
            );
            checks.push(Check::Pass(format!(
                "ljudtråden gick i realtid ({:.0} % av väggklockan)",
                ratio * 100.0
            )));
        }
        ClockVerdict::Behind(ratio) => {
            println!(
                "  ⚠ klockan:  gick {:.2} s på {:.2} s väggklocka ({:.0} % av realtid)",
                end_secs - start_secs,
                elapsed,
                ratio * 100.0
            );
            checks.push(Check::Fail(format!(
                "ljudtråden sackar ({:.0} % av realtid — bufferten hinner inte fyllas)",
                ratio * 100.0
            )));
        }
        ClockVerdict::TooShort => {
            println!("  klockan:    mätfönstret blev för kort för att säga något");
            checks.push(Check::Note("ljudtrådens klocka: för kort mätning".to_string()));
        }
    }

    // En extra runda: klockan kan gå medan mastern är tyst (ingen kanal, allt mutat).
    let mut loudest_two = loudest;
    for _ in 0..8 {
        let _ = engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick));
        std::thread::sleep(Duration::from_millis(60));
        loudest_two = loudest_two.max(engine.get_peak_level());
    }
    let _ = engine.send_command(AudioCommand::SetSongPlayback(false));
    println!("  topp:       {loudest:.4} (3 slag under mätfönstret, {hits} skickade)");
    if sound_was_heard(loudest_two) {
        checks.push(Check::Pass(format!(
            "en slagen trumma nådde mastern (topp {loudest_two:.4})"
        )));
    } else {
        checks.push(Check::Fail(format!(
            "ingen signal nådde mastern (topp {loudest_two:.4}) — enheten spelar, men tyst"
        )));
    }
    // Tyst läge mäts lika hårt men kvitterar inte högtalaren: säg det, i stället för att
    // låta en grön rad påstå något den inte täcker.
    if silent {
        checks.push(Check::Note(
            "tyst läge: mixen mättes, men inget skickades till enheten (kör --audible för att höra slagen)"
                .to_string(),
        ));
    }

    // -- 4. MIDI-in (ärlig, inte grön) ---------------------------------------------
    println!("\n-- MIDI in --");
    let (tx, _rx) = std::sync::mpsc::channel();
    match MidiKeyboardInput::connect(tx) {
        Ok(keyboard) => {
            let ports = keyboard.device_list();
            if ports.is_empty() {
                println!("  portar:     inga hittades (inget inkopplat)");
                checks.push(Check::Note(
                    "MIDI-in: inga portar hittades (inget inkopplat)".to_string(),
                ));
            } else {
                for port in &ports {
                    println!("  port:       {port}");
                }
                checks.push(Check::Pass(format!("MIDI-in hittade {} port(ar)", ports.len())));
            }
        }
        Err(e) => {
            // Att MIDI-lagret inte gick att starta är ett riktigt fel (drivrutin, behörighet)
            // och skrivs ut som det är i stället för att tigas bort.
            println!("  ⚠ {e}");
            checks.push(Check::Note(format!("MIDI-in: {e}")));
        }
    }

    // -- Utfallet -------------------------------------------------------------------
    println!("\n-- Mätt --");
    for check in &checks {
        match check {
            Check::Pass(text) => println!("  ✅ {text}"),
            Check::Fail(text) => println!("  ❌ {text}"),
            Check::Note(text) => println!("  ➖ {text}"),
        }
    }
    println!("\n{}", summary(&checks));
    println!("\nDet som INTE är mätt, och bara en människa kan se:");
    println!("  • att fönstret ritas upp och går att klicka i");
    println!("  • att transporten startar och att markören rör sig");
    if silent {
        println!("  • att slagen hörs i högtalaren — `sonix --selftest --audible` ger den kvittensen");
    }
    println!("Öppna appen utan flaggan och tryck på play — då är sista raden kvitterad.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Provet som gör självtestet värt något.** En enhet som *öppnas* men aldrig får sin
    /// callback anropad ser identisk ut i varje annan mätning — det är förhållandet mot
    /// väggklockan som skiljer dem.
    #[test]
    fn an_opened_device_that_never_runs_is_not_realtime() {
        // Callbacken kör aldrig: klockan står stilla medan väggen går.
        assert_eq!(clock_verdict(0.0, 0.0, 1.0), ClockVerdict::Behind(0.0));
        // Den går, men sackar: bufferten hinner inte fyllas.
        assert_eq!(clock_verdict(0.0, 0.2, 1.0), ClockVerdict::Behind(0.2));
        // Realtid, och exakt.
        assert_eq!(clock_verdict(0.0, 1.0, 1.0), ClockVerdict::Ran(1.0));
        // Lite efter är fortfarande realtid — en upptagen schemaläggare är inte en regress.
        assert_eq!(clock_verdict(0.0, 0.9, 1.0), ClockVerdict::Ran(0.9));
        // En klocka som går baklänges är inte "lite långsam".
        assert_eq!(clock_verdict(5.0, 1.0, 1.0), ClockVerdict::Behind(0.0));
    }

    /// För kort fönster ger **"inte mätt"**, inte en gissning åt något håll.
    #[test]
    fn a_window_too_short_says_so_instead_of_guessing() {
        assert_eq!(clock_verdict(0.0, 0.0, 0.001), ClockVerdict::TooShort);
        assert_eq!(clock_verdict(0.0, 0.0009, 0.001), ClockVerdict::TooShort);
    }

    /// Brusgolvet: nollor är tyst, en slagen trumma är ljud. Skräp (NaN) får inte bli "hördes".
    #[test]
    fn silence_is_not_sound() {
        assert!(!sound_was_heard(0.0));
        assert!(!sound_was_heard(f32::NAN));
        assert!(!sound_was_heard(-0.9)); // en negativ topp är inte ett mått
        assert!(sound_was_heard(0.5));
        assert!(sound_was_heard(AUDIBLE_PEAK * 2.0));
    }

    /// Sammanfattningen räknar bara det **mätbara**, och namnger det underkända. En anteckning
    /// får aldrig bli ett godkännande.
    #[test]
    fn the_summary_counts_the_measurable_and_names_the_failures() {
        let all_good = vec![
            Check::Pass("ljudet".into()),
            Check::Pass("klockan".into()),
        ];
        assert_eq!(summary(&all_good), "SJÄLVTEST: 2 av 2 mätbara delar OK");

        let with_note = vec![
            Check::Pass("ljudet".into()),
            Check::Note("MIDI-in: stubbe".into()),
        ];
        assert_eq!(
            summary(&with_note),
            "SJÄLVTEST: 1 av 1 mätbara delar OK (1 del(ar) inte mätbara här)"
        );

        let with_fail = vec![
            Check::Pass("ljudet".into()),
            Check::Fail("klockan sackar".into()),
            Check::Note("MIDI-in: stubbe".into()),
        ];
        let line = summary(&with_fail);
        assert!(line.contains("1 av 2 mätbara delar OK"), "{line}");
        assert!(line.contains("underkänt: klockan sackar"), "{line}");
    }

    /// **Provet som håller standarden fast.** Faller detta om någon gör testet
    /// hörbart igen, är tystnaden inte en åsikt utan en regel.
    #[test]
    fn the_selftest_is_silent_unless_audible_is_asked_for() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(!audible_from_args(&args(&["sonix", "--selftest"])));
        assert!(!audible_from_args(&args(&["sonix"])));
        assert!(audible_from_args(&args(&["sonix", "--selftest", "--audible"])));
    }
}
