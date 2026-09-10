use eframe::egui::{self, Color32, Rounding, Stroke, Ui, Vec2};
use crate::audio::plugin_host::{PluginCategory, PluginFormat, PluginManager};
use crate::ui::theme::Theme;

#[derive(Default)]
pub struct PluginViewActions {
    /// `(plugin path, stem track index)` to instantiate a CLAP processor into.
    pub load_into_track: Option<(String, usize)>,
    /// `(plugin path, stem track index)` to instantiate inside an out-of-process
    /// sandbox that streams audio over shared memory (Fas 4.5b).
    pub load_into_sandbox: Option<(String, usize)>,
    /// `(plugin path, stem track index, preset location)` to instantiate with a
    /// native `clap.preset-load/2` preset already applied.
    pub load_preset_into_track: Option<(String, usize, String)>,
    /// Stem track index whose plugin insert should be removed.
    pub remove_track: Option<usize>,
    /// Stem track index whose plugin editor should be opened (Fas 4.4b).
    pub open_gui: Option<usize>,
    /// Stem track index whose plugin editor should be closed.
    pub close_gui: Option<usize>,
    /// Plugin path to inspect out-of-process in the sandbox (Fas 4.5a).
    pub sandbox_inspect: Option<String>,
}

pub fn render_plugins_view(
    ui: &mut Ui,
    manager: &mut PluginManager,
    status_msg: &mut String,
    stem_track_count: usize,
    default_track: usize,
    active_plugins: &[Option<String>],
    gui_open: &[bool],
    sandbox_status: Option<&str>,
) -> PluginViewActions {
    ui.group(|ui| {
        // ====================================================================
        // 1. TOP HEADER & MAIN NAVIGATION TABS
        // ====================================================================
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🔌 PLUGIN & FL STUDIO BRIDGE MANAGER")).strong().size(15.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("Hantera och importera VST3, CLAP, LV2, FL Studio Native & Windows-plugins via Yabridge"))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(crate::i18n::t("🔄 Skanna mappar nu")).clicked() {
                    manager.rescan();
                    *status_msg = crate::tstatus!("✔ Plugin-skanning klar ({} plugins och presets hittades)", manager.plugins.len() + manager.presets.len());
                }

                let assist_btn = ui.selectable_label(manager.active_tab == 3, crate::i18n::t("🍷 FL Studio & Yabridge Assistent"));
                if assist_btn.clicked() { manager.active_tab = 3; }

                let imp_btn = ui.selectable_label(manager.active_tab == 2, crate::i18n::t("📥 Importera Plugin / .FST"));
                if imp_btn.clicked() { manager.active_tab = 2; }

                let paths_btn = ui.selectable_label(manager.active_tab == 1, crate::i18n::t("📁 Sökvägar & Mappar"));
                if paths_btn.clicked() { manager.active_tab = 1; }

                let db_btn = ui.selectable_label(manager.active_tab == 0, crate::i18n::t("🔌 Plugindatabas"));
                if db_btn.clicked() { manager.active_tab = 0; }
            });
        });

        ui.add_space(6.0);

        // ====================================================================
        // 2. SYSTEM STATUS BANNER (PipeWire, Sandboxing, Wine, Scan Status)
        // ====================================================================
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // PipeWire Status
                ui.label(egui::RichText::new(crate::i18n::t("🟢 PipeWire RT Ljudmotor:")).strong().size(11.0).color(Theme::FL_GREEN));
                ui.label(egui::RichText::new(crate::i18n::t("Aktiv (Sonix 512-sampels ringbuffert)")).size(11.0).color(Theme::TEXT_BRIGHT));
                ui.separator();

                // Plugin execution model
                ui.label(egui::RichText::new(crate::i18n::t("🛡 Plugin-körning:")).strong().size(11.0).color(Theme::FL_YELLOW));
                let host_available = crate::audio::plugin_host_live::is_available();
                let (host_label, host_color) = if host_available {
                    (crate::i18n::t("CLAP/VST2/VST3: host aktiv (laddar plugins)"), Theme::FL_GREEN)
                } else {
                    (crate::i18n::t("Endast metadata-skanning (bygg med --features plugin-host för CLAP/VST2/VST3-host)"), Theme::TEXT_MUTED)
                };
                ui.label(egui::RichText::new(host_label).size(11.0).color(host_color));
                ui.separator();

                // Wine / Yabridge Status
                ui.label(egui::RichText::new(crate::i18n::t("🍷 Windows / FL-brygga:")).strong().size(11.0).color(Theme::FL_PURPLE));
                ui.label(egui::RichText::new(&manager.wine_version).size(11.0).color(Theme::TEXT_BRIGHT));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&manager.last_scan_time).size(10.5).color(Theme::TEXT_MUTED));
                    ui.label(egui::RichText::new(crate::i18n::t("Senast skannad:")).size(10.5).color(Theme::TEXT_MUTED));
                });
            });
        });

        ui.add_space(8.0);

        // ====================================================================
        // 3. ACTIVE PER-TRACK INSERTS (Fas 4.2 / 4.3)
        // ====================================================================
        let mut actions = PluginViewActions::default();
        render_active_inserts(ui, active_plugins, gui_open, &mut actions);

        ui.add_space(8.0);

        // ====================================================================
        // 4. TAB CONTENT
        // ====================================================================
        let tab_actions = match manager.active_tab {
            0 => render_plugin_database_tab(
                ui,
                manager,
                status_msg,
                stem_track_count,
                default_track,
                sandbox_status,
            ),
            1 => {
                render_scan_paths_tab(ui, manager, status_msg);
                PluginViewActions::default()
            }
            2 => {
                render_import_plugin_tab(ui, manager, status_msg);
                PluginViewActions::default()
            }
            3 => {
                render_fl_yabridge_assistant_tab(ui, manager, status_msg);
                PluginViewActions::default()
            }
            _ => PluginViewActions::default(),
        };
        if tab_actions.load_into_track.is_some() {
            actions.load_into_track = tab_actions.load_into_track;
        }
        if tab_actions.load_into_sandbox.is_some() {
            actions.load_into_sandbox = tab_actions.load_into_sandbox;
        }
        if tab_actions.load_preset_into_track.is_some() {
            actions.load_preset_into_track = tab_actions.load_preset_into_track;
        }
        if tab_actions.remove_track.is_some() {
            actions.remove_track = tab_actions.remove_track;
        }
        if tab_actions.sandbox_inspect.is_some() {
            actions.sandbox_inspect = tab_actions.sandbox_inspect;
        }
        actions
    })
    .inner
}

/// Lists the plugins currently instantiated per stem track, with open/close GUI
/// and remove buttons. Empty when no plugin is loaded anywhere.
fn render_active_inserts(
    ui: &mut Ui,
    active_plugins: &[Option<String>],
    gui_open: &[bool],
    actions: &mut PluginViewActions,
) {
    let any = active_plugins.iter().any(|p| p.is_some());
    if !any {
        return;
    }
    ui.group(|ui| {
        ui.label(
            egui::RichText::new(crate::i18n::t("🎛 Aktiva plugin-inserts per spår"))
                .strong()
                .size(12.0)
                .color(Theme::FL_CYAN),
        );
        for (idx, slot) in active_plugins.iter().enumerate() {
            let Some(name) = slot else {
                continue;
            };
            let is_open = gui_open.get(idx).copied().unwrap_or(false);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::tstatus!("Spår {}", idx + 1))
                        .strong()
                        .size(10.5)
                        .color(Theme::TEXT_BRIGHT),
                );
                ui.label(egui::RichText::new(name).size(10.5).color(Theme::FL_GREEN));
                if is_open {
                    ui.label(
                        egui::RichText::new(crate::i18n::t("🪟 GUI öppet"))
                            .size(9.5)
                            .color(Theme::FL_YELLOW),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(crate::i18n::t("🗑 Ta bort")).clicked() {
                        actions.remove_track = Some(idx);
                    }
                    let gui_button = if is_open {
                        crate::i18n::t("🪟 Stäng GUI")
                    } else {
                        crate::i18n::t("🪟 Öppna GUI")
                    };
                    if ui
                        .button(gui_button)
                        .on_hover_text(crate::i18n::t(
                            "Visar pluginens eget gränssnitt i ett X11-fönster – samma instans som processar ljudet.",
                        ))
                        .clicked()
                    {
                        if is_open {
                            actions.close_gui = Some(idx);
                        } else {
                            actions.open_gui = Some(idx);
                        }
                    }
                });
            });
        }
    });
}

// ============================================================================
// TAB 0: PLUGIN DATABASE & LIBRARY
// ============================================================================
fn render_plugin_database_tab(
    ui: &mut Ui,
    manager: &mut PluginManager,
    status_msg: &mut String,
    stem_track_count: usize,
    default_track: usize,
    sandbox_status: Option<&str>,
) -> PluginViewActions {
    // Search & Filter controls
    ui.horizontal(|ui| {
        ui.label(crate::i18n::t("🔍 Sök plugin:"));
        ui.add(
            egui::TextEdit::singleline(&mut manager.search_query)
                .hint_text(crate::i18n::t("Sök på plugin-namn, tillverkare (t.ex. Sytrus, Gross Beat, Serum, FabFilter, Vital)..."))
                .desired_width(320.0),
        );
        if !manager.search_query.is_empty() && ui.button(crate::i18n::t("✕")).clicked() {
            manager.search_query.clear();
        }

        ui.separator();

        ui.label(crate::i18n::t("Format-filter:"));
        if ui.selectable_label(manager.selected_format_filter.is_none(), crate::i18n::t("Alla")).clicked() {
            manager.selected_format_filter = None;
        }
        if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::FlStudioNative), "🔥 FL Studio Native").clicked() {
            manager.selected_format_filter = Some(PluginFormat::FlStudioNative);
        }
        if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::WineYabridge), "🍷 Windows (Yabridge)").clicked() {
            manager.selected_format_filter = Some(PluginFormat::WineYabridge);
        }
        if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Vst3), "VST3 Native").clicked() {
            manager.selected_format_filter = Some(PluginFormat::Vst3);
        }
        if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Clap), "CLAP").clicked() {
            manager.selected_format_filter = Some(PluginFormat::Clap);
        }
        if ui.selectable_label(manager.selected_format_filter == Some(PluginFormat::Lv2), "LV2").clicked() {
            manager.selected_format_filter = Some(PluginFormat::Lv2);
        }
    });

    ui.add_space(4.0);

    // Category Filter Chips
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("Kategori:")).size(10.5).color(Theme::TEXT_MUTED));
        let categories = [
            (None, "Alla kategorier"),
            (Some(PluginCategory::Synth), "🎹 Synthesizer"),
            (Some(PluginCategory::Effect), "✨ Effekt"),
            (Some(PluginCategory::Equalizer), "📊 EQ & Filter"),
            (Some(PluginCategory::Compressor), "🗜 Kompressor"),
            (Some(PluginCategory::Reverb), "🌌 Reverb"),
            (Some(PluginCategory::Delay), "⏳ Delay"),
            (Some(PluginCategory::Distortion), "🔥 Distortion"),
            (Some(PluginCategory::PitchCorrection), "🎤 Pitch/Vocoder"),
            (Some(PluginCategory::Mastering), "🛡 Mastering"),
        ];

        for (cat_opt, label) in categories {
            let is_sel = manager.selected_category_filter == cat_opt;
            let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(26, 32, 44) };
            let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(label)).size(10.0).color(fg)).fill(bg)).clicked() {
                manager.selected_category_filter = cat_opt;
            }
        }
    });

    let mut actions = PluginViewActions::default();
    if let Some(status) = sandbox_status {
        ui.group(|ui| {
            ui.label(
                egui::RichText::new(status)
                    .size(10.0)
                    .color(Theme::FL_GREEN),
            );
        });
    }
    let inspection_actions =
        render_inspection_panel(ui, manager, status_msg, stem_track_count, default_track);
    actions.load_into_track = inspection_actions.load_into_track;
    actions.load_into_sandbox = inspection_actions.load_into_sandbox;
    actions.load_preset_into_track = inspection_actions.load_preset_into_track;

    ui.add_space(6.0);

    // Plugins List
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        let mut visible_count = 0;

        for plugin in manager.plugins.iter_mut() {
            // Apply format filter
            if let Some(fmt) = manager.selected_format_filter
                && plugin.format != fmt {
                    continue;
                }

            // Apply category filter
            if let Some(cat) = manager.selected_category_filter
                && plugin.category != cat {
                    continue;
                }

            // Apply search query
            if !manager.search_query.is_empty() {
                let q = manager.search_query.to_lowercase();
                if !plugin.name.to_lowercase().contains(&q)
                    && !plugin.vendor.to_lowercase().contains(&q)
                    && !plugin.author_notes.to_lowercase().contains(&q)
                    && !plugin.file_path.to_lowercase().contains(&q)
                {
                    continue;
                }
            }

            visible_count += 1;
            let badge_color = plugin.format.badge_color();

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // 1. Format Badge
                    let (b_rect, _) = ui.allocate_exact_size(Vec2::new(130.0, 24.0), egui::Sense::hover());
                    ui.painter().rect_filled(b_rect, Rounding::same(4.0), Color32::from_rgb(20, 26, 38));
                    ui.painter().rect_stroke(b_rect, Rounding::same(4.0), Stroke::new(1.2_f32, badge_color));
                    ui.painter().text(
                        b_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        plugin.format.name(),
                        egui::FontId::proportional(9.5),
                        badge_color,
                    );

                    ui.add_space(6.0);

                    // 2. Plugin Info & Details
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&plugin.name).strong().size(12.5).color(Color32::WHITE));
                            ui.label(egui::RichText::new(&plugin.version).size(10.0).color(Theme::TEXT_MUTED));
                            ui.label(
                                egui::RichText::new(format!("{} {}", plugin.category.icon(), plugin.category.label()))
                                    .size(10.0)
                                    .color(Theme::FL_ORANGE),
                            );
                        });

                        ui.label(
                            egui::RichText::new(crate::tstatus!("Tillverkare: {}  •  Sökväg: {}", plugin.vendor, plugin.file_path))
                                .size(10.0)
                                .color(Theme::TEXT_MUTED),
                        );

                        if !plugin.author_notes.is_empty() {
                            ui.label(egui::RichText::new(&plugin.author_notes).size(9.5).color(Color32::from_rgb(170, 190, 210)));
                        }
                    });

                    // 3. Right side: Action buttons & Stats
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if plugin.format == PluginFormat::Clap {
                            let host_available = crate::audio::plugin_host_live::is_available();
                            let can_load = plugin.verified && host_available;
                            let resp = ui.add_enabled(
                                can_load,
                                egui::Button::new(crate::i18n::t("🔎 Ladda & inspektera")),
                            );
                            let resp = if !host_available {
                                resp.on_disabled_hover_text(crate::i18n::t(
                                    "Bygg med --features plugin-host för att ladda CLAP-plugins.",
                                ))
                            } else if !plugin.verified {
                                resp.on_disabled_hover_text(crate::i18n::t(
                                    "Filen verifierades inte som en giltig binär.",
                                ))
                            } else {
                                resp
                            };
                            if resp.clicked() {
                                let snapshot = crate::audio::plugin_host_live::inspect(&plugin.file_path);
                                *status_msg = match &snapshot.error {
                                    Some(err) => crate::tstatus!("⚠ Kunde inte ladda plugin: {}", err),
                                    None => crate::tstatus!(
                                        "🔬 Läste {} parametrar från '{}'",
                                        snapshot.parameters.len(),
                                        snapshot.info.as_ref().map(|i| i.name.as_str()).unwrap_or(&snapshot.path)
                                    ),
                                };
                                manager.inspection = Some(snapshot);
                            }

                            let sandbox_resp = ui
                                .add_enabled(
                                    can_load,
                                    egui::Button::new(crate::i18n::t("🧪 Sandbox-inspektera")),
                                )
                                .on_hover_text(crate::i18n::t(
                                    "Laddar pluginen i en separat process (Fas 4.5a) så att en krasch inte tar ner Sonix.",
                                ));
                            if sandbox_resp.clicked() {
                                actions.sandbox_inspect = Some(plugin.file_path.clone());
                            }
                        }

                        if ui.button(crate::i18n::t("📂 Visa i mapp")).clicked() {
                            let dir = std::path::Path::new(&plugin.file_path)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| plugin.file_path.clone());
                            let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
                            *status_msg = crate::tstatus!("📂 Öppnade mapp: {}", dir);
                        }

                        let (badge, color) = if plugin.verified {
                            (crate::i18n::t("✔ Verifierad"), Theme::FL_GREEN)
                        } else {
                            (crate::i18n::t("⚠ Ej verifierad"), Theme::FL_YELLOW)
                        };
                        ui.label(egui::RichText::new(badge).strong().size(10.0).color(color));

                        // Stats column (real file size, no fabricated CPU/latency)
                        ui.vertical(|ui| {
                            let kb = plugin.file_size_bytes as f64 / 1024.0;
                            let size_str = if kb > 1024.0 {
                                format!("{:.1} MB", kb / 1024.0)
                            } else {
                                format!("{:.0} KB", kb)
                            };
                            ui.label(egui::RichText::new(size_str).size(10.0).color(Theme::TEXT_MUTED));
                            if !plugin.verify_note.is_empty() {
                                ui.label(egui::RichText::new(&plugin.verify_note).size(9.0).color(Theme::TEXT_MUTED));
                            }
                        });

                        if plugin.verified {
                            ui.label(egui::RichText::new(crate::i18n::t("✔ Verifierad binär")).size(9.5).color(Theme::FL_GREEN));
                        }
                    });
                });
            });
            ui.add_space(2.0);
        }

        if visible_count == 0 {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Inga plugins matchade din sökning eller filter.")).color(Theme::TEXT_MUTED).size(13.0));
                if ui.button(crate::i18n::t("Återställ filter")).clicked() {
                    manager.search_query.clear();
                    manager.selected_format_filter = None;
                    manager.selected_category_filter = None;
                }
            });
        }
    });

    actions
}

// ============================================================================
// TAB 1: SCAN PATHS & FOLDERS MANAGER
// ============================================================================
fn render_scan_paths_tab(ui: &mut Ui, manager: &mut PluginManager, status_msg: &mut String) {
    ui.label(egui::RichText::new(crate::i18n::t("📁 SÖKVÄGAR FÖR PLUGIN-SKANNING")).strong().size(13.5).color(Theme::FL_YELLOW));
    ui.label(
        egui::RichText::new(crate::i18n::t("Sonix genomsöker följande mappar efter Linux-native VST3/CLAP/LV2 samt Windows/Wine & FL Studio VST-kataloger:"))
            .size(11.0)
            .color(Theme::TEXT_MUTED),
    );

    ui.add_space(8.0);

    // List of scan paths
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        let mut to_remove = None;

        for (idx, scan_path) in manager.scan_paths.iter_mut().enumerate() {
            let expanded = if let Some(stripped) = scan_path.path.strip_prefix("~/") {
                std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join(stripped)).unwrap_or_else(|_| std::path::PathBuf::from(&scan_path.path))
            } else {
                std::path::PathBuf::from(&scan_path.path)
            };
            let exists_on_disk = expanded.exists();

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut scan_path.enabled, "");

                    // Existence indicator
                    if exists_on_disk {
                        ui.label(egui::RichText::new(crate::i18n::t("🟢 Finns")).size(10.0).color(Theme::FL_GREEN));
                    } else {
                        ui.label(egui::RichText::new(crate::i18n::t("⚪ Ej skapad")).size(10.0).color(Theme::TEXT_MUTED));
                    }

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&scan_path.path).strong().size(11.5).color(Color32::WHITE));
                        ui.label(
                            egui::RichText::new(format!(
                                "{} • {}",
                                scan_path.description,
                                if scan_path.is_fl_path { crate::i18n::t("FL Studio Specifik") } else if scan_path.is_wine { crate::i18n::t("Wine/Windows") } else { crate::i18n::t("Native Linux") }
                            ))
                            .size(9.5)
                            .color(if scan_path.is_fl_path { Theme::FL_ORANGE } else { Theme::TEXT_MUTED }),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(crate::i18n::t("🗑 Ta bort")).clicked() {
                            to_remove = Some(idx);
                        }
                    });
                });
            });
            ui.add_space(2.0);
        }

        if let Some(idx) = to_remove {
            manager.remove_scan_path(idx);
            *status_msg = crate::i18n::t("Tog bort sökväg från skanningslistan.").to_string();
        }
    });

    ui.add_space(10.0);

    // Add new custom scan path section
    ui.group(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("➕ Lägg till anpassad plugin-mapp (t.ex. extern hårddisk eller FL Studio-installation)")).strong().size(12.0).color(Theme::FL_CYAN));
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("Mappsökväg:"));
            ui.add(
                egui::TextEdit::singleline(&mut manager.new_custom_path_input)
                    .hint_text(crate::i18n::t("/media/user/Plugins eller ~/.wine/drive_c/..."))
                    .desired_width(340.0),
            );

            if ui.button(crate::i18n::t("➕ Lägg till mapp")).clicked() {
                if !manager.new_custom_path_input.trim().is_empty() {
                    let path = manager.new_custom_path_input.trim().to_string();
                    let is_wine = path.contains(".wine") || path.contains("drive_c");
                    let is_fl = path.to_lowercase().contains("fl studio") || path.to_lowercase().contains("image-line");
                    manager.add_scan_path(path, crate::i18n::t("Anpassad plugin-mapp").to_string(), is_wine, is_fl);
                    manager.new_custom_path_input.clear();
                    *status_msg = crate::i18n::t("✔ Lade till ny plugin-mapp. Klicka 'Skanna mappar nu' för att uppdatera.").to_string();
                }
            }
        });
    });
}

// ============================================================================
// TAB 2: MANUAL IMPORT OF PLUGINS & .FST PRESETS
// ============================================================================
fn render_import_plugin_tab(ui: &mut Ui, manager: &mut PluginManager, status_msg: &mut String) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("📥 MANUELL PLUGIN- OCH PRESET-IMPORT")).strong().size(13.5).color(Theme::FL_CYAN));
            ui.label(
                egui::RichText::new(crate::i18n::t("Importera fristående filer (.vst3, .clap, .dll, .so, .lv2) eller FL Studio Preset-filer (.fst) direkt:"))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
        });
    });

    ui.add_space(8.0);

    ui.group(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Filsökväg:"));
                ui.add(
                    egui::TextEdit::singleline(&mut manager.manual_import_file_input)
                        .hint_text(crate::i18n::t("/sökväg/till/plugin.vst3 eller ~/.wine/.../Sytrus.dll eller preset.fst"))
                        .desired_width(380.0),
                );
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Tillverkare / Vendor (valfritt):"));
                ui.add(
                    egui::TextEdit::singleline(&mut manager.manual_import_vendor_input)
                        .hint_text(crate::i18n::t("t.ex. Image-Line, FabFilter, Xfer Records, u-he..."))
                        .desired_width(220.0),
                );

                ui.separator();

                ui.label(crate::i18n::t("Kategori:"));
                let categories = [
                    (PluginCategory::Synth, "🎹 Synthesizer"),
                    (PluginCategory::Effect, "✨ Effekt"),
                    (PluginCategory::Equalizer, "📊 Equalizer"),
                    (PluginCategory::Compressor, "🗜 Kompressor"),
                    (PluginCategory::Reverb, "🌌 Reverb"),
                    (PluginCategory::Delay, "⏳ Delay"),
                    (PluginCategory::Distortion, "🔥 Distortion"),
                    (PluginCategory::PitchCorrection, "🎤 Pitch/Vocoder"),
                    (PluginCategory::Mastering, "🛡 Mastering"),
                ];

                let selected_cat = categories.get(manager.manual_import_category_idx).map(|c| c.0).unwrap_or(PluginCategory::Synth);
                egui::ComboBox::from_id_salt("manual_import_category_dropdown")
                    .selected_text(categories.get(manager.manual_import_category_idx).map(|c| crate::i18n::t(c.1)).unwrap_or(crate::i18n::t("Välj")))
                    .show_ui(ui, |ui| {
                        for (idx, &(_cat_val, cat_label)) in categories.iter().enumerate() {
                            if ui.selectable_label(manager.manual_import_category_idx == idx, crate::i18n::t(cat_label)).clicked() {
                                manager.manual_import_category_idx = idx;
                            }
                        }
                    });

                if ui.add(
                    egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Importera Fil Nu")).strong().color(Color32::BLACK))
                        .fill(Theme::FL_GREEN)
                        .min_size(Vec2::new(140.0, 26.0)),
                ).clicked() {
                    if !manager.manual_import_file_input.trim().is_empty() {
                        let path = manager.manual_import_file_input.clone();
                        let vendor = manager.manual_import_vendor_input.clone();
                        match manager.import_file(&path, &vendor, selected_cat) {
                            Ok(desc) => {
                                *status_msg = crate::tstatus!("✔ Importerade framgångsrikt '{}' som {}!", desc.name, desc.format.name());
                                manager.manual_import_file_input.clear();
                                manager.manual_import_vendor_input.clear();
                            }
                            Err(msg) => {
                                *status_msg = msg;
                            }
                        }
                    } else {
                        *status_msg = crate::i18n::t("Ange en giltig filsökväg först.").to_string();
                    }
                }
            });
        });
    });

    ui.add_space(10.0);

    // FL Studio .FST Presets Library
    ui.label(egui::RichText::new(crate::i18n::t("📄 IMPORTERADE FL STUDIO PRESETS (.FST)")).strong().size(12.5).color(Theme::FL_ORANGE));
    ui.label(
        egui::RichText::new(crate::i18n::t("Katalogiserade .fst-filer (endast metadata – FL Studios stängda .fst-format kan inte avkodas, så preset-datan visas men kan inte tillämpas):"))
            .size(10.5)
            .color(Theme::TEXT_MUTED),
    );

    ui.add_space(4.0);

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for preset in &manager.presets {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📄")).size(14.0));

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&preset.preset_name).strong().size(11.5).color(Theme::TEXT_BRIGHT));
                        ui.label(
                            egui::RichText::new(crate::tstatus!("Mål: {}  •  Typ: {}  •  {}", preset.target_plugin, preset.preset_type, preset.file_path))
                                .size(9.5)
                                .color(Theme::TEXT_MUTED),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled(false, egui::Button::new(crate::i18n::t("⚡ Tillämpa Preset")))
                            .on_disabled_hover_text(crate::i18n::t("Kan inte tillämpas: .fst-presetdata kan inte avkodas. Plugins (CLAP/VST2/VST3) kan däremot laddas i plugin-panelen."));
                        ui.label(egui::RichText::new(format!("{} KB", preset.filesize_bytes / 1024)).size(9.5).color(Theme::TEXT_MUTED));
                    });
                });
            });
            ui.add_space(2.0);
        }
    });
}

// ============================================================================
// TAB 3: FL STUDIO & YABRIDGE SETUP ASSISTANT
// ============================================================================
fn render_fl_yabridge_assistant_tab(ui: &mut Ui, manager: &mut PluginManager, status_msg: &mut String) {
    ui.label(egui::RichText::new(crate::i18n::t("🍷 FL STUDIO & WINDOWS VST BRYGG-ASSISTENT")).strong().size(14.0).color(Theme::FL_PURPLE));
    ui.label(
        egui::RichText::new(crate::i18n::t("Hur du kör Image-Line plugins (Sytrus, Harmor, Gross Beat, FL Studio VSTi) och Windows VSTs i Sonix på Linux"))
            .size(11.0)
            .color(Theme::TEXT_MUTED),
    );

    ui.add_space(8.0);

    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        // Step 1: Image-Line FL Studio Plugins
        ui.group(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("1. Image-Line & FL Studio Inbyggda Plugins")).strong().size(12.5).color(Theme::FL_ORANGE));
            ui.label(
                egui::RichText::new(
                    crate::i18n::t("Image-Line har officiella VSTi/VST-versioner av Sytrus, Harmor, Gross Beat, Maximus, Vocodex och Edison.\n\
                     Dessa installeras i Windows/Wine-katalogen:\n\
                     ~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/")
                ).size(10.5).color(Theme::TEXT_BRIGHT)
            );
        });

        ui.add_space(6.0);

        // Step 2: FL Studio as VSTi
        ui.group(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("2. Köra hela FL Studio som ett instrument inuti Sonix (FL Studio VSTi)")).strong().size(12.5).color(Theme::FL_CYAN));
            ui.label(
                egui::RichText::new(
                    crate::i18n::t("Vägen dit går via FL Studio VSTi (.dll) körd genom Wine + yabridge. Sonix kan ladda och köra VST2/VST3/CLAP-bryggor (med PDC, state och GUI-fönster). Det som ännu inte verifierats här är en riktig yabridge-brygga med Wine + display.")
                ).size(10.5).color(Theme::TEXT_BRIGHT)
            );
        });

        ui.add_space(6.0);

        // Step 3: Yabridge CLI Sync
        ui.group(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("3. Synkronisera med Yabridge (Zero-Latency IPC)")).strong().size(12.5).color(Theme::FL_GREEN));
            ui.label(
                egui::RichText::new(
                    crate::i18n::t("För att generera Linux-native VST3/CLAP-broar för alla Windows-plugins, kör i din terminal:\n\
                     yabridgectl add \"$HOME/.wine/drive_c/Program Files/Common Files/VST3\"\n\
                     yabridgectl add \"$HOME/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST\"\n\
                     yabridgectl sync")
                ).size(10.5).color(Color32::from_rgb(180, 230, 180))
            );

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if manager.yabridge_installed {
                    ui.label(egui::RichText::new(crate::i18n::t("🟢 yabridgectl hittad i PATH")).size(10.5).color(Theme::FL_GREEN));
                } else {
                    ui.label(egui::RichText::new(crate::i18n::t("⚪ yabridgectl hittades inte – installera yabridge för Windows-plugins")).size(10.5).color(Theme::FL_YELLOW));
                }
                if ui.button(crate::i18n::t("⚡ Kör yabridgectl sync nu")).clicked() {
                    match crate::audio::plugin_host::run_yabridge_sync() {
                        Ok(out) => {
                            manager.rescan();
                            *status_msg = crate::tstatus!("✔ Yabridge-synk klar: {}", out);
                        }
                        Err(err) => {
                            *status_msg = crate::tstatus!("⚠ Yabridge-synk misslyckades: {}", err);
                        }
                    }
                }
            });
        });

        ui.add_space(6.0);

        // Step 4: Process-Isolering (Crash Sandboxing)
        ui.group(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("4. Process-Isolering & Kraschskydd")).strong().size(12.5).color(Theme::FL_YELLOW));
            ui.label(
                egui::RichText::new(
                    crate::i18n::t("Kraschskydd: plugins kan köras i en separat sandbox-process med minnesdelad ljudtransport. En kraschande plugin startas automatiskt om (max 3 gånger) utan att Sonix stängs – använd \"🧪 Sandbox-inspektera\" och \"🧪 Ladda in i sandbox\" i plugin-panelen.")
                ).size(10.5).color(Theme::TEXT_MUTED)
            );
        });
    });
}

// ============================================================================
// LIVE CLAP INSPECTION PANEL (Fas 4.1)
// ============================================================================
fn param_flags_label(p: &crate::audio::plugin_host_live::PluginParameter) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if p.is_automatable() {
        parts.push(crate::i18n::t("auto"));
    }
    if p.is_stepped() {
        parts.push(crate::i18n::t("steg"));
    }
    if p.is_modulatable() {
        parts.push(crate::i18n::t("mod"));
    }
    if p.is_bypass() {
        parts.push(crate::i18n::t("bypass"));
    }
    if p.is_readonly() {
        parts.push(crate::i18n::t("låst"));
    }
    if p.is_hidden() {
        parts.push(crate::i18n::t("dold"));
    }
    parts.join(", ")
}

fn render_inspection_panel(
    ui: &mut Ui,
    manager: &mut PluginManager,
    _status_msg: &mut String,
    stem_track_count: usize,
    default_track: usize,
) -> PluginViewActions {
    let mut actions = PluginViewActions::default();
    if manager.inspection.is_none() {
        return actions;
    }
    if manager.plugin_target_track == 0 && default_track > 0 {
        manager.plugin_target_track = default_track;
    }

    let mut close = false;
    ui.group(|ui| {
        let Some(snapshot) = manager.inspection.as_ref() else {
            return;
        };

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🔬 Inspekterad plugin")).strong().size(12.5).color(Theme::FL_CYAN));
            ui.label(egui::RichText::new(&snapshot.backend).size(10.5).color(Theme::FL_GREEN));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(crate::i18n::t("✕ Stäng")).clicked() {
                    close = true;
                }
                ui.label(egui::RichText::new(&snapshot.path).size(9.5).color(Theme::TEXT_MUTED));
            });
        });

        if let Some(err) = &snapshot.error {
            ui.label(egui::RichText::new(format!("⚠ {}", err)).size(10.5).color(Theme::FL_ORANGE));
            return;
        }

        if let Some(info) = &snapshot.info {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&info.name).strong().size(12.0).color(Color32::WHITE));
                if !info.version.is_empty() {
                    ui.label(egui::RichText::new(&info.version).size(10.0).color(Theme::TEXT_MUTED));
                }
                if !info.vendor.is_empty() {
                    ui.label(egui::RichText::new(format!("• {}", info.vendor)).size(10.0).color(Theme::FL_ORANGE));
                }
            });
            if !info.id.is_empty() {
                ui.label(egui::RichText::new(format!("id: {}", info.id)).size(9.5).color(Theme::TEXT_MUTED));
            }
            if !info.description.is_empty() {
                ui.label(egui::RichText::new(&info.description).size(10.0).color(Theme::TEXT_BRIGHT));
            }
            if !info.features.is_empty() {
                ui.label(
                    egui::RichText::new(crate::tstatus!("Egenskaper: {}", info.features.join(", ")))
                        .size(9.5)
                        .color(Theme::TEXT_MUTED),
                );
            }
        }

        // GUI capability reported by `clap.gui` (Fas 4.4a).
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t("🖼 Plugin-GUI:"))
                    .strong()
                    .size(10.5)
                    .color(Theme::FL_CYAN),
            );
            match &snapshot.gui {
                Some(gui) => {
                    let resize = if gui.can_resize {
                        crate::i18n::t("kan ändra storlek")
                    } else {
                        crate::i18n::t("fast storlek")
                    };
                    ui.label(
                        egui::RichText::new(crate::tstatus!(
                            "🖼 GUI: {} {}×{} ({}) – öppnas i ett X11-fönster från plugin-hanteraren",
                            gui.api,
                            gui.width,
                            gui.height,
                            resize
                        ))
                        .size(10.0)
                        .color(Theme::TEXT_BRIGHT),
                    );
                }
                None => {
                    ui.label(
                        egui::RichText::new(crate::i18n::t("stöds inte (ingen clap.gui)"))
                            .size(10.0)
                            .color(Theme::TEXT_MUTED),
                    );
                }
            }
        });

        // Instantiate this plugin into a stem track (Fas 4.2).
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t("🎛 Ladda in i stämspår:"))
                    .strong()
                    .size(10.5)
                    .color(Theme::FL_YELLOW),
            );
            if stem_track_count == 0 {
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Inga stämspår – separera eller importera ett spår först.",
                    ))
                    .size(10.0)
                    .color(Theme::TEXT_MUTED),
                );
            } else {
                let max_track = stem_track_count - 1;
                let target = manager.plugin_target_track.min(max_track);
                ui.add(
                    egui::DragValue::new(&mut manager.plugin_target_track)
                        .range(0..=max_track)
                        .speed(0.1),
                );
                ui.label(
                    egui::RichText::new(crate::tstatus!("spår {}", target + 1))
                        .size(10.0)
                        .color(Theme::TEXT_MUTED),
                );
                if ui
                    .button(crate::i18n::t("▶ Ladda in"))
                    .on_hover_text(crate::i18n::t(
                        "Instansierar en riktig CLAP-processor och sätter den som insert på spåret.",
                    ))
                    .clicked()
                {
                    actions.load_into_track = Some((snapshot.path.clone(), target));
                }
                if ui
                    .button(crate::i18n::t("🧪 Ladda in i sandbox"))
                    .on_hover_text(crate::i18n::t(
                        "Kör pluginen i en separat process och strömmar ljudet via delat minne.",
                    ))
                    .clicked()
                {
                    actions.load_into_sandbox = Some((snapshot.path.clone(), target));
                }
            }
        });

        // Load one of the plugin's own presets (clap.preset-load/2) while
        // instantiating it (Fas 4.3).
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t("🎚 Native preset (sökväg):"))
                    .size(10.5)
                    .color(Theme::FL_YELLOW),
            );
            ui.add(
                egui::TextEdit::singleline(&mut manager.plugin_preset_location)
                    .hint_text(crate::i18n::t("/sökväg/till/preset.clap-preset"))
                    .desired_width(240.0),
            );
            let preset_ok = !manager.plugin_preset_location.trim().is_empty() && stem_track_count > 0;
            let resp = ui.add_enabled(
                preset_ok,
                egui::Button::new(crate::i18n::t("▶ Ladda in med preset")),
            );
            if resp.clicked() {
                let max_track = stem_track_count - 1;
                let target = manager.plugin_target_track.min(max_track);
                actions.load_preset_into_track = Some((
                    snapshot.path.clone(),
                    target,
                    manager.plugin_preset_location.trim().to_string(),
                ));
            }
        });
        ui.label(
            egui::RichText::new(crate::i18n::t(
                "Laddar en ny instans av pluginen med preseten applicerad via clap.preset-load/2.",
            ))
            .size(9.5)
            .color(Theme::TEXT_MUTED),
        );

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(crate::tstatus!(
                "Parametrar ({}):",
                snapshot.parameters.len()
            ))
            .strong()
            .size(11.0)
            .color(Theme::FL_YELLOW),
        );

        if snapshot.parameters.is_empty() {
            ui.label(egui::RichText::new(crate::i18n::t("Inga parametrar rapporterade.")).size(10.0).color(Theme::TEXT_MUTED));
            return;
        }

        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            for p in &snapshot.parameters {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&p.name).strong().size(10.5).color(Theme::TEXT_BRIGHT));
                    if !p.module.is_empty() {
                        ui.label(egui::RichText::new(format!("[{}]", p.module)).size(9.5).color(Theme::TEXT_MUTED));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.3} … {:.3}  std {:.3}",
                            p.min_value,
                            p.max_value,
                            p.default_value
                        ))
                        .size(9.5)
                        .color(Theme::TEXT_MUTED),
                    );
                    let flags = param_flags_label(p);
                    if !flags.is_empty() {
                        ui.label(egui::RichText::new(flags).size(9.0).color(Theme::FL_CYAN));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("#{}", p.id)).size(9.0).color(Theme::TEXT_MUTED));
                    });
                });
            }
        });
    });

    if close {
        manager.inspection = None;
    }
    actions
}


