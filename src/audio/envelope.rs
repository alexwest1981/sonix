#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AdsrParams {
    pub attack: f32,  // in seconds
    pub decay: f32,   // in seconds
    pub sustain: f32, // 0.0 .. 1.0 amplitude
    pub release: f32, // in seconds
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self {
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.2,
        }
    }
}

impl AdsrParams {
    /// **Identiteten** — ingen envelop alls: full nivå från första samplet, ingen
    /// nedgång, inget släpp.
    ///
    /// Den finns som ett **namngivet** värde och inte som en flagga, av samma skäl som
    /// `plan_track_order` finns som en ren funktion: samplern (Fas 8.4) ska kunna säga
    /// "ingen envelop är vald" utan att ha ett dolt läge vid sidan av siffrorna. Ett
    /// projekt som sparades innan samplern fanns bär den här, och då är ljudet exakt
    /// som förut — `the_identity_envelope_changes_nothing` håller den.
    pub fn identity() -> Self {
        Self {
            attack: 0.0,
            decay: 0.0,
            sustain: 1.0,
            release: 0.0,
        }
    }

    /// Är det här identiteten, alltså ingen envelop? Jämförelsen är exakt (talen kommer
    /// från UI-reglage och jämförs som de skrivs), för en "nästan identitet" är en
    /// förändring av ljudet.
    pub fn is_identity(&self) -> bool {
        self.attack <= 0.0 && self.decay <= 0.0 && self.sustain >= 1.0 && self.release <= 0.0
    }
}

/// **Velociteten som gain** (Fas 8.4/7) — hur hårt noten slogs, som faktor på nivån.
///
/// En sampler ska svara på anslaget: en svagare not låter svagare. `sensitivity` är **hur
/// mycket** velocity får påverka, och det är den ratt varje etablerad sampler har (Ableton
/// Simpler: "Vel → Volume"; FL:s sampler: "Vel"). Kurvan är den **linjära** — den som
/// velocityn alltid har haft i den här kanalen (motorn multiplicerade volymen rakt av) — så
/// att standardvärdet `1,0` ger **exakt** samma tal som förut, inte ett som liknar:
///
/// * `sensitivity = 1,0` → `velocity` (identiskt med beteendet före ratten, se provet som
///   jämför med `==` och inte med en tolerans).
/// * `sensitivity = 0,0` → alltid `1,0`: anslaget påverkar inte nivån alls.
/// * däremellan → `(1 - s) + s · velocity`.
///
/// Båda ingångarna kläms till `0,0–1,0`. Stegets velocity kläms redan till `0,1–1,0` i
/// pianorullen, så klämningen ändrar inget för den vägen — den finns för att ett värde utanför
/// spannet (en trasig MIDI-fil, en framtida automationskurva) inte ska kunna förstärka.
pub fn velocity_gain(velocity: f32, sensitivity: f32) -> f32 {
    let v = velocity.clamp(0.0, 1.0);
    let s = sensitivity.clamp(0.0, 1.0);
    (1.0 - s) + s * v
}

#[derive(Debug, Clone, Copy)]
pub struct AdsrVoice {
    pub stage: EnvelopeStage,
    pub current_level: f32,
    pub sample_rate: f32,
}

impl AdsrVoice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            stage: EnvelopeStage::Idle,
            current_level: 0.0,
            sample_rate,
        }
    }

    pub fn gate_on(&mut self) {
        self.stage = EnvelopeStage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
        }
    }

    pub fn reset(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.current_level = 0.0;
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.stage != EnvelopeStage::Idle
    }

    #[inline(always)]
    pub fn next_sample(&mut self, params: &AdsrParams) -> f32 {
        match self.stage {
            EnvelopeStage::Idle => 0.0,
            EnvelopeStage::Attack => {
                let attack_samples = (params.attack * self.sample_rate).max(1.0);
                let step = 1.0 / attack_samples;
                self.current_level += step;
                if self.current_level >= 1.0 {
                    self.current_level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                }
                self.current_level
            }
            EnvelopeStage::Decay => {
                let decay_samples = (params.decay * self.sample_rate).max(1.0);
                let step = (1.0 - params.sustain) / decay_samples;
                self.current_level -= step;
                if self.current_level <= params.sustain {
                    self.current_level = params.sustain;
                    self.stage = EnvelopeStage::Sustain;
                }
                self.current_level
            }
            EnvelopeStage::Sustain => {
                self.current_level = params.sustain;
                self.current_level
            }
            EnvelopeStage::Release => {
                let release_samples = (params.release * self.sample_rate).max(1.0);
                let step = self.current_level / release_samples;
                self.current_level -= step.max(0.00001);
                if self.current_level <= 0.0001 {
                    self.current_level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
                self.current_level
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_voice_is_silent() {
        let mut v = AdsrVoice::new(48000.0);
        assert!(!v.is_active());
        assert_eq!(v.next_sample(&AdsrParams::default()), 0.0);
    }

    #[test]
    fn adsr_reaches_sustain_then_releases_to_idle() {
        let sr = 1000.0;
        let p = AdsrParams {
            attack: 0.01,
            decay: 0.01,
            sustain: 0.5,
            release: 0.01,
        };
        let mut v = AdsrVoice::new(sr);
        v.gate_on();

        let mut saw_attack = false;
        let mut saw_decay = false;
        for _ in 0..1000 {
            v.next_sample(&p);
            if v.stage == EnvelopeStage::Attack {
                saw_attack = true;
            }
            if v.stage == EnvelopeStage::Decay {
                saw_decay = true;
            }
            if v.stage == EnvelopeStage::Sustain {
                break;
            }
        }
        assert!(saw_attack && saw_decay, "should pass through attack and decay");
        assert_eq!(v.stage, EnvelopeStage::Sustain);
        assert!((v.current_level - 0.5).abs() < 1e-4, "sustain level {}", v.current_level);

        v.gate_off();
        assert_eq!(v.stage, EnvelopeStage::Release);
        for _ in 0..1000 {
            v.next_sample(&p);
            if !v.is_active() {
                break;
            }
        }
        assert!(!v.is_active(), "envelope should return to idle");
        assert_eq!(v.current_level, 0.0);
    }

    #[test]
    fn full_sensitivity_is_exactly_the_velocity() {
        // **Exakt**, inte nästan: standardvärdet 1,0 ska ge precis det tal motorn räknade
        // förut (`volume * velocity`), för ett projekt som sparades då ska låta identiskt.
        for v in [0.1, 0.25, 0.5, 0.9, 1.0] {
            assert_eq!(velocity_gain(v, 1.0), v, "velocity {v}");
        }
    }

    #[test]
    fn zero_sensitivity_ignores_the_velocity_entirely() {
        for v in [0.0, 0.1, 0.5, 1.0] {
            assert_eq!(velocity_gain(v, 0.0), 1.0, "velocity {v}");
        }
    }

    #[test]
    fn sensitivity_blends_between_off_and_the_velocity() {
        assert_eq!(velocity_gain(0.5, 0.5), 0.75);
        assert_eq!(velocity_gain(0.25, 0.5), 0.625);
    }

    #[test]
    fn out_of_range_values_are_clamped_and_never_amplify() {
        // En velocity utanför 0–1 får inte bli en förstärkare, och en känslighet utanför
        // 0–1 får inte vända regeln (negativ känslighet hade gjort en stark not svagare).
        assert_eq!(velocity_gain(-1.0, 1.0), 0.0);
        assert_eq!(velocity_gain(2.0, 1.0), 1.0);
        assert_eq!(velocity_gain(0.5, -1.0), 1.0);
        assert_eq!(velocity_gain(0.5, 5.0), 0.5);
    }

    #[test]
    fn reset_silences_active_voice() {
        let mut v = AdsrVoice::new(48000.0);
        v.gate_on();
        v.next_sample(&AdsrParams::default());
        v.reset();
        assert!(!v.is_active());
        assert_eq!(v.current_level, 0.0);
    }
}
