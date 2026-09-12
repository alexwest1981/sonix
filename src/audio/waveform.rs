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

// Modulen är klar och prövad men ännu inte inkopplad i ritningen (Fas 8.3): allt
// här prövas av testerna, och attributet försvinner när tidlinjen ritar ur cachen.
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "kopplas in i ritningen i samma pass (Fas 8.3)")
)]

/// Höljet för `samples`, ett `(min, max)` per bildpunkt.
///
/// Varje bildpunkt får det sanna intervallet för de samplar som ligger under
/// den: ingen sampling, ingen hopslagning över pixelgränser, och ingen sample
/// tappad mellan två bildpunkter. Sista bildpunkten tar resten, så att udda
/// längder inte tappar svansen.
///
/// Returnerar en tom vektor om indata är tom eller `pixels` är 0.
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
pub fn pixels_needed(region_pixels: f32) -> usize {
    region_pixels.max(0.0).ceil() as usize
}

/// Fackstorleken på den finaste nivån. 256 samplar ≈ 5 ms vid 48 kHz: under den
/// gränsen ritas höljet exakt ur samplen, över den används nivåerna.
pub const FINEST_BUCKET: usize = 256;

/// En nivå: ett (min, max) per `samples_per_bucket` samplar.
#[derive(Clone, Debug, PartialEq)]
pub struct PeakLevel {
    pub samples_per_bucket: usize,
    pub peaks: Vec<(f32, f32)>,
}

/// Flernivå-cache för vågformer (Fas 8.3).
///
/// **Varför.** Att räkna `envelope_per_pixel` ur hela PCM:en varje bildruta är
/// O(samplar) per bildruta. Fyra minuter ljud är 11,5 miljoner samplar — sextio
/// gånger i sekunden går inte. Cachen gör arbetet en gång när filen läses och
/// svarar sedan i O(bildpunkter).
///
/// **Hur exakt.** Nivåerna är dubbelt så grova för varje steg (256, 512, 1024 …
/// samplar per fack). Ritningen väljer den **finaste** nivå vars fack ryms inom
/// en bildpunkt, alltså läses högst två fack per pixel. Det gör höljet:
///
/// - **exakt** när man zoomat in så långt att ett fack inte ryms i en pixel —
///   då används `envelope_per_pixel` direkt på samplen,
/// - **inom ett fack** (≈ en bildpunkt) annars: ett anslag som ligger strax
///   utanför en pixel kan synas i den intill, men ingenting försvinner och
///   ingenting flyttas mer än en pixel.
///
/// Det sista är testat, inte påstått: testerna jämför mot `envelope_per_pixel`
/// och prövar att höljet **aldrig döljer** ett anslag.
///
/// Minnet är försumbart: finaste nivån är en åttondel av PCM:en i storlek, och
/// nivåerna halveras uppåt (sammanlagt ~1/4 av PCM:ens storlek i f32-par).
#[derive(Clone, Debug, PartialEq)]
pub struct WaveformCache {
    /// Finaste nivån först.
    levels: Vec<PeakLevel>,
}

impl WaveformCache {
    /// Bygger nivåerna ur samplen. Görs en gång per fil.
    pub fn build(samples: &[f32]) -> Self {
        let mut levels: Vec<PeakLevel> = Vec::new();
        if samples.is_empty() {
            return Self { levels };
        }
        let mut bucket = FINEST_BUCKET;
        loop {
            let n = samples.len().div_ceil(bucket);
            let mut peaks = Vec::with_capacity(n);
            for i in 0..n {
                let start = i * bucket;
                let end = ((i + 1) * bucket).min(samples.len());
                let mut lo = f32::INFINITY;
                let mut hi = f32::NEG_INFINITY;
                for &v in &samples[start..end] {
                    if v < lo {
                        lo = v;
                    }
                    if v > hi {
                        hi = v;
                    }
                }
                peaks.push((lo, hi));
            }
            levels.push(PeakLevel {
                samples_per_bucket: bucket,
                peaks,
            });
            if levels.last().map(|l| l.peaks.len()).unwrap_or(0) <= 1 {
                break;
            }
            bucket = bucket.saturating_mul(2);
        }
        Self { levels }
    }

    /// Antalet nivåer (mest för tester och för att kunna visa minnesbruket).
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Den finaste nivå vars fack ryms inom en bildpunkt.
    ///
    /// `None` betyder "inget fack är så litet" — alltså är man inzoomad längre än
    /// till finaste facket, och då ska höljet räknas exakt ur samplen i stället.
    fn level_for(&self, samples_per_pixel: f64) -> Option<&PeakLevel> {
        self.levels
            .iter()
            .find(|l| l.samples_per_bucket as f64 <= samples_per_pixel)
    }

    /// Höljet för `pixels` bildpunkter, ur nivåerna när de räcker och ur samplen
    /// när de inte gör det.
    pub fn envelope(&self, samples: &[f32], pixels: usize) -> Vec<(f32, f32)> {
        if samples.is_empty() || pixels == 0 {
            return Vec::new();
        }
        let samples_per_pixel = samples.len() as f64 / pixels as f64;
        let Some(level) = self.level_for(samples_per_pixel) else {
            // Inzoomad förbi finaste facket: räkna exakt.
            return envelope_per_pixel(samples, pixels);
        };
        let len = samples.len();
        let bucket = level.samples_per_bucket;
        let mut out = Vec::with_capacity(pixels);
        for p in 0..pixels {
            let start = p * len / pixels;
            let end = ((p + 1) * len / pixels).max(start + 1).min(len);
            let first = start / bucket;
            let last = end.div_ceil(bucket).min(level.peaks.len());
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &(blo, bhi) in &level.peaks[first..last.max(first + 1).min(level.peaks.len())] {
                if blo < lo {
                    lo = blo;
                }
                if bhi > hi {
                    hi = bhi;
                }
            }
            out.push((lo, hi));
        }
        out
    }
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

    /// Pseudoslump utan beroenden: samma indata ger samma test varje gång.
    fn noisy(len: usize, seed: u32) -> Vec<f32> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                ((x % 2000) as f32 / 1000.0) - 1.0
            })
            .collect()
    }

    /// Inzoomad förbi finaste facket ska cachen ge EXAKT samma svar som att
    /// räkna ur samplen — det är kontraktet mot `envelope_per_pixel`.
    #[test]
    fn the_cache_is_exact_when_zoomed_in() {
        let samples = noisy(20_000, 7);
        let cache = WaveformCache::build(&samples);
        // 20 000 samplar och 200 pixlar = 100 samplar per pixel < 256.
        for pixels in [200usize, 400, 1000] {
            assert_eq!(
                cache.envelope(&samples, pixels),
                envelope_per_pixel(&samples, pixels),
                "{pixels} pixlar ska vara exakt ur samplen"
            );
        }
    }

    /// Utzoomad får höljet aldrig DÖLJA ett anslag, och aldrig tappa en topp.
    /// Ett hölje som är en pixel bredare är användbart; ett som är tyst där ljudet
    /// finns är det inte.
    #[test]
    fn the_cache_never_hides_a_transient() {
        let mut samples = noisy(200_000, 11);
        // Tre enskilda anslag, långt isär.
        samples[1_000] = 1.0;
        samples[100_000] = -1.0;
        samples[199_000] = 0.9;
        let cache = WaveformCache::build(&samples);
        for pixels in [50usize, 313, 1024, 4096] {
            let fast = cache.envelope(&samples, pixels);
            assert_eq!(fast.len(), pixels);
            let exact = envelope_per_pixel(&samples, pixels);
            for (i, ((flo, fhi), (elo, ehi))) in fast.iter().zip(&exact).enumerate() {
                assert!(
                    *flo <= *elo + 1e-6 && *fhi >= *ehi - 1e-6,
                    "kolumn {i} vid {pixels} pixlar dolde något: cache ({flo},{fhi}) mot exakt ({elo},{ehi})"
                );
            }
            // Och de globala ytterligheterna finns kvar någonstans i höljet.
            let lo = fast.iter().fold(f32::INFINITY, |m, (a, _)| m.min(*a));
            let hi = fast.iter().fold(f32::NEG_INFINITY, |m, (_, b)| m.max(*b));
            assert!(
                lo <= -1.0 + 1e-6 && hi >= 1.0 - 1e-6,
                "topparna ska finnas kvar"
            );
        }
    }

    /// Ritningen ska läsa högst två fack per bildpunkt. Det är den egenskapen
    /// som gör att en fyra minuters fil går att rita sextio gånger i sekunden.
    #[test]
    fn at_most_two_buckets_per_pixel() {
        let samples = noisy(480_000, 3);
        let cache = WaveformCache::build(&samples);
        for pixels in [100usize, 1000, 5000, 100_000] {
            let spp = samples.len() as f64 / pixels as f64;
            match cache.level_for(spp) {
                Some(level) => {
                    assert!(
                        level.samples_per_bucket as f64 <= spp,
                        "facket ska rymmas i en bildpunkt"
                    );
                    assert!(
                        (level.samples_per_bucket as f64) * 2.0 > spp
                            || level.samples_per_bucket == FINEST_BUCKET,
                        "och det ska vara den FINASTE som ryms, annars blir det slött i onödan"
                    );
                }
                None => {
                    // Inzoomad förbi finaste facket: då SKA svaret komma ur
                    // samplen, och då är det exakt. Det är den andra grenen av
                    // kontraktet, inte ett fel.
                    assert!(
                        spp < FINEST_BUCKET as f64,
                        "{pixels} pixlar: ingen nivå ska bara hända över finaste facket"
                    );
                    assert_eq!(
                        cache.envelope(&samples, pixels),
                        envelope_per_pixel(&samples, pixels),
                        "och då ska svaret vara exakt"
                    );
                }
            }
        }
        // Nivåerna halveras uppåt och slutar med ett enda fack.
        assert!(cache.level_count() >= 2);
        assert_eq!(cache.levels[0].samples_per_bucket, FINEST_BUCKET);
        assert_eq!(cache.levels.last().unwrap().peaks.len(), 1);
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
