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
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// 33 ms = 30 bildrutor i sekunden. Under det är bildrutan över budget, och det
/// är den som känns som lagg.
const FRAME_BUDGET_MS: f32 = 33.0;

/// Loggen kapas hit: hellre en färsk fil än en som växer utan att någon läser den.
const LOG_CAP_BYTES: u64 = 1_048_576;

/// **Ett häng är en fas som står still i mer än så här.** Fönstret blir "not responding" hos
/// window-managern innan dess, och då hinner ingen bildruta skriva något — därför en tråd vid
/// sidan om som säger vilken fas det står i.
const HANG_PROBE_SECS: f32 = 2.0;

/// Processens start, så loggraderna går att jämföra med en tidslinje (och så ett **glapp**
/// syns som ett glapp: en rad mitt i en hängning finns inte, men tiden mellan två rader gör).
static PROC_START: OnceLock<Instant> = OnceLock::new();

/// Vad bildrutan senast blev klar med, och när.
///
/// Fällan `perf_mark` ligger **efter** varje fas, så `last_done == "skarmdump"` betyder att
/// hela `update` gick igenom och att blockeringen sitter i fönstrets present; varje annat värde
/// pekar ut fasen som *efterföljande* — den som inte blev klar.
struct Phase {
    last_done: &'static str,
    last_ms: f32,
    frame_done: Instant,
    reported: bool,
}

static PHASE: OnceLock<Mutex<Phase>> = OnceLock::new();

fn since_start_secs() -> f32 {
    PROC_START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Ska hängproben skriva nu? Ren funktion, så tröskeln går att pröva utan en tråd.
fn hang_due(secs: f32, already_reported: bool) -> bool {
    secs >= HANG_PROBE_SECS && !already_reported
}

/// Skriver till samma logg som fasmätaren, med samma tak.
fn log_line(line: &str) {
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

/// Startar hängproben en gång: en tråd som tittar var 200:e ms och säger till när en fas
/// (eller hela bildrutan) stått still för länge. En frisk session märker den inte.
fn phase() -> &'static Mutex<Phase> {
    PHASE.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            // Ett giftigt lås får inte göra proben till en tyst no-op: den finns för att
            // säga till när appen är tyst, så den får inte själv dö utan ett spår.
            let mut p = phase().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let idle = p.frame_done.elapsed().as_secs_f32();
            if hang_due(idle, p.reported) {
                p.reported = true;
                let (last, ms) = (p.last_done, p.last_ms);
                drop(p);
                let where_hint = match last {
                    "skarmdump" => "efter update — alltså i fönstrets present",
                    "bildruta-start" => "före första fasen i bildrutan (tema) — eller precis efter att en bildruta börjat",
                    "" => "före den allra första fasen (uppstart)",
                    _ => "fasen efter den",
                };
                log_line(&format!(
                    "[{:7.2}s] HÄNG: ingen bildruta på {idle:.1} s · senast klar: '{last}' ({ms:.1} ms) · blockeringen sitter i {where_hint}\n",
                    since_start_secs()
                ));
            }
        });
        Mutex::new(Phase {
            last_done: "",
            last_ms: 0.0,
            frame_done: Instant::now(),
            reported: false,
        })
    })
}

impl SonixApp {
    /// **Början av bildrutan.** Utan den står `last_done` kvar som `"skarmdump"` från förra
    /// bildrutan, och en hängning före första fasen (i `theme::reload_if_changed` eller
    /// `apply_style`) skulle pekas ut som fönstrets present — alltså i fel kod.
    pub(crate) fn perf_frame_start(&mut self) {
        let mut p = phase().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        p.last_done = "bildruta-start";
        p.last_ms = 0.0;
    }

    /// Märker en fas: `let t = Instant::now(); … ; self.perf_mark("fas", t);`
    pub(crate) fn perf_mark(&mut self, name: &'static str, since: Instant) {
        let ms = since.elapsed().as_secs_f32() * 1000.0;
        self.perf_marks.push((name, ms));
        // Hängproben behöver veta hur långt bildrutan kom innan den stannade.
        let mut p = phase().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        p.last_done = name;
        p.last_ms = ms;
        p.reported = false;
    }

    /// Stänger bildrutan: median var femte sekund, och varje spik med faserna.
    pub(crate) fn perf_frame_end(&mut self, start: Instant) {
        let mut p = phase().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        p.frame_done = Instant::now();
        p.reported = false;
        drop(p);
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
                "[{:7.2}s] frame {total:.1} ms (budget {FRAME_BUDGET_MS:.0}) · {}\n",
                since_start_secs(),
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
        log_line(line);
    }
}

#[cfg(test)]
mod hang_probe_tests {
    use super::hang_due;

    /// Tröskeln är två sekunder och en fas rapporteras **en gång**: en hängning ska ge en rad
    /// som pekar ut fasen, inte en rad var 200:e ms.
    #[test]
    fn a_hang_is_reported_once_past_the_threshold() {
        assert!(!hang_due(0.0, false));
        assert!(!hang_due(1.9, false));
        assert!(hang_due(2.0, false));
        assert!(!hang_due(2.0, true), "samma fas ska inte skrivas två gånger");
        assert!(!hang_due(30.0, true));
    }
}
