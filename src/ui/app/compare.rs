//! A/B-jämförelse av mixen — Abletons `Compare Copy` / `Compare Switch`.
//!
//! En `MixSnapshot` är mixen fryst till siffror (volym, pan, mute per spår + masternivå).
//! `Compare` håller två sådana och växlar vilken som gäller; `apply_to` lämnar ut
//! **(spårindex, volym, pan, mute)** så anroparen kan sätta värdena utan att känna till spåren.
//!
//! Status: byggs — ren data, inget egui, så den går att pröva utan fönster.
//! Rör inte: `switch` byter bara när **båda** sidorna finns, annars står jämförelsen still.

#[derive(Clone, Debug, PartialEq)]
pub struct MixSnapshot {
    /// Namnet användaren ser, t.ex. "A" eller "B".
    pub name: String,
    /// En triplett per spår, i projektets ordning: (volym, pan, mute).
    pub tracks: Vec<(f32, f32, bool)>,
    /// Masternivån.
    pub master: f32,
}

#[derive(Default)]
pub struct Compare {
    a: Option<MixSnapshot>,
    b: Option<MixSnapshot>,
    /// Sant när A är den som hörs (efter ett `switch`).
    pub active: bool,
}

impl Compare {
    pub fn capture_a(&mut self, snapshot: MixSnapshot) {
        self.a = Some(snapshot);
    }

    pub fn capture_b(&mut self, snapshot: MixSnapshot) {
        self.b = Some(snapshot);
    }

    /// Växlar aktiv sida och lämnar den som ska appliceras: A först, B sedan.
    /// Utan båda sidorna händer ingenting — ingen halv jämförelse.
    pub fn switch(&mut self) -> Option<&MixSnapshot> {
        if self.a.is_none() || self.b.is_none() {
            return None;
        }
        self.active = !self.active;
        if self.active {
            self.a.as_ref()
        } else {
            self.b.as_ref()
        }
    }

    /// Värdena att sätta, indexerade mot spåren i snapshotet.
    pub fn apply_to(&self, snapshot: &MixSnapshot) -> Vec<(usize, f32, f32, bool)> {
        snapshot
            .tracks
            .iter()
            .enumerate()
            .map(|(i, &(vol, pan, mute))| (i, vol, pan, mute))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(name: &str, vol: f32) -> MixSnapshot {
        MixSnapshot {
            name: name.to_string(),
            tracks: vec![(vol, -0.5, false), (vol * 0.5, 0.25, true)],
            master: vol * 2.0,
        }
    }

    #[test]
    fn ett_prov_per_operation() {
        let mut cmp = Compare::default();
        let a = snap("A", 1.0);
        let b = snap("B", 0.5);

        // A fångas
        cmp.capture_a(a.clone());
        assert_eq!(cmp.a.as_ref(), Some(&a));

        // B fångas
        cmp.capture_b(b.clone());
        assert_eq!(cmp.b.as_ref(), Some(&b));

        // switch ger A först och B sedan
        assert_eq!(cmp.switch().map(|s| s.name.as_str()), Some("A"));
        assert_eq!(cmp.switch().map(|s| s.name.as_str()), Some("B"));

        // apply_to ger rätt tripletter
        assert_eq!(
            cmp.apply_to(&b),
            vec![(0, 0.5, -0.5, false), (1, 0.25, 0.25, true)]
        );

        // switch utan båda sidor ger None
        let mut ensam = Compare::default();
        ensam.capture_a(a);
        assert_eq!(ensam.switch(), None);
        assert!(!ensam.active);
        assert_eq!(Compare::default().switch(), None);
    }
}
