//! **Pianorullsskript** (Rhai): en liten körningsyta för att ändra noterna i en tagning.
//!
//! Skriptet får en lista noter och kan läsa och skriva dem. Allt arbete sker på en **kopia**:
//! ett skript som kastar fel halvvägs lämnar tagningen orörd, för en trasig rad ska inte kunna
//! halvtransponera ett färdigt pattern.
//!
//! Ytan är medvetet liten — noter, inte projektet:
//!
//! ```rhai
//! // Transponera allt utom bastonerna och mjuka upp anslagen.
//! let n = count();
//! for i in 0..n {
//!     if pitch(i) >= 60 { set_pitch(i, pitch(i) + 12); }
//!     set_velocity(i, velocity(i) * 0.8);
//! }
//! print(`ändrade ${n} noter`);
//! ```
//!
//! `print` hamnar i statusraden, inte i terminalen — skriptet körs i UI-tråden.

use crate::midi_take::Take;

/// Lägsta och högsta notnummer som får skrivas. Ett skript ska inte kunna skriva 300 och
/// tysta noten för alltid.
const KEY_RANGE: std::ops::RangeInclusive<u8> = 0..=127;

/// Kör ett skript mot en tagning. `Ok` innehåller det skriptet skrev med `print`, `Err` är
/// felet som ska visas för användaren (rad och kolumn kommer från Rhai).
pub fn run(source: &str, take: &mut Take) -> Result<Vec<String>, String> {
    // Kopian är poängen: allt nedan muterar `notes`, och tagningen skrivs bara tillbaka om
    // skriptet gick hela vägen.
    let notes = take.notes.clone();

    let mut engine = rhai::Engine::new();
    engine.set_max_operations(2_000_000);

    // Skrivare och läsare delar samma `Rc<RefCell>`: `set_pitch` följt av `pitch` måste se
    // samma tagning, annars vore skriptet en lögn.
    let shared = std::rc::Rc::new(std::cell::RefCell::new(notes));

    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("count", move || shared.borrow().len() as i64);
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("pitch", move |i: i64| -> i64 {
            shared.borrow().get(i as usize).map_or(0, |n| n.key as i64)
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("velocity", move |i: i64| -> f64 {
            shared.borrow().get(i as usize).map_or(0.0, |n| n.velocity as f64)
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("start", move |i: i64| -> f64 {
            shared.borrow().get(i as usize).map_or(0.0, |n| n.pos as f64)
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_pitch", move |i: i64, key: i64| {
            if let Some(note) = shared.borrow_mut().get_mut(i as usize) {
                note.key = key.clamp(*KEY_RANGE.start() as i64, *KEY_RANGE.end() as i64) as u8;
            }
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_velocity", move |i: i64, v: f64| {
            if let Some(note) = shared.borrow_mut().get_mut(i as usize) {
                note.velocity = (v as f32).clamp(0.0, 1.0);
            }
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_start", move |i: i64, pos: f64| {
            if let Some(note) = shared.borrow_mut().get_mut(i as usize) {
                note.pos = (pos as f32).clamp(0.0, 15.99);
            }
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("transpose", move |semitones: i64| {
            for note in shared.borrow_mut().iter_mut() {
                let key = note.key as i64 + semitones;
                note.key = key.clamp(*KEY_RANGE.start() as i64, *KEY_RANGE.end() as i64) as u8;
            }
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("scale_velocity", move |factor: f64| {
            for note in shared.borrow_mut().iter_mut() {
                note.velocity = (note.velocity * factor as f32).clamp(0.0, 1.0);
            }
        });
    }

    // `print` är Rhais egen funktion; texten fångas med `on_print` i stället för att
    // skrivas till terminalen — skriptet körs i UI-tråden och resultatet ska till statusraden.
    let sink = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    {
        let sink = std::rc::Rc::clone(&sink);
        engine.on_print(move |text| sink.borrow_mut().push(text.to_string()));
    }
    if let Err(e) = engine.run(source) {
        return Err(format!("{e}"));
    }

    // Bara om skriptet gick igenom: `take` rörs inte på felvägen.
    take.notes = shared.borrow().clone();
    let log = sink.borrow().clone();
    Ok(log)
}

/// **Skriptmappen.** `.rhai`-filer här listas i pianorullens meny; en fil som ligger någon
/// annanstans går att köra via filväljaren.
pub fn script_dir() -> std::path::PathBuf {
    // Sökvägar byggs bara i `paths.rs` — även de här (SONIX_CONFIG_DIR och XDG styr).
    crate::paths::paths().config_dir().join("scripts")
}

/// Ett exempelskript som skrivs första gången mappen används — menyn ska visa ytan, inte en
/// tom ruta, och filen är samtidigt dokumentationen av vad ett skript får göra.
const SAMPLE: &str = r#"// Rullande bas: transponera ned en oktav och mjuka upp anslagen.
// API: count(), pitch(i), velocity(i), start(i),
//      set_pitch(i, n), set_velocity(i, v), set_start(i, pos),
//      transpose(semitones), scale_velocity(faktor), print(text)
let n = count();
for i in 0..n {
    if pitch(i) >= 60 { set_pitch(i, pitch(i) - 12); }
    set_velocity(i, velocity(i) * 0.8);
}
print(`ändrade ${n} noter`);
"#;

pub fn scripts() -> Vec<std::path::PathBuf> {
    let dir = script_dir();
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("exempel.rhai"), SAMPLE);
    }
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "rhai"))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take() -> Take {
        let mut t = Take::new();
        t.push(0.0, 60, 0.5);
        t.push(4.0, 64, 1.0);
        t
    }

    #[test]
    fn a_script_can_transpose_and_read_back_what_it_wrote() {
        let mut t = take();
        run(
            "transpose(12); set_velocity(0, 0.9); print(`kvar ${count()}`);",
            &mut t,
        )
        .expect("skriptet ska gå igenom");
        assert_eq!(t.notes[0].key, 72);
        assert_eq!(t.notes[1].key, 76);
        assert!((t.notes[0].velocity - 0.9).abs() < 1e-6);
    }

    /// `pitch` efter `set_pitch` måste se det skriptet själv skrev.
    #[test]
    fn reads_see_writes_in_the_same_script() {
        let mut t = take();
        let log = run(
            "set_pitch(0, 40); print(`${pitch(0)}`);",
            &mut t,
        )
        .expect("skriptet ska gå igenom");
        assert_eq!(log, vec!["40".to_string()]);
    }

    /// Ett skript som kastar fel halvvägs får inte lämna tagningen halvändrad.
    #[test]
    fn a_failing_script_leaves_the_take_untouched() {
        let mut t = take();
        let before = t.clone();
        let err = run("transpose(12); nope();", &mut t);
        assert!(err.is_err(), "okänt anrop ska ge fel");
        assert_eq!(t.notes, before.notes);
    }

    /// Notnummer utanför 0–127 kläms, så en trasig loop inte kan tysta en not för alltid.
    #[test]
    fn pitches_are_clamped_to_the_midi_range() {
        let mut t = take();
        run("set_pitch(0, 400); set_pitch(1, -20);", &mut t).unwrap();
        assert_eq!(t.notes[0].key, 127);
        assert_eq!(t.notes[1].key, 0);
    }

    /// Ett skript får inte kunna snurra för evigt och låsa UI-tråden.
    #[test]
    fn an_endless_loop_is_stopped() {
        let mut t = take();
        let err = run("loop { }", &mut t);
        assert!(err.is_err(), "oändlig loop ska stoppas av operationsgränsen");
    }
}
