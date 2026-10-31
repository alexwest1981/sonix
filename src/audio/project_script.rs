//! **Projektskript:** samma Rhai-sandlåda som pianorullens skript, men ytan är projektet i
//! stället för en tagning — tempo, kanaler och deras steg. Skriptet får en **kopia** av det det
//! rör, precis som tagning-skriptet: går det inte igenom skrivs ingenting tillbaka.
//!
//! # The project API (identical in Rhai, Lua and Python)
//!
//! Indices are zero-based. `track` indexes a channel; a track's clips come from the project, and
//! a track's first clip is its step grid. Anything outside range is a script **error** — never a
//! panic and never a silent no-op.
//!
//! | function | meaning |
//! |---|---|
//! | `bpm()` / `set_bpm(v)` | project tempo, clamped to 20–999 |
//! | `pattern_count()` | number of patterns |
//! | `steps()` | steps in the longest channel |
//! | `track_count()` | number of tracks |
//! | `track_name(i)` / `set_track_name(i, s)` | track name |
//! | `track_volume(i)` / `set_track_volume(i, v)` | 0.0–1.0, clamped |
//! | `track_mute(i)` / `set_track_mute(i, on)` | mute flag |
//! | `clip_count(track)` | clips on a track |
//! | `clip_len(track, clip)` / `set_clip_len(track, clip, steps)` | clip length, 1–64 steps |
//! | `note_count(track, clip)` | notes in a clip |
//! | `note_at(track, clip, idx)` | `{step, note, velocity}` |
//! | `add_note(track, clip, step, note, velocity)` | inserts, kept sorted by `step` |
//! | `clear_notes(track, clip)` | removes the clip's notes |
//! | `log(s)` / `print(s)` | the log sink |
//! | `sample_seconds(p)` / `sample_rate(p)` / `sample_frames(p)` | WAV header only; 0 if unreadable |
//!
//! The older channel surface — `channel_count`, `channel_name`, `step`, `set_step`,
//! `clear_channel` — still works unchanged and stays lenient with bad indices.
//!
//! Status: stabil — egen körningsyta, egna prov.

/// One note in a clip: position in steps, MIDI note number and velocity 0.0–1.0.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptNote {
    pub step: usize,
    pub note: u8,
    pub velocity: f32,
}

/// One clip on one track. The app keeps a step grid per channel, so the caller builds a track's
/// clip 0 from that grid; `len_steps` is how much of it the clip covers.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptClip {
    pub track: usize,
    pub len_steps: usize,
    pub notes: Vec<ScriptNote>,
    /// Set once the script changed this clip's notes. Only then does the clip, rather than the
    /// channel's own steps, decide what the grid holds after the run.
    pub edited: bool,
}

impl ScriptClip {
    /// The app's step grid as the track's first clip. Steps are 0-based; a step that is off is
    /// simply not a note.
    pub fn from_grid(track: usize, steps: &[bool], notes: &[u8]) -> Self {
        let notes = steps
            .iter()
            .zip(notes.iter())
            .enumerate()
            .filter(|(_, (on, _))| **on)
            .map(|(step, (_, note))| ScriptNote { step, note: *note, velocity: 1.0 })
            .collect();
        Self { track, len_steps: steps.len(), notes, edited: false }
    }
}

/// Kanalen som skriptet ser den: namn, volym, mute och de 64 stegen.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptChannel {
    pub name: String,
    pub steps: Vec<bool>,
    pub volume: f32,
    pub mute: bool,
}

impl ScriptChannel {
    #[cfg(test)]
    pub fn new(name: &str, steps: Vec<bool>) -> Self {
        Self { name: name.to_string(), steps, volume: 1.0, mute: false }
    }
}

/// Projektets yta. Bara det här får ett skript röra — ingen motor, inga filer, inga nätanrop.
#[derive(Clone, Debug, PartialEq)]
pub struct Ctx {
    pub bpm: f32,
    pub pattern_count: usize,
    pub channels: Vec<ScriptChannel>,
    /// Clips, flat and grouped by `track`. The n-th clip of a track is `clip` index n.
    pub clips: Vec<ScriptClip>,
}

/// Clip length the surface accepts: the step grid is 64 steps wide.
const MAX_CLIP_STEPS: i64 = 64;

impl Ctx {
    /// Steglängden som skriptet arbetar mot. Alla kanaler har samma antal steg i appen, men
    /// kopian kan vara kortare i ett äldre projekt — den längsta vinner.
    pub fn steps(&self) -> usize {
        self.channels.iter().map(|c| c.steps.len()).max().unwrap_or(0)
    }

    // ---- indices ------------------------------------------------------------------------

    fn track_index(&self, track: i64) -> Result<usize, String> {
        usize::try_from(track)
            .ok()
            .filter(|i| *i < self.channels.len())
            .ok_or_else(|| format!("track {track} is outside 0..{}", self.channels.len()))
    }

    /// Position in `clips` of the `clip`-th clip on `track`.
    fn clip_index(&self, track: i64, clip: i64) -> Result<usize, String> {
        let track_index = self.track_index(track)?;
        let count = self.clips.iter().filter(|c| c.track == track_index).count();
        let nth = usize::try_from(clip)
            .ok()
            .filter(|i| *i < count)
            .ok_or_else(|| format!("clip {clip} on track {track_index} is outside 0..{count}"))?;
        self.clips
            .iter()
            .enumerate()
            .filter(|(_, c)| c.track == track_index)
            .nth(nth)
            .map(|(at, _)| at)
            .ok_or_else(|| format!("clip {clip} on track {track_index} is outside 0..{count}"))
    }

    // ---- tracks -------------------------------------------------------------------------

    pub fn track_count(&self) -> i64 {
        self.channels.len() as i64
    }

    pub fn track_name(&self, track: i64) -> Result<String, String> {
        let at = self.track_index(track)?;
        Ok(self.channels[at].name.clone())
    }

    pub fn set_track_name(&mut self, track: i64, name: &str) -> Result<(), String> {
        let at = self.track_index(track)?;
        self.channels[at].name = name.to_string();
        Ok(())
    }

    pub fn track_volume(&self, track: i64) -> Result<f64, String> {
        let at = self.track_index(track)?;
        Ok(self.channels[at].volume as f64)
    }

    pub fn set_track_volume(&mut self, track: i64, value: f64) -> Result<(), String> {
        let at = self.track_index(track)?;
        // Same range as the mixer's volume knob.
        self.channels[at].volume = (value as f32).clamp(0.0, 1.0);
        Ok(())
    }

    pub fn track_mute(&self, track: i64) -> Result<bool, String> {
        let at = self.track_index(track)?;
        Ok(self.channels[at].mute)
    }

    pub fn set_track_mute(&mut self, track: i64, on: bool) -> Result<(), String> {
        let at = self.track_index(track)?;
        self.channels[at].mute = on;
        Ok(())
    }

    // ---- clips and notes ----------------------------------------------------------------

    pub fn clip_count(&self, track: i64) -> Result<i64, String> {
        let track_index = self.track_index(track)?;
        Ok(self.clips.iter().filter(|c| c.track == track_index).count() as i64)
    }

    pub fn clip_len(&self, track: i64, clip: i64) -> Result<i64, String> {
        let at = self.clip_index(track, clip)?;
        Ok(self.clips[at].len_steps as i64)
    }

    pub fn set_clip_len(&mut self, track: i64, clip: i64, steps: i64) -> Result<(), String> {
        let at = self.clip_index(track, clip)?;
        let len_steps = steps.clamp(1, MAX_CLIP_STEPS) as usize;
        self.clips[at].len_steps = len_steps;
        self.clips[at].edited = true;
        // A shorter clip cannot hold notes past its end.
        self.clips[at].notes.retain(|n| n.step < len_steps);
        self.sync_grid(track, at);
        Ok(())
    }

    pub fn note_count(&self, track: i64, clip: i64) -> Result<i64, String> {
        let at = self.clip_index(track, clip)?;
        Ok(self.clips[at].notes.len() as i64)
    }

    pub fn note_at(&self, track: i64, clip: i64, index: i64) -> Result<ScriptNote, String> {
        let at = self.clip_index(track, clip)?;
        let notes = &self.clips[at].notes;
        usize::try_from(index)
            .ok()
            .and_then(|i| notes.get(i))
            .cloned()
            .ok_or_else(|| format!("note {index} is outside 0..{}", notes.len()))
    }

    pub fn add_note(
        &mut self,
        track: i64,
        clip: i64,
        step: i64,
        note: i64,
        velocity: f64,
    ) -> Result<(), String> {
        let at = self.clip_index(track, clip)?;
        let len = self.clips[at].len_steps;
        let step_index = usize::try_from(step)
            .ok()
            .filter(|s| *s < len)
            .ok_or_else(|| format!("step {step} is outside clip {clip} on track {track} (0..{len})"))?;
        let note_number = u8::try_from(note)
            .map_err(|_| format!("note {note} is not a MIDI note (0..127)"))?;
        let velocity = (velocity as f32).clamp(0.0, 1.0);
        let clip_ref = &mut self.clips[at];
        let position = clip_ref.notes.partition_point(|n| n.step <= step_index);
        clip_ref.notes.insert(position, ScriptNote { step: step_index, note: note_number, velocity });
        clip_ref.edited = true;
        self.sync_grid(track, at);
        Ok(())
    }

    pub fn clear_notes(&mut self, track: i64, clip: i64) -> Result<(), String> {
        let at = self.clip_index(track, clip)?;
        self.clips[at].notes.clear();
        self.clips[at].edited = true;
        self.sync_grid(track, at);
        Ok(())
    }

    /// What the track's step grid must hold after the run: `None` for a step that is off, else
    /// the MIDI note. `None` for the whole track means the script never touched its notes, so
    /// the channel's own steps stay as the script left them.
    pub fn edited_grid(&self, track: usize) -> Option<Vec<Option<u8>>> {
        let clip = self
            .clips
            .iter()
            .find(|c| c.track == track)
            .filter(|c| c.edited)?;
        Some(
            (0..self.channels.get(track)?.steps.len())
                .map(|step| {
                    clip.notes
                        .iter()
                        .find(|n| n.step == step)
                        .filter(|_| step < clip.len_steps)
                        .map(|n| n.note)
                })
                .collect(),
        )
    }

    /// Keeps a track's step grid and its first clip saying the same thing: a clip edit shows up
    /// in the grid too, so a script can mix both surfaces without one silently winning.
    fn sync_grid(&mut self, track: i64, at: usize) {
        let Ok(track) = usize::try_from(track) else { return };
        if self.clips.iter().position(|c| c.track == track) != Some(at) {
            return;
        }
        let clip = &self.clips[at];
        let Some(channel) = self.channels.get_mut(track) else { return };
        for (step, slot) in channel.steps.iter_mut().enumerate() {
            *slot = step < clip.len_steps && clip.notes.iter().any(|n| n.step == step);
        }
    }
}

/// Kör ett projektskript. `Ok` är det skriptet skrev med `print`, `Err` är felet som ska visas
/// (rad och kolumn kommer från Rhai).
pub fn run(source: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    run_named(source, "", ctx)
}

/// **One choice for all three languages.** First line `--!lua` or a `.lua` name goes to the
/// embedded Lua 5.4, `--!python` or a `.py` name to the embedded CPython, everything else keeps
/// Rhai. Callers do not care which one runs. A `.py` script in a build without the `python`
/// feature is an error, never a silent Rhai run of the wrong language.
pub fn run_named(source: &str, name: &str, ctx: &mut Ctx) -> Result<Vec<String>, String> {
    let first = source.lines().next().unwrap_or("").trim();
    // **Rubriken är ett val, inte kod.** `--!lua` är en giltig kommentar i Lua men inte i Python
    // (`--` betyder något annat där) och inte i Rhai alls, så raden tas bort innan motorn ser den.
    let body = if matches!(first, "--!lua" | "--!python" | "--!rhai") {
        source.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        source
    };
    if name.ends_with(".py") || first == "--!python" {
        #[cfg(feature = "python")]
        {
            return crate::audio::python_script::run(body, ctx);
        }
        #[cfg(not(feature = "python"))]
        {
            return Err(
                "Python scripting was not built into this binary (build with `--features python`)"
                    .to_string(),
            );
        }
    }
    let wants_lua = name.ends_with(".lua") || first == "--!lua";
    if wants_lua {
        crate::audio::lua_script::run(body, ctx)
    } else {
        run_rhai(body, ctx)
    }
}

/// A refused index reaches the script as a runtime error with the host's own message.
fn api<T>(result: Result<T, String>) -> Result<T, Box<rhai::EvalAltResult>> {
    result.map_err(|e| {
        Box::new(rhai::EvalAltResult::ErrorRuntime(e.into(), rhai::Position::NONE))
    })
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

    // The project surface. Same names, same meanings as the Lua edition.
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("track_count", move || shared.borrow().track_count());
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("track_name", move |track: i64| {
            api(shared.borrow().track_name(track))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_track_name", move |track: i64, name: &str| {
            api(shared.borrow_mut().set_track_name(track, name))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("track_volume", move |track: i64| {
            api(shared.borrow().track_volume(track))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_track_volume", move |track: i64, value: f64| {
            api(shared.borrow_mut().set_track_volume(track, value))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("track_mute", move |track: i64| {
            api(shared.borrow().track_mute(track))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_track_mute", move |track: i64, on: bool| {
            api(shared.borrow_mut().set_track_mute(track, on))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("clip_count", move |track: i64| api(shared.borrow().clip_count(track)));
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("clip_len", move |track: i64, clip: i64| {
            api(shared.borrow().clip_len(track, clip))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("set_clip_len", move |track: i64, clip: i64, steps: i64| {
            api(shared.borrow_mut().set_clip_len(track, clip, steps))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("note_count", move |track: i64, clip: i64| {
            api(shared.borrow().note_count(track, clip))
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("note_at", move |track: i64, clip: i64, index: i64| {
            api(shared.borrow().note_at(track, clip, index)).map(|n| {
                let mut map = rhai::Map::new();
                map.insert("step".into(), rhai::Dynamic::from_int(n.step as i64));
                map.insert("note".into(), rhai::Dynamic::from_int(n.note as i64));
                map.insert("velocity".into(), rhai::Dynamic::from_float(n.velocity as f64));
                map
            })
        });
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn(
            "add_note",
            move |track: i64, clip: i64, step: i64, note: i64, velocity: f64| {
                api(shared.borrow_mut().add_note(track, clip, step, note, velocity))
            },
        );
    }
    {
        let shared = std::rc::Rc::clone(&shared);
        engine.register_fn("clear_notes", move |track: i64, clip: i64| {
            api(shared.borrow_mut().clear_notes(track, clip))
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
    {
        // `log` is the same sink as `print`. One registration per type: Rhai's standard package
        // owns a `log(f64)` of its own (the natural logarithm), and a wildcard parameter would
        // lose to it, so each type is spelled out.
        macro_rules! log_fn {
            ($($ty:ty),* $(,)?) => {$(
                let logger = std::rc::Rc::clone(&sink);
                engine.register_fn("log", move |value: $ty| logger.borrow_mut().push(value.to_string()));
            )*};
        }
        log_fn!(f64, i64, bool, rhai::ImmutableString);
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
                ScriptChannel::new("Kick", vec![false; 16]),
                ScriptChannel::new("Snare", vec![false; 16]),
            ],
            clips: vec![
                ScriptClip::from_grid(0, &[false; 16], &[60; 16]),
                ScriptClip::from_grid(1, &[false; 16], &[60; 16]),
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

    /// A `.py` script in a build without the `python` feature is refused — never quietly run
    /// as Rhai, which would execute a different language than the file says.
    #[cfg(not(feature = "python"))]
    #[test]
    fn a_python_script_without_the_feature_is_refused() {
        let mut c = ctx();
        let before = c.clone();
        for (source, name) in [("set_bpm(140.0)\n", "drums.py"), ("--!python\nprint(1)\n", "")] {
            let err = run_named(source, name, &mut c).unwrap_err();
            println!("{name:?} -> {err}");
            assert!(err.contains("not built"), "{err}");
            assert_eq!(c, before, "a refused script writes nothing back");
        }
    }

    /// With the feature compiled in, the same header and suffix pick the Python engine.
    #[cfg(feature = "python")]
    #[test]
    fn a_python_header_picks_the_python_engine() {
        let mut by_header = ctx();
        let log = run_named("--!python\nset_bpm(155.0)\nprint(bpm())\n", "", &mut by_header).unwrap();
        assert_eq!(by_header.bpm, 155.0);
        assert_eq!(log, vec!["155.0".to_string()], "Rhai would have failed on that syntax");

        let mut by_name = ctx();
        run_named("set_bpm(155.0)", "song.py", &mut by_name).unwrap();
        assert_eq!(by_name.bpm, 155.0);
    }

    /// (d) The older twelve-function surface still works after the project API landed.
    #[test]
    fn the_old_surface_still_works() {
        let mut c = ctx();
        let log = run(
            "set_bpm(140.0); set_step(0, 0, true); clear_channel(1); \
             print(bpm()); print(pattern_count()); print(steps()); print(channel_count()); \
             print(channel_name(0)); print(step(0, 0)); print(sample_frames(\"/finns/inte.wav\"));",
            &mut c,
        )
        .unwrap();
        assert_eq!(log, vec!["140.0", "3", "16", "2", "Kick", "true", "0"]);
        assert!(c.channels[0].steps[0]);
    }

    #[test]
    fn clips_report_their_notes_and_length() {
        let mut c = ctx();
        run(
            "set_clip_len(0, 0, 8); add_note(0, 0, 4, 60, 0.5); add_note(0, 0, 0, 36, 1.0);",
            &mut c,
        )
        .unwrap();
        assert_eq!(c.clips[0].len_steps, 8);
        assert_eq!(c.clips[0].notes.len(), 2);
        assert_eq!(c.clips[0].notes[0].step, 0, "notes stay sorted by step");
        assert!(c.channels[0].steps[0] && c.channels[0].steps[4], "the grid follows the clip");

        run("clear_notes(0, 0);", &mut c).unwrap();
        assert!(c.clips[0].notes.is_empty());
        assert!(c.channels[0].steps.iter().all(|s| !*s));
    }

    /// What the app writes back: a clip the script edited decides the grid, an untouched clip
    /// leaves the channel's own steps alone.
    #[test]
    fn only_an_edited_clip_overrides_the_step_grid() {
        let mut steps_only = ctx();
        run("set_step(0, 3, true);", &mut steps_only).unwrap();
        assert_eq!(steps_only.edited_grid(0), None, "steps alone stay the grid's business");

        let mut notes = ctx();
        run("add_note(0, 0, 3, 48, 0.9);", &mut notes).unwrap();
        let grid = notes.edited_grid(0).expect("an added note is an edit");
        assert_eq!(grid[3], Some(48));
        assert_eq!(grid[2], None);
        assert_eq!(grid.len(), 16);

        let mut shortened = ctx();
        run("add_note(0, 0, 12, 48, 0.9); set_clip_len(0, 0, 8);", &mut shortened).unwrap();
        let grid = shortened.edited_grid(0).unwrap();
        assert_eq!(grid[12], None, "a note past the clip's end is not written back");

        let mut cleared = ctx();
        run("add_note(0, 0, 3, 48, 0.9); clear_notes(0, 0);", &mut cleared).unwrap();
        assert!(cleared.edited_grid(0).unwrap().iter().all(Option::is_none));
    }

    /// **Rubrikraden körs inte som kod.** `--!python` är ingen giltig Python-kommentar, så raden
    /// måste bort innan motorn ser den — annars fälls varje skript som använder rubriken.
    #[test]
    fn the_language_header_is_not_executed() {
        let mut ctx = ctx();
        ctx.bpm = 120.0;
        let source = "--!lua\nset_bpm(150)\nlog(\"ok\")";
        run_named(source, "inline", &mut ctx).expect("lua with header");
        assert_eq!(ctx.bpm, 150.0, "rubriken ska inte hindra skriptet");

        #[cfg(feature = "python")]
        {
            let mut py = ctx();
            py.bpm = 120.0;
            run_named("--!python\nset_bpm(160)\nlog('ok')", "inline", &mut py)
                .expect("python with header");
            assert_eq!(py.bpm, 160.0);
        }
    }
}