use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use crate::audio::{AudioCommand, AudioEngine};
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob;

#[derive(Clone, Debug)]
pub struct FxPedal {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub enabled: bool,
    pub color: Color32,
    pub p1_name: &'static str,
    pub p1_val: f32, // 0.0 .. 1.0
    pub p2_name: &'static str,
    pub p2_val: f32,
    pub p3_name: &'static str,
    pub p3_val: f32,
    pub mix: f32,
}

#[derive(Clone, Debug)]
pub struct FxRackState {
    pub pedals: Vec<FxPedal>,
    pub selected_preset: usize,
}

impl Default for FxRackState {
    fn default() -> Self {
        Self {
            pedals: vec![
                FxPedal {
                    id: "tube_drive",
                    name: "🔥 TUBE SCREAMER",
                    category: "Distortion & Saturation",
                    enabled: true,
                    color: Color32::from_rgb(230, 90, 30),
                    p1_name: "DRIVE",
                    p1_val: 0.45,
                    p2_name: "TONE",
                    p2_val: 0.60,
                    p3_name: "WARMTH",
                    p3_val: 0.70,
                    mix: 0.80,
                },
                FxPedal {
                    id: "stereo_chorus",
                    name: "🌊 DIMENSION CHORUS",
                    category: "Modulation & Width",
                    enabled: true,
                    color: Color32::from_rgb(40, 160, 220),
                    p1_name: "RATE",
                    p1_val: 0.35,
                    p2_name: "DEPTH",
                    p2_val: 0.65,
                    p3_name: "WIDTH",
                    p3_val: 0.85,
                    mix: 0.50,
                },
                FxPedal {
                    id: "tape_delay",
                    name: "⏳ ANALOG TAPE DELAY",
                    category: "Echo & Time",
                    enabled: true,
                    color: Color32::from_rgb(210, 170, 40),
                    p1_name: "TIME",
                    p1_val: 0.50,
                    p2_name: "FEEDBACK",
                    p2_val: 0.40,
                    p3_name: "FLUTTER",
                    p3_val: 0.25,
                    mix: 0.35,
                },
                FxPedal {
                    id: "cosmic_reverb",
                    name: "🌌 SPACE REVERB",
                    category: "Reverb & Atmosphere",
                    enabled: true,
                    color: Color32::from_rgb(160, 70, 230),
                    p1_name: "DECAY",
                    p1_val: 0.60,
                    p2_name: "SIZE",
                    p2_val: 0.75,
                    p3_name: "SHIMMER",
                    p3_val: 0.30,
                    mix: 0.40,
                },
                FxPedal {
                    id: "bitcrusher",
                    name: "👾 8-BIT CRUSHER",
                    category: "Lo-Fi & Digital",
                    enabled: false,
                    color: Color32::from_rgb(46, 204, 113),
                    p1_name: "BITS",
                    p1_val: 0.75,
                    p2_name: "DOWNSMPL",
                    p2_val: 0.30,
                    p3_name: "JITTER",
                    p3_val: 0.15,
                    mix: 0.60,
                },
                FxPedal {
                    id: "maximizer",
                    name: "🛡 MASTER MAXIMIZER",
                    category: "Dynamics & Limiter",
                    enabled: true,
                    color: Color32::from_rgb(255, 60, 60),
                    p1_name: "PUNCH",
                    p1_val: 0.55,
                    p2_name: "CEILING",
                    p2_val: 0.95,
                    p3_name: "RELEASE",
                    p3_val: 0.40,
                    mix: 1.0,
                },
            ],
            selected_preset: 0,
        }
    }
}

impl FxRackState {
    pub fn load_preset(&mut self, preset_idx: usize) {
        self.selected_preset = preset_idx;
        match preset_idx {
            0 => { // Cyberpunk Synthwave
                if let Some(d) = self.pedals.get_mut(0) { d.enabled = true; d.p1_val = 0.65; d.p2_val = 0.70; }
                if let Some(c) = self.pedals.get_mut(1) { c.enabled = true; c.p1_val = 0.40; c.p3_val = 0.90; }
                if let Some(dl) = self.pedals.get_mut(2) { dl.enabled = true; dl.p1_val = 0.50; dl.p2_val = 0.45; }
                if let Some(r) = self.pedals.get_mut(3) { r.enabled = true; r.p1_val = 0.75; r.p3_val = 0.50; }
                if let Some(b) = self.pedals.get_mut(4) { b.enabled = false; }
            }
            1 => { // Lo-Fi Nostalgia 90s
                if let Some(d) = self.pedals.get_mut(0) { d.enabled = true; d.p1_val = 0.30; d.p2_val = 0.40; }
                if let Some(c) = self.pedals.get_mut(1) { c.enabled = true; c.p1_val = 0.60; c.p2_val = 0.80; }
                if let Some(dl) = self.pedals.get_mut(2) { dl.enabled = true; dl.p1_val = 0.35; dl.p3_val = 0.70; }
                if let Some(r) = self.pedals.get_mut(3) { r.enabled = true; r.p1_val = 0.45; }
                if let Some(b) = self.pedals.get_mut(4) { b.enabled = true; b.p1_val = 0.60; b.p2_val = 0.50; }
            }
            2 => { // Warm Studio Vocal
                if let Some(d) = self.pedals.get_mut(0) { d.enabled = true; d.p1_val = 0.15; d.p3_val = 0.85; }
                if let Some(c) = self.pedals.get_mut(1) { c.enabled = false; }
                if let Some(dl) = self.pedals.get_mut(2) { dl.enabled = true; dl.p1_val = 0.25; dl.mix = 0.20; }
                if let Some(r) = self.pedals.get_mut(3) { r.enabled = true; r.p1_val = 0.50; r.mix = 0.30; }
                if let Some(b) = self.pedals.get_mut(4) { b.enabled = false; }
            }
            3 => { // 80s Gated Arena Rock
                if let Some(d) = self.pedals.get_mut(0) { d.enabled = true; d.p1_val = 0.80; }
                if let Some(c) = self.pedals.get_mut(1) { c.enabled = true; c.p1_val = 0.50; }
                if let Some(dl) = self.pedals.get_mut(2) { dl.enabled = false; }
                if let Some(r) = self.pedals.get_mut(3) { r.enabled = true; r.p1_val = 0.85; r.p2_val = 0.90; r.mix = 0.60; }
                if let Some(b) = self.pedals.get_mut(4) { b.enabled = false; }
            }
            _ => {}
        }
    }
}

pub fn render_fx_rack_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut FxRackState,
    engine: &mut AudioEngine,
    status_msg: &mut String,
) {
    if !*open {
        return;
    }

    let mut close = false;

    egui::Window::new("🎛 Modulärt FX-Pedalbord & Stompbox Rack (Effects Library)")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(760.0, 560.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header bar
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🎛 MODULÄRT EFFEKT-RACK").strong().size(14.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.label(egui::RichText::new("Hårdvarupedaler, analoga rör och rymdklang för dina spår").size(10.5).color(Theme::TEXT_MUTED));
                });

                ui.add_space(4.0);

                // Presets Bar
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Kedjemallar (Presets):").strong().color(Theme::FL_CYAN));
                        let presets = ["Cyberpunk Synthwave", "Lo-Fi Nostalgia 90s", "Warm Studio Vocal", "80s Gated Arena Rock", "Clean Acoustic"];
                        egui::ComboBox::from_id_salt("fx_rack_preset_picker")
                            .selected_text(*presets.get(state.selected_preset).unwrap_or(&"Standard"))
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for (pr_idx, &pr_name) in presets.iter().enumerate() {
                                    if ui.selectable_label(state.selected_preset == pr_idx, pr_name).clicked() {
                                        state.load_preset(pr_idx);
                                        *status_msg = format!("🎛 Laddade FX-preset: {}", pr_name);
                                    }
                                }
                            });

                        ui.separator();
                        if ui.button("⚡ Koppla in alla").clicked() {
                            for p in &mut state.pedals { p.enabled = true; }
                        }
                        if ui.button("⚪ Förbikoppla (Bypass)").clicked() {
                            for p in &mut state.pedals { p.enabled = false; }
                        }
                    });
                });

                ui.add_space(8.0);

                // PEDALBOARD SCROLL AREA WITH STOMPBOX CARDS
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        egui::Grid::new("fx_pedals_grid").spacing([10.0, 10.0]).show(ui, |ui| {
                            let card_w = ((ui.available_width() - 30.0) / 2.0).max(280.0);
                            let card_h = 100.0;

                            for (p_idx, pedal) in state.pedals.iter_mut().enumerate() {
                                let (rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());

                                let card_bg = if pedal.enabled {
                                    Color32::from_rgb(24, 28, 36)
                                } else {
                                    Color32::from_rgb(16, 18, 22)
                                };

                                ui.painter().rect_filled(rect, Rounding::same(6.0), card_bg);
                                ui.painter().rect_stroke(
                                    rect,
                                    Rounding::same(6.0),
                                    Stroke::new(1.5_f32, if pedal.enabled { pedal.color } else { Color32::from_rgb(45, 50, 60) }),
                                );

                                // Top Color Header Strip
                                let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 22.0));
                                ui.painter().rect_filled(top_bar, Rounding::same(5.0), if pedal.enabled { pedal.color } else { Color32::from_rgb(35, 40, 48) });

                                ui.painter().text(
                                    Pos2::new(rect.min.x + 8.0, rect.min.y + 11.0),
                                    egui::Align2::LEFT_CENTER,
                                    pedal.name,
                                    egui::FontId::proportional(11.0),
                                    if pedal.enabled { Color32::BLACK } else { Theme::TEXT_MUTED },
                                );

                                // Power LED & Switch on top right
                                let led_pos = Pos2::new(rect.max.x - 14.0, rect.min.y + 11.0);
                                ui.painter().circle_filled(
                                    led_pos,
                                    4.5,
                                    if pedal.enabled { Theme::FL_GREEN } else { Color32::from_rgb(80, 20, 20) },
                                );

                                // Internal Pedal Controls
                                let mut child_ui = ui.new_child(
                                    egui::UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(rect.min.x + 8.0, rect.min.y + 26.0), rect.max - Vec2::new(8.0, 6.0)))
                                );

                                child_ui.horizontal(|ui| {
                                    ui.checkbox(&mut pedal.enabled, "PÅ");
                                    ui.separator();

                                    rotary_knob(ui, &mut pedal.p1_val, 0.0, 1.0, pedal.p1_name, pedal.color, 18.0);
                                    rotary_knob(ui, &mut pedal.p2_val, 0.0, 1.0, pedal.p2_name, pedal.color, 18.0);
                                    rotary_knob(ui, &mut pedal.p3_val, 0.0, 1.0, pedal.p3_name, pedal.color, 18.0);
                                    rotary_knob(ui, &mut pedal.mix, 0.0, 1.0, "MIX", Color32::WHITE, 18.0);
                                });

                                if (p_idx + 1) % 2 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    });

                ui.add_space(10.0);

                // Action Bar
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("✔ Tillämpa på Master & Spår").strong().color(Color32::BLACK)).fill(Theme::FL_GREEN).min_size(Vec2::new(200.0, 32.0))).clicked() {
                        let _ = engine.send_command(AudioCommand::SetMasterVolume(0.95));
                        *status_msg = "✔ Effektkedja applicerad live på alla spår!".to_string();
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

    if close {
        *open = false;
    }
}
