use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct FilterParams {
    pub cutoff: f32,    // Hz (20.0 .. 20000.0)
    pub resonance: f32, // Q (0.1 .. 10.0)
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff: 8000.0,
            resonance: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StateVariableFilter {
    pub sample_rate: f32,
    ic1eq: f32,
    ic2eq: f32,
}

impl StateVariableFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    #[inline(always)]
    pub fn process_lowpass(&mut self, input: f32, params: &FilterParams) -> f32 {
        let cutoff = params.cutoff.clamp(20.0, self.sample_rate * 0.48);
        let q = params.resonance.clamp(0.1, 10.0);

        let g = (PI * cutoff / self.sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;

        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        v2
    }
}

/// **Samplerns filter** (Fas 8.4/7): ett lågpassfilter per röst med en **egen** ADSR som flyttar
/// cutoffen i oktaver (`filter_cutoff_at`).
///
/// Standardvärdet är **avstängt** (`on: false`) — då rörs samplarna inte alls, och ett projekt
/// från före filtret låter exakt som det gjorde. `cutoff_hz`/`resonance` ärvs ur
/// `FilterParams::default()` i stället för att skrivas en gång till (två tabeller för samma sak
/// driver isär), och `env_amount_octaves` är `0,0`: även med filtret **på** står cutoffen still
/// tills ratten flyttas.
///
/// Envelopen har en **musikalisk** standard (attack 0, decay 0,3 s, sustain 0, släpp 0,2 s) och
/// inte identiteten: filtret är avstängt med `on`, inte med envelopen, och när man väl slår på
/// det ska amount-ratten ge en **svepande** klang — det är hela poängen med en filterenvelop.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SamplerFilter {
    pub on: bool,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub env_amount_octaves: f32,
    pub env: crate::audio::envelope::AdsrParams,
}

impl Default for SamplerFilter {
    fn default() -> Self {
        let d = FilterParams::default();
        Self {
            on: false,
            cutoff_hz: d.cutoff,
            resonance: d.resonance,
            env_amount_octaves: 0.0,
            env: crate::audio::envelope::AdsrParams {
                attack: 0.0,
                decay: 0.3,
                sustain: 0.0,
                release: 0.2,
            },
        }
    }
}

/// **Filterts öppning just nu** (Fas 8.4/7): kanalens cutoff förskjuten av filterenvelopens
/// nivå, räknad i **oktaver**.
///
/// Oktaven är enheten därför att det är den musikaliska: `+4,0` oktaver är samma *avstånd* i
/// klang oavsett om grundcutoffen är 200 Hz eller 2 kHz, medan en faktor hade betytt olika sak
/// beroende på var ratten stod. Det är samma val FL:s Sampler och Abletons Simpler gör (deras
/// env-amount är i oktaver respektive procent av spannet).
///
/// `amount = 0,0` → **exakt** `base` (envelopen rör ingenting), vilket är vad en kanal utan
/// filterenvelop får. Resultatet kläms till det hörbara spannet `20 Hz … 20 kHz`.
pub fn filter_cutoff_at(base_cutoff_hz: f32, env_amount_octaves: f32, env_level: f32) -> f32 {
    let base = base_cutoff_hz.clamp(20.0, 20_000.0);
    let level = env_level.clamp(0.0, 1.0);
    (base * (env_amount_octaves * level).exp2()).clamp(20.0, 20_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_of_sine(cutoff: f32, freq: f32) -> f32 {
        let sr = 48000.0;
        let mut f = StateVariableFilter::new(sr);
        let params = FilterParams { cutoff, resonance: 0.707 };
        let warmup = 4800;
        let measure = 4800;
        let mut sum = 0.0f32;
        for i in 0..(warmup + measure) {
            let x = (2.0 * PI * freq * i as f32 / sr).sin();
            let y = f.process_lowpass(x, &params);
            if i >= warmup {
                sum += y * y;
            }
        }
        (sum / measure as f32).sqrt()
    }

    #[test]
    fn the_default_filter_is_off_and_moves_nothing() {
        let f = SamplerFilter::default();
        assert!(!f.on, "filtret ska vara avstängt som standard");
        assert_eq!(f.env_amount_octaves, 0.0);
        // Och även påslaget utan amount står cutoffen exakt still.
        assert_eq!(filter_cutoff_at(f.cutoff_hz, f.env_amount_octaves, 1.0), f.cutoff_hz);
    }

    #[test]
    fn a_zero_amount_leaves_the_cutoff_exactly_where_it_was() {
        // **Exakt**, inte nästan: en kanal utan filterenvelop ska inte flytta cutoffen alls.
        for base in [100.0, 440.0, 1000.0, 8000.0] {
            for level in [0.0, 0.5, 1.0] {
                assert_eq!(filter_cutoff_at(base, 0.0, level), base, "{base} Hz, nivå {level}");
            }
        }
    }

    #[test]
    fn an_amount_opens_the_filter_by_octaves() {
        // Full envelop och +4 oktaver = 16 gånger cutoffen; halv envelop = 2 oktaver = 4 gånger.
        assert_eq!(filter_cutoff_at(100.0, 4.0, 1.0), 1600.0);
        assert_eq!(filter_cutoff_at(100.0, 4.0, 0.5), 400.0);
        assert_eq!(filter_cutoff_at(100.0, 0.0, 1.0), 100.0);
    }

    #[test]
    fn the_cutoff_stays_inside_the_audible_range() {
        assert_eq!(filter_cutoff_at(20_000.0, 8.0, 1.0), 20_000.0);
        assert_eq!(filter_cutoff_at(20.0, -8.0, 1.0), 20.0);
        // En negativ amount är en **stängande** envelop, och den är giltig.
        assert_eq!(filter_cutoff_at(2000.0, -2.0, 1.0), 500.0);
    }

    #[test]
    fn lowpass_attenuates_high_frequencies() {
        let low = rms_of_sine(500.0, 100.0);
        let high = rms_of_sine(500.0, 15000.0);
        assert!(low > 0.5, "passband should survive, got {}", low);
        assert!(high < low * 0.1, "stopband {} should be far below {}", high, low);
    }

    #[test]
    fn reset_clears_filter_state() {
        let sr = 48000.0;
        let mut f = StateVariableFilter::new(sr);
        let params = FilterParams { cutoff: 200.0, resonance: 5.0 };
        for i in 0..1000 {
            let x = (2.0 * PI * 100.0 * i as f32 / sr).sin();
            f.process_lowpass(x, &params);
        }
        f.reset();
        let tail = f.process_lowpass(0.0, &params);
        assert!(tail.abs() < 1e-6, "state should be cleared, got {}", tail);
    }
}
