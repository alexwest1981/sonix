//! **Riff-generator** (FL): en fras ur en skala, i stället för en tom ruta.
//!
//! Generatorn är avsiktligt liten: den väljer toner ur en skala, håller sig inom ett omfång och
//! lägger anslagen på ett rutnät. Den gör inget som en musiker inte skulle göra första gången —
//! poängen är att komma igång, och att resultatet går att spela in över.

use crate::midi_take::{Take, STEPS_PER_BAR};
use crate::rng::Rng;

/// Skalorna generatorn kan välja ur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Major,
    Minor,
    Dorian,
    Pentatonic,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::Major, Scale::Minor, Scale::Dorian, Scale::Pentatonic];

    /// Avstånden i halvtoner från grundtonen.
    pub fn steps(self) -> &'static [u8] {
        match self {
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Pentatonic => &[0, 3, 5, 7, 10],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scale::Major => "Dur",
            Scale::Minor => "Moll",
            Scale::Dorian => "Dorisk",
            Scale::Pentatonic => "Pentatonisk",
        }
    }
}

/// Vad generatorn ska göra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiffParams {
    pub root: u8,
    pub scale: Scale,
    /// Hur många anslag i takten (1–16). Tagningen är en takt, inte flera.
    pub notes_per_bar: usize,
    /// Hur långt ifrån grundtonen frasen får gå, i skalsteg.
    pub range_steps: usize,
    pub seed: u64,
}

impl Default for RiffParams {
    fn default() -> Self {
        Self {
            root: 36,
            scale: Scale::Minor,
            notes_per_bar: 8,
            range_steps: 7,
            seed: 1,
        }
    }
}

/// **Generera frasen.** Samma frö ger samma fras, varje gång — annars går den inte att prova.
pub fn generate(params: &RiffParams) -> Take {
    let mut take = Take::new();
    let per_bar = params.notes_per_bar.clamp(1, 16);
    let scale = params.scale.steps();
    let range = params.range_steps.max(1);
    let mut rng = Rng::new(params.seed);

    // Rutnätet är 16 steg; anslagen fördelas jämnt över takten.
    let step = STEPS_PER_BAR / per_bar as f32;
    for i in 0..per_bar {
        // Ett anslag per position, men inte på varje: frasen ska andas.
        if i > 0 && rng.next_f32() < 0.18 {
            continue;
        }
        let degree = (rng.next_f32() * range as f32) as usize;
        let octave = degree / scale.len();
        let key = params.root as i32 + scale[degree % scale.len()] as i32 + 12 * octave as i32;
        let key = key.clamp(0, 127) as u8;
        // Anslaget varierar, men inte slumpmässigt: tunga slag starkare, som en spelad fras.
        let velocity = if i % 2 == 0 { 0.85 } else { 0.65 } + rng.next_f32() * 0.1;
        take.push(i as f32 * step, key, velocity);
    }
    take
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Samma frö ger samma fras, och tonerna håller sig i skalan.** Det är de två löftena:
    /// ett slumpat riff som inte går att upprepa är ett prov som inte går att lita på.
    #[test]
    fn the_riff_is_repeatable_and_stays_in_its_scale() {
        let params = RiffParams::default();
        let a = generate(&params);
        let b = generate(&params);
        assert_eq!(a.notes, b.notes, "samma frö ska ge samma fras");
        assert!(!a.notes.is_empty(), "en fras ska ha noter");

        let steps = params.scale.steps();
        for n in &a.notes {
            let semis = (n.key as i32 - params.root as i32).rem_euclid(12) as u8;
            assert!(steps.contains(&semis), "ton {} ligger utanför skalan", n.key);
            assert!(n.pos >= 0.0 && n.pos < STEPS_PER_BAR);
        }

        // Ett annat frö ska ge en annan fras — annars vore fröet en prydnad.
        let c = generate(&RiffParams { seed: 99, ..params });
        assert_ne!(a.notes, c.notes);
    }

    /// **Tätare fras ger fler noter, och de ligger utspridda över takten** — en generator som
    /// tryckte allt på steg 0 vore tyst i praktiken.
    #[test]
    fn a_denser_riff_has_more_notes_spread_over_the_bar() {
        let sparse = generate(&RiffParams { notes_per_bar: 4, ..RiffParams::default() });
        let dense = generate(&RiffParams { notes_per_bar: 16, ..RiffParams::default() });
        assert!(dense.notes.len() > sparse.notes.len(), "16 anslag ska ge fler än 4");
        let last = dense.notes.iter().map(|n| n.pos).fold(0.0f32, f32::max);
        assert!(last > STEPS_PER_BAR * 0.75, "frasen ska nå slutet av takten: {last}");
        assert!(dense.notes.iter().all(|n| n.pos >= 0.0 && n.pos < STEPS_PER_BAR));
    }
}
