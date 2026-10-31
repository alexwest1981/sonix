//! WavPack as an **optional** decode backend — `dlopen`, never linked.
//!
//! **Why optional.** Before this, `.wv` only decoded through the external `ffmpeg` binary.
//! `libwavpack` is opened at run time through `libloading` (the same loader `opus.rs` uses), so
//! the shipped binary still starts without it and [`is_available`] is the plain question the
//! caller asks before committing (see `wav_reader::load_audio_pcm`). Build/install steps:
//! `docs/WAVPACK.md`.
//!
//! **Lossless means lossless.** WavPack hands back the original integer samples, so the only
//! conversion is the scale: `1 / 2^(bits-1)`, the same one `load_wav_pcm` and ffmpeg use, which
//! makes our output bit-identical to ffmpeg's own f32 decode. Float streams (`MODE_FLOAT`) are
//! passed through untouched.

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::OnceLock;

/// `MODE_FLOAT` from `wavpack.h`: samples are IEEE floats in the int32 buffer.
const MODE_FLOAT: i32 = 0x8;
/// Frames per `WavpackUnpackSamples` call.
const CHUNK: usize = 4096;

type Context = *mut c_void;

/// The C API, resolved once. Holding the `Library` keeps it loaded for the process lifetime.
struct Api {
    open: unsafe extern "C" fn(*const c_char, *mut c_char, i32, i32) -> Context,
    close: unsafe extern "C" fn(Context) -> Context,
    unpack: unsafe extern "C" fn(Context, *mut i32, u32) -> u32,
    get_num_samples: unsafe extern "C" fn(Context) -> i64,
    get_rate: unsafe extern "C" fn(Context) -> u32,
    get_channels: unsafe extern "C" fn(Context) -> i32,
    get_bits: unsafe extern "C" fn(Context) -> i32,
    get_bytes: unsafe extern "C" fn(Context) -> i32,
    get_mode: unsafe extern "C" fn(Context) -> i32,
    get_errors: unsafe extern "C" fn(Context) -> i32,
    _lib: libloading::Library,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

/// Is the library installed? Probed once and cached — every import asks.
pub fn is_available() -> bool {
    api().is_some()
}

fn api() -> Option<&'static Api> {
    API.get_or_init(|| unsafe { open() }).as_ref()
}

/// Sonames first (a system install has them on the loader path), then the two absolute paths a
/// `--prefix=$HOME/.local` build leaves behind, which `dlopen` cannot find by name alone.
fn candidates() -> Vec<String> {
    let mut names = vec!["libwavpack.so.1".to_string(), "libwavpack.so".to_string()];
    if let Ok(home) = std::env::var("HOME") {
        names.push(format!("{home}/.local/lib/libwavpack.so.1"));
        names.push(format!("{home}/.local/lib/libwavpack.so"));
    }
    names
}

unsafe fn open() -> Option<Api> {
    candidates().into_iter().find_map(|name| {
        let lib = unsafe { libloading::Library::new(&name) }.ok()?;
        unsafe { resolve(lib) }
    })
}

/// Looks up every symbol by its documented C name. One missing symbol means this is not a
/// libwavpack we can use, and the next candidate is tried.
unsafe fn resolve(lib: libloading::Library) -> Option<Api> {
    unsafe {
        Some(Api {
            open: *lib.get(b"WavpackOpenFileInput\0").ok()?,
            close: *lib.get(b"WavpackCloseFile\0").ok()?,
            unpack: *lib.get(b"WavpackUnpackSamples\0").ok()?,
            get_num_samples: *lib.get(b"WavpackGetNumSamples64\0").ok()?,
            get_rate: *lib.get(b"WavpackGetSampleRate\0").ok()?,
            get_channels: *lib.get(b"WavpackGetNumChannels\0").ok()?,
            get_bits: *lib.get(b"WavpackGetBitsPerSample\0").ok()?,
            get_bytes: *lib.get(b"WavpackGetBytesPerSample\0").ok()?,
            get_mode: *lib.get(b"WavpackGetMode\0").ok()?,
            get_errors: *lib.get(b"WavpackGetNumErrors\0").ok()?,
            _lib: lib,
        })
    }
}

/// Decodes a `.wv` file to a stereo pair at the file's own sample rate.
///
/// Mono is duplicated to both channels (the rest of the app carries stereo). Every failure names
/// what went wrong; a missing library says so and names WavPack.
pub fn decode_file(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let api = api().ok_or_else(|| {
        "WavPack is not installed (libwavpack.so.1); install WavPack to decode .wv in-process, \
         ffmpeg is used instead"
            .to_string()
    })?;
    let cpath = CString::new(path).map_err(|_| format!("{path}: path contains a NUL byte"))?;
    let mut error = [0 as c_char; 256];
    let ctx = unsafe { (api.open)(cpath.as_ptr(), error.as_mut_ptr(), 0, 0) };
    if ctx.is_null() {
        return Err(format!(
            "{path}: not a readable WavPack file: {}",
            error_text(&error)
        ));
    }
    let decoded = read_all(api, ctx, path);
    unsafe { (api.close)(ctx) };
    decoded
}

fn read_all(api: &Api, ctx: Context, path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let rate = unsafe { (api.get_rate)(ctx) };
    let channels = unsafe { (api.get_channels)(ctx) };
    let bits = unsafe { (api.get_bits)(ctx) };
    let bytes = unsafe { (api.get_bytes)(ctx) };
    let mode = unsafe { (api.get_mode)(ctx) };
    let total = unsafe { (api.get_num_samples)(ctx) };
    if rate == 0 || channels < 1 {
        return Err(format!(
            "{path}: WavPack header has no usable format ({rate} Hz, {channels} channels)"
        ));
    }
    let float = mode & MODE_FLOAT != 0;
    if float && bytes != 4 {
        return Err(format!(
            "{path}: {}-bit floating point WavPack is not supported",
            bytes * 8
        ));
    }
    let scale: f32 = if float {
        1.0
    } else {
        (1.0 / 2f64.powi(bits.clamp(1, 32) - 1)) as f32
    };

    let ch = channels as usize;
    let mut pcm = vec![0i32; CHUNK * ch];
    let mut left = Vec::new();
    let mut right = Vec::new();
    loop {
        let got = unsafe { (api.unpack)(ctx, pcm.as_mut_ptr(), CHUNK as u32) } as usize;
        if got == 0 {
            break;
        }
        for frame in pcm[..got * ch].chunks_exact(ch) {
            // `frame[0]` is the int32 sample; for float streams the same bits are the float.
            let sample = |v: i32| {
                if float {
                    f32::from_bits(v as u32)
                } else {
                    v as f32 * scale
                }
            };
            left.push(sample(frame[0]));
            right.push(sample(if ch > 1 { frame[1] } else { frame[0] }));
        }
    }

    let errors = unsafe { (api.get_errors)(ctx) };
    if left.is_empty() {
        return Err(format!(
            "{path}: WavPack file decoded to no samples ({errors} block errors)"
        ));
    }
    if total > 0 && (left.len() as i64) < total {
        return Err(format!(
            "{path}: WavPack file is truncated or corrupt — decoded {} of {total} frames \
             ({errors} block errors)",
            left.len()
        ));
    }
    Ok((left, right, rate))
}

fn error_text(error: &[c_char; 256]) -> String {
    let text = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
    if text.is_empty() {
        "libwavpack gave no reason".to_string()
    } else {
        text.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    /// A missing library must say so in the test output — never pass as if it had measured.
    fn require_library() -> bool {
        if !is_available() {
            println!("SKIP: libwavpack is not installed, there is nothing to decode with");
            return false;
        }
        true
    }

    fn temp_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("sonix-wavpack-{}-{name}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    fn ffmpeg(args: &[&str]) -> std::process::Output {
        std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y"])
            .args(args)
            .output()
            .expect("ffmpeg should start")
    }

    /// Encodes the 440 Hz test tone and returns the path, or `None` with the reason printed.
    fn encode_tone(path: &str, extra: &[&str]) -> bool {
        let mut args = vec![
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=2",
            "-c:a",
            "wavpack",
        ];
        args.extend_from_slice(extra);
        args.push(path);
        let out = ffmpeg(&args);
        if !out.status.success() {
            println!(
                "SKIP: ffmpeg could not encode WavPack: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return false;
        }
        true
    }

    /// ffmpeg's own decode of `path` as f32le, in the file's native channel layout.
    fn ffmpeg_f32(path: &str) -> Vec<f32> {
        let out = ffmpeg(&["-i", path, "-f", "f32le", "-"]);
        assert!(
            out.status.success(),
            "ffmpeg could not decode the file it wrote: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        out.stdout
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect()
    }

    /// Goertzel: the DFT at one frequency in O(n), which is all a peak search needs.
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

    fn dominant(x: &[f32], lo: f64, hi: f64, step: f64) -> f64 {
        let mut best = (0.0f64, 0.0f64);
        let mut f = lo;
        while f <= hi {
            let power = goertzel(x, f);
            if power > best.1 {
                best = (f, power);
            }
            f += step;
        }
        best.0
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt()
    }

    fn db(x: f64) -> f64 {
        20.0 * x.max(1e-12).log10()
    }

    /// (a) length/rate/tone, (b) bit-identity with ffmpeg's f32 decode, (d) the app's route.
    #[test]
    fn a_wavpack_file_decodes_to_the_tone_that_was_encoded() {
        if !require_library() {
            return;
        }
        let path = temp_path("tone.wv");
        if !encode_tone(&path, &[]) {
            return;
        }

        let (l, r, rate) = decode_file(&path).expect("the file we just built must decode");
        let seconds = l.len() as f64 / rate as f64;
        let peak = dominant(&l[l.len() / 4..l.len() * 3 / 4], 300.0, 600.0, 0.5);
        let ours = rms(&l);
        println!(
            "libwavpack: {rate} Hz, {} frames, {seconds:.4} s, dominant {peak:.1} Hz, \
             RMS {ours:.8} ({:.2} dBFS)",
            l.len(),
            db(ours)
        );
        assert_eq!(rate, SR, "the sample rate must follow the file");
        assert_eq!(l.len(), r.len(), "the channels must stay the same length");
        assert!(
            (l.len() as i64 - 88_200).abs() <= 2,
            "expected 88200 frames for 2 s at 44.1 kHz, got {}",
            l.len()
        );
        assert!(
            (peak - 440.0).abs() <= 2.0,
            "dominant frequency {peak:.1} Hz is not 440 Hz"
        );

        // (b) ffmpeg's decode of the same file: lossless must be sample-for-sample identical.
        // The lavfi source is mono, so ffmpeg emits one channel — compare it to our left.
        let reference = ffmpeg_f32(&path);
        let theirs = rms(&reference);
        let delta = (db(ours) - db(theirs)).abs();
        let max_diff = reference
            .iter()
            .zip(&l)
            .map(|(a, b)| (a - b).abs())
            .fold(0f64, |m, d| m.max(d as f64));
        let lsb = 1.0 / 32_768.0;
        println!(
            "ffmpeg: {} samples, RMS {theirs:.8} ({:.2} dBFS) — RMS difference {delta:.6} dB, \
             max sample difference {max_diff:.9} (1 LSB = {lsb:.9})",
            reference.len(),
            db(theirs)
        );
        assert_eq!(
            reference.len(),
            l.len(),
            "ffmpeg decodes {} samples, we decode {}",
            reference.len(),
            l.len()
        );
        assert!(
            delta < 0.01,
            "our RMS and ffmpeg's differ by {delta:.6} dB — the scale is wrong"
        );
        assert!(
            max_diff <= lsb,
            "max sample difference {max_diff:.9} exceeds 1 LSB ({lsb:.9}) — not lossless"
        );

        // The route the app uses: load_audio_pcm must pick libwavpack. The ffmpeg fallback
        // forces 44.1 kHz, so a 48 kHz file proves the in-process path won.
        let hi = temp_path("48k.wv");
        let out = ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:a",
            "wavpack",
            &hi,
        ]);
        assert!(out.status.success(), "ffmpeg could not write the 48 kHz file");
        let (routed, _, routed_rate) =
            crate::audio::wav_reader::load_audio_pcm(&hi).expect("load_audio_pcm");
        println!("load_audio_pcm: {routed_rate} Hz, {} frames", routed.len());
        assert_eq!(
            routed_rate, 48_000,
            "load_audio_pcm used the ffmpeg fallback (44.1 kHz)"
        );
        assert!((routed.len() as i64 - 48_000).abs() <= 2);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&hi);
    }

    /// (c) 24-bit WavPack: same frames, same level as ffmpeg's decode of the same file.
    #[test]
    fn a_24_bit_file_decodes_to_the_same_level_as_ffmpeg() {
        if !require_library() {
            return;
        }
        let path = temp_path("24bit.wv");
        if !encode_tone(&path, &["-bits_per_raw_sample", "24"]) {
            return;
        }

        let (l, _, rate) = decode_file(&path).expect("the 24-bit file must decode");
        let reference = ffmpeg_f32(&path);
        let ours = rms(&l);
        let theirs = rms(&reference);
        let delta = (db(ours) - db(theirs)).abs();
        let max_diff = reference
            .iter()
            .zip(&l)
            .map(|(a, b)| (a - b).abs())
            .fold(0f64, |m, d| m.max(d as f64));
        println!(
            "24-bit: {rate} Hz, {} frames, RMS ours {ours:.8} ({:.2} dBFS), ffmpeg {theirs:.8} \
             ({:.2} dBFS), difference {delta:.6} dB, max sample difference {max_diff:.9}",
            l.len(),
            db(ours),
            db(theirs)
        );
        assert_eq!(rate, SR);
        assert!(
            (l.len() as i64 - 88_200).abs() <= 2,
            "24-bit frame count {} is not 88200",
            l.len()
        );
        assert_eq!(reference.len(), l.len());
        assert!(
            delta < 0.01,
            "our RMS and ffmpeg's differ by {delta:.6} dB at 24 bits"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// (d) Truncation and a non-WavPack file are `Err` naming the failure, never a panic.
    #[test]
    fn a_broken_file_is_an_error_that_says_what_failed() {
        if !require_library() {
            return;
        }
        let good = temp_path("whole.wv");
        if !encode_tone(&good, &[]) {
            return;
        }
        let bytes = std::fs::read(&good).unwrap();
        let cut = temp_path("truncated.wv");
        std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        let err = decode_file(&cut).expect_err("a truncated file must not decode");
        println!("truncated: Err({err})");
        assert!(
            err.contains("truncated") || err.contains("corrupt"),
            "the reason should name the truncation: {err}"
        );

        let garbage = temp_path("garbage.wv");
        std::fs::write(&garbage, b"this is not a wavpack file, just text").unwrap();
        let err = decode_file(&garbage).expect_err("garbage must not decode");
        println!("garbage: Err({err})");
        assert!(
            err.contains("WavPack"),
            "the reason should name WavPack: {err}"
        );

        let missing = temp_path("missing.wv");
        let err = decode_file(&missing).expect_err("a missing file must not decode");
        println!("missing: Err({err})");
        assert!(err.contains("missing.wv"), "the reason should name the file: {err}");

        for p in [&good, &cut, &garbage] {
            let _ = std::fs::remove_file(p);
        }
    }
}
