use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::patcher::{ModularGraph, NodeType, PatchCable};
use crate::ui::theme::Theme;

pub fn render_patcher_view(ui: &mut Ui, graph: &mut ModularGraph, anim_phase: f32) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🧩 MODULAR PATCHER & THE GRID (Bitwig & FL Patcher Style)").strong().size(13.5).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new("Visuell modulär miljö: Koppla ihop ljudsignaler, syntmoduler, filter och LFO med virtuella kablar").size(11.0).color(Theme::TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Återställ Standard-patch").clicked() {
                    graph.load_default_preset();
                }
                if ui.button("🗑 Rensa Allt").clicked() {
                    graph.nodes.clear();
                    graph.cables.clear();
                }
            });
        });

        ui.add_space(6.0);

        // Quick Node Creation Toolbar
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("➕ Lägg till modul:").strong().size(11.0).color(Theme::FL_ORANGE));
            if ui.button("+ 🔊 Oscillator").clicked() {
                graph.add_node(NodeType::Oscillator, Pos2::new(180.0, 60.0));
            }
            if ui.button("+ 🌊 SVF Filter").clicked() {
                graph.add_node(NodeType::Filter, Pos2::new(360.0, 60.0));
            }
            if ui.button("+ 📈 ADSR Envelope").clicked() {
                graph.add_node(NodeType::Envelope, Pos2::new(180.0, 220.0));
            }
            if ui.button("+ 🌀 LFO Modulator").clicked() {
                graph.add_node(NodeType::Lfo, Pos2::new(30.0, 220.0));
            }
            if ui.button("+ 🌊 Stereo Delay").clicked() {
                graph.add_node(NodeType::Delay, Pos2::new(540.0, 60.0));
            }
            if ui.button("+ ✨ Space Reverb").clicked() {
                graph.add_node(NodeType::Reverb, Pos2::new(540.0, 220.0));
            }
            if ui.button("+ 🔥 Tube Drive").clicked() {
                graph.add_node(NodeType::Distortion, Pos2::new(480.0, 140.0));
            }
        });

        ui.add_space(8.0);

        // Interactive Canvas
        let canvas_size = Vec2::new(ui.available_width().max(850.0), 480.0);
        let (canvas_rect, response) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());
        let painter = ui.painter();

        // Canvas Background Grid
        painter.rect_filled(canvas_rect, Rounding::same(6.0), Color32::from_rgb(14, 18, 24));
        painter.rect_stroke(canvas_rect, Rounding::same(6.0), Stroke::new(1.5_f32, Color32::from_rgb(30, 42, 58)));

        // Helper to convert canvas-local Pos2 to screen Pos2
        let to_screen = |local: Pos2| -> Pos2 {
            Pos2::new(canvas_rect.min.x + local.x, canvas_rect.min.y + local.y)
        };

        // Draw dot grid
        let grid_step = 28.0;
        for gx in (canvas_rect.min.x as i32..canvas_rect.max.x as i32).step_by(grid_step as usize) {
            for gy in (canvas_rect.min.y as i32..canvas_rect.max.y as i32).step_by(grid_step as usize) {
                painter.circle_filled(Pos2::new(gx as f32, gy as f32), 1.0, Color32::from_rgb(26, 34, 46));
            }
        }

        let node_width = 160.0;

        // Draw Cables (Glowing Bezier Curves)
        for cable in &graph.cables {
            let from_node_opt = graph.nodes.iter().find(|n| n.id == cable.from_node);
            let to_node_opt = graph.nodes.iter().find(|n| n.id == cable.to_node);

            if let (Some(fn_node), Some(tn_node)) = (from_node_opt, to_node_opt) {
                let from_pin_pos = to_screen(Pos2::new(
                    fn_node.pos.x + node_width,
                    fn_node.pos.y + 36.0 + cable.from_pin as f32 * 18.0,
                ));
                let to_pin_pos = to_screen(Pos2::new(
                    tn_node.pos.x,
                    tn_node.pos.y + 36.0 + cable.to_pin as f32 * 18.0,
                ));

                let cp1 = Pos2::new(from_pin_pos.x + 50.0, from_pin_pos.y);
                let cp2 = Pos2::new(to_pin_pos.x - 50.0, to_pin_pos.y);

                // Cable Bezier Curve
                let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                    [from_pin_pos, cp1, cp2, to_pin_pos],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(3.0_f32, cable.color),
                );
                painter.add(curve);

                // Animated glowing signal pulse traveling along cable
                let pulse_t = (anim_phase * 0.5 + (cable.from_node * 3) as f32 * 0.2) % 1.0;
                let pulse_pos = eval_bezier(from_pin_pos, cp1, cp2, to_pin_pos, pulse_t);
                painter.circle_filled(pulse_pos, 3.5, Color32::WHITE);
            }
        }

        // Handle Active Dragging Cable
        if let Some((f_node_id, f_pin)) = graph.connecting_from {
            if let Some(fn_node) = graph.nodes.iter().find(|n| n.id == f_node_id) {
                let from_pin_pos = to_screen(Pos2::new(
                    fn_node.pos.x + node_width,
                    fn_node.pos.y + 36.0 + f_pin as f32 * 18.0,
                ));
                if let Some(ptr_pos) = response.interact_pointer_pos() {
                    let cp1 = Pos2::new(from_pin_pos.x + 40.0, from_pin_pos.y);
                    let cp2 = Pos2::new(ptr_pos.x - 40.0, ptr_pos.y);
                    let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                        [from_pin_pos, cp1, cp2, ptr_pos],
                        false,
                        Color32::TRANSPARENT,
                        Stroke::new(2.5_f32, Color32::from_rgb(255, 230, 80)),
                    );
                    painter.add(curve);
                }
            }
        }

        let mut connect_to = None;
        let mut start_connect = None;
        let mut remove_cable_idx = None;

        // Render Nodes
        let nodes_len = graph.nodes.len();
        for i in 0..nodes_len {
            let (node_id, local_pos, title, color, in_pins, out_pins) = {
                let n = &graph.nodes[i];
                (n.id, n.pos, n.title.clone(), n.color, n.inputs.clone(), n.outputs.clone())
            };

            let screen_pos = to_screen(local_pos);
            let node_height = (55.0 + (in_pins.len().max(out_pins.len()) as f32 * 19.0)).max(95.0);
            let node_rect = Rect::from_min_size(screen_pos, Vec2::new(node_width, node_height));
            let header_rect = Rect::from_min_size(screen_pos, Vec2::new(node_width, 24.0));

            // Node Body
            painter.rect_filled(node_rect, Rounding::same(6.0), Color32::from_rgb(22, 28, 38));
            painter.rect_stroke(node_rect, Rounding::same(6.0), Stroke::new(1.5_f32, color));

            // Node Header
            painter.rect_filled(header_rect, Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 }, color);
            painter.text(header_rect.center(), egui::Align2::CENTER_CENTER, &title, egui::FontId::proportional(11.0), Color32::BLACK);

            // Input Pins (Left)
            for (p_idx, pin_name) in in_pins.iter().enumerate() {
                let pin_screen_pos = to_screen(Pos2::new(local_pos.x, local_pos.y + 36.0 + p_idx as f32 * 18.0));
                let pin_hover_rect = Rect::from_center_size(pin_screen_pos, Vec2::splat(14.0));

                let is_hovered = response.hover_pos().map(|p| pin_hover_rect.contains(p)).unwrap_or(false);
                let pin_fill = if is_hovered { Theme::FL_YELLOW } else { Color32::from_rgb(40, 60, 80) };

                painter.circle_filled(pin_screen_pos, 5.0, pin_fill);
                painter.circle_stroke(pin_screen_pos, 5.0, Stroke::new(1.5_f32, Theme::FL_CYAN));
                painter.text(Pos2::new(pin_screen_pos.x + 9.0, pin_screen_pos.y), egui::Align2::LEFT_CENTER, *pin_name, egui::FontId::proportional(9.0), Color32::from_rgb(180, 200, 220));

                // Check connect on release
                if is_hovered && response.drag_stopped() {
                    connect_to = Some((node_id, p_idx));
                }
                if is_hovered && response.secondary_clicked() {
                    remove_cable_idx = graph.cables.iter().position(|c| c.to_node == node_id && c.to_pin == p_idx);
                }
            }

            // Output Pins (Right)
            for (p_idx, pin_name) in out_pins.iter().enumerate() {
                let pin_screen_pos = to_screen(Pos2::new(local_pos.x + node_width, local_pos.y + 36.0 + p_idx as f32 * 18.0));
                let pin_hover_rect = Rect::from_center_size(pin_screen_pos, Vec2::splat(14.0));

                let is_hovered = response.hover_pos().map(|p| pin_hover_rect.contains(p)).unwrap_or(false);
                let pin_fill = if is_hovered { Theme::FL_YELLOW } else { Color32::from_rgb(40, 60, 80) };

                painter.circle_filled(pin_screen_pos, 5.0, pin_fill);
                painter.circle_stroke(pin_screen_pos, 5.0, Stroke::new(1.5_f32, Theme::FL_ORANGE));
                painter.text(Pos2::new(pin_screen_pos.x - 9.0, pin_screen_pos.y), egui::Align2::RIGHT_CENTER, *pin_name, egui::FontId::proportional(9.0), Color32::from_rgb(220, 210, 180));

                // Check click start drag
                if is_hovered && response.drag_started() {
                    start_connect = Some((node_id, p_idx));
                }
                if is_hovered && response.secondary_clicked() {
                    remove_cable_idx = graph.cables.iter().position(|c| c.from_node == node_id && c.from_pin == p_idx);
                }
            }

            // Node Dragging Check (by dragging header)
            if response.dragged() {
                if let Some(ptr) = response.interact_pointer_pos() {
                    if header_rect.contains(ptr) {
                        let mut new_pos = graph.nodes[i].pos + response.drag_delta();
                        new_pos.x = new_pos.x.clamp(10.0, (canvas_rect.width() - node_width - 10.0).max(10.0));
                        new_pos.y = new_pos.y.clamp(10.0, (canvas_rect.height() - node_height - 10.0).max(10.0));
                        graph.nodes[i].pos = new_pos;
                    }
                }
            }
        }

        // Apply Cable Connection
        if let Some((from_node, from_pin)) = start_connect {
            graph.connecting_from = Some((from_node, from_pin));
        }

        if let Some((to_node, to_pin)) = connect_to {
            if let Some((from_node, from_pin)) = graph.connecting_from {
                if from_node != to_node {
                    // Check if cable already exists
                    if !graph.cables.iter().any(|c| c.from_node == from_node && c.from_pin == from_pin && c.to_node == to_node && c.to_pin == to_pin) {
                        let color = Color32::from_rgb(100 + (from_node as u8 * 40) % 155, 200, 240);
                        graph.cables.push(PatchCable {
                            from_node,
                            from_pin,
                            to_node,
                            to_pin,
                            color,
                        });
                    }
                }
            }
            graph.connecting_from = None;
        } else if response.drag_stopped() {
            graph.connecting_from = None;
        }

        if let Some(idx) = remove_cable_idx {
            if idx < graph.cables.len() {
                graph.cables.remove(idx);
            }
        }
    });
}

fn eval_bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
    let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;
    Pos2::new(x, y)
}
