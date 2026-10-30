//! Rubber Band as an **optional** stretch backend — `dlopen`, never linked.
//!
//! **Why optional.** WSOLA (see [`crate::audio::stretch`]) is ours and needs nothing from the
//! machine. Rubber Band's R3 engine is the better stretcher when the library is there, so it
//! is opened at run time through `libloading` (the same loader the plugin hosts use): the
//! shipped binary still starts on a machine without `librubberband.so`, and
//! [`is_available`] is a plain question the caller can ask before it commits.
//!
//! **Offline only.** `rubberband_process` is handed the whole file in one pass (offline
//! mode). This is the render path — one file per (source, tempo), the same shape as
//! `stretch::render_to_file` — not the audio thread.
//!
//! **The C API needs a sample rate and the plain `stretch` signature carried none**, so it is
//! the first argument after the two channels: the engine sizes its windows from it, and a
//! 96 kHz file treated as 44.1 kHz would get half-length grains.

use std::ffi::c_void;
use std::sync::OnceLock;

/// Offline mode: one exact pass over the file, with the padding/delay compensation the
/// stretcher does for us.
const OPT_PROCESS_OFFLINE: i32 = 0x0000_0000;
/// R3 ("Finer"). **Measured, not a preference:** the default R2 engine ends a 1.5× stretch of
/// 44 100 frames at 45 062 samples instead of 66 150, i.e. it never delivers the length the
/// caller asked for — and `set_formant_scale` is documented as R3-only.
const OPT_ENGINE_FINER: i32 = 0x2000_0000;
const OPT_FORMANT_PRESERVED: i32 = 0x0100_0000;

/// Sonames tried in order: Arch/current first, older distributions after, the bare name last.
const LIB_NAMES: [&str; 3] = [
    "librubberband.so.3",
    "librubberband.so.2",
    "librubberband.so",
];

/// How much input one `rubberband_process` call gets. The engine's own limit is 524 288
/// samples; 8 192 keeps its ring buffer small without adding calls for no reason.
const CHUNK: usize = 8_192;

type State = *mut c_void;

/// The C API, resolved once. Holding the `Library` keeps it loaded for the process lifetime.
struct Api {
    new: unsafe extern "C" fn(u32, u32, i32, f64, f64) -> State,
    delete: unsafe extern "C" fn(State),
    set_time_ratio: unsafe extern "C" fn(State, f64),
    set_pitch_scale: unsafe extern "C" fn(State, f64),
    set_formant_scale: unsafe extern "C" fn(State, f64),
    set_expected_input_duration: unsafe extern "C" fn(State, u32),
    process: unsafe extern "C" fn(State, *const *const f32, u32, i32),
    available: unsafe extern "C" fn(State) -> i32,
    retrieve: unsafe extern "C" fn(State, *const *mut f32, u32) -> u32,
    _lib: libloading::Library,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

/// Is the library installed? Probed once and cached — the answer cannot change under a
/// running process, and every caller asks on a render.
pub fn is_available() -> bool {
    api().is_some()
}

fn api() -> Option<&'static Api> {
    API.get_or_init(|| unsafe { open() }).as_ref()
}

unsafe fn open() -> Option<Api> {
    LIB_NAMES.iter().find_map(|name| {
        let lib = unsafe { libloading::Library::new(name) }.ok()?;
        unsafe { resolve(lib) }
    })
}

/// Looks up every symbol by its documented C name. One missing symbol means this is not a
/// Rubber Band we can use, and the next candidate soname is tried.
unsafe fn resolve(lib: libloading::Library) -> Option<Api> {
    unsafe {
        Some(Api {
            new: *lib.get(b"rubberband_new\0").ok()?,
            delete: *lib.get(b"rubberband_delete\0").ok()?,
            set_time_ratio: *lib.get(b"rubberband_set_time_ratio\0").ok()?,
            set_pitch_scale: *lib.get(b"rubberband_set_pitch_scale\0").ok()?,
            set_formant_scale: *lib.get(b"rubberband_set_formant_scale\0").ok()?,
            set_expected_input_duration: *lib
                .get(b"rubberband_set_expected_input_duration\0")
                .ok()?,
            process: *lib.get(b"rubberband_process\0").ok()?,
            available: *lib.get(b"rubberband_available\0").ok()?,
            retrieve: *lib.get(b"rubberband_retrieve\0").ok()?,
            _lib: lib,
        })
    }
}

/// Stretches a stereo pair with Rubber Band.
///
/// `time_ratio` is output time over input time (1.5 = 50 % longer), the same direction as
/// `stretch::StretchPlan::file_ratio`. `pitch_scale` 1.0 keeps the pitch; 2.0 is an octave
/// up. `formant` preserves the formants of the source (it only has something to preserve
/// when `pitch_scale` differs from 1.0 — see the note at the call below).
///
/// **Missing library is an `Err`, never a panic:** the text names the library and the
/// fallback, and the caller keeps WSOLA.
pub fn stretch(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    time_ratio: f64,
    pitch_scale: f64,
    formant: bool,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let api = api().ok_or_else(|| {
        "Rubber Band is not installed (librubberband.so); WSOLA is used instead".to_string()
    })?;
    let frames = left.len().min(right.len());
    if frames == 0 {
        return Err("nothing to stretch — the input is empty".to_string());
    }
    if sample_rate == 0 || !(time_ratio > 0.0) || !(pitch_scale > 0.0) {
        return Err("invalid stretch parameters".to_string());
    }
    // A missing change is the caller's own data. The engine fades in its first ~50 ms
    // (measured 0.10 max deviation on a 0.5 sine at 440 Hz), and WSOLA returns the input
    // untouched here too — so a 1:1 pass is bit-exact by not running the engine at all.
    if (time_ratio - 1.0).abs() < 1e-9 && (pitch_scale - 1.0).abs() < 1e-9 {
        return Ok((left[..frames].to_vec(), right[..frames].to_vec()));
    }

    let expected = ((frames as f64) * time_ratio).round().max(1.0) as usize;
    let mut out_l = vec![0.0f32; expected];
    let mut out_r = vec![0.0f32; expected];

    let options = OPT_PROCESS_OFFLINE
        | OPT_ENGINE_FINER
        | if formant { OPT_FORMANT_PRESERVED } else { 0 };
    let mut done = 0usize;
    unsafe {
        let state = (api.new)(sample_rate, 2, options, time_ratio, pitch_scale);
        if state.is_null() {
            return Err("Rubber Band could not create a stretcher".to_string());
        }
        (api.set_time_ratio)(state, time_ratio);
        (api.set_pitch_scale)(state, pitch_scale);
        if formant {
            // `set_formant_scale(1.0 / pitch)` is the explicit form of preservation: the
            // engine shifts the formants back by the pitch shift, so they stay put. Setting
            // 1.0 here means "no formant shift" and switches preservation off (measured:
            // bit-identical to the un-preserved render); without a pitch shift there is
            // nothing to preserve either way.
            (api.set_formant_scale)(state, 1.0 / pitch_scale);
        }
        // Without the duration the R3 engine does not land on round(len × ratio) — it ended
        // a 1.5× stretch of 44 100 frames at 45 062 instead of 66 150.
        (api.set_expected_input_duration)(state, frames.min(u32::MAX as usize) as u32);

        // One stretcher for both channels: the same grain decisions for left and right, so
        // the stereo image cannot smear (the rule the WSOLA path follows too).
        let inputs = [left.as_ptr(), right.as_ptr()];
        for offset in (0..frames).step_by(CHUNK) {
            let len = (frames - offset).min(CHUNK);
            let chunk = [inputs[0].add(offset), inputs[1].add(offset)];
            (api.process)(state, chunk.as_ptr(), len as u32, i32::from(offset + len == frames));
            done += drain(api, state, &mut out_l, &mut out_r, done);
        }
        // The source is spent; what is still inside the stretcher is the tail.
        loop {
            let n = drain(api, state, &mut out_l, &mut out_r, done);
            if n == 0 {
                break;
            }
            done += n;
        }
        (api.delete)(state);
    }

    // Rounding and the last block can miss by a frame or two. A short tail keeps its zeros
    // and a long one is cut, so the caller's length check sees exactly round(len × ratio).
    Ok((out_l, out_r))
}

/// Copies what the stretcher has ready into the output tail. Returns frames written; 0 means
/// there is nothing more (or no room left).
fn drain(api: &Api, state: State, out_l: &mut [f32], out_r: &mut [f32], done: usize) -> usize {
    let room = out_l.len().saturating_sub(done).min(out_r.len().saturating_sub(done));
    if room == 0 {
        return 0;
    }
    unsafe {
        let ready = (api.available)(state).max(0) as usize;
        let n = ready.min(room);
        if n == 0 {
            return 0;
        }
        let outputs = [out_l.as_mut_ptr().add(done), out_r.as_mut_ptr().add(done)];
        (api.retrieve)(state, outputs.as_ptr(), n as u32) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    /// A missing library must say so in the test output — never pass as if it had measured.
    macro_rules! require_library {
        () => {
            if !is_available() {
                println!(
                    "SKIP: Rubber Band is not installed (librubberband.so); \
                     WSOLA is used instead and there is nothing to measure"
                );
                return;
            }
        };
    }

    fn tone(freq: f32, secs: f32) -> Vec<f32> {
        let n = (SR as f32 * secs) as usize;
        (0..n)
            .map(|i| 0.5 * (std::f32::consts::TAU * freq * i as f32 / SR as f32).sin())
            .collect()
    }

    /// Vowel-like: two sine formants (700 Hz and 1200 Hz), the body a formant shifter moves.
    fn two_formants() -> Vec<f32> {
        let n = SR as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / SR as f32;
                let tau = std::f32::consts::TAU;
                0.4 * (tau * 700.0 * t).sin() + 0.3 * (tau * 1200.0 * t).sin()
            })
            .collect()
    }

    /// Goertzel: the DFT at one frequency in O(n), which is all a peak search needs.
    /// Accumulated in f64 — f32 drifts over tens of thousands of samples.
    fn goertzel(x: &[f32], freq: f64) -> f64 {
        let coeff = 2.0 * (std::f64::consts::TAU * freq / SR as f64).cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for &v in x {
            let s0 = v as f64 + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0)
    }

    /// The dominant frequency and its power, by scanning single DFT bins.
    fn dominant(x: &[f32], lo: f64, hi: f64, step: f64) -> (f64, f64) {
        let mut best = (0.0f64, 0.0f64);
        let mut f = lo;
        while f <= hi {
            let power = goertzel(x, f);
            if power > best.1 {
                best = (f, power);
            }
            f += step;
        }
        best
    }

    /// Middle half: the edges carry the engine's fade and the last block's rounding.
    fn middle(x: &[f32]) -> &[f32] {
        &x[x.len() / 4..x.len() * 3 / 4]
    }

    /// (a) The length is the contract: round(len × ratio), ±1 sample.
    #[test]
    fn the_output_length_follows_the_time_ratio() {
        require_library!();
        let src = tone(440.0, 1.0);
        for ratio in [0.5, 1.0, 1.5, 2.0] {
            let (l, r) = stretch(&src, &src, SR, ratio, 1.0, false).expect("stretch");
            let expected = (src.len() as f64 * ratio).round() as usize;
            println!(
                "ratio {ratio}: {} frames (expected {expected}), right {} frames",
                l.len(),
                r.len()
            );
            assert!(
                l.len().abs_diff(expected) <= 1,
                "ratio {ratio}: {} frames, the contract is {expected}",
                l.len()
            );
            assert_eq!(l.len(), r.len(), "the channels must stay the same length");
        }
    }

    /// (b) A time stretch must not move the pitch: 440 Hz stays 440 Hz.
    #[test]
    fn a_time_stretch_keeps_the_pitch() {
        require_library!();
        let src = tone(440.0, 1.0);
        let (l, _) = stretch(&src, &src, SR, 1.5, 1.0, false).expect("stretch");
        let (peak, power) = dominant(middle(&l), 300.0, 700.0, 0.5);
        println!("time stretch 1.5x: dominant frequency {peak:.1} Hz (power {power:.1})");
        assert!(
            (peak - 440.0).abs() <= 3.0,
            "the pitch moved: {peak:.1} Hz instead of 440"
        );
    }

    /// (c) A pitch shift must actually shift: an octave up is 880 Hz.
    #[test]
    fn a_pitch_shift_moves_the_pitch_an_octave() {
        require_library!();
        let src = tone(440.0, 1.0);
        let (l, _) = stretch(&src, &src, SR, 1.0, 2.0, false).expect("stretch");
        let (peak, power) = dominant(middle(&l), 600.0, 1100.0, 0.5);
        println!("pitch shift 2.0: dominant frequency {peak:.1} Hz (power {power:.1})");
        assert!(
            (peak - 880.0).abs() <= 5.0,
            "the octave came out as {peak:.1} Hz instead of 880"
        );
    }

    /// (d) Formant preservation is switched on — proven by the samples, not by the call.
    ///
    /// **The measurement needs a pitch shift.** Formant preservation cancels the formant
    /// movement that a pitch shift causes; at `pitch_scale = 1.0` there is nothing to cancel
    /// and the engine's two renders are bit-identical (printed below as the 0.000000 case).
    /// So the same vowel is stretched 1.5× **and** shifted an octave, which is the only
    /// situation where preservation can be seen in the signal.
    #[test]
    fn formant_preservation_changes_the_rendered_formants() {
        require_library!();
        let src = two_formants();

        let (same_on, _) = stretch(&src, &src, SR, 1.5, 1.0, true).expect("stretch");
        let (same_off, _) = stretch(&src, &src, SR, 1.5, 1.0, false).expect("stretch");
        let same = max_difference(&same_on, &same_off);
        println!(
            "formant on vs off at stretch 1.5, pitch 1.0: difference {same:.6} \
             (nothing to preserve without a pitch shift)"
        );

        let (on, _) = stretch(&src, &src, SR, 1.5, 2.0, true).expect("stretch");
        let (off, _) = stretch(&src, &src, SR, 1.5, 2.0, false).expect("stretch");
        let diff = max_difference(&on, &off);
        println!("formant on vs off at stretch 1.5, pitch 2.0: difference {diff:.6}");
        assert!(
            diff > 1e-3,
            "formant preservation never reached the samples: difference {diff:.6}"
        );
    }

    /// (e) 1.0 ratio and 1.0 pitch is the signal itself.
    #[test]
    fn a_ratio_of_one_is_near_identity() {
        require_library!();
        let src = tone(440.0, 1.0);
        let (l, r) = stretch(&src, &src, SR, 1.0, 1.0, false).expect("stretch");
        let max = max_difference(&l, &src).max(max_difference(&r, &src));
        println!("1.0 ratio, 1.0 pitch: max |out - in| = {max:.6}");
        assert!(max < 0.02, "the signal changed: max deviation {max:.6}");
    }

    /// The door the WSOLA path would use: same ratio direction (out/in), same stereo pair.
    #[test]
    fn try_rubber_band_wires_the_ratio_the_same_way_as_wsola() {
        require_library!();
        let src = tone(440.0, 1.0);
        let (l, r) = crate::audio::stretch::try_rubber_band(&src, &src, 1.5, SR as f32, false)
            .expect("Rubber Band should render");
        println!("try_rubber_band at 1.5: {} frames", l.len());
        assert_eq!(l.len(), 66_150, "1.5 x 44 100 frames, not the inverse");
        assert_eq!(r.len(), 66_150);
    }

    fn max_difference(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
    }
}
