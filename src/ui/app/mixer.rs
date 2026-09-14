//! Mixern — kanalracket, FX-racket och automationen.
//!
//! Automationskurvan interpoleras i `lane_value_at` (i `state`) och läses av
//! `apply_automation` varje bildruta. Målen kommer från `AutomationParam::ALL` —
//! **härled listan ur listan**; ett handskrivet index pekar tyst fel.
//!
//! Status: byggs — mixern och FX-racket.
//! Rör inte: kommandona bär hela inställningen; att bygga en ny nollar tyst frekvenser och Q.

use super::*;

/// Automatically classify stem and project tracks by name to assign distinct category color, icon, and track kind.
pub fn classify_track_style(name: &str) -> (TrackKind, &'static str, Color32) {
    let n = name.to_lowercase();
    if n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || (n.contains("voice") && (n.contains("rec") || n.contains("mic") || n.contains("sång"))) {
        (TrackKind::VocalAudio, "🎤", Color32::WHITE)
    } else if n.contains("lead") || n.contains("huvudsång") || n.contains("0 lead") || n.contains("leadvocal") || n.contains("lead vox") {
        (TrackKind::VocalAudio, "🎙", Color32::from_rgb(0, 225, 245)) // Vibrant Cyan
    } else if n.contains("backing") || n.contains("backvocal") || n.contains("back vocal") || n.contains("chorus") || n.contains("harmony") || n.contains("harmonies") || n.contains("bgv") || n.contains("1 backing") || n.contains("choir") || n.contains("kör") || n.contains("stämmor") || n.contains("vocal") || n.contains("vox") || n.contains("sång") {
        (TrackKind::VocalAudio, "🗣", Color32::from_rgb(225, 75, 235)) // Magenta / Purple
    } else if n.contains("percussion") || n.contains("6 percussion") || n.contains("perc") || n.contains("shaker") || n.contains("conga") || n.contains("bongo") || n.contains("tambourine") || n.contains("cowbell") {
        (TrackKind::Drums, "🪘", Color32::from_rgb(255, 105, 50)) // Vibrant Coral Rust
    } else if n.contains("drum") || n.contains("2 drums") || n.contains("trumm") || n.contains("kick") || n.contains("snare") || n.contains("hihat") || n.contains("hat") || n.contains("beat") || n.contains("slagverk") {
        (TrackKind::Drums, "🥁", Color32::from_rgb(255, 140, 25)) // Electric Orange
    } else if n.contains("bass") || n.contains("3 bass") || n.contains("bas") || n.contains("sub") || n.contains("808") {
        (TrackKind::Bassline, "🎸", Color32::from_rgb(45, 225, 105)) // Vivid Neon Green
    } else if n.contains("string") || n.contains("7 strings") || n.contains("stråk") || n.contains("violin") || n.contains("cello") || n.contains("orchestra") || n.contains("orchestral") {
        (TrackKind::SynthLead, "🎻", Color32::from_rgb(40, 220, 185)) // Emerald / Aquamarine Teal
    } else if n.contains("guitar") || n.contains("4 guitar") || n.contains("gitarr") || n.contains("acoustic") || n.contains("electric") || n.contains("strum") || n.contains("gtr") {
        (TrackKind::SynthLead, "🎸", Color32::from_rgb(255, 195, 30)) // Golden Amber
    } else if n.contains("key") || n.contains("5 keyboard") || n.contains("piano") || n.contains("flygel") || n.contains("organ") || n.contains("orgel") || n.contains("clav") || n.contains("rhodes") {
        (TrackKind::SynthLead, "🎹", Color32::from_rgb(65, 155, 255)) // Royal / Electric Blue
    } else if n.contains("synth") || n.contains("8 synth") || n.contains("synthesizer") || n.contains("pad") || n.contains("pluck") || n.contains("arp") || n.contains("brass") {
        (TrackKind::SynthLead, "🎛", Color32::from_rgb(175, 95, 255)) // Neon Violet
    } else if n.contains("fx") || n.contains("sfx") || n.contains("riser") || n.contains("sweep") || n.contains("impact") || n.contains("noise") || n.contains("effekt") || n.contains("atmos") || n.contains("other") || n.contains("9 other") {
        (TrackKind::Fx, "✨", Color32::from_rgb(255, 80, 150)) // Rose Pink
    } else {
        (TrackKind::CustomAudio, "🎵", Color32::from_rgb(160, 175, 200))
    }
}

impl SonixApp {
/// Pulls the real output waveform samples produced since the last UI frame
/// and keeps a sliding history for the oscilloscope display.
pub(crate) fn update_scope_history(&mut self) {
    let new = self.engine.drain_scope_samples();
    let had_new = !new.is_empty();
    if had_new {
        self.scope_history.extend(new);
        if self.scope_history.len() > Self::SCOPE_HISTORY_MAX {
            let excess = self.scope_history.len() - Self::SCOPE_HISTORY_MAX;
            self.scope_history.drain(0..excess);
        }
    }
    self.update_spectrum_levels(had_new);
}
}

impl SonixApp {
/// Registret i utgången (Fas 8.13): bandnivåerna räknas ur scope-historiken.
///
/// Registret och nivåmätaren använder **samma** dB-skala (`spectrum::FLOOR_DB`),
/// så de två kan inte visa olika höjd för samma ljud. Golvet är satt ur en
/// mätning: Alex' stämmor toppar mellan 0,005 och 0,36, och ett högre golv hade
/// gjort registret till en tom ruta på riktig musik.
///
/// Utan nya sampel ska mätaren **falla**, inte stå kvar och se levande ut. Ett
/// instrument som visar något fastän inget spelas är samma sorts lögn som de
/// tysta klippen i 8.5 — därför är det två vägar, inte en.
fn update_spectrum_levels(&mut self, had_new: bool) {
    if self.spectrum_levels.len() != crate::audio::spectrum::REGISTERS.len() {
        self.spectrum_levels = vec![0.0; crate::audio::spectrum::REGISTERS.len()];
    }
    if had_new {
        let rate = self.engine.sample_rate.max(1) as f32;
        let new = crate::audio::spectrum::band_levels(&self.scope_history, rate);
        // Upp fort, ned långsamt — en mätare som studsar är omöjlig att läsa.
        crate::audio::spectrum::smooth(&mut self.spectrum_levels, &new, 0.55, 0.12);
    } else {
        for level in &mut self.spectrum_levels {
            *level *= 0.85;
        }
    }
}
}

impl SonixApp {
/// Lägger ett pattern i UI:t — kanalernas steg/toner och piano-rollen —
/// **utan** att först spara det som står där.
///
/// Används av ångringen (Fas 6.4), som redan har skrivit tillbaka rätt
/// `patterns` och inte får skriva över dem med det gamla UI-läget.
pub(crate) fn load_pattern_into_ui(&mut self, pat_idx: usize) {
    if pat_idx >= self.patterns.len() {
        return;
    }
    for (ch_i, ch) in self.channels.iter_mut().enumerate() {
        if ch_i < self.patterns[pat_idx].channel_steps.len() {
            ch.steps = self.patterns[pat_idx].channel_steps[ch_i];
            ch.notes = self.patterns[pat_idx].channel_notes[ch_i];
        }
    }
    self.piano_roll_grid = self.patterns[pat_idx].piano_roll_grid;
}
}

impl SonixApp {
pub fn select_pattern(&mut self, pat_idx: usize) {
    if pat_idx < self.patterns.len() {
        // Save current channel steps to current pattern
        for (ch_i, ch) in self.channels.iter().enumerate() {
            if ch_i < self.patterns[self.selected_pattern].channel_steps.len() {
                self.patterns[self.selected_pattern].channel_steps[ch_i] = ch.steps;
                self.patterns[self.selected_pattern].channel_notes[ch_i] = ch.notes;
            }
        }
        self.patterns[self.selected_pattern].piano_roll_grid = self.piano_roll_grid;

        // Load new pattern
        self.selected_pattern = pat_idx;
        self.load_pattern_into_ui(pat_idx);
        self.status_message = crate::tstatus!("Aktivt mönster: {}", self.patterns[pat_idx].name);
    }
}
}

impl SonixApp {
pub fn sync_active_pattern_from_ui(&mut self) {
    if self.selected_pattern < self.patterns.len() {
        for (ch_i, ch) in self.channels.iter().enumerate() {
            if ch_i < self.patterns[self.selected_pattern].channel_steps.len() {
                self.patterns[self.selected_pattern].channel_steps[ch_i] = ch.steps;
                self.patterns[self.selected_pattern].channel_notes[ch_i] = ch.notes;
            }
        }
        self.patterns[self.selected_pattern].piano_roll_grid = self.piano_roll_grid;
    }
}
}

impl SonixApp {
pub fn select_sound_for_channel(&mut self, ch_idx: usize, item: &LibrarySampleItem) {
    if ch_idx >= self.channels.len() {
        return;
    }
    if !assign_library_sample_to_channel(&mut self.channels[ch_idx], item) {
        self.status_message = crate::tstatus!(
            "⚠ Kunde inte läsa '{}' — kanalen behåller sitt ljud (mp3 stöds inte, konvertera till wav)",
            item.name
        );
        return;
    }
    let item_name = item.name.clone();
    self.status_message = crate::tstatus!("Kanal {} ändrad till: {}", ch_idx + 1, item_name);
    self.audition_library_sample(item);
}
}

impl SonixApp {
/// Provspelar ett bibliotekssample.
///
/// **Tre fall, inte två.** Har samplen ingen fil alls spelar den inbyggda
/// synten sin röst för kategorin — det är appens eget ljud och ett riktigt
/// svar. Men hade samplen en fil som **inte gick att läsa** (mp3, trasig wav)
/// föll samma väg ned i syntrösten också: den som lyssnade hörde ett ljud och
/// trodde det var filen. Nu står det i statusraden i stället.
pub fn audition_library_sample(&mut self, item: &LibrarySampleItem) {
    if let Some(path) = item.file_path.as_deref() {
        match load_sample_pcm_arcs(path) {
            Some((l, r, sr)) => {
                let _ = self.engine.send_command(AudioCommand::PlayAudition {
                    left: l,
                    right: r,
                    sample_rate: sr as f32,
                    volume: 0.95,
                    pitch_ratio: 1.0,
                    time_stretch_ratio: 1.0,
                    is_reverse: false,
                    loop_playback: false,
                });
            }
            None => {
                self.status_message = crate::tstatus!(
                    "⚠ Kunde inte provspela '{}' — filen går inte att läsa: {} (mp3 stöds inte, konvertera till wav)",
                    item.name,
                    path
                );
            }
        }
        return;
    }

    match item.category.as_str() {
        "Kicks" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
        "Snares" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
        "Claps" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
        "Hi-Hats" => {
            if item.name.to_lowercase().contains("open") {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
            } else {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
            }
        }
        "Percussion" => {
            if item.name.to_lowercase().contains("high") {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomHigh));
            } else {
                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomLow));
            }
        }
        _ => {
            let freq = midi_to_freq(item.default_note);
            let _ = self.engine.send_command(AudioCommand::NoteOn { note: item.default_note, freq, velocity: 0.9 });
        }
    }
}
}

impl SonixApp {
/// Evaluates every enabled automation lane for all tracks at the current
/// song position and forwards changed values to the audio engine. Values
/// are cached per parameter so we only emit commands on real changes.
pub fn apply_automation(&mut self) {
    if !self.is_playing {
        return;
    }
    // **Automationen utvärderas i takter** (Fas 8.2): punkterna hör till en plats i
    // musiken, och spelhuvudets sekunder blir en takt först genom tempokartan. Det här är
    // ett av de ställen kompilatorn inte kan vakta — `value_at` tar en `f32` hur som
    // helst, och sekunder hade sett precis lika rimliga ut. Konverteringen står därför
    // ensam och uttryckligt, en gång, utanför loopen.
    let bar = self.tempo_map().bar_at_secs(self.song_time as f64) as f32;

    // **Bussarnas egna kurvor** (Fas 8.8). Passet ligger utanför spårloopen: bussen ägs inte
    // av något av spåren — flera kan skicka till samma. Ingen egen cache behövs heller:
    // bussens nuvarande värde *är* tillståndet, så jämförelsen sker mot det.
    for li in 0..self.bus_automation.len() {
        let (bus, level) = {
            let lane = &self.bus_automation[li];
            if !lane.enabled || lane.bus >= crate::audio::synth::NUM_BUSES {
                continue;
            }
            (lane.bus, lane.level_at(bar))
        };
        let Some(level) = level else { continue };
        // Samma område som fadern, så en kurva inte kan ställa bussen utanför sitt eget reglage.
        let level = level.clamp(0.0, 1.5);
        if (level - self.bus_volume[bus]).abs() > 1e-4 {
            self.bus_volume[bus] = level;
            let _ = self.engine.send_command(AudioCommand::SetBusState {
                bus,
                volume: level,
                muted: self.bus_muted[bus],
                solo: self.bus_solo[bus],
            });
        }
    }
    for ti in 0..self.playlist_tracks.len() {
        let mut target = [f32::NAN; AutomationParam::COUNT];
        {
            let track = &self.playlist_tracks[ti];
            for lane in &track.automation {
                if !lane.enabled {
                    continue;
                }
                if let Some(v) = lane.value_at(bar) {
                    let (lo, hi) = lane.param.range();
                    target[lane.param.index()] = v.clamp(lo, hi);
                }
            }
        }
        let mut state_cmd = None;
        let mut mix_cmd = None;
        let mut eq_cmd: Option<crate::audio::master_fx::TrackEqSettings> = None;
        {
            let track = &mut self.playlist_tracks[ti];
            let set = |cache: &mut f32, v: f32| -> bool {
                if v.is_finite() && (v - *cache).abs() > 1e-4 {
                    *cache = v;
                    true
                } else {
                    false
                }
            };
            let mut state_changed = false;
            let mut mix_changed = false;
            if set(&mut track.automation_last[0], target[0]) {
                track.volume = target[0];
                state_changed = true;
            }
            if set(&mut track.automation_last[1], target[1]) {
                track.pan = target[1];
                state_changed = true;
            }
            if set(&mut track.automation_last[2], target[2]) {
                track.reverb_send = target[2];
                mix_changed = true;
            }
            if set(&mut track.automation_last[3], target[3]) {
                track.delay_send = target[3];
                mix_changed = true;
            }
            // **8.8: tre mål till, och ingen motorändring behövdes** — `SetStemMixParams`
            // nedanför bär tröskeln, förhållandet och transponeringen redan. Det var bara
            // automationens väg som slutade vid de fyra första.
            if set(&mut track.automation_last[4], target[4]) {
                track.comp_threshold_db = target[4];
                mix_changed = true;
            }
            if set(&mut track.automation_last[5], target[5]) {
                track.comp_ratio = target[5];
                mix_changed = true;
            }
            if set(&mut track.automation_last[6], target[6]) {
                track.pitch_semitones = target[6];
                mix_changed = true;
            }
            // **EQ:n (8.8): banden ligger på spåret**, och `SetTrackEq` bär hela
            // inställningen — därför muteras spårets egen EQ och skickas tillbaka i sin
            // helhet. Att bygga en ny `TrackEqSettings` här hade tyst nollat frekvenser och
            // Q, alltså ändrat mer än kurvan rör.
            let mut eq_changed = false;
            if set(&mut track.automation_last[7], target[7]) {
                track.eq.low_gain_db = target[7];
                eq_changed = true;
            }
            if set(&mut track.automation_last[8], target[8]) {
                track.eq.mid_gain_db = target[8];
                eq_changed = true;
            }
            if set(&mut track.automation_last[9], target[9]) {
                track.eq.high_gain_db = target[9];
                eq_changed = true;
            }
            if eq_changed {
                eq_cmd = Some(track.eq.to_settings());
            }
            if state_changed {
                state_cmd = Some((track.volume, track.pan, track.muted, track.solo));
            }
            if mix_changed {
                mix_cmd = Some((
                    track.comp_threshold_db,
                    track.comp_ratio,
                    track.reverb_send,
                    track.delay_send,
                    track.pitch_semitones,
                ));
            }
        }
        if let Some(settings) = eq_cmd {
            let _ = self.engine.send_command(AudioCommand::SetTrackEq {
                track_index: ti,
                settings,
            });
        }
        if let Some((volume, pan, muted, solo)) = state_cmd {
            let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
                track_index: ti,
                volume,
                pan,
                muted,
                solo,
            });
        }
        if let Some((comp_threshold_db, comp_ratio, reverb_send, delay_send, pitch_semitones)) =
            mix_cmd
        {
            let _ = self.engine.send_command(AudioCommand::SetTrackMix {
                track_index: ti,
                comp_threshold_db,
                comp_ratio,
                reverb_send,
                delay_send,
                pitch_semitones,
            });
        }
    }
}
}

impl SonixApp {
pub(crate) fn render_automation_lane(
    &mut self,
    ui: &mut egui::Ui,
    rect: Rect,
    resp: &egui::Response,
    bar_w: f32,
    // **Tempokartan, inte ett enda tempo** (Fas 8.2 steg 3): lanens punkter ligger i
    // sekunder (om automation ska flytta med tempot är en öppen fråga), men *ritningen*
    // måste veta var i tiden en takt ligger — annars hamnar punkterna fel efter ett byte.
    track_idx: usize,
) {
    let param = self.automation_param;
    let (lo, hi) = param.range();
    let span = (hi - lo).max(1e-6);
    let val_to_y = |v: f32| rect.max.y - ((v - lo) / span).clamp(0.0, 1.0) * rect.height();
    let y_to_val = |y: f32| lo + ((rect.max.y - y) / rect.height()).clamp(0.0, 1.0) * span;
    // Punkterna ligger i **takter**, så ritningen behöver ingen omräkning: x är takten
    // gånger taktbredden. Före Fas 8.2 gick den här vägen via sekunder och kartan.
    let bar_to_x = |b: f32| rect.min.x + b * bar_w;

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(11, 14, 19));
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 10.0),
        egui::Align2::LEFT_CENTER,
        crate::tstatus!("📈 {} — {}", param.label(), self.playlist_tracks[track_idx].name),
        egui::FontId::proportional(10.5),
        Theme::FL_CYAN,
    );
    painter.line_segment(
        [
            Pos2::new(rect.min.x, val_to_y((lo + hi) * 0.5)),
            Pos2::new(rect.max.x, val_to_y((lo + hi) * 0.5)),
        ],
        Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 48)),
    );
    let total_bars = (rect.width() / bar_w).ceil() as usize;
    for b in 0..=total_bars {
        let x = rect.min.x + b as f32 * bar_w;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(0.5_f32, Color32::from_rgb(26, 30, 40)),
        );
    }

    let lane_idx = self.playlist_tracks[track_idx]
        .automation
        .iter()
        .position(|l| l.param == param);

    if let Some(li) = lane_idx {
        let lane = &self.playlist_tracks[track_idx].automation[li];
        if lane.enabled && !lane.points.is_empty() {
            let first = &lane.points[0];
            let last = lane.points.last().unwrap();
            painter.line_segment(
                [
                    Pos2::new(rect.min.x, val_to_y(first.value)),
                    Pos2::new(bar_to_x(first.time_bars), val_to_y(first.value)),
                ],
                Stroke::new(2.0_f32, Theme::FL_CYAN),
            );
            for w in lane.points.windows(2) {
                painter.line_segment(
                    [
                        Pos2::new(bar_to_x(w[0].time_bars), val_to_y(w[0].value)),
                        Pos2::new(bar_to_x(w[1].time_bars), val_to_y(w[1].value)),
                    ],
                    Stroke::new(2.0_f32, Theme::FL_CYAN),
                );
            }
            painter.line_segment(
                [
                    Pos2::new(bar_to_x(last.time_bars), val_to_y(last.value)),
                    Pos2::new(rect.max.x, val_to_y(last.value)),
                ],
                Stroke::new(2.0_f32, Theme::FL_CYAN),
            );
            for (pi, p) in lane.points.iter().enumerate() {
                let c = Pos2::new(bar_to_x(p.time_bars), val_to_y(p.value));
                let dragging = self.automation_drag == Some((track_idx, li, pi));
                painter.circle_filled(
                    c,
                    if dragging { 6.0 } else { 4.5 },
                    if dragging { Theme::FL_ORANGE } else { Theme::FL_CYAN },
                );
                painter.circle_stroke(c, 5.5, Stroke::new(1.0_f32, Color32::BLACK));
            }
        }
    }

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            // **Snäppet sker i takter** (Fas 8.2): ett sextondelssteg är en plats i
            // takten, inte ett antal sekunder, och sekunder-per-takt är inte ett tal över
            // ett tempobyte. Sekunden hämtas ur kartan **en gång efteråt**.
            let raw_bar = ((pos.x - rect.min.x) / bar_w).max(0.0);
            let bar = snap_bar(self.timeline_snap_mode, raw_bar);
            let val = y_to_val(pos.y).clamp(lo, hi);
            let li = match lane_idx {
                Some(li) => li,
                None => {
                    self.playlist_tracks[track_idx].automation.push(AutomationLane {
                        param,
                        enabled: true,
                        points: Vec::new(),
                    });
                    self.playlist_tracks[track_idx].automation.len() - 1
                }
            };
            let lane = &mut self.playlist_tracks[track_idx].automation[li];
            lane.points.push(AutomationPoint { time_bars: bar, value: val });
            lane.sort_points();
            self.status_message = crate::tstatus!("📈 Automation: la till punkt (takt {:.2}, {:.2})", bar, val);
        }
    }

    if resp.drag_started() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(li) = lane_idx {
                let pts = &self.playlist_tracks[track_idx].automation[li].points;
                let mut best = None;
                let mut best_d = 14.0_f32;
                for (pi, p) in pts.iter().enumerate() {
                    let c = Pos2::new(bar_to_x(p.time_bars), val_to_y(p.value));
                    let d = c.distance(pos);
                    if d < best_d {
                        best_d = d;
                        best = Some(pi);
                    }
                }
                if let Some(pi) = best {
                    self.automation_drag = Some((track_idx, li, pi));
                }
            }
        }
    }
    if resp.dragged() {
        if let Some((t, li, pi)) = self.automation_drag
            && t == track_idx
            && let Some(pos) = resp.interact_pointer_pos()
        {
            // **Snäppet sker i takter** (Fas 8.2): ett sextondelssteg är en plats i
            // takten, inte ett antal sekunder, och sekunder-per-takt är inte ett tal över
            // ett tempobyte. Sekunden hämtas ur kartan **en gång efteråt**.
            let raw_bar = ((pos.x - rect.min.x) / bar_w).max(0.0);
            let bar = snap_bar(self.timeline_snap_mode, raw_bar);
            let val = y_to_val(pos.y).clamp(lo, hi);
            if let Some(p) = self.playlist_tracks[track_idx]
                .automation
                .get_mut(li)
                .and_then(|l| l.points.get_mut(pi))
            {
                p.time_bars = bar;
                p.value = val;
            }
        }
    }
    if resp.drag_stopped() {
        if let Some((t, li, _)) = self.automation_drag {
            if t == track_idx
                && let Some(lane) = self.playlist_tracks[track_idx].automation.get_mut(li)
            {
                lane.sort_points();
            }
        }
        self.automation_drag = None;
    }

    if resp.secondary_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(li) = lane_idx {
                let pts = &self.playlist_tracks[track_idx].automation[li].points;
                let mut best = None;
                let mut best_d = 14.0_f32;
                for (pi, p) in pts.iter().enumerate() {
                    let c = Pos2::new(bar_to_x(p.time_bars), val_to_y(p.value));
                    let d = c.distance(pos);
                    if d < best_d {
                        best_d = d;
                        best = Some(pi);
                    }
                }
                if let Some(pi) = best {
                    self.playlist_tracks[track_idx].automation[li].points.remove(pi);
                    self.status_message = crate::tstatus!("📈 Automation: tog bort punkt");
                }
            }
        }
    }
}
}

impl SonixApp {
pub(crate) fn render_channel_rack(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        // Pattern Selector Bar (FL Studio Style)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX CHANNEL RACK")).strong().size(13.0).color(Theme::FL_ORANGE));
            ui.separator();

            ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(11.0).color(Theme::TEXT_MUTED));
            let p_len = self.patterns.len();
            for pat_idx in 0..p_len {
                let is_active = self.selected_pattern == pat_idx;
                let p_color = self.patterns[pat_idx].color;
                let fill = if is_active { p_color } else { Theme::PANEL_BG };
                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(11.0).color(text_color)).fill(fill)).clicked() {
                    self.select_pattern(pat_idx);
                }
            }

            ui.separator();

            // Swing Knob
            rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 22.0);

            ui.separator();

            if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Ljud")).color(Theme::FL_GREEN)).clicked() {
                self.show_import_modal = true;
            }

            if ui.button(crate::i18n::t("⚡ Slumpa Beats")).clicked() {
                for ch in &mut self.channels {
                    for s in 0..16 {
                        ch.steps[s] = (s % 4 == 0) || (rand_simple(s * 13 + ch.name.len()) > 0.68);
                    }
                }
                self.sync_active_pattern_from_ui();
            }
            if ui.button(crate::i18n::t("🗑 Rensa Steg")).clicked() {
                for ch in &mut self.channels {
                    ch.steps = [false; 16];
                }
                self.sync_active_pattern_from_ui();
            }
            if ui.button(crate::i18n::t("🎹 Öppna Piano Roll")).clicked() {
                self.view_mode = ViewMode::PianoRoll;
            }
            if ui.button(crate::i18n::t("🎼 Öppna Arranger")).clicked() {
                self.view_mode = ViewMode::PlaylistArranger;
            }
        });

        ui.add_space(8.0);

        let mut toggle_picker_for = None;
        let mut toggle_chopper_for = None;

        for ch_idx in 0..self.channels.len() {
            let is_selected = self.selected_channel == ch_idx;
            let is_picker_open = self.active_sample_picker_channel == Some(ch_idx);
            let is_chopper_open = self.active_chopper_channel == Some(ch_idx);
            let ch = &mut self.channels[ch_idx];

            ui.horizontal(|ui| {
                // Mute & Solo LEDs
                let mute_color = if ch.muted { Color32::from_rgb(50, 20, 20) } else { Theme::FL_GREEN };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).strong().size(10.0).color(Color32::WHITE)).fill(mute_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
                    ch.muted = !ch.muted;
                }
                let solo_color = if ch.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).strong().size(10.0).color(Color32::WHITE)).fill(solo_color).min_size(Vec2::new(18.0, 18.0))).clicked() {
                    ch.solo = !ch.solo;
                }

                // VOL & PAN Knobs
                rotary_knob(ui, &mut ch.volume, 0.0, 1.0, "VOL", ch.color, 24.0);
                rotary_knob(ui, &mut ch.pan, -1.0, 1.0, "PAN", Color32::from_rgb(180, 190, 200), 24.0);

                // Channel Select Button
                let btn_fill = if is_selected { Theme::PANEL_BG } else { Theme::CHANNEL_BG };
                let ch_btn = egui::Button::new(egui::RichText::new(format!("{} {}", ch.icon, ch.name)).strong().color(ch.color))
                    .fill(btn_fill)
                    .min_size(Vec2::new(110.0, 26.0));
                if ui.add(ch_btn).clicked() {
                    self.selected_channel = ch_idx;
                }

                // Quick Switch Sound Button [🔄 Byt]
                let byt_fill = if is_picker_open { Theme::FL_CYAN } else { Color32::from_rgb(32, 38, 48) };
                let byt_text_color = if is_picker_open { Color32::BLACK } else { Theme::FL_CYAN };
                let byt_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Byt")).size(10.0).color(byt_text_color))
                    .fill(byt_fill)
                    .min_size(Vec2::new(42.0, 24.0));
                if ui.add(byt_btn).on_hover_text(crate::i18n::t("Byt ut detta beat/ljud direkt från biblioteket")).clicked() {
                    toggle_picker_for = Some(ch_idx);
                }

                // Chop & Pitch Inspector Button [✂ Chop]
                let chop_fill = if is_chopper_open { Theme::FL_YELLOW } else { Color32::from_rgb(38, 36, 28) };
                let chop_text_color = if is_chopper_open { Color32::BLACK } else { Theme::FL_YELLOW };
                let chop_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Chop")).size(10.0).color(chop_text_color))
                    .fill(chop_fill)
                    .min_size(Vec2::new(52.0, 24.0));
                if ui.add(chop_btn).on_hover_text(crate::i18n::t("Öppna Waveform Chopper & Pitch Slicer")).clicked() {
                    toggle_chopper_for = Some(ch_idx);
                }

                ui.add_space(4.0);

                // 16 FL-Studio Style Tactile Step Buttons
                for step_idx in 0..16 {
                    let is_active = ch.steps[step_idx];
                    let is_playhead = self.is_playing && self.current_step == step_idx;
                    let beat_group_a = (step_idx / 4) % 2 == 0;

                    let resp = fl_step_button(
                        ui,
                        is_active,
                        is_playhead,
                        beat_group_a,
                        ch.color,
                        Vec2::new(26.0, 24.0),
                    );

                    if resp.clicked() {
                        ch.steps[step_idx] = !is_active;
                    }
                }
            });
            ui.add_space(3.0);
        }

        if let Some(ch_idx) = toggle_picker_for {
            self.active_sample_picker_channel = if self.active_sample_picker_channel == Some(ch_idx) { None } else { Some(ch_idx) };
        }
        if let Some(ch_idx) = toggle_chopper_for {
            self.active_chopper_channel = if self.active_chopper_channel == Some(ch_idx) { None } else { Some(ch_idx) };
        }

        // Interactive Sound Library Picker Overlay
        if let Some(picker_ch) = self.active_sample_picker_channel {
            ui.add_space(6.0);
            ui.group(|ui| {
                let cur_name = self.channels.get(picker_ch).map(|c| c.name.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::tstatus!("📚 BYT LJUD: Välj sample för Kanal {} ({})", picker_ch + 1, cur_name)).strong().size(12.5).color(Theme::FL_CYAN));
                    ui.separator();
                    if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Eget Ljud")).color(Theme::FL_GREEN)).clicked() {
                        self.show_import_modal = true;
                    }
                    if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Väljare")).color(Color32::from_rgb(255, 100, 100))).clicked() {
                        self.active_sample_picker_channel = None;
                    }
                });

                ui.add_space(4.0);

                // Category Filter Tabs
                let categories = ["Alla", "Kicks", "Snares", "Claps", "Hi-Hats", "Percussion", "Vocal Chops", "808 & Bass", "Egna Importerade"];
                ui.horizontal_wrapped(|ui| {
                    for (cat_i, &cat_name) in categories.iter().enumerate() {
                        let is_sel = self.selected_lib_category == cat_i;
                        let fill = if is_sel { Theme::FL_CYAN } else { Theme::PANEL_BG };
                        let text_col = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(cat_name).strong().size(11.0).color(text_col)).fill(fill)).clicked() {
                            self.selected_lib_category = cat_i;
                        }
                    }
                });

                ui.add_space(4.0);

                // Sample Items Grid
                let sel_cat = categories[self.selected_lib_category.min(categories.len() - 1)];
                let items_to_show: Vec<LibrarySampleItem> = self.sample_library.iter()
                    .filter(|item| sel_cat == "Alla" || item.category == sel_cat)
                    .cloned()
                    .collect();

                let mut chosen_item = None;
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    egui::Grid::new("sample_picker_grid").num_columns(4).spacing([8.0, 8.0]).show(ui, |ui| {
                        for (i, item) in items_to_show.iter().enumerate() {
                            ui.group(|ui| {
                                ui.set_width(190.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&item.icon).size(15.0));
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&item.name).strong().size(11.0).color(item.color));
                                        ui.label(egui::RichText::new(&item.category).size(9.0).color(Theme::TEXT_MUTED));
                                    });
                                });

                                 // Mini wave preview
                                let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(175.0, 18.0), egui::Sense::hover());
                                ui.painter().rect_filled(wf_rect, Rounding::same(2.0), Color32::from_rgb(16, 18, 22));
                                let mid_y = wf_rect.center().y;
                                let num_pts = item.waveform.len().max(1);
                                let dx = wf_rect.width() / num_pts as f32;
                                for (wi, &val) in item.waveform.iter().enumerate() {
                                    let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                    let h = (val.abs() * 7.5).max(1.0);
                                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.3_f32, item.color));
                                }

                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new(crate::i18n::t("▶ Prov")).size(9.5)).on_hover_text(crate::i18n::t("Provspela med riktigt ljud")).clicked() {
                                        self.audition_library_sample(item);
                                    }
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✅ Välj")).strong().size(9.5).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                                        chosen_item = Some(item.clone());
                                    }
                                });
                            });

                            if (i + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });

                if let Some(item) = chosen_item {
                    self.select_sound_for_channel(picker_ch, &item);
                    self.active_sample_picker_channel = None;
                }
            });
        }

        // Interactive Sample Chopper & Pitch Transpose Inspector
        if let Some(chop_ch) = self.active_chopper_channel
            && chop_ch < self.channels.len() {
                ui.add_space(6.0);
                ui.group(|ui| {
                    let ch = &mut self.channels[chop_ch];
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("✂ SAMPLE CHOPPER & PITCH INSPECTOR - Kanal {} ({} {})", chop_ch + 1, ch.icon, ch.name)).strong().size(12.5).color(Theme::FL_YELLOW));
                        ui.separator();
                        if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela Chop")).color(Theme::FL_GREEN)).clicked() {
                            let note = (ch.notes[0] as i16 + ch.pitch_semitones as i16).clamp(0, 127) as u8;
                            let base_freq = midi_to_freq(note);
                            let fine_mul = (ch.pitch_fine_cents / 1200.0).exp2();
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: base_freq * fine_mul, velocity: ch.volume });
                        }
                        if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Chopper")).color(Color32::from_rgb(255, 100, 100))).clicked() {
                            self.active_chopper_channel = None;
                        }
                    });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        // Waveform Chopper Slicer
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Waveform Chop & Slice Trim:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                            let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(380.0, 65.0), egui::Sense::click_and_drag());
                            ui.painter().rect_filled(wf_rect, Rounding::same(3.0), Color32::from_rgb(14, 16, 20));
                            ui.painter().rect_stroke(wf_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 65)));

                            let mid_y = wf_rect.center().y;
                            let num_pts = ch.waveform_preview.len().max(1);
                            let dx = wf_rect.width() / num_pts as f32;
                            for (wi, &val) in ch.waveform_preview.iter().enumerate() {
                                let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                let h = (val.abs() * 26.0).max(1.0);
                                ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.8_f32, ch.color));
                            }

                            let start_x = wf_rect.min.x + ch.sample_start * wf_rect.width();
                            let end_x = wf_rect.min.x + ch.sample_end * wf_rect.width();

                            if ch.sample_start > 0.0 {
                                let dim_rect = Rect::from_min_max(wf_rect.min, Pos2::new(start_x, wf_rect.max.y));
                                ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                            }
                            if ch.sample_end < 1.0 {
                                let dim_rect = Rect::from_min_max(Pos2::new(end_x, wf_rect.min.y), wf_rect.max);
                                ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                            }

                            // Slicekartan (Fas 8.7): varje hittad gräns som ett tunt
                            // streck. Den aktiva slicens ram ritas ovanpå.
                            for (si, (s0, _s1)) in ch.slices.iter().enumerate() {
                                if si == 0 {
                                    continue;
                                }
                                let x = wf_rect.min.x + s0 * wf_rect.width();
                                ui.painter().line_segment(
                                    [Pos2::new(x, wf_rect.min.y), Pos2::new(x, wf_rect.max.y)],
                                    Stroke::new(1.0_f32, Theme::FL_CYAN),
                                );
                            }

                            let active_slice_rect = Rect::from_min_max(Pos2::new(start_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y));
                            ui.painter().rect_stroke(active_slice_rect, Rounding::ZERO, Stroke::new(1.5_f32, Theme::FL_YELLOW));
                            ui.painter().line_segment([Pos2::new(start_x, wf_rect.min.y), Pos2::new(start_x, wf_rect.max.y)], Stroke::new(2.5_f32, Theme::FL_GREEN));
                            ui.painter().line_segment([Pos2::new(end_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y)], Stroke::new(2.5_f32, Color32::from_rgb(255, 70, 70)));

                            // **Nudge: dra en gräns** (Fas 8.7 steg 2). Gränsen ligger
                            // mellan två slicar, så `nudge_slice_boundary` flyttar **båda** —
                            // flyttas bara den ena uppstår ett glapp (tyst i slicen) eller
                            // ett överlapp (samma ljud två gånger). Filens kanter (den
                            // första och den sista) ritas men går inte att dra: en karta som
                            // inte täcker hela filen spelar inte hela filen längre.
                            //
                            // Minsta slice i kartans enhet (andel av filen): 0,5 %. Den
                            // klämningen finns för att en gräns inte ska kunna korsa sin
                            // granne — detektionens 30 ms är en annan regel (när två slag
                            // slås ihop) och blandas inte in här.
                            const MIN_SLICE: f32 = 0.005;
                            for bi in 1..ch.slices.len() {
                                let bx = wf_rect.min.x + ch.slices[bi].0 * wf_rect.width();
                                let zone = Rect::from_min_max(
                                    Pos2::new(bx - 3.0, wf_rect.min.y),
                                    Pos2::new(bx + 3.0, wf_rect.max.y),
                                );
                                let resp = ui.interact(
                                    zone,
                                    egui::Id::new(("slice-nudge", bi)),
                                    egui::Sense::drag(),
                                );
                                if resp.dragged() {
                                    let delta = resp.drag_delta().x / wf_rect.width();
                                    if delta != 0.0 {
                                        ch.slices = crate::audio::onset::nudge_slice_boundary(
                                            &ch.slices, bi, delta, MIN_SLICE,
                                        );
                                    }
                                }
                                if resp.hovered() || resp.dragged() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                }
                            }

                            // Quick Slicer Presets
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Chop Snabbval:")).size(10.0).color(Theme::TEXT_MUTED));
                                if ui.button(crate::i18n::t("Fullt")).clicked() { ch.sample_start = 0.0; ch.sample_end = 1.0; }
                                if ui.button(crate::i18n::t("1/2")).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.5; }
                                if ui.button(crate::i18n::t("2/2")).clicked() { ch.sample_start = 0.5; ch.sample_end = 1.0; }
                                if ui.button(crate::i18n::t("1/4")).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.25; }
                                if ui.button(crate::i18n::t("2/4")).clicked() { ch.sample_start = 0.25; ch.sample_end = 0.5; }
                                if ui.button(crate::i18n::t("3/4")).clicked() { ch.sample_start = 0.50; ch.sample_end = 0.75; }
                                if ui.button(crate::i18n::t("4/4")).clicked() { ch.sample_start = 0.75; ch.sample_end = 1.0; }
                            });

                            // Slagletning (Fas 8.7). Knappen som förut hette
                            // "⚡ Transient" satte bara slutet till 18 % av filen —
                            // ett påhittat svar på en riktig fråga. Nu letas slagen
                            // i filen, och resultatet är en slicekarta.
                            ui.horizontal(|ui| {
                                let mut detected: Option<(Vec<(f32, f32)>, usize)> = None;
                                let mut message: Option<String> = None;
                                if ui
                                    .button(egui::RichText::new(crate::i18n::t("🔍 Hitta slag")).color(Theme::FL_CYAN))
                                    .on_hover_text(crate::i18n::t("Leta transients i filen och dela den i slicar"))
                                    .clicked()
                                {
                                    match ch.pcm_audio.as_ref() {
                                        // 8.5-regeln: går ljudet inte att läsa säger vi det.
                                        None => {
                                            message = Some(crate::i18n::t("⚠ Kanalens ljud går inte att läsa — inga slag kan hittas").to_string());
                                        }
                                        Some((left, _right, sr)) => {
                                            // **En regel, två detektorer** (Fas 8.7 steg 2):
                                            // kroppen bor i `detect_slice_map`, så båda
                                            // knapparna går genom samma väg — valet byter
                                            // detektor och inget annat. Att skriva av
                                            // blocket en gång per detektor hade varit två
                                            // vägar till samma sak.
                                            match crate::audio::onset::detect_slice_map(
                                                left,
                                                *sr as f32,
                                                false, // höljesdetektorn: rätt för trummor
                                                &crate::audio::onset::OnsetParams::default(),
                                            ) {
                                                Some((map, count)) => detected = Some((map, count)),
                                                None => message = Some(crate::i18n::t("Inga slag hittades i filen").to_string()),
                                            }
                                        }
                                    }
                                }
                                // **Andra dörren in i samma regel**: spektral flux. Kroppen
                                // är densamma, bara detektorn skiljer — alltså inte en andra
                                // väg, bara ett andra val.
                                if ui
                                    .button(crate::i18n::t("🔍 Slagletning (spektral)"))
                                    .on_hover_text(crate::i18n::t(
                                        "Samma sak, men letar klangförändring i stället för nivå — starkare på melodiöst material och sång, onödigt för rena trumloopar.",
                                    ))
                                    .clicked()
                                {
                                    match ch.pcm_audio.as_ref() {
                                        None => {
                                            message = Some(crate::i18n::t("⚠ Kanalens ljud går inte att läsa — inga slag").to_string())
                                        }
                                        Some((left, _right, sr)) => {
                                            match crate::audio::onset::detect_slice_map(
                                                left,
                                                *sr as f32,
                                                true, // spektral flux
                                                &crate::audio::onset::OnsetParams::default(),
                                            ) {
                                                Some((map, count)) => detected = Some((map, count)),
                                                None => message = Some(crate::i18n::t("Inga slag hittades i filen").to_string()),
                                            }
                                        }
                                    }
                                }
                                if let Some(msg) = message {
                                    self.status_message = msg;
                                }
                                if let Some((slices, onsets)) = detected {
                                    ch.sample_start = slices[0].0;
                                    ch.sample_end = slices[0].1;
                                    ch.slices = slices;
                                    self.status_message = crate::tstatus!(
                                        "🔍 {} slag hittade — {} slicar (den första spelas nu)",
                                        onsets,
                                        ch.slices.len()
                                    );
                                }
                                // **Dumpa slicarna till stegraden** (Fas 8.7 steg 2). En not
                                // per slice, kromatiskt från basnoten — samma adressering som
                                // `window_for_note` redan spelar efter, alltså behövs ingen
                                // motorändring: noten *är* adressen. Fler slicar än steg
                                // kapas, och det sägs rakt ut i stället för att tigas.
                                // **Dumpa slicarna till piano rollen** (Fas 8.7 steg 2).
                                // Samma sak mot ett annat rutnät: `piano_roll_grid` är
                                // `[[bool; 16]; 24]`, alltså samma form som stegraden med 24
                                // rader i stället för 16 — så samma `slices_to_steps` duger,
                                // med radantalet som tak. Raden en slice hamnar på är
                                // **slicens index**, precis som noten är `bas + index`; därför
                                // slås inga slicar ihop, och adresseringen stämmer fortfarande
                                // med `window_for_note`. (Att folda noten med
                                // `note_to_roll_offset` vore fel här: den är gjord för *spelade*
                                // noter, och en foldning hade lagt två slicar på samma rad.)
                                if ui
                                    .button(crate::i18n::t("⬇ Slicar → piano roll"))
                                    .on_hover_text(crate::i18n::t(
                                        "Lägger en not per slice i piano rollen, kromatiskt från basnoten. Fler slicar än rutnätet rymmer kapas.",
                                    ))
                                    .clicked()
                                {
                                    // **Rutnätet har två axlar, och de betyder olika saker:**
                                    // raderna är tonhöjd och stegen är tid. Slice `i` hamnar
                                    // därför på **steg i** i **rad i** — tid och tonhöjd är båda
                                    // slicens nummer — och taket är det *mindre* av de två
                                    // antalen, inte radantalet. (Första försöket tog radantalet
                                    // och lappade tiden med `% 16`; det hade lagt slice 17 på
                                    // samma steg som slice 1.)
                                    let step_count = self
                                        .patterns
                                        .get(self.selected_pattern)
                                        .map_or(0, |p| p.piano_roll_grid.first().map_or(0, |row| row.len()));
                                    let row_count = self
                                        .patterns
                                        .get(self.selected_pattern)
                                        .map_or(0, |p| p.piano_roll_grid.len());
                                    let capacity = crate::audio::onset::grid_capacity(row_count, step_count);
                                    let dumped = crate::audio::onset::slices_to_steps(
                                        &ch.slices,
                                        ch.sample_base_note,
                                        capacity,
                                    );
                                    if dumped.is_empty() {
                                        self.status_message = crate::i18n::t(
                                            "⚠ Inga slicar att dumpa — kör en slagletning först.",
                                        )
                                        .to_string();
                                    } else if let Some(pattern) =
                                        self.patterns.get_mut(self.selected_pattern)
                                    {
                                        let skipped = ch.slices.len().saturating_sub(dumped.len());
                                        // Rutnätet töms för de rader dumpen rör, så en
                                        // gammal dump inte ligger kvar och pekar på slicar
                                        // som inte längre finns.
                                        for row in pattern.piano_roll_grid.iter_mut() {
                                            row.fill(false);
                                        }
                                        // Steg och rad är båda slicens nummer: slicarna hamnar
                                        // i ordning i tiden, och adresseringen (`bas + i`
                                        // spelar slice `i`) stämmer fortfarande.
                                        for (i, _note) in &dumped {
                                            pattern.piano_roll_grid[*i][*i] = true;
                                        }
                                        self.status_message = if skipped > 0 {
                                            crate::tstatus!(
                                                "⬇ {} slicar till piano rollen ({} kapades — rutnätet rymmer {}).",
                                                dumped.len(),
                                                skipped,
                                                capacity
                                            )
                                        } else {
                                            crate::tstatus!("⬇ {} slicar till piano rollen.", dumped.len())
                                        };
                                    }
                                }
                                if ui
                                    .button(crate::i18n::t("⬇ Slicar → steg"))
                                    .on_hover_text(crate::i18n::t(
                                        "Lägger en not per slice på stegraden, kromatiskt från basnoten. Fler slicar än steg kapas.",
                                    ))
                                    .clicked()
                                {
                                    let dumped = crate::audio::onset::slices_to_steps(
                                        &ch.slices,
                                        ch.sample_base_note,
                                        ch.steps.len(),
                                    );
                                    if dumped.is_empty() {
                                        self.status_message = crate::i18n::t(
                                            "⚠ Inga slicar att dumpa — kör en slagletning först.",
                                        )
                                        .to_string();
                                    } else {
                                        let skipped = ch.slices.len().saturating_sub(dumped.len());
                                        // Raden byggs om från grunden: en dump beskriver
                                        // **hela** kartan, så gamla steg kan inte ligga kvar
                                        // och peka på slicar som inte längre är med.
                                        // `fill` i stället för ett magiskt 16: radens
                                        // längd står i typen, och då kan talet inte driva isär.
                                        ch.steps.fill(false);
                                        for (step, note) in &dumped {
                                            ch.steps[*step] = true;
                                            ch.notes[*step] = *note;
                                        }
                                        self.status_message = if skipped > 0 {
                                            crate::tstatus!(
                                                "⬇ {} slicar till stegraden ({} kapades — raden har {} steg).",
                                                dumped.len(),
                                                skipped,
                                                ch.steps.len()
                                            )
                                        } else {
                                            crate::tstatus!("⬇ {} slicar till stegraden.", dumped.len())
                                        };
                                    }
                                }
                                if !ch.slices.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!("{} {}", ch.slices.len(), crate::i18n::t("slicar")))
                                            .size(10.0)
                                            .color(Theme::TEXT_MUTED),
                                    );
                                }
                            });

                            // Slicarna som knappar: att välja en sätter trimfönstret,
                            // och trimfönstret är vad provspelningen spelar.
                            if !ch.slices.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t("Slicar (klicka för att höra):"))
                                            .size(10.0)
                                            .color(Theme::TEXT_MUTED),
                                    );
                                    let active = (0..ch.slices.len()).find(|&i| {
                                        (ch.sample_start - ch.slices[i].0).abs() < 1e-4
                                            && (ch.sample_end - ch.slices[i].1).abs() < 1e-4
                                    });
                                    for i in 0..ch.slices.len() {
                                        if ui.selectable_label(active == Some(i), format!("{}", i + 1)).clicked() {
                                            ch.sample_start = ch.slices[i].0;
                                            ch.sample_end = ch.slices[i].1;
                                        }
                                    }
                                    if ui.button(crate::i18n::t("Rensa slicar")).clicked() {
                                        ch.slices.clear();
                                    }
                                });
                            }
                        });

                        ui.separator();

                        // Pitch & Envelope Controls
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Pitch & Tonhöjd:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Semitoner:"));
                                ui.add(egui::Slider::new(&mut ch.pitch_semitones, -24..=24).text("st"));
                                if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_semitones = 0; }
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Fine Cents:"));
                                ui.add(egui::Slider::new(&mut ch.pitch_fine_cents, -100.0..=100.0).text("ct"));
                                if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_fine_cents = 0.0; }
                            });

                            let total_cents = ch.pitch_semitones as f32 * 100.0 + ch.pitch_fine_cents;
                            let semitone_text = if ch.pitch_semitones >= 0 { format!("+{}", ch.pitch_semitones) } else { format!("{}", ch.pitch_semitones) };
                            ui.label(egui::RichText::new(format!("Totalt: {} st ({:+.0} ct)", semitone_text, total_cents)).color(Theme::FL_CYAN).size(10.0));

                            ui.separator();

                            ui.label(egui::RichText::new(crate::i18n::t("Klipp & Sampler:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Start Trim:"));
                                ui.add(egui::Slider::new(&mut ch.sample_start, 0.0..=1.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("End Trim:"));
                                ui.add(egui::Slider::new(&mut ch.sample_end, 0.0..=1.0));
                            });
                            ui.checkbox(&mut ch.is_reverse, "🔄 Reverse Waveform");

                            // **Samplern** (Fas 8.4). Före den här panelen fanns en
                            // "Attack / Decay"-ratt som *ingen* DSP läste — fältet fanns,
                            // reglaget syntes, och ingenting hördes. Den är ersatt av en
                            // riktig envelop, och det gamla värdet flyttas in i attacken
                            // när ett projekt från förr öppnas (se `from_saved`).
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Loop:")).strong().size(11.0).color(Theme::TEXT_MUTED));
                            ui.vertical(|ui| {
                                for (mode, label) in [
                                    (crate::audio::LoopMode::Off, crate::i18n::t("Av (en skott)")),
                                    (crate::audio::LoopMode::UntilRelease, crate::i18n::t("Till not-av")),
                                    (crate::audio::LoopMode::Forever, crate::i18n::t("För evigt")),
                                ] {
                                    if ui
                                        .radio_value(&mut ch.loop_mode, mode, label)
                                        .on_hover_text(crate::i18n::t("Av = spela en gång. Till not-av = loopen går medan noten hålls och resten efter loopen spelas vid not-av. För evigt = loopen fortsätter även efter not-avet."))
                                        .changed()
                                    {
                                        // En ny loop utan punkter vore en loop ingen hör:
                                        // sätt ett hörbart spann (sista fjärdedelen) första
                                        // gången ett läge slås på.
                                        if mode != crate::audio::LoopMode::Off
                                            && ch.sample_loop_end <= ch.sample_loop_start + 0.01
                                        {
                                            ch.sample_loop_start = 0.75;
                                            ch.sample_loop_end = 1.0;
                                        }
                                    }
                                }
                                let looping = ch.loop_mode != crate::audio::LoopMode::Off;
                                ui.add_enabled_ui(looping, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(crate::i18n::t("Loopstart:"));
                                        ui.add(egui::Slider::new(&mut ch.sample_loop_start, 0.0..=1.0));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(crate::i18n::t("Loopslut:"));
                                        ui.add(egui::Slider::new(&mut ch.sample_loop_end, 0.0..=1.0));
                                    });
                                    // Ett bakvänt par är ingen loop — säg det i stället för
                                    // att tiga, för motorn spelar då som en en-skottsprovare.
                                    if looping && ch.sample_loop_end <= ch.sample_loop_start {
                                        ui.label(
                                            egui::RichText::new(crate::i18n::t("⚠ Loopslut måste ligga efter loopstart — nu spelas ljudet som en skott."))
                                                .color(Theme::FL_YELLOW)
                                                .size(10.0),
                                        );
                                    }
                                    ui.checkbox(&mut ch.ping_pong, crate::i18n::t("↔ Ping-pong (vänd i ändarna)"));
                                });
                            });

                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Envelope (ADSR):")).strong().size(11.0).color(Theme::TEXT_MUTED));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Attack:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.attack, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Decay:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.decay, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Sustain:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.sustain, 0.0..=1.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Release:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.release, 0.0..=4.0).suffix(" s").fixed_decimals(3));
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .button(crate::i18n::t("Ingen envelop"))
                                    .on_hover_text(crate::i18n::t("Identiteten: full nivå från första samplet, inget släpp. Det är så en kanal utan envelop låter — och så ett projekt från före samplern låter."))
                                    .clicked()
                                {
                                    ch.amp_env = crate::audio::envelope::AdsrParams::identity();
                                }
                                if ch.amp_env.is_identity() {
                                    ui.label(egui::RichText::new(crate::i18n::t("(ingen envelop vald)")).color(Theme::TEXT_MUTED).size(10.0));
                                }
                            });
                        });
                    });
                });
            }

        self.sync_active_pattern_from_ui();
    });
}
}

impl SonixApp {
pub fn apply_alchemy_morph(&mut self) {
    let px = self.alchemy_puck[0];
    let py = self.alchemy_puck[1];
    use std::f32::consts::PI;

    let snapshots = [
        (-0.5 * PI, 0.005, 0.35, 4200.0, 1.8, Waveform::Saw),
        (-0.25 * PI, 0.35, 0.6, 2800.0, 1.2, Waveform::Saw),
        (0.0 * PI, 0.005, 0.15, 1200.0, 4.5, Waveform::Saw),
        (0.25 * PI, 0.01, 0.25, 6000.0, 2.0, Waveform::Square),
        (0.5 * PI, 0.5, 0.8, 1500.0, 0.8, Waveform::Triangle),
        (0.75 * PI, 0.08, 0.4, 3500.0, 2.5, Waveform::Triangle),
        (1.0 * PI, 0.002, 0.6, 8000.0, 3.0, Waveform::Sine),
        (1.25 * PI, 0.001, 0.08, 12000.0, 0.7, Waveform::Square),
    ];

    let mut total_weight = 0.0;
    let mut morph_attack = 0.0;
    let mut morph_decay = 0.0;
    let mut morph_cutoff = 0.0;
    let mut morph_res = 0.0;
    let mut closest_wf = Waveform::Saw;
    let mut min_dist = f32::MAX;

    for (angle, att, dec, cut, res, wf) in snapshots {
        let sx = angle.cos();
        let sy = angle.sin();
        let dist = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt().max(0.05);
        let w = 1.0 / (dist.powf(2.5));
        total_weight += w;
        morph_attack += att * w;
        morph_decay += dec * w;
        morph_cutoff += cut * w;
        morph_res += res * w;

        if dist < min_dist {
            min_dist = dist;
            closest_wf = wf;
        }
    }

    if total_weight > 0.0001 {
        self.adsr.attack = morph_attack / total_weight;
        self.adsr.decay = morph_decay / total_weight;
        self.filter.cutoff = (morph_cutoff / total_weight).clamp(200.0, 18000.0);
        self.filter.resonance = (morph_res / total_weight).clamp(0.5, 9.0);
        self.waveform = closest_wf;

        let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
        let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
        let _ = self.engine.send_command(AudioCommand::SetWaveform(self.waveform));
    }
}
}

impl SonixApp {
pub(crate) fn render_alchemy_morph_pad(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("✨ SONIX ALCHEMY SYNTHESIZER (Transform Pad)")).strong().size(14.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("8-punkters realtids-morphing mellan subtraktiva, FM- och wavetable-synteser")).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // Center Morphing Pad
            ui.group(|ui| {
                ui.set_width(380.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("TRANSFORM PAD")).strong().size(12.0).color(Theme::FL_CYAN));
                    ui.label(egui::RichText::new(crate::i18n::t("Dra den lysande markören för att sömlöst smälta samman filter, vågformer och ADSR")).size(10.0).color(Theme::TEXT_MUTED));
                    ui.add_space(6.0);

                    if alchemy_transform_matrix(ui, &mut self.alchemy_puck, Vec2::new(360.0, 300.0)) {
                        self.apply_alchemy_morph();
                    }
                });
            });

            ui.separator();

            // Live Morphed Parameters
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("AKTIVA SYNTPARAMETRAR (MORPHED)")).strong().size(11.0).color(Theme::FL_ORANGE));
                    ui.add_space(6.0);

                    ui.label(crate::tstatus!("Vågform: {}", self.waveform.name()));
                    ui.label(format!("Filter Cutoff: {:.0} Hz", self.filter.cutoff));
                    ui.label(crate::tstatus!("Resonans (Q): {:.2}", self.filter.resonance));
                    ui.label(format!("Attack: {:.3}s  •  Decay: {:.2}s", self.adsr.attack, self.adsr.decay));

                    ui.add_space(8.0);
                    ui.separator();

                    ui.label(egui::RichText::new(crate::i18n::t("SNABBVAL SNAPSHOTS")).strong().size(11.0).color(Theme::FL_CYAN));
                    let snaps = [
                        ("1. Pluck", 0.0_f32, -1.0_f32),
                        ("2. Lush String", 0.707, -0.707),
                        ("3. 303 Acid", 1.0, 0.0),
                        ("4. Synthwave", 0.707, 0.707),
                        ("5. Warm Pad", 0.0, 1.0),
                        ("6. Cosmic Brass", -0.707, 0.707),
                        ("7. Digital Bell", -1.0, 0.0),
                        ("8. Chiptune", -0.707, -0.707),
                    ];

                    ui.columns(2, |cols| {
                        for (i, (s_name, px, py)) in snaps.iter().enumerate() {
                            let col_i = i % 2;
                            if cols[col_i].button(*s_name).clicked() {
                                self.alchemy_puck = [*px, *py];
                                self.apply_alchemy_morph();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    if ui.button(crate::i18n::t("🎹 Provspela Ton (C4)")).clicked() {
                        self.play_note(60);
                    }
                });
            });
        });
    });
}
}

impl SonixApp {
pub(crate) fn render_remix_fx_view(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 REMIX FX & GROSS BEAT (DJ Performance Touch)")).strong().size(14.0).color(Theme::FL_GREEN));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("DJ-effekter, Tape Stop, Filter Sweeps och Stutter Glitch")).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // DJ XY Filter Touch Surface
            ui.group(|ui| {
                ui.set_width(340.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("DJ FILTER & CRUSH XY PAD")).strong().size(11.0).color(Theme::FL_GREEN));
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(320.0, 240.0), Sense::click_and_drag());
                    let painter = ui.painter();

                    if (resp.dragged() || resp.clicked())
                        && let Some(pos) = resp.interact_pointer_pos() {
                            self.remix_xy[0] = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                            self.remix_xy[1] = (1.0 - (pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);

                            self.filter.cutoff = 200.0 + self.remix_xy[0] * 12000.0;
                            self.filter.resonance = 0.5 + self.remix_xy[1] * 7.5;
                            let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                        }

                    painter.rect_filled(rect, Rounding::same(6.0), Color32::from_rgb(14, 22, 18));
                    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, Theme::FL_GREEN));

                    let c = rect.center();
                    painter.line_segment([Pos2::new(rect.min.x, c.y), Pos2::new(rect.max.x, c.y)], Stroke::new(1.0_f32, Color32::from_rgb(25, 45, 30)));
                    painter.line_segment([Pos2::new(c.x, rect.min.y), Pos2::new(c.x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(25, 45, 30)));

                    let puck_x = rect.min.x + self.remix_xy[0] * rect.width();
                    let puck_y = rect.max.y - self.remix_xy[1] * rect.height();
                    painter.circle_filled(Pos2::new(puck_x, puck_y), 12.0, Color32::from_rgba_unmultiplied(46, 204, 113, 100));
                    painter.circle_filled(Pos2::new(puck_x, puck_y), 6.0, Theme::FL_GREEN);

                    ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz  •  Res: {:.1}", self.filter.cutoff, self.filter.resonance)).size(10.0).color(Theme::FL_GREEN));
                });
            });

            ui.separator();

            // DJ Performance Buttons
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("DJ PERFORMANCE PADS")).strong().size(11.0).color(Theme::FL_ORANGE));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        // Tape Stop
                        let stop_col = if self.remix_tape_stop { Theme::FL_ORANGE } else { Color32::from_rgb(50, 40, 30) };
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏸ TAPE STOP")).strong().size(12.0).color(Color32::WHITE)).fill(stop_col).min_size(Vec2::new(100.0, 40.0))).clicked() {
                            self.remix_tape_stop = !self.remix_tape_stop;
                            let _ = self.engine.send_command(AudioCommand::SetTapeStop { active: self.remix_tape_stop });
                        }

                        // Vinyl Scratch
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📻 VINYL SCRATCH")).strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(30, 45, 60)).min_size(Vec2::new(120.0, 40.0))).clicked() {
                            self.drive = 4.5;
                            let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(crate::i18n::t("GROSS BEAT / STUTTER GLITCH")).strong().size(11.0).color(Theme::FL_CYAN));
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.remix_stutter == 1, "⚡ 1/4 Repeat").clicked() {
                            self.remix_stutter = if self.remix_stutter == 1 { 0 } else { 1 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/4 Beat Repeat aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 2, "⚡ 1/8 Stutter").clicked() {
                            self.remix_stutter = if self.remix_stutter == 2 { 0 } else { 2 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/8 Stutter aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 3, "⚡ 1/16 Roll").clicked() {
                            self.remix_stutter = if self.remix_stutter == 3 { 0 } else { 3 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/16 High-Speed Roll aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 4, "🌀 Reverse").clicked() {
                            self.remix_stutter = if self.remix_stutter == 4 { 0 } else { 4 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("🌀 Gross Beat Reverse aktiv").to_string();
                        }
                    });

                    ui.add_space(10.0);
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Styr DJ-effekter under uppspelning för live-remixing av dina mönster!")).size(10.0).color(Theme::TEXT_MUTED));
                    });
                });
            });
        });
    });
}
}

impl SonixApp {
pub(crate) fn render_effects_mixer_rack(&mut self, ui: &mut egui::Ui) {
    // Ensure selected timeline track is within bounds
    if !self.playlist_tracks.is_empty() && self.selected_timeline_track >= self.playlist_tracks.len() {
        self.selected_timeline_track = 0;
    }

    ui.group(|ui| {
        // Header Bar (Responsive Wrapped)
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX PRO MULTI-TRACK MIXER & DEDIKERAD EQ-STUDIO")).strong().size(13.5).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new(crate::tstatus!("Låt: '{}' • {} låtspår med 3-bands parametrisk EQ, dynamik & effekter", self.project_name, self.playlist_tracks.len())).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(6.0);

        // ================================================================
        // 1. TIER 1: MULTI-TRACK MIXER CHANNEL STRIPS (MASTER + ACTUAL SONG TRACKS)
        // ================================================================
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

                // Master Channel Strip
                ui.group(|ui| {
                    ui.set_width(78.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("MASTER")).strong().size(11.0).color(Theme::FL_ORANGE));
                        ui.add_space(2.0);
                        rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", Color32::WHITE, 16.0);
                        rotary_knob(ui, &mut self.stereo_width, 0.0, 2.0, "BREDD", Theme::FL_CYAN, 16.0);
                        ui.add_space(2.0);
                        let peak = self.engine.get_peak_level();
                        if vertical_fader(ui, &mut self.master_volume, 0.0, 1.25, peak, Theme::FL_ORANGE, 110.0) {
                            let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                        }
                        let gain_db = if self.master_volume <= 0.001 { -60.0 } else { 20.0 * self.master_volume.log10() };
                        ui.label(egui::RichText::new(format!("{:.0}%\n{:+.1}dB", self.master_volume * 100.0, gain_db)).strong().size(8.5).color(Theme::FL_ORANGE));
                    });
                });

                ui.separator();

                // Channel Strips for each Playlist Track in the user's project
                let num_tracks = self.playlist_tracks.len();
                for t_idx in 0..num_tracks {
                    let is_selected = self.selected_timeline_track == t_idx;
                    let t = &self.playlist_tracks[t_idx];
                    let track_name = t.name.clone();
                    let track_color = t.color;
                    let track_icon = t.icon;
                    let is_mic = (t_idx == num_tracks - 1 && (track_name.to_lowercase().contains("mic") || track_name.to_lowercase().contains("mik") || t.kind == TrackKind::VocalAudio))
                        || track_name.to_lowercase().contains("mic (")
                        || track_name.to_lowercase().contains("mikrofon");

                    let border_color = if is_selected {
                        if is_mic { Color32::from_rgb(0, 200, 255) } else { track_color }
                    } else {
                        Color32::from_rgb(32, 40, 52)
                    };

                    let card_bg = if is_selected {
                        Color32::from_rgb(26, 32, 44)
                    } else if is_mic {
                        Color32::from_rgb(20, 25, 35)
                    } else {
                        Theme::CHANNEL_BG
                    };

                    ui.group(|ui| {
                        ui.set_width(82.0);
                        ui.painter().rect_stroke(ui.max_rect(), Rounding::same(3.0), Stroke::new(if is_selected { 1.5_f32 } else { 1.0_f32 }, border_color));

                        ui.vertical_centered(|ui| {
                            // Track Title / Select Button
                            let btn_label = if is_mic {
                                format!("{} Mic", track_icon)
                            } else {
                                let short_n = if track_name.chars().count() > 9 {
                                    format!("{}…", track_name.chars().take(8).collect::<String>())
                                } else {
                                    track_name.clone()
                                };
                                format!("{} {}", track_icon, short_n)
                            };

                            let btn_col = if is_selected { Color32::WHITE } else { track_color };
                            let name_btn = egui::Button::new(egui::RichText::new(btn_label).strong().size(9.0).color(btn_col))
                                .fill(card_bg)
                                .min_size(Vec2::new(76.0, 18.0));
                            if ui.add(name_btn).on_hover_text(crate::tstatus!("Spår {}: {}
Klicka för att öppna dedikerad EQ & detaljer", t_idx + 1, track_name)).clicked() {
                                self.selected_timeline_track = t_idx;
                                self.status_message = crate::tstatus!("Valde '{}' för EQ & mixjustering", track_name);
                            }

                            // Quick EQ Status / Bypass Pill & Rec Arm
                            ui.horizontal(|ui| {
                                let eq_on = self.playlist_tracks[t_idx].eq.enabled;
                                let eq_pill_bg = if eq_on { Color32::from_rgb(20, 50, 40) } else { Color32::from_rgb(45, 30, 30) };
                                let eq_pill_col = if eq_on { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                                let eq_txt = if eq_on { "EQ ON" } else { "EQ OFF" };
                                if ui.add(egui::Button::new(egui::RichText::new(eq_txt).size(7.0).color(eq_pill_col)).fill(eq_pill_bg).min_size(Vec2::new(36.0, 12.0))).clicked() {
                                    self.playlist_tracks[t_idx].eq.enabled = !self.playlist_tracks[t_idx].eq.enabled;
                                    self.sync_track_audio_state(t_idx);
                                }

                                let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                                let rec_bg = if is_armed { Color32::from_rgb(200, 30, 30) } else { Color32::from_rgb(26, 32, 42) };
                                let rec_col = if is_armed { Color32::WHITE } else { Theme::TEXT_MUTED };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("REC")).size(7.0).color(rec_col)).fill(rec_bg).min_size(Vec2::new(28.0, 12.0))).clicked() {
                                    self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                                }
                            });

                            // Mini EQ Curve Thumbnail
                            let (thumb_rect, _) = ui.allocate_exact_size(Vec2::new(68.0, 22.0), Sense::hover());
                            let eq_snapshot = self.playlist_tracks[t_idx].eq.clone();
                            mini_track_eq_curve(
                                ui.painter(),
                                thumb_rect,
                                eq_snapshot.low_gain_db,
                                eq_snapshot.mid_gain_db,
                                eq_snapshot.high_gain_db,
                                if is_mic { Color32::from_rgb(0, 220, 255) } else { track_color },
                                eq_snapshot.enabled,
                            );

                            // Pan Rotary Knob
                            let mut pan_val = self.playlist_tracks[t_idx].pan;
                            if rotary_knob(ui, &mut pan_val, -1.0, 1.0, "PAN", Color32::from_rgb(180, 195, 210), 15.0) {
                                self.playlist_tracks[t_idx].pan = pan_val;
                                self.sync_track_audio_state(t_idx);
                            }

                            // Mute / Solo Buttons
                            ui.horizontal(|ui| {
                                let muted = self.playlist_tracks[t_idx].muted;
                                let m_col = if muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 36, 48) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).size(7.5).color(if muted { Color32::WHITE } else { Theme::TEXT_MUTED })).fill(m_col).min_size(Vec2::new(16.0, 14.0))).clicked() {
                                    self.playlist_tracks[t_idx].muted = !muted;
                                    self.sync_track_audio_state(t_idx);
                                }

                                let solo = self.playlist_tracks[t_idx].solo;
                                let s_col = if solo { Theme::FL_ORANGE } else { Color32::from_rgb(30, 36, 48) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).size(7.5).color(if solo { Color32::BLACK } else { Theme::TEXT_MUTED })).fill(s_col).min_size(Vec2::new(16.0, 14.0))).clicked() {
                                    self.playlist_tracks[t_idx].solo = !solo;
                                    self.sync_track_audio_state(t_idx);
                                }
                            });

                            ui.add_space(2.0);

                            // Track Vertical Volume Fader with peak meter
                            let mut vol_val = self.playlist_tracks[t_idx].volume;
                            let meter_level = if self.is_playing && !self.playlist_tracks[t_idx].muted {
                                if is_mic {
                                    self.vocal_studio.mic_vu_level
                                } else {
                                    self.engine.get_peak_level()
                                }
                            } else {
                                0.0
                            };

                            let fader_col = if is_mic { Color32::from_rgb(0, 220, 255) } else { track_color };
                            if vertical_fader(ui, &mut vol_val, 0.0, 1.25, meter_level, fader_col, 100.0) {
                                self.playlist_tracks[t_idx].volume = vol_val;
                                self.sync_track_audio_state(t_idx);
                            }

                            let gain_db = if vol_val <= 0.001 { -60.0 } else { 20.0 * vol_val.log10() };
                            ui.label(egui::RichText::new(format!("{:.0}%\n{:+.1}dB", vol_val * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                        });
                    });
                }
            });
        });

        ui.add_space(8.0);

        // ================================================================
        // 1b. TIER 1B: SUB-MIX BUSSES & VCA GROUPS (Fas 5.2)
        // ================================================================
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🧩 SUB-MIX-BUSSAR & VCA-GRUPPER")).strong().size(11.5).color(Theme::FL_CYAN));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Bussar summerar spår; VCA styr grupper utan att routa ljudet.")).size(9.5).color(Theme::TEXT_MUTED));
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                let mut group_changed = false;

                for b in 0..crate::audio::synth::NUM_BUSES {
                    let bus_name = crate::i18n::t(crate::audio::synth::BUS_NAMES[b]);
                    ui.group(|ui| {
                        ui.set_width(76.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(format!("BUS {}", bus_name)).strong().size(9.5).color(Theme::FL_CYAN));
                            let mut vol = self.bus_volume[b];
                            if vertical_fader(ui, &mut vol, 0.0, 1.25, 0.0, Theme::FL_CYAN, 78.0) {
                                self.bus_volume[b] = vol;
                                group_changed = true;
                            }
                            ui.horizontal(|ui| {
                                let muted = self.bus_muted[b];
                                let m_col = if muted { Theme::FL_RED } else { Theme::TEXT_MUTED };
                                if ui.add(egui::Button::new(egui::RichText::new("M").size(10.0).color(m_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                    self.bus_muted[b] = !muted;
                                    group_changed = true;
                                }
                                let solo = self.bus_solo[b];
                                let s_col = if solo { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                                if ui.add(egui::Button::new(egui::RichText::new("S").size(10.0).color(s_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                    self.bus_solo[b] = !solo;
                                    group_changed = true;
                                }
                            });
                            // **Bussens egna kurvor** (Fas 8.8): här skapas de — vid
                            // reglaget de hör till. Motorn lyder dem redan; knapparna
                            // skriver bara ner det läge handen ställt in. Ingen gissning
                            // om vad kurvan "borde" vara: den blir vad fadern står på.
                            ui.horizontal(|ui| {
                                let ix = self.bus_automation.iter().position(|l| l.bus == b);
                                let på = ix.is_some_and(|i| self.bus_automation[i].enabled);
                                let punkter = ix.map_or(0, |i| self.bus_automation[i].points.len());
                                let färg = if på { Theme::FL_CYAN } else { Theme::TEXT_MUTED };
                                if ui
                                    .add(egui::Button::new(egui::RichText::new("🔗").size(10.0).color(färg)).min_size(egui::vec2(22.0, 16.0)))
                                    .on_hover_text(crate::tstatus!(
                                        "Bussens kurva — {} ({} punkter). Bussen äger den själv; flera spår kan skicka hit.",
                                        if på { crate::i18n::t("på") } else { crate::i18n::t("av") },
                                        punkter
                                    ))
                                    .clicked()
                                {
                                    match ix {
                                        Some(i) => self.bus_automation[i].enabled = !på,
                                        None => self.bus_automation.push(BusAutomationLane {
                                            bus: b,
                                            enabled: true,
                                            points: Vec::new(),
                                        }),
                                    }
                                }
                                if ui
                                    .add(egui::Button::new(egui::RichText::new("＋").size(10.0).color(Theme::TEXT_MUTED)).min_size(egui::vec2(22.0, 16.0)))
                                    .on_hover_text(crate::i18n::t("Lägg en punkt här, på faderns nuvarande nivå"))
                                    .clicked()
                                {
                                    let bar = self.tempo_map().bar_at_secs(self.song_time as f64) as f32;
                                    let value = self.bus_volume[b];
                                    let i = match ix {
                                        Some(i) => {
                                            self.bus_automation[i].enabled = true;
                                            i
                                        }
                                        None => {
                                            self.bus_automation.push(BusAutomationLane {
                                                bus: b,
                                                enabled: true,
                                                points: Vec::new(),
                                            });
                                            self.bus_automation.len() - 1
                                        }
                                    };
                                    let lane = &mut self.bus_automation[i];
                                    // En punkt per takt: två på samma plats ger en lodrät
                                    // kurva, och en sådan kan ingen mena.
                                    lane.points.retain(|p| (p.time_bars - bar).abs() > 1e-3);
                                    lane.points.push(AutomationPoint { time_bars: bar, value });
                                    lane.points.sort_by(|x, y| {
                                        x.time_bars
                                            .partial_cmp(&y.time_bars)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    self.status_message = crate::tstatus!(
                                        "🔗 Buss {}: punkt vid takt {:.2} ({:.0} %)",
                                        bus_name,
                                        bar,
                                        value * 100.0
                                    );
                                }
                            });
                            let gain_db = if self.bus_volume[b] <= 0.001 { -60.0 } else { 20.0 * self.bus_volume[b].log10() };
                            ui.label(egui::RichText::new(format!("{:.0}% {:+.1}dB", self.bus_volume[b] * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                        });
                    });
                }

                ui.separator();

                for v in 0..crate::audio::synth::NUM_VCAS {
                    ui.group(|ui| {
                        ui.set_width(76.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(format!("VCA {}", v + 1)).strong().size(9.5).color(Theme::FL_PURPLE));
                            let mut vol = self.vca_faders[v];
                            if vertical_fader(ui, &mut vol, 0.0, 1.25, 0.0, Theme::FL_PURPLE, 78.0) {
                                self.vca_faders[v] = vol;
                                group_changed = true;
                            }
                            ui.horizontal(|ui| {
                                let muted = self.vca_muted[v];
                                let m_col = if muted { Theme::FL_RED } else { Theme::TEXT_MUTED };
                                if ui.add(egui::Button::new(egui::RichText::new("M").size(10.0).color(m_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                    self.vca_muted[v] = !muted;
                                    group_changed = true;
                                }
                                let solo = self.vca_solos[v];
                                let s_col = if solo { Theme::FL_YELLOW } else { Theme::TEXT_MUTED };
                                if ui.add(egui::Button::new(egui::RichText::new("S").size(10.0).color(s_col)).min_size(egui::vec2(22.0, 16.0))).clicked() {
                                    self.vca_solos[v] = !solo;
                                    group_changed = true;
                                }
                            });
                            let gain_db = if self.vca_faders[v] <= 0.001 { -60.0 } else { 20.0 * self.vca_faders[v].log10() };
                            ui.label(egui::RichText::new(format!("{:.0}% {:+.1}dB", self.vca_faders[v] * 100.0, gain_db)).size(8.0).color(Theme::TEXT_MUTED));
                        });
                    });
                }

                if group_changed {
                    self.sync_group_state();
                }
            });
        });

        ui.add_space(8.0);

        // ================================================================
        // 2. TIER 2: DEDICATED PARAMETRIC EQ & CHANNEL STRIP FOR SELECTED TRACK
        // ================================================================
        if !self.playlist_tracks.is_empty() {
            let sel_idx = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
            let sel_t_name = self.playlist_tracks[sel_idx].name.clone();
            let sel_t_col = self.playlist_tracks[sel_idx].color;
            let sel_t_icon = self.playlist_tracks[sel_idx].icon;
            let mut track_dirty = false;

            ui.group(|ui| {
                // Header for Selected Track EQ Console
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("🎛 DEDIKERAD PARAMETRISK EQ — {} {}", sel_t_icon, sel_t_name)).strong().size(12.5).color(sel_t_col));
                    ui.separator();
                    let mut eq_enabled = self.playlist_tracks[sel_idx].eq.enabled;
                    let eq_txt = if eq_enabled { "EQ Aktiv" } else { "Bypass (Av)" };
                    let eq_col = if eq_enabled { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                    if ui.checkbox(&mut eq_enabled, egui::RichText::new(eq_txt).size(11.0).color(eq_col)).changed() {
                        self.playlist_tracks[sel_idx].eq.enabled = eq_enabled;
                        track_dirty = true;
                    }

                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("Buss:")).size(10.0).color(Theme::TEXT_MUTED));
                    let cur_bus = self.playlist_tracks[sel_idx].bus.min(crate::audio::synth::NUM_BUSES - 1);
                    egui::ComboBox::from_id_salt("sel_track_bus")
                        .selected_text(crate::i18n::t(crate::audio::synth::BUS_NAMES[cur_bus]))
                        .width(84.0)
                        .show_ui(ui, |ui| {
                            for b in 0..crate::audio::synth::NUM_BUSES {
                                if ui.selectable_label(cur_bus == b, crate::i18n::t(crate::audio::synth::BUS_NAMES[b])).clicked() && cur_bus != b {
                                    self.playlist_tracks[sel_idx].bus = b;
                                    track_dirty = true;
                                }
                            }
                        });

                    ui.label(egui::RichText::new("VCA:").size(10.0).color(Theme::TEXT_MUTED));
                    let cur_vca = self.playlist_tracks[sel_idx].vca;
                    egui::ComboBox::from_id_salt("sel_track_vca")
                        .selected_text(match cur_vca {
                            Some(v) => format!("VCA {}", v + 1),
                            None => crate::i18n::t("Ingen").to_string(),
                        })
                        .width(84.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(cur_vca.is_none(), crate::i18n::t("Ingen")).clicked() {
                                self.playlist_tracks[sel_idx].vca = None;
                                track_dirty = true;
                            }
                            for v in 0..crate::audio::synth::NUM_VCAS {
                                if ui.selectable_label(cur_vca == Some(v), format!("VCA {}", v + 1)).clicked() {
                                    self.playlist_tracks[sel_idx].vca = Some(v);
                                    track_dirty = true;
                                }
                            }
                        });

                    // Sidokedja (Fas 8.3): spåret duckas av ett annat spårs ljud.
                    ui.label(egui::RichText::new(crate::i18n::t("Sidokedja:")).size(10.0).color(Theme::TEXT_MUTED));
                    if self.playlist_tracks.len() < 2 {
                        // En sidokedja behöver ett annat spår att lyssna på.
                        ui.label(egui::RichText::new(crate::i18n::t("(behöver två spår)")).size(10.0).color(Theme::TEXT_MUTED));
                    } else {
                        let cur_key = self.playlist_tracks[sel_idx].sidechain_from;
                        egui::ComboBox::from_id_salt("sel_track_sidechain")
                            .selected_text(match cur_key {
                                Some(k) => format!("{} {}", crate::i18n::t("Spår"), k + 1),
                                None => crate::i18n::t("Ingen").to_string(),
                            })
                            .width(84.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(cur_key.is_none(), crate::i18n::t("Ingen")).clicked() && cur_key.is_some() {
                                    self.playlist_tracks[sel_idx].sidechain_from = None;
                                    track_dirty = true;
                                }
                                for k in 0..self.playlist_tracks.len() {
                                    // Ett spår kan inte ducka sig självt — det är en slinga.
                                    if k == sel_idx {
                                        continue;
                                    }
                                    if ui.selectable_label(cur_key == Some(k), format!("{} {}", crate::i18n::t("Spår"), k + 1)).clicked() && cur_key != Some(k) {
                                        self.playlist_tracks[sel_idx].sidechain_from = Some(k);
                                        track_dirty = true;
                                    }
                                }
                            });
                        if cur_key.is_some() {
                            let sc_track = &mut self.playlist_tracks[sel_idx];
                            track_dirty |= ui
                                .add(egui::Slider::new(&mut sc_track.sidechain_amount_db, 0.0..=24.0).text(crate::i18n::t("Duckning (dB)")))
                                .changed();
                            track_dirty |= ui
                                .add(egui::Slider::new(&mut sc_track.sidechain_threshold_db, -60.0..=0.0).text(crate::i18n::t("Tröskel (dB)")))
                                .changed();
                        }
                    }

                    // Sends (Fas 8.13 bussar, Fas 8.3 spår): en del av spårets signal
                    // till en annan **buss** eller in i ett annat **spårs kedja**.
                    // Post-fader i båda fallen — samma signal som går till spårets
                    // egen buss, efter volym, EQ, kompressor och sidokedja.
                    //
                    // En spår-send kan bli en slinga (A → B → A), och en slinga går
                    // inte att beräkna: den skulle mata sig själv. Därför prövas varje
                    // ändring mot `plan_track_order` **innan** den får skrivas, och
                    // ett nej säger vilka två spår det gällde. Motorn har samma regel
                    // och behåller sin förra ordning om en fil ändå skulle bära en
                    // slinga — den gissar aldrig.
                    ui.label(egui::RichText::new(crate::i18n::t("Skicka till:")).size(10.0).color(Theme::TEXT_MUTED));
                    {
                        // Kandidaterna och grafen samlas in **före** det mutabla lånet
                        // av spårets send-lista: att läsa ett annat spår inifrån den
                        // låningen går inte.
                        let track_choices: Vec<(usize, String)> = self
                            .playlist_tracks
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| *i != sel_idx)
                            .map(|(i, t)| (i, crate::tstatus!("{} — {}", i + 1, t.name)))
                            .collect();
                        let track_count = self.playlist_tracks.len();
                        let base_graph: Vec<Vec<usize>> = self
                            .playlist_tracks
                            .iter()
                            .map(|t| {
                                t.sends
                                    .iter()
                                    .map(|s| s.target.as_track().unwrap_or(usize::MAX))
                                    .collect()
                            })
                            .collect();

                        let sends = &mut self.playlist_tracks[sel_idx].sends;
                        let mut remove_send: Option<usize> = None;
                        for (s_idx, send) in sends.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                let selected = match send.target {
                                    crate::audio::SendTarget::Bus { target_bus } => {
                                        let b = target_bus.min(crate::audio::synth::NUM_BUSES - 1);
                                        crate::audio::synth::BUS_NAMES[b].to_string()
                                    }
                                    crate::audio::SendTarget::Track { target_track } => {
                                        format!("Spår {}", target_track + 1)
                                    }
                                };
                                egui::ComboBox::from_id_salt(format!("sel_track_send_{s_idx}"))
                                    .selected_text(selected)
                                    .width(120.0)
                                    .show_ui(ui, |ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Bussar")).size(10.0).color(Theme::TEXT_MUTED));
                                        for (bi, name) in crate::audio::synth::BUS_NAMES.iter().enumerate() {
                                            if ui
                                                .selectable_label(send.target.as_bus() == Some(bi), *name)
                                                .clicked()
                                            {
                                                send.target = crate::audio::SendTarget::bus(bi);
                                                track_dirty = true;
                                            }
                                        }
                                        ui.separator();
                                        ui.label(egui::RichText::new(crate::i18n::t("Spår (in i dess kedja)")).size(10.0).color(Theme::TEXT_MUTED));
                                        for (ti, name) in &track_choices {
                                            if ui
                                                .selectable_label(send.target.as_track() == Some(*ti), name)
                                                .clicked()
                                            {
                                                // Slingkontrollen: samma graf, men med
                                                // den här raden ändrad.
                                                let mut graph = base_graph.clone();
                                                if let Some(row) = graph.get_mut(sel_idx).and_then(|r| r.get_mut(s_idx)) {
                                                    *row = *ti;
                                                }
                                                match crate::audio::command::plan_track_order(track_count, &graph) {
                                                    Ok(_) => {
                                                        send.target = crate::audio::SendTarget::track(*ti);
                                                        track_dirty = true;
                                                    }
                                                    Err((a, b)) => {
                                                        self.status_message = crate::tstatus!(
                                                            "⚠ Det skulle bli en slinga (spår {} → spår {}) — senden ändrades inte.",
                                                            a + 1, b + 1
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    });
                                track_dirty |= ui
                                    .add(egui::Slider::new(&mut send.level, 0.0..=2.0).show_value(true))
                                    .on_hover_text(crate::i18n::t("Hur mycket av spåret som går till målet (1,0 = lika starkt som spårets egen utgång)"))
                                    .changed();
                                if ui.button("🗑").on_hover_text(crate::i18n::t("Ta bort senden")).clicked() {
                                    remove_send = Some(s_idx);
                                }
                            });
                        }
                        if let Some(i) = remove_send {
                            sends.remove(i);
                            track_dirty = true;
                        }
                        if sends.len() < 4
                            && ui
                                .button(crate::i18n::t("➕ Ny send"))
                                .on_hover_text(crate::i18n::t("Skicka en del av spåret till en buss eller in i ett annat spårs kedja — t.ex. FX-bussen"))
                                .clicked()
                        {
                            // FX-bussen är standardmål: det är den vanligaste
                            // parallella vägen, och den kan ändras direkt.
                            sends.push(crate::audio::StemSend {
                                target: crate::audio::SendTarget::bus(crate::audio::synth::NUM_BUSES - 1),
                                level: 0.5,
                            });
                            track_dirty = true;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}", sel_idx + 1, self.playlist_tracks.len())).size(10.5).color(Theme::TEXT_MUTED));
                    });
                });

                ui.add_space(4.0);

                // Responsive 2-Row / Multi-Column Section for Parametric EQ, Curves, Presets & Dynamics
                let mut status_msg_update: Option<String> = None;
                let track_mut = &mut self.playlist_tracks[sel_idx];

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);

                    // Block A: 3-Band Parametric EQ Knobs
                    ui.group(|ui| {
                        ui.set_width(260.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("📊 3-BANDS FREKVENSJUSTERING")).strong().size(10.5).color(Theme::FL_ORANGE));
                            ui.separator();

                            ui.horizontal(|ui| {
                                // Low Band
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("LOW SHELF")).size(8.5).color(Theme::FL_ORANGE));
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.low_gain_db, -12.0, 12.0, "GAIN", Theme::FL_ORANGE, 20.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.low_freq, 30.0, 400.0, "FREQ", Theme::FL_ORANGE, 16.0);
                                    ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.low_freq, track_mut.eq.low_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                });

                                ui.separator();

                                // Mid Band
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("MID BELL")).size(8.5).color(Theme::FL_YELLOW));
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_gain_db, -12.0, 12.0, "GAIN", Theme::FL_YELLOW, 20.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_freq, 200.0, 8000.0, "FREQ", Theme::FL_YELLOW, 16.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_q, 0.4, 4.0, "Q", Theme::FL_YELLOW, 16.0);
                                    ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.mid_freq, track_mut.eq.mid_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                });

                                ui.separator();

                                // High Band
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("HIGH SHELF")).size(8.5).color(Theme::FL_CYAN));
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.high_gain_db, -12.0, 12.0, "GAIN", Theme::FL_CYAN, 20.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.eq.high_freq, 2500.0, 16000.0, "FREQ", Theme::FL_CYAN, 16.0);
                                    ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.high_freq, track_mut.eq.high_gain_db)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                            });
                        });
                    });

                    // Block B: Interactive EQ Curve Display
                    ui.group(|ui| {
                        ui.set_width(320.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("📈 INTERAKTIV FREKVENSKURVA")).strong().size(10.5).color(Theme::FL_CYAN));
                            ui.separator();
                            track_dirty |= eq_curve_visualizer(
                                ui,
                                &mut track_mut.eq.low_gain_db,
                                &mut track_mut.eq.mid_gain_db,
                                &mut track_mut.eq.high_gain_db,
                                Vec2::new(305.0, 84.0),
                            );
                        });
                    });

                    // Block C: Quick EQ Presets & Reset
                    ui.group(|ui| {
                        ui.set_width(200.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("⚡ SNABBPRESETS")).strong().size(10.5).color(Theme::FL_GREEN));
                            ui.separator();

                            ui.horizontal_wrapped(|ui| {
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sång (Vocal Air)")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 50, 70))).clicked() {
                                    track_mut.eq.low_gain_db = -2.5;
                                    track_mut.eq.low_freq = 120.0;
                                    track_mut.eq.mid_gain_db = 2.0;
                                    track_mut.eq.mid_freq = 3200.0;
                                    track_mut.eq.high_gain_db = 4.0;
                                    track_mut.eq.high_freq = 11000.0;
                                    track_mut.eq.enabled = true;
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Sång (Vocal Air)' på {}", sel_t_name));
                                }

                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗣 Kör / Harmoni")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 30, 70))).clicked() {
                                    track_mut.eq.low_gain_db = -4.0;
                                    track_mut.eq.low_freq = 160.0;
                                    track_mut.eq.mid_gain_db = -1.5;
                                    track_mut.eq.mid_freq = 1200.0;
                                    track_mut.eq.high_gain_db = 3.0;
                                    track_mut.eq.high_freq = 9000.0;
                                    track_mut.eq.enabled = true;
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Kör / Harmoni' på {}", sel_t_name));
                                }

                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🥁 Trum-Punch")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(70, 45, 20))).clicked() {
                                    track_mut.eq.low_gain_db = 4.5;
                                    track_mut.eq.low_freq = 80.0;
                                    track_mut.eq.mid_gain_db = -2.5;
                                    track_mut.eq.mid_freq = 450.0;
                                    track_mut.eq.high_gain_db = 3.5;
                                    track_mut.eq.high_freq = 7000.0;
                                    track_mut.eq.enabled = true;
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Trum-Punch' på {}", sel_t_name));
                                }

                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Bas / Sub Power")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(20, 60, 35))).clicked() {
                                    track_mut.eq.low_gain_db = 5.0;
                                    track_mut.eq.low_freq = 65.0;
                                    track_mut.eq.mid_gain_db = -2.0;
                                    track_mut.eq.mid_freq = 600.0;
                                    track_mut.eq.high_gain_db = -4.0;
                                    track_mut.eq.high_freq = 4000.0;
                                    track_mut.eq.enabled = true;
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Bas / Sub Power' på {}", sel_t_name));
                                }

                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Gitarr / Presence")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(65, 55, 20))).clicked() {
                                    track_mut.eq.low_gain_db = -2.0;
                                    track_mut.eq.low_freq = 100.0;
                                    track_mut.eq.mid_gain_db = 3.0;
                                    track_mut.eq.mid_freq = 2400.0;
                                    track_mut.eq.high_gain_db = 2.0;
                                    track_mut.eq.high_freq = 6000.0;
                                    track_mut.eq.enabled = true;
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Gitarr / Presence' på {}", sel_t_name));
                                }

                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Nollställ EQ")).size(8.5).color(Color32::WHITE)).fill(Color32::from_rgb(45, 50, 60))).clicked() {
                                    track_mut.eq = TrackEq::default();
                                    track_dirty = true;
                                    status_msg_update = Some(crate::tstatus!("🔄 Nollställde EQ för {}", sel_t_name));
                                }
                            });
                        });
                    });

                    // Block D: Dynamics & FX Sends for this Track
                    ui.group(|ui| {
                        ui.set_width(250.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🎚 DYNAMIK & EFFEKT-SÄNDNING")).strong().size(10.5).color(Theme::FL_PURPLE));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("KOMPRESSOR")).size(8.0).color(Theme::FL_GREEN));
                                    track_dirty |= rotary_knob(ui, &mut track_mut.comp_threshold_db, -36.0, 0.0, "TRSH", Theme::FL_GREEN, 17.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.comp_ratio, 1.0, 10.0, "RATIO", Theme::FL_GREEN, 17.0);
                                });

                                ui.separator();

                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("SÄNDNING")).size(8.0).color(Theme::FL_PURPLE));
                                    track_dirty |= rotary_knob(ui, &mut track_mut.reverb_send, 0.0, 1.0, "REV", Theme::FL_PURPLE, 17.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.delay_send, 0.0, 1.0, "DLY", Theme::FL_CYAN, 17.0);
                                });

                                ui.separator();

                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("TONHÖJD")).size(8.0).color(Theme::FL_YELLOW));
                                    track_dirty |= pitch_knob(ui, &mut track_mut.pitch_semitones, -12.0, 12.0, "PITCH", Theme::FL_YELLOW, 17.0);
                                    ui.label(egui::RichText::new(format!("{:+.2} st", track_mut.pitch_semitones)).size(7.5).color(Theme::TEXT_MUTED));
                                });
                            });
                        });
                    });
                });

                if let Some(msg) = status_msg_update {
                    self.status_message = msg;
                }
            });

            if track_dirty {
                self.sync_track_audio_state(sel_idx);
            }
        }

        ui.add_space(8.0);

        // ================================================================
        // 3. TIER 3: MASTER FX STUDIO (SPACE, DELAY & ANALOG DRIVE)
        // ================================================================
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 6.0);

                // Delay FX
                ui.group(|ui| {
                    ui.set_width(320.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🌊 MASTER STEREO DELAY")).strong().size(10.5).color(Theme::FL_CYAN));
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.delay.time_ms, 30.0, 800.0, "TID (ms)", Theme::FL_CYAN, 19.0);
                            ch |= rotary_knob(ui, &mut self.delay.feedback, 0.0, 0.88, "FEEDBACK", Theme::FL_CYAN, 19.0);
                            ch |= rotary_knob(ui, &mut self.delay.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 19.0);
                            if ch { let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay)); }
                        });
                    });
                });

                // Reverb FX
                ui.group(|ui| {
                    ui.set_width(320.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("✨ MASTER SPACE REVERB")).strong().size(10.5).color(Theme::FL_PURPLE));
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.reverb.room_size, 0.1, 0.95, "RUM", Theme::FL_PURPLE, 19.0);
                            ch |= rotary_knob(ui, &mut self.reverb.damping, 0.05, 0.9, "DÄMP", Theme::FL_PURPLE, 19.0);
                            ch |= rotary_knob(ui, &mut self.reverb.mix, 0.0, 1.0, "MIX", Theme::FL_ORANGE, 19.0);
                            if ch { let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb)); }
                        });
                    });
                });

                // Drive & Limiter FX
                ui.group(|ui| {
                    ui.set_width(340.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🔥 MASTER ANALOG DRIVE & LIMITER")).strong().size(10.5).color(Theme::FL_YELLOW));
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut ch = false;
                            ch |= rotary_knob(ui, &mut self.stompbox_drive, 1.0, 8.0, "DRIVE", Theme::FL_ORANGE, 19.0);
                            ch |= rotary_knob(ui, &mut self.stompbox_tone, 1000.0, 10000.0, "TON", Theme::FL_YELLOW, 19.0);
                            ui.checkbox(&mut self.stompbox_cab, "4x12 Cab");
                            if ch {
                                self.drive = self.stompbox_drive;
                                let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                            }
                        });
                    });
                });
            });
        });
    });
}
}

impl SonixApp {
pub(crate) fn render_synth_hardware_rack(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX HARDWARE SYNTHESIZER")).strong().size(13.0).color(Theme::FL_CYAN));
            ui.separator();

            // Presets
            ui.label(egui::RichText::new(crate::i18n::t("Presets:")).color(Theme::TEXT_MUTED));
            let presets = [
                (Preset::CleanPluck, "Clean Pluck"),
                (Preset::WarmPad, "Warm Pad"),
                (Preset::AcidBass, "303 Acid Bass"),
                (Preset::ChiptuneLead, "8-Bit Chiptune"),
                (Preset::CosmicBrass, "Cosmic Brass"),
            ];
            for (p, name) in presets {
                let is_active = self.current_preset == p;
                let fill = if is_active { Theme::FL_CYAN } else { Theme::PANEL_BG };
                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                if ui.add(egui::Button::new(egui::RichText::new(name).strong().color(text_color)).fill(fill)).clicked() {
                    self.current_preset = p;
                    let (wf, adsr, flt) = p.settings();
                    self.waveform = wf;
                    self.adsr = adsr;
                    self.filter = flt;
                    let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                }
            }
        });

        ui.add_space(8.0);

        ui.columns(4, |cols| {
            // Column 1: Waveform & Octave
            cols[0].group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("OSCILLATOR")).strong());
                let waveforms = [
                    (Waveform::Sine, crate::i18n::t("∿ Sinus")),
                    (Waveform::Saw, crate::i18n::t("⩘ Sågtand")),
                    (Waveform::Square, crate::i18n::t("⊓ Fyrkant")),
                    (Waveform::Triangle, crate::i18n::t("⋀ Triangel")),
                ];
                for (wf, name) in waveforms {
                    let is_wf = self.waveform == wf;
                    if ui.selectable_label(is_wf, name).clicked() {
                        self.waveform = wf;
                        let _ = self.engine.send_command(AudioCommand::SetWaveform(wf));
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Oktav:"));
                    if ui.button(crate::i18n::t("➖ (Z)")).clicked() && self.octave > 1 { self.octave -= 1; }
                    ui.label(egui::RichText::new(format!("C{}", self.octave)).strong().color(Theme::FL_CYAN));
                    if ui.button(crate::i18n::t("➕ (X)")).clicked() && self.octave < 8 { self.octave += 1; }
                });
            });

            // Column 2: Resonant filter controls
            cols[1].group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("RESONANT SVF FILTER (12 dB)")).strong().color(Theme::FL_ORANGE));
                ui.horizontal(|ui| {
                    let mut flt_changed = false;
                    let mut fenv_changed = false;
                    flt_changed |= rotary_knob(ui, &mut self.filter.cutoff, 60.0, 18000.0, "CUTOFF", Theme::FL_ORANGE, 40.0);
                    flt_changed |= rotary_knob(ui, &mut self.filter.resonance, 0.5, 9.5, "RESO (Q)", Theme::FL_YELLOW, 40.0);
                    fenv_changed |= rotary_knob(ui, &mut self.filter_env_amount, -6.0, 6.0, "ENV ±oct", Theme::FL_ORANGE, 40.0);
                    if flt_changed {
                        let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                    }
                    if fenv_changed {
                        let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
                            amount: self.filter_env_amount,
                            adsr: self.filter_env,
                        });
                    }
                });
                ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz | Q: {:.1} | Env: {:+.1} oct", self.filter.cutoff, self.filter.resonance, self.filter_env_amount)).size(10.0).color(Theme::TEXT_MUTED));
            });

            // Column 3: ADSR Rotary Envelopes
            cols[2].group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("ADSR ENVELOPE")).strong().color(Theme::FL_PURPLE));
                let mut adsr_changed = false;
                ui.horizontal(|ui| {
                    adsr_changed |= rotary_knob(ui, &mut self.adsr.attack, 0.002, 2.0, "ATTK", Theme::FL_PURPLE, 32.0);
                    adsr_changed |= rotary_knob(ui, &mut self.adsr.decay, 0.01, 3.0, "DECAY", Theme::FL_PURPLE, 32.0);
                    adsr_changed |= rotary_knob(ui, &mut self.adsr.sustain, 0.0, 1.0, "SUST", Theme::FL_PURPLE, 32.0);
                    adsr_changed |= rotary_knob(ui, &mut self.adsr.release, 0.01, 4.0, "REL", Theme::FL_PURPLE, 32.0);
                });
                if adsr_changed {
                    let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
                }
            });

            // Column 4: ADSR Visual Curve Display
            cols[3].group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("ENVELOPE GRAF")).strong().color(Theme::FL_GREEN));
                let (rect, _) = ui.allocate_exact_size(Vec2::new(130.0, 68.0), Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(rect, Rounding::same(3.0), Theme::LCD_BG);
                painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 52)));

                let p0 = Pos2::new(rect.min.x + 4.0, rect.max.y - 4.0);
                let p1 = Pos2::new(rect.min.x + 4.0 + (self.adsr.attack * 20.0).min(30.0), rect.min.y + 6.0);
                let p2 = Pos2::new(p1.x + (self.adsr.decay * 18.0).min(30.0), rect.max.y - 4.0 - (self.adsr.sustain * (rect.height() - 14.0)));
                let p3 = Pos2::new(p2.x + 25.0, p2.y);
                let p4 = Pos2::new((p3.x + (self.adsr.release * 16.0).min(35.0)).min(rect.max.x - 4.0), rect.max.y - 4.0);

                painter.line_segment([p0, p1], Stroke::new(2.0_f32, Theme::FL_GREEN));
                painter.line_segment([p1, p2], Stroke::new(2.0_f32, Theme::FL_GREEN));
                painter.line_segment([p2, p3], Stroke::new(2.0_f32, Theme::FL_GREEN));
                painter.line_segment([p3, p4], Stroke::new(2.0_f32, Theme::FL_GREEN));
            });
        });
    });
}
}

fn rand_simple(seed: usize) -> f32 {
    let mut x = (seed as u32).wrapping_mul(1103515245).wrapping_add(12345);
    x = (x >> 16) & 0x7FFF;
    x as f32 / 32767.0
}

