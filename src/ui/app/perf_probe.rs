//! **Fasmätaren.** Bildrutans egna tal, skrivna till disk när de inte håller.
//!
//! Skälet den finns: "det laggar mellan varven" går inte att felsöka med en
//! profiler som bara jag kan starta, för den mäter scener jag bygger. Appen ska
//! själv skriva ner vad bildrutan gjorde när den sprack — då är Alex' egen session
//! beviset, och siffrorna pekar ut fasen i stället för gissningen.
//!
//! Kostnaden i en frisk bildruta är några `Instant::now()`: markeringarna hamnar i
//! en vektor som töms varje bildruta, och bara en bildruta över budget skrivs till
//! `logs/frame_spikes.log` (kapad till 1 MB — en logg ingen läser är ingen logg).

use super::*;
use std::time::Instant;

/// 33 ms = 30 bildrutor i sekunden. Under det är bildrutan över budget, och det
/// är den som känns som lagg.
const FRAME_BUDGET_MS: f32 = 33.0;

/// Loggen kapas hit: hellre en färsk fil än en som växer utan att någon läser den.
const LOG_CAP_BYTES: u64 = 1_048_576;

impl SonixApp {
    /// Märker en fas: `let t = Instant::now(); … ; self.perf_mark("fas", t);`
    pub(crate) fn perf_mark(&mut self, phase: &'static str, since: Instant) {
        let ms = since.elapsed().as_secs_f32() * 1000.0;
        self.perf_marks.push((phase, ms));
    }

    /// Stänger bildrutan: median var femte sekund, och varje spik med faserna.
    pub(crate) fn perf_frame_end(&mut self, start: Instant) {
        let total = start.elapsed().as_secs_f32() * 1000.0;
        self.perf_frames.push_back(total);
        if self.perf_frames.len() > 600 {
            self.perf_frames.pop_front();
        }

        let now = Instant::now();
        let due = now.duration_since(self.perf_last_write).as_secs_f32() >= 5.0;
        if due {
            self.perf_last_write = now;
            self.perf_write_summary();
        }
        if total >= FRAME_BUDGET_MS {
            let phases: Vec<String> = self
                .perf_marks
                .iter()
                .map(|(name, ms)| format!("{name} {ms:.1}"))
                .collect();
            self.perf_write(&format!(
                "frame {total:.1} ms (budget {FRAME_BUDGET_MS:.0}) · {}\n",
                phases.join(" · ")
            ));
        }
    }

    /// Medianen är det som känns; toppen säger vilken fas som sprack.
    fn perf_write_summary(&mut self) {
        if self.perf_frames.len() < 30 {
            return;
        }
        let mut sorted: Vec<f32> = self.perf_frames.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[sorted.len() / 2];
        let p95 = sorted[(sorted.len() * 95) / 100];
        let max = *sorted.last().unwrap_or(&0.0);
        let fps = if median > 0.0 { 1000.0 / median } else { 0.0 };
        self.perf_write(&format!(
            "sammanfattning: {:.1} ms median ({fps:.0} fps), p95 {p95:.1} ms, max {max:.1} ms, {} bildrutor\n",
            median,
            self.perf_frames.len()
        ));
    }

    fn perf_write(&self, line: &str) {
        let path = crate::paths::paths().log_file("frame_spikes.log");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > LOG_CAP_BYTES {
                let _ = std::fs::File::create(&path);
            }
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = std::io::Write::write_all(&mut f, line.as_bytes());
        }
    }
}
