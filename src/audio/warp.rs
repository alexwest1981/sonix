//! Warp-lägen — fyra sätt att sträcka, med olika kompromiss mellan skärpa och utsmetning.
//!
//! Ableton har Beats/Tones/Texture/Complex. **Vi använder inte de namnen**, därför att
//! mätningen inte stöder dem: Abletons "Beats" har transientdetektering (kornet hoppas
//! över/förlängs vid anslaget), och vår WSOLA har ingen sådan. Uppmätt på en klickrad
//! sträckt 2× ger vårt *kortaste* korn **sämst** transienter av alla lägen — att kalla det
//! "Beats" vore ett löfte vi inte håller. Lägena heter därför vad de gör:
//!
//! | Läge | Korn | Sökvidd | Topp | Crest | Förhandsläckage | Ton-RMS |
//! | :--- | ---: | ---: | ---: | ---: | ---: | ---: |
//! | Fast | 20 ms | 12 ms | 0,413 | 54,8 | 1,647 | 0,306 |
//! | Tone | 60 ms | 6 ms | 0,766 | 67,8 | 0,000 | 0,349 |
//! | Smooth | 120 ms | 8 ms | 0,766 | 67,8 | 0,000 | 0,342 |
//! | Complex | 40 ms | 10 ms | **0,900** | 64,7 | 0,000 | **0,351** |
//! | Transient | 40 ms | 10 ms | **0,900** | **82,5** | 0,000 | 0,351 |
//!
//! (Klick var 250 ms, 40 sampel; 220 Hz-ton; 48 kHz. Se
//! `stretch::tests::the_warp_modes_differ_in_measured_smearing`.)
//!
//! **Transientläget** gör något annat än de fyra kornbaserade: det delar filen vid de
//! anslag detektorn hittar, **kopierar attacken orörd** (5 ms) och sträcker bara kroppen
//! mellan anslagen, med 1 ms övergång in i den sträckta delen. Mätt: toppen blir exakt
//! källans (0,900 mot 0,900) och varje attack finns **ordagrant** i resultatet, vilket
//! inget kornbaserat läge kan lova. Att i stället styra kornen mot anslagen mättes och
//! förkastades: topp 0,450 och förhandsläckage 1,89, eftersom en transient som ligger i
//! fyra överlappande korn delas upp hur kornet än placeras.
//!
//! `Complex` är bäst på både topp och ton och är därför standard — samma väg som
//! `stretch_stereo` alltid har tagit. `Fast` är den billiga: kortast korn, bredast sök,
//! och de tydligaste artefakterna.

/// Ett sträckläge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WarpMode {
    /// Kortast korn: snabbast att rendera, men tappar mest topp och tonhöjd.
    Fast,
    /// Långa korn: håller tonhöjd och övertoner, lite utsmetning.
    Tone,
    /// Längst korn: minst förhandsläckage — mjukast, men tappar topp.
    Smooth,
    /// 40 ms med brett sökfönster: bäst på både topp och ton. Standard.
    Complex,
    /// Anslagen **kopieras orörda** och bara kropparna sträcks. Enda läget där attacken
    /// med säkerhet är källans egen (mätt: topp 0,900 = källan, varje attack ordagrann).
    /// Kornparametrarna nedan gäller **kropparna** mellan anslagen, inte attacken.
    Transient,
}

impl Default for WarpMode {
    fn default() -> Self {
        WarpMode::Complex
    }
}

impl WarpMode {
    pub const ALL: [WarpMode; 5] = [
        WarpMode::Fast,
        WarpMode::Tone,
        WarpMode::Smooth,
        WarpMode::Complex,
        WarpMode::Transient,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WarpMode::Fast => "Fast",
            WarpMode::Tone => "Tone",
            WarpMode::Smooth => "Smooth",
            WarpMode::Complex => "Complex",
            WarpMode::Transient => "Transient",
        }
    }

    /// Kornets längd i millisekunder.
    pub fn frame_ms(self) -> f32 {
        match self {
            WarpMode::Fast => 20.0,
            WarpMode::Tone => 60.0,
            WarpMode::Smooth => 120.0,
            WarpMode::Complex => 40.0,
            WarpMode::Transient => 40.0,
        }
    }

    /// Sökvidden i millisekunder: hur långt kornet får flyttas för att hitta en lika fas.
    pub fn search_ms(self) -> f32 {
        match self {
            WarpMode::Fast => 12.0,
            WarpMode::Tone => 6.0,
            WarpMode::Smooth => 8.0,
            WarpMode::Complex => 10.0,
            WarpMode::Transient => 10.0,
        }
    }

    /// `(korn, hopp, sökvidd)` i sampel, för [`crate::audio::vocal_harmonizer::Wsola`].
    ///
    /// Hoppet är en fjärdedel av kornet (75 % överlapp) — samma för alla lägen, så det är
    /// kornets längd som skiljer och inte överlappningen.
    pub fn wsola_params(self, sample_rate: f32) -> (usize, usize, usize) {
        let sr = sample_rate.max(8_000.0);
        let frame = ((self.frame_ms() / 1000.0) * sr).round().max(64.0) as usize;
        let hop = (frame / 4).max(1);
        let search = ((self.search_ms() / 1000.0) * sr).round().max(8.0) as usize;
        (frame, hop, search)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_modes_use_longer_grains_and_all_stay_in_order() {
        let sr = 48_000.0;
        let fast = WarpMode::Fast.wsola_params(sr).0;
        let tone = WarpMode::Tone.wsola_params(sr).0;
        let smooth = WarpMode::Smooth.wsola_params(sr).0;
        let complex = WarpMode::Complex.wsola_params(sr).0;
        assert!(fast < complex && complex < tone && tone < smooth);
        // 120 ms vid 48 kHz är 5 760 sampel — inte 120 sampel.
        assert_eq!(smooth, 5_760);
    }

    #[test]
    fn params_are_sane_at_any_sample_rate() {
        for mode in WarpMode::ALL {
            for sr in [8_000.0, 44_100.0, 96_000.0, 192_000.0] {
                let (frame, hop, search) = mode.wsola_params(sr);
                assert!(frame >= 64, "{mode:?} vid {sr}: korn {frame}");
                assert!(hop >= 1 && hop < frame, "{mode:?}: hopp {hop} av {frame}");
                assert!(search >= 8 && search < frame, "{mode:?}: sök {search} av {frame}");
            }
        }
    }

}
