//! **Reparation av inspelat material** — det FL kallar "klicka bort skarvar".
//!
//! En klick i en inspelning är ett hopp mellan två närliggande sampel. Att rätta det genom att
//! sänka toppen dämpar också musiken; att rätta det genom att interpolera över hoppet lagar
//! bara hoppet. Det är därför det här är en egen liten sak och inte en limiter.

/// Ett hopp större än så här mellan två sampel räknas som en klick. 0,25 är en fjärdedel av
/// full skala mellan två sampel — musik rör sig sällan så långt på 20 µs, medan en dålig klipp
/// gör det.
pub const CLICK_THRESHOLD: f32 = 0.25;

/// Längden på det interpolerade stycket, i sampel. 32 sampel vid 48 kHz är 0,67 ms: kortare än
/// en klick hörs, längre än en period i diskanten.
pub const REPAIR_SAMPLES: usize = 32;

/// **Laga klickar i en kanal.** Returnerar antalet lagade hopp.
///
/// Varje hopp ersätts av en rät linje mellan samplet före och samplet efter, så att materialet
/// på båda sidor står orört — det är hela poängen med att laga i stället för att dämpa.
pub fn declick(channel: &mut [f32]) -> usize {
    let mut repaired = 0;
    let mut n = 1;
    while n < channel.len() {
        let jump = (channel[n] - channel[n - 1]).abs();
        if jump > CLICK_THRESHOLD {
            // Klickets slut: första samplet där signalen planar ut igen, eller fönstrets slut.
            let start = n;
            let end = (n + REPAIR_SAMPLES).min(channel.len() - 1);
            let (a, b) = (channel[start - 1], channel[end]);
            let span = (end - start + 1) as f32;
            for (i, k) in (start..=end).enumerate() {
                let t = (i + 1) as f32 / span;
                channel[k] = a + (b - a) * t;
            }
            repaired += 1;
            n = end + 1;
        } else {
            n += 1;
        }
    }
    repaired
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Klickarna försvinner och musiken står kvar.** Provet mäter det som definierar en klick
    /// — största hoppet mellan två sampel — och kräver att resten av signalen är orörd.
    #[test]
    fn declick_removes_the_jumps_and_leaves_the_rest_alone() {
        let sr = 48_000.0;
        let mut v: Vec<f32> = (0..4_800)
            .map(|n| (2.0 * std::f32::consts::PI * 440.0 * n as f32 / sr).sin() * 0.5)
            .collect();
        let clean = v.clone();
        // Tre klickar: tvära hopp som inte hör till sinusen.
        for k in [1_000usize, 2_500, 4_000] {
            v[k] = -v[k];
        }
        let max_jump = |s: &[f32]| {
            s.windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max)
        };
        let before = max_jump(&v);

        let repaired = declick(&mut v);
        let after = max_jump(&v);

        println!("största hopp före {before:.3} efter {after:.3}, lagade {repaired}");
        assert_eq!(repaired, 3, "tre klickar in, tre lagade");
        assert!(before > CLICK_THRESHOLD, "provet måste börja med klickar");
        assert!(after < before * 0.2, "hoppen ska bort: {before:.3} → {after:.3}");

        // Utanför de lagade fönstren ska signalen vara exakt som förut.
        let touched = |i: usize| [1_000, 2_500, 4_000].iter().any(|k| i >= *k && i < k + REPAIR_SAMPLES + 1);
        for i in 0..clean.len() {
            if !touched(i) {
                assert_eq!(v[i], clean[i], "sampel {i} skulle stå orört");
            }
        }
    }

    /// **Ren musik lagas inte.** Ett prov som lagar allt vore ett prov som dämpar.
    #[test]
    fn declick_leaves_clean_audio_untouched() {
        let sr = 48_000.0;
        let mut v: Vec<f32> = (0..4_800)
            .map(|n| (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin() * 0.9)
            .collect();
        let before = v.clone();
        assert_eq!(declick(&mut v), 0);
        assert_eq!(v, before);
    }
}
