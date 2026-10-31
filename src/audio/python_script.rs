//! **Project script, Python edition:** the same sandbox as `project_script`, run by an embedded
//! CPython instead of Rhai. Every host function carries the same name and meaning as its Rhai and
//! Lua twins, so a script ports between the three languages by changing its syntax only. The
//! surface is documented in full in `project_script`.
//!
//! Two things are Python's own: `print` writes to the log sink (space-joined, like Python), and an
//! exception comes back as `Err` naming the exception type and the line it broke on.
//!
//! The script runs on one dedicated thread that owns the interpreter. That is not decoration:
//! CPython only services pending calls in the interpreter's main thread, and a pending call is how
//! the watchdog below stops a runaway script from any Python version without a `settrace` hook.
//!
//! Status: stable — own run surface, own tests. Opt-in: `--features python`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc};
use std::time::Duration;

use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use super::project_script::Ctx;

/// How long a script may run. A project script takes microseconds; this only bites a runaway loop.
const TIME_BUDGET: Duration = Duration::from_secs(1);

/// The API object is `_host`; binding its methods as module globals is what makes the twins real.
const PREAMBLE: &str = r#"
_g = globals()
for _n in dir(_host):
    if not _n.startswith("_"):
        _g[_n] = getattr(_host, _n)
_script_err = None
try:
    exec(compile(_src, "<script>", "exec"), _g)
except SyntaxError as e:
    _script_err = f"SyntaxError at line {e.lineno}: {e.msg}"
except BaseException as e:
    _tb = e.__traceback__
    while _tb.tb_next:
        _tb = _tb.tb_next
    _script_err = f"{type(e).__name__} at line {_tb.tb_lineno}: {e}"
"#;

/// Runs a Python project script. `Ok` is what the script logged; `Err` names the exception type
/// and the line it broke on. Nothing is written back unless the whole script went through.
pub fn run(source: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    // `--!python` is dispatch metadata, not Python — blank it so error lines still match the file.
    let source = match source.split_once('\n') {
        Some((first, rest)) if first.trim() == "--!python" => format!("\n{rest}"),
        _ => source.to_string(),
    };
    let (reply, replies) = mpsc::channel();
    jobs()?
        .send((source, ctx.clone(), reply))
        .map_err(|_| "the python script thread is gone".to_string())?;
    match replies
        .recv()
        .map_err(|_| "the python script thread is gone".to_string())?
    {
        Ok((log, updated)) => {
            *ctx = updated;
            Ok(log)
        }
        Err(e) => Err(e),
    }
}

type Job = (
    String,
    Ctx,
    mpsc::Sender<Result<(Vec<String>, Ctx), String>>,
);

static JOBS: OnceLock<Result<mpsc::Sender<Job>, String>> = OnceLock::new();

/// The one thread that owns the interpreter, started on first use.
fn jobs() -> Result<&'static mpsc::Sender<Job>, String> {
    JOBS.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("python-script".to_string())
            .spawn(move || {
                // Initializing here is what makes this thread the interpreter's main thread.
                pyo3::prepare_freethreaded_python();
                for (source, ctx, reply) in rx {
                    let _ = reply.send(execute(&source, ctx));
                }
            })
            .map_err(|e| format!("the python script thread could not start: {e}"))?;
        Ok(tx)
    })
    .as_ref()
    .map_err(Clone::clone)
}

fn execute(source: &str, ctx: Ctx) -> Result<(Vec<String>, Ctx), String> {
    Python::with_gil(|py| {
        let host = Py::new(py, Host::new(ctx)).map_err(|e| e.to_string())?;
        let globals = PyDict::new(py);
        globals
            .set_item("_host", &host)
            .map_err(|e| e.to_string())?;
        globals
            .set_item("_src", source)
            .map_err(|e| e.to_string())?;
        let preamble = std::ffi::CString::new(PREAMBLE).map_err(|e| e.to_string())?;

        // The watchdog: one pending call, which CPython runs at the next bytecode boundary.
        let interrupt = Arc::new(AtomicI64::new(0));
        let (done, waited) = mpsc::channel::<()>();
        let fired = Arc::clone(&interrupt);
        let watchdog = std::thread::spawn(move || {
            if matches!(
                waited.recv_timeout(TIME_BUDGET),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                fired.store(1, Ordering::SeqCst);
                unsafe { ffi::Py_AddPendingCall(Some(trip), std::ptr::null_mut()) };
            }
        });

        let outcome = py.run(preamble.as_c_str(), Some(&globals), None);
        drop(done);
        let _ = watchdog.join();
        // A trip that landed after the script finished must not hit the next script.
        unsafe {
            ffi::Py_MakePendingCalls();
            ffi::PyErr_Clear();
        }
        outcome.map_err(|e| e.to_string())?;

        if let Some(err) = globals
            .get_item("_script_err")
            .ok()
            .flatten()
            .filter(|e| !e.is_none())
        {
            return Err(err
                .extract::<String>()
                .unwrap_or_else(|_| "script failed".to_string()));
        }
        if interrupt.load(Ordering::SeqCst) == 1 {
            return Err("script exceeded its 1 s budget".to_string());
        }
        let (log, updated) = host.borrow(py).snapshot();
        Ok((log, updated))
    })
}

/// Raises `KeyboardInterrupt` in the interpreter thread; `-1` makes CPython keep the exception.
extern "C" fn trip(_: *mut std::ffi::c_void) -> std::ffi::c_int {
    const MSG: &[u8] = b"project script exceeded its 1 s budget\0";
    unsafe { ffi::PyErr_SetString(ffi::PyExc_KeyboardInterrupt, MSG.as_ptr().cast()) };
    -1
}

/// The whole surface a script gets: the same 28 names the Rhai and Lua editions register. Note
/// that Python keeps its builtins — a script can `import os`, unlike the Lua edition.
/// (shortcut: builtins unrestricted, restrict them if scripts become untrusted input.)
#[pyclass]
struct Host {
    ctx: Mutex<Ctx>,
    log: Mutex<Vec<String>>,
}

impl Host {
    fn new(ctx: Ctx) -> Self {
        Self {
            ctx: Mutex::new(ctx),
            log: Mutex::new(Vec::new()),
        }
    }

    fn ctx(&self) -> MutexGuard<'_, Ctx> {
        self.ctx.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sink(&self) -> MutexGuard<'_, Vec<String>> {
        self.log.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn push(&self, text: String) {
        self.sink().push(text);
    }

    fn snapshot(&self) -> (Vec<String>, Ctx) {
        (self.sink().clone(), self.ctx().clone())
    }
}

fn refused(e: String) -> PyErr {
    PyRuntimeError::new_err(e)
}

#[pymethods]
impl Host {
    fn bpm(&self) -> f64 {
        self.ctx().bpm as f64
    }

    fn set_bpm(&self, value: f64) {
        // Same limits as the tempo control: below 20 or above 999 is not music.
        self.ctx().bpm = (value as f32).clamp(20.0, 999.0);
    }

    fn pattern_count(&self) -> i64 {
        self.ctx().pattern_count as i64
    }

    fn steps(&self) -> i64 {
        self.ctx().steps() as i64
    }

    fn track_count(&self) -> i64 {
        self.ctx().track_count()
    }

    fn track_name(&self, track: i64) -> PyResult<String> {
        self.ctx().track_name(track).map_err(refused)
    }

    fn set_track_name(&self, track: i64, name: &str) -> PyResult<()> {
        self.ctx().set_track_name(track, name).map_err(refused)
    }

    fn track_volume(&self, track: i64) -> PyResult<f64> {
        self.ctx().track_volume(track).map_err(refused)
    }

    fn set_track_volume(&self, track: i64, value: f64) -> PyResult<()> {
        self.ctx().set_track_volume(track, value).map_err(refused)
    }

    fn track_mute(&self, track: i64) -> PyResult<bool> {
        self.ctx().track_mute(track).map_err(refused)
    }

    fn set_track_mute(&self, track: i64, on: bool) -> PyResult<()> {
        self.ctx().set_track_mute(track, on).map_err(refused)
    }

    fn clip_count(&self, track: i64) -> PyResult<i64> {
        self.ctx().clip_count(track).map_err(refused)
    }

    fn clip_len(&self, track: i64, clip: i64) -> PyResult<i64> {
        self.ctx().clip_len(track, clip).map_err(refused)
    }

    fn set_clip_len(&self, track: i64, clip: i64, steps: i64) -> PyResult<()> {
        self.ctx().set_clip_len(track, clip, steps).map_err(refused)
    }

    fn note_count(&self, track: i64, clip: i64) -> PyResult<i64> {
        self.ctx().note_count(track, clip).map_err(refused)
    }

    fn note_at(&self, py: Python<'_>, track: i64, clip: i64, index: i64) -> PyResult<Py<PyAny>> {
        let found = self.ctx().note_at(track, clip, index).map_err(refused)?;
        let note = PyDict::new(py);
        note.set_item("step", found.step)?;
        note.set_item("note", found.note)?;
        note.set_item("velocity", found.velocity)?;
        Ok(note.into_any().unbind())
    }

    fn add_note(&self, track: i64, clip: i64, step: i64, note: i64, velocity: f64) -> PyResult<()> {
        self.ctx()
            .add_note(track, clip, step, note, velocity)
            .map_err(refused)
    }

    fn clear_notes(&self, track: i64, clip: i64) -> PyResult<()> {
        self.ctx().clear_notes(track, clip).map_err(refused)
    }

    // The older channel surface stays lenient with bad indices, as in the other two engines.
    fn channel_count(&self) -> i64 {
        self.ctx().channels.len() as i64
    }

    fn channel_name(&self, index: i64) -> String {
        self.ctx()
            .channels
            .get(index.max(0) as usize)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    fn step(&self, channel: i64, step: i64) -> bool {
        self.ctx()
            .channels
            .get(channel.max(0) as usize)
            .and_then(|c| c.steps.get(step.max(0) as usize).copied())
            .unwrap_or(false)
    }

    fn set_step(&self, channel: i64, step: i64, on: bool) {
        let mut ctx = self.ctx();
        if let Some(ch) = ctx.channels.get_mut(channel.max(0) as usize)
            && let Some(slot) = ch.steps.get_mut(step.max(0) as usize)
        {
            *slot = on;
        }
    }

    fn clear_channel(&self, channel: i64) {
        if let Some(ch) = self.ctx().channels.get_mut(channel.max(0) as usize) {
            ch.steps.iter_mut().for_each(|s| *s = false);
        }
    }

    // Sample header, not sample data: an unreadable file answers 0 instead of stopping the script.
    fn sample_seconds(&self, path: &str) -> f64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.seconds())
            .unwrap_or(0.0)
    }

    fn sample_rate(&self, path: &str) -> i64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.sample_rate as i64)
            .unwrap_or(0)
    }

    fn sample_frames(&self, path: &str) -> i64 {
        crate::audio::wav_reader::wav_info(path)
            .map(|i| i.frames as i64)
            .unwrap_or(0)
    }

    #[pyo3(signature = (*args))]
    fn print(&self, args: &Bound<'_, PyTuple>) -> PyResult<()> {
        let text = args
            .iter()
            .map(|v| v.str().map(|s| s.to_string()))
            .collect::<PyResult<Vec<_>>>()?
            .join(" ");
        self.push(text);
        Ok(())
    }

    /// `log` is the same sink as `print`, for one value.
    fn log(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.push(value.str()?.to_string());
        Ok(())
    }
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

    /// (a) One script written twice — rename track 0, set its volume, add four notes to clip 0,
    /// log the note count — must leave an identical `Ctx` behind in Python and in Lua.
    #[test]
    fn the_same_song_script_builds_the_same_project_in_python_and_lua() {
        let python_source = r#"
set_track_name(0, "Kick 808")
set_track_volume(0, 0.5)
for s in (0, 4, 8, 12):
    add_note(0, 0, s, 36 + s, 0.8)
log(note_count(0, 0))
"#;
        let lua_source = r#"
set_track_name(0, "Kick 808")
set_track_volume(0, 0.5)
for _, s in ipairs({0, 4, 8, 12}) do add_note(0, 0, s, 36 + s, 0.8) end
log(note_count(0, 0))
"#;

        let mut from_lua = ctx();
        let lua_log = project_script::run_named(lua_source, "song.lua", &mut from_lua).unwrap();
        let mut from_python = ctx();
        let python_log =
            project_script::run_named(python_source, "song.py", &mut from_python).unwrap();

        println!("lua:    {from_lua:?}");
        println!("python: {from_python:?}");
        println!("log lua {lua_log:?} python {python_log:?}");

        assert_eq!(
            python_log, lua_log,
            "the same script logs the same note count"
        );
        assert_eq!(python_log, vec!["4".to_string()]);
        assert_eq!(
            from_python, from_lua,
            "the same script leaves identical projects"
        );
        assert_eq!(from_python.channels[0].name, "Kick 808");
        assert_eq!(from_python.clips[0].notes.len(), 4);
    }

    /// (b) A syntax error and a runtime error both come back as `Err` naming the type and line,
    /// with nothing written back.
    #[test]
    fn python_errors_name_the_exception_and_the_line() {
        let cases = [
            (
                "set_bpm(140.0)\nif True\n    pass\n",
                "SyntaxError",
                "line 2",
            ),
            ("x = 0\ny = 1 / x\n", "ZeroDivisionError", "line 2"),
            ("note_count(99, 0)\n", "RuntimeError", "line 1"),
        ];
        for (source, kind, line) in cases {
            let mut c = ctx();
            let before = c.clone();
            let err = run(source, &mut c).unwrap_err();
            println!("{source:?} -> {err}");
            assert!(err.contains(kind), "{source:?}: {err}");
            assert!(err.contains(line), "{source:?}: {err}");
            assert_eq!(c, before, "a failed script writes nothing back");
        }
    }

    /// (c) An endless loop is stopped by the time budget, not by the user.
    #[test]
    fn a_runaway_script_is_stopped() {
        let mut c = ctx();
        let started = std::time::Instant::now();
        let err = run("while True:\n    pass\n", &mut c).unwrap_err();
        let elapsed = started.elapsed();
        println!("runaway stopped after {elapsed:?}: {err}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }

    /// (d) Every function the other two engines expose is callable from Python, once each.
    #[test]
    fn the_whole_surface_is_callable() {
        let mut c = ctx();
        let log = run(
            r#"
print(bpm())
set_bpm(140)
print(pattern_count())
print(steps())
print(track_count())
print(track_name(0))
set_track_name(0, "Kick 808")
print(track_volume(0))
set_track_volume(0, 0.5)
print(track_mute(0))
set_track_mute(1, True)
print(clip_count(0))
print(clip_len(0, 0))
set_clip_len(0, 0, 16)
print(note_count(0, 0))
add_note(0, 0, 0, 36, 0.9)
print(note_at(0, 0, 0)["step"])
clear_notes(0, 0)
print(channel_count())
print(channel_name(0))
print(step(0, 0))
set_step(0, 0, True)
clear_channel(1)
print(sample_seconds("/does/not/exist.wav"))
print(sample_rate("/does/not/exist.wav"))
print(sample_frames("/does/not/exist.wav"))
log("log")
print(len([n for n in dir(_host) if not n.startswith("_")]))
"#,
            &mut c,
        )
        .unwrap();

        for line in &log {
            println!("{line}");
        }
        assert_eq!(
            log.len(),
            19,
            "17 printed results, one log, one surface count: {log:?}"
        );
        assert_eq!(log[0], "124.0", "bpm");
        assert_eq!(log[4], "Kick", "the track name before the rename");
        assert_eq!(log[12], "Kick 808", "the renamed track");
        assert_eq!(log[17], "log");
        assert_eq!(
            log[18], "28",
            "the surface is the same 28 functions as Rhai and Lua"
        );
        assert_eq!(c.channels[0].name, "Kick 808");
    }
}
