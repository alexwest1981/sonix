//! MIDI — notschemat, importen, exporten och trumkanalerna.
//!
//! Adresseringen är kromatisk: noten `bas + i` spelar slice `i` (\`window_for_note\`).
//! Två axlar betyder olika saker — rader är tonhöjd, steg är tid — och taket är det
//! **mindre** av de två antalen (\`grid_capacity\`).
//!
//! Status: byggs — MIDI-vägarna.
//! Rör inte: `notes_by_bar`/`pattern_bar_notes` är samma notbild som exporten skriver.

use super::*;

/// Standardnotnummer per trumkanal (kanal 0–5) — samma som trumspåret använder
/// när en kanal saknar eget sample.
const DRUM_CHANNEL_KEYS: [u8; 6] = [36, 38, 39, 42, 46, 49];

/// MIDI-noterna ett pattern ger upphov till för ett spår av en viss typ.
///
/// Ren funktion (Fas 6.3): mappningen från appens 16-stegsrutor till noter ska
/// kunna testas utan GUI eller ljudmotor. Mappningen speglar `trigger_song_step`
/// — trummor tar kanal 0–5, synthspåret tar piano-roll-rutnätet (med kanal 6 som
/// reserv när rutnätet är tomt) och basspåret kanal 7.
pub(crate) fn pattern_bar_notes(
    pattern: &Pattern,
    kind: TrackKind,
    channel: u8,
    bar_start_step: u32,
    velocities: &[u8; 16],
) -> Vec<crate::audio::smf::MidiNote> {
    use crate::audio::smf::{MidiNote, TICKS_PER_STEP_16TH};
    let mut out: Vec<MidiNote> = Vec::new();
    let place = |key: u8, step: usize, chan: u8, out: &mut Vec<MidiNote>| {
        out.push(MidiNote {
            start: (bar_start_step + step as u32) * TICKS_PER_STEP_16TH,
            length: TICKS_PER_STEP_16TH,
            channel: chan,
            key: key.min(127),
            velocity: velocities[step].clamp(1, 127),
        });
    };
    match kind {
        TrackKind::Drums => {
            for ch in 0..DRUM_CHANNEL_KEYS.len() {
                let Some(steps) = pattern.channel_steps.get(ch) else {
                    continue;
                };
                let notes = pattern.channel_notes.get(ch);
                for step in 0..16 {
                    if steps[step] {
                        let key = notes.map(|n| n[step]).unwrap_or(DRUM_CHANNEL_KEYS[ch]);
                        place(key, step, 9, &mut out);
                    }
                }
            }
        }
        TrackKind::SynthLead => {
            let grid_active = (0..24).any(|r| (0..16).any(|s| pattern.piano_roll_grid[r][s]));
            if grid_active {
                for row in 0..24 {
                    for step in 0..16 {
                        if pattern.piano_roll_grid[row][step] {
                            place(48 + row as u8, step, channel, &mut out);
                        }
                    }
                }
            } else if let (Some(steps), Some(notes)) = (
                pattern.channel_steps.get(6),
                pattern.channel_notes.get(6),
            ) {
                for step in 0..16 {
                    if steps[step] {
                        place(notes[step], step, channel, &mut out);
                    }
                }
            }
        }
        TrackKind::Bassline => {
            if let (Some(steps), Some(notes)) = (
                pattern.channel_steps.get(7),
                pattern.channel_notes.get(7),
            ) {
                for step in 0..16 {
                    if steps[step] {
                        place(notes[step], step, channel, &mut out);
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// MIDI-kanal för ett melodiskt spår. Kanal 9 är percussion, så den hoppas över.
pub(crate) fn midi_channel_for_track(track_index: usize) -> u8 {
    let c = if track_index >= 9 { track_index + 1 } else { track_index };
    (c % 16) as u8
}

/// Trumtangent → appens trumkanal 0–5 (nära General MIDI).
pub(crate) fn drum_channel_for_key(key: u8) -> Option<usize> {
    match key {
        35..=37 => Some(0),              // bastrumma
        38 | 40 => Some(1),              // virvel
        39 => Some(2),                   // handklapp
        41..=44 => Some(3),              // sluten hi-hat
        45..=48 => Some(4),              // öppen hi-hat
        49..=59 => Some(5),              // crash/cymbal (utom 54, 56, 58 = tamburin/klocka)
        _ => None,
    }
}

/// Vad en MIDI-import gjorde med filen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MidiImportReport {
    pub notes_placed: usize,
    /// Takter som fick ett eget mönster (och, vid fler än en, en kloss på spåret).
    pub bars_imported: usize,
    /// Noter vars takt ligger utanför arrangemanget (32 takter).
    pub dropped_beyond_arrangement: usize,
    /// Noter utanför appens rutnät (trumtangenter utan kanal, toner utanför 48–71).
    pub dropped_out_of_range: usize,
}

/// Steg per takt: samma rutnät som appens pattern — hämtat från tempokartan,
/// så att det bara finns en källa.
pub const STEPS_PER_BAR: usize = crate::audio::tempo::STEPS_PER_BAR;

/// Arrangemangets längd i takter — `clips` har 32 platser.
pub const ARRANGEMENT_BARS: usize = 32;

/// Noterna grupperade per takt, i taktordning.
///
/// Ren funktion (Fas 6.3), så att "inga noter tappas" går att pröva utan att
/// starta appen: en fil på fyra takter ska ge fyra grupper, inte en.
pub fn notes_by_bar(
    notes: &[crate::audio::smf::MidiNote],
    file_ppq: u16,
) -> Vec<(usize, Vec<crate::audio::smf::MidiNote>)> {
    let step_ticks = (file_ppq as u32 / 4).max(1);
    let mut groups: Vec<(usize, Vec<crate::audio::smf::MidiNote>)> = Vec::new();
    for n in notes {
        let bar = (n.start / step_ticks / STEPS_PER_BAR as u32) as usize;
        match groups.iter_mut().find(|(b, _)| *b == bar) {
            Some((_, list)) => list.push(n.clone()),
            None => groups.push((bar, vec![n.clone()])),
        }
    }
    groups.sort_by_key(|(b, _)| *b);
    groups
}

/// Lägger EN takts SMF-noter i ett pattern (steg 0–15).
///
/// Noterna routas dit appen själv spelar dem: percussion (kanal 9) till
/// trumkanalerna 0–5, 48–71 till piano-rollen, och allt under 48 till
/// baskanalen (7) — appens basspår spelar kanal 7 med råa notnummer, så en
/// basstämma från en annan DAW hör hemma där i stället för att slängas.
///
/// **Notlängden följer med** för toner: en not som håller över ett steg tänder
/// alla steg den klingar igenom. Trumslag tänder bara sitt eget steg — ett slag
/// är ett slag, och en lång not i en trumkanal är inte tre slag.
///
/// Ren funktion (Fas 6.3). Allt som ändå inte får plats räknas i stället för
/// att tyst försvinna — en import som tappar halva filen måste säga det.
pub(crate) fn apply_bar_to_pattern(
    notes: &[crate::audio::smf::MidiNote],
    file_ppq: u16,
    pattern: &mut Pattern,
) -> MidiImportReport {
    let step_ticks = (file_ppq as u32 / 4).max(1);
    let mut rep = MidiImportReport::default();
    while pattern.channel_steps.len() < 8 {
        pattern.channel_steps.push([false; 16]);
    }
    while pattern.channel_notes.len() < 8 {
        pattern.channel_notes.push([60; 16]);
    }

    for n in notes {
        let step = ((n.start / step_ticks) % STEPS_PER_BAR as u32) as usize;
        if n.channel == 9 {
            match drum_channel_for_key(n.key) {
                Some(ch) => {
                    pattern.channel_steps[ch][step] = true;
                    pattern.channel_notes[ch][step] = n.key;
                    rep.notes_placed += 1;
                }
                None => rep.dropped_out_of_range += 1,
            }
            continue;
        }
        // Toner: längden i steg, minst ett, klippt mot taktens slut.
        let span = (n.length.div_ceil(step_ticks).max(1) as usize).min(STEPS_PER_BAR - step);
        if (48..72).contains(&n.key) {
            for s in step..step + span {
                pattern.piano_roll_grid[(n.key - 48) as usize][s] = true;
            }
            rep.notes_placed += 1;
        } else if n.key < 48 {
            // Basstämma: appens basspår spelar kanal 7 med notnumret direkt.
            for s in step..step + span {
                pattern.channel_steps[7][s] = true;
                pattern.channel_notes[7][s] = n.key;
            }
            rep.notes_placed += 1;
        } else {
            rep.dropped_out_of_range += 1;
        }
    }
    rep
}

impl SonixApp {
/// Samlar arrangemanget som MIDI-spår (Fas 6.3). Ljudspår (regioner) hoppas
/// över — de har ingen MIDI-motsvarighet.
fn collect_song_midi(&self) -> Vec<crate::audio::smf::MidiTrack> {
    let mut tracks: Vec<crate::audio::smf::MidiTrack> = Vec::new();
    for (t_idx, track) in self.playlist_tracks.iter().enumerate() {
        if !matches!(
            track.kind,
            TrackKind::Drums | TrackKind::SynthLead | TrackKind::Bassline
        ) {
            continue;
        }
        let velocities: [u8; 16] = std::array::from_fn(|i| {
            (self.step_velocities[i] * track.volume * 127.0).clamp(1.0, 127.0) as u8
        });
        let channel = midi_channel_for_track(t_idx);
        let mut notes = Vec::new();
        for bar in 0..32usize {
            if let Some(pat_idx) = track.clips[bar]
                && let Some(pat) = self.patterns.get(pat_idx)
            {
                notes.extend(pattern_bar_notes(
                    pat,
                    track.kind,
                    channel,
                    (bar * 16) as u32,
                    &velocities,
                ));
            }
        }
        tracks.push(crate::audio::smf::MidiTrack {
            name: track.name.clone(),
            notes,
        });
    }
    if tracks.iter().all(|t| t.notes.is_empty()) {
        // Ingen spelad patternrad: exportera det valda patternet som en takt,
        // så knappen inte ger en tom fil.
        if let Some(pat) = self.patterns.get(self.selected_pattern) {
            let velocities: [u8; 16] = std::array::from_fn(|i| {
                (self.step_velocities[i] * 127.0).clamp(1.0, 127.0) as u8
            });
            let mut notes = pattern_bar_notes(pat, TrackKind::Drums, 9, 0, &velocities);
            notes.extend(pattern_bar_notes(pat, TrackKind::SynthLead, 0, 0, &velocities));
            notes.extend(pattern_bar_notes(pat, TrackKind::Bassline, 1, 0, &velocities));
            tracks.clear();
            tracks.push(crate::audio::smf::MidiTrack {
                name: pat.name.clone(),
                notes,
            });
        }
    }
    tracks
}
}

impl SonixApp {
/// Skriver arrangemanget som `.mid` i exportmappen (Fas 6.3).
pub fn export_song_midi(&mut self) {
    let tracks = self.collect_song_midi();
    let note_count: usize = tracks.iter().map(|t| t.notes.len()).sum();
    if note_count == 0 {
        self.status_message =
            crate::i18n::t("⚠ Inga MIDI-noter att exportera — rita i ett pattern först.")
                .to_string();
        return;
    }
    // Exporten frågar tempokartan i stället för att räkna själv: i dag har
    // kartan en enda punkt (projektets tempo) och filen blir byte för byte
    // den samma som förut — men den dag projekten har tempobyten följer de
    // med ut i MIDI-filen utan att någon behöver komma ihåg det här stället.
    let tempo_points: Vec<(f64, f32)> = self
        .tempo_map()
        .points()
        .iter()
        .map(|p| (p.start_bar as f64, p.bpm))
        .collect();
    let bytes = crate::audio::smf::write_midi_with_tempo(&tempo_points, &tracks);
    let dir = crate::paths::paths().exports_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("{}.mid", crate::autosave::slug(&self.project_name)));
    match crate::autosave::write_atomic(&file, &bytes) {
        Ok(()) => {
            self.status_message = crate::tstatus!(
                "🎼 Exporterade {} noter i {} spår till {}",
                note_count,
                tracks.len(),
                file.display()
            );
        }
        Err(e) => {
            self.status_message =
                crate::tstatus!("⚠ Kunde inte skriva MIDI-filen: {}", e);
        }
    }
}
}

impl SonixApp {
/// Importerar en `.mid` till det valda patternet (Fas 6.3).
pub fn import_midi_into_selected_pattern(&mut self, path: &str) -> Result<MidiImportReport, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("kunde inte läsa {path}: {e}"))?;
    let parsed = crate::audio::smf::parse_midi(&bytes)?;
    let total: usize = parsed.tracks.iter().map(|t| t.notes.len()).sum();
    if total == 0 {
        return Err("filen innehöll inga noter".to_string());
    }
    let notes: Vec<crate::audio::smf::MidiNote> = parsed
        .notes_with_track()
        .into_iter()
        .map(|(_, n)| n.clone())
        .collect();
    let groups = notes_by_bar(&notes, parsed.ppq);
    let bars_in_file = groups.len();
    let idx = self.selected_pattern.min(self.patterns.len().saturating_sub(1));
    let Some(pat) = self.patterns.get(idx) else {
        return Err("inget pattern att importera till".to_string());
    };
    let name = pat.name.clone();
    let color = pat.color;

    // En fil på en takt beter sig precis som förut — mönstret fylls, och
    // ingen kloss sätts, för den som bygger mönster för hand vill placera dem
    // själv. En fil på flera takter blir i stället ett arrangemang: en kloss
    // per takt på det valda spåret, annars skulle noterna bara hamna i
    // mönster man inte ser.
    let as_arrangement = bars_in_file > 1;
    let track_idx = self.selected_timeline_track;
    let mut report = MidiImportReport::default();

    for (bar, bar_notes) in groups {
        if bar >= ARRANGEMENT_BARS {
            report.dropped_beyond_arrangement += bar_notes.len();
            continue;
        }
        let pat_idx = if bar == 0 {
            idx
        } else {
            self.patterns.push(Pattern {
                name: crate::tstatus!("{} ⋅ takt {}", name, bar + 1),
                color,
                channel_steps: Vec::new(),
                channel_notes: Vec::new(),
                piano_roll_grid: [[false; 16]; 24],
                take: crate::midi_take::Take::new(),
            });
            self.patterns.len() - 1
        };
        if let Some(pat) = self.patterns.get_mut(pat_idx) {
            let r = apply_bar_to_pattern(&bar_notes, parsed.ppq, pat);
            report.notes_placed += r.notes_placed;
            report.dropped_out_of_range += r.dropped_out_of_range;
        }
        if as_arrangement
            && let Some(track) = self.playlist_tracks.get_mut(track_idx)
        {
            track.clips[bar] = Some(pat_idx);
        }
        report.bars_imported += 1;
    }
    // Tempot i filen används bara som förslag när projektet står kvar på sin
    // ursprungs-BPM; annars vore en import en tyst tempoändring. Samma regel
    // gäller filens **tempobyten** (Fas 8.2 steg 3), och regeln ligger som en
    // ren funktion i `tempo`, så att den kan prövas utan fönster.
    let from_file = self.bpm_source_is_file();
    let imported_points = if from_file {
        crate::audio::tempo::tempo_points_for_import(&parsed.tempo_events, self.bpm)
    } else {
        None
    };
    let mut tempo_note = String::new();
    match imported_points {
        Some(points) => {
            let count = points.len();
            self.tempo_points = points;
            // `bpm` speglar kartans första punkt (Fas 8.2) — samma regel som
            // när ett tempobyte sätts för hand.
            self.bpm = self.tempo_map().bpm_at(0.0);
            if count > 1 {
                tempo_note = crate::tstatus!(" — och {} tempopunkter ur filen", count);
            }
        }
        None => {
            if from_file {
                if let Some(bpm) = parsed.bpm {
                    self.bpm = bpm.clamp(40.0, 260.0);
                }
            } else if parsed.tempo_events.len() > 1 {
                // Filens byten togs inte in. Att tiga om det vore att tappa
                // dem utan att säga till.
                tempo_note = crate::tstatus!(
                    " — filens {} tempobyten togs inte in (projektet har eget tempo)",
                    parsed.tempo_events.len()
                );
            }
        }
    }
    let fmt = if parsed.format == 0 {
        crate::i18n::t(" [format 0: en spår]")
    } else {
        ""
    };
    let placed_where = if as_arrangement {
        crate::tstatus!(
            " i {} mönster på spåret (takt 1–{})",
            report.bars_imported,
            report.bars_imported
        )
    } else {
        format!(" till pattern '{}'", name)
    };
    self.status_message = if report.dropped_beyond_arrangement + report.dropped_out_of_range == 0 {
        crate::tstatus!(
            "🎼 Importerade {} noter{}{}",
            report.notes_placed,
            placed_where,
            fmt
        )
    } else {
        crate::tstatus!(
            "🎼 Importerade {} noter{}{} — hoppade över {} bortom takt {} och {} utanför rutnätet",
            report.notes_placed,
            placed_where,
            fmt,
            report.dropped_beyond_arrangement,
            ARRANGEMENT_BARS,
            report.dropped_out_of_range
        )
    };
    if !tempo_note.is_empty() {
        self.status_message = format!("{}{}", self.status_message, tempo_note);
    }
    Ok(report)
}
}

impl SonixApp {
/// Sant när projektets tempo inte ändrats sedan starten — då får en import
/// föreslå filens tempo.
fn bpm_source_is_file(&self) -> bool {
    (self.bpm - 120.0).abs() < 0.05
}
}

impl SonixApp {
/// Import av `.mid` (Fas 6.3): fil, målpattern och vad som hände.
pub(crate) fn render_midi_import_modal(&mut self, ctx: &egui::Context) {
    if !self.show_midi_import_modal {
        return;
    }
    let mut path = self.midi_import_path.clone();
    let target = self
        .patterns
        .get(self.selected_pattern)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let mut do_import = false;
    let mut close_clicked = false;
    let mut open = self.show_midi_import_modal;
    egui::Window::new(crate::i18n::t("🎼 Importera MIDI-fil (.mid)"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(600.0, 190.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Noterna läggs i det valda patternet (första takten). Trummor går till trumkanalerna, toner till piano-rollen.",
                ))
                .size(11.5)
                .color(Theme::TEXT_BRIGHT),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Fil:"));
                ui.add(
                    egui::TextEdit::singleline(&mut path)
                        .desired_width(440.0)
                        .hint_text("/sökväg/till/fil.mid"),
                );
            });
            ui.add_space(8.0);
            if ui.button(crate::i18n::t("  Importera  ")).clicked() {
                do_import = true;
            }
            if ui.button(crate::i18n::t("Stäng")).clicked() {
                close_clicked = true;
            }
            if !target.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(crate::tstatus!("Målpattern: '{}'", target))
                        .size(11.0)
                        .color(Theme::FL_CYAN),
                );
            }
        });
    self.midi_import_path = path;
    if do_import {
        let file = self.midi_import_path.clone();
        if let Err(e) = self.import_midi_into_selected_pattern(&file) {
            self.status_message = crate::tstatus!("⚠ Kunde inte importera MIDI: {}", e);
        }
    }
    self.show_midi_import_modal = open && !close_clicked;
}
}

