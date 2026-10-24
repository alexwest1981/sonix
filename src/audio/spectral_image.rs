//! **Bild → spektrum** (Harmor): en PNG blir dragstängerna i den additiva synten.
//!
//! Bilden läses som ljusstyrka, rad för rad: nedersta raden är grundtonen och raden ovanför
//! är överton 2, och så vidare upp till `HARMONIC_COUNT`. Alla kolumner vägs samman, eftersom
//! den här delen av Harmor bara tar **en** spektralbild — tidsaxeln (bilden som spelas upp
//! över tid) och `.fnv`-envelopes är kvar att bygga.
//!
//! Ingen FFT: en additiv synt behöver bara nivåerna per överton, och då räcker det att
//! medelvärdet per rad blir en nivå. Att FFT:a en bild vore att räkna om samma sak två gånger.

use crate::audio::synth::HARMONIC_COUNT;

/// Läser en PNG ur minnet och lämnar nivåerna för övertonerna 1–64 (index 0 = grundton).
///
/// Raderna räknas **nedifrån och upp**: en ljus prick längst ned blir grundtonen, en ljus
/// prick högst upp blir överton 64. Bilden behöver inte vara kvadratisk — raderna delas in i
/// `HARMONIC_COUNT` band och varje band blir medelvärdet av sina rader.
pub fn envelope_from_png(bytes: &[u8]) -> Result<[f32; HARMONIC_COUNT], String> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| format!("bilden går inte att läsa: {e}"))?;
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    if w == 0 || h == 0 {
        return Err("bilden är tom".to_string());
    }

    let mut out = [0.0f32; HARMONIC_COUNT];
    for (band, level) in out.iter_mut().enumerate() {
        // Band 0 = nedersta raderna (låg frekvens), band 15 = översta.
        let from = h as usize * band / HARMONIC_COUNT;
        let to = (h as usize * (band + 1) / HARMONIC_COUNT).max(from + 1).min(h as usize);
        let mut sum = 0.0f32;
        let mut n = 0usize;
        for offset in from..to {
            // Bildens y går uppifrån och ned; spektrumet räknas nedifrån och upp.
            let y = h as usize - 1 - offset;
            for x in 0..w as usize {
                sum += gray.get_pixel(x as u32, y as u32)[0] as f32 / 255.0;
                n += 1;
            }
        }
        *level = if n > 0 { sum / n as f32 } else { 0.0 };
    }

    // Normalisera till 1,0: en mörk bild ska ge samma klang som en ljus, bara svagare —
    // och en bild som är helt svart ska ge tystnad, inte division med noll.
    let peak = out.iter().cloned().fold(0.0f32, f32::max);
    if peak > 0.0 {
        for level in out.iter_mut() {
            *level = (*level / peak).clamp(0.0, 1.0);
        }
    }
    Ok(out)
}

/// Läser en PNG från disk. Felmeddelandet är avsiktligt i klartext: det hamnar i statusraden.
pub fn envelope_from_file(path: &std::path::Path) -> Result<[f32; HARMONIC_COUNT], String> {
    let bytes = std::fs::read(path).map_err(|e| format!("filen går inte att läsa: {e}"))?;
    envelope_from_png(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skriver en PNG i minnet: `rows[i]` är ljusstyrkan (0–255) för rad i, uppifrån räknat.
    fn png_with_rows(rows: &[u8]) -> Vec<u8> {
        let w = 8u32;
        let h = rows.len() as u32;
        let mut img = image::GrayImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, image::Luma([rows[y as usize]]));
            }
        }
        let mut out = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut out);
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        out
    }

    /// **Bilden blir ett spektrum**: mörk bild = tyst, ljus nederkant = grundton,
    /// ljus överkant = överton 64, och nivåerna normaliseras till 1,0.
    #[test]
    fn a_bright_bottom_row_becomes_the_fundamental() {
        let mut rows = vec![0u8; 64];
        for r in rows.iter_mut().skip(60) {
            *r = 255; // nedersta bandet
        }
        let env = envelope_from_png(&png_with_rows(&rows)).unwrap();
        assert!(env[0] > 0.9, "grundtonen ska dominera: {env:?}");
        assert!(env[HARMONIC_COUNT - 1] < 0.1, "toppen ska vara mörk: {env:?}");

        let mut rows = vec![0u8; 64];
        for r in rows.iter_mut().take(4) {
            *r = 255; // översta bandet
        }
        let env = envelope_from_png(&png_with_rows(&rows)).unwrap();
        assert!(env[HARMONIC_COUNT - 1] > 0.9, "översta övertonen ska dominera: {env:?}");
        assert!(env[0] < 0.1, "botten ska vara mörk: {env:?}");

        let dark = envelope_from_png(&png_with_rows(&[0u8; 64])).unwrap();
        assert!(dark.iter().all(|v| *v == 0.0), "en svart bild är tyst");
    }

    #[test]
    fn something_that_is_not_a_png_is_reported_not_panicked() {
        let err = envelope_from_png(b"inte en bild").unwrap_err();
        assert!(err.contains("bilden går inte att läsa"), "{err}");
    }
}
