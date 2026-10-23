//! **VFX Script (lite)**: en formel blir en automationskurva.
//!
//! FL:s VFX Script är en hel skriptmotor med eget GUI. Det här är den del som går att göra
//! utan att bädda in ett interpreter: formelspråket som redan finns i [`crate::formula`]
//! utvärderas en gång per punkt och skrivs in i en automationsbana. Resultatet är en kurva
//! som går att se, spara och rätta för hand efteråt — ingen svart låda.
//!
//! `t` är **taktslag**, inte sekunder: en formel ska betyda samma sak oavsett tempo.

use crate::ui::app::state::AutomationPoint;

/// Standardformeln: en mjuk våg, en period per takt.
pub const DEFAULT_SOURCE: &str = "sin(t * 6.283)";

/// Räknar ut kurvan: `bars` takter med `per_bar` punkter per takt.
///
/// Formelns värde ligger i −1…1 (det är språkets eget spann), så det mappas till 0…1 som
/// automationsbanan använder. Ett tolkningsfel lämnas vidare som en läsbar mening.
pub fn points_from_formula(
    src: &str,
    bars: usize,
    per_bar: usize,
) -> Result<Vec<AutomationPoint>, String> {
    let formula = crate::formula::Formula::compile(src)?;
    let per_bar = per_bar.clamp(1, 64);
    let bars = bars.clamp(1, 32);
    let mut points = Vec::with_capacity(bars * per_bar);
    for i in 0..bars * per_bar {
        let t = i as f32 / per_bar as f32;
        points.push(AutomationPoint {
            time_bars: t,
            value: (formula.eval(t, 0.0) * 0.5 + 0.5).clamp(0.0, 1.0),
        });
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Formeln blir en kurva**: `t` är takter, värdet mappas från −1…1 till 0…1, och
    /// kurvan har en punkt per steg — inte en per takt.
    #[test]
    fn a_formula_becomes_one_point_per_step() {
        let points = points_from_formula("t", 2, 4).unwrap();
        assert_eq!(points.len(), 8);
        assert_eq!(points[0].time_bars, 0.0);
        assert!((points[0].value - 0.5).abs() < 1e-5, "t=0 ger mitten");
        // Formelns eget spann är −1…1, så t=1 kläms till 1 innan mappningen.
        assert!((points[4].value - 1.0).abs() < 1e-5, "t=1 ger toppen");
        assert_eq!(points[7].time_bars, 1.75);

        // En hel period per takt: kvart in i takt 1 är toppen.
        let wave = points_from_formula(DEFAULT_SOURCE, 1, 8).unwrap();
        assert!((wave[2].value - 1.0).abs() < 1e-3, "toppen: {}", wave[2].value);
        assert!((wave[6].value - 0.0).abs() < 1e-3, "dalen: {}", wave[6].value);
    }

    #[test]
    fn values_are_clamped_and_bad_syntax_is_reported() {
        let loud = points_from_formula("100", 1, 2).unwrap();
        assert!(loud.iter().all(|p| p.value == 1.0));
        let quiet = points_from_formula("-100", 1, 2).unwrap();
        assert!(quiet.iter().all(|p| p.value == 0.0));

        let err = points_from_formula("sin(", 1, 4).unwrap_err();
        assert!(!err.is_empty(), "felet ska gå att visa: {err}");
    }
}
