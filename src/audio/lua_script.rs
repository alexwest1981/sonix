//! **Project script, Lua edition:** the same sandbox as `project_script`, run by the embedded
//! Lua 5.4 instead of Rhai. The twelve host functions carry the same names and meanings, so a
//! script ports between the two languages by changing its syntax only.
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

/// The twelve host functions and the `print` sink — the whole surface a script gets.
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
/// `io`, `os`, `package` and `debug` stay out: the twelve functions above are the whole surface.
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
    use crate::audio::project_script::{self, ScriptChannel};

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

    /// (b) A broken script answers with the line it broke on.
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
}
