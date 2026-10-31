//! Video export: frames plus a mixdown become one playable MP4.
//!
//! One `ffmpeg` child is fed raw RGB on stdin and muxes the audio in the same pass — the
//! same bargain as playback: the CLI is already on the machine, and one process per clip
//! beats linking a codec into the build. Every wait is bounded and every child is reaped,
//! because an export runs next to the UI and a stuck child would freeze the app.
// Called by the export dialog, which is a separate change; until then only the tests use it.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::video_engine::{run_bounded, Frame, VideoSource};

/// x264 has to finish the file once stdin closes; a slow machine still needs room for it.
const FINISH_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a failing encoder gets to say why before it is killed.
const EXIT_TIMEOUT: Duration = Duration::from_secs(2);
/// stderr of a failing ffmpeg is a diagnostic, not a log.
const MAX_STDERR: usize = 4096;

/// Writes one MP4 from raw frames and an existing audio file.
///
/// The frame count follows the audio: `-shortest` would cut a longer picture anyway, and a
/// shorter one is held on its last frame by `export_clip`.
pub struct VideoEncoder {
    child: Child,
    /// `None` once the pipe is closed or broken, so a later push cannot reuse a dead handle.
    stdin: Option<ChildStdin>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    width: u32,
    height: u32,
    out: PathBuf,
    frames: u64,
    /// Set when the file is complete, so `Drop` does not delete it.
    done: bool,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32, fps: f64, audio: &str, out: &str) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err(format!("cannot encode {width}x{height} frames"));
        }
        if !(fps.is_finite() && fps > 0.0) {
            return Err(format!("frame rate {fps} is not usable"));
        }
        let out = PathBuf::from(out);
        if let Some(dir) = out.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|err| {
                format!("could not create the output directory {}: {err}", dir.display())
            })?;
        }
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24"])
            .arg("-s")
            .arg(format!("{width}x{height}"))
            .arg("-r")
            .arg(fps.to_string())
            .args(["-i", "-", "-i", audio])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
            .arg(&out)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("could not start ffmpeg: {err}"))?;
        let stdin = child.stdin.take();
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let reader = thread::spawn(move || read_capped(&mut stderr));
        Ok(Self { child, stdin, stderr: Some(reader), width, height, out, frames: 0, done: false })
    }

    /// Hands one frame, top row first, three bytes per pixel, to the encoder.
    ///
    /// shortcut: a write blocks until ffmpeg drains the pipe, so a stalled encoder delays the
    /// export instead of failing fast; the waits in `finish` and the abort path are bounded.
    pub fn push_frame(&mut self, rgb: &[u8]) -> Result<(), String> {
        let expected = self.width as usize * self.height as usize * 3;
        if rgb.len() != expected {
            return Err(format!(
                "frame is {} bytes, expected {expected} for {}x{} RGB24",
                rgb.len(),
                self.width,
                self.height
            ));
        }
        let written = match self.stdin.as_mut() {
            Some(pipe) => pipe.write_all(rgb).map_err(|err| err.to_string()),
            None => return Err("the encoder is already closed".to_string()),
        };
        if let Err(err) = written {
            return Err(self.abort(&format!("the frame could not be sent: {err}")));
        }
        self.frames += 1;
        Ok(())
    }

    /// Closes the pipe and waits for the file. Fails — and leaves nothing behind — when the
    /// encoder never finished or wrote no frame at all.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        self.stdin = None; // EOF: ffmpeg sees the end of the picture and finalizes the file.
        if self.frames == 0 {
            return Err(self.abort("no frames were written, so there is nothing to encode"));
        }
        let status = self.wait_or_kill(FINISH_TIMEOUT);
        let stderr = self.take_stderr();
        match status {
            Some(status) if status.success() => {
                self.done = true;
                Ok(self.out.clone())
            }
            Some(_) => {
                let _ = std::fs::remove_file(&self.out);
                Err(format!(
                    "ffmpeg failed to write {}: {}",
                    self.out.display(),
                    reason(stderr, "ffmpeg exited with a failure and said nothing")
                ))
            }
            None => {
                let _ = std::fs::remove_file(&self.out);
                Err(format!(
                    "ffmpeg did not finish within {} s and was killed",
                    FINISH_TIMEOUT.as_secs()
                ))
            }
        }
    }

    /// Kills the encoder and returns the most useful message available.
    fn abort(&mut self, fallback: &str) -> String {
        self.stdin = None;
        let status = self.wait_or_kill(EXIT_TIMEOUT);
        let stderr = self.take_stderr();
        let _ = std::fs::remove_file(&self.out);
        let killed = status.is_none();
        format!(
            "ffmpeg failed to encode {}{}: {}",
            self.out.display(),
            if killed { " (it was killed after hanging)" } else { "" },
            reason(stderr, fallback)
        )
    }

    /// Waits up to `timeout` for the child; `None` means it had to be killed.
    fn wait_or_kill(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return None;
                }
            }
        }
    }

    fn take_stderr(&mut self) -> String {
        let bytes = self.stderr.take().and_then(|reader| reader.join().ok()).unwrap_or_default();
        String::from_utf8_lossy(&bytes).trim().to_string()
    }

    /// Process id of the encoder; the tests check it is really gone. The UI has no use for it.
    #[cfg(test)]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        // An abandoned export: no child outlives it, and no half-written file pretends to
        // be a clip.
        self.stdin = None;
        let _ = self.wait_or_kill(EXIT_TIMEOUT);
        self.take_stderr();
        let _ = std::fs::remove_file(&self.out);
    }
}

/// Decodes `source` at `fps` and muxes it with `audio` into `out`. Returns the frames written.
///
/// The length comes from the audio; a still image, or a clip shorter than the mix, plays its
/// last frame for the rest.
pub fn export_clip(source: &str, audio: &str, out: &str, fps: f64) -> Result<u64, String> {
    if !(fps.is_finite() && fps > 0.0) {
        return Err(format!("frame rate {fps} is not usable"));
    }
    if !VideoSource::is_available() {
        return Err(
            "ffmpeg is not installed, so no video can be encoded; install it with your package manager"
                .to_string(),
        );
    }
    let audio_secs = audio_duration(audio)?;
    let frames = (audio_secs * fps).ceil() as u64;
    if frames == 0 {
        return Err(format!("{audio} is {audio_secs:.3} s long, which is no frame at {fps} fps"));
    }
    let (mut video, still) = match VideoSource::open(source) {
        Ok(clip) => (Some(clip), None),
        // A still has no duration, which is exactly what `probe` rejects, so it is worth one
        // more try before the file is called unusable.
        Err(bad_video) => match still_frame(source) {
            Some(frame) => (None, Some(frame)),
            None => return Err(bad_video),
        },
    };
    let (width, height) = match (&video, &still) {
        (Some(clip), _) => (clip.info.width, clip.info.height),
        (_, Some(frame)) => (frame.width, frame.height),
        _ => return Err(format!("{source} has no picture")),
    };
    let mut encoder = VideoEncoder::new(width, height, fps, audio, out)?;
    for index in 0..frames {
        let rgb = match (&mut video, &still) {
            (Some(clip), _) => clip.frame_at(index as f64 / fps)?.rgb,
            (_, Some(frame)) => frame.rgb.clone(),
            _ => return Err(format!("{source} has no picture")),
        };
        encoder.push_frame(&rgb)?;
    }
    let path = encoder.finish()?;
    println!("exported {} frames to {}", frames, path.display());
    Ok(frames)
}

/// Length of the audio file in seconds. The mix decides how long the video is.
fn audio_duration(path: &str) -> Result<f64, String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path])
        .output()
        .map_err(|err| format!("could not start ffprobe: {err}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let secs: f64 = text.trim().parse().map_err(|_| {
        format!(
            "could not read a duration from {path}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
    })?;
    if !(secs > 0.0) {
        return Err(format!("{path} has no duration"));
    }
    Ok(secs)
}

/// A still image decoded once, or `None` when the file holds no picture either.
fn still_frame(path: &str) -> Option<Frame> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height"])
        .args(["-of", "csv=p=0", path])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut dims = text.trim().split(',');
    let width: u32 = dims.next()?.trim().parse().ok()?;
    let height: u32 = dims.next()?.trim().parse().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let rgb = decode_one(path, width, height).ok()?;
    Some(Frame { width, height, rgb, pts_secs: 0.0, produced_at: Instant::now() })
}

/// One frame of any file as raw RGB24, bounded like every other decode.
fn decode_one(path: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let size = width as usize * height as usize * 3;
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-i", path, "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]);
    let out = run_bounded(cmd, size)?;
    if !out.ok || out.stdout.len() < size {
        return Err(format!("could not decode a frame from {path}: {}", out.stderr));
    }
    Ok(out.stdout[..size].to_vec())
}

/// ffmpeg's own words if it had any, otherwise the caller's.
fn reason(stderr: String, fallback: &str) -> String {
    match stderr.is_empty() {
        true => fallback.to_string(),
        false => stderr,
    }
}

/// Keeps the first `MAX_STDERR` bytes and drains the rest, so a chatty encoder can never
/// block on a full pipe.
fn read_capped(stream: &mut impl Read) -> Vec<u8> {
    let mut kept = Vec::new();
    let _ = stream.take(MAX_STDERR as u64).read_to_end(&mut kept);
    let _ = std::io::copy(stream, &mut std::io::sink());
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A 3 s 320x240 25 fps clip and a 2 s 440 Hz mix, both made by ffmpeg.
    struct Fixture {
        dir: PathBuf,
        clip: String,
        audio: String,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn make(args: &[&str], out: &Path) {
        let output = Command::new("ffmpeg").args(args).arg(out).output().expect("ffmpeg is available");
        assert!(
            output.status.success(),
            "could not create {}: {}",
            out.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    fn fixture(tag: &str) -> Option<Fixture> {
        if !VideoSource::is_available() {
            println!("skipping {tag}: ffmpeg is not installed, so nothing can be encoded");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("sonix_vx_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let clip = dir.join("clip.mp4");
        make(
            &[
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=25:duration=3",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ],
            &clip,
        );
        let audio = dir.join("audio.wav");
        make(
            &[
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
            ],
            &audio,
        );
        Some(Fixture {
            dir,
            clip: clip.to_string_lossy().to_string(),
            audio: audio.to_string_lossy().to_string(),
        })
    }

    fn ffprobe(args: &[&str]) -> String {
        let out = Command::new("ffprobe").args(args).output().expect("ffprobe comes with ffmpeg");
        assert!(
            out.status.success(),
            "ffprobe {args:?}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn proc_exists(pid: u32) -> bool {
        Path::new(&format!("/proc/{pid}")).exists()
    }

    /// (a) 2 s of picture at 25 fps decodes back as 50 frames of the same size.
    #[test]
    fn a_two_second_export_has_fifty_frames_at_25_fps() {
        let Some(fx) = fixture("frames") else { return };
        let out = fx.dir.join("frames.mp4");
        let written = export_clip(&fx.clip, &fx.audio, &out.to_string_lossy(), 25.0)
            .expect("export should succeed");
        let csv = ffprobe(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_frames,r_frame_rate,width,height",
            "-of",
            "csv",
            &out.to_string_lossy(),
        ]);
        println!("wrote {written} frames; ffprobe: {csv}");
        // `-of csv` prefixes the section name and prints values in its own field order:
        // width, height, r_frame_rate, nb_frames for a stream with no header row.
        let mut fields: Vec<&str> = csv.split(',').map(str::trim).collect();
        if matches!(fields.first(), Some(&"stream" | &"video" | &"audio")) {
            fields.remove(0);
        }
        assert_eq!(fields.len(), 4, "unexpected ffprobe output {csv:?}");
        let (width, height) = (
            fields[0].parse::<u32>().expect("width"),
            fields[1].parse::<u32>().expect("height"),
        );
        let (num, den) = fields[2].split_once('/').expect("a rate fraction");
        let rate = num.parse::<f64>().expect("num") / den.parse::<f64>().expect("den");
        let frames: i64 = fields[3].parse().expect("nb_frames");
        println!("decoded back: {frames} frames, {rate:.3} fps, {width}x{height}");
        assert!((frames - 50).abs() <= 2, "{frames} frames, expected 50 (+/- 2)");
        assert!((rate - 25.0).abs() < 0.01, "rate {rate}");
        assert_eq!((width, height), (320, 240));
        assert_eq!(written, 50);
    }

    /// (b) The muxed audio runs as long as the picture, not past it.
    #[test]
    fn the_audio_stream_lasts_as_long_as_the_video() {
        let Some(fx) = fixture("sync") else { return };
        let out = fx.dir.join("sync.mp4").to_string_lossy().to_string();
        export_clip(&fx.clip, &fx.audio, &out, 25.0).expect("export");

        let stream = |pick: &str| {
            ffprobe(&[
                "-v",
                "error",
                "-select_streams",
                pick,
                "-show_entries",
                "stream=duration",
                "-of",
                "csv=p=0",
                &out,
            ])
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("no duration for {pick}"))
        };
        let container = ffprobe(&["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", &out])
            .parse::<f64>()
            .expect("container duration");
        let (video, audio) = (stream("v:0"), stream("a:0"));
        println!("durations: video {video:.3} s, audio {audio:.3} s, container {container:.3} s");
        assert!((video - audio).abs() <= 0.1, "audio {audio:.3} s is not the video {video:.3} s");
        assert!((container - video).abs() <= 0.1, "container {container:.3} s cuts the video");
    }

    /// (c) Colour survives the trip through libx264 and back.
    #[test]
    fn five_red_frames_decode_back_as_red() {
        let Some(fx) = fixture("red") else { return };
        let out = fx.dir.join("red.mp4");
        let mut encoder =
            VideoEncoder::new(64, 48, 25.0, &fx.audio, &out.to_string_lossy()).expect("encoder");
        let red = vec![255u8, 0, 0].repeat(64 * 48);
        for _ in 0..5 {
            encoder.push_frame(&red).expect("a frame must fit");
        }
        let path = encoder.finish().expect("finish");

        let mut source = VideoSource::open(&path.to_string_lossy()).expect("open the export");
        let frame = source.frame_at(0.0).expect("first frame");
        let pixels = (64 * 48) as f64;
        let mean = |offset: usize| {
            frame.rgb.iter().skip(offset).step_by(3).map(|v| *v as f64).sum::<f64>() / pixels
        };
        let (r, g, b) = (mean(0), mean(1), mean(2));
        println!("first decoded frame: R {r:.1}, G {g:.1}, B {b:.1}");
        assert!(r > 200.0, "red is gone: R {r:.1}");
        assert!(g < 60.0 && b < 60.0, "the colour bled: G {g:.1}, B {b:.1}");
    }

    /// A still has no duration, so it must play for as long as the mix instead of failing.
    #[test]
    fn a_still_image_is_held_for_the_whole_mix() {
        let Some(fx) = fixture("still") else { return };
        let still = fx.dir.join("still.png");
        make(&["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=red:size=96x64:d=1", "-frames:v", "1"], &still);
        let out = fx.dir.join("still.mp4").to_string_lossy().to_string();
        let written = export_clip(&still.to_string_lossy(), &fx.audio, &out, 25.0).expect("export");
        let frame = VideoSource::open(&out)
            .expect("open the export")
            .frame_at(0.0)
            .expect("first frame");
        println!(
            "{written} frames from a still image, {}x{}, first pixel {:?}",
            frame.width,
            frame.height,
            &frame.rgb[..3]
        );
        // 96x64 gets padded to the nearest even size that x264 accepts.
        assert!(written >= 48, "2 s at 25 fps is 50 frames, got {written}");
        assert_eq!(frame.rgb.len() % 3, 0);
        assert!(frame.rgb[0] > 200 && frame.rgb[1] < 60, "the still must be red");
    }

    /// (d) Failures are errors with a reason, and no child or file outlives them.
    #[test]
    fn a_failed_export_returns_an_error_and_leaves_nothing_behind() {
        let Some(fx) = fixture("bad") else { return };

        // An output directory that cannot be created.
        let bad = "/proc/sonix_export_no_such_dir/out.mp4";
        let started = Instant::now();
        let err = export_clip(&fx.clip, &fx.audio, bad, 25.0)
            .expect_err("an unusable output path must fail");
        println!("invalid output after {:.1} ms: {err}", started.elapsed().as_secs_f64() * 1e3);
        assert!(err.contains("sonix_export_no_such_dir"), "the error must name the directory: {err}");

        // Zero frames: nothing to write, and the spawned child must be gone afterwards.
        let out = fx.dir.join("empty.mp4");
        let encoder =
            VideoEncoder::new(64, 48, 25.0, &fx.audio, &out.to_string_lossy()).expect("encoder");
        let pid = encoder.pid();
        let err = encoder.finish().expect_err("an export without frames must fail");
        println!("zero frames: {err} (pid {pid})");
        assert!(err.contains("no frames"), "unhelpful error: {err}");
        assert!(!proc_exists(pid), "encoder {pid} survived finish()");
        assert!(!out.exists(), "a failed export left a file behind");

        // A missing audio makes ffmpeg quit under the frames: the push must report it.
        let out = fx.dir.join("no_audio.mp4");
        let mut encoder = VideoEncoder::new(
            64,
            48,
            25.0,
            "/nonexistent/sonix-no-such.wav",
            &out.to_string_lossy(),
        )
        .expect("spawn");
        let pid = encoder.pid();
        let frame = vec![0u8; 64 * 48 * 3];
        let mut pushed = 0;
        let err = loop {
            assert!(pushed < 64, "ffmpeg kept accepting frames with no audio input");
            match encoder.push_frame(&frame) {
                Ok(()) => pushed += 1,
                Err(err) => break err,
            }
        };
        println!("missing audio after {pushed} frames: {err}");
        assert!(err.contains("ffmpeg"), "the error must say who failed: {err}");
        assert!(!proc_exists(pid), "encoder {pid} survived a failed push");
        assert!(!out.exists(), "a failed export left a file behind");
    }

    /// (e) A frame of the wrong length is refused before it can reach the encoder.
    #[test]
    fn a_wrong_sized_frame_is_refused_with_the_expected_size() {
        let Some(fx) = fixture("wrong") else { return };
        let out = fx.dir.join("wrong.mp4");
        let mut encoder =
            VideoEncoder::new(64, 48, 25.0, &fx.audio, &out.to_string_lossy()).expect("encoder");
        let err = encoder.push_frame(&vec![0u8; 64 * 48 * 2]).expect_err("a short frame");
        println!("{err}");
        assert!(err.contains("9216"), "the error must name the expected size: {err}");
        assert!(err.contains("64x48"), "and the size it is for: {err}");
        drop(encoder);
        assert!(!out.exists(), "an abandoned encoder must not leave a file");
    }
}
