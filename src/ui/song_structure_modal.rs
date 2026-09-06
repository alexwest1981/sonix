use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use crate::ui::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectionType {
    Intro,
    Verse,
    PreChorus,
    Chorus,
    Bridge,
    Drop,
    Solo,
    Outro,
}

impl SectionType {
    pub fn name_and_color(&self) -> (&'static str, Color32) {
        match self {
            SectionType::Intro => ("INTRO", Color32::from_rgb(240, 180, 40)),
            SectionType::Verse => ("VERS", Color32::from_rgb(50, 150, 240)),
            SectionType::PreChorus => ("BRYGGA", Color32::from_rgb(170, 80, 240)),
            SectionType::Chorus => ("REFRÄNG", Color32::from_rgb(255, 60, 60)),
            SectionType::Bridge => ("STICK", Color32::from_rgb(255, 120, 40)),
            SectionType::Drop => ("DROP / CLIMAX", Color32::from_rgb(0, 240, 200)),
            SectionType::Solo => ("SOLO", Color32::from_rgb(255, 215, 0)),
            SectionType::Outro => ("OUTRO", Color32::from_rgb(46, 204, 113)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SongSectionItem {
    pub section_type: SectionType,
    pub name: String,
    pub length_bars: usize,
}

#[derive(Clone, Debug)]
pub struct SongStructureState {
    pub sections: Vec<SongSectionItem>,
    pub selected_preset: usize,
}

impl Default for SongStructureState {
    fn default() -> Self {
        let mut s = Self {
            sections: Vec::new(),
            selected_preset: 0,
        };
        s.load_preset(0);
        s
    }
}

impl SongStructureState {
    pub fn load_preset(&mut self, preset_idx: usize) {
        self.selected_preset = preset_idx;
        match preset_idx {
            0 => { // Pop Standard
                self.sections = vec![
                    SongSectionItem { section_type: SectionType::Intro, name: "Intro".to_string(), length_bars: 4 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Vers 1".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::PreChorus, name: "Brygga 1".to_string(), length_bars: 4 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Refräng 1".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Vers 2".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::PreChorus, name: "Brygga 2".to_string(), length_bars: 4 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Refräng 2".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Solo, name: "Solo / Stick".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Slutrefräng".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Outro, name: "Outro".to_string(), length_bars: 4 },
                ];
            }
            1 => { // EDM Anthem
                self.sections = vec![
                    SongSectionItem { section_type: SectionType::Intro, name: "DJ Intro".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Build-Up 1".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Drop, name: "DROP 1".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Bridge, name: "Breakdown".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Build-Up 2".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Drop, name: "MAIN DROP".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Outro, name: "DJ Outro".to_string(), length_bars: 16 },
                ];
            }
            2 => { // Trap & Hip-Hop
                self.sections = vec![
                    SongSectionItem { section_type: SectionType::Intro, name: "Intro".to_string(), length_bars: 4 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Hook 1".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Vers 1 (16t)".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Hook 2".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Vers 2 (16t)".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Slut-Hook".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Outro, name: "Outro (Fade)".to_string(), length_bars: 4 },
                ];
            }
            _ => { // Synthwave Extended
                self.sections = vec![
                    SongSectionItem { section_type: SectionType::Intro, name: "Arp Intro".to_string(), length_bars: 8 },
                    SongSectionItem { section_type: SectionType::Verse, name: "Main Theme A".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Chorus, name: "Lead Climax B".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Solo, name: "Synthesizer Solo".to_string(), length_bars: 16 },
                    SongSectionItem { section_type: SectionType::Outro, name: "Outro Drive".to_string(), length_bars: 16 },
                ];
            }
        }
    }

    pub fn total_bars(&self) -> usize {
        self.sections.iter().map(|s| s.length_bars).sum()
    }
}

pub fn render_song_structure_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut SongStructureState,
    bpm: f32,
    status_msg: &mut String,
    mut on_apply_structure: impl FnMut(&[SongSectionItem]),
) {
    if !*open {
        return;
    }

    let mut close = false;
    let mut trigger_apply = false;

    let tot_bars = state.total_bars();
    let sec_per_bar = 60.0 / bpm.max(40.0) * 4.0;
    let total_secs = tot_bars as f32 * sec_per_bar;
    let mins = (total_secs / 60.0).floor() as u32;
    let secs = (total_secs % 60.0).floor() as u32;

    egui::Window::new("📑 Låtstruktur & Formdelar (Song Section Arranger)")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(760.0, 520.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header bar
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📑 STRUKTUR- & ARRANGEMANGSBYGGARE").strong().size(14.0).color(Theme::FL_CYAN));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Totalt: {} takter • Tid: {:02}:{:02} @ {:.1} BPM", tot_bars, mins, secs, bpm)).strong().color(Theme::FL_ORANGE));
                });

                ui.add_space(4.0);

                // Presets Bar
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Låtmall / Genre:").strong().color(Theme::FL_YELLOW));
                        let presets = ["Pop Hit Radio (Standard)", "EDM Festival Anthem", "Hip-Hop & Trap", "Synthwave Extended"];
                        egui::ComboBox::from_id_salt("song_structure_preset_picker")
                            .selected_text(*presets.get(state.selected_preset).unwrap_or(&"Pop"))
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for (pr_idx, &pr_name) in presets.iter().enumerate() {
                                    if ui.selectable_label(state.selected_preset == pr_idx, pr_name).clicked() {
                                        state.load_preset(pr_idx);
                                        *status_msg = format!("📑 Laddade låtstruktur: {}", pr_name);
                                    }
                                }
                            });

                        ui.separator();
                        if ui.button("➕ Lägg till Vers").clicked() {
                            state.sections.push(SongSectionItem { section_type: SectionType::Verse, name: "Ny Vers".to_string(), length_bars: 8 });
                        }
                        if ui.button("➕ Lägg till Refräng").clicked() {
                            state.sections.push(SongSectionItem { section_type: SectionType::Chorus, name: "Ny Refräng".to_string(), length_bars: 8 });
                        }
                    });
                });

                ui.add_space(8.0);

                // VISUAL TIMELINE OVERVIEW STRIP
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Tidslinjeöversikt (Övergripande form)").strong().color(Theme::TEXT_BRIGHT));
                    ui.add_space(4.0);

                    let (ov_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
                    ui.painter().rect_filled(ov_rect, Rounding::same(4.0), Color32::from_rgb(16, 18, 24));

                    let total_b = tot_bars.max(1) as f32;
                    let mut curr_x = ov_rect.min.x;

                    for sec in &state.sections {
                        let sec_w = (sec.length_bars as f32 / total_b) * ov_rect.width();
                        let (_, col) = sec.section_type.name_and_color();
                        let sec_rect = Rect::from_min_size(Pos2::new(curr_x, ov_rect.min.y), Vec2::new(sec_w, ov_rect.height()));
                        ui.painter().rect_filled(sec_rect, Rounding::same(2.0), col);
                        ui.painter().rect_stroke(sec_rect, Rounding::same(2.0), Stroke::new(1.0_f32, Color32::from_rgb(20, 24, 30)));

                        if sec_w > 40.0 {
                            ui.painter().text(
                                sec_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &sec.name,
                                egui::FontId::proportional(10.0),
                                Color32::BLACK,
                            );
                        }
                        curr_x += sec_w;
                    }
                });

                ui.add_space(8.0);

                // SECTIONS LIST / REORDER CARDS
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        let mut to_remove = None;
                        let mut move_up = None;
                        let mut move_down = None;

                        let mut bar_offset = 0;
                        let num_sections = state.sections.len();

                        for s_idx in 0..num_sections {
                            let sec = &mut state.sections[s_idx];
                            let (_, col) = sec.section_type.name_and_color();

                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    // Left color pill
                                    let (p_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 24.0), Sense::hover());
                                    ui.painter().rect_filled(p_rect, Rounding::same(3.0), col);

                                    ui.label(egui::RichText::new(format!("Takt {:03} - {:03}", bar_offset + 1, bar_offset + sec.length_bars)).monospace().color(Theme::FL_CYAN));
                                    bar_offset += sec.length_bars;

                                    ui.separator();

                                    ui.text_edit_singleline(&mut sec.name);

                                    ui.separator();
                                    ui.label("Längd:");
                                    for l in [2, 4, 8, 16, 32] {
                                        if ui.selectable_label(sec.length_bars == l, format!("{}t", l)).clicked() {
                                            sec.length_bars = l;
                                        }
                                    }

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("🗑").on_hover_text("Ta bort sektion").clicked() {
                                            to_remove = Some(s_idx);
                                        }
                                        if s_idx < num_sections - 1 && ui.button("▼").clicked() {
                                            move_down = Some(s_idx);
                                        }
                                        if s_idx > 0 && ui.button("▲").clicked() {
                                            move_up = Some(s_idx);
                                        }
                                    });
                                });
                            });
                            ui.add_space(2.0);
                        }

                        if let Some(r) = to_remove { state.sections.remove(r); }
                        if let Some(u) = move_up { state.sections.swap(u, u - 1); }
                        if let Some(d) = move_down { state.sections.swap(d, d + 1); }
                    });

                ui.add_space(8.0);

                // Action Bar
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("📥 Applicera Formdelar på Tidslinjen").strong().color(Color32::BLACK)).fill(Theme::FL_GREEN).min_size(Vec2::new(240.0, 32.0))).clicked() {
                        trigger_apply = true;
                        close = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Stäng").clicked() {
                            close = true;
                        }
                    });
                });
            });
        });

    if trigger_apply && !state.sections.is_empty() {
        on_apply_structure(&state.sections);
    }

    if close {
        *open = false;
    }
}
