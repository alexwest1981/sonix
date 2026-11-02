//! Mixern — kanalracket, FX-racket och automationen.
//!
//! Automationskurvan interpoleras i `lane_value_at` (i `state`) och läses av
//! `apply_automation` varje bildruta. Målen kommer från `AutomationParam::ALL` —
//! **härled listan ur listan**; ett handskrivet index pekar tyst fel.
//!
//! Status: byggs — mixern och FX-racket.
//! Rör inte: kommandona bär hela inställningen; att bygga en ny nollar tyst frekvenser och Q.

use super::*;
use crate::ui::theme::Theme;

/// A chosen `.sf2` waiting for the preset choice. `selected == 0` keeps the old behaviour:
/// all zones of the first instrument that has any.
pub struct Sf2Pick {
    pub channel: usize,
    pub path: String,
    pub selected: usize,
    pub sf: crate::audio::sf2::SoundFont,
}

/// Automatically classify stem and project tracks by name to assign distinct category color, icon, and track kind.
pub fn classify_track_style(name: &str) -> (TrackKind, &'static str, Color32) {
    let n = name.to_lowercase();
    if n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || (n.contains("voice") && (n.contains("rec") || n.contains("mic") || n.contains("sång"))) {
        (TrackKind::VocalAudio, "🎤", crate::ui::theme::palette().text())
    } else if n.contains("lead") || n.contains("huvudsång") || n.contains("0 lead") || n.contains("leadvocal") || n.contains("lead vox") {
        (TrackKind::VocalAudio, "🎙", crate::ui::theme::palette().track_cyan()) // Vibrant Cyan
    } else if n.contains("backing") || n.contains("backvocal") || n.contains("back vocal") || n.contains("chorus") || n.contains("harmony") || n.contains("harmonies") || n.contains("bgv") || n.contains("1 backing") || n.contains("choir") || n.contains("kör") || n.contains("stämmor") || n.contains("vocal") || n.contains("vox") || n.contains("sång") {
        (TrackKind::VocalAudio, "🗣", crate::ui::theme::palette().track_magenta()) // Magenta / Purple
    } else if n.contains("percussion") || n.contains("6 percussion") || n.contains("perc") || n.contains("shaker") || n.contains("conga") || n.contains("bongo") || n.contains("tambourine") || n.contains("cowbell") {
        (TrackKind::Drums, "🪘", crate::ui::theme::palette().track_amber()) // Vibrant Coral Rust
    } else if n.contains("drum") || n.contains("2 drums") || n.contains("trumm") || n.contains("kick") || n.contains("snare") || n.contains("hihat") || n.contains("hat") || n.contains("beat") || n.contains("slagverk") {
        (TrackKind::Drums, "🥁", crate::ui::theme::palette().track_orange()) // Electric Orange
    } else if n.contains("bass") || n.contains("3 bass") || n.contains("bas") || n.contains("sub") || n.contains("808") {
        (TrackKind::Bassline, "🎸", crate::ui::theme::palette().track_green()) // Vivid Neon Green
    } else if n.contains("string") || n.contains("7 strings") || n.contains("stråk") || n.contains("violin") || n.contains("cello") || n.contains("orchestra") || n.contains("orchestral") {
        (TrackKind::SynthLead, "🎻", crate::ui::theme::palette().track_teal()) // Emerald / Aquamarine Teal
    } else if n.contains("guitar") || n.contains("4 guitar") || n.contains("gitarr") || n.contains("acoustic") || n.contains("electric") || n.contains("strum") || n.contains("gtr") {
        (TrackKind::SynthLead, "🎸", crate::ui::theme::palette().track_violet()) // Golden Amber
    } else if n.contains("key") || n.contains("5 keyboard") || n.contains("piano") || n.contains("flygel") || n.contains("organ") || n.contains("orgel") || n.contains("clav") || n.contains("rhodes") {
        (TrackKind::SynthLead, "🎹", crate::ui::theme::palette().track_blue()) // Royal / Electric Blue
    } else if n.contains("synth") || n.contains("8 synth") || n.contains("synthesizer") || n.contains("pad") || n.contains("pluck") || n.contains("arp") || n.contains("brass") {
        (TrackKind::SynthLead, "🎛", crate::ui::theme::palette().track_steel()) // Neon Violet
    } else if n.contains("fx") || n.contains("sfx") || n.contains("riser") || n.contains("sweep") || n.contains("impact") || n.contains("noise") || n.contains("effekt") || n.contains("atmos") || n.contains("other") || n.contains("9 other") {
        (TrackKind::Fx, "✨", crate::ui::theme::palette().track_rose()) // Rose Pink
    } else {
        (TrackKind::CustomAudio, "🎵", crate::ui::theme::palette().text_muted())
    }
}

impl SonixApp {
/// Mixen fryst till siffror — det A/B-knapparna jämför: volym, pan och mute per
/// spår i projektets ordning, plus masternivån.
pub(crate) fn mix_snapshot(&self, name: &str) -> crate::ui::app::compare::MixSnapshot {
    crate::ui::app::compare::MixSnapshot {
        name: name.to_string(),
        tracks: self.playlist_tracks.iter().map(|t| (t.volume, t.pan, t.muted)).collect(),
        master: self.master_volume,
    }
}

/// **Transmitter**: kanalens valda sampelfil blir hörbara händelser på ett nytt spår.
pub(crate) fn add_transmitter_track(&mut self) {
    let Some(path) = self.channels.get(self.selected_channel).and_then(|c| c.sample_path.clone()) else {
        self.status_message =
            crate::i18n::t("Välj en fil först — den valda kanalen har inget sampel").to_string();
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            self.status_message = crate::tstatus!("⚠️ Kunde inte läsa '{}': {}", path, e);
            return;
        }
    };
    let sr = 44_100u32;
    let pcm = crate::instruments::transmitter_wav(&bytes, sr);
    if pcm.is_empty() {
        self.status_message = crate::i18n::t("Filen gav inga händelser att spela").to_string();
        return;
    }
    let bars = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
        .bars_for_secs_at(0.0, pcm.len() as f64 / sr as f64) as f32;
    let peaks: Vec<f32> = pcm
        .chunks((pcm.len() / 256).max(1))
        .map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())))
        .collect();
    let name = std::path::Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| crate::i18n::t("Transmitter").to_string());
    let arc = std::sync::Arc::new(pcm);
    let mut track = PlaylistTrack::new(format!("📡 {}", name), "📡", TrackKind::CustomAudio, crate::ui::theme::palette().cyan());
    track.custom_clip_name = Some(name.clone());
    track.pcm_audio = Some((arc.clone(), arc, sr));
    // Regionen är det som gör spåret hörbart: motorn spelar regioner, inte pcm-bufferten.
    track.regions = vec![AudioRegion {
        id: self.playlist_tracks.len() + 1,
        name: name.clone(),
        start_bar: 0.0,
        length_bars: bars.max(0.25),
        sample_offset_sec: 0.0,
        source_path: Some(path),
        waveform_peaks: peaks,
        volume: 1.0,
        fade_in_bars: 0.0,
        fade_out_bars: 0.0,
        muted: false,
        is_reverse: false,
        color: crate::ui::theme::palette().cyan(),
        loop_length_bars: 0.0,
        // Filens tempo är okänt: ljudet rörs inte när projektets tempo ändras.
        source_bpm: 0.0,
        tape: false,
        warp_mode: Default::default(),
}];
    self.playlist_tracks.push(track);
    let idx = self.playlist_tracks.len() - 1;
    self.sync_track_stem_to_engine(idx);
    self.status_message = crate::tstatus!("📡 Transmitter: {} på nytt spår {}", name, idx + 1);
}
}

impl SonixApp {
/// Pulls the real output waveform samples produced since the last UI frame
/// and keeps a sliding history for the oscilloscope display.
pub(crate) fn update_scope_history(&mut self) {
    let new = self.engine.drain_scope_samples();
    let had_new = !new.is_empty();
    if had_new {
        for (mid, side) in new {
            self.scope_history.push(mid);
            self.scope_side.push(side);
        }
        if self.scope_history.len() > Self::SCOPE_HISTORY_MAX {
            let excess = self.scope_history.len() - Self::SCOPE_HISTORY_MAX;
            self.scope_history.drain(0..excess);
            self.scope_side.drain(0..excess);
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
/// **Fruity Scratcher / Wave Traveller**: gnuggar samplet fram och tillbaka, sex halva
/// pass i 0,7× hastighet — handen, inte en loop. Rösten tystnar av sig själv.
pub fn scratch_library_sample(&mut self, item: &LibrarySampleItem) {
    let Some(path) = item.file_path.as_deref() else {
        self.status_message = crate::tstatus!(
            "⚠ '{}' har ingen ljudfil att scratcha — välj ett sample med fil",
            item.name
        );
        return;
    };
    match load_sample_pcm_arcs(path) {
        Some((left, right, sr)) => {
            let end = left.len() as f32;
            let _ = self.engine.send_command(AudioCommand::PlayScratch {
                left,
                right,
                sample_rate: sr,
                volume: 0.95,
                start_frame: 0.0,
                end_frame: end,
                speed: 0.7,
                passes: 6,
            });
            self.status_message = crate::tstatus!("🖐 Scratch: '{}'", item.name);
        }
        None => {
            self.status_message = crate::tstatus!(
                "⚠ Kunde inte scratcha '{}' — filen går inte att läsa: {}",
                item.name,
                path
            );
        }
    }
}

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
            let _ = self.engine.send_command(AudioCommand::NoteOn { note: item.default_note, freq, velocity: 0.9, channel: 0 });
        }
    }
}
}

impl SonixApp {
/// **Spårets plugin-parametrar** (Fas 8.8) — listan som bara finns när en plugin är laddad.
///
/// Handtaget i `plugin_handles` är samma instans som motorn renderar (`PluginHandle` delar
/// `Arc` med inserten), så listan kommer från pluginen själv och inte från en kopia. Ingen
/// plugin → ingen lista, och ingen gissning om vilka parametrar den "borde" ha.
pub(crate) fn plugin_parameters(
    &self,
    track: usize,
) -> &[crate::audio::plugin_host_live::PluginParameter] {
    self.plugin_handles
        .get(track)
        .and_then(|h| h.as_ref())
        .map_or(&[], |h| h.parameters())
}

/// En parameter hos spårets plugin, om den finns.
pub(crate) fn plugin_parameter(
    &self,
    track: usize,
    param_id: u32,
) -> Option<&crate::audio::plugin_host_live::PluginParameter> {
    self.plugin_parameters(track)
        .iter()
        .find(|p| p.id == param_id)
}

/// **Målets kurva** (Fas 8.8) — en väg till båda listorna.
///
/// Spårkurvan bor på spåret (`playlist_tracks[i].automation`, nycklad på `param`), pluginens
/// i `plugin_automation` (nycklad på `(spår, param_id)`). Ritningen och redigeringen går
/// genom det här uppslaget i stället för att känna till listorna var för sig — och ett mål som
/// hör till ett **annat** spår än det som ritas ger `None`, så en plugin-kurva aldrig ritas
/// över fel spår.
pub(crate) fn automation_curve(
    &self,
    target: AutomationTarget,
    track_idx: usize,
) -> Option<AutomationCurve<'_>> {
    match target {
        AutomationTarget::Track(param) => {
            let lane = self
                .playlist_tracks
                .get(track_idx)?
                .automation
                .iter()
                .find(|l| l.param == param)?;
            Some(AutomationCurve {
                enabled: lane.enabled,
                points: &lane.points,
            })
        }
        AutomationTarget::Plugin { track, param_id } => {
            if track != track_idx {
                return None;
            }
            let lane = self
                .plugin_automation
                .iter()
                .find(|l| l.track == track && l.param_id == param_id)?;
            Some(AutomationCurve {
                enabled: lane.enabled,
                points: &lane.points,
            })
        }
    }
}

/// Punkterna för målet — och **skapar lane:n om den inte finns**.
///
/// Det är första punkten som skapar kurvan, precis som för spårets rattar: en tom lane som
/// ligger och väntar är en lane som ser ut att finnas utan att göra något. Namnet skrivs in
/// med en gång för plugin-målet, medan pluginen är laddad och vet vad parametern heter.
pub(crate) fn automation_points_mut(
    &mut self,
    target: AutomationTarget,
    track_idx: usize,
) -> Option<&mut Vec<AutomationPoint>> {
    match target {
        AutomationTarget::Track(param) => {
            let track = self.playlist_tracks.get_mut(track_idx)?;
            if !track.automation.iter().any(|l| l.param == param) {
                track.automation.push(AutomationLane {
                    param,
                    enabled: true,
                    points: Vec::new(),
                });
            }
            track
                .automation
                .iter_mut()
                .find(|l| l.param == param)
                .map(|l| &mut l.points)
        }
        AutomationTarget::Plugin { track, param_id } => {
            if track != track_idx {
                return None;
            }
            if !self
                .plugin_automation
                .iter()
                .any(|l| l.track == track && l.param_id == param_id)
            {
                let param_name = self
                    .plugin_parameter(track, param_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                self.plugin_automation.push(PluginAutomationLane {
                    track,
                    param_id,
                    param_name,
                    enabled: true,
                    points: Vec::new(),
                    last_sent: None,
                });
            }
            self.plugin_automation
                .iter_mut()
                .find(|l| l.track == track && l.param_id == param_id)
                .map(|l| &mut l.points)
        }
    }
}

/// **Målets område** — och varför plugin-målet inte får ett påhittat 0..1.
///
/// Spårets rattar har sina egna kända områden (`AutomationParam::range`). En CLAP-parameter
/// mäter i sitt eget: en nivå kan gå i dB och en frekvens i Hz. Området läses därför ur
/// **pluginens egen deskriptor**. Är pluginen inte laddad finns inget område att läsa — då
/// ritas kurvan i 0..1, och värdet kläms först när pluginen svarar (se `apply_automation`).
pub(crate) fn automation_target_range(&self, target: AutomationTarget) -> (f32, f32) {
    match target {
        AutomationTarget::Track(param) => param.range(),
        AutomationTarget::Plugin { track, param_id } => self
            .plugin_parameter(track, param_id)
            .map(|p| (p.min_value as f32, p.max_value as f32))
            .filter(|(lo, hi)| lo.is_finite() && hi.is_finite() && hi > lo)
            .unwrap_or((0.0, 1.0)),
    }
}

/// Målets etikett i tidslinjen.
pub(crate) fn automation_target_label(&self, target: AutomationTarget) -> String {
    match target {
        AutomationTarget::Track(param) => param.label().to_string(),
        AutomationTarget::Plugin { track, param_id } => {
            let namn = self
                .plugin_parameter(track, param_id)
                .map(|p| p.name.clone())
                .filter(|n| !n.is_empty())
                // **Namnet sparades när kurvan skapades** — just för det här fallet. En
                // projektfil kan öppnas utan att pluginen är installerad, och en kurva som
                // bara visar ett id går inte att läsa.
                .unwrap_or_else(|| {
                    self.plugin_automation
                        .iter()
                        .find(|l| l.track == track && l.param_id == param_id)
                        .map(|l| l.param_name.clone())
                        .unwrap_or_default()
                });
            if namn.is_empty() {
                crate::tstatus!("🔌 plugin-parameter {}", param_id)
            } else {
                crate::tstatus!("🔌 {}", namn)
            }
        }
    }
}
}

/// Skriver masterns och plugin-kurvornas tillstånd tillbaka in i appens fält.
///
/// **Ren funktion** — ingen motor och ingen `self` — just för att reglerna i återställningen ska
/// gå att pröva utan en ljudenhet: matchningen på pedal-**id** (en omordning får inte flytta ett
/// värde till fel pedal) och nollningen av kurvcachen (`last_sent`), som är det som gör att en
/// ångrad plugin-kurva hörs igen. `apply_non_track_sound` är den tunna hylsan runt den som också
/// skickar till motorn.
// Lång signatur med flit: parametrarna är motorns/ritningens hela tillstånd, och en
// påhittad struct skulle bara flytta samma fält ett steg.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_non_track_sound(
    s: &NonTrackSound,
    plugin_automation: &mut Vec<PluginAutomationLane>,
    delay: &mut crate::audio::DelayParams,
    reverb: &mut crate::audio::ReverbParams,
    drive: &mut f32,
    master_volume: &mut f32,
    pedals: &mut [crate::ui::fx_rack_modal::FxPedal],
    eq_nodes: &mut Vec<crate::ui::fx_rack_modal::VisualEqNode>,
    compressor_release_ms: &mut f32,
) {
    *plugin_automation = s.plugin_automation.clone();
    for lane in plugin_automation.iter_mut() {
        lane.last_sent = None;
    }
    *delay = s.delay;
    *reverb = s.reverb;
    *drive = s.drive;
    *master_volume = s.master_volume;
    for pedal in pedals.iter_mut() {
        if let Some(sp) = s.pedals.iter().find(|sp| sp.id == pedal.id) {
            pedal.enabled = sp.enabled;
            pedal.p1_val = sp.p1_val;
            pedal.p2_val = sp.p2_val;
            pedal.p3_val = sp.p3_val;
        }
    }
    *eq_nodes = s.eq_nodes.clone();
    *compressor_release_ms = s.compressor_release_ms;
}

impl SonixApp {
/// Läser av det som hörs utanför spåren — se [`NonTrackSound`].
pub(crate) fn non_track_sound(&self) -> NonTrackSound {
    NonTrackSound {
        plugin_automation: self.plugin_automation.clone(),
        delay: self.delay,
        reverb: self.reverb,
        drive: self.drive,
        master_volume: self.master_volume,
        pedals: self
            .fx_rack_state
            .pedals
            .iter()
            .map(|p| PedalSound {
                id: p.id.to_string(),
                enabled: p.enabled,
                p1_val: p.p1_val,
                p2_val: p.p2_val,
                p3_val: p.p3_val,
            })
            .collect(),
        eq_nodes: self.fx_rack_state.eq_nodes.clone(),
        compressor_release_ms: self.fx_rack_state.compressor_release_ms,
    }
}

/// Skriver tillbaka det som hörs utanför spåren och **skickar det till motorn** (Fas 6.2).
///
/// Ordningen är avsiktlig: plugin-kurvornas `last_sent` nollas **först**, och `apply_automation`
/// (som kör varje bildruta) skickar då om varje värde nästa bildruta. Utan nollningen hade en
/// ångrad kurva stått rätt i gränssnittet medan pluginen spelade vidare med det gamla värdet.
/// Pedalerna matchas på **id**, inte på plats: en omordning får inte flytta ett värde till fel
/// pedal.
pub(crate) fn apply_non_track_sound(&mut self, s: &NonTrackSound) {
    write_non_track_sound(
        s,
        &mut self.plugin_automation,
        &mut self.delay,
        &mut self.reverb,
        &mut self.drive,
        &mut self.master_volume,
        &mut self.fx_rack_state.pedals,
        &mut self.fx_rack_state.eq_nodes,
        &mut self.fx_rack_state.compressor_release_ms,
    );
    self.sync_non_track_sound_to_engine();
}


/// Skickar masterns kedja till motorn — delay, reverb, drive, volym och master-FX.
pub(crate) fn sync_non_track_sound_to_engine(&mut self) {
    let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay));
    let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb));
    let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
    let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
    let _ = self
        .engine
        .send_command(AudioCommand::SetMasterFx(
            self.fx_rack_state.build_master_fx_params(),
        ));
}

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
    // **Plugin-insertarnas kurvor** (Fas 8.8). Samma form som bussen — ett pass utanför
    // spårloopen, för lane:n bär sitt eget spår — men med en skillnad som är verklig:
    // bussens nuvarande värde *är* UI-tillstånd, medan en plugin-parameters värde ägs av
    // motorn och inte går att läsa tillbaka här. Jämförelsen sker därför mot lane:ns egen
    // `last_sent`, annars hade varje bildruta under uppspelning skickat ett kommando.
    //
    // Ingen plugin laddad → ingen kurva att följa: värdet lämnas orört i stället för att
    // skickas till en insert som inte finns, och `last_sent` förblir `None` så att första
    // värdet går fram när pluginen väl svarar.
    for li in 0..self.plugin_automation.len() {
        let (track, param_id, value, changed) = {
            let lane = &self.plugin_automation[li];
            if !lane.enabled {
                continue;
            }
            let Some(param) = self.plugin_parameter(lane.track, lane.param_id) else {
                continue;
            };
            let Some(raw) = lane.value_at(bar) else { continue };
            // **Området kommer från parametern**, inte från ett påhittat 0..1: en parameter
            // som mäter i dB eller Hz hade annars fått ett värde ur fel skala.
            let value = PluginAutomationLane::clamp_to_range(raw, (param.min_value, param.max_value));
            let changed = lane
                .last_sent
                .is_none_or(|last| (value - last).abs() > 1e-4);
            (lane.track, lane.param_id, value, changed)
        };
        if !changed {
            continue;
        }
        self.plugin_automation[li].last_sent = Some(value);
        let _ = self.engine.send_command(AudioCommand::SetPluginParameter {
            track_index: track,
            param_id,
            value,
        });
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
    // **Målet, inte bara spårets rattar** (Fas 8.8): kurvan kan höra till spårets egen ratt
    // eller till en parameter hos spårets plugin. Området kommer från målet — en plugin-
    // parameter mäter i sitt eget (dB, Hz), inte i 0..1.
    let target = self.automation_target;
    let (lo, hi) = self.automation_target_range(target);
    let label = self.automation_target_label(target);
    let span = (hi - lo).max(1e-6);
    let val_to_y = |v: f32| rect.max.y - ((v - lo) / span).clamp(0.0, 1.0) * rect.height();
    let y_to_val = |y: f32| lo + ((rect.max.y - y) / rect.height()).clamp(0.0, 1.0) * span;
    // Punkterna ligger i **takter**, så ritningen behöver ingen omräkning: x är takten
    // gånger taktbredden. Före Fas 8.2 gick den här vägen via sekunder och kartan.
    let bar_to_x = |b: f32| rect.min.x + b * bar_w;

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 10.0),
        egui::Align2::LEFT_CENTER,
        crate::tstatus!("📈 {} — {}", label, self.playlist_tracks[track_idx].name),
        egui::FontId::proportional(Theme::FONT_S),
        crate::ui::theme::palette().cyan(),
    );
    painter.line_segment(
        [
            Pos2::new(rect.min.x, val_to_y((lo + hi) * 0.5)),
            Pos2::new(rect.max.x, val_to_y((lo + hi) * 0.5)),
        ],
        Stroke::new(0.5_f32, crate::ui::theme::palette().grid()),
    );
    let total_bars = (rect.width() / bar_w).ceil() as usize;
    for b in 0..=total_bars {
        let x = rect.min.x + b as f32 * bar_w;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(0.5_f32, crate::ui::theme::palette().panel()),
        );
    }

    let curve = self.automation_curve(target, track_idx);

    if let Some(curve) = curve
        && curve.enabled && !curve.points.is_empty() {
            let first = &curve.points[0];
            let last = curve.points.last().unwrap();
            painter.line_segment(
                [
                    Pos2::new(rect.min.x, val_to_y(first.value)),
                    Pos2::new(bar_to_x(first.time_bars), val_to_y(first.value)),
                ],
                Stroke::new(2.0_f32, crate::ui::theme::palette().cyan()),
            );
            for w in curve.points.windows(2) {
                painter.line_segment(
                    [
                        Pos2::new(bar_to_x(w[0].time_bars), val_to_y(w[0].value)),
                        Pos2::new(bar_to_x(w[1].time_bars), val_to_y(w[1].value)),
                    ],
                    Stroke::new(2.0_f32, crate::ui::theme::palette().cyan()),
                );
            }
            painter.line_segment(
                [
                    Pos2::new(bar_to_x(last.time_bars), val_to_y(last.value)),
                    Pos2::new(rect.max.x, val_to_y(last.value)),
                ],
                Stroke::new(2.0_f32, crate::ui::theme::palette().cyan()),
            );
            for (pi, p) in curve.points.iter().enumerate() {
                let c = Pos2::new(bar_to_x(p.time_bars), val_to_y(p.value));
                let dragging = self.automation_drag == Some((track_idx, target, pi));
                painter.circle_filled(
                    c,
                    if dragging { 6.0 } else { 4.5 },
                    if dragging { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().cyan() },
                );
                painter.circle_stroke(c, 5.5, Stroke::new(1.0_f32, Color32::BLACK));
            }
        }

    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos() {
            // **Snäppet sker i takter** (Fas 8.2): ett sextondelssteg är en plats i
            // takten, inte ett antal sekunder, och sekunder-per-takt är inte ett tal över
            // ett tempobyte. Sekunden hämtas ur kartan **en gång efteråt**.
            let raw_bar = ((pos.x - rect.min.x) / bar_w).max(0.0);
            let bar = snap_bar(self.timeline_snap_mode, raw_bar);
            let val = y_to_val(pos.y).clamp(lo, hi);
            if let Some(points) = self.automation_points_mut(target, track_idx) {
                points.push(AutomationPoint { time_bars: bar, value: val });
                sort_automation_points(points);
                self.status_message = crate::tstatus!("📈 Automation: la till punkt (takt {:.2}, {:.2})", bar, val);
            }
        }

    if resp.drag_started()
        && let Some(pos) = resp.interact_pointer_pos() {
            // Träffytan läses ur kurvan som **ritades**, samma uppslag som ritningen — två
            // egna uppslag hade kunnat peka på var sin lane.
            let pts: Option<&[AutomationPoint]> =
                self.automation_curve(target, track_idx).map(|c| c.points);
            if let Some(pts) = pts {
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
                    self.automation_drag = Some((track_idx, target, pi));
                }
            }
        }
    if resp.dragged()
        && let Some((t, mål, pi)) = self.automation_drag
            && t == track_idx
            && mål == target
            && let Some(pos) = resp.interact_pointer_pos()
        {
            // **Snäppet sker i takter** (Fas 8.2): ett sextondelssteg är en plats i
            // takten, inte ett antal sekunder, och sekunder-per-takt är inte ett tal över
            // ett tempobyte. Sekunden hämtas ur kartan **en gång efteråt**.
            let raw_bar = ((pos.x - rect.min.x) / bar_w).max(0.0);
            let bar = snap_bar(self.timeline_snap_mode, raw_bar);
            let val = y_to_val(pos.y).clamp(lo, hi);
            if let Some(p) = self
                .automation_points_mut(target, track_idx)
                .and_then(|pts| pts.get_mut(pi))
            {
                p.time_bars = bar;
                p.value = val;
            }
        }
    if resp.drag_stopped() {
        if let Some((t, mål, _)) = self.automation_drag
            && t == track_idx
                && mål == target
                && let Some(points) = self.automation_points_mut(target, track_idx)
            {
                sort_automation_points(points);
            }
        self.automation_drag = None;
    }

    if resp.secondary_clicked()
        && let Some(pos) = resp.interact_pointer_pos() {
            let pts: Option<&[AutomationPoint]> =
                self.automation_curve(target, track_idx).map(|c| c.points);
            if let Some(pts) = pts {
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
                if let Some(pi) = best
                    && let Some(points) = self.automation_points_mut(target, track_idx)
                        && pi < points.len()
                    {
                        points.remove(pi);
                        self.status_message = crate::tstatus!("📈 Automation: tog bort punkt");
                    }
            }
        }
}
}

