//! **Capture MIDI** (Ableton): fångar det du nyss spelade utan att spela in.
//!
//! Bufferten håller noterna med **tid sedan första noten**, inte med stegnummer: när man sitter
//! och provspelar står transporten still, så det finns ingen stegposition att läsa. Tiden blir
//! steg först när noterna läggs i en tagning, och då är tempot känt.
//!
//! `shortcut: noterna läggs i en takt (mönstret är ett takt-rutnät) och första noten blir steg 0 —
//! flytta den om frasen börjar senare; uppgradera när patterns blir flera takter.`

/// Takter i tagningen: 16 steg per takt vid 4/4.
const STEPS_PER_BAR: f32 = 16.0;

/// Hur många noter som hålls. Fler än så är inte en fras man nyss spelade.
pub const CAPTURE_LIMIT: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapturedNote {
    /// Sekunder sedan den **första** fångade noten.
    pub at_secs: f32,
    pub key: u8,
    pub velocity: f32,
}

#[derive(Default)]
pub struct NoteCapture {
    notes: Vec<CapturedNote>,
    /// Taket för den här bufferten när den inte är standard (score-loggen vill ha fler
    /// noter än en fras). `0` betyder `CAPTURE_LIMIT`.
    limit: usize,
}

impl NoteCapture {
    /// En buffert med eget tak. Score-loggen vill hålla en hel session, inte en fras.
    pub fn with_limit(limit: usize) -> Self {
        Self {
            notes: Vec::new(),
            limit: limit.max(1),
        }
    }

    fn cap(&self) -> usize {
        if self.limit == 0 { CAPTURE_LIMIT } else { self.limit }
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Lägg en not. Den äldsta faller bort när bufferten är full — det är de **senaste** noterna
    /// som är frasen.
    pub fn push(&mut self, at_secs: f32, key: u8, velocity: f32) {
        if self.notes.len() >= self.cap() {
            self.notes.remove(0);
        }
        self.notes.push(CapturedNote {
            at_secs: at_secs.max(0.0),
            key,
            velocity: velocity.clamp(0.0, 1.0),
        });
    }

    /// Noterna som `(position i takten, not, anslag)`, räknat om från tid till steg vid `bpm`.
    ///
    /// 16 steg per takt i 4/4 ger `bpm / 15` steg per sekund. Positionen viker runt takten, så en
    /// fras som är längre än en takt hamnar ovanpå sig själv — se modulens `shortcut`.
    pub fn to_steps(&self, bpm: f32) -> Vec<(f32, u8, f32)> {
        let steps_per_sec = bpm.max(1.0) / 15.0;
        self.notes
            .iter()
            .map(|n| {
                let raw = n.at_secs * steps_per_sec;
                (raw.rem_euclid(STEPS_PER_BAR), n.key, n.velocity)
            })
            .collect()
    }

    /// Töm bufferten och lämna noterna som steg.
    pub fn drain_to_steps(&mut self, bpm: f32) -> Vec<(f32, u8, f32)> {
        let out = self.to_steps(bpm);
        self.notes.clear();
        out
    }

    /// Noterna som `(steg sedan första noten, not, anslag)` — **utan** att vika runt takten.
    ///
    /// Capture MIDI viker in sin fras i en takt; score-loggen får inte göra det, för då
    /// försvinner pauserna som gör frasen till en fras. Tiden räknas med samma
    /// `bpm / 15` steg per sekund, så en takt är 16 steg även här.
    pub fn to_steps_unfolded(&self, bpm: f32) -> Vec<(f32, u8, f32)> {
        let steps_per_sec = bpm.max(1.0) / 15.0;
        self.notes
            .iter()
            .map(|n| (n.at_secs * steps_per_sec, n.key, n.velocity))
            .collect()
    }

    /// Töm bufferten och lämna noterna som ofoldade steg (score-loggen).
    pub fn drain_unfolded(&mut self, bpm: f32) -> Vec<(f32, u8, f32)> {
        let out = self.to_steps_unfolded(bpm);
        self.notes.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_notes_land_on_the_step_grid_by_their_timing() {
        let mut cap = NoteCapture::default();
        // 120 bpm: 8 steg per sekund, alltså 0, 4 och 8 steg.
        for (t, key) in [(0.0, 60u8), (0.5, 64), (1.0, 67)] {
            cap.push(t, key, 0.9);
        }
        let steps = cap.to_steps(120.0);
        assert_eq!(steps.len(), 3);
        assert!((steps[0].0 - 0.0).abs() < 1e-3);
        assert!((steps[1].0 - 4.0).abs() < 1e-3);
        assert!((steps[2].0 - 8.0).abs() < 1e-3);
        assert_eq!((steps[1].1, steps[2].1), (64, 67));
        // Att läsa ut noterna tömmer dem inte …
        assert_eq!(cap.len(), 3);
        // … men drain gör det.
        assert_eq!(cap.drain_to_steps(120.0).len(), 3);
        assert!(cap.is_empty());
    }

    #[test]
    fn the_buffer_keeps_the_latest_notes_and_folds_a_long_phrase_into_one_bar() {
        let mut cap = NoteCapture::default();
        for i in 0..(CAPTURE_LIMIT + 5) {
            cap.push(i as f32 * 0.01, 60, 0.5);
        }
        assert_eq!(cap.len(), CAPTURE_LIMIT, "den äldsta noten ska falla bort");
        // En fras som är längre än en takt viks in i takten: alla positioner ligger inom 0–16,
        // och de överlevande noterna är de **senaste** (här 1,32 s → 10,56 steg).
        let steps = cap.to_steps(120.0);
        assert!(steps.iter().all(|(pos, _, _)| (0.0..16.0).contains(pos)));
        assert!((steps[CAPTURE_LIMIT - 1].0 - 10.56).abs() < 1e-2);
    }

    /// Score-loggen får **inte** vika: en paus på fyra sekunder är fyra takter, inte noll.
    #[test]
    fn the_unfolded_positions_keep_the_pauses_between_phrases() {
        let mut log = NoteCapture::with_limit(2048);
        // 120 bpm = 8 steg/s. 0 s, 0,5 s och 2,25 s → 0, 4 och 18 steg.
        for (t, key) in [(0.0, 60u8), (0.5, 64), (2.25, 67)] {
            log.push(t, key, 0.9);
        }
        let steps = log.to_steps_unfolded(120.0);
        assert_eq!(steps.len(), 3);
        assert!((steps[0].0 - 0.0).abs() < 1e-3);
        assert!((steps[1].0 - 4.0).abs() < 1e-3);
        assert!((steps[2].0 - 18.0).abs() < 1e-3, "pausen ska ligga kvar mellan noterna");
        // Samma buffert vikt hade gett 2 steg — provet fångar skillnaden.
        assert!((log.to_steps(120.0)[2].0 - 2.0).abs() < 1e-3);
        assert_eq!(log.drain_unfolded(120.0).len(), 3);
        assert!(log.is_empty());
    }

    /// Ett eget tak gäller: score-loggen behåller fler noter än en fras.
    #[test]
    fn a_larger_limit_keeps_more_of_the_session() {
        let mut log = NoteCapture::with_limit(5);
        for i in 0..12 {
            log.push(i as f32, 60, 0.5);
        }
        assert_eq!(log.len(), 5);
        // De fem senaste: 7–11 s.
        let first = log.to_steps_unfolded(120.0)[0].0;
        assert!((first - 56.0).abs() < 1e-3, "{first}");
    }
}
