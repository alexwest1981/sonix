//! **Control Room**: det man lyssnar genom, inte det man exporterar.
//!
//! Monitorsektionen sitter efter masterns FX-kedja och rör bara det som går till högtalarna:
//! nivå, dim (−20 dB), mono och tyst. Exporten skapar en egen motor utan dessa kommandon, så
//! en dimmad monitor kan aldrig smyga sig in i en renderad fil.

/// Monitorläget. `Default` är neutralt — det är vad exportmotorn alltid ser.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlRoom {
    /// 0,0–2,0 (1,0 = oförändrat).
    pub level: f32,
    pub dim: bool,
    pub mono: bool,
    pub mute: bool,
}

impl Default for ControlRoom {
    fn default() -> Self {
        Self {
            level: 1.0,
            dim: false,
            mono: false,
            mute: false,
        }
    }
}

impl ControlRoom {
    /// Dämpningen i dim-läget: −20 dB, det vill säga en tiondel.
    pub const DIM_GAIN: f32 = 0.1;

    /// Den sammanlagda förstärkningen till monitorerna.
    pub fn gain(&self) -> f32 {
        if self.mute {
            0.0
        } else {
            self.level.clamp(0.0, 2.0) * if self.dim { Self::DIM_GAIN } else { 1.0 }
        }
    }

    /// Monitorsteget: mono lägger ihop sidorna **före** förstärkningen, så en dimmad mono
    /// inte tappar 3 dB mot en dimmad stereo.
    pub fn apply(&self, left: f32, right: f32) -> (f32, f32) {
        let gain = self.gain();
        if self.mono {
            let mid = (left + right) * 0.5;
            (mid * gain, mid * gain)
        } else {
            (left * gain, right * gain)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_monitor_changes_nothing() {
        let cr = ControlRoom::default();
        assert_eq!(cr.gain(), 1.0);
        assert_eq!(cr.apply(0.5, -0.25), (0.5, -0.25));
    }

    #[test]
    fn dim_is_twenty_db_and_mute_is_silence() {
        assert!((ControlRoom { dim: true, ..Default::default() }.gain() - 0.1).abs() < 1e-6);
        assert_eq!(ControlRoom { mute: true, ..Default::default() }.gain(), 0.0);
        // Tyst vinner över dim: ingen av dem ska kunna "låsa upp" ljud.
        assert_eq!(ControlRoom { mute: true, dim: true, ..Default::default() }.gain(), 0.0);
    }

    #[test]
    fn mono_sums_the_sides_after_the_monitor_level() {
        let cr = ControlRoom { mono: true, level: 2.0, ..Default::default() };
        let (l, r) = cr.apply(1.0, 0.0);
        assert!((l - 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6, "{l} {r}");
        // Sidorna är identiska i mono, även när ingången inte är det.
        let (l, r) = cr.apply(0.2, -0.6);
        assert_eq!(l, r);
    }

    #[test]
    fn the_level_is_clamped_so_a_typo_cannot_blow_the_speakers() {
        assert_eq!(ControlRoom { level: 50.0, ..Default::default() }.gain(), 2.0);
        assert_eq!(ControlRoom { level: -3.0, ..Default::default() }.gain(), 0.0);
    }
}
