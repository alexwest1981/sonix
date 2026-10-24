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
/// Fler stämmor än fyra kostar mer än de låter bättre: fyra räcker för en tjock klang.
const MAX_UNISON: usize = 4;

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
    /// **Unison** (0.9.72): antal stämmor 1–4 och deras spridning i **cent**.
    /// `unison = 1` är standard och då räknas exakt som förut.
    pub unison: u8,
    pub detune: f32,
    /// **LFO** (0.9.71): hastighet i Hz och djup 0–1. `lfo_target`: 0 = av, 1 = tonhöjd,
    /// 2 = cutoff. Djupet är noll som standard, så en gammal patch står still.
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub lfo_target: u8,
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
    /// **Tonhöjdsenvelop per operator** i halvtoner (−24..24). Samma ADSR som amplituden formar
    /// tonhöjden — Sytrus har egna kurvor per operator; att återanvända envelopen ger samma
    /// rörelse med ett fält i stället för sex. `#[serde(default)]` → 0, så gamla patchar står still.
    #[serde(default = "no_pitch")]
    pub pitch_amount: [f32; SIX_OP_COUNT],
    /// **Filterenvelop per operator** i oktaver (−4..4): opens cutoff följer operadens egen ADSR.
    #[serde(default = "no_pitch")]
    pub filter_env: [f32; SIX_OP_COUNT],
    /// **Klaviaturskala per operator** (0..1): 1,0 följer filtret med tonhöjden en oktav per
    /// oktav, så höga toner behåller samma klangfärg som låga. 0 = filtret står still.
    #[serde(default = "no_pitch")]
    pub key_track: [f32; SIX_OP_COUNT],
    /// **Envelopkurva per operator** (−2..2): 0 = rak, positiv = snabb start och lång svans,
    /// negativ = mjuk start. Samma ADSR, olika form — Sytrus har tension per envelop.
    #[serde(default = "no_pitch")]
    pub env_curve: [f32; SIX_OP_COUNT],
    /// **Panorering per operator** (−1..1, 0 = mitten). Linjär lag: mitten ger (1,1), så en
    /// patch utan panorering låter exakt som förut.
    #[serde(default = "no_pitch")]
    pub pan: [f32; SIX_OP_COUNT],
    /// **Andra LFO:n** — Sytrus har flera. Egen fas, eget mål, så tonhöjd och cutoff kan röra sig
    /// olika fort. Djup 0 = av.
    #[serde(default)]
    pub lfo2_rate: f32,
    #[serde(default)]
    pub lfo2_depth: f32,
    #[serde(default)]
    pub lfo2_target: u8,
}

pub fn no_pitch() -> [f32; SIX_OP_COUNT] {
    [0.0; SIX_OP_COUNT]
}

/// Lågpasset per operator. Egen struct i stället för `FilterParams`: matrisen sparas som
/// JSON, och sex `FilterParams` i en array kräver egen serde-väg — två fält klarar sig själva.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpFilter {
    pub cutoff: f32,
    pub resonance: f32,
    /// Filtertyp: 0 = lågpass (standard, som före 0.9.70), 1 = bandpass, 2 = högpass,
    /// 3 = notch. `#[serde(default)]` → 0, så gamla patchar filtrerar precis som förut.
    #[serde(default)]
    pub mode: u8,
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
        mode: 0,
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
            unison: 1,
            detune: 12.0,
            lfo_rate: 5.0,
            lfo_depth: 0.0,
            lfo_target: 0,
            envs: neutral_envs(),
            filters: open_filters(),
            pitch_amount: no_pitch(),
            filter_env: no_pitch(),
            key_track: no_pitch(),
            env_curve: no_pitch(),
            pan: no_pitch(),
            lfo2_rate: 0.0,
            lfo2_depth: 0.0,
            lfo2_target: 0,
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
    /// En fasuppsättning per **unisonstämma** — stämma 0 är den vanliga, och med `unison = 1`
    /// används bara den.
    phases: [[f32; SIX_OP_COUNT]; MAX_UNISON],
    last: [[f32; SIX_OP_COUNT]; MAX_UNISON],
    envs: [AdsrVoice; SIX_OP_COUNT],
    filters: [[StateVariableFilter; SIX_OP_COUNT]; MAX_UNISON],
    lfo_phase: f32,
    lfo2_phase: f32,
    sample_rate: f32,
}

impl SixOpVoice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phases: [[0.0; SIX_OP_COUNT]; MAX_UNISON],
            last: [[0.0; SIX_OP_COUNT]; MAX_UNISON],
            envs: std::array::from_fn(|_| AdsrVoice::new(sample_rate)),
            filters: std::array::from_fn(|_| {
                std::array::from_fn(|_| StateVariableFilter::new(sample_rate))
            }),
            lfo_phase: 0.0,
            lfo2_phase: 0.0,
            sample_rate,
        }
    }

    /// Nytt anslag: faserna nollställs (annars ärver noten förra notens vågformsläge) och
    /// alla sex enveloper öppnar.
    pub fn reset(&mut self) {
        self.phases = [[0.0; SIX_OP_COUNT]; MAX_UNISON];
        self.last = [[0.0; SIX_OP_COUNT]; MAX_UNISON];
        for env in &mut self.envs {
            env.gate_on();
        }
        for voice_filters in &mut self.filters {
            for filter in voice_filters {
                filter.reset();
            }
        }
    }

    pub fn gate_off(&mut self) {
        for env in &mut self.envs {
            env.gate_off();
        }
    }

    /// Ett sampel ur matrisen. `inc` är notens fasökning per sampel (2π·f/sr).
    ///
    /// Med `unison > 1` räknas matrisen en gång per stämma med lite olika tonhöjd och fas, och
    /// summorna delas med √n så att nivån står still. Enveloperna tickas **en** gång per sampel
    /// och delas av stämmorna — annars skulle de sex enveloperna gå n gånger för fort.
    #[inline]
    /// Monosummeringen av `next_sample_stereo` — alla prov och alla vägar som inte bryr sig om
    /// bredd går här, och med pan 0 är den identisk med förut.
    #[cfg(test)]
    pub fn next_sample(&mut self, params: &SixOpParams, inc: f32) -> f32 {
        let (l, r) = self.next_sample_stereo(params, inc);
        (l + r) * 0.5
    }

    pub fn next_sample_stereo(&mut self, params: &SixOpParams, inc: f32) -> (f32, f32) {
        // Klaviaturskalan mäts mot 440 Hz: hur många oktaver tonen ligger över/under.
        let key_octaves = (inc * self.sample_rate / TAU / 440.0).max(1e-6).log2();
        // **LFO:** en fas per röst, som en analog synt. Djupet är 0 som standard, och då är
        // `lfo` 0 och ingen operator påverkas.
        self.lfo2_phase += TAU * params.lfo2_rate.max(0.0) / self.sample_rate.max(1.0);
        if self.lfo2_phase >= TAU {
            self.lfo2_phase -= TAU;
        }
        let lfo2 = if params.lfo2_depth == 0.0 {
            0.0
        } else {
            self.lfo2_phase.sin() * params.lfo2_depth
        };
        self.lfo_phase += TAU * params.lfo_rate.max(0.0) / self.sample_rate.max(1.0);
        if self.lfo_phase >= TAU {
            self.lfo_phase -= TAU;
        }
        let lfo = if params.lfo_depth > 0.0 && params.lfo_target != 0 {
            self.lfo_phase.sin() * params.lfo_depth
        } else {
            0.0
        };
        // Tonhöjd: ±6 % vid fullt djup, alltså ungefär ±1 halvton — en vibrato, inte en siren.
        let mut pitch_scale = if params.lfo_target == 1 {
            1.0 + lfo * 0.06
        } else {
            1.0
        };
        if params.lfo2_target == 1 {
            pitch_scale *= 1.0 + lfo2 * 0.06;
        }

        let mut env_now = [0.0f32; SIX_OP_COUNT];
        for (target, env) in self.envs.iter_mut().enumerate() {
            let value = env.next_sample(&params.envs[target]);
            let curve = params.env_curve[target];
            env_now[target] = if curve == 0.0 {
                value
            } else {
                value.clamp(0.0, 1.0).powf((2.0f32).powf(-curve))
            };
        }

        let voices = (params.unison.max(1) as usize).min(MAX_UNISON);
        let mut total_l = 0.0;
        let mut total_r = 0.0;
        for u in 0..voices {
            // Stämman ligger `detune` cent från mitten, jämnt fördelade. Faktorn räknas per
            // stämma — den får inte ärvas vidare, då blir sista stämman kraftigt förstämd.
            let detune = if voices > 1 {
                (u as f32 / (voices - 1) as f32 - 0.5) * params.detune * 0.01
            } else {
                0.0
            };
            let (l, r) = self.matrix_sample(
                params,
                inc * pitch_scale * (1.0 + detune),
                &env_now,
                lfo,
                lfo2,
                key_octaves,
                u,
            );
            total_l += l;
            total_r += r;
        }
        let norm = (voices as f32).sqrt();
        (total_l / norm, total_r / norm)
    }

    /// Matrisen för **en** unisonstämma. Indexen är både källa och mål i samma array
    /// (`out[source]` när `out[target]` skrivs), så en iteratoromskrivning gör koden otydligare.
    #[allow(clippy::needless_range_loop, reason = "källa och mål indexerar samma array")]
    // Argumenten är inparametrar i innerloopen; en struct hade kopierats per sampel.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn matrix_sample(
        &mut self,
        params: &SixOpParams,
        inc: f32,
        env_now: &[f32; SIX_OP_COUNT],
        lfo: f32,
        lfo2: f32,
        key_octaves: f32,
        u: usize,
    ) -> (f32, f32) {
        let mut out = [0.0f32; SIX_OP_COUNT];
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for target in 0..SIX_OP_COUNT {
            let mut modulation = 0.0;
            for source in 0..SIX_OP_COUNT {
                let amount = params.matrix[source][target];
                if amount != 0.0 {
                    let value = if source < target {
                        out[source]
                    } else {
                        self.last[u][source]
                    };
                    modulation += amount * value;
                }
            }
            // Tonhöjdsenvelopen: samma värde som amplituden, skalat i halvtoner.
            let pitch = params.pitch_amount[target];
            let bend = if pitch == 0.0 {
                1.0
            } else {
                (2.0f32).powf(pitch * env_now[target] / 12.0)
            };
            let mut phase = self.phases[u][target] + inc * params.ratios[target] * bend;
            if phase >= TAU || phase < 0.0 {
                phase -= TAU * (phase / TAU).floor();
            }
            self.phases[u][target] = phase;
            // Envelopen sitter **före** matrisen: den formar både det som hörs och det som
            // modulerar, precis som på en FM-synt.
            let mut value = (phase + modulation).sin() * env_now[target];
            // Filtret sitter efter envelopen, före matrisen: en filtrerad modulator tar sina
            // övertoner med sig in i ljudet. Avstängt filter kostar ingenting.
            let mut filter = params.filters[target];
            if params.key_track[target] != 0.0 {
                filter.cutoff = (filter.cutoff
                    * (2.0f32).powf(params.key_track[target] * key_octaves))
                    .clamp(20.0, 20_000.0);
            }
            // Filterenvelopen: operadens egen ADSR öppnar filtret i oktaver.
            if params.filter_env[target] != 0.0 {
                filter.cutoff =
                    (filter.cutoff * (2.0f32).powf(params.filter_env[target] * env_now[target]))
                        .clamp(20.0, 20_000.0);
            }
            if params.lfo2_target == 2 && lfo2 != 0.0 {
                filter.cutoff = (filter.cutoff * (2.0f32).powf(lfo2 * 2.0)).clamp(20.0, 20_000.0);
            }
            if params.lfo_target == 2 && lfo != 0.0 {
                // Cutoff i oktaver: ±2 oktaver vid fullt djup.
                filter.cutoff = (filter.cutoff * (2.0f32).powf(lfo * 2.0)).clamp(20.0, 20_000.0);
            }
            if filter.cutoff < FILTER_OPEN_HZ {
                let p = filter.params();
                let state = &mut self.filters[u][target];
                value = match filter.mode {
                    1 => state.process_bandpass(value, &p),
                    2 => state.process_highpass(value, &p),
                    3 => state.process_notch(value, &p),
                    _ => state.process_lowpass(value, &p),
                };
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
        // **Nivå och panorering efter ring/add:** annars hade en additiv eller
        // ringmodulerad operator hörts utan sitt bidrag. Panlagen är linjär, så mitten (0)
        // ger (1,1) — samma nivå som en patch utan panorering alltid haft.
        for target in 0..SIX_OP_COUNT {
            let level = params.levels[target];
            let pan = params.pan[target].clamp(-1.0, 1.0);
            let (gl, gr) = if pan < 0.0 {
                (1.0, 1.0 + pan)
            } else {
                (1.0 - pan, 1.0)
            };
            out_l += out[target] * level * gl;
            out_r += out[target] * level * gr;
        }
        self.last[u] = out;
        (out_l, out_r)
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

    /// **Tonhöjdsenvelopen böjer operatorn.** +12 halvtoner med en envelop som håller 1,0 ska
    /// flytta grundtonen en okt upp — mätt i spektrat, inte på känsla.
    #[test]
    fn a_pitch_envelope_moves_the_operator_an_octave() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams { attack: 0.0, decay: 0.0, sustain: 1.0, release: 0.2 };
        let flat = render(&params, 440.0, 9_600);
        let body = &flat[2_400..];
        assert!(mag_at(body, 440.0) > 0.2, "utan böjning ligger tonen på 440");

        params.pitch_amount[0] = 12.0;
        let bent = render(&params, 440.0, 9_600);
        let body = &bent[2_400..];
        assert!(mag_at(body, 880.0) > 0.2, "en okt upp ska ge 880 Hz");
        assert!(mag_at(body, 440.0) < 0.05, "440 ska vara borta");
    }

    /// **Filterenvelopen öppnar filtret.** Samma lågpass, samma ton — bara envelopens djup skiljer.
    #[test]
    fn a_filter_envelope_opens_the_filter() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams { attack: 0.0, decay: 0.0, sustain: 1.0, release: 0.2 };
        // En sinus har inga övertoner att filtrera: modulation gör operator 1 rik på dem.
        params.ratios[1] = 2.0;
        params.matrix[1][0] = 3.0;
        params.filters[0] = OpFilter { cutoff: 300.0, resonance: 0.7, mode: 0 };

        let closed = render(&params, 220.0, 9_600);
        let closed_high = mag_at(&closed[4_800..], 660.0);

        params.filter_env[0] = 3.0;
        let open = render(&params, 220.0, 9_600);
        let open_high = mag_at(&open[4_800..], 660.0);

        assert!(
            open_high > closed_high * 4.0,
            "öppet filter ska släppa igenom 1,1 kHz: {open_high:.4} mot {closed_high:.4}"
        );
    }

    /// **Två LFO:er rör olika saker.** Den andra får eget djup och eget mål; med bara den första
    /// på tonhöjd ska den andra på cutoff ändra spektrat.
    #[test]
    fn a_second_lfo_can_move_the_filter_independently() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams { attack: 0.0, decay: 0.0, sustain: 1.0, release: 0.2 };
        params.filters[0] = OpFilter { cutoff: 800.0, resonance: 1.0, mode: 0 };
        params.lfo_rate = 5.0;
        params.lfo_depth = 0.5;
        params.lfo_target = 1;
        let one = render(&params, 220.0, 9_600);

        params.lfo2_rate = 7.0;
        params.lfo2_depth = 1.0;
        params.lfo2_target = 2;
        let two = render(&params, 220.0, 9_600);

        let diff: f32 = one
            .iter()
            .zip(two.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / one.len() as f32;
        assert!(diff > 0.001, "den andra LFO:n ska höras: {diff:.5}");
    }

    /// **Klaviaturskalan håller klangen över oktavrena.** Utan den blir en ton en oktav upp
    /// mycket starkare filtrerad; med `key_track = 1,0` följer filtret med.
    #[test]
    fn key_tracking_keeps_the_timbre_across_octaves() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams { attack: 0.0, decay: 0.0, sustain: 1.0, release: 0.2 };
        params.ratios[1] = 2.0;
        params.matrix[1][0] = 3.0;
        params.filters[0] = OpFilter { cutoff: 400.0, resonance: 0.7, mode: 0 };

        let brightness = |pitch: f32, p: &SixOpParams| {
            let out = render(p, pitch, 9_600);
            let body = &out[4_800..];
            mag_at(body, pitch * 3.0) / mag_at(body, pitch).max(1e-6)
        };

        let fixed_gap = (brightness(220.0, &params) - brightness(880.0, &params)).abs();
        params.key_track[0] = 1.0;
        let tracked_gap = (brightness(220.0, &params) - brightness(880.0, &params)).abs();

        assert!(
            tracked_gap < fixed_gap * 0.5,
            "klaviaturskalan ska minska skillnaden: {tracked_gap:.4} mot {fixed_gap:.4}"
        );
    }

    /// **Envelopkurvan ändrar formen, inte tiden.** Samma attack: en positiv kurva är uppe fort,
    /// en negativ smyger i gång. Mätt som RMS i de första 40 ms.
    #[test]
    fn an_envelope_curve_changes_the_attack_shape() {
        let mut params = SixOpParams::default();
        params.envs[0] = AdsrParams { attack: 0.2, decay: 0.0, sustain: 1.0, release: 0.2 };
        let early = |p: &SixOpParams| {
            let out = render(p, 220.0, 4_800);
            let n = 1_920; // 40 ms vid 48 kHz
            (out[..n].iter().map(|v| v * v).sum::<f32>() / n as f32).sqrt()
        };

        let straight = early(&params);
        params.env_curve[0] = 2.0;
        let fast = early(&params);
        params.env_curve[0] = -2.0;
        let slow = early(&params);

        assert!(fast > straight * 1.5, "snabb kurva ska ge mer tidigt: {fast:.4} mot {straight:.4}");
        assert!(slow < straight * 0.7, "mjuk kurva ska ge mindre tidigt: {slow:.4} mot {straight:.4}");
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

    /// **Filtertypen per operator.** Samma beräkning, olika utgång: ett lågpass på 2 kHz släpper
    /// igenom en 220 Hz-ton, ett högpass på samma cutoff tar bort den.
    #[test]
    fn the_operator_filter_can_be_a_highpass() {
        let mut params = SixOpParams::default();
        params.filters[0] = OpFilter {
            cutoff: 2_000.0,
            resonance: 0.7,
            mode: 0,
        };
        let low = mag_at(&render(&params, 220.0, 4_800)[1_200..], 220.0);
        params.filters[0].mode = 2;
        let high = mag_at(&render(&params, 220.0, 4_800)[1_200..], 220.0);
        println!("  220 Hz: lågpass {low:.4}, högpass {high:.4}");
        assert!(low > 0.2, "lågpasset ska släppa igenom: {low:.4}");
        assert!(high < 0.05, "högpasset ska ta bort grundtonen: {high:.4}");
    }

    /// **LFO på tonhöjden** gör vibrato: bärvågen tappar energi och sidoband dyker upp kring den.
    /// Djup 0 ska ge exakt den torra tonen — annars påverkar reglaget ljudet även avstängt.
    #[test]
    fn the_lfo_bends_the_pitch_only_when_the_depth_is_up() {
        let mut params = SixOpParams::default();
        let dry = mag_at(&render(&params, 440.0, 9_600)[1_200..], 440.0);
        params.lfo_depth = 1.0;
        params.lfo_target = 1;
        params.lfo_rate = 10.0;
        let wet_out = render(&params, 440.0, 9_600);
        let body = &wet_out[1_200..];
        let carrier = mag_at(body, 440.0);
        let side = mag_at(body, 440.0 * 1.03);
        println!("  bärvåg torr {dry:.4} med LFO {carrier:.4}; sidoband {side:.4}");
        assert!(dry > 0.5, "torrt: {dry:.4}");
        assert!(
            carrier < dry * 0.5,
            "vibrato ska sprida energin: {carrier:.4} av {dry:.4}"
        );
        assert!(side > 0.02, "sidoband kring bärvågen: {side:.4}");
    }

    /// **Unison ändrar klangen men inte nivån.** Fyra stämmor med 20 cent spridning ska ge en
    /// annan vågform än en stämma, men samma RMS — annars är det en volymratt, inte en kör.
    #[test]
    fn unison_thickens_the_sound_without_raising_the_level() {
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let params = SixOpParams::default();
        let mono = render(&params, 440.0, 9_600);
        let mut wide = params;
        wide.unison = 4;
        wide.detune = 20.0;
        let uni = render(&wide, 440.0, 9_600);
        let diff: Vec<f32> = mono.iter().zip(&uni).map(|(a, b)| a - b).collect();
        let (m, u, d) = (rms(&mono[1_200..]), rms(&uni[1_200..]), rms(&diff[1_200..]));
        println!("  mono {m:.4}, unison {u:.4}, skillnad {d:.4}");
        assert!(d > 0.05, "unison ska ändra vågformen: {d:.4}");
        assert!(
            (m - u).abs() < m * 0.3,
            "nivån ska stå still: {u:.4} mot {m:.4}"
        );
    }
}
