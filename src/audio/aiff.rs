//! AIFF / AIFC read here, in process.
//!
//! **Why.** Symphonia has no IFF reader in this build, so `.aiff`/`.aifc` imports went through
//! the external `ffmpeg` binary and a machine without it could not open them at all. The
//! container is small enough to read by hand: `FORM`, one `COMM`, one `SSND`, and the PCM in
//! between. Only the compression types a DAW actually exports are accepted (`NONE`, `sowt`,
//! `fl32`); every other byte pattern is an `Err` that names what failed — this runs on files
//! the user picked, not on trusted input.

/// A decoded sample as f32 plus the divisor that brought the integer form into [-1, 1).
fn sample(bytes: &[u8], bits: u16, little: bool, float: bool) -> f32 {
    if float {
        // fl32/FL32 are big-endian only; there is no little-endian float type in AIFC.
        return f32::from_bits(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
    }
    let (v, div) = match bits {
        8 => (bytes[0] as i8 as i32, 128.0),
        16 => {
            let mut b = [bytes[0], bytes[1]];
            if little {
                b.reverse();
            }
            (i16::from_be_bytes(b) as i32, 32_768.0)
        }
        24 => {
            let mut b = [bytes[0], bytes[1], bytes[2]];
            if little {
                b.reverse();
            }
            let raw = ((b[0] as i32) << 16 | (b[1] as i32) << 8 | b[2] as i32) << 8 >> 8;
            (raw, 8_388_608.0)
        }
        32 => {
            let mut b = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if little {
                b.reverse();
            }
            (i32::from_be_bytes(b), 2_147_483_648.0)
        }
        _ => unreachable!("sample width is validated before decoding"),
    };
    v as f32 / div
}

/// The 80-bit IEEE 754 extended float AIFF stores its sample rate in — 10 bytes, big endian:
/// a sign bit, a 15-bit exponent biased by 16383, and an explicit 64-bit mantissa (no hidden
/// bit, unlike f32/f64).
fn extended_to_f64(b: &[u8]) -> f64 {
    let neg = b[0] & 0x80 != 0;
    let exp = (((b[0] & 0x7f) as i32) << 8) | b[1] as i32;
    let mant = u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]);
    if exp == 0x7fff {
        return f64::NAN; // infinity or NaN: no sample rate is hiding in there
    }
    if exp == 0 && mant == 0 {
        return 0.0;
    }
    let mag = mant as f64 * 2f64.powi(exp - 16_383 - 63);
    if neg {
        -mag
    } else {
        mag
    }
}

/// Decodes an AIFF or AIFC file into stereo at the file's own rate; mono is duplicated.
pub fn decode_file(path: &str) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    let data = std::fs::read(path).map_err(|e| format!("could not read {path}: {e}"))?;
    decode_bytes(&data)
}

fn decode_bytes(data: &[u8]) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    if data.len() < 12 || &data[0..4] != b"FORM" {
        return Err("not an IFF file: no FORM header".to_string());
    }
    let aifc = match &data[8..12] {
        b"AIFF" => false,
        b"AIFC" => true,
        other => {
            return Err(format!(
                "FORM type '{}' is neither AIFF nor AIFC",
                String::from_utf8_lossy(other)
            ))
        }
    };

    // The FORM size field is not trusted; every chunk is bounded by the real file length.
    let (mut channels, mut frames, mut bits, mut rate, mut compression) = (0u16, 0u32, 0u16, 0u32, *b"NONE");
    let mut ssnd: Option<(usize, usize)> = None;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]) as usize;
        let body = pos + 8;
        if body + size > data.len() {
            return Err(format!(
                "chunk '{}' claims {} bytes but only {} remain in the file",
                String::from_utf8_lossy(id),
                size,
                data.len() - body
            ));
        }
        let d = &data[body..body + size];
        match id {
            b"COMM" => {
                let need = if aifc { 22 } else { 18 };
                if size < need {
                    return Err(format!(
                        "COMM chunk is {size} bytes, {need} required for {}",
                        if aifc { "AIFC" } else { "AIFF" }
                    ));
                }
                channels = u16::from_be_bytes([d[0], d[1]]);
                frames = u32::from_be_bytes([d[2], d[3], d[4], d[5]]);
                bits = u16::from_be_bytes([d[6], d[7]]);
                let secs = extended_to_f64(&d[8..18]);
                if !secs.is_finite() || secs < 1.0 || secs > 1_000_000.0 {
                    return Err(format!("COMM sample rate is not a usable frequency: {secs}"));
                }
                rate = secs.round() as u32;
                if aifc {
                    compression = [d[18], d[19], d[20], d[21]];
                }
            }
            b"SSND" => {
                if size < 8 {
                    return Err(format!("SSND chunk is {size} bytes, 8 required for its header"));
                }
                let offset = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) as usize;
                // d[4..8] is the block alignment, which is informational for a whole file.
                if offset > size - 8 {
                    return Err(format!(
                        "SSND offset {offset} runs past the {} bytes of PCM it holds",
                        size - 8
                    ));
                }
                ssnd = Some((body + 8 + offset, size - 8 - offset));
            }
            _ => {} // FVER, NAME, ANNO, MARK…, nothing we need
        }
        pos = body + size + (size & 1);
    }

    if channels == 0 {
        return Err("no COMM chunk, or it declares zero channels".to_string());
    }
    let (start, avail) = ssnd.ok_or("no SSND chunk")?;
    let float = match &compression {
        b"NONE" => false,
        b"sowt" => false,
        b"fl32" | b"FL32" => true,
        other => {
            return Err(format!(
                "unsupported AIFC compression type '{}'",
                String::from_utf8_lossy(other)
            ))
        }
    };
    let little = &compression == b"sowt";
    let width = match (bits, float) {
        (8, false) => 1,
        (16, false) => 2,
        (24, false) => 3,
        (32, _) => 4,
        _ => {
            return Err(format!(
                "unsupported sample size {bits} bits (8, 16, 24 and 32 are handled)"
            ))
        }
    };
    let needed = frames as usize * channels as usize * width;
    if avail < needed {
        return Err(format!(
            "SSND holds {avail} bytes but COMM declares {frames} frames x {channels} channels x {bits} bits = {needed} bytes"
        ));
    }

    let (mut left, mut right) = (
        Vec::with_capacity(frames as usize),
        Vec::with_capacity(frames as usize),
    );
    let mut p = start;
    for _ in 0..frames {
        let mut first = 0.0f32;
        let mut second = None;
        for c in 0..channels as usize {
            let s = sample(&data[p..p + width], bits, little, float);
            p += width;
            match c {
                0 => first = s,
                1 => second = Some(s),
                _ => {} // more than two channels: the extras are dropped
            }
        }
        left.push(first);
        right.push(second.unwrap_or(first));
    }
    Ok((left, right, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    fn temp_path(tag: &str, ext: &str) -> String {
        std::env::temp_dir()
            .join(format!("sonix-aiff-probe-{tag}-{}.{ext}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    }

    /// Builds a fixture with ffmpeg. `false` means "no ffmpeg here" — said out loud, never silent.
    fn encode(tag: &str, codec: &str, ext: &str, seconds: f64, gain: f64) -> Option<String> {
        if !std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            println!("SKIP: ffmpeg is not installed, so no {tag} fixture can be built");
            return None;
        }
        let path = temp_path(tag, ext);
        let out = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:sample_rate={SR}:duration={seconds}"),
                "-c:a",
                codec,
                // ffmpeg's sine source peaks at 1/8 full scale; `gain` lifts it for the tests.
                "-af",
                &format!("volume={gain}"),
                &path,
            ])
            .output()
            .expect("ffmpeg should start");
        if !out.status.success() {
            println!(
                "SKIP: ffmpeg could not write {tag} ({codec}): {}",
                String::from_utf8_lossy(&out.stderr)
            );
            return None;
        }
        Some(path)
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

    /// (a) A 16-bit AIFF lands on 44.1 kHz, 88 200 frames, and the tone inside is 440 Hz.
    #[test]
    fn a_16_bit_aiff_decodes_to_the_tone_that_was_encoded() {
        let Some(path) = encode("s16", "pcm_s16be", "aiff", 2.0, 1.0) else {
            return;
        };
        let (l, r, rate) = decode_file(&path).expect("a 16-bit AIFF must decode");
        let peak = dominant(&l, 400.0, 480.0, 0.5);
        println!(
            "s16: rate {rate} Hz, {} frames, peak {peak:.1} Hz, peak sample {:.4}, RMS {:.2} dBFS",
            l.len(),
            l.iter().fold(0.0f32, |m, v| m.max(v.abs())),
            db(rms(&l))
        );
        assert_eq!(rate, SR, "sample rate");
        assert!(
            (l.len() as i64 - 88_200).abs() <= 2,
            "expected 88200 frames, got {}",
            l.len()
        );
        assert_eq!(l.len(), r.len(), "stereo channels must be the same length");
        assert!((peak - 440.0).abs() <= 2.0, "dominant {peak:.2} Hz, expected 440");

        // The route the app uses: the chain has to land on this reader, not on ffmpeg.
        let (via, _, via_rate) =
            super::super::wav_reader::load_audio_pcm(&path).expect("load_audio_pcm on an AIFF");
        println!(
            "load_audio_pcm: {via_rate} Hz, {} frames{}",
            via.len(),
            if via == l { ", identical to decode_file" } else { "" }
        );
        assert_eq!((via_rate, via.len()), (rate, l.len()), "load_audio_pcm route");
        let _ = std::fs::remove_file(&path);
    }

    /// (b) 24-bit and 8-bit variants: the frame count is right and, for 24-bit, the level is
    /// the same as the 16-bit encode of the same source (quantisation only).
    #[test]
    fn other_sample_sizes_decode_with_the_same_frames_and_level() {
        let Some(p16) = encode("s16b", "pcm_s16be", "aiff", 1.0, 1.0) else {
            return;
        };
        let Some(p24) = encode("s24", "pcm_s24be", "aiff", 1.0, 1.0) else {
            let _ = std::fs::remove_file(&p16);
            return;
        };
        let Some(p8) = encode("s8", "pcm_s8", "aiff", 1.0, 1.0) else {
            let _ = std::fs::remove_file(&p16);
            let _ = std::fs::remove_file(&p24);
            return;
        };
        let (l16, _, r16) = decode_file(&p16).expect("16-bit");
        let (l24, _, r24) = decode_file(&p24).expect("24-bit");
        let (l8, _, r8) = decode_file(&p8).expect("8-bit");
        let (db16, db24, db8) = (db(rms(&l16)), db(rms(&l24)), db(rms(&l8)));
        println!(
            "frames: 16-bit {} | 24-bit {} | 8-bit {} (rate {} {} {})",
            l16.len(),
            l24.len(),
            l8.len(),
            r16,
            r24,
            r8
        );
        println!("RMS: 16-bit {db16:.3} dBFS | 24-bit {db24:.3} dBFS | 8-bit {db8:.3} dBFS");
        for (bits, got) in [("16", l16.len()), ("24", l24.len()), ("8", l8.len())] {
            assert!(
                (got as i64 - 44_100).abs() <= 2,
                "{bits}-bit: expected 44100 frames, got {got}"
            );
        }
        assert!(
            (db24 - db16).abs() <= 0.2,
            "24-bit RMS {db24:.3} dB is {:.3} dB away from 16-bit {db16:.3} dB",
            (db24 - db16).abs()
        );
        let _ = std::fs::remove_file(&p16);
        let _ = std::fs::remove_file(&p24);
        let _ = std::fs::remove_file(&p8);
    }

    /// (c) AIFC: float32 at full scale, and `sowt` (little endian) against the big-endian file.
    #[test]
    fn aifc_float_and_sowt_decode() {
        let Some(pf) = encode("fl32", "pcm_f32be", "aifc", 1.0, 8.0) else {
            return;
        };
        let (fl, _, frate) = decode_file(&pf).expect("float AIFC must decode");
        let fpeak = fl.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        println!(
            "fl32: rate {frate} Hz, {} frames, peak {fpeak:.6} (full scale), RMS {:.2} dBFS",
            fl.len(),
            db(rms(&fl))
        );
        assert_eq!(frate, SR, "float AIFC rate");
        assert!((fl.len() as i64 - 44_100).abs() <= 2, "float frames");
        assert!(
            (0.99..=1.0).contains(&fpeak),
            "a full-scale float sine should peak at ~1.0, got {fpeak:.6} — clipping or an endian slip"
        );
        assert!(
            fl.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
            "float decode must stay finite and inside full scale"
        );
        let _ = std::fs::remove_file(&pf);

        // ffmpeg writes pcm_s16le into an AIFC container as `sowt`, so this is a real fixture.
        let Some(ple) = encode("sowt", "pcm_s16le", "aifc", 1.0, 1.0) else {
            return;
        };
        let header = std::fs::read(&ple).unwrap();
        let sowt = header.windows(4).any(|w| w == b"sowt");
        println!("sowt fixture: FORM type '{}', compression tag present: {sowt}", String::from_utf8_lossy(&header[8..12]));
        assert!(sowt, "fixture should be an AIFC declaring sowt");
        let (le, _, ler) = decode_file(&ple).expect("sowt AIFC must decode");
        let Some(pbe) = encode("sowt-ref", "pcm_s16be", "aiff", 1.0, 1.0) else {
            let _ = std::fs::remove_file(&ple);
            return;
        };
        let (be, _, ber) = decode_file(&pbe).expect("big-endian reference");
        let (le_db, be_db) = (db(rms(&le)), db(rms(&be)));
        println!(
            "sowt: rate {ler} Hz, {} frames, RMS {le_db:.3} dB | NONE: rate {ber} Hz, {} frames, RMS {be_db:.3} dB",
            le.len(),
            be.len()
        );
        assert_eq!(ler, ber, "rate");
        assert_eq!(le.len(), be.len(), "sowt frame count");
        assert!(
            (le_db - be_db).abs() < 0.01,
            "sowt reads {le_db:.3} dB but the big-endian file reads {be_db:.3} dB — byte order is wrong"
        );
        let _ = std::fs::remove_file(&ple);
        let _ = std::fs::remove_file(&pbe);
    }

    /// (d) The 80-bit extended rate, from hand-built 10-byte sequences.
    #[test]
    fn the_extended_sample_rate_is_parsed_by_hand() {
        // The canonical encodings: exponent 0x400E is 2^15, 0x400F is 2^16.
        let cases: [([u8; 10], u32); 3] = [
            ([0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0], 44_100),
            ([0x40, 0x0e, 0xbb, 0x80, 0, 0, 0, 0, 0, 0], 48_000),
            ([0x40, 0x0f, 0xbb, 0x80, 0, 0, 0, 0, 0, 0], 96_000),
        ];
        for (bytes, want) in cases {
            let got = extended_to_f64(&bytes);
            println!(
                "{} -> {got} (want {want})",
                bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
            );
            assert_eq!(got, want as f64, "extended rate");
        }
    }

    /// (e) A truncated file and a bogus COMM size are `Err` naming the failure, never a panic.
    #[test]
    fn broken_files_are_errors_that_say_what_failed() {
        let Some(path) = encode("trunc", "pcm_s16be", "aiff", 1.0, 1.0) else {
            return;
        };
        let full = std::fs::read(&path).unwrap();
        let cut = temp_path("cut", "aiff");
        std::fs::write(&cut, &full[..full.len() / 2]).unwrap();
        let err = decode_file(&cut).expect_err("a file cut inside SSND must not decode");
        println!("truncated: Err({err})");
        assert!(err.contains("SSND"), "the reason should name the chunk: {err}");

        let bad = temp_path("badcomm", "aiff");
        let mut bytes = full.clone();
        let comm = 12; // COMM is the first chunk in an AIFF from ffmpeg
        bytes[comm + 4..comm + 8].copy_from_slice(&4u32.to_be_bytes());
        std::fs::write(&bad, &bytes).unwrap();
        let err = decode_file(&bad).expect_err("a 4-byte COMM must not decode");
        println!("COMM size 4: Err({err})");
        assert!(err.contains("COMM"), "the reason should name COMM: {err}");

        let none = temp_path("garbage", "aiff");
        std::fs::write(&none, b"not an iff file at all").unwrap();
        let err = decode_file(&none).expect_err("garbage must not decode");
        println!("garbage: Err({err})");
        assert!(err.contains("FORM"), "the reason should name the header: {err}");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&cut);
        let _ = std::fs::remove_file(&bad);
        let _ = std::fs::remove_file(&none);
    }
}
