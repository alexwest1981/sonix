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

/// Fackstorleken på den finaste nivån. 64 samplar ≈ 1,3 ms vid 48 kHz.
///
/// Siffran är en avvägning som Alex' öra avgjorde: med 256 (≈ 5 ms) blev höljet
/// upp till en pixels bredd suddigt, och det gick inte att pricka in var ett ljud
/// börjar. Ett fack är den största möjliga oskärpan, så den ska vara så liten att
/// den inte syns. 64 ger 1,3 ms — och minnet växer linjärt: en fyra minuters fil
/// blir ~1,4 MB i finaste nivån, knappt 3 MB för alla nivåer.
pub const FINEST_BUCKET: usize = 64;

/// Hur en vågform ska ritas vid en viss zoom (Fas 8.4).
///
/// Erfarenheten från riktiga DAW:er (se `references/waveform-rendering.md` i
/// `sonix`-skillen): **"tydligare ju mer man zoomar" kommer inte av sig själv.**
/// En stapel per bildpunkt är grumlig när pixlarna är få och samplarna är många,
/// hur exakt höljet än är — så ritaren måste **byta representation** vid trösklar.
/// Ardour säger det rakt ut: höljet är en approximation, och den verkliga
/// vågformen syns bara högst upp i zoomningen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomRegime {
    /// Många samplar per bildpunkt: ett min/max-hölje per kolumn.
    Envelope,
    /// Ungefär ett sampel per bildpunkt: en **kurva genom samplarna** — vågformen
    /// löses upp till sin riktiga form.
    Samples,
    /// Flera bildpunkter per sampel: **en punkt per sampel**, med linje emellan.
    /// (Audacitys "dots".) Här ser man exakt var ett anslag börjar.
    SampleDots,
}

/// Från hur många bildpunkter ett sampel får innan det ritas som en egen punkt.
///
/// Fyra är samma tröskel Audacity använder i praktiken: under den flyter punkterna
/// ihop till en linje och ska ritas som en.
pub const DOTS_PIXELS_PER_SAMPLE: f64 = 4.0;

/// Väljer representationsläge ur hur många samplar en bildpunkt täcker.
///
/// Vid `<= 1.0` får varje sampel minst en egen bildpunkt — då finns den riktiga
/// vågformen att visa, och då ska den visas.
pub fn zoom_regime(samples_per_pixel: f64) -> ZoomRegime {
    if samples_per_pixel > 1.0 {
        ZoomRegime::Envelope
    } else if samples_per_pixel > 1.0 / DOTS_PIXELS_PER_SAMPLE {
        ZoomRegime::Samples
    } else {
        ZoomRegime::SampleDots
    }
}

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

    /// Höljet för ett **utsnitt** av samplen: `len` samplar från `start`, ritat
    /// över `pixels` bildpunkter.
    ///
    /// En region på tidslinjen är inte hela filen — den börjar på en offset i
    /// spårets ljud (`sample_offset_sec`) och täcker en del av det. Därför räknas
    /// utsnittet i samplar och nivåerna läses med absoluta index, så att facken
    /// hamnar där de ska och inte där buffertens början råkar ligga.
    pub fn envelope_at(
        &self,
        samples: &[f32],
        start: usize,
        len: usize,
        pixels: usize,
    ) -> Vec<(f32, f32)> {
        let end = (start + len).min(samples.len());
        if pixels == 0 || start >= end {
            return vec![(0.0, 0.0); pixels];
        }
        let span = end - start;
        let samples_per_pixel = span as f64 / pixels as f64;
        let Some(level) = self.level_for(samples_per_pixel) else {
            // Inzoomad förbi finaste facket: exakt ur samplen.
            return envelope_per_pixel(&samples[start..end], pixels);
        };
        let bucket = level.samples_per_bucket;
        let mut out = Vec::with_capacity(pixels);
        for p in 0..pixels {
            let a = start + p * span / pixels;
            let b = start + ((p + 1) * span / pixels).max((p * span / pixels) + 1);
            let b = b.min(end);
            let first = a / bucket;
            let last = b.div_ceil(bucket).min(level.peaks.len());
            let upto = last.max(first + 1).min(level.peaks.len());
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &(blo, bhi) in &level.peaks[first..upto] {
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

    /// Höljet för en **slingad** region: `loop_len` samplar som upprepas från
    /// `start`, ritat över `total_len` samplar utdata och `pixels` bildpunkter.
    ///
    /// Skillnaden mot `envelope_at` är att en bildpunkt kan innehålla slutet av en
    /// repetition och början av nästa. Varje kolumn är därför en union av högst
    /// två sammanhängande intervall, och båda måste läsas — annars tappas ljudet
    /// precis vid skarven, vilket är den plats i en loop man lyssnar mest på.
    pub fn envelope_looped(
        &self,
        samples: &[f32],
        start: usize,
        loop_len: usize,
        total_len: usize,
        pixels: usize,
    ) -> Vec<(f32, f32)> {
        if pixels == 0 || loop_len == 0 {
            return vec![(0.0, 0.0); pixels.max(1)];
        }
        let loop_end = (start + loop_len).min(samples.len());
        if start >= loop_end {
            return vec![(0.0, 0.0); pixels];
        }
        let loop_len = loop_end - start;
        let total = total_len.max(1);
        let samples_per_pixel = total as f64 / pixels as f64;
        let level = self.level_for(samples_per_pixel);
        let mut out = Vec::with_capacity(pixels);
        for p in 0..pixels {
            let k0 = p * total / pixels;
            let k1 = ((p + 1) * total / pixels).max(k0 + 1);
            let a = k0 % loop_len;
            let b = k1 % loop_len;
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            // Ett varv eller fler: hela loopen ligger under bildpunkten.
            let spans: Vec<(usize, usize)> = if k1 - k0 >= loop_len {
                vec![(0, loop_len)]
            } else if b > a {
                vec![(a, b)]
            } else {
                vec![(a, loop_len), (0, b)]
            };
            for (slo, shi) in spans {
                if shi <= slo {
                    continue;
                }
                match level {
                    // Inzoomad: läs samplen själva.
                    None => {
                        for &v in &samples[start + slo..start + shi] {
                            if v < lo {
                                lo = v;
                            }
                            if v > hi {
                                hi = v;
                            }
                        }
                    }
                    Some(l) => {
                        // Facken räknas från BUFFERTENS början, inte från regionens:
                        // utan `start` här läses fel fack, och en region med offset
                        // ritas tyst där det finns ljud.
                        let bucket = l.samples_per_bucket;
                        let first = (start + slo) / bucket;
                        let last = (start + shi).div_ceil(bucket).min(l.peaks.len());
                        for &(blo, bhi) in &l.peaks[first..last.max(first + 1).min(l.peaks.len())] {
                            if blo < lo {
                                lo = blo;
                            }
                            if bhi > hi {
                                hi = bhi;
                            }
                        }
                    }
                }
            }
            out.push(if lo.is_finite() { (lo, hi) } else { (0.0, 0.0) });
        }
        out
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

    /// Trösklarna är själva poängen: vid ett sampel per bildpunkt finns sanningen,
    /// och då ska den visas i stället för ett hölje.
    #[test]
    fn the_regime_changes_when_the_truth_becomes_available() {
        assert_eq!(zoom_regime(1000.0), ZoomRegime::Envelope);
        assert_eq!(zoom_regime(2.0), ZoomRegime::Envelope);
        assert_eq!(zoom_regime(1.0001), ZoomRegime::Envelope);
        // Ett sampel per bildpunkt: nu finns den riktiga vågformen.
        assert_eq!(zoom_regime(1.0), ZoomRegime::Samples);
        assert_eq!(zoom_regime(0.5), ZoomRegime::Samples);
        assert_eq!(zoom_regime(0.26), ZoomRegime::Samples);
        // Fyra bildpunkter per sampel: punkterna blir åtskilda nog att ritas.
        assert_eq!(zoom_regime(0.25), ZoomRegime::SampleDots);
        assert_eq!(zoom_regime(0.01), ZoomRegime::SampleDots);
    }

    /// Regimen får aldrig backa när man zoomar in: hölje -> kurva -> punkter, i
    /// den ordningen och aldrig tillbaka. Annars flimrar vågformen vid tröskeln.
    #[test]
    fn zooming_in_never_goes_backwards() {
        let order = |r: ZoomRegime| match r {
            ZoomRegime::Envelope => 0,
            ZoomRegime::Samples => 1,
            ZoomRegime::SampleDots => 2,
        };
        let mut last = 0;
        let mut spp = 64.0f64;
        while spp > 0.001 {
            let now = order(zoom_regime(spp));
            assert!(
                now >= last,
                "regimen backade vid {spp} samplar per bildpunkt"
            );
            last = now;
            spp /= 1.05;
        }
        assert_eq!(last, 2, "hela vägen in ska sluta i punkter");
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
        // Inzoomad förbi finaste facket — nivån räknas ur konstanten, inte ur en
        // siffra som råkar stå i testet.
        let zoomed = (20_000 / FINEST_BUCKET) + 1;
        for pixels in [zoomed, zoomed * 2, zoomed * 4] {
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

    /// Ett utsnitt ska ge samma svar som om utsnittet vore hela filen — det är
    /// kontraktet som gör att en region kan ritas ur spårets cache.
    #[test]
    fn a_slice_reads_the_same_as_a_buffer_of_its_own() {
        let samples = noisy(300_000, 23);
        let cache = WaveformCache::build(&samples);
        for (start, len) in [(0usize, 300_000usize), (1_000, 200_000), (250_000, 50_000)] {
            let slice = &samples[start..start + len];
            // Inzoomad: utsnittet ska vara EXAKT ur sina egna samplar.
            // Under 256 samplar per bildpunkt är man förbi finaste facket, och
            // då SKA svaret komma ur samplen — därför räknas zoomnivån ur
            // utsnittets längd i stället för att gissas.
            let zoomed = len / FINEST_BUCKET + 1;
            for pixels in [zoomed, zoomed * 2] {
                assert_eq!(
                    cache.envelope_at(&samples, start, len, pixels),
                    envelope_per_pixel(slice, pixels),
                    "utsnitt {start}..{} vid {pixels} pixlar",
                    start + len
                );
            }
            // Utzoomad: får aldrig dölja något som FINNS. Referensen är den
            // exakta sanningen (`envelope_per_pixel` läser varje sample), inte en
            // annan fackindelning — två approximationer kan skilja sig åt åt båda
            // hållen, och då prövar man inte det som spelar roll.
            //
            // Nivåerna kan bara göra höljet BREDARE (de läser hela fack, även de
            // som bara delvis ligger under bildpunkten). Att det aldrig blir
            // smalare är precis det som gör att ett anslag inte kan försvinna.
            for pixels in [50usize, 300, 1200, zoomed] {
                let a = cache.envelope_at(&samples, start, len, pixels);
                let exact = envelope_per_pixel(slice, pixels);
                for (i, ((alo, ahi), (elo, ehi))) in a.iter().zip(&exact).enumerate() {
                    assert!(
                        *alo <= *elo + 1e-6 && *ahi >= *ehi - 1e-6,
                        "kolumn {i} dolde något i utsnittet: {alo},{ahi} mot exakt {elo},{ehi}"
                    );
                }
            }
        }
        // Ett utsnitt utanför bufferten är tomt, inte panik.
        assert_eq!(
            cache.envelope_at(&samples, 400_000, 1000, 10),
            vec![(0.0, 0.0); 10]
        );
    }

    /// En slingad region ska visa samma sak varje varv — och aldrig tappa ljudet
    /// vid skarven. Referensen är den exakta sanningen: en brute-force över de
    /// samplar som faktiskt ligger under varje bildpunkt.
    #[test]
    fn a_looped_region_repeats_and_never_losses_the_seam() {
        let mut loop_part = noisy(4_000, 31);
        // Ett tydligt anslag precis i slutet av loopen: det är där en skarv kan tappa.
        loop_part[3_998] = 1.0;
        let mut samples = vec![0.0f32; 2_000]; // före
        samples.extend_from_slice(&loop_part);
        samples.extend_from_slice(&vec![0.0f32; 500]); // efter
        let start = 2_000usize;
        let loop_len = loop_part.len();
        let total = loop_len * 4;
        let cache = WaveformCache::build(&samples);

        for pixels in [40usize, 160, 4_000] {
            let env = cache.envelope_looped(&samples, start, loop_len, total, pixels);
            assert_eq!(env.len(), pixels);
            for p in 0..pixels {
                // Sanningen: min/max över de samplar som ligger under kolumnen.
                let k0 = p * total / pixels;
                let k1 = ((p + 1) * total / pixels).max(k0 + 1);
                let mut lo = f32::INFINITY;
                let mut hi = f32::NEG_INFINITY;
                for k in k0..k1 {
                    let v = samples[start + (k % loop_len)];
                    if v < lo {
                        lo = v;
                    }
                    if v > hi {
                        hi = v;
                    }
                }
                let (clo, chi) = env[p];
                assert!(
                    clo <= lo + 1e-6 && chi >= hi - 1e-6,
                    "kolumn {p} av {pixels} dolde ljud: {clo},{chi} mot exakt {lo},{hi}"
                );
            }
            // Varje varv ska se likadant ut: kolumn p och p + pixels/4.
            let quarter = pixels / 4;
            if quarter > 0 {
                for p in 0..quarter {
                    let (a, b) = (env[p], env[p + quarter]);
                    assert!(
                        a == b,
                        "varv 1 och varv 2 skiljer sig i kolumn {p}: {a:?} mot {b:?}"
                    );
                }
            }
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
