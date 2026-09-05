use std::f32::consts::PI;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrumType {
    Kick,
    Snare,
    Clap,
    HiHatClosed,
    HiHatOpen,
    Crash,
    TomLow,
    TomHigh,
}

#[derive(Debug, Clone, Copy)]
pub struct DrumVoice {
    pub drum_type: DrumType,
    pub active: bool,
    pub time: f32,
    pub sample_rate: f32,
    pub rng_state: u32,
}

impl DrumVoice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            drum_type: DrumType::Kick,
            active: false,
            time: 0.0,
            sample_rate,
            rng_state: 123456789,
        }
    }

    pub fn trigger(&mut self, drum_type: DrumType) {
        self.drum_type = drum_type;
        self.active = true;
        self.time = 0.0;
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.time = 0.0;
    }

    fn next_noise(&mut self) -> f32 {
        // Fast Xorshift pseudo-random number generator
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[inline(always)]
    pub fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let dt = 1.0 / self.sample_rate;
        self.time += dt;

        let sample = match self.drum_type {
            DrumType::Kick => {
                let duration = 0.35;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                // Pitch envelope: drops from 160Hz to 45Hz
                let pitch_env = (-self.time * 28.0).exp();
                let freq = 45.0 + 115.0 * pitch_env;
                let phase = 2.0 * PI * freq * self.time;
                let amp_env = (-self.time * 9.0).exp();
                let click = if self.time < 0.005 { 0.4 * self.next_noise() } else { 0.0 };
                (phase.sin() * 1.2 + click) * amp_env
            }
            DrumType::Snare => {
                let duration = 0.25;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                // Tone component + Noise component
                let tone_env = (-self.time * 22.0).exp();
                let tone = (2.0 * PI * 185.0 * self.time).sin() * tone_env * 0.6;
                let noise_env = (-self.time * 15.0).exp();
                let noise = self.next_noise() * noise_env * 0.8;
                tone + noise
            }
            DrumType::Clap => {
                let duration = 0.28;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                // Multi-pulse burst transient for authentic handclap
                let pulse = if self.time < 0.012 {
                    1.0
                } else if self.time < 0.024 {
                    0.8
                } else if self.time < 0.038 {
                    1.2
                } else {
                    (- (self.time - 0.038) * 18.0).exp()
                };
                self.next_noise() * pulse * 0.85
            }
            DrumType::HiHatClosed => {
                let duration = 0.06;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                let env = (-self.time * 75.0).exp();
                self.next_noise() * env * 0.7
            }
            DrumType::HiHatOpen => {
                let duration = 0.35;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                let env = (-self.time * 12.0).exp();
                self.next_noise() * env * 0.6
            }
            DrumType::Crash => {
                let duration = 1.2;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                let env = (-self.time * 3.5).exp();
                let noise = self.next_noise() * 0.6;
                let ring = (2.0 * PI * 820.0 * self.time).sin() * 0.2 + (2.0 * PI * 1240.0 * self.time).sin() * 0.2;
                (noise + ring) * env
            }
            DrumType::TomLow => {
                let duration = 0.45;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                let pitch_env = (-self.time * 18.0).exp();
                let freq = 65.0 + 80.0 * pitch_env;
                let amp_env = (-self.time * 7.0).exp();
                (2.0 * PI * freq * self.time).sin() * amp_env * 0.9
            }
            DrumType::TomHigh => {
                let duration = 0.35;
                if self.time >= duration {
                    self.active = false;
                    return 0.0;
                }
                let pitch_env = (-self.time * 22.0).exp();
                let freq = 120.0 + 130.0 * pitch_env;
                let amp_env = (-self.time * 9.0).exp();
                (2.0 * PI * freq * self.time).sin() * amp_env * 0.9
            }
        };

        sample
    }
}
