//! **AI:ns händer.** Tar modellens verktygsanrop och utför dem mot appen — efter validering.
//!
//! Bara verktyg som redan har en säker väg i appen finns här. `set_channel_instrument` finns i
//! schemat men saknar utförare: att byta ljud kräver webbläsarens urval, och att gissa ett
//! instrumentnamn vore värre än att säga att det inte är byggt.

use std::sync::{Arc, Mutex};

use crate::audio::ai_tools;

use super::SonixApp;

impl SonixApp {
    /// Skickar användarens text till modellen i bakgrunden. `None` = ingen förfrågan pågår.
    pub fn ai_send_prompt(&mut self) {
        if self.ai_chat_pending.is_some() {
            self.status_message = crate::tstatus!("⏳ Väntar redan på assistenten");
            return;
        }
        let text = self.ai_assistant.prompt_input.trim().to_string();
        if text.is_empty() {
            self.status_message = crate::tstatus!("⚠ Skriv något först");
            return;
        }
        let config = self.ai_assistant.config.clone();
        let slot: Arc<Mutex<Option<Result<String, String>>>> = Arc::new(Mutex::new(None));
        self.ai_chat_pending = Some(slot.clone());
        std::thread::spawn(move || {
            let result = crate::audio::ai_client::chat_with_tools(&config, &text);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
        self.status_message = crate::tstatus!("🤖 Frågar assistenten...");
    }

    /// Plockar upp svaret och utför anropen. Returnerar en rad till statusfältet.
    pub fn poll_ai_chat(&mut self) -> Option<String> {
        let slot = self.ai_chat_pending.clone()?;
        let ready = slot.lock().ok().and_then(|mut g| g.take())?;
        self.ai_chat_pending = None;
        let line = match ready {
            Ok(text) => self.ai_apply_tool_calls(&text),
            Err(e) => {
                let line = format!("⚠ Assistenten svarade inte: {e}");
                self.ai_assistant.last_error = Some(e);
                line
            }
        };
        self.ai_assistant.last_status = line.clone();
        Some(line)
    }

    /// Tolkar modellens svar: antingen text (då visas den) eller verktygsanrop (då utförs de).
    pub fn ai_apply_tool_calls(&mut self, text: &str) -> String {
        if !(text.trim_start().starts_with('{') || text.trim_start().starts_with('[')) {
            self.ai_assistant.last_reply = text.trim().to_string();
            return format!("💬 {}", first_line(text));
        }
        let calls = match ai_tools::parse_calls(text) {
            Ok(calls) => calls,
            Err(e) => return format!("⚠ Kunde inte läsa svaret: {e}"),
        };

        let mut done = Vec::new();
        for call in calls {
            match call.tool.as_str() {
                "set_tempo" => {
                    let bpm = call.args.get("bpm").and_then(|v| v.as_f64());
                    match bpm {
                        Some(bpm) if (40.0..=240.0).contains(&bpm) => {
                            self.bpm = bpm as f32;
                            done.push(format!("tempo {bpm:.0} BPM"));
                        }
                        Some(bpm) => done.push(format!("tempo {bpm:.0} ligger utanför 40–240")),
                        None => done.push("tempo saknade bpm".to_string()),
                    }
                }
                "write_drum_pattern" => {
                    let style = call
                        .args
                        .get("style")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let bars = call.args.get("bars").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                    match self.ai_write_drum_pattern(&style, bars) {
                        Ok(line) => done.push(line),
                        Err(e) => done.push(format!("trumkomp: {e}")),
                    }
                }
                "mute_track" => {
                    let idx = call.args.get("track").and_then(|v| v.as_u64()).unwrap_or(999) as usize;
                    let muted = call.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(true);
                    match self.playlist_tracks.get_mut(idx) {
                        Some(track) => {
                            track.muted = muted;
                            done.push(format!(
                                "spår {} {}",
                                idx + 1,
                                if muted { "tystat" } else { "påslaget" }
                            ));
                        }
                        None => done.push(format!("spår {} finns inte", idx + 1)),
                    }
                }
                "plan_sections" => match ai_tools::parse_section_plan(&call.args) {
                    Ok(plan) => {
                        let summary = plan
                            .iter()
                            .map(|s| format!("{} ({} takter)", s.name, s.bars))
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.ai_pending_plan = Some(plan);
                        done.push(format!("plan att bekräfta: {summary}"));
                    }
                    Err(e) => done.push(format!("plan: {e}")),
                },
                "set_key" => done.push("tonart: inte byggt än".to_string()),
                "set_channel_instrument" => done.push("instrumentbyte: inte byggt än".to_string()),
                other => done.push(format!("okänt verktyg '{other}'")),
            }
        }
        let line = format!("🤖 {}", done.join(" · "));
        self.ai_assistant.last_reply = line.clone();
        line
    }

    /// Skriver ett komp på de tre första kanalerna (kick, virvel, hi-hat). Kanalerna måste finnas
    /// och vara tomma just nu — AI:n får inte skriva över ett komp användaren redan har.
    fn ai_write_drum_pattern(&mut self, style: &str, bars: usize) -> Result<String, String> {
        let (kick, snare, hat) = ai_tools::drum_layers(style, bars)?;
        if self.channels.len() < 3 {
            return Err("behöver minst tre kanaler (kick, virvel, hi-hat)".to_string());
        }
        let busy: Vec<bool> = self.channels.iter().take(3).map(|c| c.steps.iter().any(|s| *s)).collect();
        let names: Vec<String> = self.channels.iter().take(3).map(|c| c.name.clone()).collect();
        ai_tools::ensure_channels_free(&busy, &names)?;
        for (i, (layer, note)) in [(&kick, 36u8), (&snare, 38), (&hat, 42)].into_iter().enumerate() {
            let ch = &mut self.channels[i];
            ch.steps = *layer;
            for step in 0..ai_tools::STEPS {
                if layer[step] {
                    ch.notes[step] = note;
                }
            }
        }
        self.sync_active_pattern_from_ui();
        Ok(format!("trumkomp '{style}' i {bars} takter"))
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .chars()
        .take(160)
        .collect()
}

