#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
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
    fn reset_silences_active_voice() {
        let mut v = AdsrVoice::new(48000.0);
        v.gate_on();
        v.next_sample(&AdsrParams::default());
        v.reset();
        assert!(!v.is_active());
        assert_eq!(v.current_level, 0.0);
    }
}
