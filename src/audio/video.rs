//! **Videospår** (Cubase: video playback, FL: video-clip). Sonix visar en film synkad till
//! tidslinjen så man kan skriva musik mot bild. Filmen är inte en del av mixen — den är en
//! referens, och därför går den inte genom mastern.
//!
//! Avkodningen sker med `ffmpeg`/`ffprobe` i ett eget underordnat program, en bildruta i
//! taget, i stället för ett eget kodekbibliotek i bygget: ffmpeg finns redan på maskinen
//! (samma binär som exporten använder), och en bildruta per uppspelning är billigare än att
//! länka in libavcodec för att visa en referensfilm.

use std::process::Command;

/// Det som behövs för att lägga filmen på tidslinjen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_secs: f64,
}

impl VideoInfo {
    /// Antal bildrutor i filmen, avrundat uppåt — sista rutan räknas med.
    pub fn frame_count(&self) -> u64 {
        (self.duration_secs * self.fps).ceil().max(0.0) as u64
    }
}

/// Läser ut storlek, bildfrekvens och längd ur filen.
pub fn probe(path: &str) -> Result<VideoInfo, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            // Längden står oftast i `format`, inte i strömmen — mp4 skriver den där.
            "stream=width,height,r_frame_rate:format=duration",
            "-of",
            "json",
            path,
        ])
        .output()
        .map_err(|err| format!("kunde inte starta ffprobe: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe avvisade filen: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let width = json_number(&text, "\"width\"").ok_or("filen har ingen bredd")? as u32;
    let height = json_number(&text, "\"height\"").ok_or("filen har ingen höjd")? as u32;
    let fps = json_rate(&text, "\"r_frame_rate\"").ok_or("filen har ingen bildfrekvens")?;
    let duration_secs = json_number(&text, "\"duration\"")
        .ok_or("filen har ingen längd (är den en bild?)")?;
    if width == 0 || height == 0 || fps <= 0.0 || duration_secs <= 0.0 {
        return Err("filen har oanvändbara mått".to_string());
    }
    Ok(VideoInfo { width, height, fps, duration_secs })
}

/// Vilken bildruta som ska visas vid en position i **sekunder in i filmen**.
///
/// `offset_secs` är var filmen börjar på tidslinjen: en film som startar på takt 5 visas
/// från sin egen ruta 0 där. Före filmens början och efter dess slut finns ingen ruta — då
/// ska videospåret vara tomt, inte frysa på första eller sista bilden.
pub fn frame_index(position_secs: f64, info: &VideoInfo, offset_secs: f64) -> Option<u64> {
    let local = position_secs - offset_secs;
    if local < 0.0 || local >= info.duration_secs {
        return None;
    }
    let index = (local * info.fps).floor() as u64;
    let last = info.frame_count().saturating_sub(1);
    Some(index.min(last))
}

/// Hämtar **en** bildruta som RGB24 (tre byte per pixel, radvis uppifrån och ner).
pub fn frame_rgb(path: &str, time_secs: f64, info: &VideoInfo) -> Result<Vec<u8>, String> {
    let size = info.width as usize * info.height as usize * 3;
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-ss",
            &format!("{:.3}", time_secs.max(0.0)),
            "-i",
            path,
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .output()
        .map_err(|err| format!("kunde inte starta ffmpeg: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg kunde inte läsa bildrutan: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if output.stdout.len() < size {
        return Err(format!(
            "bildrutan är {} byte, väntade {}",
            output.stdout.len(),
            size
        ));
    }
    Ok(output.stdout[..size].to_vec())
}

/// Läser ett tal efter en nyckel i ffprobes JSON. Minimal tolkning: ffprobe skriver ett
/// objekt per ström, och vi behöver bara de fyra fälten — ingen JSON-dependency för det.
fn json_number(text: &str, key: &str) -> Option<f64> {
    let at = text.find(key)? + key.len();
    let rest = text[at..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    // ffprobe skriver `duration` som en **sträng** ("2.000000") och `width` som ett tal.
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == '+'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// `"r_frame_rate": "30000/1001"` — ffprobe skriver bildfrekvensen som ett bråk.
fn json_rate(text: &str, key: &str) -> Option<f64> {
    let at = text.find(key)? + key.len();
    let rest = text[at..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let raw = &rest[..end];
    match raw.split_once('/') {
        Some((num, den)) => {
            let num: f64 = num.parse().ok()?;
            let den: f64 = den.parse().ok()?;
            if den == 0.0 {
                None
            } else {
                Some(num / den)
            }
        }
        None => raw.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(duration: f64, fps: f64) -> VideoInfo {
        VideoInfo { width: 64, height: 48, fps, duration_secs: duration }
    }

    /// Filmen börjar på sin offset, och utanför den finns ingen bildruta att visa.
    #[test]
    fn a_frame_is_chosen_from_the_timeline_offset() {
        let v = info(2.0, 10.0);
        assert_eq!(frame_index(0.0, &v, 0.0), Some(0));
        assert_eq!(frame_index(0.15, &v, 0.0), Some(1));
        assert_eq!(frame_index(1.99, &v, 0.0), Some(19));
        assert_eq!(frame_index(2.0, &v, 0.0), None, "efter slutet är spåret tomt");
        // En film som börjar på 5 s: inget visas innan, ruta 0 visas precis vid 5 s.
        assert_eq!(frame_index(4.9, &v, 5.0), None);
        assert_eq!(frame_index(5.0, &v, 5.0), Some(0));
        assert_eq!(frame_index(5.25, &v, 5.0), Some(2));
    }

    /// Sista rutan klampas: 29,97 fps i 1 s ger 30 rutor, och position 0,999 s får inte
    /// peka på ruta 30 som inte finns.
    #[test]
    fn the_last_frame_is_clamped() {
        let v = info(1.0, 29.97);
        assert_eq!(v.frame_count(), 30);
        assert_eq!(frame_index(0.999, &v, 0.0), Some(29));
    }

    /// **Ände-till-ände:** en film med en röd och en grön sekvens avkodas med ffmpeg, och
    /// bildrutan på 1,5 s ska vara grön — det mäts i pixlarna, inte i tidsstämpeln.
    #[test]
    fn a_real_file_gives_the_right_pixels_at_the_right_time() {
        let dir = std::env::temp_dir().join(format!("sonix_video_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("two_colors.mp4");
        let path_str = path.to_string_lossy().to_string();
        // ffmpeg är ett **körtidsberoende**, inte ett byggberoende: finns det inte på
        // maskinen kan videospåret inte användas, men det ska inte fälla bygget. Provet
        // hoppar därför över med ett meddelande i stället för att ljuga om grönt.
        let have = |tool: &str| {
            Command::new(tool)
                .arg("-version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !have("ffmpeg") || !have("ffprobe") {
            println!("hoppar över: ffmpeg/ffprobe finns inte på maskinen");
            return;
        }
        // mpeg4-kodaren finns i varje ffmpeg-bygge; libx264 gör det inte (Ubuntu utan
        // universe, vissa runners), och provet ska inte hänga på vilken kodare som råkade
        // följa med.
        let output = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=64x48:d=1:r=10",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=64x48:d=1:r=10",
                "-filter_complex",
                "[0:v][1:v]concat=n=2:v=1:a=0",
                "-c:v",
                "mpeg4",
                "-q:v",
                "2",
                "-pix_fmt",
                "yuv420p",
                &path_str,
            ])
            .output()
            .expect("ffmpeg ska finnas");
        assert!(
            output.status.success(),
            "kunde inte skapa testfilmen: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );

        let info = probe(&path_str).expect("ffprobe ska läsa filen");
        assert_eq!((info.width, info.height), (64, 48));
        assert!((info.fps - 10.0).abs() < 0.01, "fps: {}", info.fps);
        assert!((info.duration_secs - 2.0).abs() < 0.2, "längd: {}", info.duration_secs);

        let mean = |rgb: &[u8]| {
            let n = (rgb.len() / 3) as f32;
            let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
            for px in rgb.chunks(3) {
                r += px[0] as f32;
                g += px[1] as f32;
                b += px[2] as f32;
            }
            (r / n, g / n, b / n)
        };
        let first = frame_rgb(&path_str, 0.5, &info).expect("ruta vid 0,5 s");
        let second = frame_rgb(&path_str, 1.5, &info).expect("ruta vid 1,5 s");
        let (r1, g1, _) = mean(&first);
        let (r2, g2, _) = mean(&second);
        println!("0,5 s: röd {r1:.0} grön {g1:.0} | 1,5 s: röd {r2:.0} grön {g2:.0}");
        assert!(r1 > 150.0 && g1 < 80.0, "0,5 s ska vara röd: R {r1:.0} G {g1:.0}");
        assert!(g2 > 100.0 && r2 < 80.0, "1,5 s ska vara grön: R {r2:.0} G {g2:.0}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
