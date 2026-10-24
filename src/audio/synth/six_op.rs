//! **Sytrus 6-op**: sex operatorer i en matris.
//!
//! Varje operator är en sinus med eget frekvensförhållande, och varje par kan modulera
//! varandra (`matrix[source][target]`, i radianer). Det som skiljer det här från `FmParams`
//! är att **alla** kan modulera **alla** — en 6×6-matris i stället för ett fast par — och att
//! en operator kan återkoppla till sig själv (en sågtand ur en sinus).
//!
//! Ordningen är den i `next_sample`, och den är en regel, inte en detalj: en källa med **lägre**
//! index är redan räknad den här samplet och läses direkt; en källa med **samma eller högre**
//! index läses från föregående sampel. Utan den regeln vore en framåtriktad koppling antingen
//! ett sampel fel eller omöjlig, och egen återkoppling vore implicit.

use std::f32::consts::TAU;

/// Antal operatorer. Sex är Sytrus egna tak, och matrisen är 6×6.
pub const SIX_OP_COUNT: usize = 6;

/// Matrisen och rattarna. `Default` är en ren sinus: operator 1 hörs, ingen modulerar.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SixOpParams {
    /// Frekvensförhållande per operator (1,0 = notens frekvens).
    pub ratios: [f32; SIX_OP_COUNT],
    /// Utgångsnivå per operator. En operator med nivå 0 hörs inte men kan fortfarande modulera.
    pub levels: [f32; SIX_OP_COUNT],
    /// `matrix[källa][mål]`: hur mycket källan fasmodulerar målet, i radianer.
    pub matrix: [[f32; SIX_OP_COUNT]; SIX_OP_COUNT],
}

impl Default for SixOpParams {
    fn default() -> Self {
        let mut levels = [0.0; SIX_OP_COUNT];
        levels[0] = 1.0;
        Self {
            ratios: [1.0; SIX_OP_COUNT],
            levels,
            matrix: [[0.0; SIX_OP_COUNT]; SIX_OP_COUNT],
        }
    }
}

impl SixOpParams {
    /// En kedja: operator i modulerar i+1, och bara den sista hörs — den klassiska
    /// FM-stacken, och det snabbaste sättet att höra att matrisen fungerar.
    pub fn chain(index: f32) -> Self {
        let mut params = Self::default();
        for source in 0..SIX_OP_COUNT - 1 {
            params.matrix[source][source + 1] = index;
        }
        params.levels = [0.0; SIX_OP_COUNT];
        params.levels[SIX_OP_COUNT - 1] = 1.0;
        params
    }
}

/// Röstens tillstånd: en fas per operator och föregående sampels utgångar (för återkoppling
/// och för källor med högre index).
#[derive(Clone, Copy, Debug)]
pub struct SixOpVoice {
    phases: [f32; SIX_OP_COUNT],
    last: [f32; SIX_OP_COUNT],
}

impl Default for SixOpVoice {
    fn default() -> Self {
        Self {
            phases: [0.0; SIX_OP_COUNT],
            last: [0.0; SIX_OP_COUNT],
        }
    }
}

impl SixOpVoice {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nollställer faserna vid anslaget — annars ärver en ny not förra notens vågformsläge.
    pub fn reset(&mut self) {
        self.phases = [0.0; SIX_OP_COUNT];
        self.last = [0.0; SIX_OP_COUNT];
    }

    /// Ett sampel ur matrisen. `inc` är notens fasökning per sampel (2π·f/sr).
    #[inline]
    pub fn next_sample(&mut self, params: &SixOpParams, inc: f32) -> f32 {
        let mut out = [0.0f32; SIX_OP_COUNT];
        for target in 0..SIX_OP_COUNT {
            let mut modulation = 0.0;
            for source in 0..SIX_OP_COUNT {
                let amount = params.matrix[source][target];
                if amount != 0.0 {
                    let value = if source < target {
                        out[source]
                    } else {
                        self.last[source]
                    };
                    modulation += amount * value;
                }
            }
            let mut phase = self.phases[target] + inc * params.ratios[target];
            if phase >= TAU || phase < 0.0 {
                phase -= TAU * (phase / TAU).floor();
            }
            self.phases[target] = phase;
            out[target] = (phase + modulation).sin();
        }
        self.last = out;
        let mut sample = 0.0;
        for (value, level) in out.iter().zip(params.levels.iter()) {
            sample += value * level;
        }
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(params: &SixOpParams, freq: f32, samples: usize) -> Vec<f32> {
        let sample_rate = 48_000.0;
        let inc = TAU * freq / sample_rate;
        let mut voice = SixOpVoice::new();
        (0..samples).map(|_| voice.next_sample(params, inc)).collect()
    }

    fn mag_at(x: &[f32], freq: f32) -> f32 {
        let w = TAU * freq / 48_000.0;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, v) in x.iter().enumerate() {
            re += v * (w * i as f32).cos();
            im += v * (w * i as f32).sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f32
    }

    /// **Utan modulering är 6-op en sinus.** Matrisen noll och bara operator 1 hörd ger
    /// exakt den vanliga oscillatorn — annars vore varje jämförelse mot FM meningslös.
    #[test]
    fn without_modulation_six_op_is_a_sine() {
        let params = SixOpParams::default();
        let out = render(&params, 440.0, 4_800);
        let body = &out[1_200..];
        let fundamental = mag_at(body, 440.0);
        let second = mag_at(body, 880.0);
        assert!(fundamental > 0.2, "grundtonen: {fundamental:.4}");
        assert!(
            second < fundamental * 0.01,
            "ingen andra överton utan modulering: {second:.5} mot {fundamental:.4}"
        );
    }

    /// **Modulering lägger sidoband där FM säger.** Bärvåg 440 Hz, modulator med
    /// förhållande 0,5 (220 Hz) och index 2,0: sidobanden hamnar på 440 ± 220 = 220 och 660 Hz.
    /// Mätt i spektrum, inte på nivå — en ren volymökning hade inte gett dem.
    #[test]
    fn modulation_puts_sidebands_where_fm_says() {
        let mut params = SixOpParams::default();
        params.ratios[1] = 0.5;
        params.levels[1] = 0.0; // modulerar men hörs inte själv
        params.matrix[1][0] = 2.0;
        let out = render(&params, 440.0, 9_600);
        let body = &out[2_400..];
        let carrier = mag_at(body, 440.0);
        let low = mag_at(body, 220.0);
        let high = mag_at(body, 660.0);
        assert!(carrier > 0.05, "bärvågen: {carrier:.4}");
        assert!(low > carrier * 0.2, "nedre sidobandet: {low:.4} mot {carrier:.4}");
        assert!(high > carrier * 0.2, "övre sidobandet: {high:.4} mot {carrier:.4}");
    }

    /// **Egen återkoppling läser föregående sampel.** Utan det skulle operatorn modulera sig
    /// själv med ett värde som inte finns än, och återkopplingen bli noll — alltså ingen sågtand.
    #[test]
    fn feedback_uses_the_previous_sample() {
        let mut params = SixOpParams::default();
        params.matrix[0][0] = 1.5;
        let out = render(&params, 220.0, 9_600);
        let body = &out[2_400..];
        let fundamental = mag_at(body, 220.0);
        let second = mag_at(body, 440.0);
        let third = mag_at(body, 660.0);
        assert!(
            second > fundamental * 0.1 && third > fundamental * 0.05,
            "återkopplingen ska ge övertoner: f {fundamental:.4}, 2f {second:.4}, 3f {third:.4}"
        );
    }
}
