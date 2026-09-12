//! Tempokarta (Fas 8.2, steg 1).
//!
//! I dag har Sonix **ett** tempo: varje omräkning mellan takter och sekunder
//! multiplicerar med samma tal. En tempokarta gör tiden *styckvis linjär* — men
//! då måste varje omräkning som inte konsulterar kartan bli tyst fel, och det är
//! den sämsta sortens fel (det hörs först när något låter fel).
//!
//! Därför är den här modulen **steg 1**: typen, och beviset att **ett enda tempo
//! ger exakt samma tal som i dag** — inte "ungefär", utan samma f32, för både
//! takter och steg. Först när det beviset håller får något annat än klockan byta
//! till kartan.
//!
//! **Omfång i det här steget:** tempobyten sker på **taktgränser**. Taktart (3/4,
//! 6/8 …) är en egen fråga och ingår inte — den rör fler ställen än tempot.

use serde::{Deserialize, Serialize};

/// Steg per takt: 16-delssteg i 4/4 — samma rutnät som appens pattern.
pub const STEPS_PER_BAR: usize = 16;

/// Ett tempobyte: från och med `start_bar` gäller `bpm`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub start_bar: u32,
    pub bpm: f32,
}

/// Tempot som funktion av taktnummer.
///
/// Normaliserad när den skapas: sorterad, en punkt per takt (den sista vinner),
/// och **alltid minst en punkt som börjar på takt 0**. Det gör att
/// `bpm_at`/`secs_at_bar` alltid har ett svar och att en tom karta inte kan
/// uppstå av misstag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoMap {
    points: Vec<TempoPoint>,
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::single(120.0)
    }
}

impl TempoMap {
    /// Kartan för ett enda tempo — den form appen har i dag.
    pub fn single(bpm: f32) -> Self {
        Self {
            points: vec![TempoPoint { start_bar: 0, bpm }],
        }
    }

    /// Bygger en karta och normaliserar den.
    ///
    /// Används av testerna i dag, och av UI:t när tempobyten får en egen vy
    /// (steg 3) — därför står den kvar även om inget anropar den än.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "väntar på UI:t för tempobyten (8.2 steg 3)")
    )]
    pub fn from_points(mut points: Vec<TempoPoint>) -> Self {
        points.sort_by_key(|p| p.start_bar);
        // En punkt per takt, och den SISTA vinner: två svar på samma takt vore
        // ett svar för mycket, och den sist tillagda är den avsikt som gäller.
        // (Observera att `dedup_by_key` behåller den *första* — därför den här
        // loopen i stället.)
        let mut unique: Vec<TempoPoint> = Vec::with_capacity(points.len());
        for p in points {
            match unique.last_mut() {
                Some(prev) if prev.start_bar == p.start_bar => *prev = p,
                _ => unique.push(p),
            }
        }
        if unique.is_empty() {
            // En tom karta kan inte uppstå: den får ett begripligt standardtempo.
            // (Annars skulle `bpm_at` inte ha något svar alls.)
            unique.push(TempoPoint {
                start_bar: 0,
                bpm: 120.0,
            });
        } else if unique[0].start_bar != 0 {
            // Ingen punkt i takt 0: den första flyttas dit i stället för att
            // takten före den första punkten blir utan svar. Ingen dubblett kan
            // uppstå, eftersom en punkt i takt 0 hade varit först i ordningen.
            unique[0].start_bar = 0;
        }
        Self { points: unique }
    }

    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "väntar på UI:t för tempobyten (8.2 steg 3)")
    )]
    pub fn points(&self) -> &[TempoPoint] {
        &self.points
    }

    /// Sant när kartan beskriver ett enda tempo — då gäller dagens matematik
    /// exakt, och det är den vägen som prövas i kompatibilitetstestet.
    pub fn is_single(&self) -> bool {
        self.points.len() == 1
    }

    /// Tempot som gäller i takten `bar`.
    pub fn bpm_at(&self, bar: f64) -> f32 {
        let mut bpm = self.points[0].bpm;
        for p in &self.points {
            if (p.start_bar as f64) <= bar {
                bpm = p.bpm;
            } else {
                break;
            }
        }
        bpm
    }

    /// Sekunder per takt i takten `bar`.
    ///
    /// Med ett enda tempo **exakt** dagens uttryck, `(60 / bpm) * 4`, räknat i
    /// f32 — så att alla som jämför före och efter får samma tal, bit för bit.
    /// Med flera punkter räknas i f64, där förlusten hade varit större.
    pub fn secs_per_bar_at(&self, bar: f64) -> f64 {
        if self.is_single() {
            return (((60.0 / self.points[0].bpm) * 4.0) as f64).max(0.0);
        }
        ((60.0f64 / self.bpm_at(bar) as f64) * 4.0).max(0.0)
    }

    /// Sekunder per taktslag (fjärdedelsnot) i takten `bar`.
    ///
    /// Samma uttryck som appen använder för svepets svans och för SMF, så att
    /// även det är bitvis oförändrat med ett tempo.
    pub fn secs_per_beat_at(&self, bar: f64) -> f64 {
        if self.is_single() {
            return ((60.0 / self.points[0].bpm) as f64).max(0.0);
        }
        (60.0f64 / self.bpm_at(bar) as f64).max(0.0)
    }

    /// Sekunder per 16-delssteg i takten `bar`.
    ///
    /// Klockan i appen räknar `(60 / bpm) / 4`, och enpunktsfallet gör samma sak
    /// — inte `sekunder per takt / 16`, som ger ett annat tal i sista biten.
    pub fn secs_per_step_at(&self, bar: f64) -> f64 {
        if self.is_single() {
            return (((60.0 / self.points[0].bpm) / 4.0) as f64).max(0.0);
        }
        ((60.0f64 / self.bpm_at(bar) as f64) / 4.0).max(0.0)
    }

    /// Sekunden där takten `bar` börjar, räknat från takt 0.
    pub fn secs_at_bar(&self, bar: f64) -> f64 {
        if bar <= 0.0 {
            return 0.0;
        }
        if self.is_single() {
            // Samma väg som i dag: takt gånger sekunder-per-takt.
            return (bar * self.secs_per_bar_at(bar)).max(0.0);
        }
        let mut secs = 0.0f64;
        for (i, p) in self.points.iter().enumerate() {
            let start = p.start_bar as f64;
            if start >= bar {
                break;
            }
            let end = self
                .points
                .get(i + 1)
                .map(|n| n.start_bar as f64)
                .unwrap_or(bar)
                .min(bar);
            if end > start {
                secs += (end - start) * ((60.0f64 / p.bpm as f64) * 4.0);
            }
        }
        secs.max(0.0)
    }

    /// Längden i sekunder av `bars` takter som börjar i takten `from_bar`.
    ///
    /// Med ett enda tempo **exakt** `bars * sekunder-per-takt`, räknat som i dag
    /// — inte en differens av två positioner, som kan skilja i sista biten. Med
    /// flera punkter summeras varje tempoavsnitt för sig, så att en kloss som
    /// sträcker sig över ett tempobyte får sin rätta längd i stället för att
    /// mätas med tempot som råkade gälla där den började.
    pub fn secs_for_bars_at(&self, from_bar: f64, bars: f64) -> f64 {
        if bars <= 0.0 {
            return 0.0;
        }
        if self.is_single() {
            return (((60.0 / self.points[0].bpm) * 4.0) as f64 * bars).max(0.0);
        }
        self.secs_at_bar(from_bar + bars) - self.secs_at_bar(from_bar)
    }

    /// Takten som innehåller sekunden `secs` — kartans invers.
    ///
    /// Inversen är **inte** exakt i flyttal (den är en division), så den som
    /// behöver ett exakt taktnummer ska hålla reda på takten i stället för att
    /// räkna tillbaka den.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "behövs när importerade filers längd räknas om till takter (8.2 steg 3)"
        )
    )]
    pub fn bar_at_secs(&self, secs: f64) -> f64 {
        if secs <= 0.0 {
            return 0.0;
        }
        let mut acc = 0.0f64;
        for (i, p) in self.points.iter().enumerate() {
            let spb = (60.0f64 / p.bpm as f64) * 4.0;
            let start = p.start_bar as f64;
            let next = self.points.get(i + 1).map(|n| n.start_bar as f64);
            let span = next.map(|n| n - start);
            match span {
                Some(d) if acc + d * spb <= secs => acc += d * spb,
                _ => return start + (secs - acc) / spb.max(f64::MIN_POSITIVE),
            }
        }
        // Bortom sista punkten gäller sista tempot.
        let last = self.points.last().expect("minst en punkt");
        let spb = (60.0f64 / last.bpm as f64) * 4.0;
        last.start_bar as f64 + (secs - acc) / spb.max(f64::MIN_POSITIVE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Beviset för hela steget: med ett enda tempo är kartan **bitvis** samma
    /// matematik som appen använder i dag — för både takter och steg.
    #[test]
    fn one_tempo_reproduces_todays_math_bit_for_bit() {
        for bpm in [40.0f32, 90.0, 120.0, 126.0, 174.0, 260.0] {
            let map = TempoMap::single(bpm);
            let legacy_bar = ((60.0 / bpm) * 4.0) as f64;
            let legacy_step = ((60.0 / bpm) / 4.0) as f64;
            for bar in [0.0f64, 1.0, 3.5, 32.0, 128.0] {
                assert_eq!(
                    map.secs_per_bar_at(bar),
                    legacy_bar,
                    "takt {bar} vid {bpm} BPM ska vara exakt som i dag"
                );
                assert_eq!(
                    map.secs_per_step_at(bar),
                    legacy_step,
                    "steget vid {bar} och {bpm} BPM ska vara exakt som i dag"
                );
            }
            // Och takt→sekunder är fortfarande en multiplikation.
            for bar in [0.0f64, 1.0, 2.0, 7.0, 32.0] {
                assert_eq!(
                    map.secs_at_bar(bar),
                    (bar * legacy_bar) as f64,
                    "sekunden för takt {bar} vid {bpm} BPM"
                );
            }
        }
    }

    #[test]
    fn two_tempos_are_piecewise_linear() {
        // 120 BPM i takt 0–3, 60 BPM från takt 4.
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 120.0,
            },
            TempoPoint {
                start_bar: 4,
                bpm: 60.0,
            },
        ]);
        let two = (60.0f64 / 120.0) * 4.0; // 2 s per takt
        let one = (60.0f64 / 60.0) * 4.0; // 4 s per takt

        assert_eq!(map.bpm_at(0.0), 120.0);
        assert_eq!(map.bpm_at(3.99), 120.0);
        assert_eq!(map.bpm_at(4.0), 60.0, "bytet gäller från och med takten");
        assert_eq!(map.bpm_at(99.0), 60.0);

        assert!((map.secs_at_bar(4.0) - 4.0 * two).abs() < 1e-9);
        assert!(
            (map.secs_at_bar(8.0) - (4.0 * two + 4.0 * one)).abs() < 1e-9,
            "fyra takter i vardera tempot"
        );
        assert!((map.secs_at_bar(5.0) - (4.0 * two + one)).abs() < 1e-9);
    }

    /// Längden ska vara exakt dagens multiplikation med ett tempo, och rätt
    /// summerad över ett tempobyte med flera.
    #[test]
    fn a_length_is_exact_with_one_tempo_and_piecewise_with_several() {
        for bpm in [40.0f32, 120.0, 174.0] {
            let map = TempoMap::single(bpm);
            let legacy = ((60.0 / bpm) * 4.0) as f64;
            for (from, bars) in [(0.0f64, 1.0f64), (3.0, 4.0), (0.5, 0.25), (12.0, 3.5)] {
                assert_eq!(
                    map.secs_for_bars_at(from, bars),
                    legacy * bars,
                    "{bars} takter från {from} vid {bpm} BPM ska vara exakt som i dag"
                );
            }
            assert_eq!(map.secs_for_bars_at(0.0, 0.0), 0.0);
            assert_eq!(
                map.secs_for_bars_at(5.0, -1.0),
                0.0,
                "negativ längd ger noll"
            );
        }

        // Tempobyte mitt i klossen: 120 BPM i takt 0–3, 60 BPM därefter.
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 120.0,
            },
            TempoPoint {
                start_bar: 4,
                bpm: 60.0,
            },
        ]);
        let two = (60.0f64 / 120.0) * 4.0;
        let one = (60.0f64 / 60.0) * 4.0;
        // Från takt 2, fyra takter lång: 2 takter i 120 + 2 i 60.
        assert!((map.secs_for_bars_at(2.0, 4.0) - (2.0 * two + 2.0 * one)).abs() < 1e-9);
        // Helt inom ett tempoavsnitt: exakt.
        assert_eq!(map.secs_for_bars_at(4.0, 2.0), one * 2.0);
    }

    #[test]
    fn the_inverse_lands_back_on_the_same_bar() {
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 90.0,
            },
            TempoPoint {
                start_bar: 8,
                bpm: 160.0,
            },
            TempoPoint {
                start_bar: 16,
                bpm: 60.0,
            },
        ]);
        for bar in [0.0f64, 1.0, 7.5, 8.0, 12.0, 16.0, 24.0, 40.0] {
            let secs = map.secs_at_bar(bar);
            let back = map.bar_at_secs(secs);
            assert!(
                (back - bar).abs() < 1e-6,
                "takt {bar} -> {secs} s -> {back} (ska vara samma takt)"
            );
        }
    }

    #[test]
    fn the_map_normalises_whatever_it_is_given() {
        // Osorterad, med dubblett och utan punkt i takt 0.
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 7,
                bpm: 100.0,
            },
            TempoPoint {
                start_bar: 4,
                bpm: 140.0,
            },
            TempoPoint {
                start_bar: 4,
                bpm: 130.0,
            },
            TempoPoint {
                start_bar: 1,
                bpm: 150.0,
            },
        ]);
        let bars: Vec<u32> = map.points().iter().map(|p| p.start_bar).collect();
        assert_eq!(bars, vec![0, 4, 7], "sorterad, en per takt");
        assert_eq!(
            map.points()[0].bpm,
            150.0,
            "den första punkten flyttas till takt 0"
        );
        assert_eq!(
            map.points()[1].bpm,
            130.0,
            "vid dubblett i samma takt vinner den sista i ordningen"
        );

        // En tom karta kan inte uppstå: den får ett begripligt standardtempo.
        let empty = TempoMap::from_points(Vec::new());
        assert!(empty.is_single());
        assert_eq!(empty.bpm_at(0.0), 120.0);
    }

    #[test]
    fn a_single_map_is_recognised_as_single() {
        assert!(TempoMap::single(140.0).is_single());
        assert!(TempoMap::from_points(vec![TempoPoint {
            start_bar: 0,
            bpm: 99.0
        }])
        .is_single());
        assert!(!TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 99.0
            },
            TempoPoint {
                start_bar: 2,
                bpm: 100.0
            },
        ])
        .is_single());
    }

    #[test]
    fn a_tempo_change_at_bar_zero_absorbs_the_old_first_point() {
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 110.0,
            },
            TempoPoint {
                start_bar: 0,
                bpm: 128.0,
            },
        ]);
        assert!(map.is_single());
        assert_eq!(map.bpm_at(0.0), 128.0);
    }

    #[test]
    fn a_map_survives_a_round_trip_through_json() {
        let map = TempoMap::from_points(vec![
            TempoPoint {
                start_bar: 0,
                bpm: 128.0,
            },
            TempoPoint {
                start_bar: 16,
                bpm: 96.0,
            },
        ]);
        let json = serde_json::to_string(&map).unwrap();
        let back: TempoMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back, map);
        // Och en karta som saknas i en äldre fil får standardtempot.
        let default: TempoMap = TempoMap::default();
        assert!(default.is_single());
        assert_eq!(default.bpm_at(0.0), 120.0);
    }
}
