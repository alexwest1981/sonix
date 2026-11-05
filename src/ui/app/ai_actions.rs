//! **AI:ns händer.** Tar modellens verktygsanrop och utför dem mot appen — efter validering.
//!
//! Bara verktyg som redan har en säker väg i appen finns här. `set_channel_instrument` har en
//! utförare: den matchar namnet mot `SYNTH_PRESETS` och byter syntens förval — den *hittar inte
//! på* ett ljud, och ett namn som inte finns avvisas med listan över kända.

use std::sync::{Arc, Mutex};

use crate::audio::ai_tools;

use super::{SharedResult, SonixApp, SunoMeta, SunoTask};

/// **Ett kvitto med sin dom.** `ok` sätts av utföraren, aldrig av texten: en MCP-klient avgör på
/// `isError`, och ett avvisat anrop som ser lyckat ut är värre än inget kvitto alls.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiReceipt {
    pub ok: bool,
    pub text: String,
}

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
        self.ai_assistant.follow_ups = 0;
        let config = self.ai_assistant.config.clone();
        // Kanalerna följer med prompten: modellen ska skriva mot kanalernas egna nummer och
        // namn, inte gissa index.
        let text = format!("{text}\n\n[{}]", self.ai_channel_context());
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
            Ok(text) => {
                // **Svarar den bara med tempo och kit frågar vi igen.** Mätt mot
                // `deepseek-flash`: första svaret är set_tempo + set_key + load_drum_kit, och
                // själva kompet kommer först när den får veta att de kördes. Utan den andra
                // rundan stod assistenten still efter halva jobbet.
                let wrote = ["write_pattern", "write_notes", "write_drum_pattern"]
                    .iter()
                    .any(|w| text.contains(w));
                let receipt = self.ai_apply_tool_calls(&text);
                if !wrote && self.ai_assistant.follow_ups < 2 && !text.trim().is_empty() {
                    self.ai_send_follow_up();
                }
                receipt
            }
            Err(e) => {
                let line = format!("⚠ Assistenten svarade inte: {e}");
                self.ai_assistant.last_error = Some(e);
                line
            }
        };
        self.ai_assistant.last_status = line.clone();
        Some(line)
    }

    /// Runda två: talar om vad som redan kördes och ber om själva musiken. Samma tråd och
    /// samma kvittoslott som första rundan — ingen ny väg genom nätet.
    fn ai_send_follow_up(&mut self) {
        if self.ai_chat_pending.is_some() {
            return;
        }
        self.ai_assistant.follow_ups += 1;
        let config = self.ai_assistant.config.clone();
        let done = self.ai_assistant.last_status.clone();
        let text = format!(
            "Du körde: {done}.\nSkriv nu själva musiken: write_pattern med lanes för trummorna \
             (och kanske perc) och write_notes för bas/synt. Svara bara med JSON-anrop.\n\n[{}]",
            self.ai_channel_context()
        );
        let slot: Arc<Mutex<Option<Result<String, String>>>> = Arc::new(Mutex::new(None));
        self.ai_chat_pending = Some(slot.clone());
        std::thread::spawn(move || {
            let result = crate::audio::ai_client::chat_with_tools(&config, &text);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
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
            done.push(self.ai_execute_call(&call).text);
        }
        let line = format!("🤖 {}", done.join(" · "));
        self.ai_assistant.last_reply = line.clone();
        line
    }

    /// Utför **ett** verktygsanrop. Delas av assistentvyn och HTTP-tjänsten, så en regel bara
    /// finns på ett ställe.
    pub fn ai_execute_call(&mut self, call: &crate::audio::ai_tools::Call) -> AiReceipt {
        match self.ai_run_call(call) {
            Ok(text) => AiReceipt { ok: true, text },
            Err(text) => AiReceipt { ok: false, text },
        }
    }

    /// Själva utförandet. `Ok` = anropet gjorde det det bad om, `Err` = det gjorde inget (och
    /// texten säger varför). Flaggan får aldrig gissas ur texten: en MCP-klient avgör på
    /// `isError`, och ett avvisat anrop som ser lyckat ut är värre än ingen kvitto alls.
    fn ai_run_call(&mut self, call: &crate::audio::ai_tools::Call) -> Result<String, String> {
        match call.tool.as_str() {
            "set_tempo" => {
                let bpm = call.args.get("bpm").and_then(|v| v.as_f64());
                match bpm {
                    Some(bpm) if (40.0..=240.0).contains(&bpm) => {
                        self.set_project_bpm(bpm as f32);
                        Ok(format!("tempo {bpm:.0} BPM"))
                    }
                    Some(bpm) => Err(format!("tempo {bpm:.0} ligger utanför 40–240")),
                    None => Err("tempo saknade bpm".to_string()),
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
                    Ok(line) => Ok(line),
                    Err(e) => Err(format!("trumkomp: {e}")),
                }
            }
            "mute_track" => {
                let idx = call.args.get("track").and_then(|v| v.as_u64()).unwrap_or(999) as usize;
                let muted = call.args.get("muted").and_then(|v| v.as_bool()).unwrap_or(true);
                match self.playlist_tracks.get_mut(idx) {
                    Some(track) => {
                        track.muted = muted;
                        Ok(format!("spår {} {}", idx + 1, if muted { "tystat" } else { "påslaget" }))
                    }
                    None => Err(format!("spår {} finns inte", idx + 1)),
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
                    Ok(format!("plan att bekräfta: {summary}"))
                }
                Err(e) => Err(format!("plan: {e}")),
            },
            "write_pattern" => match crate::audio::ai_tools::parse_lanes(&call.args) {
                Ok(lanes) => {
                    let mut done = Vec::new();
                    let mut failed = Vec::new();
                    for lane in &lanes {
                        match self.ai_write_lane(lane) {
                            Ok(line) => done.push(line),
                            Err(e) => failed.push(e),
                        }
                    }
                    self.sync_active_pattern_from_ui();
                    done.extend(failed.iter().cloned());
                    // **En stämma som avvisades gör hela anropet avvisat.** Annars rapporteras ett
                    // halvt komp som lyckat (EGM runda 4).
                    if failed.is_empty() {
                        Ok(done.join(" · "))
                    } else {
                        Err(done.join(" · "))
                    }
                }
                Err(e) => Err(format!("mönster: {e}")),
            },
            "write_notes" => self.ai_write_notes(&call.args),
            "load_drum_kit" => {
                let genre = call.args.get("genre").and_then(|v| v.as_str()).unwrap_or("");
                self.ai_load_drum_kit(genre)
            }
            "analyze_mix" => self.ai_analyze_mix(),
            "groove_report" => {
                let track = call.args.get("track").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
                self.ai_groove_report(track)
            }
            "chop_channel" => {
                let channel = call.args.get("channel").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
                let spectral = call.args.get("spectral").and_then(|v| v.as_bool()).unwrap_or(false);
                self.ai_chop_channel(channel, spectral)
            }
            "rename_track" => {
                let track = call.args.get("track").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
                let name = call.args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                self.ai_rename_track(track, name)
            }
            "write_script" => {
                let source = call.args.get("source").and_then(|v| v.as_str()).unwrap_or("");
                self.ai_write_script(source)
            }
            "master_to_target" => {
                let lufs = call.args.get("lufs").and_then(|v| v.as_f64()).unwrap_or(f64::NAN);
                self.ai_master_to_target(lufs as f32)
            }
            "set_key" => Err("tonart: inte byggt än".to_string()),
            "set_channel_instrument" => {
                let wanted = call.args.get("instrument").and_then(|v| v.as_str()).unwrap_or("");
                self.ai_set_instrument(wanted)
            }
            other => Err(format!("okänt verktyg '{other}'")),
        }
    }

    /// Byter syntljud. Namnet måste matcha ett av `SYNTH_PRESETS` (skiftlägesoberoende, delsträng
    /// räcker) — AI:n får inte hitta på ett ljud som inte finns.
    fn ai_set_instrument(&mut self, wanted: &str) -> Result<String, String> {
        if wanted.trim().is_empty() {
            return Err("instrument: inget namn angavs".to_string());
        }
        let Some((name, preset)) = crate::audio::command::find_preset(wanted) else {
            let names = crate::audio::command::SYNTH_PRESETS
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("instrument: '{wanted}' finns inte (kända: {names})"));
        };
        self.current_preset = preset;
        self.apply_preset_settings(preset);
        let _ = self.engine.send_command(crate::audio::command::AudioCommand::LoadPreset(preset));
        Ok(format!("instrument: {name}"))
    }

/// Skriver **en** stämma på en kanal. Kanalen måste finnas och vara tom: ett komp som redan
    /// spelar skrivs aldrig över av ett verktygsanrop, det sägs tillbaka i statusraden.
    fn ai_write_lane(&mut self, lane: &crate::audio::ai_tools::Lane) -> Result<String, String> {
        let hits = lane.steps.iter().filter(|s| **s).count();
        let Some(ch) = self.channels.get_mut(lane.channel) else {
            return Err(format!("kanal {} finns inte", lane.channel + 1));
        };
        if ch.steps.iter().any(|s| *s) {
            return Err(format!("kanal {} ({}) har redan steg — töm den först", lane.channel + 1, ch.name));
        }
        let name = ch.name.clone();
        ch.steps = lane.steps;
        if let Some(note) = lane.note {
            for (i, on) in lane.steps.iter().enumerate() {
                if *on {
                    ch.notes[i] = note;
                }
            }
        }
        Ok(format!("{name}: {hits} slag"))
    }

    /// Skriver melodi/bas på en kanal — som **en klippa på tidslinjen**, inte i rutnätet.
    ///
    /// Klippan bär sina egna noter (position, anslag och längd), så den går att dra utan att
    /// noterna rör sig och att spara som `.mid`. Rutnätet hade en not per steg och varken
    /// anslag eller notlängd — därför fastnade AI:ns melodi i mönstret.
    fn ai_write_notes(&mut self, args: &serde_json::Value) -> Result<String, String> {
        let notes = match crate::audio::ai_tools::parse_notes(args) {
            Ok(notes) => notes,
            Err(e) => return Err(format!("noter: {e}")),
        };
        let channel = args.get("channel").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
        let Some(ch) = self.channels.get(channel) else {
            return Err(format!("kanal {} finns inte", channel + 1));
        };
        let name = ch.name.clone();
        if ch.steps.iter().any(|s| *s) {
            return Err(format!("kanal {} ({}) har redan steg — töm den först", channel + 1, name));
        }
        let pattern = self.selected_pattern;
        let Some(last_track) = self.playlist_tracks.len().checked_sub(1) else {
            return Err("noter: det finns inget spår att lägga klippan på".to_string());
        };
        let track_idx = self.selected_timeline_track.min(last_track);
        if self.playlist_tracks[track_idx]
            .pattern_clips
            .iter()
            .any(|c| c.pattern == pattern && c.lane == channel && !c.notes.is_empty())
        {
            return Err(format!(
                "kanal {} ({}) har redan en klippa med noter — ta bort den först",
                channel + 1,
                name
            ));
        }
        // Längden täcker noterna **och** patternet: en klippa på en takt klipper av noter
        // AI:n skrev på steg 20, och en kortare än patternet spelar inte dess sista takter.
        let span_bars = notes.iter().map(|h| h.step / 16 + 1).max().unwrap_or(1);
        let bars = span_bars
            .max(self.patterns.get(pattern).map_or(1, |p| p.bars))
            .clamp(1, 4);
        let id = self
            .playlist_tracks
            .iter()
            .flat_map(|t| t.pattern_clips.iter().map(|c| c.id))
            .max()
            .unwrap_or(0)
            + 1;
        let clip_notes = notes
            .iter()
            .map(|h| crate::midi_take::TakeNote {
                // Klippans noter räknas i steg från klippans start — patternet kan vara fyra takter.
                pos: h.step as f32,
                key: h.pitch,
                velocity: h.velocity as f32 / 127.0,
                probability: 1.0,
                length_steps: h.length as f32,
                articulation: crate::midi_take::Articulation::Normal,
                applied: false,
            })
            .collect();
        let start_bar = self.song_bar as f32;
        // shortcut: den offline-renderade exporten spelar klippor först i steg 5 (då med
        // `render_project_offline`); tills dess hörs en AI-melodi i appen men inte i en
        // exporterad mix. Rutnätet skrevs med flit inte — se `docs/AI-CLIPS.md`.
        self.playlist_tracks[track_idx]
            .pattern_clips
            .push(crate::ui::app::state::PatternClip {
                id,
                pattern,
                lane: channel,
                start_bar,
                length_bars: bars as f32,
                name: name.clone(),
                notes: clip_notes,
            });
        Ok(crate::tstatus!(
            "📎 '{}': {} noter i en klippa på takt {} ({} takter)",
            name,
            notes.len(),
            start_bar + 1.0,
            bars
        ))
    }

    /// **Väljer ljud ur användarens eget bibliotek.** Varje trumkanal får det sample som bäst
    /// matchar dess roll och den stil användaren bad om; en fil som inte går att läsa lämnas
    /// kanalen orörd (samma regel som i kanalracket), och då provas ingen annan fil — den hade
    /// gett ett annat ljud än det som står i statusraden.
    fn ai_load_drum_kit(&mut self, genre: &str) -> Result<String, String> {
        if self.sample_library.is_empty() {
            return Err("kit: biblioteket är tomt (skanna en mapp först)".to_string());
        }
        let items: Vec<(String, String)> = self
            .sample_library
            .iter()
            .map(|i| (i.name.clone(), i.category.clone()))
            .collect();
        let mut used: Vec<usize> = Vec::new();
        let mut placed: Vec<String> = Vec::new();
        for idx in 0..self.channels.len() {
            let Some(role) = crate::audio::ai_tools::role_of(&self.channels[idx].name) else {
                continue;
            };
            if !matches!(role, "kick" | "snare" | "clap" | "hat" | "openhat" | "crash" | "perc") {
                continue;
            }
            let Some(pick) = crate::audio::ai_tools::pick_sample(role, genre, &items, &used) else {
                continue;
            };
            let item = self.sample_library[pick].clone();
            if super::browser::assign_library_sample_to_channel(&mut self.channels[idx], &item) {
                used.push(pick);
                placed.push(format!("{role}: {}", item.name));
            }
        }
        if placed.is_empty() {
            Err(format!("kit: inga trumljud i biblioteket matchade '{genre}'"))
        } else {
            Ok(format!("kit ({genre}): {}", placed.join(", ")))
        }
    }

    /// Kanalerna modellen får adressera. Utan listan gissar den index, och då hamnar ett komp på
    /// fel kanal — den ska skriva mot kanalernas egna namn och nummer.
    pub fn ai_channel_context(&self) -> String {
        let list: Vec<String> = self
            .channels
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!("{} {} {}", i, c.icon, c.name)
            })
            .collect();
        // **Spåren och sektionerna också.** Utan dem gissar modellen vad som redan finns i
        // arrangemanget — och då skriver den över eller föreslår något som står där.
        let tracks: Vec<String> = self
            .playlist_tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut parts = vec![format!("{i} {}", t.name)];
                if t.muted {
                    parts.push("tystad".to_string());
                }
                if t.clips.iter().any(|c| c.is_some()) {
                    parts.push("pattern".to_string());
                }
                if !t.regions.is_empty() {
                    parts.push("ljud".to_string());
                }
                if t.pattern_clips.iter().any(|c| !c.notes.is_empty()) {
                    parts.push("klippa".to_string());
                }
                parts.join(" ")
            })
            .collect();
        let sections = if self.sections.is_empty() {
            "inga".to_string()
        } else {
            self.sections
                .iter()
                .map(|s| format!("{} takt {}–{}", s.name, s.start_bar + 1, s.start_bar + s.bars))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "Aktuellt projekt: {} BPM, {} kanaler, {} spår.\nKanaler: {}\nSpår: {}\nSektioner: {}",
            self.bpm,
            self.channels.len(),
            self.playlist_tracks.len(),
            list.join(" | "),
            if tracks.is_empty() { "inga".to_string() } else { tracks.join(" | ") },
            sections
        )
    }

    // =================================================================
    // Mätverktygen (AI:n får läsa siffror, inte gissa)
    // =================================================================

    /// Hur många takter mätningarna spelar upp. En hel låt per spår är för långsamt att vänta
    /// på i en samtalsrunda; 32 takter räcker för att höra vem som ligger var.
    /// shortcut: bara början av låten mäts — mät hela när en nivå skiljer sig mellan vers och refräng.
    const AI_MEASURE_BARS: usize = 32;

    /// Renderar ett spår för sig (eller hela mixen) och returnerar det uppmätta.
    fn ai_render_track(&self, track: usize) -> Result<Vec<f32>, String> {
        let mut spec = self.build_render_spec(Some(track), 48_000);
        spec.num_bars = spec.num_bars.clamp(1, Self::AI_MEASURE_BARS);
        spec.tail_secs = 0.0;
        self.render_buffer(&spec, false)
    }

    fn ai_render_mix(&self) -> Result<Vec<f32>, String> {
        let spec = self.build_render_spec(None, 48_000);
        self.render_buffer(&spec, false)
    }

    /// **Mixgranskningen.** LUFS och topp per spår, ur `audio::loudness`, plus sektionerna.
    /// Modellen får siffrorna som text — den får inte hitta på dem.
    fn ai_analyze_mix(&mut self) -> Result<String, String> {
        let count = self.playlist_tracks.len().min(16);
        if count == 0 {
            return Err("mätning: inga spår i arrangemanget".to_string());
        }
        let mut lines = Vec::new();
        for i in 0..count {
            let name = self.playlist_tracks[i].name.clone();
            match self.ai_render_track(i) {
                Ok(buf) if buf.iter().any(|s| s.abs() > 1.0e-4) => {
                    let lufs = crate::audio::loudness::integrated_lufs(&buf, 48_000);
                    let peak = crate::audio::loudness::true_peak_db(&buf);
                    lines.push(format!("{i} {name}: {lufs:.1} LUFS, topp {peak:.1} dB"));
                }
                Ok(_) => lines.push(format!("{i} {name}: tyst")),
                Err(e) => lines.push(format!("{i} {name}: kunde inte mätas ({e})")),
            }
        }
        let sections = if self.sections.is_empty() {
            "inga sektioner".to_string()
        } else {
            self.sections
                .iter()
                .map(|s| format!("{} takt {}–{}", s.name, s.start_bar + 1, s.start_bar + s.bars))
                .collect::<Vec<_>>()
                .join(", ")
        };
        Ok(format!(
            "mätt {} spår (första {} takterna). {} Sektioner: {}",
            count,
            Self::AI_MEASURE_BARS,
            lines.join(" | "),
            sections
        ))
    }

    /// **Ligger spåret på rutnätet?** Slagen hittas i den renderade spårmixen, avvikelsen räknas
    /// mot sextondelar. Skriver inget — det här är ett mått, inte ett ingrepp.
    fn ai_groove_report(&mut self, track: usize) -> Result<String, String> {
        let Some(t) = self.playlist_tracks.get(track) else {
            return Err(format!("spår {} finns inte", track + 1));
        };
        let name = t.name.clone();
        let buf = match self.ai_render_track(track) {
            Ok(b) => b,
            Err(e) => return Err(format!("mätning: {e}")),
        };
        // Vänsterkanalen räcker: slagen sitter på samma ställe i båda.
        let mono: Vec<f32> = buf.chunks(2).map(|f| f[0]).collect();
        let onsets = crate::audio::onset::detect_onsets(
            &mono,
            48_000.0,
            &crate::audio::onset::OnsetParams::default(),
        );
        let step_secs = 60.0 / self.bpm.max(20.0) / 4.0;
        match crate::audio::ai_tools::groove_drift_ms(&onsets, 48_000, step_secs) {
            Some((median, max, hits)) => Ok(format!(
                "spår {} ({name}): {hits} slag, median {median:.1} ms från rutnätet, värst {max:.1} ms",
                track + 1
            )),
            // **Inga slag är ett svar, inte ett fel.** `analyze_mix` säger samma sak om en tyst
            // mix: mätningen gjorde sitt jobb. Text säger uttryckligen att inget mättes, så en
            // modell kan inte påstå ett sväng — men klientens `isError` ska betyda "anropet
            // gjorde inte det det bad om", inte "det fanns inget att mäta" (EGM runda 5, fynd 8).
            None => Ok(format!(
                "spår {} ({name}): inga slag hittades — inget att mäta",
                track + 1
            )),
        }
    }

    /// **Chop.** Slagen letas i kanalens eget ljud (samma detektor som mixerpanelens knappar) och
    /// en slice per sextondel läggs i ett **tomt** mönster — ett komp som redan spelar rörs inte.
    fn ai_chop_channel(&mut self, channel: usize, spectral: bool) -> Result<String, String> {
        let Some(ch) = self.channels.get_mut(channel) else {
            return Err(format!("kanal {} finns inte", channel + 1));
        };
        let name = ch.name.clone();
        if ch.steps.iter().any(|s| *s) {
            return Err(format!("kanal {} ({name}) har redan steg — töm den först", channel + 1));
        }
        let Some((left, _right, sr)) = ch.pcm_audio.as_ref() else {
            return Err(format!("kanal {} ({name}): kanalens ljud går inte att läsa — inga slag kan hittas", channel + 1));
        };
        let Some((slices, _count)) = crate::audio::onset::detect_slice_map(
            left,
            *sr as f32,
            spectral,
            &crate::audio::onset::OnsetParams::default(),
        ) else {
            return Err(format!("kanal {} ({name}): inga slag hittades", channel + 1));
        };
        ch.sample_start = slices[0].0;
        ch.sample_end = slices[0].1;
        ch.slices = slices.clone();
        // En slice per sextondel, högst en takt: 64 slicar i rad är inte ett komp.
        let placed = slices.len().min(16);
        ch.steps.fill(false);
        for step in ch.steps.iter_mut().take(placed) {
            *step = true;
        }
        Ok(format!(
            "kanal {} ({name}): {} slicar hittade, {placed} i mönstret",
            channel + 1,
            slices.len()
        ))
    }

    /// Byler namn på ett spår. Tomt namn avvisas — ett namnlöst spår går inte att prata om.
    fn ai_rename_track(&mut self, track: usize, name: &str) -> Result<String, String> {
        let clean: String = name.trim().chars().take(40).collect();
        if clean.is_empty() {
            return Err("namn: inget namn angavs".to_string());
        }
        let Some(t) = self.playlist_tracks.get_mut(track) else {
            return Err(format!("spår {} finns inte", track + 1));
        };
        let old = std::mem::replace(&mut t.name, clean.clone());
        Ok(format!("spår {}: {old} → {clean}", track + 1))
    }

    /// **Rhai från modellen.** Skriptet kör mot en **kopia** av den valda tagningen — samma regel
    /// som skriptrutan i pianorullen — så ett fel halvvägs lämnar tagningen orörd. Men det kör
    /// **utan filsystemet**: `load_midi`/`save_midi` finns inte i modellens motor, för ett skript
    /// som kommer utifrån (HTTP/MCP) ska inte kunna skriva över vilken fil som helst.
    fn ai_write_script(&mut self, source: &str) -> Result<String, String> {
        if let Err(e) = crate::audio::ai_tools::script_is_allowed(source) {
            return Err(format!("skript: {e}"));
        }
        let pattern = self.selected_pattern;
        let Some(pat) = self.patterns.get_mut(pattern) else {
            return Err("skript: ingen tagning vald".to_string());
        };
        let mut copy = pat.take.clone();
        match crate::audio::script::run_isolated(source, &mut copy) {
            Ok(lines) => {
                let notes = copy.notes.len();
                pat.take = copy;
                if lines.is_empty() {
                    Ok(format!("skript: {notes} noter efter körning"))
                } else {
                    Ok(format!("skript: {notes} noter efter körning · {}", lines.join(" · ")))
                }
            }
            Err(e) => Err(format!("skript: {e} (tagningen orörd)")),
        }
    }

    /// **Mastering mot ett mått.** Närmaste loudness-preset sätts, mixen renderas en gång och
    /// normaliseras på samma väg som exporten (`normalize_to_preset`), och både före och efter
    /// rapporteras uppmätt. Modellen får inte påstå en nivå den inte kan mäta.
    fn ai_master_to_target(&mut self, target_lufs: f32) -> Result<String, String> {
        if !(-30.0..=0.0).contains(&target_lufs) {
            return Err(format!("master: {target_lufs:.1} LUFS är inget rimligt mål (−30 till 0)"));
        }
        let idx = crate::audio::ai_tools::nearest_loudness_index(target_lufs);
        let preset = crate::audio::LoudnessPreset::ALL[idx];
        let buf = match self.ai_render_mix() {
            Ok(b) => b,
            Err(e) => return Err(format!("master: kunde inte rendera ({e})")),
        };
        let before = crate::audio::loudness::integrated_lufs(&buf, 48_000);
        let mut normalized = buf;
        crate::audio::normalize_to_preset(&mut normalized, 48_000, preset);
        let after = crate::audio::loudness::integrated_lufs(&normalized, 48_000);
        let peak = crate::audio::loudness::true_peak_db(&normalized);
        self.export_loudness_idx = idx;
        Ok(format!(
            "master: mål {target_lufs:.1} LUFS → {} ({}). Uppmätt {before:.1} LUFS före, {after:.1} LUFS efter, topp {peak:.1} dB",
            preset.label(),
            preset.target_lufs()
        ))
    }

    /// **Bekräftar** en väntande sektionsplan: sektionerna läggs efter varandra från takt 1.
    /// Inget skrivs förrän hit — planen är ett förslag, inte ett ingrepp.
    pub fn apply_pending_plan(&mut self) -> String {
        let Some(plan) = self.ai_pending_plan.take() else {
            return "ingen plan att skapa".to_string();
        };
        // **Sektionerna skrivs här, alltså hör steget hit.** `plan_sections` lagrar bara en
        // väntande plan; knappen som lägger in den tog förut inget ångringssteg (EGM runda 5).
        self.push_undo("Sektioner från plan");
        for (name, start_bar, bars) in ai_tools::sections_from_plan(&plan, 0) {
            self.sections.push(crate::ui::app::state::Section { name, start_bar, bars });
        }
        self.status_message = crate::tstatus!("🧭 {} sektioner skapade", self.sections.len());
        self.status_message.clone()
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
            for (step, on) in layer.iter().enumerate() {
                if *on {
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

/// The file the listen step already saved for this take, if it is still on disk. Reusing it is
/// what keeps listen-then-import from paying the download credit twice.
fn saved_clip_file(clips: &[crate::ui::app::SunoClip], id: &str) -> Option<std::path::PathBuf> {
    clips
        .iter()
        .find(|clip| clip.id == id)
        .and_then(|clip| clip.file.clone())
        .filter(|file| file.exists())
}

impl SonixApp {
    /// **Det panelen behöver veta om en session.** Ett ställe, så den tysta starten och ett
    /// uttryckligt "logga in" ger panelen samma svar.
    fn suno_connected_task(
        session: &mut crate::audio::suno::SunoSession,
    ) -> Result<SunoTask, String> {
        let tools: Vec<String> = session.tools().iter().map(|t| t.name.clone()).collect();
        let tool_name = session.generation_tool().map(|t| t.name.clone());
        let schema = session.generation_schema();
        let voice_param = session.voice_param();
        // Voices are best-effort: a server without one is a normal beta surface, not an error.
        let voices = match session.voices() {
            Ok(list) => list.into_iter().map(|v| (v.id, v.name)).collect::<Vec<_>>(),
            Err(e) => {
                println!("suno: voices unavailable ({e})");
                Vec::new()
            }
        };
        let voice_hint = session.voices_hint();
        Ok(SunoTask::Connected {
            tools,
            tool_name,
            schema,
            voices,
            voice_hint,
            voice_param,
        })
    }

    /// **Ansluter med de sparade tokensen — och öppnar aldrig en webbläsare.**
    ///
    /// Körs en gång vid start. En app som öppnar en flik när man startar den är en webbläsare,
    /// inte en DAW (Alex 2026-11-05). Access-token lever en timme; refresh-token förnyas här
    /// utan att någon sida behöver visas. Går det inte är svaret "inte inloggad" i
    /// inställningarna — inloggning är ett uttryckligt val, inte något som händer av sig själv.
    pub fn suno_connect_silently(&mut self) {
        if self.suno_pending.is_some() {
            return;
        }
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_silent = true;
        self.suno_status = crate::i18n::t("☀ Kontrollerar Suno-inloggningen…").to_string();
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached().and_then(|mut session| {
                SonixApp::suno_connected_task(&mut session)
            });
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Signs in on a background thread: `connect_with_slot` opens the browser and blocks until
    /// the redirect arrives, which would freeze the UI thread.
    pub fn suno_connect(&mut self) {
        if self.suno_pending.is_some() {
            self.suno_status = crate::i18n::t("⏳ Already waiting on Suno.").to_string();
            return;
        }
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_silent = false;
        self.suno_status = crate::i18n::t(
            "☀ Connecting — finish the sign-in in the browser, then come back here.",
        )
        .to_string();
        let slot_url = self.suno_login_url.clone();
        std::thread::spawn(move || {
            let result =
                crate::audio::suno::SunoSession::connect_with_slot(Some(slot_url))
                    .and_then(|mut session| SonixApp::suno_connected_task(&mut session));
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Creates a song on a background thread and puts the result on the timeline.
    pub fn suno_create(&mut self) {
        let prompt = self.suno_prompt_input.trim().to_string();
        // Antingen en beskrivning (Simple) eller egna stilar/text (Custom) — båda är giltiga.
        let custom = self.suno_form.iter().any(|(name, field)| {
            matches!(name.as_str(), "style" | "lyrics")
                && matches!(field, crate::ui::app::suno_form::SunoField::Text { value, .. }
                    if !value.trim().is_empty())
        });
        // En cover eller mashup behöver ingen beskrivning — källåten är underlaget.
        let has_source = self.suno_source_song.is_some();
        if prompt.is_empty() && !custom && !has_source {
            self.suno_status = crate::i18n::t("⚠ Write a prompt first.").to_string();
            return;
        }
        if self.suno_pending.is_some() {
            self.suno_status = crate::i18n::t("⏳ Already waiting on Suno.").to_string();
            return;
        }
        // Formuläret är det som skickas: panelen har ritat det ur verktygets schema, så varje
        // fält bär serverns eget namn.
        let mut fields = self.suno_form.clone();
        // **Fältet heter `description`.** `audio-create` har `additionalProperties: false`, så ett
        // påhittat `prompt`-fält gav `invalid_request: invalid request body`.
        if let Some(slot) = crate::ui::app::suno_form::text_field_mut(
            &mut fields,
            &["description", "prompt"],
        ) {
            *slot = prompt.clone();
        }
        if !self.suno_voices.is_empty()
            && let Some(param) = self
                .suno_voice_param
                .clone()
                .or_else(|| Some("voice_id".to_string()))
            && let Some((voice_id, _)) = self.suno_voices.get(self.suno_voice_choice)
            && let Some((_, field)) = fields.iter_mut().find(|(name, field)| {
                *name == param
                    && matches!(field, crate::ui::app::suno_form::SunoField::Text { .. })
            })
        {
            *field = crate::ui::app::suno_form::SunoField::Text {
                value: voice_id.clone(),
                multiline: false,
            };
        }
        let mut args = crate::ui::app::suno_form::args_from_form(&fields);
        // **Rätt verktyg för valet.** Audio = cover av en låt, Audio + Inspo = mashup av två,
        // annars en helt ny låt. Fält verktyget inte har rensas bort i `generate_named`.
        let tool = match (&self.suno_source_song, &self.suno_inspo_song) {
            (Some((source, _)), Some((inspo, _))) => {
                if let Some(object) = args.as_object_mut() {
                    object.insert("id".to_string(), serde_json::json!(source));
                    object.insert("additional_audio_id".to_string(), serde_json::json!(inspo));
                }
                Some("audio-mashups-create")
            }
            (Some((source, _)), None) => {
                if let Some(object) = args.as_object_mut() {
                    object.insert("id".to_string(), serde_json::json!(source));
                }
                Some("audio-covers-create")
            }
            _ => None,
        };
        // `shortcut:` create connects again from the saved tokens instead of holding a session,
        // upgrade if the handshake ever costs real time.
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_status = crate::i18n::t("⏳ Suno is creating the two takes…").to_string();
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached().and_then(|mut session| {
                match tool {
                    Some(name) => session.generate_named(name, args),
                    None => session.generate_with(args),
                }
            })
            .map(SunoTask::Generated);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Lists the user's own Suno library on a background thread.
    pub fn suno_load_library(&mut self) {
        if self.suno_pending.is_some() {
            self.suno_status = crate::i18n::t("⏳ Already waiting on Suno.").to_string();
            return;
        }
        let query = self.suno_library_query.trim().to_string();
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_status = crate::i18n::t("⏳ Reading your Suno library…").to_string();
        std::thread::spawn(move || {
            let filter = Some(query.as_str()).filter(|q| !q.is_empty());
            let result = crate::audio::suno::SunoSession::connect_cached()
                .and_then(|mut session| session.library(filter, 50))
                .map(SunoTask::Library);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Fills the clip rows with title and length — `audio-retrieve` is metadata only, no credit.
    pub fn suno_load_clip_titles(&mut self) {
        if self.suno_clips.is_empty() || self.suno_pending.is_some() {
            return;
        }
        let ids: Vec<String> = self.suno_clips.iter().map(|c| c.id.clone()).collect();
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached().and_then(|mut session| {
                let mut songs = Vec::new();
                for id in &ids {
                    if let Ok(song) = session.retrieve_song(id) {
                        songs.push(song);
                    }
                }
                Ok(SunoTask::Clips(songs))
            });
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Auditions one alternative: the audio costs a download credit, then the file opens in the
    /// system player so both takes can be compared.
    pub fn suno_audition(&mut self, index: usize) {
        let Some(clip) = self.suno_clips.get(index).cloned() else {
            return;
        };
        if let Some(file) = clip.file.clone() {
            crate::platform::open_path(&file.to_string_lossy());
            return;
        }
        if self.suno_pending.is_some() {
            return;
        }
        let id = clip.id.clone();
        self.suno_clip_pending = Some(index);
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_status = crate::i18n::t("📥 Hämtar ljudet — en kredit.").to_string();
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached()
                .and_then(|mut session| session.download_song_confirmed(&id))
                .map(SunoTask::Generated);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Plays one library row: the file is fetched (one credit) and opened in the system player.
    pub fn suno_play_library(&mut self, index: usize) {
        let Some(song) = self.suno_library.get(index).cloned() else {
            return;
        };
        if self.suno_pending.is_some() {
            return;
        }
        // **Redan hämtad = gratis.** Det är *hämtningen* som kostar en nedladdningskredit, inte
        // lyssnandet — men den här vägen hämtade om låten varje gång, även en som redan låg på
        // disk. Samma regel som alternativflikarna har haft hela tiden (Alex 2026-11-05:
        // "det kostar tokens för att ens lyssna på en redan färdig låt i biblioteket").
        if let Some(file) = saved_clip_file(&self.suno_clips, &song.id) {
            crate::platform::open_path(&file.to_string_lossy());
            self.suno_status =
                crate::tstatus!("🎧 Spelar '{}' från disken — inget hämtat, ingen kredit", song.title);
            return;
        }
        self.suno_library_pending = Some(index);
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_status = crate::tstatus!("🎧 Hämtar {} — en kredit…", song.title);
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached()
                .and_then(|mut session| session.download_song_confirmed(&song.id))
                .map(SunoTask::Generated);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Separates the chosen alternative — in place in the Suno tab, with the progress bar.
    pub fn suno_separate_clip(&mut self, index: usize) {
        let Some(file) = self
            .suno_clips
            .get(index)
            .and_then(|clip| clip.file.clone())
        else {
            self.suno_status =
                crate::i18n::t("⚠ Lyssna på alternativet först — filen hämtas då.").to_string();
            return;
        };
        self.suno_chain = false;
        self.start_stem_separation_with(&file.to_string_lossy(), false);
    }

    /// Sends the finished stems to the timeline and lands there.
    pub fn suno_export_stems(&mut self) {
        let folder = self
            .stem_project
            .stem_files
            .first()
            .and_then(|p| std::path::Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string());
        let Some(folder) = folder else {
            self.suno_status = crate::i18n::t("⚠ Inga stämmor att exportera än.").to_string();
            return;
        };
        let title = self.stem_project.track_title.clone();
        // **Tempo från själva låten.** Sunos API lämnar ingen BPM i metadata (mätt: duration,
        // description, lyrics, type — inget tempo), så det mäts ur den hämtade filen och blir
        // projektets tempo; annars ligger stämmorna i fel takt mot resten.
        let bpm = self
            .stem_project
            .source_path
            .as_deref()
            .and_then(|p| crate::audio::stem_separator::detect_bpm_from_file(p).ok())
            .map(|bpm| {
                println!("suno: tempo ur filen: {bpm:.1} BPM");
                bpm
            })
            .unwrap_or(self.bpm);
        self.bpm = bpm;
        self.import_suno_stems_from_folder(&folder, &title, bpm);
        self.view_mode = crate::ui::app::ViewMode::PlaylistArranger;
        self.suno_status = crate::i18n::t("✔ Stämmorna ligger på tidslinjen.").to_string();
    }

    /// Imports the song whose download the user just confirmed — one credit is spent, unless the
    /// listen step already saved the file: then that file is imported and nothing is fetched.
    pub fn suno_import_confirmed(&mut self, id: &str) {
        if self.suno_pending.is_some() {
            return;
        }
        if let Some(file) = saved_clip_file(&self.suno_clips, id) {
            let title = self
                .suno_clips
                .iter()
                .find(|clip| clip.id == id)
                .map(|clip| clip.title.clone())
                .unwrap_or_default();
            self.install_suno_song(crate::audio::suno::GeneratedSong {
                files: vec![file],
                stems: false,
                title,
                raw: serde_json::Value::Null,
                clip_ids: Vec::new(),
            });
            return;
        }
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        self.suno_status = crate::i18n::t("⏳ Downloading — one credit.").to_string();
        let id = id.to_string();
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached()
                .and_then(|mut session| session.download_song_confirmed(&id))
                .map(SunoTask::Generated);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Playlists, workspaces and account share one shape: fetch on a thread, park the answer.
    pub fn suno_load_meta(&mut self, which: SunoMeta) {
        if self.suno_pending.is_some() {
            self.suno_status = crate::i18n::t("⏳ Already waiting on Suno.").to_string();
            return;
        }
        let slot: SharedResult<SunoTask> = Arc::new(Mutex::new(None));
        self.suno_pending = Some(slot.clone());
        std::thread::spawn(move || {
            let result = crate::audio::suno::SunoSession::connect_cached().and_then(|mut session| {
                Ok(match which {
                    SunoMeta::Playlists => SunoTask::Playlists(session.playlists()?),
                    SunoMeta::Workspaces => SunoTask::Workspaces(session.workspaces()?),
                    SunoMeta::Account => SunoTask::Account(Box::new(session.account()?)),
                })
            });
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Polls the one slot; called every frame.
    pub fn poll_suno(&mut self) {
        let Some(slot) = self.suno_pending.clone() else {
            return;
        };
        let taken = slot.lock().ok().and_then(|mut guard| guard.take());
        let Some(result) = taken else {
            return;
        };
        self.suno_pending = None;
        match result {
            Ok(SunoTask::Connected {
                tools,
                tool_name,
                schema,
                voices,
                voice_hint,
                voice_param,
            }) => {
                self.suno_tools = tools;
                // Arbetsytorna behövs för "Spara i": väljaren kan inte fyllas av gissningar.
                self.suno_load_meta(SunoMeta::Workspaces);
                self.suno_tool_name = tool_name;
                // The server's schema decides the fields, but a value typed before sign-in is not
                // thrown away: it is carried over by field name.
                let typed = std::mem::take(&mut self.suno_form);
                self.suno_form = crate::ui::app::suno_form::merge_fields(
                    &typed,
                    schema
                        .as_ref()
                        .map(crate::ui::app::suno_form::fields_from_schema)
                        .unwrap_or_default(),
                );
                self.suno_voices = voices;
                self.suno_voice_hint = voice_hint;
                self.suno_voice_param = voice_param;
                self.suno_voice_choice = 0;
                self.suno_signed_in = true;
                let field_count = self.suno_form.len();
                // Vid start räcker ett kort besked: en rad som skvallrar om schemafält hör
                // till det uttryckliga inloggningen, inte till uppstarten.
                self.suno_status = if self.suno_silent {
                    crate::i18n::t("☀ Inloggad på Suno").to_string()
                } else {
                    crate::tstatus!(
                        "✔ Signed in — {} fields from the server's own schema, {} voices.",
                        field_count,
                        self.suno_voices.len()
                    )
                };
                if let Some(hint) = self.suno_voice_hint.clone() {
                    println!("{hint}");
                }
            }
            Ok(SunoTask::Generated(song)) => self.install_suno_song(song),
            Ok(SunoTask::Playlists(list)) => {
                self.suno_playlists = list;
                self.suno_status = String::new();
            }
            Ok(SunoTask::Workspaces(list)) => {
                self.suno_workspaces = list;
                // Modellistan ligger i kontosvaret; ta den direkt efter, en fråga i taget.
                self.suno_load_meta(SunoMeta::Account);
            }
            Ok(SunoTask::Clips(songs)) => {
                for song in songs {
                    if let Some(clip) = self.suno_clips.iter_mut().find(|c| c.id == song.id) {
                        clip.title = song.title;
                        clip.seconds = song.duration_secs;
                    }
                }
                self.suno_status = String::new();
            }
            Ok(SunoTask::Account(account)) => {
                self.suno_account = Some(*account);
                self.suno_status = String::new();
            }
            Ok(SunoTask::Library(songs)) => {
                self.suno_status = crate::tstatus!("☀ {} songs in the library.", songs.len());
                self.suno_library = songs;
            }
            Err(error) => {
                self.suno_signed_in = false;
                // **Ett misslyckande vid start får inte se ut som att appen väntar på en
                // webbläsare.** Den säger var inloggningen bor, och ingenting öppnas.
                self.suno_status = if self.suno_silent {
                    crate::i18n::t("☀ Inte inloggad på Suno — logga in i Inställningar → AI")
                        .to_string()
                } else {
                    error
                };
            }
        }
    }

    /// One file is one clip, several are one channel per file, a `.zip` is the existing
    /// stem-bundle import.
    fn install_suno_song(&mut self, song: crate::audio::suno::GeneratedSong) {
        use crate::ui::app::import::{SunoArrival, classify_suno_files};
        // **Kedjan.** Servern skapade clips men lämnade inga filer: ljudet kostar en
        // nedladdningskredit. Är hämtningen förvald tar vi den nu, utan ett extra klick.
        if self.suno_library_pending.take().is_some() {
            if let Some(file) = song.files.first().cloned() {
                crate::platform::open_path(&file.to_string_lossy());
                self.suno_status = crate::i18n::t("🎧 Spelar upp i systemets spelare.").to_string();
            }
            self.suno_last = Some(song);
            return;
        }
        if let Some(index) = self.suno_clip_pending.take() {
            // Hämtningen gällde lyssning: filen hör till raden, inte till arrangemanget.
            if let Some(file) = song.files.first().cloned() {
                if let Some(clip) = self.suno_clips.get_mut(index) {
                    clip.file = Some(file.clone());
                }
                crate::platform::open_path(&file.to_string_lossy());
                self.suno_status = crate::i18n::t("🎧 Spelar upp i systemets spelare.").to_string();
            }
            self.suno_last = Some(song);
            return;
        }
        if song.files.is_empty() && !song.clip_ids.is_empty() {
            // Två alternativ, ingen fil: visa dem och låt användaren välja.
            self.suno_clips = song
                .clip_ids
                .iter()
                .map(|id| crate::ui::app::SunoClip {
                    id: id.clone(),
                    title: String::new(),
                    seconds: None,
                    file: None,
                })
                .collect();
            self.suno_status = crate::tstatus!(
                "✅ {} alternativ klara — lyssna och välj ett.",
                self.suno_clips.len()
            );
            self.suno_last = Some(song);
            self.suno_load_clip_titles();
            return;
        }
        // **Vart ljudet ska.** Bordet ger ett spår per fil; separatorn tar första filen som mix
        // och delar den i stämspår — samma väg som en importerad fil.
        match classify_suno_files(&song.files) {
            SunoArrival::One(path) => {
                let tmpl = crate::ui::add_track_modal::get_track_templates()
                    .into_iter()
                    .find(|t| t.category == crate::ui::add_track_modal::AddTrackCategory::Import)
                    .expect("the add-track modal ships an import template");
                self.import_audio_file_as_track(&path.to_string_lossy(), &tmpl);
                self.suno_status =
                    crate::i18n::t("✔ One file — imported as one clip.").to_string();
            }
            SunoArrival::Stems(files) => match files.first().and_then(|p| p.parent()) {
                // One folder per song, so the folder is exactly these files; the stem importer
                // lists and numbers them in file-name order.
                Some(dir) => {
                    self.import_suno_stems_from_folder(&dir.to_string_lossy(), &song.title, 0.0);
                    self.suno_status = crate::tstatus!(
                        "✔ {} files — imported as stems, one channel per file.",
                        files.len()
                    );
                }
                None => {
                    self.suno_status =
                        crate::i18n::t("⚠ Suno returned no files.").to_string();
                }
            },
            SunoArrival::Zip(path) => {
                self.import_suno_zip(&path.to_string_lossy());
                self.suno_status =
                    crate::i18n::t("✔ The ZIP went to the stem import.").to_string();
            }
        }
        self.suno_last = Some(song);
    }
}

#[cfg(test)]
mod tests {
    use super::saved_clip_file;
    use crate::ui::app::SunoClip;

    /// Listening then importing must not pay the download credit twice: the take the listen step
    /// saved is reused, so the import makes no `audio-download` tool call.
    #[test]
    fn a_saved_take_is_imported_without_a_second_download() {
        let dir = std::env::temp_dir().join(format!("sonix-suno-saved-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let saved = dir.join("take-a.wav");
        std::fs::write(&saved, b"RIFF").expect("write the saved take");
        let clips = vec![
            SunoClip {
                id: "clip-1".to_string(),
                title: "Take A".to_string(),
                seconds: Some(12.0),
                file: Some(saved.clone()),
            },
            // The file was deleted behind us: this row still needs the download credit.
            SunoClip {
                id: "clip-2".to_string(),
                title: "Take B".to_string(),
                seconds: Some(12.0),
                file: Some(dir.join("gone.wav")),
            },
        ];
        // The import spawns a download only when the lookup comes back empty; count those calls.
        let mut download_tool_calls = 0;
        let source = saved_clip_file(&clips, "clip-1");
        if source.is_none() {
            download_tool_calls += 1;
        }
        println!("reused file: {source:?}");
        println!("download tool calls: {download_tool_calls}");
        assert_eq!(source, Some(saved));
        assert_eq!(download_tool_calls, 0, "the saved take was fetched again");
        println!(
            "a row without a file: {:?}",
            saved_clip_file(&clips, "clip-2")
        );
        assert_eq!(saved_clip_file(&clips, "clip-2"), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}

