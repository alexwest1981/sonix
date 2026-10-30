//! **Projektskript:** samma Rhai-sandlåda som pianorullens skript, men ytan är projektet i
//! stället för en tagning — tempo, kanaler och deras steg. Skriptet får en **kopia** av det det
//! rör, precis som tagning-skriptet: går det inte igenom skrivs ingenting tillbaka.
//!
//! Status: stabil — egen körningsyta, egna prov.

/// Kanalen som skriptet ser den: namn och de 64 stegen.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptChannel {
    pub name: String,
    pub steps: Vec<bool>,
}

/// Projektets yta. Bara det här får ett skript röra — ingen motor, inga filer, inga nätanrop.
#[derive(Clone, Debug, PartialEq)]
pub struct Ctx {
    pub bpm: f32,
    pub pattern_count: usize,
    pub channels: Vec<ScriptChannel>,
}

impl Ctx {
    /// Steglängden som skriptet arbetar mot. Alla kanaler har samma antal steg i appen, men
    /// kopian kan vara kortare i ett äldre projekt — den längsta vinner.
    pub fn steps(&self) -> usize {
        self.channels.iter().map(|c| c.steps.len()).max().unwrap_or(0)
    }
}

/// Kör ett projektskript. `Ok` är det skriptet skrev med `print`, `Err` är felet som ska visas
/// (rad och kolumn kommer från Rhai).
pub fn run(source: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    run_named(source, "", ctx)
}

/// **Ett vägval för båda språken.** Första raden `--!lua`, eller ett namn som slutar på `.lua`,
/// går till den inbyggda Lua 5.4; allt annat behåller Rhai. Anropare bryr sig inte om vilket.
pub fn run_named(source: &str, name: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    let wants_lua =
        name.ends_with(".lua") || source.lines().next().is_some_and(|line| line.trim() == "--!lua");
    if wants_lua {
        crate::audio::lua_script::run(source, ctx)
    } else {
        run_rhai(source, ctx)
    }
}

fn run_rhai(source: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(2_000_000);

    let shared = std::rc::Rc::new(std::cell::RefCell::new(ctx.clone()));

    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("bpm", move || shared.borrow().bpm as f64);
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_bpm", move |value: f64| {
            let mut ctx = shared.borrow_mut();
            // Samma gränser som temporeglaget: under 20 eller över 999 är inte musik.
            ctx.bpm = (value as f32).clamp(20.0, 999.0);
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("pattern_count", move || {
            shared.borrow().pattern_count as i64
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("steps", move || shared.borrow().steps() as i64);
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("channel_count", move || {
            shared.borrow().channels.len() as i64
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("channel_name", move |index: i64| -> String {
            shared
                .borrow()
                .channels
                .get(index.max(0) as usize)
                .map(|c| c.name.clone())
                .unwrap_or_default()
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("step", move |channel: i64, step: i64| -> bool {
            shared
                .borrow()
                .channels
                .get(channel.max(0) as usize)
                .and_then(|c| c.steps.get(step.max(0) as usize).copied())
                .unwrap_or(false)
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_step", move |channel: i64, step: i64, on: bool| {
            let mut ctx = shared.borrow_mut();
            if let Some(ch) = ctx.channels.get_mut(channel.max(0) as usize)
                && let Some(slot) = ch.steps.get_mut(step.max(0) as usize)
            {
                *slot = on;
            }
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("clear_channel", move |channel: i64| {
            let mut ctx = shared.borrow_mut();
            if let Some(ch) = ctx.channels.get_mut(channel.max(0) as usize) {
                ch.steps.iter_mut().for_each(|s| *s = false);
            }
        });
    }

    // **Samplingshuvud, inte samplingsdata.** Skriptet får längd och format på en fil utan att
    // någon lastar in den; en oläsbar fil ger 0 i stället för att avbryta skriptet.
    engine.register_fn("sample_seconds", |path: &str| -> f64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.seconds())
            .unwrap_or(0.0)
    });
    engine.register_fn("sample_rate", |path: &str| -> i64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.sample_rate as i64)
            .unwrap_or(0)
    });
    engine.register_fn("sample_frames", |path: &str| -> i64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.frames as i64)
            .unwrap_or(0)
    });

    let sink = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    {
        let sink = std::rc::Rc::clone(&sink);
        engine.on_print(move |text| sink.borrow_mut().push(text.to_string()));
    }
    if let Err(e) = engine.run(source) {
        return Err(format!("{e}"));
    }

    // Bara om skriptet gick igenom: kopian skrivs tillbaka.
    *ctx = shared.borrow().clone();
    let log = sink.borrow().clone();
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx {
            bpm: 124.0,
            pattern_count: 3,
            channels: vec![
                ScriptChannel { name: "Kick".into(), steps: vec![false; 16] },
                ScriptChannel { name: "Snare".into(), steps: vec![false; 16] },
            ],
        }
    }

    #[test]
    fn a_project_script_sets_tempo_and_steps() {
        let mut c = ctx();
        run(
            "set_bpm(140.0); set_step(0, 0, true); set_step(0, 8, true); print(channel_name(0));",
            &mut c,
        )
        .unwrap();
        assert_eq!(c.bpm, 140.0);
        assert!(c.channels[0].steps[0] && c.channels[0].steps[8]);
        assert!(!c.channels[1].steps[0], "bara kanal 0 rörs");
    }

    #[test]
    fn a_failing_script_leaves_the_project_alone() {
        let mut c = ctx();
        let before = c.clone();
        assert!(run("set_bpm(90.0); nope();", &mut c).is_err());
        assert_eq!(c, before, "halvvägs arbete ska inte skrivas tillbaka");
    }

    #[test]
    fn a_script_can_read_a_sample_header() {
        let path = std::env::temp_dir().join("sonix_script_sample.wav");
        let path = path.to_string_lossy().to_string();
        let samples = vec![0.0f32; 24_000];
        crate::audio::wav_writer::write_pcm_f32_to_wav(&path, &samples, 48_000, 1).unwrap();

        let mut c = ctx();
        let log = run(
            &format!("print(sample_rate(\"{path}\")); print(sample_seconds(\"{path}\"));"),
            &mut c,
        )
        .unwrap();
        assert_eq!(log[0], "48000");
        assert!(log[1].parse::<f64>().unwrap() - 0.5 < 1e-6, "{log:?}");
        let _ = std::fs::remove_file(&path);

        let mut c = ctx();
        let log = run("print(sample_seconds(\"/finns/inte.wav\"));", &mut c).unwrap();
        assert_eq!(log[0], "0.0", "oläsbar fil ger 0, inte ett avbrott");
    }

    #[test]
    fn steps_outside_the_grid_are_ignored() {
        let mut c = ctx();
        run("set_step(0, 999, true); set_step(99, 0, true);", &mut c).unwrap();
        assert!(c.channels[0].steps.iter().all(|s| !*s));
    }

    #[test]
    fn a_lua_header_picks_the_lua_engine() {
        let mut c = ctx();
        let log = run("--!lua\nset_bpm(150)\nprint(bpm())", &mut c).unwrap();
        assert_eq!(c.bpm, 150.0);
        assert_eq!(log, vec!["150".to_string()], "Rhai would have failed on that syntax");

        let mut by_name = ctx();
        run_named("set_bpm(150)", "drums.lua", &mut by_name).unwrap();
        assert_eq!(by_name.bpm, 150.0);

        let mut plain = ctx();
        run_named("set_bpm(150.0)", "drums.rhai", &mut plain).unwrap();
        assert_eq!(plain.bpm, 150.0);
    }
}
