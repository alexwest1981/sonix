use eframe::egui::{self, Color32, Rounding, Stroke, Ui, Vec2};
use crate::audio::plugin_host::{PluginFormat, PluginManager};
use crate::ui::theme::Theme;

pub fn render_plugins_view(ui: &mut Ui, manager: &mut PluginManager, status_msg: &mut String) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔌 PLUGIN & WINE/YABRIDGE BRIDGE MANAGER").strong().size(14.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new("Sömlös hantering av CLAP, VST3, LV2 och isolerade Windows-plugins via Yabridge").size(11.0).color(Theme::TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Skanna plugins").clicked() {
                    manager.rescan();
                    *status_msg = "✔ Plugin-skanning klar (CLAP, VST3, LV2, Yabridge)".to_string();
                }
            });
        });

        ui.add_space(8.0);

        // System Architecture & Sandboxing Status Banners
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // PipeWire Status
                ui.label(egui::RichText::new("🟢 PipeWire RT Ljudmotor:").strong().size(11.0).color(Theme::FL_GREEN));
                ui.label(egui::RichText::new("44.1 kHz • 64 buffert (1.4 ms)").size(11.0).color(Theme::TEXT_BRIGHT));
                ui.separator();

                // Crash Sandboxing Status
                ui.label(egui::RichText::new("🛡 Process-Isolering (Sandboxing):").strong().size(11.0).color(Theme::FL_YELLOW));
                let sand_text = if manager.sandboxing_enabled { "AKTIV (Säkert mot krascher)" } else { "AV" };
                ui.label(egui::RichText::new(sand_text).size(11.0).color(Theme::FL_GREEN));
                ui.separator();

                // Windows Yabridge Status
                ui.label(egui::RichText::new("🍷 Windows Yabridge-brygga:").strong().size(11.0).color(Theme::FL_PURPLE));
                ui.label(egui::RichText::new(&manager.wine_version).size(11.0).color(Theme::TEXT_BRIGHT));
            });
        });

        ui.add_space(8.0);

        // Filter Bar & Search
        ui.horizontal(|ui| {
            ui.label("🔍 Sök plugin:");
            ui.text_edit_singleline(&mut manager.search_query);
            ui.separator();

            ui.label("Format-filter:");
            if ui.selectable_label(manager.selected_format_filter.is_none(), "Alla").clicked() {
                manager.selected_format_filter = None;
            }
            if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Clap), "CLAP").clicked() {
                manager.selected_format_filter = Some(PluginFormat::Clap);
            }
            if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Vst3), "VST3 (Native)").clicked() {
                manager.selected_format_filter = Some(PluginFormat::Vst3);
            }
            if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Lv2), "LV2").clicked() {
                manager.selected_format_filter = Some(PluginFormat::Lv2);
            }
            if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::WineYabridge), "🍷 Windows VST (Yabridge)").clicked() {
                manager.selected_format_filter = Some(PluginFormat::WineYabridge);
            }
        });

        ui.add_space(8.0);

        // Plugins Table
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for plugin in manager.plugins.iter_mut() {
                // Filter matching
                if let Some(fmt) = manager.selected_format_filter
                    && plugin.format != fmt {
                        continue;
                    }
                if !manager.search_query.is_empty() {
                    let q = manager.search_query.to_lowercase();
                    if !plugin.name.to_lowercase().contains(&q) && !plugin.vendor.to_lowercase().contains(&q) {
                        continue;
                    }
                }

                let badge_color = plugin.format.badge_color();
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        // Format Badge
                        let (b_rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 22.0), egui::Sense::hover());
                        ui.painter().rect_filled(b_rect, Rounding::same(3.0), Color32::from_rgb(24, 30, 42));
                        ui.painter().rect_stroke(b_rect, Rounding::same(3.0), Stroke::new(1.0_f32, badge_color));
                        ui.painter().text(b_rect.center(), egui::Align2::CENTER_CENTER, plugin.format.name(), egui::FontId::proportional(9.0), badge_color);

                        // Plugin Title & Vendor
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&plugin.name).strong().size(12.0).color(Color32::WHITE));
                            ui.label(egui::RichText::new(&plugin.vendor).size(10.0).color(Theme::TEXT_MUTED));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let load_label = if plugin.is_loaded { "✔ Aktiv i Projekt" } else { "➕ Ladda Plugin" };
                            let load_color = if plugin.is_loaded { Theme::FL_GREEN } else { Theme::FL_ORANGE };
                            if ui.add(egui::Button::new(egui::RichText::new(load_label).strong().size(11.0).color(Color32::WHITE)).fill(load_color)).clicked() {
                                plugin.is_loaded = !plugin.is_loaded;
                                *status_msg = format!("Laddade plugin: {}", plugin.name);
                            }

                            // CPU & Sandboxed badge
                            ui.label(egui::RichText::new(format!("CPU: {:.1}%", plugin.cpu_usage)).size(10.0).color(Theme::TEXT_MUTED));
                            if plugin.is_sandboxed {
                                ui.label(egui::RichText::new("🛡 Sandboxed").size(10.0).color(Theme::FL_CYAN));
                            }
                        });
                    });
                });
                ui.add_space(2.0);
            }
        });
    });
}
