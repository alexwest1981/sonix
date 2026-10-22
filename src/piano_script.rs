//! **Pianorullsskript** (FL Studio: piano roll-skript, Ableton: Max, Cubase: Logical
//! Editor). Ett litet textskript i stället för en kedja av knappar: välj noter, ändra dem,
//! välj nästa grupp. Ett skript är läsbart i efterhand och går att spara i ett projekt.
//!
//! Språket är medvetet litet — en kommandorad per rad (eller `;`), byggt av ett urval och
//! en åtgärd:
//!
//! ```text
//! select all
//! transpose -12
//! select key > 60
//! velocity *0.8
//! quantize 100 1
//! ```
//!
//! Villkoren jämför `key` (MIDI-not), `vel` (0–1) eller `pos` (steg i takten) med ett tal.
//! Åtgärderna är `transpose`, `velocity` (`*faktor` eller `=värde`), `humanize`, `strum`,
//! `quantize` och `delete`. Ett okänt ord är ett **fel med radnummer**, inte en tyst no-op —
//! ett skript som inte gör något är värre än ett som säger att det inte förstod.

use crate::midi_take::{Take, TakeGrid, STEPS_PER_BAR};

/// Vad en rad gjorde: hur många noter urvalet omfattade och hur många som faktiskt ändrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineReport {
    pub matched: usize,
    pub changed: usize,
}

/// Resultatet av ett helt skript, en post per rad som inte är tom eller en kommentar.
pub fn run_script(take: &mut Take, text: &str, seed: u64) -> Result<Vec<LineReport>, String> {
    // Urvalet är index i `take.notes`. `None` = allt (innan första `select`).
    let mut selected: Option<Vec<usize>> = None;
    let mut reports = Vec::new();

    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for command in line.split(';') {
            let command = command.trim();
            if command.is_empty() {
                continue;
            }
            let report = run_line(take, command, &mut selected, seed)
                .map_err(|err| format!("Rad {}: {}", line_no + 1, err))?;
            reports.push(report);
        }
    }

    Ok(reports)
}

fn run_line(
    take: &mut Take,
    command: &str,
    selected: &mut Option<Vec<usize>>,
    seed: u64,
) -> Result<LineReport, String> {
    let mut words = command.split_whitespace();
    let verb = words.next().unwrap_or("").to_lowercase();
    let rest: Vec<&str> = words.collect();

    match verb.as_str() {
        "select" => {
            let indices = select(take, &rest)?;
            let report = LineReport { matched: indices.len(), changed: 0 };
            *selected = Some(indices);
            Ok(report)
        }
        // Åtgärderna nedan arbetar på urvalet; utan `select` är urvalet allt.
        _ => {
            let targets: Vec<usize> = match selected {
                Some(list) => list.clone(),
                None => (0..take.notes.len()).collect(),
            };
            let mut changed = 0;
            match verb.as_str() {
                "transpose" => {
                    let semitones: i32 = rest
                        .first()
                        .ok_or("transpose behöver ett antal semitoner")?
                        .parse()
                        .map_err(|_| "transpose: antalet semitoner är inte ett heltal")?;
                    for i in &targets {
                        let key = take.notes[*i].key as i32 + semitones;
                        let clamped = key.clamp(0, 127) as u8;
                        if clamped != take.notes[*i].key {
                            take.notes[*i].key = clamped;
                            changed += 1;
                        }
                    }
                }
                "velocity" => {
                    let arg = *rest.first().ok_or("velocity behöver *faktor eller =värde")?;
                    for i in &targets {
                        let before = take.notes[*i].velocity;
                        let after = if let Some(factor) = arg.strip_prefix('*') {
                            before * factor.parse::<f32>().map_err(|_| "velocity: ogiltig faktor")?
                        } else if let Some(value) = arg.strip_prefix('=') {
                            value.parse::<f32>().map_err(|_| "velocity: ogiltigt värde")?
                        } else {
                            return Err("velocity: skriv *faktor eller =värde".to_string());
                        };
                        let after = after.clamp(0.0, 1.0);
                        if (after - before).abs() > f32::EPSILON {
                            take.notes[*i].velocity = after;
                            changed += 1;
                        }
                    }
                }
                "humanize" => {
                    let amount: f32 = rest
                        .first()
                        .ok_or("humanize behöver en mängd 0–1")?
                        .parse()
                        .map_err(|_| "humanize: mängden är inte ett tal")?;
                    for (n, i) in targets.iter().enumerate() {
                        // Fast frö per not och rad: samma skript ger samma resultat.
                        let mut rng = crate::rng::Rng::new(seed ^ (*i as u64) << 8 ^ n as u64);
                        let before = (take.notes[*i].pos, take.notes[*i].velocity);
                        take.notes[*i].pos = (before.0 + rng.next_sym() * 0.15 * amount)
                            .clamp(0.0, STEPS_PER_BAR - 0.01);
                        take.notes[*i].velocity =
                            (before.1 + rng.next_sym() * 0.3 * amount).clamp(0.05, 1.0);
                        changed += 1;
                    }
                }
                "strum" => {
                    let arg = *rest.first().ok_or("strum behöver en spridning i steg")?;
                    let down = arg.starts_with('-');
                    let spread: f32 = arg.trim_start_matches('-').parse().map_err(|_| {
                        "strum: spridningen är inte ett tal (negativt = nedstrum)"
                    })?;
                    changed = take.strum(spread, down);
                }
                "quantize" => {
                    let strength: f32 = rest
                        .first()
                        .ok_or("quantize behöver styrka 0–100")?
                        .parse::<f32>()
                        .map_err(|_| "quantize: styrkan är inte ett tal")?
                        / 100.0;
                    let grid_idx: usize = rest
                        .get(1)
                        .map(|s| s.parse::<usize>())
                        .transpose()
                        .map_err(|_| "quantize: rutnätet är inte ett index")?
                        .unwrap_or(1);
                    let grid = TakeGrid::from_index(grid_idx);
                    let before: Vec<f32> = targets.iter().map(|i| take.notes[*i].pos).collect();
                    take.quantize(strength.clamp(0.0, 1.0), 0.0, grid);
                    for (n, i) in targets.iter().enumerate() {
                        if (take.notes[*i].pos - before[n]).abs() > f32::EPSILON {
                            changed += 1;
                        }
                    }
                }
                "delete" => {
                    // Ta bort bakifrån så indexen håller.
                    let mut sorted = targets.clone();
                    sorted.sort_unstable();
                    sorted.reverse();
                    for i in sorted {
                        if i < take.notes.len() {
                            take.notes.remove(i);
                            changed += 1;
                        }
                    }
                    *selected = None;
                }
                other => return Err(format!("okänt kommando: {other}")),
            }
            Ok(LineReport { matched: targets.len(), changed })
        }
    }
}

/// `select all` eller `select <fält> <op> <tal>`.
fn select(take: &Take, rest: &[&str]) -> Result<Vec<usize>, String> {
    if rest.is_empty() {
        return Err("select behöver 'all' eller ett villkor".to_string());
    }
    if rest[0].eq_ignore_ascii_case("all") {
        return Ok((0..take.notes.len()).collect());
    }
    if rest.len() < 3 {
        return Err("select: skriv t.ex. 'select key > 60'".to_string());
    }
    let field = rest[0].to_lowercase();
    let op = rest[1];
    let value: f32 = rest[2].parse().map_err(|_| "select: talet går inte att läsa")?;
    let read = |n: &crate::midi_take::TakeNote| -> f32 {
        match field.as_str() {
            "key" => n.key as f32,
            "vel" | "velocity" => n.velocity,
            "pos" | "step" => n.pos,
            _ => f32::NAN,
        }
    };
    if field != "key" && field != "vel" && field != "velocity" && field != "pos" && field != "step"
    {
        return Err(format!("select: okänt fält '{field}' (key, vel eller pos)"));
    }
    let keeps = |n: &crate::midi_take::TakeNote| -> bool {
        let v = read(n);
        match op {
            ">" => v > value,
            ">=" => v >= value,
            "<" => v < value,
            "<=" => v <= value,
            "==" => (v - value).abs() < 1e-6,
            "!=" => (v - value).abs() >= 1e-6,
            _ => false,
        }
    };
    if !matches!(op, ">" | ">=" | "<" | "<=" | "==" | "!=") {
        return Err(format!("select: okänd jämförelse '{op}'"));
    }
    Ok(take
        .notes
        .iter()
        .enumerate()
        .filter(|(_, n)| keeps(n))
        .map(|(i, _)| i)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_with(notes: &[(f32, u8, f32)]) -> Take {
        let mut take = Take::new();
        for (pos, key, velocity) in notes {
            take.push(*pos, *key, *velocity);
        }
        take
    }

    #[test]
    fn a_script_transposes_every_note() {
        let mut take = take_with(&[(0.0, 60, 0.8), (1.0, 64, 0.8), (2.0, 67, 0.8)]);
        let reports = run_script(&mut take, "transpose 12", 1).expect("skriptet ska gå igenom");
        assert_eq!(reports[0].matched, 3);
        assert_eq!(reports[0].changed, 3);
        assert_eq!(take.notes.iter().map(|n| n.key).collect::<Vec<_>>(), vec![72, 76, 79]);
    }

    #[test]
    fn a_selection_limits_what_the_next_line_touches() {
        let mut take = take_with(&[(0.0, 60, 0.8), (1.0, 72, 0.8)]);
        run_script(&mut take, "select key > 60\ntranspose -12", 1).expect("ska gå igenom");
        // Den låga noten är orörd, den höga flyttades en oktav ner.
        assert_eq!(take.notes[0].key, 60);
        assert_eq!(take.notes[1].key, 60);
    }

    #[test]
    fn velocity_accepts_both_factor_and_value() {
        let mut take = take_with(&[(0.0, 60, 0.8)]);
        run_script(&mut take, "velocity *0.5", 1).expect("faktor");
        assert!((take.notes[0].velocity - 0.4).abs() < 1e-6);
        run_script(&mut take, "velocity =0.9", 1).expect("värde");
        assert!((take.notes[0].velocity - 0.9).abs() < 1e-6);
    }

    #[test]
    fn deleting_removes_the_selected_notes_and_nothing_else() {
        let mut take = take_with(&[(0.0, 60, 0.8), (1.0, 72, 0.8), (2.0, 64, 0.8)]);
        let reports = run_script(&mut take, "select key >= 64\ndelete", 1).expect("ska gå igenom");
        assert_eq!(reports[1].changed, 2);
        assert_eq!(take.notes.len(), 1);
        assert_eq!(take.notes[0].key, 60);
    }

    #[test]
    fn an_unknown_word_is_an_error_with_its_line_number() {
        let mut take = take_with(&[(0.0, 60, 0.8)]);
        let err = run_script(&mut take, "transpose 2\nwobble 3", 1).expect_err("ska falla");
        assert!(err.starts_with("Rad 2:"), "felet pekar ut raden: {err}");
        assert!(err.contains("wobble"), "felet namnger ordet: {err}");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let mut take = take_with(&[(0.0, 60, 0.8)]);
        let reports = run_script(&mut take, "// bara en kommentar\n\n   \ntranspose 1", 1)
            .expect("ska gå igenom");
        assert_eq!(reports.len(), 1, "bara en rad gjorde något");
        assert_eq!(take.notes[0].key, 61);
    }

    #[test]
    fn the_same_script_gives_the_same_result_twice() {
        let mut a = take_with(&[(0.0, 60, 0.8), (1.0, 64, 0.8)]);
        let mut b = a.clone();
        run_script(&mut a, "humanize 1", 7).expect("ska gå igenom");
        run_script(&mut b, "humanize 1", 7).expect("ska gå igenom");
        assert_eq!(a.notes, b.notes, "humanize ska vara deterministisk per frö");
    }
}
