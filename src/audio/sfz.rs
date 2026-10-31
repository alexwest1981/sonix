//! **SFZ instruments** — reads a `.sfz` file into the same zones the sampler already plays.
//!
//! SFZ is plain text: a `<section>` header (`global`, `master`, `group`, `region`, `control`)
//! followed by `opcode=value` pairs. A `<region>` is one sample with a key/velocity range, which
//! is exactly a [`SampleZone`]; `global` and `group` opcodes are defaults that regions inherit.
//! The instrument therefore needs no new playback code — it lands in the keymap like an `.sf2`.
//!
//! **Read:** `sample`, `key`, `lokey`, `hikey`, `pitch_keycenter`, `lovel`, `hivel`,
//! `loop_start`, `loop_end`, `loop_mode`, `offset`, `end`, `tune` (folded into the root, see
//! below), `volume`, `pan`, `seq_length`/`seq_position` (order kept as written).
//!
//! **Ignored, with intent:** `<master>` and `<control>` (the latter carries `default_path`,
//! which would change sample resolution — not read), and every opcode not listed above,
//! including `ampeg_attack`/`ampeg_release`: `SampleZone` has no envelope, so they map to 0 and
//! the voice's own envelope applies. Round-robin needs per-note voice state the sampler does not
//! have, so `seq_*` regions are kept side by side in file order and the last matching zone wins
//! (the keymap rule) — importing both is right, selecting between them is not supported yet.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::keymap::SampleZone;

/// One instrument (one `.sfz` file). `name` is the file stem.
#[derive(Debug, Clone, Default)]
pub struct SfzInstrument {
    pub name: String,
    pub zones: Vec<SampleZone>,
}

#[derive(Clone, Copy)]
enum Section {
    Global,
    Group,
    Region,
    /// `<master>` / `<control>` (and anything unknown): values are dropped.
    Skip,
}

/// Reads an `.sfz` file and resolves its samples relative to the file's own directory.
///
/// A region whose sample cannot be loaded is skipped and logged; an error is returned only when
/// **no** region loaded, and then it names every skipped file.
pub fn load(path: &str) -> Result<SfzInstrument, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    let p = Path::new(path);
    let dir = p.parent().unwrap_or_else(|| Path::new("."));
    let name = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let (zones, failures) = parse(dir, &text);
    for f in &failures {
        eprintln!("sonix: sfz: skipped region: {f}");
    }
    if zones.is_empty() {
        let why = if failures.is_empty() {
            "no <region> sections".to_string()
        } else {
            failures.join("; ")
        };
        return Err(format!("no loadable regions in '{path}': {why}"));
    }
    Ok(SfzInstrument { name, zones })
}

/// Parses the text; returns the zones that loaded and one message per region that did not.
fn parse(dir: &Path, text: &str) -> (Vec<SampleZone>, Vec<String>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut global: HashMap<String, String> = HashMap::new();
    let mut group: HashMap<String, String> = HashMap::new();
    let mut regions: Vec<HashMap<String, String>> = Vec::new();
    let mut section = Section::Skip;

    for line in text.lines() {
        // shortcut: a `//` inside a value (a URL, a doubled slash in a path) truncates it;
        // upgrade to a quote-aware scanner if such a library shows up.
        let line = line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(hdr) = token.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
                section = match hdr.trim().to_ascii_lowercase().as_str() {
                    "global" => {
                        global.clear();
                        Section::Global
                    }
                    "group" => {
                        group.clear();
                        Section::Group
                    }
                    "region" => {
                        regions.push(HashMap::new());
                        Section::Region
                    }
                    _ => Section::Skip,
                };
            } else if let Some((k, v)) = token.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim().trim_matches('"').to_string();
                match section {
                    Section::Global => {
                        global.insert(k, v);
                    }
                    Section::Group => {
                        group.insert(k, v);
                    }
                    Section::Region => {
                        if let Some(r) = regions.last_mut() {
                            r.insert(k, v);
                        }
                    }
                    Section::Skip => {}
                }
            }
        }
    }

    let mut zones = Vec::new();
    let mut failures = Vec::new();
    for (i, r) in regions.iter().enumerate() {
        if r.is_empty() {
            continue;
        }
        let get = |k: &str| -> Option<&str> {
            r.get(k)
                .or_else(|| group.get(k))
                .or_else(|| global.get(k))
                .map(|s| s.as_str())
        };
        let n = |k: &str| get(k).and_then(|v| v.trim().parse::<f32>().ok());
        let Some(sample) = get("sample") else {
            failures.push(format!("region {} has no sample", i + 1));
            continue;
        };
        let file = if Path::new(sample).is_absolute() {
            Path::new(sample).to_path_buf()
        } else {
            dir.join(sample)
        };
        let file = file.to_string_lossy().to_string();
        let (mut l, mut r, rate) = match super::wav_reader::load_audio_pcm(&file) {
            Ok(pcm) => pcm,
            Err(e) => {
                failures.push(format!("'{file}': {e}"));
                continue;
            }
        };

        // `offset`/`end` trim the file; loop points are absolute file frames, so they move too.
        let offset = n("offset").unwrap_or(0.0).max(0.0) as usize;
        let offset = offset.min(l.len());
        let end = n("end")
            .filter(|e| *e > 0.0)
            .map(|e| (e as usize).min(l.len()))
            .unwrap_or(l.len());
        if end <= offset {
            failures.push(format!(
                "'{file}': end ({end}) is not after offset ({offset})"
            ));
            continue;
        }
        l = l[offset..end].to_vec();
        r = r[offset..end].to_vec();

        // `volume` (dB) and `pan` (-100..100) are baked into the samples: a zone has no gain
        // or pan field, and baking is cheaper than widening every zone for two scalars.
        let gain = n("volume").map(|db| 10f32.powf(db / 20.0)).unwrap_or(1.0);
        let pan = n("pan").unwrap_or(0.0).clamp(-100.0, 100.0);
        if gain != 1.0 || pan != 0.0 {
            let gl = gain * (1.0 - pan.max(0.0) / 100.0);
            let gr = gain * (1.0 + pan.min(0.0) / 100.0);
            l.iter_mut().for_each(|s| *s *= gl);
            r.iter_mut().for_each(|s| *s *= gr);
        }

        let looping = matches!(get("loop_mode"), Some("loop_continuous" | "loop_sustain"));
        let loop_frames = if looping {
            let ls = n("loop_start").unwrap_or(0.0).max(0.0) as i64;
            let le = n("loop_end").unwrap_or(0.0) as i64;
            let (ls, le) = (ls - offset as i64, le - offset as i64);
            (ls >= 0 && le > ls && le as usize <= l.len()).then_some((ls as u32, le as u32))
        } else {
            None
        };

        // `tune` (cents) has no field either; it is folded into the root one semitone at a time.
        let key = get("key").and_then(note_number);
        let base = get("pitch_keycenter")
            .and_then(note_number)
            .or(key)
            .unwrap_or(60) as i32;
        let root = (base - (n("tune").unwrap_or(0.0) / 100.0).round() as i32).clamp(0, 127) as u8;
        let key_low = get("lokey").and_then(note_number).or(key).unwrap_or(0);
        let key_high = get("hikey").and_then(note_number).or(key).unwrap_or(127);

        zones.push(SampleZone {
            sample_path: Some(file),
            pcm: Some((Arc::new(l), Arc::new(r), rate)),
            root,
            key_low,
            key_high,
            vel_low: n("lovel").unwrap_or(0.0).clamp(0.0, 127.0) / 127.0,
            vel_high: n("hivel").unwrap_or(127.0).clamp(0.0, 127.0) / 127.0,
            loop_frames,
        });
    }
    (zones, failures)
}

/// `c4` → 60, `f#3` → 54, `a#2` → 46, `60` → 60 (SFZ treats C4 as middle C).
fn note_number(v: &str) -> Option<u8> {
    let s = v.trim();
    if let Ok(n) = s.parse::<i32>() {
        return u8::try_from(n).ok();
    }
    let b = s.as_bytes();
    let mut acc: i32 = match b.first()?.to_ascii_lowercase() {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    };
    let mut i = 1;
    match b.get(i) {
        Some(b'#') => {
            acc += 1;
            i += 1;
        }
        Some(b'b') => {
            acc -= 1;
            i += 1;
        }
        _ => {}
    }
    let octave: i32 = s.get(i..)?.trim().parse().ok()?;
    u8::try_from(12 * (octave + 1) + acc).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sonix-sfz-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A 16-bit mono WAV with `frames` nonzero frames, so the length is checkable.
    fn wav(path: &Path, frames: usize) {
        let data = frames as u32 * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&44_100u32.to_le_bytes());
        v.extend_from_slice(&88_200u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data.to_le_bytes());
        for i in 0..frames as i16 {
            v.extend_from_slice(&((i % 100 + 1) * 100).to_le_bytes());
        }
        std::fs::write(path, v).unwrap();
    }

    fn write_sfz(d: &Path, name: &str, text: &str) -> String {
        let p = d.join(name);
        std::fs::write(&p, text).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn three_regions_load_with_their_ranges_roots_and_loops() {
        let d = dir("three");
        wav(&d.join("low.wav"), 200);
        wav(&d.join("mid.wav"), 300);
        wav(&d.join("high.wav"), 400);
        let p = write_sfz(
            &d,
            "three.sfz",
            "<global>\nvolume=0\n\n<group>\nlovel=0 hivel=127\n\n<region>\nsample=low.wav\nlokey=c2 hikey=b3\npitch_keycenter=c4\nloop_mode=loop_continuous\nloop_start=20 loop_end=120\n\n<region>\nsample=mid.wav\nlokey=c4 hikey=b4\npitch_keycenter=e4\nloop_mode=loop_continuous\nloop_start=10 loop_end=200\n\n<region>\nsample=high.wav\nlokey=c5 hikey=c7\npitch_keycenter=g5\noffset=50\nloop_mode=loop_continuous\nloop_start=60 loop_end=350\n",
        );
        let inst = load(&p).expect("three regions should load");
        assert_eq!(inst.name, "three");
        assert_eq!(inst.zones.len(), 3);
        for z in &inst.zones {
            let (l, r, rate) = z.pcm.as_ref().expect("zone has audio");
            println!(
                "{} keys {}..{} root {} loop {:?} frames {} rate {}",
                Path::new(z.sample_path.as_deref().unwrap())
                    .file_name()
                    .unwrap()
                    .to_string_lossy(),
                z.key_low,
                z.key_high,
                z.root,
                z.loop_frames,
                l.len(),
                rate
            );
            assert_eq!(l.len(), r.len());
        }
        assert_eq!((inst.zones[0].key_low, inst.zones[0].key_high), (36, 59));
        assert_eq!((inst.zones[1].key_low, inst.zones[1].key_high), (60, 71));
        assert_eq!((inst.zones[2].key_low, inst.zones[2].key_high), (72, 96));
        assert_eq!(
            [inst.zones[0].root, inst.zones[1].root, inst.zones[2].root],
            [60, 64, 79]
        );
        assert_eq!(inst.zones[0].loop_frames, Some((20, 120)));
        assert_eq!(inst.zones[1].loop_frames, Some((10, 200)));
        // offset 50 trims the sample and moves the loop with it.
        assert_eq!(inst.zones[2].loop_frames, Some((10, 300)));
        assert_eq!(inst.zones[2].pcm.as_ref().unwrap().0.len(), 350);
    }

    #[test]
    fn note_names_and_numbers_give_the_right_midi_numbers() {
        for (text, want) in [("c4", 60u8), ("f#3", 54), ("a#2", 46), ("60", 60)] {
            let got = note_number(text).expect("note should parse");
            println!("{text} -> {got}");
            assert_eq!(got, want, "{text}");
        }
        assert_eq!(note_number("c-1"), Some(0));
        assert_eq!(note_number("g9"), Some(127));
        assert_eq!(note_number("h4"), None);
    }

    #[test]
    fn round_robin_regions_are_both_kept_in_file_order() {
        let d = dir("rr");
        wav(&d.join("rr1.wav"), 100);
        wav(&d.join("rr2.wav"), 100);
        let p = write_sfz(
            &d,
            "rr.sfz",
            "<region>\nsample=rr1.wav\nkey=c4\nseq_length=2 seq_position=1\n\n<region>\nsample=rr2.wav\nkey=c4\nseq_length=2 seq_position=2\n",
        );
        let inst = load(&p).expect("round robin should load");
        let order: Vec<String> = inst
            .zones
            .iter()
            .map(|z| {
                Path::new(z.sample_path.as_deref().unwrap())
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        println!("round-robin order: {order:?}");
        assert_eq!(order, vec!["rr1.wav".to_string(), "rr2.wav".to_string()]);
        assert_eq!(
            (inst.zones[0].key_low, inst.zones[0].key_high),
            (60, 60),
            "key= sets both ends"
        );
    }

    #[test]
    fn a_missing_sample_is_reported_and_the_others_still_load() {
        let d = dir("missing");
        wav(&d.join("ok.wav"), 100);
        let p = write_sfz(
            &d,
            "missing.sfz",
            "<region>\nsample=ok.wav\nkey=c4\n\n<region>\nsample=gone.wav\nkey=d4\n",
        );
        let text = std::fs::read_to_string(&p).unwrap();
        let (zones, failures) = parse(&d, &text);
        println!("failures: {failures:?}");
        println!("surviving zones: {}", zones.len());
        assert_eq!(zones.len(), 1);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("gone.wav"), "{:?}", failures);
        // The same file through `load` is Ok — one bad region does not throw the rest away.
        assert_eq!(load(&p).unwrap().zones.len(), 1);
    }

    #[test]
    fn a_file_with_no_loadable_region_is_an_error_naming_the_reason() {
        let d = dir("empty");
        let p = write_sfz(&d, "empty.sfz", "<region>\nsample=nowhere.wav\nkey=c4\n");
        let err = load(&p).expect_err("no sample, no instrument");
        println!("error: {err}");
        assert!(err.contains("nowhere.wav"), "{err}");

        let p2 = write_sfz(&d, "headers-only.sfz", "<control>\ndefault_path=./x\n");
        let err2 = load(&p2).expect_err("headers only is an error");
        println!("error: {err2}");
        assert!(err2.contains("no <region>"), "{err2}");
    }
}
