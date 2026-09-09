use eframe::egui::{self, Color32, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};
use std::f32::consts::PI;
use super::theme::Theme;

/// Draw a tactile rotary knob (like FL Studio / analog synth hardware)
pub fn rotary_knob(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    label: &str,
    color: Color32,
    size: f32,
) -> bool {
    let mut changed = false;
    let knob_width = size.max(46.0);
    let desired_size = Vec2::new(knob_width, size + 16.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

    let center = Pos2::new(rect.center().x, rect.min.y + size * 0.5);
    let radius = size * 0.42;

    if response.dragged() {
        let delta = response.drag_delta();
        let range = max - min;
        // Dragging up increases, down decreases
        let change = (-delta.y + delta.x * 0.5) * (range / 150.0);
        let new_val = (*value + change).clamp(min, max);
        if new_val != *value {
            *value = new_val;
            changed = true;
        }
    }

    // Normalized value 0.0 .. 1.0
    let norm = ((*value - min) / (max - min)).clamp(0.0, 1.0);

    // Angles: from -135 deg to +135 deg (270 deg total range)
    let start_angle = 135.0_f32.to_radians();
    let end_angle = 405.0_f32.to_radians();
    let current_angle = start_angle + norm * (end_angle - start_angle);

    let painter = ui.painter();

    // 1. Knob outer shadow & body
    painter.circle_filled(center, radius + 2.0, Color32::from_rgb(12, 14, 18));
    painter.circle_filled(center, radius, Color32::from_rgb(36, 40, 50));
    painter.circle_stroke(center, radius, Stroke::new(1.5_f32, Color32::from_rgb(55, 62, 75)));

    // 2. Inner dial cap
    painter.circle_filled(center, radius * 0.72, Color32::from_rgb(26, 30, 38));

    // 3. Indicator track arc (background)
    let arc_radius = radius * 0.88;
    for i in 0..16 {
        let a = start_angle + (i as f32 / 15.0) * (end_angle - start_angle);
        let dot_pos = Pos2::new(center.x + a.cos() * arc_radius, center.y + a.sin() * arc_radius);
        let dot_color = if a <= current_angle {
            color
        } else {
            Color32::from_rgb(45, 50, 60)
        };
        painter.circle_filled(dot_pos, 1.2, dot_color);
    }

    // 4. Indicator pointer line
    let pointer_end = Pos2::new(
        center.x + current_angle.cos() * (radius * 0.65),
        center.y + current_angle.sin() * (radius * 0.65),
    );
    let pointer_start = Pos2::new(
        center.x + current_angle.cos() * (radius * 0.2),
        center.y + current_angle.sin() * (radius * 0.2),
    );
    painter.line_segment([pointer_start, pointer_end], Stroke::new(2.5_f32, color));

    // 5. Label below knob
    let text_pos = Pos2::new(rect.center().x, rect.max.y - 6.0);
    painter.text(
        text_pos,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        Theme::TEXT_MUTED,
    );

    changed
}

/// Draw an FL Studio style tactile Step Button with LED
pub fn fl_step_button(
    ui: &mut Ui,
    active: bool,
    is_playhead: bool,
    beat_group_a: bool,
    accent_color: Color32,
    size: Vec2,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();

    // Base body color (FL Studio 4-beat color alternating grouping)
    let (base_bg, top_highlight) = if beat_group_a {
        (Color32::from_rgb(46, 52, 64), Color32::from_rgb(60, 68, 84))
    } else {
        (Color32::from_rgb(32, 36, 44), Color32::from_rgb(44, 50, 60))
    };

    let (bg_color, stroke_color) = if is_playhead {
        (Color32::from_rgb(255, 230, 90), Color32::WHITE)
    } else if active {
        (accent_color, Color32::from_rgb(220, 240, 255))
    } else {
        (base_bg, Color32::from_rgb(20, 22, 28))
    };

    // 1. Button beveled body
    painter.rect_filled(rect, Rounding::same(3.0), bg_color);
    painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, stroke_color));

    // 2. Bevel top highlight if not active
    if !active && !is_playhead {
        let highlight_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0));
        painter.rect_filled(highlight_rect, Rounding::same(1.0), top_highlight);
    }

    // 3. Center Glowing LED indicator
    let led_center = rect.center();
    let led_radius = 3.5;
    if active {
        // Glowing halo
        painter.circle_filled(led_center, led_radius + 2.5, Color32::from_rgba_unmultiplied(255, 255, 255, 120));
        painter.circle_filled(led_center, led_radius, Color32::WHITE);
    } else {
        painter.circle_filled(led_center, 2.0, Color32::from_rgb(18, 20, 26));
    }

    response
}

/// Draw a live bouncing oscilloscope display (FL Studio Wave Candy style)
pub fn oscilloscope_display(ui: &mut Ui, samples: &[f32], peak: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();

    // Dark LCD monitor screen
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(10, 14, 18));
    painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.5_f32, Color32::from_rgb(30, 38, 48)));

    // Grid lines on LCD
    let grid_color = Color32::from_rgb(18, 26, 34);
    let mid_y = rect.center().y;
    painter.line_segment([Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)], Stroke::new(1.0_f32, grid_color));

    for i in 1..4 {
        let x = rect.min.x + (rect.width() * i as f32 / 4.0);
        painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(1.0_f32, grid_color));
    }

    // Real output waveform: newest samples scroll in from the right edge
    if samples.len() >= 2 {
        let n = samples.len();
        let amp = (rect.height() * 0.44).max(2.0);
        let clipped = peak >= 0.99;
        let line_color = if clipped { Color32::from_rgb(255, 150, 60) } else { Color32::from_rgb(0, 255, 220) };
        let glow_color = if clipped { Color32::from_rgba_unmultiplied(255, 120, 0, 60) } else { Color32::from_rgba_unmultiplied(0, 240, 255, 60) };

        // Cap the number of drawn points while keeping the full window shape.
        let max_points = 240usize;
        let stride = (n / max_points).max(1);

        let mut points = Vec::with_capacity(n / stride + 1);
        let mut i = 0usize;
        while i < n {
            let t = i as f32 / (n - 1) as f32;
            let x = rect.min.x + t * rect.width();
            let v = samples[i].clamp(-1.0, 1.0);
            points.push(Pos2::new(x, mid_y - v * amp));
            i += stride;
        }
        // Newest sample pinned to the right edge
        let v = samples[n - 1].clamp(-1.0, 1.0);
        points.push(Pos2::new(rect.max.x, mid_y - v * amp));

        painter.add(egui::Shape::line(points.clone(), Stroke::new(3.0_f32, glow_color)));
        painter.add(egui::Shape::line(points, Stroke::new(1.5_f32, line_color)));
    }
}

/// Draw a vertical Mixer Channel Fader with dB markings and adjacent LED VU meter ladder (FL Studio & GarageBand style)
pub fn vertical_fader(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    meter_peak: f32,
    accent_color: Color32,
    fader_height: f32,
) -> bool {
    let mut changed = false;
    let fader_width = 38.0;
    let meter_width = 12.0;
    let total_width = fader_width + meter_width + 8.0;
    
    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_width, fader_height), Sense::click_and_drag());
    let painter = ui.painter();

    let fader_rect = Rect::from_min_size(rect.min, Vec2::new(fader_width, fader_height));
    let meter_rect = Rect::from_min_size(Pos2::new(rect.min.x + fader_width + 6.0, rect.min.y), Vec2::new(meter_width, fader_height));

    if response.dragged() {
        let delta_y = response.drag_delta().y;
        let range = max - min;
        let change = -delta_y * (range / (fader_height - 24.0));
        let new_val = (*value + change).clamp(min, max);
        if new_val != *value {
            *value = new_val;
            changed = true;
        }
    }

    let norm = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let track_margin = 14.0;
    let track_top = fader_rect.min.y + track_margin;
    let track_bottom = fader_rect.max.y - track_margin;
    let track_x = fader_rect.center().x;

    // 1. Fader Groove Background
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(track_x - 3.0, track_top), Pos2::new(track_x + 3.0, track_bottom)),
        Rounding::same(2.0),
        Color32::from_rgb(14, 16, 20),
    );
    painter.rect_stroke(
        Rect::from_min_max(Pos2::new(track_x - 3.0, track_top), Pos2::new(track_x + 3.0, track_bottom)),
        Rounding::same(2.0),
        Stroke::new(1.0_f32, Color32::from_rgb(32, 38, 48)),
    );

    // 2. dB Scale Markings (-inf, -24, -12, -6, 0dB, +3dB)
    let marks = [
        (1.0, "+3", Color32::from_rgb(255, 100, 80)),
        (0.8, " 0", Color32::from_rgb(220, 230, 245)),
        (0.6, "-6", Color32::from_rgb(140, 150, 170)),
        (0.4, "-12", Color32::from_rgb(110, 120, 140)),
        (0.2, "-24", Color32::from_rgb(90, 100, 120)),
        (0.0, "-∞", Color32::from_rgb(70, 80, 100)),
    ];
    for (m_norm, m_txt, m_col) in marks {
        let y = track_bottom - m_norm * (track_bottom - track_top);
        painter.line_segment([Pos2::new(fader_rect.min.x + 2.0, y), Pos2::new(track_x - 5.0, y)], Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 65)));
        painter.text(
            Pos2::new(fader_rect.min.x + 1.0, y),
            egui::Align2::LEFT_CENTER,
            m_txt,
            egui::FontId::proportional(8.0),
            m_col,
        );
    }

    // 3. Metallic Fader Thumb Cap
    let thumb_y = track_bottom - norm * (track_bottom - track_top);
    let thumb_width = 30.0;
    let thumb_height = 18.0;
    let thumb_rect = Rect::from_center_size(Pos2::new(track_x, thumb_y), Vec2::new(thumb_width, thumb_height));

    // Cap gradient & bevel
    painter.rect_filled(thumb_rect, Rounding::same(3.0), Color32::from_rgb(50, 56, 68));
    painter.rect_stroke(thumb_rect, Rounding::same(3.0), Stroke::new(1.5_f32, Color32::from_rgb(180, 190, 205)));
    // Center glowing indicator stripe on fader cap
    painter.line_segment(
        [Pos2::new(thumb_rect.min.x + 4.0, thumb_y), Pos2::new(thumb_rect.max.x - 4.0, thumb_y)],
        Stroke::new(2.0_f32, accent_color),
    );

    // 4. Multi-Segment LED Peak Meter Ladder
    painter.rect_filled(meter_rect, Rounding::same(2.0), Color32::from_rgb(12, 14, 18));
    painter.rect_stroke(meter_rect, Rounding::same(2.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 38, 48)));

    let num_segments = 24;
    let segment_h = (meter_rect.height() - 4.0) / num_segments as f32;
    let active_segments = (meter_peak.clamp(0.0, 1.2) * num_segments as f32) as usize;

    for s in 0..num_segments {
        let seg_idx_from_bottom = s;
        let is_lit = seg_idx_from_bottom < active_segments;
        let y_top = meter_rect.max.y - 2.0 - (s + 1) as f32 * segment_h;
        let seg_rect = Rect::from_min_size(Pos2::new(meter_rect.min.x + 1.5, y_top + 0.8), Vec2::new(meter_width - 3.0, segment_h - 1.2));

        let lit_color = if s >= 22 {
            Color32::from_rgb(255, 40, 40) // Red clip
        } else if s >= 18 {
            Color32::from_rgb(255, 140, 20) // Orange warning
        } else if s >= 12 {
            Color32::from_rgb(255, 220, 30) // Yellow nominal
        } else {
            Color32::from_rgb(46, 204, 113) // Green safe
        };

        let fill_color = if is_lit {
            lit_color
        } else {
            Color32::from_rgb(
                (lit_color.r() as f32 * 0.15) as u8,
                (lit_color.g() as f32 * 0.15) as u8,
                (lit_color.b() as f32 * 0.15) as u8,
            )
        };

        painter.rect_filled(seg_rect, Rounding::same(1.0), fill_color);
    }

    changed
}

/// Draw an Interactive 3-Band Parametric EQ Display (FL Studio Parametric EQ 2 style)
pub fn eq_curve_visualizer(
    ui: &mut Ui,
    low_gain: &mut f32,
    mid_gain: &mut f32,
    high_gain: &mut f32,
    size: Vec2,
) -> bool {
    let mut changed = false;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter();

    // Dark screen
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(12, 16, 22));
    painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 68)));

    let mid_y = rect.center().y;

    // Grid lines (dB & Freq)
    let grid_stroke = Stroke::new(1.0_f32, Color32::from_rgb(22, 28, 38));
    painter.line_segment([Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(34, 44, 58)));
    painter.line_segment([Pos2::new(rect.min.x + rect.width() * 0.33, rect.min.y), Pos2::new(rect.min.x + rect.width() * 0.33, rect.max.y)], grid_stroke);
    painter.line_segment([Pos2::new(rect.min.x + rect.width() * 0.66, rect.min.y), Pos2::new(rect.min.x + rect.width() * 0.66, rect.max.y)], grid_stroke);

    // Interactive Dragging on Node Zones
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let rel_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let rel_gain = (((mid_y - pos.y) / (rect.height() * 0.45)) * 12.0).clamp(-12.0, 12.0);
            if rel_x < 0.33 {
                *low_gain = rel_gain;
                changed = true;
            } else if rel_x > 0.66 {
                *high_gain = rel_gain;
                changed = true;
            } else {
                *mid_gain = rel_gain;
                changed = true;
            }
        }
    }

    // EQ Curve points
    let steps = 48;
    let mut curve_pts = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32; // 0.0 to 1.0 (freq axis 20Hz to 20kHz)
        let x = rect.min.x + t * rect.width();

        // Low shelf filter curve influence
        let low_w = (1.0 - (t / 0.38)).clamp(0.0, 1.0).powf(2.0);
        // Mid bell filter curve influence
        let mid_dist = (t - 0.5).abs() / 0.25;
        let mid_w = (1.0 - mid_dist.min(1.0)).powf(2.0);
        // High shelf filter curve influence
        let high_w = ((t - 0.62) / 0.38).clamp(0.0, 1.0).powf(2.0);

        let total_db = *low_gain * low_w + *mid_gain * mid_w + *high_gain * high_w; // -12dB to +12dB
        let y_offset = (total_db / 12.0) * (rect.height() * 0.4);
        let y = mid_y - y_offset;
        curve_pts.push(Pos2::new(x, y));
    }

    // Glow and fill under curve
    painter.add(egui::Shape::line(curve_pts.clone(), Stroke::new(4.0_f32, Color32::from_rgba_unmultiplied(255, 128, 0, 80))));
    painter.add(egui::Shape::line(curve_pts, Stroke::new(2.0_f32, Theme::FL_ORANGE)));

    // Frequency labels
    painter.text(Pos2::new(rect.min.x + 8.0, rect.max.y - 10.0), egui::Align2::LEFT_CENTER, "LOW (100Hz)", egui::FontId::proportional(9.0), Theme::TEXT_MUTED);
    painter.text(Pos2::new(rect.center().x, rect.max.y - 10.0), egui::Align2::CENTER_CENTER, "MID (1kHz)", egui::FontId::proportional(9.0), Theme::TEXT_MUTED);
    painter.text(Pos2::new(rect.max.x - 8.0, rect.max.y - 10.0), egui::Align2::RIGHT_CENTER, "HIGH (10kHz)", egui::FontId::proportional(9.0), Theme::TEXT_MUTED);

    changed
}

/// Mini EQ Thumbnail for Track Channel Strips
pub fn mini_track_eq_curve(
    painter: &egui::Painter,
    rect: Rect,
    low_gain: f32,
    mid_gain: f32,
    high_gain: f32,
    accent_col: Color32,
    enabled: bool,
) {
    painter.rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(14, 18, 24));
    painter.rect_stroke(rect, Rounding::same(2.0), Stroke::new(0.8_f32, Color32::from_rgb(32, 40, 52)));

    let mid_y = rect.center().y;
    painter.line_segment([Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)], Stroke::new(0.5_f32, Color32::from_rgb(26, 34, 46)));

    if !enabled {
        painter.line_segment([Pos2::new(rect.min.x + 2.0, mid_y), Pos2::new(rect.max.x - 2.0, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(80, 85, 95)));
        return;
    }

    let steps = 16;
    let mut pts = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let x = rect.min.x + t * rect.width();

        let low_w = (1.0 - (t / 0.38)).clamp(0.0, 1.0).powf(2.0);
        let mid_dist = (t - 0.5).abs() / 0.25;
        let mid_w = (1.0 - mid_dist.min(1.0)).powf(2.0);
        let high_w = ((t - 0.62) / 0.38).clamp(0.0, 1.0).powf(2.0);

        let total_db = low_gain * low_w + mid_gain * mid_w + high_gain * high_w;
        let y_offset = (total_db / 12.0) * (rect.height() * 0.40);
        let y = mid_y - y_offset;
        pts.push(Pos2::new(x, y.clamp(rect.min.y + 1.0, rect.max.y - 1.0)));
    }

    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, accent_col)));
}

/// Draw an interactive 2D Drummer XY Performance Matrix (GarageBand Virtual Session Drummer style)
pub fn drummer_xy_matrix(
    ui: &mut Ui,
    complexity: &mut f32, // X: 0.0 (Simple) .. 1.0 (Complex)
    loudness: &mut f32,   // Y: 0.0 (Soft) .. 1.0 (Loud)
    size: Vec2,
) -> bool {
    let mut changed = false;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter();

    if (response.dragged() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos() {
            let norm_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let norm_y = (1.0 - (pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
            if norm_x != *complexity || norm_y != *loudness {
                *complexity = norm_x;
                *loudness = norm_y;
                changed = true;
            }
        }

    // 1. Matrix Background (Dark brushed radar screen)
    painter.rect_filled(rect, Rounding::same(6.0), Color32::from_rgb(18, 22, 30));
    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, Color32::from_rgb(45, 55, 75)));

    // 2. Crosshairs & concentric rings
    let center = rect.center();
    painter.line_segment([Pos2::new(rect.min.x, center.y), Pos2::new(rect.max.x, center.y)], Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 52)));
    painter.line_segment([Pos2::new(center.x, rect.min.y), Pos2::new(center.x, rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(30, 38, 52)));

    painter.circle_stroke(center, rect.height() * 0.22, Stroke::new(1.0_f32, Color32::from_rgb(26, 34, 46)));
    painter.circle_stroke(center, rect.height() * 0.42, Stroke::new(1.0_f32, Color32::from_rgb(26, 34, 46)));

    // 3. Axis Edge Labels (Loud, Soft, Simple, Complex)
    painter.text(Pos2::new(center.x, rect.min.y + 12.0), egui::Align2::CENTER_CENTER, "LOUD", egui::FontId::proportional(11.0), Color32::from_rgb(220, 230, 245));
    painter.text(Pos2::new(center.x, rect.max.y - 12.0), egui::Align2::CENTER_CENTER, "SOFT", egui::FontId::proportional(11.0), Color32::from_rgb(120, 135, 155));
    painter.text(Pos2::new(rect.min.x + 32.0, center.y), egui::Align2::CENTER_CENTER, "SIMPLE", egui::FontId::proportional(11.0), Color32::from_rgb(120, 135, 155));
    painter.text(Pos2::new(rect.max.x - 36.0, center.y), egui::Align2::CENTER_CENTER, "COMPLEX", egui::FontId::proportional(11.0), Color32::from_rgb(220, 230, 245));

    // 4. Draggable Glowing Yellow Puck
    let puck_x = rect.min.x + *complexity * rect.width();
    let puck_y = rect.max.y - *loudness * rect.height();
    let puck_pos = Pos2::new(puck_x, puck_y);

    // Glowing halo rings
    painter.circle_filled(puck_pos, 16.0, Color32::from_rgba_unmultiplied(255, 200, 40, 50));
    painter.circle_filled(puck_pos, 10.0, Color32::from_rgba_unmultiplied(255, 215, 60, 120));
    painter.circle_filled(puck_pos, 6.0, Color32::from_rgb(255, 225, 80));
    painter.circle_stroke(puck_pos, 6.0, Stroke::new(1.5_f32, Color32::WHITE));

    changed
}

/// Draw an interactive 8-Snapshot Morphing Pad (Alchemy Synthesizer Transform Pad style)
pub fn alchemy_transform_matrix(
    ui: &mut Ui,
    puck_pos_norm: &mut [f32; 2], // X: -1.0..1.0, Y: -1.0..1.0
    size: Vec2,
) -> bool {
    let mut changed = false;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter();

    if (response.dragged() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos() {
            let norm_x = ((pos.x - rect.center().x) / (rect.width() * 0.42)).clamp(-1.0, 1.0);
            let norm_y = ((pos.y - rect.center().y) / (rect.height() * 0.42)).clamp(-1.0, 1.0);
            if norm_x != puck_pos_norm[0] || norm_y != puck_pos_norm[1] {
                puck_pos_norm[0] = norm_x;
                puck_pos_norm[1] = norm_y;
                changed = true;
            }
        }

    // Pad Background
    painter.rect_filled(rect, Rounding::same(6.0), Color32::from_rgb(14, 18, 24));
    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, Color32::from_rgb(0, 150, 180)));

    let center = rect.center();
    let pad_radius = rect.height() * 0.36;

    // 8 Snapshots around perimeter
    let snapshots = [
        (-0.5 * PI, "1. Pluck"),
        (-0.25 * PI, "2. Lush String"),
        (0.0 * PI, "3. 303 Acid"),
        (0.25 * PI, "4. Synthwave"),
        (0.5 * PI, "5. Warm Pad"),
        (0.75 * PI, "6. Cosmic Brass"),
        (1.0 * PI, "7. Digital Bell"),
        (1.25 * PI, "8. Chiptune"),
    ];

    for (angle, label) in snapshots {
        let x = center.x + angle.cos() * pad_radius;
        let y = center.y + angle.sin() * pad_radius;
        let p = Pos2::new(x, y);

        // Snapshot pad dot
        painter.circle_filled(p, 7.0, Color32::from_rgb(25, 45, 60));
        painter.circle_stroke(p, 7.0, Stroke::new(1.5_f32, Color32::from_rgb(0, 200, 240)));
        painter.circle_filled(p, 3.0, Color32::from_rgb(0, 255, 220));

        // Label offset slightly outward
        let text_p = Pos2::new(center.x + angle.cos() * (pad_radius + 24.0), center.y + angle.sin() * (pad_radius + 16.0));
        painter.text(text_p, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(9.0), Color32::from_rgb(180, 220, 240));
        painter.line_segment([center, p], Stroke::new(1.0_f32, Color32::from_rgb(24, 38, 50)));
    }

    // Draggable Cyan Cursor
    let cursor_x = center.x + puck_pos_norm[0] * pad_radius;
    let cursor_y = center.y + puck_pos_norm[1] * pad_radius;
    let cursor_pos = Pos2::new(cursor_x, cursor_y);

    painter.circle_filled(cursor_pos, 18.0, Color32::from_rgba_unmultiplied(0, 220, 255, 40));
    painter.circle_filled(cursor_pos, 10.0, Color32::from_rgba_unmultiplied(0, 240, 255, 120));
    painter.circle_filled(cursor_pos, 6.0, Color32::from_rgb(180, 255, 255));
    painter.circle_stroke(cursor_pos, 6.0, Stroke::new(1.5_f32, Color32::WHITE));

    changed
}

