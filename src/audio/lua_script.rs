//! **Project script, Lua edition:** the same sandbox as `project_script`, run by the embedded
//! Lua 5.4 instead of Rhai. Every host function carries the same name and meaning as its Rhai
//! twin, so a script ports between the two languages by changing its syntax only. The surface is
//! documented in full in `project_script`.
//!
//! Status: stable — own run surface, own tests.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use mlua::{HookTriggers, Lua, LuaOptions, StdLib, VmState};

use super::project_script::Ctx;

/// Instructions the VM may execute before the script is stopped. A project script spends a few
/// thousand; `while true do end` is cut here instead of hanging the app.
const INSTRUCTION_BUDGET: u32 = 5_000_000;
/// How often the hook above wakes up. Coarse enough to cost nothing on real scripts.
const HOOK_STEP: u32 = 10_000;

/// Runs a Lua project script. `Ok` is what the script printed, `Err` is the message to show
/// (Lua puts the line number in it).
pub fn run(source: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    let lua = new_state().map_err(err)?;
    let shared = Rc::new(RefCell::new(ctx.clone()));
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    register(&lua, &shared, &log).map_err(err)?;

    lua.load(source).set_name("project script").exec().map_err(err)?;

    // Only a script that went through writes back.
    *ctx = shared.borrow().clone();
    Ok(log.borrow().clone())
}

/// The host functions and the `print` sink — the whole surface a script gets.
fn register(
    lua: &Lua,
    shared: &Rc<RefCell<Ctx>>,
    log: &Rc<RefCell<Vec<String>>>,
) -> mlua::Result<()> {
    let globals = lua.globals();

    // **Sample header, not sample data.** Length and format of a file without loading it; an
    // unreadable file answers 0 instead of stopping the script.
    let g = Rc::clone(shared);
    globals.set("bpm", lua.create_function(move |_, ()| Ok(g.borrow().bpm as f64))?)?;
    let g = Rc::clone(shared);
    globals.set("set_bpm", lua.create_function(move |_, value: f64| {
        // Same limits as the tempo control: below 20 or above 999 is not music.
        g.borrow_mut().bpm = (value as f32).clamp(20.0, 999.0);
        Ok(())
    })?)?;
    let g = Rc::clone(shared);
    globals.set("pattern_count", lua.create_function(move |_, ()| {
        Ok(g.borrow().pattern_count as i64)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("steps", lua.create_function(move |_, ()| Ok(g.borrow().steps() as i64))?)?;
    let g = Rc::clone(shared);
    globals.set("channel_count", lua.create_function(move |_, ()| {
        Ok(g.borrow().channels.len() as i64)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("channel_name", lua.create_function(move |_, index: i64| {
        Ok(g.borrow()
            .channels
            .get(index.max(0) as usize)
            .map(|c| c.name.clone())
            .unwrap_or_default())
    })?)?;
    let g = Rc::clone(shared);
    globals.set("step", lua.create_function(move |_, (channel, step): (i64, i64)| {
        Ok(g.borrow()
            .channels
            .get(channel.max(0) as usize)
            .and_then(|c| c.steps.get(step.max(0) as usize).copied())
            .unwrap_or(false))
    })?)?;
    let g = Rc::clone(shared);
    globals.set("set_step", lua.create_function(move |_, (channel, step, on): (i64, i64, bool)| {
        let mut ctx = g.borrow_mut();
        if let Some(ch) = ctx.channels.get_mut(channel.max(0) as usize)
            && let Some(slot) = ch.steps.get_mut(step.max(0) as usize)
        {
            *slot = on;
        }
        Ok(())
    })?)?;
    let g = Rc::clone(shared);
    globals.set("clear_channel", lua.create_function(move |_, channel: i64| {
        if let Some(ch) = g.borrow_mut().channels.get_mut(channel.max(0) as usize) {
            ch.steps.iter_mut().for_each(|s| *s = false);
        }
        Ok(())
    })?)?;
    globals.set("sample_seconds", lua.create_function(|_, path: String| {
        Ok(crate::audio::wav_reader::wav_info(&path).map(|i| i.seconds()).unwrap_or(0.0))
    })?)?;
    globals.set("sample_rate", lua.create_function(|_, path: String| {
        Ok(crate::audio::wav_reader::wav_info(&path).map(|i| i.sample_rate as i64).unwrap_or(0))
    })?)?;
    globals.set("sample_frames", lua.create_function(|_, path: String| {
        Ok(crate::audio::wav_reader::wav_info(&path).map(|i| i.frames as i64).unwrap_or(0))
    })?)?;

    // The project surface. Same names, same meanings as the Rhai edition.
    let g = Rc::clone(shared);
    globals.set("track_count", lua.create_function(move |_, ()| Ok(g.borrow().track_count()))?)?;
    let g = Rc::clone(shared);
    globals.set("track_name", lua.create_function(move |_, track: i64| {
        g.borrow().track_name(track).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("set_track_name", lua.create_function(move |_, (track, name): (i64, String)| {
        g.borrow_mut().set_track_name(track, &name).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("track_volume", lua.create_function(move |_, track: i64| {
        g.borrow().track_volume(track).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("set_track_volume", lua.create_function(move |_, (track, value): (i64, f64)| {
        g.borrow_mut().set_track_volume(track, value).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("track_mute", lua.create_function(move |_, track: i64| {
        g.borrow().track_mute(track).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("set_track_mute", lua.create_function(move |_, (track, on): (i64, bool)| {
        g.borrow_mut().set_track_mute(track, on).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("clip_count", lua.create_function(move |_, track: i64| {
        g.borrow().clip_count(track).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("clip_len", lua.create_function(move |_, (track, clip): (i64, i64)| {
        g.borrow().clip_len(track, clip).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("set_clip_len", lua.create_function(move |_, (track, clip, steps): (i64, i64, i64)| {
        g.borrow_mut().set_clip_len(track, clip, steps).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("note_count", lua.create_function(move |_, (track, clip): (i64, i64)| {
        g.borrow().note_count(track, clip).map_err(mlua::Error::runtime)
    })?)?;
    let g = Rc::clone(shared);
    globals.set("note_at", lua.create_function(move |lua, (track, clip, index): (i64, i64, i64)| {
        let note = g.borrow().note_at(track, clip, index).map_err(mlua::Error::runtime)?;
        let table = lua.create_table()?;
        table.set("step", note.step as i64)?;
        table.set("note", note.note as i64)?;
        table.set("velocity", note.velocity as f64)?;
        Ok(table)
    })?)?;
    let g = Rc::clone(shared);
    globals.set(
        "add_note",
        lua.create_function(move |_, (track, clip, step, note, velocity): (i64, i64, i64, i64, f64)| {
            g.borrow_mut()
                .add_note(track, clip, step, note, velocity)
                .map_err(mlua::Error::runtime)
        })?,
    )?;
    let g = Rc::clone(shared);
    globals.set("clear_notes", lua.create_function(move |_, (track, clip): (i64, i64)| {
        g.borrow_mut().clear_notes(track, clip).map_err(mlua::Error::runtime)
    })?)?;

    let sink = Rc::clone(log);
    globals.set("print", lua.create_function(move |_, args: mlua::Variadic<mlua::Value>| {
        let text = args
            .iter()
            .map(|v| v.to_string().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\t");
        sink.borrow_mut().push(text);
        Ok(())
    })?)?;
    // `log` is the same sink as `print`, for one value.
    let sink = Rc::clone(log);
    globals.set("log", lua.create_function(move |_, value: mlua::Value| {
        sink.borrow_mut().push(value.to_string().unwrap_or_default());
        Ok(())
    })?)?;

    let spent = Cell::new(0u32);
    lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_STEP), move |_, _| {
        spent.set(spent.get().saturating_add(HOOK_STEP));
        if spent.get() > INSTRUCTION_BUDGET {
            return Err(mlua::Error::runtime(format!(
                "script stopped after {INSTRUCTION_BUDGET} instructions"
            )));
        }
        Ok(VmState::Continue)
    });
    Ok(())
}

/// A Lua with the libraries a project script needs and none of the ones that touch the machine.
/// `io`, `os`, `package` and `debug` stay out: the functions above are the whole surface.
fn new_state() -> mlua::Result<Lua> {
    Lua::new_with(
        StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH,
        LuaOptions::default(),
    )
}

fn err(e: mlua::Error) -> String {
    format!("{e}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::project_script::{self, ScriptChannel, ScriptClip};

    fn ctx() -> Ctx {
        Ctx {
            bpm: 124.0,
            pattern_count: 3,
            channels: vec![
                ScriptChannel::new("Kick", vec![false; 16]),
                ScriptChannel::new("Snare", vec![false; 16]),
            ],
            clips: vec![
                ScriptClip::from_grid(0, &[false; 16], &[60; 16]),
                ScriptClip::from_grid(1, &[false; 16], &[60; 16]),
            ],
        }
    }

    /// Prints the whole project so a failing comparison shows what differs.
    fn summary(c: &Ctx) -> String {
        format!("{c:?}")
    }

    /// (a) The same edits in both languages land in the same project.
    #[test]
    fn lua_and_rhai_make_the_same_project_changes() {
        let rhai_source =
            "set_bpm(140.0); set_step(0, 0, true); set_step(0, 8, true); set_step(1, 4, true);";
        let lua_source =
            "set_bpm(140); set_step(0, 0, true); set_step(0, 8, true); set_step(1, 4, true)";

        let mut from_rhai = ctx();
        project_script::run(rhai_source, &mut from_rhai).unwrap();
        let mut from_lua = ctx();
        run(lua_source, &mut from_lua).unwrap();

        println!("rhai: bpm {} channels {:?}", from_rhai.bpm, from_rhai.channels);
        println!("lua:  bpm {} channels {:?}", from_lua.bpm, from_lua.channels);
        assert_eq!(from_lua.bpm, from_rhai.bpm);
        assert_eq!(from_lua.channels, from_rhai.channels);
        assert_eq!(from_lua.bpm, 140.0);
    }

    /// (a) One script written twice — rename track 0, set its volume, add four notes to clip 0,
    /// log the note count — must leave byte-identical `Ctx` behind.
    #[test]
    fn the_same_song_script_builds_the_same_project_in_both_languages() {
        let rhai_source = r#"
            set_track_name(0, "Kick 808");
            set_track_volume(0, 0.5);
            set_track_mute(1, true);
            for s in [0, 4, 8, 12] { add_note(0, 0, s, 36 + s, 0.8); }
            log(note_count(0, 0));
        "#;
        let lua_source = r#"
            set_track_name(0, "Kick 808")
            set_track_volume(0, 0.5)
            set_track_mute(1, true)
            for _, s in ipairs({0, 4, 8, 12}) do add_note(0, 0, s, 36 + s, 0.8) end
            log(note_count(0, 0))
        "#;

        let mut from_rhai = ctx();
        let rhai_log = project_script::run(rhai_source, &mut from_rhai).unwrap();
        let mut from_lua = ctx();
        let lua_log = run(lua_source, &mut from_lua).unwrap();

        println!("rhai: {}", summary(&from_rhai));
        println!("lua:  {}", summary(&from_lua));
        println!("log rhai {rhai_log:?} lua {lua_log:?}");

        assert_eq!(rhai_log, lua_log, "the same script logs the same note count");
        assert_eq!(lua_log, vec!["4".to_string()]);
        assert_eq!(
            summary(&from_lua),
            summary(&from_rhai),
            "the same script leaves byte-identical projects"
        );
        assert_eq!(from_lua, from_rhai);
        assert_eq!(from_rhai.channels[0].name, "Kick 808");
        assert_eq!(from_rhai.clips[0].notes.len(), 4);
    }

    /// (b) `note_at` hands back the numbers `add_note` was given — in both engines.
    #[test]
    fn note_at_returns_what_add_note_wrote() {
        let written = [(0, 36, 0.25), (5, 48, 0.5), (9, 60, 0.75), (15, 72, 0.875)];
        let expected: Vec<String> = written
            .iter()
            .flat_map(|(step, note, velocity)| {
                [step.to_string(), note.to_string(), velocity.to_string()]
            })
            .collect();

        let rhai_script =
            "for i in 0..4 { let n = note_at(0, 0, i); log(n.step); log(n.note); log(n.velocity); }";
        let lua_script = r#"
            for i = 0, 3 do
                local n = note_at(0, 0, i)
                log(n.step) log(n.note) log(n.velocity)
            end
        "#;

        for (engine, script) in [("rhai", rhai_script), ("lua", lua_script)] {
            let runner: fn(&str, &mut Ctx) -> Result<Vec<String>, String> =
                if engine == "rhai" { project_script::run } else { run };
            let mut c = ctx();
            let mut adds = String::new();
            for &(step, note, velocity) in written.iter() {
                adds.push_str(&format!("add_note(0, 0, {step}, {note}, {velocity});\n"));
            }
            // Lua has no trailing semicolons to mind, so the same statements serve both.
            let source = format!("{adds}{script}");
            let log = runner(&source, &mut c).unwrap();
            println!("{engine} note_at (step, note, velocity) x4: {log:?}");
            assert_eq!(log, expected, "{engine}");
            for (i, (step, note, velocity)) in written.iter().enumerate() {
                let got = c.note_at(0, 0, i as i64).unwrap();
                assert_eq!((got.step, got.note), (*step as usize, *note as u8));
                assert!((got.velocity - *velocity as f32).abs() < 1e-6, "{got:?}");
            }
            assert_eq!(c.note_count(0, 0).unwrap(), 4);
        }
    }

    /// (c) Indices outside the project are script errors, not panics, in both engines.
    #[test]
    fn out_of_range_indices_are_errors_in_both_engines() {
        for (engine, source) in [
            ("rhai", "note_count(99, 0);"),
            ("rhai", "note_count(0, 99);"),
            ("lua", "note_count(99, 0)"),
            ("lua", "note_count(0, 99)"),
        ] {
            let mut c = ctx();
            let before = c.clone();
            let err = project_script::run(source, &mut c).unwrap_err();
            println!("{engine} {source} -> {err}");
            assert!(
                err.contains("track 99") || err.contains("clip 99"),
                "{engine} {source}: {err}"
            );
            assert_eq!(c, before, "a refused index writes nothing back");
        }

        // A track that exists but a clip that does not is refused the same way.
        let mut c = ctx();
        let err = run("track_name(2)", &mut c).unwrap_err();
        println!("lua track_name(2) -> {err}");
        assert!(err.contains("track 2 is outside 0..2"), "{err}");
    }

    /// (c) A syntax error answers with the line it broke on.
    #[test]
    fn a_syntax_error_carries_the_line_number() {
        let mut c = ctx();
        let before = c.clone();
        let err = run("set_bpm(140)\nthis is not lua\n", &mut c).unwrap_err();
        println!("syntax error: {err}");
        assert!(err.contains(":2:"), "{err}");
        assert_eq!(c, before, "a script that failed writes nothing back");
    }

    /// (c) An endless loop is stopped by the instruction budget, not by the user.
    #[test]
    fn a_runaway_script_is_stopped() {
        let mut c = ctx();
        let started = std::time::Instant::now();
        let err = run("while true do end", &mut c).unwrap_err();
        let elapsed = started.elapsed();
        println!("runaway stopped in {elapsed:?}: {err}");
        assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
    }

    /// (d) All twelve host functions are there, once each.
    #[test]
    fn all_twelve_functions_are_registered() {
        let mut c = ctx();
        let log = run(
            r#"
local function show(label, ...) print(label, ...) end
show("bpm", bpm())
show("set_bpm", set_bpm(140))
show("pattern_count", pattern_count())
show("steps", steps())
show("channel_count", channel_count())
show("channel_name", channel_name(0))
show("step", step(0, 0))
show("set_step", set_step(0, 0, true))
show("clear_channel", clear_channel(1))
show("sample_seconds", sample_seconds("/does/not/exist.wav"))
show("sample_rate", sample_rate("/does/not/exist.wav"))
show("sample_frames", sample_frames("/does/not/exist.wav"))
"#,
            &mut c,
        )
        .unwrap();
        for line in &log {
            println!("{line}");
        }
        assert_eq!(log.len(), 12, "{log:?}");
        assert_eq!(c.bpm, 140.0);
        assert!(c.channels[0].steps[0]);
        assert_eq!(log[0], "bpm\t124");
        assert_eq!(log[5], "channel_name\tKick");
    }

    /// (e) A script that fills two clips with eight notes is fast enough to run on a keystroke.
    #[test]
    fn building_eight_notes_across_two_clips_is_fast() {
        let rhai_source = r#"
            for track in 0..2 {
                for step in [0, 2, 4, 6] {
                    add_note(track, 0, step, 36 + step, 0.8);
                }
            }
            set_clip_len(1, 0, 8);
        "#;
        let lua_source = r#"
            for track = 0, 1 do
                for _, step in ipairs({0, 2, 4, 6}) do
                    add_note(track, 0, step, 36 + step, 0.8)
                end
            end
            set_clip_len(1, 0, 8)
        "#;

        let mut c = ctx();
        let started = std::time::Instant::now();
        project_script::run(rhai_source, &mut c).unwrap();
        let rhai_ms = started.elapsed().as_secs_f64() * 1000.0;

        let mut c2 = ctx();
        let started = std::time::Instant::now();
        run(lua_source, &mut c2).unwrap();
        let lua_ms = started.elapsed().as_secs_f64() * 1000.0;

        println!("eight notes in two clips: rhai {rhai_ms:.3} ms, lua {lua_ms:.3} ms");
        assert_eq!(c.clips[1].notes.len(), 4);
        assert_eq!(c2.clips[1].notes.len(), 4);
        assert_eq!(c2.clips[1].len_steps, 8);
        assert_eq!(c2.clips[1].notes, c.clips[1].notes, "same 8 notes either way");
        assert!(rhai_ms < 50.0, "rhai {rhai_ms} ms");
        assert!(lua_ms < 50.0, "lua {lua_ms} ms");
    }
}
