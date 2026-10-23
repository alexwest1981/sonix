//! **Färgkartan** (FL:s VFX Color Mapper): notens tonhöjd → färg.
//!
//! Bara tolv färger, en per tonklass, så att en färg alltid betyder samma ton i varje oktav.
//! Mättnaden är dämpad: rutnätet ska gå att läsa, inte blända.

use eframe::egui::{self, Color32};

/// Färgen för en tangent. Samma tonklass ger samma färg i alla oktaver.
pub fn pitch_color(key: u8) -> Color32 {
    let hue = (key % 12) as f32 / 12.0;
    // Ljushet per tonklass så att C, E och G (ackordtonerna) syns starkare än ledtonerna.
    let value = match key % 12 {
        0 => 0.95,
        4 | 7 => 0.85,
        2 | 5 | 9 | 11 => 0.55,
        _ => 0.7,
    };
    egui::ecolor::Hsva::new(hue, 0.72, value, 1.0).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_same_pitch_class_has_the_same_colour_in_every_octave() {
        for key in 0..12u8 {
            assert_eq!(pitch_color(key), pitch_color(key + 12));
            assert_eq!(pitch_color(key), pitch_color(key + 60));
        }
    }

    #[test]
    fn the_twelve_pitch_classes_are_twelve_colours() {
        let colours: HashSet<[u8; 4]> = (0..12u8)
            .map(|key| pitch_color(key + 48).to_array())
            .collect();
        assert_eq!(colours.len(), 12, "en tonklass får inte dela färg med en annan");
    }

    #[test]
    fn the_root_is_brighter_than_a_leading_tone() {
        // C (0) har value 0,95, B (11) har 0,55 — alltså ljusare.
        let c = pitch_color(60);
        let b = pitch_color(71);
        assert!(c.r() as u32 + c.g() as u32 + c.b() as u32
            > b.r() as u32 + b.g() as u32 + b.b() as u32);
    }
}
