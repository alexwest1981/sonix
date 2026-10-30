//! Frame source for the video track: one frame on demand, so scrubbing and playback can
//! follow the transport.
//!
//! Decoding is delegated to the `ffmpeg` binary — the same one export uses — instead of
//! linking libavcodec: a frame is a small raw RGB buffer, and a missing decoder is a
//! runtime condition, not a build dependency. Every decode is bounded in time and size,
//! because a stuck child process must never stall the UI thread.

use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::video::{probe, VideoInfo};

/// One decoded frame, top row first, three bytes per pixel.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    /// Presentation time of the frame inside the file. The seek is accurate, so this is the
    /// frame whose time is at or before the requested position.
    // Not read yet: the UI indexes frames itself, and the field belongs to the frame contract.
    #[allow(dead_code)]
    pub pts_secs: f64,
    /// When the frame became available to the player. Playback latency is measured from
    /// here, and a frame that is old is a frame the transport has already passed.
    // Read by the playback tests only: the UI shows the newest frame it is given.
    #[allow(dead_code)]
    pub produced_at: Instant,
}

/// Small enough that a scrub over ten frames holds the whole neighbourhood, large enough
/// that going back and forth over a cut does not re-decode.
const CACHE_FRAMES: usize = 8;
const DECODE_TIMEOUT: Duration = Duration::from_secs(5);
/// stderr of a failing ffmpeg is a diagnostic line, not a log — keep a human amount.
const MAX_STDERR: usize = 4096;

pub struct VideoSource {
    path: String,
    pub info: VideoInfo,
    /// Most recently used first; keys are frame indices, not seconds, so sub-frame jitter
    /// in the transport still hits the cache.
    cache: Vec<(u64, Frame)>,
}

impl VideoSource {
    pub fn open(path: &str) -> Result<Self, String> {
        if !Self::is_available() {
            return Err("ffmpeg is not installed, so video frames cannot be decoded".to_string());
        }
        let info = probe(path)?;
        Ok(Self { path: path.to_string(), info, cache: Vec::new() })
    }

    /// Whether a decoder exists on this machine at all.
    pub fn is_available() -> bool {
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Decodes the frame shown at `time_secs` into the file (0 is the first frame).
    pub fn frame_at(&mut self, time_secs: f64) -> Result<Frame, String> {
        let last = self.info.frame_count().saturating_sub(1);
        let index = ((time_secs.max(0.0) * self.info.fps).floor() as u64).min(last);
        if let Some(pos) = self.cache.iter().position(|(i, _)| *i == index) {
            let hit = self.cache.remove(pos);
            let frame = hit.1.clone();
            self.cache.insert(0, hit);
            return Ok(frame);
        }
        let frame = self.decode(index)?;
        self.cache.insert(0, (index, frame.clone()));
        self.cache.truncate(CACHE_FRAMES);
        Ok(frame)
    }

    fn decode(&self, index: u64) -> Result<Frame, String> {
        let size = self.info.width as usize * self.info.height as usize * 3;
        let pts_secs = index as f64 / self.info.fps;
        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-v",
            "error",
            "-ss",
            &format!("{pts_secs:.3}"),
            "-i",
            &self.path,
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]);
        let out = run_bounded(cmd, size)?;
        if !out.ok {
            return Err(format!("ffmpeg could not decode the frame: {}", out.stderr));
        }
        if out.stdout.len() < size {
            return Err(format!("frame is {} bytes, expected {size}", out.stdout.len()));
        }
        Ok(Frame {
            width: self.info.width,
            height: self.info.height,
            rgb: out.stdout[..size].to_vec(),
            pts_secs,
            produced_at: Instant::now(),
        })
    }
}

/// Ring depth: a quarter of a second at 25 fps. Enough to ride out a scheduler hiccup,
/// small enough that the picture never runs visibly behind the transport.
const RING_FRAMES: usize = 6;

/// Playback of one clip: a single ffmpeg writing rawvideo for as long as the transport
/// rolls, so the cost is one process per clip instead of one per frame.
///
/// Frames are paced here, not by ffmpeg's `-re`: readrate reads half a second ahead, and a
/// preview that leads the audio is as wrong as one that lags it.
pub struct Playback {
    path: String,
    info: VideoInfo,
    /// Oldest first. The reader drops the oldest when the ring is full and `latest` takes
    /// the newest, so a slow consumer skips frames instead of falling behind.
    ring: Arc<Mutex<VecDeque<Frame>>>,
    child: Option<Child>,
    reader: Option<thread::JoinHandle<()>>,
}

impl Playback {
    pub fn new(path: &str, info: VideoInfo) -> Self {
        Self {
            path: path.to_string(),
            info,
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(RING_FRAMES))),
            child: None,
            reader: None,
        }
    }

    /// Starts a decoder that plays from `time_secs` onwards; a running one is stopped first.
    pub fn start(&mut self, time_secs: f64) -> Result<(), String> {
        self.stop();
        let time_secs = time_secs.max(0.0);
        let size = self.info.width as usize * self.info.height as usize * 3;
        let mut child = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-ss",
                &format!("{time_secs:.3}"),
                "-i",
                &self.path,
                "-vf",
                &format!(
                    "fps={},scale={}:{}",
                    self.info.fps, self.info.width, self.info.height
                ),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            // shortcut: a clip that stops decoding mid-way leaves the last picture with no
            // message; the one-shot path reports the reason on the next scrub.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("could not start ffmpeg: {err}"))?;
        let mut pipe = child.stdout.take().expect("stdout is piped");
        let ring = Arc::clone(&self.ring);
        let (width, height, fps) = (self.info.width, self.info.height, self.info.fps);
        let step = 1.0 / fps;
        self.reader = Some(thread::spawn(move || {
            let mut buf = vec![0u8; size];
            let mut index = 0u64;
            let started = Instant::now();
            while pipe.read_exact(&mut buf).is_ok() {
                // Absolute deadlines, so a late frame is followed by an on-time one instead
                // of the whole clip drifting.
                let due = started + Duration::from_secs_f64(step * index as f64);
                let now = Instant::now();
                if due > now {
                    thread::sleep(due - now);
                }
                let frame = Frame {
                    width,
                    height,
                    rgb: buf.clone(),
                    pts_secs: time_secs + index as f64 * step,
                    produced_at: Instant::now(),
                };
                let mut ring = ring.lock().unwrap_or_else(|err| err.into_inner());
                if ring.len() == RING_FRAMES {
                    ring.pop_front();
                }
                ring.push_back(frame);
                index += 1;
            }
        }));
        self.child = Some(child);
        Ok(())
    }

    /// The newest frame that has not been shown yet. Never waits for the decoder: a UI that
    /// asks faster than the clip runs gets `None` and keeps the picture it has.
    pub fn latest(&mut self) -> Option<Frame> {
        self.ring.lock().unwrap_or_else(|err| err.into_inner()).pop_back()
    }

    /// Restarts the decoder at `time_secs`; a rawvideo pipe cannot be moved any other way.
    pub fn seek(&mut self, time_secs: f64) -> Result<(), String> {
        self.start(time_secs)
    }

    /// Kills the decoder and waits for it, so no child outlives the playhead.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // The reader can still deliver what the pipe already held, so wait for it before
        // clearing: afterwards `latest` has nothing to return.
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.ring.lock().unwrap_or_else(|err| err.into_inner()).clear();
    }

    /// Process id of the decoder. Used by the tests that check it is really gone; the UI has
    /// no use for it.
    #[cfg(test)]
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Bounded {
    ok: bool,
    stdout: Vec<u8>,
    stderr: String,
}

/// Runs ffmpeg with its output capped at `cap` bytes and kills it after `DECODE_TIMEOUT`.
fn run_bounded(mut cmd: Command, cap: usize) -> Result<Bounded, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("could not start ffmpeg: {err}"))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    // Both pipes are drained on their own thread: the child blocks once a pipe fills, so
    // waiting for it without reading would deadlock on a long error message.
    let out_thread = thread::spawn(move || read_capped(stdout, cap));
    let err_thread = thread::spawn(move || read_capped(stderr, MAX_STDERR));

    let deadline = Instant::now() + DECODE_TIMEOUT;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(2)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_thread.join();
                let _ = err_thread.join();
                return Err(format!(
                    "ffmpeg did not finish within {} s and was killed",
                    DECODE_TIMEOUT.as_secs()
                ));
            }
        }
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(Bounded { ok, stdout, stderr: String::from_utf8_lossy(&stderr).trim().to_string() })
}

fn read_capped<R: Read>(stream: R, cap: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(cap.min(64 * 1024));
    let _ = stream.take(cap as u64).read_to_end(&mut buf);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A 3 s 320×240 25 fps clip with a moving timecode burned into the picture.
    struct Fixture {
        dir: PathBuf,
        path: String,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn fixture(tag: &str) -> Option<Fixture> {
        if !VideoSource::is_available() {
            println!("skipping {tag}: ffmpeg is not installed, nothing can be decoded");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("sonix_ve_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("clip.mp4");
        let out = Command::new("ffmpeg")
            .args([
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
            ])
            .arg(&path)
            .output()
            .expect("ffmpeg is available");
        assert!(
            out.status.success(),
            "{tag}: could not create the clip: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Some(Fixture { dir, path: path.to_string_lossy().to_string() })
    }

    fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
        assert_eq!(a.len(), b.len());
        let sum: u64 = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum();
        sum as f64 / a.len() as f64
    }

    /// The next frame a running playback hands out, or `None` if none arrives in time.
    fn next_frame(play: &mut Playback, wait: Duration) -> Option<Frame> {
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            if let Some(frame) = play.latest() {
                return Some(frame);
            }
            thread::sleep(Duration::from_millis(1));
        }
        None
    }

    /// Whether `/proc/<pid>` is still there. A killed and reaped child leaves no directory;
    /// a leaked one keeps it, zombie or not.
    fn proc_exists(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }

    /// Every ffmpeg or sonix process whose parent is this test binary.
    // Not asserted on: the whole test binary shares one parent, so parallel tests see each
    // other's decoders. It is printed as evidence of what was looked at.
    fn decoder_children() -> Vec<(u32, String)> {
        let me = std::process::id();
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else { return found };
        for entry in entries.flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
                continue;
            };
            let field = |key: &str| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix(key))
                    .map(|value| value.trim().to_string())
            };
            let name = field("Name:").unwrap_or_default();
            let parent = field("PPid:").and_then(|value| value.parse::<u32>().ok());
            if parent == Some(me) && (name.contains("ffmpeg") || name.contains("sonix")) {
                found.push((pid, name));
            }
        }
        found
    }

    #[test]
    fn a_frame_has_the_file_size_and_changes_over_time() {
        let Some(fx) = fixture("size") else { return };
        let mut src = VideoSource::open(&fx.path).expect("open");
        assert_eq!((src.info.width, src.info.height), (320, 240));
        assert!((src.info.fps - 25.0).abs() < 0.01, "fps {}", src.info.fps);
        assert!((src.info.duration_secs - 3.0).abs() < 0.2, "duration {}", src.info.duration_secs);

        let one = src.frame_at(1.0).expect("frame at 1 s");
        let two = src.frame_at(2.0).expect("frame at 2 s");
        assert_eq!((one.width, one.height), (320, 240));
        assert_eq!(one.rgb.len(), 320 * 240 * 3);
        assert_eq!(two.rgb.len(), 320 * 240 * 3);
        let diff = mean_abs_diff(&one.rgb, &two.rgb);
        println!("mean abs difference between 1.0 s and 2.0 s: {diff:.2} levels");
        assert!(diff > 5.0, "the picture must move: {diff:.2} levels");
    }

    #[test]
    fn the_same_timestamp_comes_from_the_cache() {
        let Some(fx) = fixture("cache") else { return };
        let mut src = VideoSource::open(&fx.path).expect("open");
        let t0 = Instant::now();
        let first = src.frame_at(1.0).expect("first decode");
        let cold = t0.elapsed();
        let t1 = Instant::now();
        let second = src.frame_at(1.0).expect("cached frame");
        let warm = t1.elapsed();
        println!(
            "decode {:.3} ms, cache hit {:.3} ms, ratio {:.1}x",
            cold.as_secs_f64() * 1e3,
            warm.as_secs_f64() * 1e3,
            cold.as_secs_f64() / warm.as_secs_f64().max(1e-9)
        );
        assert_eq!(first.rgb, second.rgb);
        assert!(
            warm.as_secs_f64() * 5.0 <= cold.as_secs_f64(),
            "cache hit {warm:?} should be at least 5x faster than a decode {cold:?}"
        );
    }

    #[test]
    fn seeking_backwards_returns_the_same_frame() {
        let Some(fx) = fixture("back") else { return };
        let mut src = VideoSource::open(&fx.path).expect("open");
        let late = src.frame_at(2.5).expect("2.5 s");
        let early = src.frame_at(0.5).expect("0.5 s");
        assert_eq!(early.rgb.len(), 320 * 240 * 3);
        let late_again = src.frame_at(2.5).expect("2.5 s again");
        assert_eq!(late_again.rgb.len(), 320 * 240 * 3);

        // The cache alone would make the match trivial, so decode 2.5 s once more in a
        // fresh source: a backwards seek must land on the same picture, not on a stale one.
        let mut other = VideoSource::open(&fx.path).expect("open");
        let fresh = other.frame_at(2.5).expect("fresh 2.5 s");

        let cached_diff = mean_abs_diff(&late.rgb, &late_again.rgb);
        let fresh_diff = mean_abs_diff(&late.rgb, &fresh.rgb);
        println!(
            "2.5 s vs cached 2.5 s: {cached_diff:.2} levels, vs fresh decode: {fresh_diff:.2} levels"
        );
        assert!(cached_diff <= 1.0, "cached frame drifted: {cached_diff:.2}");
        assert!(fresh_diff <= 1.0, "backwards seek changed the picture: {fresh_diff:.2}");
    }

    #[test]
    fn unusable_files_fail_fast_with_a_reason() {
        if !VideoSource::is_available() {
            println!("skipping unusable files: ffmpeg is not installed, nothing can be decoded");
            return;
        }
        let start = Instant::now();
        let missing = VideoSource::open("/nonexistent/sonix-no-such-clip.mp4")
            .err()
            .expect("a missing path must be an error");
        println!(
            "missing file after {:.1} ms: {missing}",
            start.elapsed().as_secs_f64() * 1e3
        );
        assert!(!missing.is_empty());

        let dir = std::env::temp_dir().join(format!("sonix_ve_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let text = dir.join("not_video.mp4");
        std::fs::write(&text, b"this is text, not a movie\n").expect("write");
        let start = Instant::now();
        let err = VideoSource::open(&text.to_string_lossy())
            .err()
            .expect("a text file must be an error");
        println!("text file after {:.1} ms: {err}", start.elapsed().as_secs_f64() * 1e3);
        assert!(err.contains("ffprobe") || err.contains("file"), "unhelpful error: {err}");
        let _ = std::fs::remove_dir_all(&dir);

        // A file that vanishes after it was opened: the decode, not the probe, must fail —
        // and the child must be killed or exit, never hang.
        let Some(fx) = fixture("gone") else { return };
        let mut src = VideoSource::open(&fx.path).expect("open");
        std::fs::remove_file(&fx.path).expect("remove clip");
        let start = Instant::now();
        let err = src.frame_at(1.0).unwrap_err();
        let elapsed = start.elapsed();
        println!("deleted clip after {:.1} ms: {err}", elapsed.as_secs_f64() * 1e3);
        assert!(elapsed < DECODE_TIMEOUT, "decode of a missing file hung: {elapsed:?}");
    }

    #[test]
    fn random_seeks_always_return_a_whole_frame() {
        let Some(fx) = fixture("random") else { return };
        let mut src = VideoSource::open(&fx.path).expect("open");
        let size = 320 * 240 * 3;
        let (mut min, mut max) = (f64::MAX, 0.0f64);
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..20 {
            // Deterministic xorshift: a failing seek has to be reproducible.
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let at = (seed % 3000) as f64 / 1000.0;
            let start = Instant::now();
            let frame = src.frame_at(at).unwrap_or_else(|err| panic!("seek to {at}: {err}"));
            let ms = start.elapsed().as_secs_f64() * 1e3;
            min = min.min(ms);
            max = max.max(ms);
            assert_eq!(frame.rgb.len(), size, "seek to {at} s");
            assert_eq!((frame.width as usize, frame.height as usize), (320, 240));
        }
        println!("20 random seeks: min {min:.3} ms, max {max:.3} ms");
    }

    /// (a) A live decoder feeds faster than the clip runs: two seconds of wall time must
    /// deliver at least a second of film at 25 fps.
    #[test]
    fn playback_delivers_frames_faster_than_the_clip_runs() {
        let Some(fx) = fixture("playrate") else { return };
        let info = probe(&fx.path).expect("probe");
        let mut play = Playback::new(&fx.path, info);
        play.start(0.0).expect("start");

        let window = Duration::from_secs(2);
        let started = Instant::now();
        let mut count = 0usize;
        while started.elapsed() < window {
            if play.latest().is_some() {
                count += 1;
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
        let elapsed = started.elapsed().as_secs_f64();
        let fps = count as f64 / elapsed;
        println!(
            "{count} frames in {elapsed:.2} s of wall time, {fps:.1} fps (the clip is 25.0 fps)"
        );
        play.stop();
        assert!(
            count >= 25,
            "{count} frames in {elapsed:.2} s cannot follow a 25 fps clip"
        );
        // One frame per second of slack: the pacer aims at exactly 25.0, and a window that
        // ends a millisecond late must not read as a failure.
        assert!(fps >= 24.0, "only {fps:.1} frames per second arrived");
    }

    /// (b) A frame is shown while it is still current: thirty consecutive frames must each
    /// be under 100 ms old when `latest` returns them.
    #[test]
    fn a_shown_frame_is_less_than_100_ms_old() {
        let Some(fx) = fixture("latency") else { return };
        let info = probe(&fx.path).expect("probe");
        let mut play = Playback::new(&fx.path, info);
        play.start(0.0).expect("start");

        let mut ages: Vec<f64> = Vec::with_capacity(30);
        let deadline = Instant::now() + Duration::from_secs(5);
        while ages.len() < 30 && Instant::now() < deadline {
            match play.latest() {
                Some(frame) => ages.push(frame.produced_at.elapsed().as_secs_f64() * 1e3),
                None => thread::sleep(Duration::from_millis(1)),
            }
        }
        play.stop();
        assert_eq!(ages.len(), 30, "only {} frames arrived", ages.len());
        ages.sort_by(f64::total_cmp);
        println!(
            "latency over 30 frames: min {:.2} ms, median {:.2} ms, max {:.2} ms",
            ages[0],
            ages[15],
            ages[29]
        );
        assert!(
            ages[29] < 100.0,
            "worst latency was {:.2} ms, the UI would show a late picture",
            ages[29]
        );
    }

    /// (c) Asking for the current frame never waits for the decoder, however often the UI
    /// asks: a thousand calls while a child is writing must stay far below one frame's cost.
    #[test]
    fn latest_never_waits_for_the_decoder() {
        let Some(fx) = fixture("nonblocking") else { return };
        let info = probe(&fx.path).expect("probe");
        let mut play = Playback::new(&fx.path, info);
        play.start(0.0).expect("start");
        // Let the child get going: the point is contention, not a cold pipe.
        assert!(next_frame(&mut play, Duration::from_secs(2)).is_some(), "no first frame");

        let started = Instant::now();
        for _ in 0..1000 {
            let _ = play.latest();
        }
        let total = started.elapsed();
        play.stop();
        println!("1000 latest() calls took {:.3} ms", total.as_secs_f64() * 1e3);
        assert!(
            total < Duration::from_millis(50),
            "1000 calls took {total:?}, the UI would stall"
        );
    }

    /// (d) A seek moves the picture, and the decoder keeps running after it.
    #[test]
    fn a_seek_changes_the_picture_and_playback_continues() {
        let Some(fx) = fixture("playseek") else { return };
        let info = probe(&fx.path).expect("probe");
        let mut play = Playback::new(&fx.path, info);
        play.start(0.0).expect("start");
        let before = next_frame(&mut play, Duration::from_secs(2)).expect("a frame before the seek");

        play.seek(2.5).expect("seek");
        let after = next_frame(&mut play, Duration::from_secs(2)).expect("a frame after the seek");
        let diff = mean_abs_diff(&before.rgb, &after.rgb);
        println!("mean abs difference across the seek: {diff:.2} levels");
        assert!(
            diff > 5.0,
            "the picture barely moved across the seek: {diff:.2} levels"
        );

        let mut more = 0usize;
        let deadline = Instant::now() + Duration::from_secs(3);
        while more < 10 && Instant::now() < deadline {
            if play.latest().is_some() {
                more += 1;
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
        play.stop();
        println!("{more} frames after the seek");
        assert!(more >= 10, "playback stopped after the seek: {more} frames");
    }

    /// (e) Stopping kills the decoder: no child survives, and the player hands out nothing
    /// instead of panicking.
    #[test]
    fn stop_kills_the_decoder_and_leaves_no_frame_behind() {
        let Some(fx) = fixture("playkill") else { return };
        let info = probe(&fx.path).expect("probe");
        let mut play = Playback::new(&fx.path, info);
        play.start(0.0).expect("start");
        let pid = play.pid().expect("a running decoder");
        assert!(proc_exists(pid), "/proc/{pid} should exist while playing");
        assert!(next_frame(&mut play, Duration::from_secs(2)).is_some(), "no first frame");

        play.stop();
        let alive = proc_exists(pid);
        let siblings = decoder_children();
        println!(
            "pid {pid}: /proc/{pid} exists after stop: {alive}; ffmpeg/sonix children of {}: {siblings:?}",
            std::process::id()
        );
        assert!(!alive, "decoder {pid} survived stop()");
        assert!(
            !siblings.iter().any(|(other, _)| *other == pid),
            "decoder {pid} is still a child: {siblings:?}"
        );
        assert!(play.latest().is_none(), "a stopped playback must yield no frame");
        assert!(play.latest().is_none(), "and no panic on asking again");
    }
}
