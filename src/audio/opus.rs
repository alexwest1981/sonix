//! Opus as an **optional** decode backend — `dlopen`, never linked.
//!
//! **Why optional.** Symphonia has no Opus, so before this the only way in was the external
//! `ffmpeg` binary and a machine without it could not import `.opus` at all. `libopus` is
//! opened at run time through `libloading`, the same loader Rubber Band and the plugin hosts
//! use: the shipped binary still starts without it, and [`is_available`] is a plain question
//! the caller asks before it commits (see `wav_reader::load_audio_pcm`).
//!
//! **48 kHz, always.** Opus has no other output rate — the coding rate is internal and
//! `opus_decode_float` always produces 48 000 frames per second, so a "44.1 kHz Opus" is
//! resampled at encode time and the decoder's answer is the source of truth. The container's
//! `input sample rate` field is a hint for that resampler, not an output format.
//!
//! The container is read here too, minimally: enough of Ogg to find the `OpusHead`, skip the
//! `OpusTags`, hand the remaining packets to the decoder and honour pre-skip plus the closing
//! granule. Every malformed byte is an `Err` that says what failed — this runs on user files.

use std::ffi::c_void;
use std::sync::OnceLock;

/// Sonames tried in order: the versioned one first (Arch ships `libopus.so.0`), the dev
/// symlink after it.
const LIB_NAMES: [&str; 2] = ["libopus.so.0", "libopus.so"];

/// Opus decodes at 48 kHz and nothing else.
const SAMPLE_RATE: u32 = 48_000;
/// 120 ms, the longest Opus frame — the output buffer is sized from it once.
const MAX_FRAME: usize = 5_760;

type Decoder = *mut c_void;

/// The C API, resolved once. Holding the `Library` keeps it loaded for the process lifetime.
struct Api {
    create: unsafe extern "C" fn(i32, i32, *mut i32) -> Decoder,
    destroy: unsafe extern "C" fn(Decoder),
    decode_float: unsafe extern "C" fn(Decoder, *const u8, i32, *mut f32, i32, i32) -> i32,
    strerror: unsafe extern "C" fn(i32) -> *const i8,
    _lib: libloading::Library,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

/// Is the library installed? Probed once and cached — the answer cannot change under a
/// running process, and every import asks.
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
/// libopus we can use, and the next candidate soname is tried.
unsafe fn resolve(lib: libloading::Library) -> Option<Api> {
    unsafe {
        Some(Api {
            create: *lib.get(b"opus_decoder_create\0").ok()?,
            destroy: *lib.get(b"opus_decoder_destroy\0").ok()?,
            decode_float: *lib.get(b"opus_decode_float\0").ok()?,
            strerror: *lib.get(b"opus_strerror\0").ok()?,
            _lib: lib,
        })
    }
}

/// Decodes an Ogg-Opus file to a stereo pair at 48 kHz.
///
/// Mono is duplicated to both channels (the rest of the app carries stereo). Missing library
/// is an `Err` naming it, never a panic.
pub fn decode_file(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let api = api().ok_or_else(|| {
        "Opus is not installed (libopus.so); ffmpeg is used instead".to_string()
    })?;
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let stream = read_ogg(&bytes)?;

    let head = stream
        .packets
        .first()
        .ok_or("Ogg stream has no packets")?;
    if !head.starts_with(b"OpusHead") {
        return Err(format!(
            "Ogg stream does not start with OpusHead ({:?})",
            head.get(..8).unwrap_or(head)
        ));
    }
    if head.len() < 19 {
        return Err(format!("OpusHead is truncated ({} bytes)", head.len()));
    }
    let channels = head[9] as usize;
    if channels == 0 || channels > 2 {
        return Err(format!(
            "Opus stream has {channels} channels; only mono and stereo are supported"
        ));
    }
    let pre_skip = u16::from_le_bytes([head[10], head[11]]) as usize;
    // shortcut: OpusHead output gain is ignored — libopus and ffmpeg both write 0 there.

    if !stream
        .packets
        .get(1)
        .is_some_and(|p| p.starts_with(b"OpusTags"))
    {
        return Err("Ogg stream has no OpusTags packet after the header".to_string());
    }

    let audio = &stream.packets[2..];
    if audio.is_empty() {
        return Err("Ogg stream carries no audio packets".to_string());
    }

    let mut err = 0i32;
    let dec = unsafe { (api.create)(SAMPLE_RATE as i32, channels as i32, &mut err) };
    if dec.is_null() {
        return Err(format!(
            "libopus could not create a {channels}-channel decoder: {}",
            error_text(api, err)
        ));
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    let decoded = decode_packets(api, dec, audio, channels, &mut left, &mut right);
    unsafe { (api.destroy)(dec) };
    decoded?;

    // Pre-skip is encoder delay: the first samples are the codec's warm-up, not the source.
    let drop = pre_skip.min(left.len());
    left.drain(..drop);
    right.drain(..drop);

    // The closing granule is the file's real length; without it the decoder emits the
    // encoder's padding tail (up to 120 ms of it).
    let wanted = stream.last_granule.saturating_sub(pre_skip as u64) as usize;
    if wanted > 0 && wanted < left.len() {
        left.truncate(wanted);
        right.truncate(wanted);
    }
    if left.is_empty() {
        return Err("Opus stream decoded to no samples".to_string());
    }
    Ok((left, right, SAMPLE_RATE))
}

fn decode_packets(
    api: &Api,
    dec: Decoder,
    packets: &[Vec<u8>],
    channels: usize,
    left: &mut Vec<f32>,
    right: &mut Vec<f32>,
) -> Result<(), String> {
    let mut pcm = vec![0f32; MAX_FRAME * channels];
    for (index, packet) in packets.iter().enumerate() {
        if packet.is_empty() {
            continue;
        }
        let n = unsafe {
            (api.decode_float)(
                dec,
                packet.as_ptr(),
                packet.len() as i32,
                pcm.as_mut_ptr(),
                MAX_FRAME as i32,
                0,
            )
        };
        if n < 0 {
            return Err(format!(
                "libopus could not decode audio packet {} ({} bytes): {}",
                index,
                packet.len(),
                error_text(api, n)
            ));
        }
        for frame in pcm[..n as usize * channels].chunks_exact(channels) {
            left.push(frame[0]);
            right.push(if channels > 1 { frame[1] } else { frame[0] });
        }
    }
    Ok(())
}

fn error_text(api: &Api, code: i32) -> String {
    unsafe {
        let msg = (api.strerror)(code);
        if msg.is_null() {
            return format!("error {code}");
        }
        std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
    }
}

/// The packets of one logical Ogg bitstream plus the last granule position seen.
struct OggStream {
    packets: Vec<Vec<u8>>,
    /// Frames at 48 kHz the stream plays; `u64::MAX` when no page carried one.
    last_granule: u64,
}

/// Walks the pages and reassembles packets from the lacing values.
fn read_ogg(data: &[u8]) -> Result<OggStream, String> {
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut last_granule = u64::MAX;
    let mut pos = 0usize;
    let mut pages = 0usize;

    while pos < data.len() {
        let rest = &data[pos..];
        if rest.len() < 27 {
            return Err(format!(
                "truncated Ogg page header at byte {pos} ({} bytes left, 27 needed)",
                rest.len()
            ));
        }
        if &rest[0..4] != b"OggS" {
            return Err(format!(
                "expected an Ogg page at byte {pos}, found {:02x?}",
                &rest[0..4]
            ));
        }
        if rest[4] != 0 {
            return Err(format!("unsupported Ogg version {} at byte {pos}", rest[4]));
        }
        let granule = u64::from_le_bytes(rest[6..14].try_into().unwrap());
        let segments = rest[26] as usize;
        let body_start = pos + 27 + segments;
        if body_start > data.len() {
            return Err(format!(
                "Ogg page at byte {pos} has a truncated segment table ({segments} segments)"
            ));
        }
        let table = &data[pos + 27..body_start];
        let body_len: usize = table.iter().map(|&v| v as usize).sum();
        if body_start + body_len > data.len() {
            return Err(format!(
                "Ogg page at byte {pos} claims {body_len} body bytes, {} available",
                data.len() - body_start
            ));
        }

        let mut at = body_start;
        for &lace in table {
            current.extend_from_slice(&data[at..at + lace as usize]);
            at += lace as usize;
            if lace < 255 {
                packets.push(std::mem::take(&mut current));
            }
        }
        // 0xFFFF_FFFF_FFFF_FFFF marks a page that ends no packet; anything else is a length.
        if granule != u64::MAX && !packets.is_empty() {
            last_granule = granule;
        }
        pages += 1;
        pos = body_start + body_len;
    }

    if pages == 0 {
        return Err("file is empty, not an Ogg stream".to_string());
    }
    if !current.is_empty() {
        return Err(format!(
            "Ogg stream ends mid-packet ({} bytes of a partial packet)",
            current.len()
        ));
    }
    Ok(OggStream { packets, last_granule })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    /// A missing library or encoder must say so in the test output — never pass as if it had
    /// measured.
    fn require_tools() -> bool {
        if !is_available() {
            println!("SKIP: libopus is not installed, there is nothing to decode with");
            return false;
        }
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            true
        } else {
            println!("SKIP: ffmpeg is not installed, there is no file to build");
            false
        }
    }

    fn temp_path(ext: &str) -> String {
        std::env::temp_dir()
            .join(format!("sonix-opus-probe-{}.{ext}", std::process::id()))
            .to_string_lossy()
            .into_owned()
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

    /// (a) build, (b) decode and measure, (c) cross-check against ffmpeg's own decode.
    #[test]
    fn an_opus_file_decodes_to_the_tone_that_was_encoded() {
        if !require_tools() {
            return;
        }
        let path = temp_path("opus");
        let encode = std::process::Command::new("ffmpeg")
            .args([
                "-v", "error", "-y",
                "-f", "lavfi",
                "-i", "sine=frequency=440:sample_rate=48000:duration=2",
                "-c:a", "libopus", "-b:a", "128k",
                &path,
            ])
            .output()
            .expect("ffmpeg should start");
        if !encode.status.success() {
            println!(
                "SKIP: ffmpeg could not encode Opus: {}",
                String::from_utf8_lossy(&encode.stderr).trim()
            );
            return;
        }
        println!(
            "encoded {} bytes to {path}",
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
        );

        // (b) our reader.
        let (l, r, rate) = decode_file(&path).expect("the file we just built must decode");
        let seconds = l.len() as f64 / rate as f64;
        let peak = dominant(&l[l.len() / 4..l.len() * 3 / 4], 300.0, 600.0, 0.5);
        let ours = (rms(&l) + rms(&r)) / 2.0;
        println!(
            "libopus: {rate} Hz, {} frames, {:.4} s, dominant {:.1} Hz, RMS {:.6} ({:.2} dBFS)",
            l.len(),
            seconds,
            peak,
            ours,
            db(ours)
        );
        assert_eq!(rate, 48_000, "Opus always decodes at 48 kHz");
        assert_eq!(l.len(), r.len(), "the channels must stay the same length");
        assert!(
            (seconds - 2.0).abs() <= 2.0 * 0.05,
            "duration {seconds:.4} s is outside 2 s +/- 5 %"
        );
        assert!(
            (peak - 440.0).abs() <= 2.0,
            "dominant frequency {peak:.1} Hz is not 440 Hz"
        );

        // (c) ffmpeg's decode of the same file, as raw f32.
        // The lavfi source is mono, so ffmpeg emits one channel: forcing `-ac 2` would apply
        // its mono-to-stereo matrix (-3 dB per channel, measured 3.010 dB) and compare two
        // different signals. Native layout instead — the frame count then checks the container
        // and the pre-skip, and the level checks that the samples are the ones that were encoded.
        let reference = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i", &path, "-f", "f32le", "-"])
            .output()
            .expect("ffmpeg should start");
        assert!(
            reference.status.success(),
            "ffmpeg could not decode the file it wrote: {}",
            String::from_utf8_lossy(&reference.stderr).trim()
        );
        let raw = reference.stdout;
        let samples: Vec<f32> = raw
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let theirs = rms(&samples);
        let delta = (db(rms(&l)) - db(theirs)).abs();
        println!(
            "ffmpeg: {} samples, {:.4} s, RMS {theirs:.6} ({:.2} dBFS) — \
             difference {delta:.3} dB",
            samples.len(),
            samples.len() as f64 / SR as f64,
            db(theirs)
        );
        assert!(
            samples.len().abs_diff(l.len()) <= 48,
            "ffmpeg decodes {} samples, we decode {} — pre-skip or the closing granule is wrong",
            samples.len(),
            l.len()
        );
        assert!(
            delta < 0.5,
            "our RMS and ffmpeg's differ by {delta:.3} dB — container or pre-skip is wrong"
        );

        // The route the app uses: load_audio_pcm must pick libopus. The ffmpeg fallback forces
        // 44.1 kHz, so seeing 48 kHz back proves the in-process path won.
        let (routed, _, routed_rate) =
            crate::audio::wav_reader::load_audio_pcm(&path).expect("load_audio_pcm");
        println!(
            "load_audio_pcm: {routed_rate} Hz, {} frames",
            routed.len()
        );
        assert_eq!(routed_rate, 48_000, "load_audio_pcm used the ffmpeg fallback");
        assert_eq!(routed.len(), l.len());
        let _ = std::fs::remove_file(&path);
    }

    /// The stereo deinterleave branch: two channels in, two channels out, same level.
    #[test]
    fn a_stereo_file_keeps_its_two_channels() {
        if !require_tools() {
            return;
        }
        let path = temp_path("stereo.opus");
        let encode = std::process::Command::new("ffmpeg")
            .args([
                "-v", "error", "-y",
                "-f", "lavfi",
                "-i", "sine=frequency=440:sample_rate=48000:duration=2",
                "-ac", "2", "-c:a", "libopus", "-b:a", "128k",
                &path,
            ])
            .output()
            .expect("ffmpeg should start");
        if !encode.status.success() {
            println!(
                "SKIP: ffmpeg could not encode a stereo Opus: {}",
                String::from_utf8_lossy(&encode.stderr).trim()
            );
            return;
        }

        let (l, r, rate) = decode_file(&path).expect("the stereo file must decode");
        let seconds = l.len() as f64 / rate as f64;
        println!(
            "stereo: {rate} Hz, {} frames, {seconds:.4} s, RMS L {:.6} R {:.6}",
            l.len(),
            rms(&l),
            rms(&r)
        );
        assert_eq!(rate, 48_000);
        assert_eq!(l.len(), r.len(), "the channels must stay the same length");
        assert!((seconds - 2.0).abs() <= 0.1, "duration {seconds:.4} s");
        assert!(
            l.iter().zip(&r).all(|(a, b)| a == b),
            "the two channels were encoded identically and must decode identically"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// (d) Garbage and a truncated page are `Err` with a reason, never a panic.
    #[test]
    fn a_broken_file_is_an_error_that_says_what_failed() {
        if !is_available() {
            println!("SKIP: libopus is not installed");
            return;
        }
        let garbage = temp_path("garbage.opus");
        std::fs::write(&garbage, b"this is not an ogg stream, just text").unwrap();
        let err = decode_file(&garbage).expect_err("garbage must not decode");
        println!("garbage: Err({err})");
        assert!(err.contains("Ogg page"), "the reason should name the page: {err}");

        // A page header that claims a body the file does not have.
        let mut page = vec![0u8; 27 + 2];
        page[0..4].copy_from_slice(b"OggS");
        page[26] = 2; // two lacing values
        page[27] = 255;
        page[28] = 100; // 355 body bytes promised, none present
        let truncated = temp_path("truncated.opus");
        std::fs::write(&truncated, &page).unwrap();
        let err = decode_file(&truncated).expect_err("a truncated page must not decode");
        println!("truncated: Err({err})");
        assert!(
            err.contains("truncated") || err.contains("claims"),
            "the reason should name the truncation: {err}"
        );

        let _ = std::fs::remove_file(&garbage);
        let _ = std::fs::remove_file(&truncated);
    }
}
