//! **Stor klocka** (FL:s Big Clock): takt, slag, steg och spelad tid i stor stil.
//!
//! Bara formatering — ingen klocka, ingen tråd. Steget kommer från appens `current_step`
//! och tiden räknas ur tempo och steg, så siffrorna stämmer med det som spelas även när
//! tempot ändras mitt i.

/// Steg per slag (sextondelar) och slag per takt i 4/4.
const STEPS_PER_BEAT: usize = 4;
const BEATS_PER_BAR: usize = 4;

/// `takt.slag.steg`, ett-baserat som i FL.
pub fn bar_beat_step(step: usize) -> String {
    let bar = step / (STEPS_PER_BEAT * BEATS_PER_BAR) + 1;
    let beat = (step / STEPS_PER_BEAT) % BEATS_PER_BAR + 1;
    let sub = step % STEPS_PER_BEAT + 1;
    format!("{bar}.{beat}.{sub}")
}

/// Spelad tid som `m:ss.mmm`. Steg 0 är `0:00.000`.
pub fn elapsed_time(step: usize, bpm: f32) -> String {
    let seconds = step as f64 * 60.0 / (bpm.max(1.0) as f64 * STEPS_PER_BEAT as f64);
    let minutes = (seconds / 60.0).floor();
    let rest = seconds - minutes * 60.0;
    format!("{minutes:.0}:{rest:06.3}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_counts_bars_beats_and_steps_from_one() {
        assert_eq!(bar_beat_step(0), "1.1.1");
        assert_eq!(bar_beat_step(3), "1.1.4");
        assert_eq!(bar_beat_step(4), "1.2.1");
        assert_eq!(bar_beat_step(15), "1.4.4");
        assert_eq!(bar_beat_step(16), "2.1.1");
    }

    #[test]
    fn the_time_follows_the_tempo() {
        // 120 BPM: en sextondel = 125 ms, en takt = 2 s.
        assert_eq!(elapsed_time(0, 120.0), "0:00.000");
        assert_eq!(elapsed_time(2, 120.0), "0:00.250");
        assert_eq!(elapsed_time(16, 120.0), "0:02.000");
        // 60 BPM: dubbelt så långsamt.
        assert_eq!(elapsed_time(16, 60.0), "0:04.000");
        // Över minuten rullar minuterna.
        assert_eq!(elapsed_time(480, 120.0), "1:00.000");
    }
}
