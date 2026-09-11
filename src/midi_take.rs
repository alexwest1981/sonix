//! Inspelade nottagningar med sin faktiska tajming (Fas 6.4).
//!
//! Varför modulen finns: appens rutnät är 16 steg av/på och säger ingenting om
//! *när* inom steget en ton spelades. Live-inspelningen skriver noten vid
//! steggränsen (`record_midi_note_at_step`) — en kvantisering som sker redan vid
//! inmatningen, utan att någon otajt data sparas. Det som saknades för
//! kvantisering och humanisering är alltså själva tagningen.
//!
//! Här bor den som rena funktioner: ingen GUI, ingen ljudmotor. Positionen
//! räknas i **steg** (en takt = 16 steg), där 0.0 är taktens början. En not på
//! 2.4 spelades 40 % av ett steg efter steg 2.

/// Antal steg i en takt (4/4, 16-delar).
pub const STEPS_PER_BAR: f32 = 16.0;

/// Hur långt fram en udda gridlinje flyttas vid swing = 1.0. En tredjedels steg
/// ger den klassiska triolfeelingen (2 mot 3).
pub const SWING_MAX_STEPS: f32 = 0.33;

/// En inspelad not.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TakeNote {
    /// Position i takten i steg. 0.0 = taktens början, 15.99 = slutet.
    pub pos: f32,
    /// MIDI-notnummer.
    pub key: u8,
    /// Anslag 0.0–1.0.
    pub velocity: f32,
}

/// En tagning: noterna som spelades, med sin tajming.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Take {
    pub notes: Vec<TakeNote>,
}

/// Liten deterministisk slumptalare (xorshift64*). Egen i stället för ett nytt
/// beroende, och framför allt för att humaniseringen ska gå att testa: samma
/// frö ger samma resultat, varje gång.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // 0 är en fast punkt i xorshift och skulle ge samma tal för alltid.
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// jämnt fördelat i [0, 1)
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// jämnt fördelat i [-1, 1)
    pub fn next_sym(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

impl Take {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, pos: f32, key: u8, velocity: f32) {
        self.notes.push(TakeNote {
            pos: pos.clamp(0.0, STEPS_PER_BAR - 0.01),
            key,
            velocity: velocity.clamp(0.0, 1.0),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Rutnätssteget en not hör till (närmaste steg), 0–15.
    pub fn slot(note: &TakeNote) -> usize {
        (note.pos.round() as i32).clamp(0, 15) as usize
    }

    /// Hur otajt tagningen är: medelavståndet från noterna till närmaste
    /// rutnätslinje, i steg. 0.0 = perfekt på rutnätet, 0.30 = slarvigt spelat.
    /// Detta är det mätbara svaret på "blev det bättre?".
    pub fn tightness(&self) -> f32 {
        if self.notes.is_empty() {
            return 0.0;
        }
        let sum: f32 = self
            .notes
            .iter()
            .map(|n| (n.pos - n.pos.round()).abs())
            .sum();
        sum / self.notes.len() as f32
    }

    /// Var en rutnätslinje ligger med sväng inräknad: udda 16-delar skjuts fram.
    pub fn swung_line(line: usize, swing: f32) -> f32 {
        if line % 2 == 1 {
            line as f32 + swing.clamp(0.0, 1.0) * SWING_MAX_STEPS
        } else {
            line as f32
        }
    }

    /// Rutnätslinjen som ligger närmast `pos`, med sväng inräknad.
    pub fn nearest_line(pos: f32, swing: f32) -> f32 {
        let mut best = 0.0f32;
        let mut best_d = f32::MAX;
        for line in 0..STEPS_PER_BAR as usize {
            let target = Self::swung_line(line, swing);
            let d = (target - pos).abs();
            if d < best_d {
                best_d = d;
                best = target;
            }
        }
        best
    }

    /// Kvantisering: dra varje not mot sin närmaste rutnätslinje.
    ///
    /// `strength` 0.0 = ingen ändring, 1.0 = exakt på rutnätet (då är tagningen
    /// så tight den kan bli). `swing` 0.0 = rakt, 1.0 = triolkänsla.
    pub fn quantize(&mut self, strength: f32, swing: f32) {
        let strength = strength.clamp(0.0, 1.0);
        for n in &mut self.notes {
            let target = Self::nearest_line(n.pos, swing);
            n.pos += (target - n.pos) * strength;
            n.pos = n.pos.clamp(0.0, STEPS_PER_BAR - 0.01);
        }
    }

    /// Humanisering: lägg på lite mänsklig otajthet och dynamik, medvetet.
    ///
    /// `timing_steps` = största tidsavvikelse i steg, `velocity_amount` = största
    /// anslagsavvikelse. `seed` styr slumptalet så att samma tagning + samma frö
    /// ger samma resultat — annars vore funktionen omöjlig att testa.
    pub fn humanize(&mut self, timing_steps: f32, velocity_amount: f32, seed: u64) {
        let mut rng = Rng::new(seed);
        let timing = timing_steps.max(0.0);
        let vel = velocity_amount.max(0.0);
        for n in &mut self.notes {
            n.pos = (n.pos + rng.next_sym() * timing).clamp(0.0, STEPS_PER_BAR - 0.01);
            n.velocity = (n.velocity + rng.next_sym() * vel).clamp(0.05, 1.0);
        }
    }

    /// Stegen som ska trigga noten, och hur långt in i steget den ska klinga.
    ///
    /// Noten spelas från steget *före* sin rutnätslinje när den ligger tidigt
    /// (2.7 → triggas av steg 2, 0.7 steg in), så en not som spelades före
    /// slaget hamnar före slaget även i uppspelningen. Returen är (steg, andel av
    /// steget).
    pub fn playback_slot(note: &TakeNote) -> (usize, f32) {
        let floor = note.pos.floor().clamp(0.0, STEPS_PER_BAR - 1.0);
        (floor as usize, note.pos - floor)
    }
}

/// Positionen en not får i tagningen, ur sekvenserns steg och fasen inom steget.
///
/// Ren funktion (Fas 6.4) så att räkningen kan testas: `step` 3 med fasen 0.25
/// blir position 3.25, det vill säga en fjärdedels steg efter steg 3.
pub fn take_pos_from(step: usize, phase: f32) -> f32 {
    (step as f32 + phase.clamp(0.0, 0.999)).clamp(0.0, STEPS_PER_BAR - 0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tight_take() -> Take {
        let mut t = Take::new();
        t.push(0.0, 36, 0.9);
        t.push(4.0, 38, 0.8);
        t.push(8.0, 36, 0.85);
        t.push(12.0, 38, 0.7);
        t
    }

    #[test]
    fn a_tight_take_measures_as_tight() {
        assert_eq!(tight_take().tightness(), 0.0);
    }

    #[test]
    fn tightness_is_the_mean_distance_to_the_grid() {
        let mut t = Take::new();
        t.push(2.25, 36, 0.9);
        t.push(6.0, 38, 0.9);
        t.push(9.75, 40, 0.9);
        // Avstånden är 0.25, 0.0 och 0.25 → 0.5/3.
        assert!((t.tightness() - 0.5 / 3.0).abs() < 1e-6, "{}", t.tightness());
    }

    #[test]
    fn full_strength_quantize_snaps_to_the_grid() {
        let mut t = Take::new();
        t.push(0.4, 36, 0.9);
        t.push(4.1, 38, 0.9);
        t.quantize(1.0, 0.0);
        assert_eq!(t.notes[0].pos, 0.0);
        assert_eq!(t.notes[1].pos, 4.0);
        assert_eq!(t.tightness(), 0.0);
    }

    #[test]
    fn half_strength_moves_halfway() {
        let mut t = Take::new();
        t.push(2.4, 36, 0.9);
        t.quantize(0.5, 0.0);
        assert!((t.notes[0].pos - 2.2).abs() < 1e-6, "{}", t.notes[0].pos);
    }

    #[test]
    fn quantize_moves_to_the_nearest_line_in_both_directions() {
        let mut t = Take::new();
        t.push(2.7, 36, 0.9); // närmare 3 än 2
        t.push(2.2, 38, 0.9); // närmare 2
        t.quantize(1.0, 0.0);
        assert_eq!(t.notes[0].pos, 3.0, "en släpande not dras framåt");
        assert_eq!(t.notes[1].pos, 2.0, "en tidig not dras bakåt");
    }

    #[test]
    fn zero_strength_changes_nothing() {
        let mut t = Take::new();
        t.push(2.4, 36, 0.9);
        let before = t.clone();
        t.quantize(0.0, 0.0);
        assert_eq!(t, before);
    }

    #[test]
    fn swing_delays_the_odd_lines() {
        assert_eq!(Take::swung_line(0, 1.0), 0.0);
        assert_eq!(Take::swung_line(2, 1.0), 2.0);
        assert!((Take::swung_line(1, 1.0) - (1.0 + SWING_MAX_STEPS)).abs() < 1e-6);
        assert_eq!(Take::swung_line(1, 0.0), 1.0, "utan sväng ligger linjen rakt");

        // En not som spelades rakt på en udda 16-del hamnar efter linjen med sväng.
        let mut t = Take::new();
        t.push(1.0, 36, 0.9);
        t.quantize(1.0, 1.0);
        assert!((t.notes[0].pos - (1.0 + SWING_MAX_STEPS)).abs() < 1e-6);
    }

    #[test]
    fn swing_keeps_even_lines_where_they_are() {
        let mut t = Take::new();
        t.push(0.05, 36, 0.9);
        t.push(4.05, 38, 0.9);
        t.quantize(1.0, 1.0);
        assert_eq!(t.notes[0].pos, 0.0);
        assert_eq!(t.notes[1].pos, 4.0);
    }

    #[test]
    fn humanize_is_deterministic_for_a_given_seed() {
        let mut a = tight_take();
        let mut b = tight_take();
        a.humanize(0.08, 0.15, 12345);
        b.humanize(0.08, 0.15, 12345);
        assert_eq!(a, b, "samma frö ska ge samma resultat");
        assert_ne!(a, tight_take(), "något ska hända");

        let mut c = tight_take();
        c.humanize(0.08, 0.15, 999);
        assert_ne!(c, a, "olika frö ska ge olika tagning");
    }

    #[test]
    fn humanize_stays_inside_the_bar() {
        let mut t = Take::new();
        t.push(0.0, 36, 0.5);
        t.push(15.99, 38, 0.5);
        t.humanize(0.5, 0.5, 7);
        for n in &t.notes {
            assert!((0.0..STEPS_PER_BAR).contains(&n.pos), "utanför takten: {}", n.pos);
            assert!((0.05..=1.0).contains(&n.velocity), "anslag utanför: {}", n.velocity);
        }
    }

    #[test]
    fn humanize_varies_the_velocity_within_the_amount() {
        let mut t = tight_take();
        t.humanize(0.0, 0.1, 4242);
        for (n, orig) in t.notes.iter().zip(tight_take().notes.iter()) {
            assert!((n.velocity - orig.velocity).abs() <= 0.1 + 1e-6);
        }
        assert_ne!(t, tight_take(), "anslagen ska varieras");
    }

    #[test]
    fn quantizing_a_humanized_take_returns_it_to_the_grid() {
        // Hela kedjan: tight → humaniserad → hårt kvantiserad är tight igen.
        let mut t = tight_take();
        t.humanize(0.12, 0.2, 5);
        assert!(t.tightness() > 0.0, "humaniseringen ska synas på tajmingen");
        t.quantize(1.0, 0.0);
        assert_eq!(t.tightness(), 0.0);
        for (n, orig) in t.notes.iter().zip(tight_take().notes.iter()) {
            assert_eq!(n.pos, orig.pos, "tillbaka på samma steg");
        }
    }

    #[test]
    fn a_small_humanize_keeps_the_notes_on_their_slots() {
        // "Bevarad karaktär": en måttlig humanisering får inte flytta noter till
        // ett annat steg — bara göra dem mänskliga.
        let mut t = tight_take();
        let slots_before: Vec<usize> = t.notes.iter().map(Take::slot).collect();
        t.humanize(0.15, 0.1, 31337);
        let slots_after: Vec<usize> = t.notes.iter().map(Take::slot).collect();
        assert_eq!(slots_before, slots_after);
    }

    #[test]
    fn playback_slot_triggers_early_notes_from_the_step_before() {
        // En not strax före steg 3 (2.7) triggas av steg 2 och klingar 0.7 steg in
        // — den hörs alltså före slaget, som den spelades.
        let n = TakeNote { pos: 2.7, key: 60, velocity: 0.8 };
        let (step, frac) = Take::playback_slot(&n);
        assert_eq!(step, 2);
        assert!((frac - 0.7).abs() < 1e-6);

        let n = TakeNote { pos: 3.0, key: 60, velocity: 0.8 };
        let (step, frac) = Take::playback_slot(&n);
        assert_eq!(step, 3);
        assert_eq!(frac, 0.0);
    }

    #[test]
    fn a_note_gets_its_position_from_the_step_and_the_phase() {
        assert_eq!(take_pos_from(0, 0.0), 0.0);
        assert!((take_pos_from(3, 0.25) - 3.25).abs() < 1e-6);
        assert!((take_pos_from(15, 0.9) - 15.9).abs() < 1e-6);
        // Fas utanför 0–1 (klockglapp) får inte putta noten ur takten.
        assert_eq!(take_pos_from(15, 3.0), STEPS_PER_BAR - 0.01);
        assert_eq!(take_pos_from(0, -1.0), 0.0);
    }

    #[test]
    fn an_empty_take_is_harmless() {
        let mut t = Take::new();
        assert_eq!(t.tightness(), 0.0);
        t.quantize(1.0, 0.5);
        t.humanize(0.2, 0.2, 1);
        assert!(t.is_empty());
    }
}
