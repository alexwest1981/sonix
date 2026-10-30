//! Red Book DDP 2.00 image export, plus the CUE/BIN sibling REAPER also ships.
//!
//! The image is data only — nothing here burns or mounts anything. The layout is
//! described in `docs/DDP.md`; the padding rule lives there and in
//! [`SECTOR_FRAMES`]/[`SECTOR_BYTES`].
//!
//! The text descriptors (DDPID, DDPMap, PQDESCR, CDTEXT) are plain ASCII records of
//! the shape this writer produces and [`tests`] parse back. They are not the
//! 18-byte CD-TEXT packs of the CD itself.
//!
//! shortcut: DDPID/DDPMap/PQDESCR/CDTEXT are ASCII lines, not the plant's binary
//! packing; upgrade when a real pressing plant validates the image.

use super::dither::TpdfDither;
use super::exporter::ExportMeta;

/// Red Book sample rate. A DDP image cannot carry any other.
pub const SAMPLE_RATE: u32 = 44_100;
/// Frames (stereo samples) in one 2352-byte CD sector at 44.1 kHz.
pub const SECTOR_FRAMES: usize = 588;
/// One CD sector: 588 frames × 2 channels × 2 bytes.
pub const SECTOR_BYTES: usize = SECTOR_FRAMES * 2 * 2;

/// One track of the image. `start_sample` is an absolute stereo frame index into the
/// source and must land on a sector boundary.
#[derive(Clone, Debug)]
pub struct TrackSpec {
    pub title: String,
    pub start_sample: u64,
    /// 12 characters: country (2) + registrant (3) + year (2) + designation (5).
    pub isrc: Option<String>,
}

/// What the caller (the CLI, mostly) needs to report after a write.
#[derive(Clone, Debug, Default)]
pub struct DdpReport {
    pub tracks: usize,
    pub sectors: u64,
    pub frames: u64,
    pub files: Vec<String>,
}

/// Uppercases and checks an ISRC. Length is counted in characters, not bytes.
fn validate_isrc(raw: &str) -> Result<String, String> {
    let isrc = raw.to_ascii_uppercase();
    if isrc.chars().count() == 12 && isrc.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(isrc);
    }
    Err(format!(
        "ISRC \"{raw}\" is invalid: expected 12 ASCII letters/digits, got {} characters{}",
        isrc.chars().count(),
        if isrc.chars().any(|c| !c.is_ascii_alphanumeric()) {
            " including a non-alphanumeric character"
        } else {
            ""
        }
    ))
}

/// Red Book absolute time of a sector: MM:SS:FF with 75 frames per second.
fn mmssff(sector: u64) -> String {
    let secs = sector / 75;
    format!("{:02}:{:02}:{:02}", secs / 60, secs % 60, sector % 75)
}

/// Validates the track list against the source length and returns the normalised
/// (uppercased) ISRCs in track order.
fn check_tracks(tracks: &[TrackSpec], frames: u64) -> Result<Vec<Option<String>>, String> {
    let Some(first) = tracks.first() else {
        return Err("no tracks given: the image needs at least one".to_string());
    };
    if first.start_sample != 0 {
        return Err(format!(
            "the first track must start at sample 0, got {}",
            first.start_sample
        ));
    }
    let mut isrcs = Vec::with_capacity(tracks.len());
    for (i, t) in tracks.iter().enumerate() {
        if t.start_sample % SECTOR_FRAMES as u64 != 0 {
            return Err(format!(
                "track {} (\"{}\") starts at sample {}, which is not a {SECTOR_FRAMES}-sample sector boundary",
                i + 1,
                t.title,
                t.start_sample
            ));
        }
        if t.start_sample >= frames {
            return Err(format!(
                "track {} (\"{}\") starts at sample {}, past the end of the source ({frames} frames)",
                i + 1,
                t.title,
                t.start_sample
            ));
        }
        if i > 0 && t.start_sample <= tracks[i - 1].start_sample {
            return Err(format!(
                "track {} (\"{}\") starts at sample {}, not after the previous track's {}",
                i + 1,
                t.title,
                t.start_sample,
                tracks[i - 1].start_sample
            ));
        }
        isrcs.push(match &t.isrc {
            Some(raw) => Some(validate_isrc(raw)?),
            None => None,
        });
    }
    Ok(isrcs)
}

/// The program as raw CD sectors: 16-bit LE stereo, each sector exactly
/// [`SECTOR_BYTES`] long, tail padded with digital silence.
fn pcm_sectors(left: &[f32], right: &[f32]) -> Vec<u8> {
    let frames = left.len().min(right.len());
    let sectors = frames.div_ceil(SECTOR_FRAMES);
    let mut out = Vec::with_capacity(sectors * SECTOR_BYTES);
    let mut dither = TpdfDither::new(crate::audio::dither::DEFAULT_SEED);
    for i in 0..sectors * SECTOR_FRAMES {
        let (l, r) = if i < frames {
            (left[i], right[i])
        } else {
            (0.0, 0.0)
        };
        out.extend_from_slice(&dither.quantize_i16(l, 0).to_le_bytes());
        out.extend_from_slice(&dither.quantize_i16(r, 1).to_le_bytes());
    }
    out
}

fn ddp_id(tracks: usize, sectors: u64) -> Vec<u8> {
    let text = format!(
        "DDP 2.00\nIMAGE_ID=SONIX\nTRACKS={tracks}\nSECTORS={sectors}\nSAMPLE_RATE={SAMPLE_RATE}\nCHANNELS=2\nBITS=16\n"
    );
    let mut id = text.into_bytes();
    // Fixed 128-byte record; the magic sits at offset 0.
    id.resize(128, b' ');
    id
}

fn pq_descr(tracks: &[TrackSpec], isrcs: &[Option<String>]) -> Vec<u8> {
    let mut out = String::new();
    for (i, t) in tracks.iter().enumerate() {
        out.push_str(&format!(
            "TN={:02} INDEX=01 ABS={} ISRC={}\n",
            i + 1,
            mmssff(t.start_sample / SECTOR_FRAMES as u64),
            isrcs[i].as_deref().unwrap_or("")
        ));
    }
    out.into_bytes()
}

/// CD-TEXT, only when there is anything to say. An empty result means "do not write
/// the file" — a CDTEXT full of blank fields is not metadata.
fn cd_text(tracks: &[TrackSpec], meta: &ExportMeta) -> Vec<u8> {
    let mut out = String::new();
    if !meta.title.is_empty() {
        out.push_str(&format!("DISC TITLE={}\n", meta.title));
    }
    if !meta.artist.is_empty() {
        out.push_str(&format!("DISC PERFORMER={}\n", meta.artist));
    }
    for (i, t) in tracks.iter().enumerate() {
        if !t.title.is_empty() {
            out.push_str(&format!("TRACK {:02} TITLE={}\n", i + 1, t.title));
        }
    }
    out.into_bytes()
}

/// DDPMap lines: one per file in the image, in image order, with its byte length and
/// the sectors it occupies (rounded up, since only AUDIO.DAT is sector-exact).
fn ddp_map(files: &[(String, usize)]) -> Vec<u8> {
    let mut out = String::new();
    for (name, bytes) in files {
        out.push_str(&format!(
            "{name} {bytes} {}\n",
            bytes.div_ceil(SECTOR_BYTES)
        ));
    }
    out.into_bytes()
}

fn load(source_wav: &str) -> Result<(Vec<f32>, Vec<f32>), String> {
    let (left, right, rate) = super::wav_reader::load_audio_pcm(source_wav)?;
    if rate != SAMPLE_RATE {
        return Err(format!(
            "source is {rate} Hz; a Red Book DDP image must be {SAMPLE_RATE} Hz. \
             Resample the file first — this export does not resample."
        ));
    }
    Ok((left, right))
}

fn write_file(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"))
}

fn ensure_dir(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

/// Writes a DDP 2.00 image (DDPID, DDPMap, PQDESCR, CDTEXT when metadata exists,
/// AUDIO.DAT) into `out_dir`.
pub fn export_ddp(
    source_wav: &str,
    out_dir: &str,
    tracks: &[TrackSpec],
    meta: &ExportMeta,
) -> Result<DdpReport, String> {
    let (left, right) = load(source_wav)?;
    let isrcs = check_tracks(tracks, left.len().min(right.len()) as u64)?;
    let dir = std::path::Path::new(out_dir);
    ensure_dir(dir)?;

    let audio = pcm_sectors(&left, &right);
    let sectors = (audio.len() / SECTOR_BYTES) as u64;
    let id = ddp_id(tracks.len(), sectors);
    let pq = pq_descr(tracks, &isrcs);
    let cdtext = cd_text(tracks, meta);
    let mut entries = vec![
        ("DDPID".to_string(), id.len()),
        ("DDPMap".to_string(), 0), // length depends on the lines below
        ("PQDESCR".to_string(), pq.len()),
    ];
    if !cdtext.is_empty() {
        entries.push(("CDTEXT".to_string(), cdtext.len()));
    }
    entries.push(("AUDIO.DAT".to_string(), audio.len()));

    // DDPMap lists itself too, so its own line carries its own byte length: iterate to
    // the fixed point instead of assuming the digit width stays put.
    let mut map = ddp_map(&entries);
    for _ in 0..4 {
        entries[1].1 = map.len();
        let next = ddp_map(&entries);
        if next == map {
            break;
        }
        map = next;
    }

    write_file(dir, "DDPID", &id)?;
    write_file(dir, "DDPMap", &map)?;
    write_file(dir, "PQDESCR", &pq)?;
    if !cdtext.is_empty() {
        write_file(dir, "CDTEXT", &cdtext)?;
    }
    write_file(dir, "AUDIO.DAT", &audio)?;

    Ok(DdpReport {
        tracks: tracks.len(),
        sectors,
        frames: left.len().min(right.len()) as u64,
        files: entries.into_iter().map(|(n, _)| n).collect(),
    })
}

/// Writes the same program as a raw CUE/BIN pair (`image.cue` + `image.bin`).
pub fn export_cue_bin(
    source_wav: &str,
    out_dir: &str,
    tracks: &[TrackSpec],
    meta: &ExportMeta,
) -> Result<DdpReport, String> {
    let (left, right) = load(source_wav)?;
    let isrcs = check_tracks(tracks, left.len().min(right.len()) as u64)?;
    let dir = std::path::Path::new(out_dir);
    ensure_dir(dir)?;

    let audio = pcm_sectors(&left, &right);
    let mut cue = String::from("FILE \"image.bin\" BINARY\n");
    if !meta.title.is_empty() || !meta.artist.is_empty() {
        cue.push_str(&format!(
            "  TITLE \"{}\"\n  PERFORMER \"{}\"\n",
            meta.title, meta.artist
        ));
    }
    for (i, t) in tracks.iter().enumerate() {
        cue.push_str(&format!("  TRACK {:02} AUDIO\n", i + 1));
        if !t.title.is_empty() {
            cue.push_str(&format!("    TITLE \"{}\"\n", t.title));
        }
        if let Some(isrc) = &isrcs[i] {
            cue.push_str(&format!("    ISRC {isrc}\n"));
        }
        cue.push_str(&format!(
            "    INDEX 01 {}\n",
            mmssff(t.start_sample / SECTOR_FRAMES as u64)
        ));
    }

    write_file(dir, "image.bin", &audio)?;
    write_file(dir, "image.cue", cue.as_bytes())?;

    Ok(DdpReport {
        tracks: tracks.len(),
        sectors: (audio.len() / SECTOR_BYTES) as u64,
        frames: left.len().min(right.len()) as u64,
        files: vec!["image.cue".to_string(), "image.bin".to_string()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::exporter::{DitherSettings, write_wav};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sonix-ddp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Sine-free deterministic ramp, so the test never depends on a synth.
    fn ramp(frames: usize) -> (Vec<f32>, Vec<f32>) {
        let left: Vec<f32> = (0..frames)
            .map(|i| ((i as f32) / frames as f32) * 0.5 - 0.25)
            .collect();
        let right = left.iter().map(|s| s * 0.5).collect();
        (left, right)
    }

    fn source_wav(dir: &std::path::Path, frames: usize, rate: u32) -> String {
        let (l, r) = ramp(frames);
        let mut interleaved = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            interleaved.push(l[i]);
            interleaved.push(r[i]);
        }
        let path = dir.join(format!("source-{rate}.wav"));
        write_wav(
            path.to_str().unwrap(),
            &interleaved,
            rate,
            16,
            false,
            &ExportMeta::default(),
            DitherSettings::default(),
        )
        .unwrap();
        path.to_string_lossy().into_owned()
    }

    fn tracks() -> Vec<TrackSpec> {
        vec![
            TrackSpec {
                title: "Opening".to_string(),
                start_sample: 0,
                isrc: Some("seabc2612345".to_string()),
            },
            TrackSpec {
                title: "Closing".to_string(),
                start_sample: 4 * SECTOR_FRAMES as u64,
                isrc: None,
            },
        ]
    }

    fn meta() -> ExportMeta {
        ExportMeta {
            title: "Två spår".to_string(),
            artist: "Sonix".to_string(),
            ..Default::default()
        }
    }

    // (a) A two-track image parses back with the documented shapes.
    #[test]
    fn ddp_image_two_tracks_parses_back() {
        let dir = temp_dir("two-tracks");
        // 10.5 sectors: the tail must be padded up to 11 sectors.
        let frames = 10 * SECTOR_FRAMES + 300;
        let src = source_wav(&dir, frames, SAMPLE_RATE);
        let out = dir.join("image");
        let report = export_ddp(&src, out.to_str().unwrap(), &tracks(), &meta()).unwrap();

        let id = std::fs::read(out.join("DDPID")).unwrap();
        assert!(id.starts_with(b"DDP 2.00"), "DDPID magic: {:?}", &id[..8]);
        assert_eq!(id.len(), 128);

        let map_text = std::fs::read_to_string(out.join("DDPMap")).unwrap();
        let mapped: Vec<&str> = map_text
            .lines()
            .map(|l| l.split(' ').next().unwrap())
            .collect();
        let mut on_disk: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut mapped_sorted: Vec<String> = mapped.iter().map(|s| s.to_string()).collect();
        mapped_sorted.sort();
        assert_eq!(mapped_sorted, on_disk, "every file on disk is in DDPMap");
        assert_eq!(report.files.len(), on_disk.len());

        let pq = std::fs::read_to_string(out.join("PQDESCR")).unwrap();
        assert_eq!(pq.lines().count(), 2, "one PQDESCR entry per track");
        assert!(pq.lines().next().unwrap().contains("ABS=00:00:00"));
        assert!(pq.lines().nth(1).unwrap().contains("ABS=00:00:04"));

        let cdtext = std::fs::read_to_string(out.join("CDTEXT")).unwrap();
        assert!(cdtext.contains("DISC TITLE=Två spår"));
        assert!(cdtext.contains("TRACK 02 TITLE=Closing"));

        let audio = std::fs::read(out.join("AUDIO.DAT")).unwrap();
        assert_eq!(audio.len() % SECTOR_BYTES, 0, "AUDIO.DAT is whole sectors");
        // Rule: frames are rounded up to the next 588-frame sector and the tail is
        // padded with digital silence, so the file is 11 full sectors.
        assert_eq!(audio.len(), 11 * SECTOR_BYTES);
        assert_eq!(report.sectors, 11);
        assert_eq!(audio.len() / 4, 11 * SECTOR_FRAMES, "frames after padding");
        assert_eq!(report.frames, frames as u64);

        let dir2 = temp_dir("cue-bin");
        let src2 = source_wav(&dir2, frames, SAMPLE_RATE);
        let out2 = dir2.join("image");
        export_cue_bin(&src2, out2.to_str().unwrap(), &tracks(), &meta()).unwrap();
        let bin = std::fs::read(out2.join("image.bin")).unwrap();
        assert_eq!(bin.len(), audio.len(), "CUE/BIN carries the same sectors");
        let cue = std::fs::read_to_string(out2.join("image.cue")).unwrap();
        assert!(cue.contains("FILE \"image.bin\" BINARY"));
        assert!(cue.contains("TRACK 02 AUDIO"));
        assert!(
            cue.contains("ISRC SEABC2612345"),
            "ISRC is uppercased: {cue}"
        );
        assert_eq!(cue.matches("INDEX 01").count(), 2);
    }

    // (b) A bad ISRC is refused by name, not silently dropped.
    #[test]
    fn ddp_invalid_isrc_is_refused() {
        let dir = temp_dir("bad-isrc");
        let src = source_wav(&dir, 2 * SECTOR_FRAMES, SAMPLE_RATE);
        for bad in ["SEABC261234", "SEABC 612345", "SEABC26123-5", ""] {
            let mut t = tracks();
            t[0].isrc = Some(bad.to_string());
            let err = export_ddp(&src, dir.join("out").to_str().unwrap(), &t, &meta()).unwrap_err();
            assert!(err.contains("ISRC"), "error names the field: {err}");
            assert!(err.contains(bad), "error quotes the value: {err}");
        }
    }

    // (c) A 48 kHz source is refused instead of being resampled behind the user's back.
    #[test]
    fn ddp_48k_source_is_refused() {
        let dir = temp_dir("48k");
        let src = source_wav(&dir, SECTOR_FRAMES, 48_000);
        let err =
            export_ddp(&src, dir.join("out").to_str().unwrap(), &tracks(), &meta()).unwrap_err();
        assert!(err.contains("48000"), "names the source rate: {err}");
        assert!(err.contains("44100"), "names the required rate: {err}");
    }
}
