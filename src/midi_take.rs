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
    /// 1/4-kvintol: fem per fjärdedel (Abletons `Grid Quintuplet`).
    Quintuplet,
    /// 1/4-septol: sju per fjärdedel (Abletons `Grid Septuplet`).
    Septuplet,
}

impl TakeGrid {
    pub const ALL: [TakeGrid; 8] = [
        TakeGrid::Quarter,
        TakeGrid::Eighth,
        TakeGrid::Sixteenth,
        TakeGrid::ThirtySecond,
        TakeGrid::TripletEighth,
        TakeGrid::TripletSixteenth,
        TakeGrid::Quintuplet,
        TakeGrid::Septuplet,
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
            TakeGrid::Quintuplet => 20,
            TakeGrid::Septuplet => 28,
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
            TakeGrid::Quintuplet => "1/4-kvintol",
            TakeGrid::Septuplet => "1/4-septol",
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

/// **Artikulation per not** (FL:s `Articulate...`): hur noten ska formas vid spelning.
///
/// Den lagras på noten och appliceras en gång på anslag och längd — inte vid varje
/// uppspelning, så samma tagning låter likadant två gånger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Articulation {
    /// Oförändrat anslag och längd.
    #[default]
    Normal,
    /// Kort not: halva längden.
    Staccato,
    /// Utliden not: 1,6 gånger längden.
    Legato,
    /// Hårdare anslag: 1,3 gånger, klämt till 0–1.
    Accent,
}

// Formerna finns för pianorullens meny.
impl Articulation {
    pub const ALL: [Articulation; 4] = [
        Articulation::Normal,
        Articulation::Staccato,
        Articulation::Legato,
        Articulation::Accent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Articulation::Normal => "Normal",
            Articulation::Staccato => "Staccato",
            Articulation::Legato => "Legato",
            Articulation::Accent => "Accent",
        }
    }

    /// Vad artikulationen gör med (anslag, längd i steg).
    ///
    /// Längd 0,0 betyder okänd längd — då finns inget att skala, och då rör
    /// artikulationen bara anslaget.
    pub fn shape(self, velocity: f32, length_steps: f32) -> (f32, f32) {
        let len = |factor: f32| {
            if length_steps <= 0.0 {
                length_steps
            } else {
                length_steps * factor
            }
        };
        match self {
            Articulation::Normal => (velocity, length_steps),
            Articulation::Staccato => (velocity, len(0.5)),
            Articulation::Legato => (velocity, len(1.6)),
            Articulation::Accent => ((velocity * 1.3).clamp(0.0, 1.0), length_steps),
        }
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
    /// Sannolikheten att noten spelas, 0–1. 1,0 = alltid (och då beter sig gamla projekt
    /// precis som förut, eftersom fältet då saknas i filen).
    #[serde(default = "always")]
    pub probability: f32,
    /// Notens längd i steg. 0,0 = okänd (äldre tagningar), och då skalas ingen längd.
    #[serde(default)]
    pub length_steps: f32,
    /// Artikulationen noten ska formas med. Saknas i filen = Normal.
    #[serde(default)]
    pub articulation: Articulation,
    /// Om artikulationen redan är applicerad på anslag och längd, så att den
    /// inte dämpas två gånger vid ett andra anrop.
    #[serde(default)]
    pub applied: bool,
}

fn always() -> f32 {
    1.0
}

/// En tagning: noterna som spelades, med sin tajming.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Take {
    pub notes: Vec<TakeNote>,
}


/// **Villkor för urval i pianorullen** (Cubase): anslag under ett tak, en viss ton, och en
/// position inom ett intervall. Ett villkor i stället för en markering per not — markeringen
/// är resultatet av villkoret, inte en flagga som måste hållas i synk med noterna.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteFilter {
    /// Bara noter med lägre anslag än så här. `None` = alla anslag.
    pub max_velocity: Option<f32>,
    /// Bara den här tonen. `None` = alla toner.
    pub key: Option<u8>,
    /// Intervall i takten, i steg. Hela takten som standard.
    pub pos_from: f32,
    pub pos_to: f32,
}

impl Default for NoteFilter {
    fn default() -> Self {
        Self {
            max_velocity: None,
            key: None,
            pos_from: 0.0,
            pos_to: STEPS_PER_BAR,
        }
    }
}

impl NoteFilter {
    pub fn matches(&self, note: &TakeNote) -> bool {
        let vel_ok = self.max_velocity.map(|v| note.velocity < v).unwrap_or(true);
        let key_ok = self.key.map(|k| note.key == k).unwrap_or(true);
        let pos_ok = note.pos >= self.pos_from && note.pos < self.pos_to;
        vel_ok && key_ok && pos_ok
    }
}

impl Take {
    /// Antal noter som villkoret fångar — urvalet, utan att något ändras.
    pub fn count_matching(&self, filter: &NoteFilter) -> usize {
        self.notes.iter().filter(|n| filter.matches(n)).count()
    }

    /// **Sätt sannolikhet på urvalet** (Abletons notsannolikhet): samma villkor som urvalet,
    /// så det som ändras är det som räknades. Returnerar antalet noter som fick värdet.
    pub fn set_probability(&mut self, filter: &NoteFilter, probability: f32) -> usize {
        let p = probability.clamp(0.0, 1.0);
        let mut touched = 0;
        for n in self.notes.iter_mut() {
            if filter.matches(n) {
                n.probability = p;
                touched += 1;
            }
        }
        touched
    }

    /// **Sätt artikulation på urvalet** — samma villkor som de andra redigeringarna, så det som
    /// ändras är det som räknades. Anslag och längd rörs inte förrän `apply_articulations`.
    pub fn set_articulation(&mut self, filter: &NoteFilter, art: Articulation) -> usize {
        let mut touched = 0;
        for n in self.notes.iter_mut() {
            if filter.matches(n) {
                n.articulation = art;
                touched += 1;
            }
        }
        touched
    }

    /// **Forma noterna efter sin artikulation**, en gång. Redan applicerade noter hoppas över,
    /// så ett andra anrop inte dämpar staccaton till hälften en gång till.
    pub fn apply_articulations(&mut self) -> usize {
        let mut applied = 0;
        for n in self.notes.iter_mut() {
            if n.applied {
                continue;
            }
            let (velocity, length) = n.articulation.shape(n.velocity, n.length_steps);
            n.velocity = velocity;
            n.length_steps = length;
            n.applied = true;
            applied += 1;
        }
        applied
    }

    /// **Radera enligt villkor.** Returnerar antalet borttagna noter, så att gränssnittet kan
    /// säga hur många det blev i stället för att bara bli tyst.
    pub fn remove_matching(&mut self, filter: &NoteFilter) -> usize {
        let before = self.notes.len();
        self.notes.retain(|n| !filter.matches(n));
        before - self.notes.len()
    }
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
            probability: 1.0,
            // Inspelningen vet inte hur länge tangenten hölls: okänd längd.
            length_steps: 0.0,
            articulation: Articulation::Normal,
            applied: false,
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
    /// Transponerar tagningen. Noter som skulle hamna utanför MIDI-omfånget kläms dit
    /// i stället för att försvinna.
    pub fn transpose(&mut self, semitones: i32) -> usize {
        let mut moved = 0;
        for n in &mut self.notes {
            let key = (n.key as i32 + semitones).clamp(0, 127) as u8;
            if key != n.key {
                n.key = key;
                moved += 1;
            }
        }
        moved
    }

    /// Skalar anslaget (0,9 = 10 % svagare). Kläms till 0–1.
    /// **Slå ihop dubbletter** (Abletons `Join Notes`, i den form tagningen kan bära): två noter på
    /// samma position och ton är samma not — kvar blir den starkaste. Utan det här blir en dubbel
    /// inspelning två noter som spelas samtidigt och låter starkare än de andra.
    pub fn dedupe(&mut self) -> usize {
        let before = self.notes.len();
        let mut kept: Vec<TakeNote> = Vec::with_capacity(before);
        for n in self.notes.drain(..) {
            let id = (Take::slot(&n), n.key);
            match kept.iter_mut().find(|k| (Take::slot(k), k.key) == id) {
                Some(existing) => {
                    if n.velocity > existing.velocity {
                        existing.velocity = n.velocity;
                    }
                }
                None => kept.push(n),
            }
        }
        self.notes = kept;
        before - self.notes.len()
    }

    /// **Tryck ihop eller dra ut frasen** (Abletons `Fit To Range`): alla positioner skalas kring
    /// den första noten, så en fras som ligger utspridd över hela takten kan samlas i en fjärdedel
    /// — eller spridas ut. Kläms mot taktens 16 steg.
    pub fn scale_positions(&mut self, divisor: f32) -> usize {
        let d = if divisor.is_finite() && divisor > 0.0 { divisor } else { 1.0 };
        let Some(first) = self.notes.iter().map(|n| n.pos).fold(None, |acc: Option<f32>, p| {
            Some(acc.map_or(p, |a| a.min(p)))
        }) else {
            return 0;
        };
        for n in &mut self.notes {
            n.pos = (first + (n.pos - first) / d).clamp(0.0, STEPS_PER_BAR - 0.01);
        }
        self.notes.len()
    }

    /// **Skjut noterna i tid** (Abletons nudge): flyttar de noter som matchar `filter` ett antal
    /// steg, och klämmer dem mot takten så att ingen hamnar utanför.
    pub fn nudge(&mut self, filter: &NoteFilter, steps: f32) -> usize {
        let mut touched = 0;
        for n in &mut self.notes {
            if !filter.matches(n) {
                continue;
            }
            let moved = (n.pos + steps).clamp(0.0, STEPS_PER_BAR - 0.01);
            if (moved - n.pos).abs() > 1e-6 {
                n.pos = moved;
                touched += 1;
            }
        }
        touched
    }

    /// **Spegla tonhöjderna** kring en pivot: en not som ligger `d` halvtoner över pivoten
    /// hamnar `d` under den. Kläms mot 0–127, så en spegling vid kanten tappar inte noten.
    ///
    /// Bara noter som matchar `filter` speglas — samma villkor som raderingen och
    /// sannolikheten, så det som ändras är det som räknades.
    pub fn invert(&mut self, filter: &NoteFilter, pivot: u8) -> usize {
        let mut touched = 0;
        for n in &mut self.notes {
            if !filter.matches(n) {
                continue;
            }
            let mirrored = 2 * pivot as i32 - n.key as i32;
            let clamped = mirrored.clamp(0, 127) as u8;
            if clamped != n.key {
                n.key = clamped;
                touched += 1;
            }
        }
        touched
    }

    pub fn scale_velocity(&mut self, factor: f32) -> usize {
        let mut changed = 0;
        for n in &mut self.notes {
            let v = (n.velocity * factor).clamp(0.0, 1.0);
            if (v - n.velocity).abs() > 0.0005 {
                n.velocity = v;
                changed += 1;
            }
        }
        changed
    }

    pub fn humanize(&mut self, timing_steps: f32, velocity_amount: f32, seed: u64) {
        let mut rng = Rng::new(seed);
        let timing = timing_steps.max(0.0);
        let vel = velocity_amount.max(0.0);
        for n in &mut self.notes {
            n.pos = (n.pos + rng.next_sym() * timing).clamp(0.0, STEPS_PER_BAR - 0.01);
            n.velocity = (n.velocity + rng.next_sym() * vel).clamp(0.05, 1.0);
        }
    }

    /// **Strumma**: noter som ligger på samma ställe sprids i tonhöjdsordning, så ackordet
    /// slås ut som på ett stränginstrument i stället för att klippa som en orgel. Noten med
    /// lägst tonhöjd ligger kvar där den låg; ingen not flyttas bakåt i tiden.
    pub fn strum(&mut self, spread_steps: f32, down: bool) -> usize {
        let spread = spread_steps.max(0.0);
        if spread == 0.0 {
            return 0;
        }
        // Gruppera på position i hundradels steg: noter som ligger på samma slag hör ihop.
        let mut groups: std::collections::BTreeMap<i64, Vec<usize>> = std::collections::BTreeMap::new();
        for (i, n) in self.notes.iter().enumerate() {
            groups.entry((n.pos * 100.0).round() as i64).or_default().push(i);
        }
        let mut moved = 0;
        for (_, mut idxs) in groups {
            if idxs.len() < 2 {
                continue;
            }
            // Upp: lägsta tonen först. Ner: högsta först — som ett nedstrum.
            idxs.sort_by_key(|i| self.notes[*i].key);
            if down {
                idxs.reverse();
            }
            for (order, i) in idxs.iter().enumerate().skip(1) {
                let shift = spread * order as f32;
                let pos = self.notes[*i].pos + shift;
                self.notes[*i].pos = pos.min(STEPS_PER_BAR - 0.01);
                moved += 1;
            }
        }
        moved
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
    // Utan slumptal spelas allt: äldre prov och offline-renderingen beter sig som förut.
    plan_for_step_rolled(take, step, step_samples, is_on, &mut || 0.0)
}

/// Samma plan, men med slumptal: en not med sannolikhet under slumptalet hoppas över.
///
/// Slumptalet matas in så att både live-uppspelning och rendering kan välja sin egen källa —
/// renderingen en fröad, så att samma låt låter likadant två gånger.
pub fn plan_for_step_rolled(
    take: &Take,
    step: usize,
    step_samples: u32,
    is_on: &dyn Fn(u8, usize) -> bool,
    roll: &mut dyn FnMut() -> f32,
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
            if n.probability < 1.0 && roll() >= n.probability {
                continue;
            }
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
    #[test]
    fn transpose_and_velocity_edit_the_take_in_place() {
        let mut t = Take::new();
        t.push(0.0, 60, 0.8);
        t.push(1.0, 127, 0.4);
        assert_eq!(t.transpose(12), 1, "bara noten som får plats flyttas");
        assert_eq!(t.notes[0].key, 72);
        assert_eq!(t.notes[1].key, 127, "över 127 kläms, inte tappas");
        assert_eq!(t.transpose(-200), 2, "allt kläms ned till 0");
        assert_eq!(t.notes[0].key, 0);

        assert_eq!(t.scale_velocity(0.5), 2);
        assert!((t.notes[0].velocity - 0.4).abs() < 1e-4);
        assert_eq!(t.scale_velocity(0.0), 2, "noll stänger av anslaget");
        assert_eq!(t.notes[0].velocity, 0.0);
        assert_eq!(t.scale_velocity(0.5), 0, "redan i botten ändras inget");
    }

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

    /// **Kvintol och septol delar fjärdedelen i fem respektive sju** — samma mekanik som
    /// triolerna, bara ett annat tal, så steget blir exakt jämnt fördelat över takten.
    #[test]
    fn quintuplets_and_septuplets_split_the_bar_evenly() {
        for (grid, lines) in [(TakeGrid::Quintuplet, 20u32), (TakeGrid::Septuplet, 28u32)] {
            assert_eq!(grid.lines_per_bar(), lines);
            let step = grid.step_size();
            assert!(
                (step * lines as f32 - STEPS_PER_BAR).abs() < 0.001,
                "{:?}: {} × {} ska bli en takt",
                grid,
                step,
                lines
            );
        }
        // Kvintolen ligger tätare än sextondelen men glesare än 1/32.
        assert!(TakeGrid::Quintuplet.step_size() < TakeGrid::Sixteenth.step_size());
        assert!(TakeGrid::Septuplet.step_size() > TakeGrid::ThirtySecond.step_size());
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
        let n = TakeNote { pos: 2.7, key: 60, velocity: 0.8, probability: 1.0, length_steps: 0.0, articulation: Articulation::Normal, applied: false };
        let (step, frac) = Take::playback_slot(&n);
        assert_eq!(step, 2);
        assert!((frac - 0.7).abs() < 1e-6);

        let n = TakeNote { pos: 3.0, key: 60, velocity: 0.8, probability: 1.0, length_steps: 0.0, articulation: Articulation::Normal, applied: false };
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
    /// **Strumning slår ut ett ackord i tonhöjdsordning.** Tre noter på samma slag blir
    /// 0, 0,05 och 0,10 steg, med den lägsta kvar där den låg — och en ensam not rörs inte.
    #[test]
    fn strumming_spreads_a_chord_in_pitch_order() {
        let mut take = Take::new();
        take.push(2.0, 64, 0.8);
        take.push(2.0, 60, 0.8);
        take.push(2.0, 67, 0.8);
        take.push(5.0, 72, 0.8); // ensam: ska stå still
        let moved = take.strum(0.05, false);
        assert_eq!(moved, 2, "två noter flyttas, den lägsta och den ensamma står still");
        let at = |key: u8| {
            take.notes
                .iter()
                .find(|n| n.key == key)
                .map(|n| n.pos)
                .expect("noten finns")
        };
        assert!((at(60) - 2.0).abs() < 1e-4, "lägst ligger kvar: {}", at(60));
        assert!((at(64) - 2.05).abs() < 1e-4, "mellan: {}", at(64));
        assert!((at(67) - 2.10).abs() < 1e-4, "högst sist: {}", at(67));
        assert!((at(72) - 5.0).abs() < 1e-4, "ensam not rörs inte");
    }

    /// Strumning med noll spridning är en no-op, och noter flyttas aldrig bakåt.
    #[test]
    fn strumming_with_zero_spread_changes_nothing() {
        let mut take = Take::new();
        take.push(1.0, 60, 0.8);
        take.push(1.0, 64, 0.8);
        assert_eq!(take.strum(0.0, false), 0);
        assert!((take.notes[0].pos - 1.0).abs() < 1e-6);
        assert!((take.notes[1].pos - 1.0).abs() < 1e-6);
    }

    /// Nedstrum: då slås den högsta tonen först och den lägsta sist.
    #[test]
    fn a_down_strum_starts_with_the_highest_note() {
        let mut take = Take::new();
        take.push(1.0, 60, 0.8);
        take.push(1.0, 67, 0.8);
        let moved = take.strum(0.1, true);
        assert_eq!(moved, 1);
        let at = |key: u8| take.notes.iter().find(|n| n.key == key).unwrap().pos;
        assert!((at(67) - 1.0).abs() < 1e-4, "högst ligger kvar: {}", at(67));
        assert!((at(60) - 1.1).abs() < 1e-4, "lägst sist: {}", at(60));
    }

    /// **Villkoret fångar det det ska och raderar bara det** — anslag under taket, en ton, och
    /// bara inom intervallet. Provet har en not utanför varje gräns, så ett filter som glömmer
    /// en av dem syns.
    #[test]
    fn a_note_filter_selects_and_deletes_only_what_it_matches() {
        let mut take = Take::new();
        take.push(0.0, 36, 0.2); // utanför intervallet (före 1.0)
        take.push(8.0, 36, 0.3); // fångas: svagt, rätt ton, i intervallet
        take.push(4.0, 38, 0.2); // fel ton
        take.push(4.0, 36, 0.9); // för starkt
        take.push(8.5, 36, 0.1); // fångas

        let filter = NoteFilter {
            max_velocity: Some(0.5),
            key: Some(36),
            pos_from: 1.0,
            pos_to: 16.0,
        };
        assert_eq!(take.count_matching(&filter), 2, "två noter ska fångas");
        assert_eq!(take.remove_matching(&filter), 2, "två ska tas bort");
        assert_eq!(take.notes.len(), 3, "tre ska stå kvar");

        // Utan villkor fångas hela takten, och då töms tagningen.
        let all = NoteFilter::default();
        assert_eq!(take.count_matching(&all), 3);
        assert_eq!(take.remove_matching(&all), 3);
        assert!(take.notes.is_empty());
    }

    /// **Sannolikheten gate:ar noten, inte anslaget.** En not med 0,0 spelas aldrig, en med 1,0
    /// alltid, och urvalet kan sätta värdet på exakt de noter villkoret fångar.
    #[test]
    fn note_probability_decides_whether_the_note_plays() {
        let mut take = Take::new();
        take.push(0.0, 36, 0.8);
        take.push(0.0, 38, 0.8);
        take.push(0.0, 40, 0.8);

        // Bara noten 38 får 50 %.
        let only_38 = NoteFilter { key: Some(38), ..NoteFilter::default() };
        assert_eq!(take.set_probability(&only_38, 0.5), 1);
        assert_eq!(take.notes[1].probability, 0.5);
        assert_eq!(take.notes[0].probability, 1.0, "andra noter ska stå orörda");

        // Slumptal under sannolikheten → spelar; över → tyst.
        let is_on = |_: u8, _: usize| true;
        let mut low_roll = || 0.2f32;
        let plays = plan_for_step_rolled(&take, 0, 1000, &is_on, &mut low_roll);
        assert_eq!(plays.play.len(), 3, "0,2 < 0,5: alla tre spelar");
        let mut high_roll = || 0.9f32;
        let silent = plan_for_step_rolled(&take, 0, 1000, &is_on, &mut high_roll);
        assert_eq!(silent.play.len(), 2, "0,9 ≥ 0,5: noten 38 tystnar");
        assert!(!silent.play.iter().any(|(k, _, _)| *k == 38));

        // 0,0 spelas aldrig, ens med slumptal 0.
        let none = NoteFilter::default();
        take.set_probability(&none, 0.0);
        let mut zero = || 0.0f32;
        assert!(plan_for_step_rolled(&take, 0, 1000, &is_on, &mut zero).play.is_empty());

        // Och den gamla vägen (utan slumptal) spelar allt, som förut.
        take.set_probability(&none, 1.0);
        assert_eq!(plan_for_step(&take, 0, 1000, &is_on).play.len(), 3);
    }

    #[test]
    fn inverting_mirrors_the_notes_around_the_pivot_and_clamps_at_the_edges() {
        let mut t = Take::new();
        t.push(0.0, 60, 0.8);  // på pivoten
        t.push(2.0, 67, 0.8);  // 7 över → 7 under
        t.push(4.0, 53, 0.8);  // 7 under → 7 över
        t.push(6.0, 2, 0.8);   // 58 under 60 → 118, alltså inte nere vid 0
        assert_eq!(t.invert(&NoteFilter::default(), 60), 3, "noten på pivoten ska inte räknas som ändrad");
        let keys: Vec<u8> = t.notes.iter().map(|n| n.key).collect();
        assert_eq!(keys, vec![60, 53, 67, 118]);

        // Bara det som matchar urvalet speglas — samma villkor som raderingen använder.
        let mut t = Take::new();
        t.push(0.0, 67, 0.8);
        t.push(2.0, 72, 0.8);
        let only_g = NoteFilter { key: Some(67), ..NoteFilter::default() };
        assert_eq!(t.invert(&only_g, 60), 1);
        let keys: Vec<u8> = t.notes.iter().map(|n| n.key).collect();
        assert_eq!(keys, vec![53, 72], "72:an ligger utanför urvalet och ska stå kvar");
    }

    /// `Join Notes` och `Fit To Range` i den form tagningen kan bära: dubbletter slås ihop till
    /// den starkaste, och positionerna kan dras ihop kring frasens början.
    #[test]
    fn duplicates_join_to_the_loudest_and_the_phrase_can_be_compressed() {
        let mut t = Take::new();
        t.push(4.0, 60, 0.4);
        t.push(4.0, 60, 0.9);   // samma position och ton: ska slås ihop
        t.push(4.0, 64, 0.5);   // samma position, annan ton: ska stå kvar
        t.push(8.0, 60, 0.6);   // samma ton, annan position: ska stå kvar
        assert_eq!(t.dedupe(), 1);
        assert_eq!(t.notes.len(), 3);
        let merged = t.notes.iter().find(|n| n.key == 60 && (n.pos - 4.0).abs() < 0.01).unwrap();
        assert!((merged.velocity - 0.9).abs() < 1e-6, "den starkaste ska vara kvar");

        // Fras 4…12 trycks ihop till hälften: 8 blir 6, 12 blir 8.
        let mut t = Take::new();
        t.push(4.0, 60, 0.8);
        t.push(12.0, 62, 0.8);
        t.scale_positions(2.0);
        let pos: Vec<f32> = t.notes.iter().map(|n| n.pos).collect();
        assert!((pos[0] - 4.0).abs() < 1e-4, "den första noten är ankaret");
        assert!((pos[1] - 8.0).abs() < 1e-4, "12 → 4 + (12−4)/2 = 8");
    }

    #[test]
    fn nudging_moves_only_the_selection_and_stops_at_the_bar_edge() {
        let mut t = Take::new();
        t.push(0.0, 60, 0.8);
        t.push(15.99, 62, 0.8);
        assert_eq!(t.nudge(&NoteFilter::default(), 0.25), 1, "noten vid kanten kan inte flyttas framåt");
        assert!((t.notes[0].pos - 0.25).abs() < 1e-4);
        assert_eq!(t.notes[1].pos, 15.99);
        // Bara ton 62 i urvalet: då flyttas den, och 60:an står kvar.
        t.nudge(&NoteFilter { key: Some(62), ..NoteFilter::default() }, -1.0);
        assert!((t.notes[0].pos - 0.25).abs() < 1e-4);
        assert!((t.notes[1].pos - 14.99).abs() < 1e-4);
    }

    /// **Artikulationen har exakt de fyra formerna** — normal rör inget, staccato halverar
    /// längden, legato drar ut den till 1,6 gånger och accent höjer anslaget 1,3 gånger.
    #[test]
    fn each_articulation_shapes_the_attack_and_the_length() {
        let (v, l) = (0.5f32, 4.0f32);
        assert_eq!(Articulation::Normal.shape(v, l), (v, l), "normal är oförändrad");
        assert_eq!(Articulation::Staccato.shape(v, l), (v, 2.0), "halva längden");
        let (vel, len) = Articulation::Legato.shape(v, l);
        assert_eq!(vel, v);
        assert!((len - 6.4).abs() < 1e-6, "4 × 1,6 = 6,4, blev {len}");
        let (vel, len) = Articulation::Accent.shape(v, l);
        assert!((vel - 0.65).abs() < 1e-6, "0,5 × 1,3 = 0,65, blev {vel}");
        assert_eq!(len, l, "accent rör inte längden");
    }

    /// **Okänd längd skalas inte**: 0,0 är ingen längd, och då får artikulationen bara
    /// röra anslaget — annars blev en staccaton med okänd längd fortfarande 0,0 och det
    /// såg ut som att något hänt.
    #[test]
    fn an_unknown_length_stays_unknown() {
        for art in [Articulation::Staccato, Articulation::Legato] {
            assert_eq!(art.shape(0.8, 0.0), (0.8, 0.0), "{:?}", art);
        }
        assert_eq!(Articulation::Accent.shape(0.9, 0.0).0, 1.0, "anslaget kläms vid 1,0");
    }

    /// Artikulationen sätts bara på urvalet, och appliceringen rör bara det som valts —
    /// noten utanför urvalet behåller både anslag och längd.
    #[test]
    fn articulation_is_set_on_the_selection_only() {
        let mut t = Take::new();
        t.push(0.0, 36, 0.8);
        t.push(0.0, 38, 0.8);
        for n in &mut t.notes {
            n.length_steps = 4.0;
        }
        let only_36 = NoteFilter { key: Some(36), ..NoteFilter::default() };
        assert_eq!(t.set_articulation(&only_36, Articulation::Staccato), 1);
        assert_eq!(t.notes[0].articulation, Articulation::Staccato);
        assert_eq!(t.notes[1].articulation, Articulation::Normal, "utanför urvalet");

        assert_eq!(t.apply_articulations(), 2, "båda noterna formas, normal utan verkan");
        assert_eq!(t.notes[0].length_steps, 2.0);
        assert_eq!(t.notes[1].length_steps, 4.0);
    }

    /// **Två anrop dämpar inte två gånger**: staccaton halveras en gång, sedan händer inget.
    #[test]
    fn applying_articulations_twice_changes_nothing_the_second_time() {
        let mut t = Take::new();
        t.push(0.0, 36, 0.8);
        t.notes[0].length_steps = 4.0;
        t.set_articulation(&NoteFilter::default(), Articulation::Staccato);
        assert_eq!(t.apply_articulations(), 1);
        let once = t.clone();
        assert!((t.notes[0].length_steps - 2.0).abs() < 1e-6);
        assert_eq!(t.apply_articulations(), 0, "andra anropet applicerar inget");
        assert_eq!(t, once, "och ändrar inget");
    }

}
