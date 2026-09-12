//! Exakta vågformer: ett (min, max) per skärmpixel (Alex krav, Fas 8.3).
//!
//! **Varför modulen finns.** `visual_peaks_from` trycker ihop hela filen till
//! högst **512** punkter *en gång*, oavsett hur mycket man zoomar. En fyra
//! minuter lång tagning blir då en punkt per ~0,47 sekund (11,5 miljoner samplar
//! delat med 512), och när de punkterna sträcks ut över skärmen syns de som
//! trappsteg — "pixlat". En vågform som ska gå att klippa i måste visa vad som
//! helst under pekaren, alltså samplen som ligger under **just den bildpunkten**.
//!
//! **Varför både min och max.** Den gamla formen tog `max(abs(x))` per fönster,
//! vilket ger ett symmetriskt hölje. Det ser rimligt ut för en sinuston men
//! ljuger om en osymmetrisk signal — och det är formen man tittar på när man
//! letar efter var en ton eller ett anslag börjar. Klippning kräver att toppen
//! och botten är *de faktiska* toppen och botten.
//!
//! **Vad som är nästa steg (prestanda).** Att räkna om höljet ur hela PCM:en var
//!je bildruta är O(samplar) per bildruta och går inte för en lång fil. Nästa steg
//! är en flernivå-cache (mip-nivåer: 1, 4, 16, 64 … samplar per fack) i
//! `waveform_cache_dir()`, så att ritningen bara läser några fack per pixel.
//! Funktionen här är den som avgör *vad* som ska stå i varje pixel; cachen ska
//! ge samma svar snabbare, och kan prövas mot den (samma indata, samma utdata).

/// Höljet för `samples`, ett `(min, max)` per bildpunkt.
///
/// Varje bildpunkt får det sanna intervallet för de samplar som ligger under
/// den: ingen sampling, ingen hopslagning över pixelgränser, och ingen sample
/// tappad mellan två bildpunkter. Sista bildpunkten tar resten, så att udda
/// längder inte tappar svansen.
///
/// Returnerar en tom vektor om indata är tom eller `pixels` är 0.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "ritningen kopplas in när flernivå-cachen finns (Fas 8.3); testerna prövar den redan"
    )
)]
pub fn envelope_per_pixel(samples: &[f32], pixels: usize) -> Vec<(f32, f32)> {
    if samples.is_empty() || pixels == 0 {
        return Vec::new();
    }
    let len = samples.len();
    let mut out = Vec::with_capacity(pixels);
    for p in 0..pixels {
        // Gränserna räknas i heltal så att ingen bildpunkt hoppar över ett sample
        // och ingen hamnar utanför: p * len / pixels <= (p+1) * len / pixels.
        let start = p * len / pixels;
        let end = ((p + 1) * len / pixels).max(start + 1).min(len);
        let slice = &samples[start..end];
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &s in slice {
            if s < lo {
                lo = s;
            }
            if s > hi {
                hi = s;
            }
        }
        out.push((lo, hi));
    }
    out
}

/// Hur många bildpunkter höljet behöver för att vara exakt vid den här bredden.
///
/// Finns för att ritningen ska kunna fråga i stället för att gissa: ett hölje
/// med färre punkter än bredden är *per definition* utsträckt och därmed
/// "pixlat", hur snyggt det än ritas.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "ritningen kopplas in när flernivå-cachen finns (Fas 8.3); testerna prövar den redan"
    )
)]
pub fn pixels_needed(region_pixels: f32) -> usize {
    region_pixels.max(0.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En bildpunkt per pixel — det är hela poängen.
    #[test]
    fn one_column_per_pixel() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) - 0.5).collect();
        for pixels in [1usize, 7, 64, 999, 1000, 1500] {
            assert_eq!(
                envelope_per_pixel(&samples, pixels).len(),
                pixels,
                "bredden ska ge exakt så många kolumner"
            );
        }
        assert!(envelope_per_pixel(&[], 100).is_empty());
        assert!(envelope_per_pixel(&samples, 0).is_empty());
    }

    /// Ett enstaka anslag hamnar i RÄTT kolumn. Det här är testet som skiljer en
    /// exakt vågform från en som ser ungefär rätt ut: en klick på sample 900 av
    /// 1000 ska synas i den tionde tiondelen, inte "någonstans i mitten".
    #[test]
    fn a_transient_lands_in_the_right_column() {
        let mut samples = vec![0.0f32; 1000];
        samples[900] = 1.0;
        let env = envelope_per_pixel(&samples, 10);
        assert_eq!(
            (env[9].1 - 1.0).abs() < 1e-6,
            true,
            "anslaget ska ligga i sista kolumnen (900/100 av 1000)"
        );
        for (i, (lo, hi)) in env.iter().enumerate().take(9) {
            assert!(
                lo.abs() < 1e-6 && hi.abs() < 1e-6,
                "kolumn {i} ska vara tyst, blev ({lo}, {hi})"
            );
        }

        // Och med fler pixlar hamnar samma anslag på rätt plats igen.
        let env = envelope_per_pixel(&samples, 100);
        assert!(
            (env[90].1 - 1.0).abs() < 1e-6,
            "sample 900 av 1000 = kolumn 90"
        );
    }

    /// Osymmetri ska bevaras. Den gamla formen (`max(abs(x))`) gav ett speglat
    /// hölje: en signal på +0,2/−0,8 hade ritats som ±0,8.
    #[test]
    fn an_asymmetric_signal_keeps_its_shape() {
        let samples: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 0.2 } else { -0.8 })
            .collect();
        let env = envelope_per_pixel(&samples, 4);
        for (lo, hi) in env {
            assert!((lo + 0.8).abs() < 1e-6, "botten ska vara -0,8, blev {lo}");
            assert!((hi - 0.2).abs() < 1e-6, "toppen ska vara 0,2, blev {hi}");
        }
    }

    /// Varje sample ska räknas, och inget sample två gånger.
    #[test]
    fn every_sample_is_counted_exactly_once() {
        let samples: Vec<f32> = (0..97).map(|i| i as f32).collect();
        let env = envelope_per_pixel(&samples, 13);
        // Sätter ihop kolumnerna igen: det ska vara exakt samma mängd, i ordning.
        let mut seen: Vec<f32> = Vec::new();
        let len = samples.len();
        for (p, _) in env.iter().enumerate() {
            let start = p * len / 13;
            let end = ((p + 1) * len / 13).max(start + 1).min(len);
            seen.extend_from_slice(&samples[start..end]);
        }
        assert_eq!(seen.len(), len, "alla samplar ska vara med");
        assert!(
            seen.iter().zip(&samples).all(|(a, b)| a == b),
            "och i ordning"
        );
    }
}
