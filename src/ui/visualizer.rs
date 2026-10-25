//! **Visualizer** (ZGameEditor-lite): vågform, spektrumstaplar och en topp-markör.
//!
//! Ingen egen ljudanalys: datat finns redan i appen (`scope_history` och `spectrum_levels`,
//! som mixern ritar) och det här är bara en större och ljusare bild av samma siffror. Att
//! analysera om samma buffer vore att räkna FFT två gånger per bildruta.

/// Toppmarkörens fall. Den håller i sig en stund och faller sedan lugnt — en markör som
/// följer nivån exakt är bara en andra stapel och säger ingenting.
pub fn decay_peak(peak: &mut [f32], levels: &[f32], fall_per_frame: f32) {
    for (i, p) in peak.iter_mut().enumerate() {
        let level = levels.get(i).copied().unwrap_or(0.0);
        *p = (*p - fall_per_frame).max(level).clamp(0.0, 1.0);
    }
}

/// Höjden på en stapel i pixlar, med en minsta synlig höjd för tysta band.
pub fn bar_height(level: f32, height: f32) -> f32 {
    (level.clamp(0.0, 1.0) * height).max(1.0)
}


/// **Goniometer**: en punkt per sampel, roterad 45°. Mono hamnar på den lodräta linjen
/// (L == R), helt ur fas på den vågräta — det är hela läsbarheten i bilden.
pub fn goniometer_xy(left: f32, right: f32) -> (f32, f32) {
    let k = std::f32::consts::FRAC_1_SQRT_2;
    ((left - right) * k, (left + right) * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Toppen faller, men bara till nivån.** En markör under nivån lyfts direkt, en över
    /// faller med ett fast steg per bildruta — och aldrig under golvet.
    #[test]
    fn the_peak_marker_falls_but_not_through_the_level() {
        let mut peaks = [0.9, 0.2, 0.0];
        let levels = [0.4, 0.5, 0.0];

        decay_peak(&mut peaks, &levels, 0.1);
        assert!((peaks[0] - 0.8).abs() < 1e-6, "toppen faller ett steg: {}", peaks[0]);
        assert!((peaks[1] - 0.5).abs() < 1e-6, "under nivån lyfts den: {}", peaks[1]);
        assert_eq!(peaks[2], 0.0);

        for _ in 0..20 {
            decay_peak(&mut peaks, &levels, 0.1);
        }
        assert!((peaks[0] - 0.4).abs() < 1e-6, "den stannar på nivån: {}", peaks[0]);
    }

    #[test]
    fn a_silent_band_still_gets_a_visible_line() {
        assert_eq!(bar_height(0.0, 100.0), 1.0);
        assert_eq!(bar_height(0.5, 100.0), 50.0);
        assert_eq!(bar_height(3.0, 100.0), 100.0, "över 1 kläms");
    }

    #[test]
    fn a_mono_signal_sits_on_the_vertical_line() {
        let (x, y) = super::goniometer_xy(0.5, 0.5);
        assert!(x.abs() < 1e-6, "mono ska ha x = 0: {x}");
        assert!(y > 0.0);
    }

    #[test]
    fn a_fully_out_of_phase_signal_sits_on_the_horizontal_line() {
        let (x, y) = super::goniometer_xy(0.5, -0.5);
        assert!(y.abs() < 1e-6, "ur fas ska ha y = 0: {y}");
        assert!(x > 0.0);
    }
}
