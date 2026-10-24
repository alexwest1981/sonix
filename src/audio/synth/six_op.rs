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

use crate::audio::envelope::{AdsrParams, AdsrVoice};
use crate::audio::filter::{FilterParams, StateVariableFilter};

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
    /// **Ringmodulering** per cell: källan multiplicerar målets amplitud. 0 = orörd,
    /// 1 = helt ringmodulerad (bärvågen försvinner, sidobanden tar över).
    #[serde(default = "no_modulation")]
    pub ring: [[f32; SIX_OP_COUNT]; SIX_OP_COUNT],
    /// **Additiv blandning** per cell: källans ljud läggs till målets utgång. Det är den tredje
    /// raden i Sytrus matris — utan den går bara FM att bygga.
    #[serde(default = "no_modulation")]
    pub add: [[f32; SIX_OP_COUNT]; SIX_OP_COUNT],
    /// Egen ADSR per operator — den ligger **före** matrisen, så en kort envelop på en
    /// modulator tar bort sidobanden igen (det är skillnaden mellan en klang och ett klick).
    /// `#[serde(default)]` gör att patchar sparade före 0.9.48 öppnas som förut.
    #[serde(default = "neutral_envs")]
    pub envs: [AdsrParams; SIX_OP_COUNT],
    /// Lågpass per operator, **efter** envelopen och före matrisen. `cutoff >= 19 000` är
    /// avstängt (filtret rör inte samplet), så en patch som inte rört det låter som förut.
    #[serde(default = "open_filters")]
    pub filters: [OpFilter; SIX_OP_COUNT],
}

/// Lågpasset per operator. Egen struct i stället för `FilterParams`: matrisen sparas som
/// JSON, och sex `FilterParams` i en array kräver egen serde-väg — två fält klarar sig själva.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpFilter {
    pub cutoff: f32,
    pub resonance: f32,
}

impl OpFilter {
    fn params(self) -> FilterParams {
        FilterParams {
            cutoff: self.cutoff,
            resonance: self.resonance,
        }
    }
}

fn no_modulation() -> [[f32; SIX_OP_COUNT]; SIX_OP_COUNT] {
    [[0.0; SIX_OP_COUNT]; SIX_OP_COUNT]
}

/// Öppet filter: över hörbarheten, alltså ingen filtrering.
fn open_filters() -> [OpFilter; SIX_OP_COUNT] {
    [OpFilter {
        cutoff: 20_000.0,
        resonance: 0.7,
    }; SIX_OP_COUNT]
}

/// Under den här gränsen körs filtret; över den är det avstängt (billigare än att räkna
/// ett filter som inte gör något).
const FILTER_OPEN_HZ: f32 = 19_000.0;

/// Nivå 1 utan kontur: envelopen ska inte ändra ljudet för en patch som inte rört den.
fn neutral_envs() -> [AdsrParams; SIX_OP_COUNT] {
    [AdsrParams {
        attack: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release: 0.0,
    }; SIX_OP_COUNT]
}

impl Default for SixOpParams {
    fn default() -> Self {
        let mut levels = [0.0; SIX_OP_COUNT];
        levels[0] = 1.0;
        Self {
            ratios: [1.0; SIX_OP_COUNT],
            levels,
            matrix: [[0.0; SIX_OP_COUNT]; SIX_OP_COUNT],
            ring: no_modulation(),
            add: no_modulation(),
            envs: neutral_envs(),
            filters: open_filters(),
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
    envs: [AdsrVoice; SIX_OP_COUNT],
    filters: [StateVariableFilter; SIX_OP_COUNT],
}

impl SixOpVoice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phases: [0.0; SIX_OP_COUNT],
            last: [0.0; SIX_OP_COUNT],
            envs: std::array::from_fn(|_| AdsrVoice::new(sample_rate)),
            filters: std::array::from_fn(|_| StateVariableFilter::new(sample_rate)),
        }
    }

    /// Nytt anslag: faserna nollställs (annars ärver noten förra notens vågformsläge) och
    /// alla sex enveloper öppnar.
    pub fn reset(&mut self) {
        self.phases = [0.0; SIX_OP_COUNT];
        self.last = [0.0; SIX_OP_COUNT];
        for env in &mut self.envs {
            env.gate_on();
        }
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    pub fn gate_off(&mut self) {
        for env in &mut self.envs {
            env.gate_off();
        }
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
            // Envelopen sitter **före** matrisen: den formar både det som hörs och det som
            // modulerar, precis som på en FM-synt.
            let env = self.envs[target].next_sample(&params.envs[target]);
            let mut value = (phase + modulation).sin() * env;
            // Filtret sitter efter envelopen, före matrisen: en filtrerad modulator tar sina
            // övertoner med sig in i ljudet. Avstängt filter kostar ingenting.
            let filter = params.filters[target];
            if filter.cutoff < FILTER_OPEN_HZ {
                value = self.filters[target].process_lowpass(value, &filter.params());
            }
            out[target] = value;
        }
        // **Ringmodulering och additiv blandning** (0.9.69) räknas på en fryst kopia, så
        // ordningen mellan cellerna inte spelar roll. Nollade celler kostar ingenting.
        let frozen = out;
        for target in 0..SIX_OP_COUNT {
            for source in 0..SIX_OP_COUNT {
                let ring = params.ring[source][target];
                if ring != 0.0 {
                    out[target] *= 1.0 - ring + ring * frozen[source];
                }
            }
        }
        for target in 0..SIX_OP_COUNT {
            for source in 0..SIX_OP_COUNT {
                let add = params.add[source][target];
                if add != 0.0 {
                    out[target] += add * frozen[source];
                }
            }
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
        let mut voice = SixOpVoice::new(48_000.0);
        voice.reset(); // som ett anslag: enveloperna öppnar
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

    /// **Egen envelop per operator.** En kort decay med sustain 0 på operator 1 ska tystna,
    /// fast matrisen står still — och en modulator vars envelop stänger ska ta bort sidobanden
    /// (det är skillnaden mellan en klang och ett klick).
    #[test]
    fn each_operator_has_its_own_envelope() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams {
            attack: 0.001,
            decay: 0.05,
            sustain: 0.0,
            release: 0.01,
        };
        let out = render(&params, 440.0, 4_800); // 100 ms
        let early = mag_at(&out[240..720], 440.0);
        let late = mag_at(&out[3_600..4_320], 440.0);
        assert!(early > 0.05, "anslaget hörs: {early:.4}");
        assert!(
            late < early * 0.2,
            "decay till sustain 0 ska tystna: {late:.4} mot {early:.4}"
        );

        // Modulatorns envelop styr sidobanden: samma matris, men modulerande operator tystnar.
        let mut with_mod = SixOpParams::default();
        with_mod.ratios[1] = 0.5;
        with_mod.levels[1] = 0.0;
        with_mod.matrix[1][0] = 2.0;
        with_mod.envs[1] = AdsrParams {
            attack: 0.001,
            decay: 0.05,
            sustain: 0.0,
            release: 0.01,
        };
        let out = render(&with_mod, 440.0, 4_800);
        let sideband_early = mag_at(&out[240..720], 660.0);
        let sideband_late = mag_at(&out[3_600..4_320], 660.0);
        assert!(sideband_early > 0.005, "sidobandet finns vid anslaget: {sideband_early:.4}");
        assert!(
            sideband_late < sideband_early * 0.2,
            "när modulatorn tystnar försvinner sidobandet: {sideband_late:.5} mot {sideband_early:.4}"
        );
    }

    /// **Filtret per operator tar bort det som ligger ovanför brytfrekvensen.** Bärvåg 440 Hz
    /// med sidoband på 220 och 660: ett lågpass på 500 Hz ska behålla 220 men dämpa 660.
    #[test]
    fn the_operator_filter_takes_the_high_side_band_away() {
        let mut params = SixOpParams::default();
        params.ratios[1] = 0.5;
        params.levels[1] = 0.0;
        params.matrix[1][0] = 2.0;
        let open = render(&params, 440.0, 9_600);

        params.filters[0].cutoff = 300.0;
        let filtered = render(&params, 440.0, 9_600);

        let low_open = mag_at(&open[2_400..], 220.0);
        let low_filtered = mag_at(&filtered[2_400..], 220.0);
        let high_open = mag_at(&open[2_400..], 660.0);
        let high_filtered = mag_at(&filtered[2_400..], 660.0);
        assert!(high_filtered < high_open * 0.3, "660 Hz ska dämpas: {high_filtered:.4} mot {high_open:.4}");
        assert!(low_filtered > low_open * 0.6, "220 Hz ska vara kvar: {low_filtered:.4} mot {low_open:.4}");
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

    /// **Additiv blandning:** källans ljud läggs till målets utgång. Operator 1 hörs, operator 2
    /// ligger på 3× frekvensen — med `add[0][1] = 1` bär den också grundtonen, utan den inte.
    #[test]
    fn additive_mixing_feeds_one_operator_into_another() {
        let mut params = SixOpParams::default();
        params.levels = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        params.ratios[1] = 3.0;
        let without = mag_at(&render(&params, 440.0, 4_800)[1_200..], 440.0);
        params.add[0][1] = 1.0;
        let with = mag_at(&render(&params, 440.0, 4_800)[1_200..], 440.0);
        println!("  grundton: utan add {without:.4}, med add {with:.4}");
        assert!(without < 0.02, "utan add ska grundtonen vara borta: {without:.4}");
        assert!(with > 0.2, "med add ska den höras: {with:.4}");
    }

    /// **Ringmodulering:** källan multiplicerar målets amplitud. Bärvåg 2×, modulator 1×:
    /// bärvågen försvinner och sidobanden hamnar på 1× och 3×.
    #[test]
    fn ring_modulation_replaces_the_carrier_with_sidebands() {
        let mut params = SixOpParams::default();
        params.levels = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        params.ratios[1] = 2.0;
        let dry = render(&params, 440.0, 4_800);
        let carrier_dry = mag_at(&dry[1_200..], 880.0);
        params.ring[0][1] = 1.0;
        let wet = render(&params, 440.0, 4_800);
        let carrier_wet = mag_at(&wet[1_200..], 880.0);
        let lower = mag_at(&wet[1_200..], 440.0);
        let upper = mag_at(&wet[1_200..], 1_320.0);
        println!(
            "  bärvåg torr {carrier_dry:.4} våt {carrier_wet:.4}; sidoband {lower:.4} / {upper:.4}"
        );
        assert!(carrier_dry > 0.2, "bärvågen ska höras torrt: {carrier_dry:.4}");
        assert!(
            carrier_wet < carrier_dry * 0.1,
            "ringmodulering ska ta bort bärvågen: {carrier_wet:.4} av {carrier_dry:.4}"
        );
        assert!(lower > 0.1 && upper > 0.1, "sidoband: {lower:.4} och {upper:.4}");
    }
}
