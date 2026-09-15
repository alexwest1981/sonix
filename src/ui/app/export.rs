//! Exporten och frysningen.
//!
//! Exporten måste ge **samma ljud som högtalarna** — samma väg som uppspelningen, inte en
//! parallell. \`build_render_spec\` bygger specen och \`render_buffer\`/offline-renderingen kör
//! den; frysningen (\`freeze_track\`) använder samma väg och nycklas på \`frozen_digest\`.
//!
//! Status: stabil — offline-renderingen och exporten.
//! Rör inte: samma väg som uppspelningen; en parallell väg hörs som en annan låt.

use super::*;

impl SonixApp {
/// Lägger de separerade stämmorna som riktiga tidslinjespår.
///
/// **Stämmorna skrivs till disk först.** Klippet får inte skapas utan sitt
/// ljud — samma regel som `import_audio_file_as_track` och de övriga
/// 8.5-vägarna. Förut pekade varje stämregion på **originalfilen** medan
/// vågformen kom från stämman i minnet. Var originalet en mp3, som appen inte
/// kan avkoda, blev klippet tyst men ritades som om det hade ljud — och
/// tystnaden kom tillbaka varje gång projektet öppnades igen. Nu skrivs varje
/// stämma som en 32-bitars WAV i projektets egen materialmapp, **läses
/// tillbaka** (så att en trasig skrivning upptäcks här i stället för i en tyst
/// uppspelning), och regionens `source_path` och vågform kommer båda ur den
/// filen.
pub fn export_separated_stems(&mut self) {
    if self.stem_project.stem_audio.is_empty() {
        self.status_message = crate::i18n::t("⚠ Ingen separerad mix att exportera.").to_string();
        return;
    }
    // Antalet punkter den grova översikten ritades med förut (512).
    const WAVEFORM_POINTS: usize = 512;
    let dir = crate::paths::paths()
        .project_assets_dir(&self.project_name)
        .join("Stems");
    let base = stem_base_name(
        self.stem_project.source_path.as_deref(),
        &self.stem_project.track_title,
    );
    let paths = match crate::audio::stem_separator::write_stems_and_read_envelopes(
        &dir,
        &base,
        &self.stem_project.stem_audio,
        self.stem_project.sample_rate,
        WAVEFORM_POINTS,
    ) {
        Ok(written) => written,
        Err(e) => {
            self.status_message = crate::tstatus!(
                "⚠ Kunde inte skriva stämmorna till disk: {} — inga klipp skapas",
                e
            );
            return;
        }
    };
    // Vågformen kommer ur filerna, i samma ordning som stämmorna. Det är
    // filen som är sanningen om vad klippet kommer att spela — inte den
    // grova översikt som räckte förut.
    let mut envelopes: Vec<Vec<f32>> = Vec::with_capacity(paths.len());
    let mut files: Vec<String> = Vec::with_capacity(paths.len());
    for (path, envelope) in paths {
        files.push(path.to_string_lossy().into_owned());
        envelopes.push(envelope);
    }

    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let bars =
        (tempo.bars_for_secs_at(0.0, self.stem_project.duration_seconds as f64) as f32)
            .max(1.0);
    let kinds = [TrackKind::VocalAudio, TrackKind::Drums, TrackKind::Bassline, TrackKind::CustomAudio];
    let mut new_tracks = Vec::new();
    // Stämman, kanalens inställningar och filen hör ihop tre och tre — gå
    // aldrig utanför någon av listorna.
    let count = self
        .stem_project
        .stems
        .len()
        .min(self.stem_project.stem_audio.len())
        .min(files.len());
    for i in 0..count {
        let ch = &self.stem_project.stems[i];
        let audio = &self.stem_project.stem_audio[i];
        let region = AudioRegion {
            source_bpm: self.bpm, // stämmorna byggdes mot projektets tempo (8.10)
            tape: false,
            id: i + 1,
            name: ch.stem_type.name().to_string(),
            start_bar: 0.0,
            length_bars: bars,
            sample_offset_sec: 0.0,
            source_path: Some(files[i].clone()),
            waveform_peaks: envelopes[i].clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: ch.stem_type.color(),
            loop_length_bars: 0.0,
        };
        let mut track = PlaylistTrack::new(ch.stem_type.name().to_string(), "🎚", kinds[i], ch.stem_type.color());
        track.volume = ch.volume;
        track.pan = ch.pan;
        track.muted = ch.muted;
        track.solo = ch.solo;
        track.regions = vec![region];
        track.pcm_audio = Some((
            std::sync::Arc::new(audio.left.clone()),
            std::sync::Arc::new(audio.right.clone()),
            self.stem_project.sample_rate,
        ));
        new_tracks.push(track);
    }
    let added = new_tracks.len();
    self.playlist_tracks.extend(new_tracks);
    self.sync_all_stems_to_engine();
    self.status_message = crate::tstatus!(
        "📥 Lade in {} stämspår i arrangeraren (som wav-filer i {})",
        added,
        dir.display()
    );
}
}

impl SonixApp {
pub fn export_wav(&mut self) {
    // Snabbexport öppnar exportdialogen (fullt projekt: format, mapp &
    // ren metadata – ingen hårdkodad sökväg kvar).
    self.open_export_modal();
}
}

impl SonixApp {
pub fn open_export_modal(&mut self) {
    if self.export_base_name.trim().is_empty()
        || self.export_base_name == crate::i18n::t("Min_Låt")
        || self.export_metadata.title.is_empty() {
        let safe = crate::audio::sanitize_filename(&self.project_name);
        if !safe.is_empty() {
            self.export_base_name = safe;
        }
        if self.export_metadata.title.is_empty() {
            self.export_metadata.title = self.project_name.clone();
        }
    }
    self.show_render_queue_modal = true;
    self.render_queue_status = crate::i18n::t("Klar för rendering").to_string();
}
}

impl SonixApp {
fn export_bars(&self) -> usize {
    self.loop_end_bar.clamp(4, 32)
}
}

impl SonixApp {
/// Dither-inställningarna från exportdialogen (Fas 6.5).
fn export_dither_settings(&self) -> crate::audio::DitherSettings {
    crate::audio::DitherSettings {
        enabled: self.export_dither,
        noise_shaping: self.export_noise_shaping,
        seed: crate::audio::dither::DEFAULT_SEED,
    }
}
}

impl SonixApp {
pub(crate) fn export_sample_rate(&self) -> u32 {
    match self.render_sample_rate_idx {
        1 => 48000,
        2 => 96000,
        _ => 44100,
    }
}
}

impl SonixApp {
/// Snapshot the current project (real Channel Rack samples, patterns,
/// timeline stems) into the pure renderer data model.
pub(crate) fn build_render_spec(&self, solo_track: Option<usize>, sample_rate: u32) -> crate::audio::RenderSpec {
    use crate::audio::{PatternSnap, RackChannel, TrackAudioSnap, TrackRole, TrackSnap, VoiceSpec};

    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let pattern_mode = self.pattern_mode;

    let rack = self.channels.iter().map(|ch| {
        let voice = ch.pcm_audio.as_ref().map(|(l, r, sr)| VoiceSpec {
            left: l.clone(),
            right: r.clone(),
            sample_rate: *sr,
            base_note: ch.sample_base_note,
            semitones: ch.pitch_semitones,
            cents: ch.pitch_fine_cents,
            volume: ch.volume,
            reverse: ch.is_reverse,
            start: ch.sample_start,
            end: ch.sample_end,
            // Samplern (Fas 8.4) följer med till filen — samma ljud i exporten
            // som i högtalarna, samma krav som för sidokedjan och sendarna.
            loop_mode: ch.loop_mode,
            loop_start: ch.sample_loop_start,
            loop_end: ch.sample_loop_end,
            ping_pong: ch.ping_pong,
            amp_env: ch.amp_env,
            velocity_sensitivity: ch.velocity_sensitivity,
            // **Filtret följer med till filen** (Fas 8.4/7) — annars hade exporten haft en
            // annan klang än högtalarna.
            filter: ch.filter,
            // **Och keymappen**: samma zoner, samma väljare, samma svar som spelvägen.
            zones: ch.zones.clone(),
        });
        RackChannel {
            voice,
            slices: ch.slices.clone(),
            fallback_volume: ch.volume,
            steps: ch.steps,
            notes: ch.notes,
        }
    }).collect();

    let patterns = self.patterns.iter().map(|p| PatternSnap {
        steps: p.channel_steps.clone(),
        notes: p.channel_notes.clone(),
        piano_roll: p.piano_roll_grid,
        take: p.take.clone(),
    }).collect();

    let tracks = self.playlist_tracks.iter().map(|t| {
        let role = match t.kind {
            TrackKind::Drums => TrackRole::Drums,
            TrackKind::SynthLead => TrackRole::Synth,
            TrackKind::Bassline => TrackRole::Bass,
            TrackKind::VocalAudio | TrackKind::CustomAudio | TrackKind::Fx => TrackRole::Audio,
        };
        // Ett fruset spår bidrar med sitt LJUd i stället för sina patterns
        // (se render_clips_for); ljudet ligger i tidslinjen nedan.
        let clips = render_clips_for(t);
        TrackSnap {
            role,
            clips,
            volume: t.volume,
            muted: t.muted,
            solo: t.solo,
        }
    }).collect();

    let timeline = self.playlist_tracks.iter().enumerate().filter_map(|(idx, t)| {
        if !frozen_audio_in_render(t, pattern_mode) {
            return None;
        }
        let (l, r, sr) = t.frozen_pcm.as_ref().or(t.pcm_audio.as_ref())?;
        // Samma väg som uppspelningen använder — ett ställe för
        // omräkningen takter→sekunder, i stället för ett här och ett där.
        let regions = self.stem_regions_for(t);
        // Sends hör till ljudspårens väg. Pattern-vägen som frysningen
        // ersätter har inga, så de nollas för det frusna spåret — annars
        // skulle det plötsligt få klang som live-uppspelningen inte hade.
        let (reverb_send, delay_send) = if t.frozen_pcm.is_some() {
            (0.0, 0.0)
        } else {
            (t.reverb_send, t.delay_send)
        };
        // Sends (Fas 8.13 bussar, Fas 8.3 spår) hör till samma väg: ett fruset spår
        // spelar sin färdigrenderade fil, och den renderades utan sends — alltså faller
        // båda slagen bort. Att *ta emot* en send är däremot som förut: mottagarens
        // kedja är densamma, och senden går in i den precis som sitt eget ljud.
        let sends = if t.frozen_pcm.is_some() {
            Vec::new()
        } else {
            t.sends.clone()
        };
        Some(TrackAudioSnap {
            track_index: idx,
            left: l.clone(),
            right: r.clone(),
            sample_rate: *sr,
            volume: t.volume,
            pan: t.pan,
            muted: t.muted,
            regions,
            eq: t.eq.to_settings(),
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send,
            delay_send,
            pitch_semitones: t.pitch_semitones,
            bus: t.bus,
            vca: t.vca,
            sidechain_from: t.sidechain_from,
            sidechain_amount_db: t.sidechain_amount_db,
            sidechain_threshold_db: t.sidechain_threshold_db,
            sends,
        })
    }).collect();

    crate::audio::RenderSpec {
        sample_rate,
        bpm: self.bpm,
        swing: self.swing,
        num_bars: self.export_bars(),
        pattern_mode,
        rack,
        patterns,
        tracks,
        timeline,
        velocities: self.step_velocities,
        solo_track,
        tail_secs: (tempo.secs_per_beat_at(0.0) as f32).min(2.0),
        bus_volume: self.bus_volume,
        bus_muted: self.bus_muted,
        bus_solo: self.bus_solo,
        vca_volume: self.vca_faders,
        vca_muted: self.vca_muted,
        vca_solo: self.vca_solos,
    }
}
}

impl SonixApp {
fn export_fx_state(&self, dry: bool) -> crate::audio::FxState {
    let mut delay = self.delay;
    let mut reverb = self.reverb;
    if dry {
        delay.mix = 0.0;
        reverb.mix = 0.0;
    }
    crate::audio::FxState {
        waveform: self.waveform,
        adsr: self.adsr,
        filter: self.filter,
        delay,
        reverb,
        drive: if dry { 1.0 } else { self.drive },
        master_volume: self.master_volume,
        master_fx: if dry {
            crate::audio::MasterFxParams::default()
        } else {
            self.fx_rack_state.build_master_fx_params()
        },
    }
}
}

impl SonixApp {
/// Render one spec to an interleaved float buffer.
pub(crate) fn render_buffer(&self, spec: &crate::audio::RenderSpec, dry: bool) -> Result<Vec<f32>, String> {
    let mut engine = crate::audio::build_offline_engine(spec, &self.export_fx_state(dry));
    Ok(crate::audio::render_project_offline(&mut engine, spec))
}
}

impl SonixApp {
/// Peak level, used to skip completely empty stem files.
fn peak_of(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, &s| m.max(s.abs()))
}
}

impl SonixApp {
/// Fingeravtryck av allt som påverkar hur ett spår låter i en offline-render.
///
/// Det är en **varning** om något ändrats, inte ett bevis på att ljudet är
/// identiskt: fingeravtrycket ser att något skiljer sig, det avgör inte vad
/// som är rätt. Mastern och bussarna är medvetet inte med — frysningen
/// renderas torr och mastern gäller live även efteråt — och en ändring av ett
/// *annat* spår spelar ingen roll, eftersom renderingen är solad.
fn frozen_digest(&self, t_idx: usize) -> u64 {
    use std::fmt::Write as _;
    let mut text = String::new();
    let _ = write!(text, "bpm={:.4};swing={:.4};", self.bpm, self.swing);
    if let Some(t) = self.playlist_tracks.get(t_idx) {
        let _ = write!(
            text,
            "spår={:?}|{:.4},{:.4},{};",
            t.clips, t.volume, t.pan, t.muted
        );
        // Mönstren klippen pekar på. Andra spår kan peka på samma mönster —
        // då är de frusna på samma innehåll, vilket är riktigt.
        for pat_idx in t.clips.iter().flatten() {
            if let Some(p) = self.patterns.get(*pat_idx) {
                let _ = write!(text, "mönster{:?};", p);
            }
        }
    }
    for ch in self.channels.iter() {
        let _ = write!(
            text,
            "kanal={:?},{:.4},{:.4},{:.4},{:.4},{},{};",
            ch.sample_path,
            ch.volume,
            ch.sample_start,
            ch.sample_end,
            ch.pitch_semitones,
            ch.sample_base_note,
            ch.is_reverse
        );
    }
    let _ = write!(text, "röst={:?},{:?},{:?};", self.waveform, self.adsr, self.filter);
    crate::autosave::fingerprint(text.as_bytes())
}
}

impl SonixApp {
/// Har spåret ändrats sedan det frystes? Då spelar det frusna ljudet inte
/// längre det som står i projektet, och det sägs i stället för att tigas.
pub fn frozen_is_stale(&self, t_idx: usize) -> bool {
    self.playlist_tracks
        .get(t_idx)
        .and_then(|t| t.frozen.as_ref())
        .is_some_and(|f| f.digest != self.frozen_digest(t_idx))
}
}

impl SonixApp {
/// Var ett fruset spår ligger: projektets egen materialmapp, samma plats som
/// inspelningar och stems. Namnet är deterministiskt, så en ny frysning
/// skriver över sin egen fil i stället för att lämna skräp.
pub fn frozen_path(&self, t_idx: usize, name: &str) -> String {
    crate::paths::paths()
        .project_assets_dir(&self.project_name)
        .join("Frozen")
        .join(format!("{}-{}.wav", crate::autosave::slug(name), t_idx))
        .to_string_lossy()
        .into_owned()
}
}

impl SonixApp {
/// Fryser ett spår: renderar det offline och lägger in ljudet som ett
/// stem-spår, så att uppspelningen slipper köra syntesen för det.
///
/// Renderingen går genom **samma väg som exporten** (torr, solad på spåret),
/// och spårets fader lämnas utanför — den ska fortsätta gälla live, som på
/// ett ofruset spår.
pub fn freeze_track(&mut self, t_idx: usize) {
    let Some(track) = self.playlist_tracks.get(t_idx) else {
        return;
    };
    if !track.can_freeze() {
        self.status_message = crate::i18n::t(
            "❄ Spåret kan inte frysas — det är redan fruset, eller ett ljudspår som redan ligger som ljud",
        )
        .to_string();
        return;
    }
    if self.plugin_slots.get(t_idx).is_some_and(|s| s.is_some()) {
        self.status_message = crate::i18n::t(
            "❄ Spåret har en plugin-insert, och offline-renderingen kan inte återskapa den — ta bort den först",
        )
        .to_string();
        return;
    }
    let name = track.name.clone();
    let (volume, pan) = (track.volume, track.pan);
    let sample_rate = self.engine.sample_rate;

    // Sola på spåret och ta bort dess fader ur specen: det som renderas är
    // spårets eget ljud, inte dess placering i mixen.
    let mut spec = self.build_render_spec(Some(t_idx), sample_rate);
    if let Some(t) = spec.tracks.get_mut(t_idx) {
        t.volume = 1.0;
    }
    let buffer = match self.render_buffer(&spec, true) {
        Ok(b) => b,
        Err(e) => {
            self.status_message =
                crate::tstatus!("❄ Kunde inte rendera spåret: {}", e);
            return;
        }
    };
    if buffer.is_empty() || Self::peak_of(&buffer) <= 0.0 {
        self.status_message = crate::tstatus!(
            "❄ '{}' är tyst i den här låten — det finns inget att frysa",
            name
        );
        return;
    }

    let path = self.frozen_path(t_idx, &name);
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // 32-bitars flyttal: en frysning är ett mellansteg och ska inte kvantisera
    // en enda gång innan den riktiga exporten gör det.
    let meta = crate::audio::ExportMeta {
        title: name.clone(),
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        year: String::new(),
        comment: "Fruset spår (mellansteg i Sonix)".to_string(),
    };
    if let Err(e) = crate::audio::write_export_with(
        &path,
        crate::audio::ExportFormat::Wav32,
        &buffer,
        sample_rate,
        &meta,
        crate::audio::DitherSettings::default(),
    ) {
        self.status_message =
            crate::tstatus!("❄ Kunde inte skriva den frusna filen: {}", e);
        return;
    }
    // Läs tillbaka från filen i stället för att behålla bufferten: då är det
    // filen som är sanningen, och en trasig skrivning upptäcks nu.
    let Some((l, r, pcm_sr)) = load_sample_pcm_arcs(&path) else {
        self.status_message = crate::tstatus!(
            "❄ Den frusna filen gick inte att läsa tillbaka: {}",
            path
        );
        return;
    };

    self.push_undo(&format!("Frys '{}'", name));
    let len_secs = l.len() as f32 / pcm_sr as f32;
    let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
        track_index: t_idx,
        left: l.clone(),
        right: r.clone(),
        sample_rate: pcm_sr as f32,
        volume,
        pan,
        start_time_secs: 0.0,
    });
    let digest = self.frozen_digest(t_idx);
    if let Some(t) = self.playlist_tracks.get_mut(t_idx) {
        t.frozen = Some(FrozenTrack {
            path: path.clone(),
            digest,
            stamp: crate::autosave::now_stamp(),
        });
        t.frozen_pcm = Some((l, r, pcm_sr));
    }
    self.sync_track_regions(t_idx);
    self.status_message = crate::tstatus!(
        "❄ Frös '{}': {:.1} s ljud, och pattern-uppspelningen hoppas över [Ångra: Ctrl+Z]",
        name,
        len_secs
    );
}
}

impl SonixApp {
/// Tina upp ett spår: pattern-uppspelningen tar över igen.
///
/// Den frusna filen lämnas kvar i projektets mapp — den är användarens
/// material, och en ny frysning skriver över samma namn.
pub fn unfreeze_track(&mut self, t_idx: usize) {
    let Some(track) = self.playlist_tracks.get(t_idx) else {
        return;
    };
    if !track.is_frozen() {
        return;
    }
    let name = track.name.clone();
    let (volume, pan) = (track.volume, track.pan);
    self.push_undo(&format!("Tina '{}'", name));
    // Att lasta ett stem med TOM ljudbuffert tömmer platsen i motorn: den
    // behåller eq/comp/plugin och det positionella indexet, vilket en
    // borttagen post inte skulle göra.
    let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
        track_index: t_idx,
        left: std::sync::Arc::new(Vec::new()),
        right: std::sync::Arc::new(Vec::new()),
        sample_rate: self.engine.sample_rate as f32,
        volume,
        pan,
        start_time_secs: 0.0,
    });
    if let Some(t) = self.playlist_tracks.get_mut(t_idx) {
        t.frozen = None;
        t.frozen_pcm = None;
    }
    self.sync_track_regions(t_idx);
    self.status_message = crate::tstatus!(
        "🔥 '{}' är upptinat — pattern-uppspelningen är tillbaka [Ångra: Ctrl+Z]",
        name
    );
}
}

impl SonixApp {
pub(crate) fn render_batch_export_modal(&mut self, ctx: &egui::Context) {
    if !self.show_render_queue_modal {
        return;
    }

    let mut close = false;
    egui::Window::new(self.tr("📤 Exportera projekt"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(620.0);
            ui.label(egui::RichText::new(self.tr("Rendera hela projektet offline (riktiga WAV-samples, tidslinje-audio & effekter) med ren metadata – ingen AI- eller leverantörsinformation läggs någonsin till.")).size(11.0).color(Theme::TEXT_MUTED));
            ui.add_space(8.0);

            // 1. Scope
            ui.group(|ui| {
                ui.label(egui::RichText::new(self.tr("1. VAD SKALL EXPORTERAS")).strong().size(11.5).color(Theme::FL_ORANGE));
                ui.separator();
                let scopes: [(&'static str, &'static str); 3] = [
                    (crate::i18n::t("Hel låt – Master Mix (Fullt Projekt)"), crate::i18n::t("Renderar hela låten/arrangemanget med alla Channel Rack-samples, tidslinje-audio (stems/mic) och master-effekter.")),
                    (crate::i18n::t("Individuella spår – Torra (Stems Dry)"), crate::i18n::t("Exporterar varje tidslinjespår för sig utan reverb/delay för extern mixning.")),
                    (crate::i18n::t("Individuella spår – Med FX (Stems Wet)"), crate::i18n::t("Exporterar varje tidslinjespår för sig med alla effekter och modulation.")),
                ];
                for (i, (title, desc)) in scopes.iter().enumerate() {
                    if ui.selectable_label(self.render_scope_idx == i, format!("⦿ {}", self.tr(title))).clicked() {
                        self.render_scope_idx = i;
                    }
                    ui.label(egui::RichText::new(format!("    {}", self.tr(desc))).size(9.5).color(Theme::TEXT_MUTED));
                }
            });

            ui.add_space(6.0);

            // 2. Format & quality
            ui.group(|ui| {
                ui.label(egui::RichText::new(self.tr("2. FORMAT & LJUDKVALITET")).strong().size(11.5).color(Theme::FL_CYAN));
                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    ui.label(self.tr("Format:"));
                    for (f_i, fname) in Self::EXPORT_FORMATS.iter().enumerate() {
                        let label = crate::i18n::t(fname.label());
                        if ui.selectable_label(self.render_format_idx == f_i, label).clicked() {
                            self.render_format_idx = f_i;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(self.tr("Samplingsfrekvens:"));
                    let rates = ["44.1 kHz (CD Standard)", "48.0 kHz (Film & Video)", "96.0 kHz (Hi-Res Studio)"];
                    for (r_i, rname) in rates.iter().enumerate() {
                        if ui.selectable_label(self.render_sample_rate_idx == r_i, *rname).clicked() {
                            self.render_sample_rate_idx = r_i;
                        }
                    }
                });

                // Dither (Fas 6.5): gäller 16-bitars WAV, där kvantiseringen
                // annars lägger felet som distorsion i stället för brus.
                // Etiketterna läses först: `&mut self.fält` och `self.tr(...)` får
                // inte samsas i samma anrop.
                let dither_label = self.tr("Dither (TPDF, 16-bitars WAV)");
                let shaping_label = self.tr("Noise shaping");
                let dither_hint = crate::i18n::t("Gör kvantiseringsfelet okorrelerat med materialet: jämnt brusgolv i stället för distorsion på svaga partier.");
                let shaping_hint = crate::i18n::t("Flyttar bruset uppåt i frekvens, där örat hör det sämre. Lägger mer energi i diskanten.");
                let dither_on = self.export_dither;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.export_dither, dither_label)
                        .on_hover_text(dither_hint);
                    ui.add_enabled_ui(dither_on, |ui| {
                        ui.checkbox(&mut self.export_noise_shaping, shaping_label)
                            .on_hover_text(shaping_hint);
                    });
                });
            });

            ui.add_space(6.0);

            // 3. Filnamn & mapp
            ui.group(|ui| {
                ui.label(egui::RichText::new(self.tr("3. FILNAMN & MAPPA")).strong().size(11.5).color(Theme::FL_YELLOW));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(self.tr("Låt-/filnamn:"));
                    ui.text_edit_singleline(&mut self.export_base_name);
                });
                ui.horizontal(|ui| {
                    ui.label(self.tr("Mapp:"));
                    ui.add(egui::TextEdit::singleline(&mut self.export_folder).desired_width(400.0));
                });
                let fmt = Self::EXPORT_FORMATS[self.render_format_idx.min(Self::EXPORT_FORMATS.len() - 1)];
                ui.label(egui::RichText::new(format!("{} {}_{:.0}bpm.{}", self.tr("Exempel:"), self.export_base_name, self.bpm, fmt.ext())).size(10.0).color(Theme::FL_GREEN));
            });

            ui.add_space(6.0);

            // 4. Ren metadata
            ui.group(|ui| {
                ui.label(egui::RichText::new(self.tr("4. METADATA (endast Sonix – ingen AI-info)")).strong().size(11.5).color(Theme::FL_PURPLE));
                ui.separator();
                let l_title = self.tr("Titel:");
                let l_artist = self.tr("Artist:");
                let l_album = self.tr("Album:");
                let l_genre = self.tr("Genre:");
                let l_year = self.tr("År:");
                let l_comment = self.tr("Kommentar:");
                let meta = &mut self.export_metadata;
                ui.horizontal(|ui| {
                    ui.label(l_title);
                    ui.add(egui::TextEdit::singleline(&mut meta.title).desired_width(200.0));
                    ui.label(l_artist);
                    ui.add(egui::TextEdit::singleline(&mut meta.artist).desired_width(160.0));
                });
                ui.horizontal(|ui| {
                    ui.label(l_album);
                    ui.add(egui::TextEdit::singleline(&mut meta.album).desired_width(200.0));
                    ui.label(l_genre);
                    ui.add(egui::TextEdit::singleline(&mut meta.genre).desired_width(160.0));
                });
                ui.horizontal(|ui| {
                    ui.label(l_year);
                    ui.add(egui::TextEdit::singleline(&mut meta.year).desired_width(80.0));
                    ui.label(l_comment);
                    ui.add(egui::TextEdit::singleline(&mut meta.comment).desired_width(340.0));
                });
                ui.label(egui::RichText::new(self.tr("Software-markören sätts alltid till Sonix Studio. Fält lämnas tomma om du vill utelämna dem.")).size(9.5).color(Theme::TEXT_MUTED));
            });

            ui.add_space(6.0);

            // 5. Export preset & loudness normalization (EBU R128)
            ui.group(|ui| {
                ui.label(egui::RichText::new(self.tr("5. EXPORT-PRESET & LOUDNESS (EBU R128)")).strong().size(11.5).color(Theme::FL_GREEN));
                ui.separator();

                let presets: [(&str, usize, usize, usize); 5] = [
                    ("Streaming (WAV 24/48, −14 LUFS)", 1, 1, 1),
                    ("Apple Music (WAV 24/48, −16 LUFS)", 1, 1, 2),
                    ("Broadcast EBU R128 (WAV 24/48, −23 LUFS)", 1, 1, 3),
                    ("Klubb/Loud (WAV 24/44.1, −9 LUFS)", 1, 0, 4),
                    ("FLAC Master (24-bit, −14 LUFS)", 3, 1, 1),
                ];
                ui.horizontal_wrapped(|ui| {
                    ui.label(self.tr("Preset:"));
                    if ui.selectable_label(self.export_preset_idx == 0, self.tr("Anpassad")).clicked() {
                        self.export_preset_idx = 0;
                    }
                    for (i, (label, f, s, l)) in presets.iter().enumerate() {
                        if ui.selectable_label(self.export_preset_idx == i + 1, self.tr(label)).clicked() {
                            self.export_preset_idx = i + 1;
                            self.render_format_idx = *f;
                            self.render_sample_rate_idx = *s;
                            self.export_loudness_idx = *l;
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label(self.tr("Loudness-normalisering:"));
                    for (i, p) in crate::audio::LoudnessPreset::ALL.iter().enumerate() {
                        if ui.selectable_label(self.export_loudness_idx == i, self.tr(p.label())).clicked() {
                            self.export_loudness_idx = i;
                            self.export_preset_idx = 0;
                        }
                    }
                });
                let active = crate::audio::LoudnessPreset::ALL[self.export_loudness_idx.min(crate::audio::LoudnessPreset::ALL.len() - 1)];
                if active.enabled() {
                    ui.label(egui::RichText::new(crate::tstatus!("Mäts med K-viktning och grindas (BS.1770), normaliseras till {} LUFS med tak på {} dBTP (äkta peak, 4× oversampling). Gäller master-mixen; stems lämnas orörda för extern mixning.", active.target_lufs(), active.ceiling_dbtp())).size(9.5).color(Theme::FL_GREEN));
                } else {
                    ui.label(egui::RichText::new(self.tr("Ingen normalisering – exporten sker med exakt den nivå du hör.")).size(9.5).color(Theme::TEXT_MUTED));
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(self.tr("Status:")).strong());
                    ui.label(egui::RichText::new(&self.render_queue_status).color(Theme::FL_CYAN));
                });
                if self.is_rendering {
                    ui.add(egui::ProgressBar::new(self.render_progress).show_percentage());
                }
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(self.tr("🚀 STARTA EXPORT")).strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(38, 120, 90))).clicked() {
                    self.execute_batch_export();
                }
                if ui.button(self.tr("Stäng")).clicked() {
                    close = true;
                }
            });
        });

    if close {
        self.show_render_queue_modal = false;
    }
}
}

impl SonixApp {
pub fn execute_batch_export(&mut self) {
    self.sync_active_pattern_from_ui();
    self.is_rendering = true;
    self.render_progress = 0.0;

    let fmt_idx = self.render_format_idx.min(Self::EXPORT_FORMATS.len() - 1);
    let fmt = Self::EXPORT_FORMATS[fmt_idx];
    let sample_rate = self.export_sample_rate();
    let folder = if self.export_folder.trim().is_empty() {
        crate::paths::paths().exports_dir().to_string_lossy().to_string()
    } else {
        self.export_folder.trim().to_string()
    };

    let base = {
        let raw = if self.export_base_name.trim().is_empty() {
            self.project_name.clone()
        } else {
            self.export_base_name.clone()
        };
        let safe = crate::audio::sanitize_filename(&raw);
        if safe.is_empty() { crate::i18n::t("Min_Låt").to_string() } else { safe }
    };

    let mut meta = self.export_metadata.clone();
    if meta.title.trim().is_empty() {
        meta.title = base.clone();
    }

    let dry = self.render_scope_idx == 1;
    let stem_mode = self.render_scope_idx == 1 || self.render_scope_idx == 2;

    std::fs::create_dir_all(&folder).map_err(|e| {
        self.is_rendering = false;
        self.render_queue_status = crate::tstatus!("✘ Kan inte skapa mapp: {}", e);
        self.status_message = crate::tstatus!("✘ Exportfel: {}", e);
    }).ok();

    let mut written_files: Vec<String> = Vec::new();
    let mut last_error: Option<String> = None;
    let mut normalize_note: Option<String> = None;
    let loudness = crate::audio::LoudnessPreset::ALL
        [self.export_loudness_idx.min(crate::audio::LoudnessPreset::ALL.len() - 1)];

    if stem_mode {
        let track_count = self.playlist_tracks.len();
        for t_idx in 0..track_count {
            let track_name = self.playlist_tracks[t_idx].name.replace([' ', '/', '\\'], "_");
            let spec = self.build_render_spec(Some(t_idx), sample_rate);
            let buf = match self.render_buffer(&spec, dry) {
                Ok(b) => b,
                Err(e) => { last_error = Some(e); break; }
            };
            if Self::peak_of(&buf) < 1.0e-6 {
                continue; // tomt spår – hoppa över
            }
            let file_stem = format!("{}_Trk{:02}_{}", base, t_idx + 1, track_name);
            let path = format!("{}/{}.{}", folder, file_stem, fmt.ext());
            let mut stem_meta = meta.clone();
            if stem_meta.title.trim().is_empty() || stem_meta.title == base {
                stem_meta.title = format!("{} ({})", base, track_name);
            }
            let dither = self.export_dither_settings();
            match crate::audio::write_export_with(&path, fmt, &buf, sample_rate, &stem_meta, dither) {
                Ok(_) => written_files.push(path),
                Err(e) => { last_error = Some(e); break; }
            }
        }
    } else {
        let spec = self.build_render_spec(None, sample_rate);
        let mut buf = match self.render_buffer(&spec, false) {
            Ok(b) => b,
            Err(e) => { last_error = Some(e); Vec::new() }
        };
        if last_error.is_none() {
            if let Some(measured) =
                crate::audio::normalize_to_preset(&mut buf, sample_rate, loudness)
            {
                normalize_note = Some(crate::tstatus!(
                    "🔊 Normaliserad till {} LUFS (mätt {} LUFS, tak {} dBTP)",
                    loudness.target_lufs(),
                    (measured * 10.0).round() / 10.0,
                    loudness.ceiling_dbtp()
                ));
            }
            let path = format!("{}/{}_{:.0}bpm.{}", folder, base, self.bpm, fmt.ext());
            let dither = self.export_dither_settings();
            match crate::audio::write_export_with(&path, fmt, &buf, sample_rate, &meta, dither) {
                Ok(_) => written_files.push(path),
                Err(e) => last_error = Some(e),
            }
        }
    }

    self.is_rendering = false;
    self.render_progress = 1.0;
    match last_error {
        Some(e) => {
            self.render_queue_status = crate::tstatus!("✘ Exporten avbröts: {}", e);
            self.status_message = crate::tstatus!("✘ Exportfel: {}", e);
        }
        None if written_files.is_empty() => {
            self.render_queue_status = crate::i18n::t("⚠ Inga ljudfiler skapades (alla spår var tomma?).").to_string();
            self.status_message = crate::i18n::t("⚠ Inga ljud skapades.").to_string();
        }
        None => {
            self.render_queue_status = crate::tstatus!("✅ {} fil(er) exporterade till {}", written_files.len(), folder);
            if let Some(note) = &normalize_note {
                self.render_queue_status = crate::tstatus!("✅ {} fil(er) → {} · {}", written_files.len(), folder, note);
            }
            self.status_message = crate::tstatus!("✔ Klar! Exporterade {} fil(er) utan AI-metadata → {}", written_files.len(), folder);
        }
    }
}
}

