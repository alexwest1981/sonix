//! Regelverket för piano-rollen: fält, jämförelser, åtgärder och de två rena
//! funktionerna som tillämpar dem på rutnätet.
//!
//! Utdelat ur `piano_roll.rs` 2026-11-02. Proven ligger kvar där de kördes.

use super::piano_roll::{count_selected, delete_cells, transpose_cells};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuleField {
    Velocity,
    Key,
    Step,
}

impl RuleField {
    pub const ALL: [RuleField; 3] = [RuleField::Velocity, RuleField::Key, RuleField::Step];

    pub fn label(self) -> &'static str {
        match self {
            RuleField::Velocity => "Anslag",
            RuleField::Key => "Tonhöjd",
            RuleField::Step => "Steg",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCmp {
    Less,
    Greater,
    Equal,
}

impl RuleCmp {
    pub const ALL: [RuleCmp; 3] = [RuleCmp::Less, RuleCmp::Greater, RuleCmp::Equal];

    pub fn label(self) -> &'static str {
        match self {
            RuleCmp::Less => "<",
            RuleCmp::Greater => ">",
            RuleCmp::Equal => "=",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuleAction {
    Delete,
    Transpose(i32),
    SetVelocity(f32),
}

impl RuleAction {
    pub fn label(self) -> &'static str {
        match self {
            RuleAction::Delete => "radera",
            RuleAction::Transpose(_) => "transponera",
            RuleAction::SetVelocity(_) => "sätt anslag",
        }
    }
}

/// En regel: `fält cmp värde` → `åtgärd`. Cubases Logical Editor har flera rader och
/// fältet `lePosition/leType/leValue`; vi har en rad, vilket räcker för de vanligaste förvalen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub field: RuleField,
    pub cmp: RuleCmp,
    /// Jämförelsevärde: anslag 0–1, tonhöjd som MIDI-nummer, steg 1–16.
    pub value: f32,
    pub action: RuleAction,
}

impl Rule {
    fn matches(&self, row: usize, step: usize, velocity: f32) -> bool {
        let left = match self.field {
            RuleField::Velocity => velocity,
            RuleField::Key => (crate::audio::scale::PIANO_ROLL_BASE_MIDI + row as u8) as f32,
            RuleField::Step => (step + 1) as f32,
        };
        match self.cmp {
            RuleCmp::Less => left < self.value,
            RuleCmp::Greater => left > self.value,
            RuleCmp::Equal => (left - self.value).abs() < 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleResult {
    pub matched: usize,
    pub changed: usize,
}

/// Kör regeln. Finns ett urval gäller det, annars hela rutnätet — samma ordning som
/// radera/transponera-knapparna, så regeln inte blir en tredje sorts sanning.
pub fn apply_rule(
    grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    velocities: &mut [f32; crate::audio::tempo::MAX_STEPS],
    rule: &Rule,
) -> RuleResult {
    let only_selection = count_selected(sel) > 0;
    let mut matched = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !grid[row][step] || (only_selection && !sel[row][step]) {
                continue;
            }
            if rule.matches(row, step, velocities[step]) {
                matched[row][step] = true;
            }
        }
    }
    let matched_count = count_selected(&matched);
    let changed = match rule.action {
        RuleAction::Delete => delete_cells(grid, &matched),
        RuleAction::Transpose(rows) => {
            transpose_cells(grid, &matched, rows);
            matched_count
        }
        RuleAction::SetVelocity(v) => {
            let mut n = 0;
            for step in 0..crate::audio::tempo::MAX_STEPS {
                let hit = (0..24).any(|row| matched[row][step]);
                if hit && (velocities[step] - v).abs() > 1e-4 {
                    velocities[step] = v;
                    n += 1;
                }
            }
            n
        }
    };
    RuleResult { matched: matched_count, changed }
}

/// Markerar celler i steg vars anslag ligger under tröskeln.
///
/// Cubase väljer enligt ett villkor och låter sedan åtgärden gå på urvalet — vi har samma
/// ordning: markera de svaga, sedan 🗑 eller transponera. Ingen egen raderingsväg.
pub(crate) fn select_quiet(
    grid: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    velocities: &[f32; crate::audio::tempo::MAX_STEPS],
    threshold: f32,
) -> [[bool; crate::ui::app::state::MAX_STEPS]; 24] {
    let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for step in 0..crate::audio::tempo::MAX_STEPS {
        if velocities[step] >= threshold {
            continue;
        }
        for row in 0..24 {
            if grid[row][step] {
                sel[row][step] = true;
            }
        }
    }
    sel
}

/// Snappar noter till ackordets toner — melodins karaktär, inte ett fast intervall.
///
/// Noter som skulle hamna utanför rutnätet stannar, samma regel som transponeringen.
pub fn snap_grid_to_chord(
    grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    chord: &crate::ui::app::state::ChordEvent,
) -> usize {
    let only_selection = count_selected(sel) > 0;
    let mut next = *grid;
    let mut moved = 0;
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !grid[row][step] || (only_selection && !sel[row][step]) {
                continue;
            }
            let midi = crate::audio::scale::PIANO_ROLL_BASE_MIDI + row as u8;
            let snapped = crate::ui::app::state::snap_to_chord(midi, chord);
            let delta = snapped as i32 - midi as i32;
            if delta == 0 {
                continue;
            }
            let target = row as i32 + delta;
            if (0..24).contains(&target) {
                next[row][step] = false;
                next[target as usize][step] = true;
                moved += 1;
            }
        }
    }
    *grid = next;
    moved
}
