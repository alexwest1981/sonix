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

/// Hur långt fram en udda rutnätslinje flyttas vid swing = 1.0, som andel av
/// rutnätets steg. En tredjedel ger den klassiska triolfeelingen (2 mot 3).
pub const SWING_MAX_FRACTION: f32 = 0.33;

/// Rutnäten kvantiseringen kan dra noterna mot (Fas 6.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeGrid {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    /// 1/8-triol: tre per fjärdedel, tolv per takt.
    TripletEighth,
    /// 1/16-triol: sex per fjärdedel.
    TripletSixteenth,
}

impl TakeGrid {
    pub const ALL: [TakeGrid; 6] = [
        TakeGrid::Quarter,
        TakeGrid::Eighth,
        TakeGrid::Sixteenth,
        TakeGrid::ThirtySecond,
        TakeGrid::TripletEighth,
        TakeGrid::TripletSixteenth,
    ];

    /// Antal rutnätslinjer i en takt.
    pub fn lines_per_bar(self) -> u32 {
        match self {
            TakeGrid::Quarter => 4,
            TakeGrid::Eighth => 8,
            TakeGrid::Sixteenth => 16,
            TakeGrid::ThirtySecond => 32,
            TakeGrid::TripletEighth => 12,
            TakeGrid::TripletSixteenth => 24,
        }
    }

    /// Avståndet mellan två linjer, i steg.
    pub fn step_size(self) -> f32 {
        STEPS_PER_BAR / self.lines_per_bar() as f32
    }

    pub fn label(self) -> &'static str {
        match self {
            TakeGrid::Quarter => "1/4",
            TakeGrid::Eighth => "1/8",
            TakeGrid::Sixteenth => "1/16",
            TakeGrid::ThirtySecond => "1/32",
            TakeGrid::TripletEighth => "1/8-triol",
            TakeGrid::TripletSixteenth => "1/16-triol",
        }
    }

    /// Index i `ALL`, för UI-läget.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(2)
    }

    pub fn from_index(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or(TakeGrid::Sixteenth)
    }
}

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

pub use crate::rng::Rng;

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

    /// Var en rutnätslinje ligger med sväng inräknad: udda linjer skjuts fram.
    /// Svängen skalas med rutnätet, så "en tredjedel" känns likadan i 1/8 som i
    /// 1/16 — annars vore svängen bara rätt i ett av rutnäten.
    pub fn swung_line(line: usize, swing: f32, grid: TakeGrid) -> f32 {
        let step = grid.step_size();
        let base = line as f32 * step;
        if line % 2 == 1 {
            base + swing.clamp(0.0, 1.0) * step * SWING_MAX_FRACTION
        } else {
            base
        }
    }

    /// Rutnätslinjen som ligger närmast `pos`, med sväng inräknad.
    pub fn nearest_line(pos: f32, swing: f32, grid: TakeGrid) -> f32 {
        let mut best = 0.0f32;
        let mut best_d = f32::MAX;
        for line in 0..grid.lines_per_bar() as usize {
            let target = Self::swung_line(line, swing, grid);
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
    /// så tight den kan bli). `swing` 0.0 = rakt, 1.0 = triolkänsla. `grid` är
    /// rutnätet noterna dras mot (1/4 … 1/32 eller trioler).
    pub fn quantize(&mut self, strength: f32, swing: f32, grid: TakeGrid) {
        let strength = strength.clamp(0.0, 1.0);
        for n in &mut self.notes {
            let target = Self::nearest_line(n.pos, swing, grid);
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

/// Vad som ska spelas vid ett steg, ur tagningen (Fas 6.4 steg 2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakePlan {
    /// Noter att spela nu: (notnummer, fördröjning i samples, anslag 0–1).
    pub play: Vec<(u8, u32, f32)>,
    /// Notnummer vars ruta på det här steget sköts av tagningen. Rutnätets ruta
    /// ska hoppas över, annars triggas noten två gånger.
    pub skip: Vec<u8>,
}

/// Läser tagningen och avgör vad som ska hända vid `step`.
///
/// `is_on(not, rutnätssteg)` säger om rutan är tänd: en not som spelades in men
/// sedan klickats bort ska inte klinga. Är tagningen tom blir planen tom och
/// uppspelningen sköts av rutnätet precis som förut — den som aldrig spelat in
/// något märker ingen skillnad.
pub fn plan_for_step(
    take: &Take,
    step: usize,
    step_samples: u32,
    is_on: &dyn Fn(u8, usize) -> bool,
) -> TakePlan {
    let mut plan = TakePlan::default();
    if take.is_empty() {
        return plan;
    }
    for n in &take.notes {
        let slot = Take::slot(n);
        if slot == step && !plan.skip.contains(&n.key) {
            plan.skip.push(n.key);
        }
        let (trigger_step, frac) = Take::playback_slot(n);
        if trigger_step == step && is_on(n.key, slot) {
            let delay = (frac * step_samples as f32).max(0.0) as u32;
            plan.play.push((n.key, delay, n.velocity.clamp(0.0, 1.0)));
        }
    }
    plan
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
        t.quantize(1.0, 0.0, TakeGrid::Sixteenth);
        assert_eq!(t.notes[0].pos, 0.0);
        assert_eq!(t.notes[1].pos, 4.0);
        assert_eq!(t.tightness(), 0.0);
    }

    #[test]
    fn half_strength_moves_halfway() {
        let mut t = Take::new();
        t.push(2.4, 36, 0.9);
        t.quantize(0.5, 0.0, TakeGrid::Sixteenth);
        assert!((t.notes[0].pos - 2.2).abs() < 1e-6, "{}", t.notes[0].pos);
    }

    #[test]
    fn quantize_moves_to_the_nearest_line_in_both_directions() {
        let mut t = Take::new();
        t.push(2.7, 36, 0.9); // närmare 3 än 2
        t.push(2.2, 38, 0.9); // närmare 2
        t.quantize(1.0, 0.0, TakeGrid::Sixteenth);
        assert_eq!(t.notes[0].pos, 3.0, "en släpande not dras framåt");
        assert_eq!(t.notes[1].pos, 2.0, "en tidig not dras bakåt");
    }

    #[test]
    fn zero_strength_changes_nothing() {
        let mut t = Take::new();
        t.push(2.4, 36, 0.9);
        let before = t.clone();
        t.quantize(0.0, 0.0, TakeGrid::Sixteenth);
        assert_eq!(t, before);
    }

    #[test]
    fn swing_delays_the_odd_lines() {
        assert_eq!(Take::swung_line(0, 1.0, TakeGrid::Sixteenth), 0.0);
        assert_eq!(Take::swung_line(2, 1.0, TakeGrid::Sixteenth), 2.0);
        assert!(
            (Take::swung_line(1, 1.0, TakeGrid::Sixteenth) - (1.0 + SWING_MAX_FRACTION)).abs() < 1e-6
        );
        assert_eq!(
            Take::swung_line(1, 0.0, TakeGrid::Sixteenth),
            1.0,
            "utan sväng ligger linjen rakt"
        );

        // En not som spelades rakt på en udda 16-del hamnar efter linjen med sväng.
        let mut t = Take::new();
        t.push(1.0, 36, 0.9);
        t.quantize(1.0, 1.0, TakeGrid::Sixteenth);
        assert!((t.notes[0].pos - (1.0 + SWING_MAX_FRACTION)).abs() < 1e-6);
    }

    #[test]
    fn swing_keeps_even_lines_where_they_are() {
        let mut t = Take::new();
        t.push(0.05, 36, 0.9);
        t.push(4.05, 38, 0.9);
        t.quantize(1.0, 1.0, TakeGrid::Sixteenth);
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
        t.quantize(1.0, 0.0, TakeGrid::Sixteenth);
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
    fn quantizing_to_eighths_uses_only_the_eighth_lines() {
        let mut t = Take::new();
        t.push(2.4, 60, 0.9);
        t.quantize(1.0, 0.0, TakeGrid::Eighth); // linjer vid 0, 2, 4 …
        assert_eq!(t.notes[0].pos, 2.0);
    }

    #[test]
    fn quantizing_to_quarters_moves_further_than_sixteenths() {
        let mut t = Take::new();
        t.push(2.6, 60, 0.9);
        t.quantize(1.0, 0.0, TakeGrid::Quarter); // linjer vid 0, 4, 8, 12
        assert_eq!(t.notes[0].pos, 4.0, "närmaste fjärdedel är 4.0, inte 2.0");
    }

    #[test]
    fn triplet_grids_land_on_thirds() {
        let mut t = Take::new();
        t.push(1.4, 60, 0.9);
        t.quantize(1.0, 0.0, TakeGrid::TripletEighth); // 0, 1.333, 2.667 …
        assert!((t.notes[0].pos - 4.0 / 3.0).abs() < 0.01, "{}", t.notes[0].pos);

        let mut t = Take::new();
        t.push(0.7, 60, 0.9);
        t.quantize(1.0, 0.0, TakeGrid::TripletSixteenth); // 0, 0.667, 1.333 …
        assert!((t.notes[0].pos - 2.0 / 3.0).abs() < 0.01, "{}", t.notes[0].pos);
    }

    #[test]
    fn swing_scales_with_the_grid() {
        // Svängen ska kännas likadan i 1/8 som i 1/16: en tredjedel av *rutnätets*
        // eget steg, inte en tredjedels 16-del.
        let eighth = Take::swung_line(1, 1.0, TakeGrid::Eighth);
        assert!(
            (eighth - (2.0 + 2.0 * SWING_MAX_FRACTION)).abs() < 1e-6,
            "{eighth}"
        );
        let sixteenth = Take::swung_line(1, 1.0, TakeGrid::Sixteenth);
        assert!((sixteenth - (1.0 + SWING_MAX_FRACTION)).abs() < 1e-6);
    }

    #[test]
    fn every_grid_has_a_label_and_a_round_trip_index() {
        for g in TakeGrid::ALL {
            assert_eq!(TakeGrid::from_index(g.index()), g, "{:?}", g);
            assert!(!g.label().is_empty());
            assert!(g.lines_per_bar() >= 4);
        }
        assert_eq!(TakeGrid::Sixteenth.index(), 2);
        assert_eq!(TakeGrid::from_index(99), TakeGrid::Sixteenth, "ogiltigt index ger 1/16");
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
    fn a_plan_without_a_take_is_empty() {
        // Utan tagning ska uppspelningen skötas av rutnätet precis som förut.
        let plan = plan_for_step(&Take::new(), 3, 1000, &|_, _| true);
        assert_eq!(plan, TakePlan::default());
        assert!(plan.play.is_empty());
        assert!(plan.skip.is_empty());
    }

    #[test]
    fn a_late_note_gets_a_delay_and_takes_over_its_grid_cell() {
        let mut t = Take::new();
        t.push(2.25, 60, 0.7); // en fjärdedels steg efter steg 2
        let plan = plan_for_step(&t, 2, 1000, &|_, _| true);
        assert_eq!(plan.play, vec![(60, 250, 0.7)]);
        assert_eq!(plan.skip, vec![60], "rutnätets ruta får inte trigga om noten");
    }

    #[test]
    fn an_early_note_plays_from_the_step_before_its_cell() {
        // Spelad strax före steg 3: rutan hamnar på steg 3, men noten ska höras
        // redan under steg 2 (0.7 steg in) — och steg 3 ska hoppa över den.
        let mut t = Take::new();
        t.push(2.7, 60, 0.7);
        let at_two = plan_for_step(&t, 2, 1000, &|_, _| true);
        assert_eq!(at_two.play, vec![(60, 700, 0.7)]);
        let at_three = plan_for_step(&t, 3, 1000, &|_, _| true);
        assert!(at_three.play.is_empty(), "den spelades redan");
        assert_eq!(at_three.skip, vec![60], "men rutan på steg 3 ska hoppas över");
    }

    #[test]
    fn a_note_whose_cell_was_cleared_does_not_play() {
        let mut t = Take::new();
        t.push(2.25, 60, 0.7);
        // is_on säger att rutan är släckt → tagningen får inte spela den ändå.
        let plan = plan_for_step(&t, 2, 1000, &|_, _| false);
        assert!(plan.play.is_empty());
    }

    #[test]
    fn a_tight_note_plays_with_no_delay() {
        let mut t = Take::new();
        t.push(4.0, 62, 0.9);
        let plan = plan_for_step(&t, 4, 1000, &|_, _| true);
        assert_eq!(plan.play, vec![(62, 0, 0.9)]);
    }

    #[test]
    fn the_plan_asks_about_the_right_grid_cell() {
        // is_on får notens *rutnätssteg*, inte trigger-steget: en tidig not på
        // 2.7 frågar om ruta (60, 3) medan den spelas från steg 2.
        let mut t = Take::new();
        t.push(2.7, 60, 0.5);
        let asked = std::cell::RefCell::new(Vec::new());
        let plan = plan_for_step(&t, 2, 1000, &|key, slot| {
            asked.borrow_mut().push((key, slot));
            true
        });
        assert_eq!(*asked.borrow(), vec![(60, 3)]);
        assert_eq!(plan.play.len(), 1);
    }

    #[test]
    fn an_empty_take_is_harmless() {
        let mut t = Take::new();
        assert_eq!(t.tightness(), 0.0);
        t.quantize(1.0, 0.5, TakeGrid::Sixteenth);
        t.humanize(0.2, 0.2, 1);
        assert!(t.is_empty());
    }
}
