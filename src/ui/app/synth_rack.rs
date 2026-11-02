//! Syntracket: oscillator, filter, ADSR och graf.
//!
//! Utdelad ur `mixer.rs` 2026-11-02.

use super::*;
use crate::ui::theme::Theme;
use crate::audio::synth::HARMONIC_COUNT;

impl SonixApp {
pub(crate) fn render_synth_hardware_rack(&mut self, ui: &mut egui::Ui) {
    // Hela panelen håller sig inom det *synliga* området, även när racket ovanför är bredare.
    ui.set_max_width(ui.clip_rect().width().max(320.0));
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX HARDWARE SYNTHESIZER")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().cyan()));
            ui.separator();

            // Presets: **radbryts** — tolv knappar på rad gjorde hela panelen bredare än
            // fönstret, och då klipptes högerkolumnen (filtret) av kanten.
            ui.label(egui::RichText::new(crate::i18n::t("Presets:")).color(crate::ui::theme::palette().text_muted()));
            for p in Preset::ALL {
                let name = p.label();
                let is_active = self.current_preset == p;
                let fill = if is_active { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().panel() };
                let text_color = if is_active { Color32::BLACK } else { crate::ui::theme::palette().text() };
                if ui.add(egui::Button::new(egui::RichText::new(name).strong().color(text_color)).fill(fill)).clicked() {
                    self.current_preset = p;
                    self.apply_preset_settings(p);
                    let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                }
            }
        });

        ui.add_space(8.0);

        // **Panelen får aldrig växa utanför fönstret.** På 1280×800 räckte inte höjden: racket
        // åt 8 × 84 px och synten klipptes av nedtill. Nu rullar synten i stället, med sin höjd
        // tagen ur det som faktiskt finns kvar.
        let rack_height = (ui.available_height() - 46.0).max(150.0);
        egui::ScrollArea::vertical()
            .id_salt("synth_hardware_scroll")
            .max_height(rack_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
        // **Två kolumner, med högersidan staplad.** Oscillatorn är en hel kolumn hög; med fyra
        // kolumner blev den för smal och filterpanelen la sig över den. Nu får oscillatorn halva
        // bredden, och filter + ADSR + graf ligger i högerkolumnen ovanpå varandra — panelen blir
        // aldrig högre än oscillatorn.
        // **Responsiv: två kolumner bara när de får plats.** Vid 1280 klipptes filterkolumnen
        // av högerkanten; där staplas sektionerna i stället och scrollområdet tar höjden.
        // Bredden tas ur *synligt* område: i ett scrollområde ärver `available_width` rackets
        // fulla bredd, som är bredare än fönstret — därför mättes fel förut.
        // shortcut: under ~1100 px staplas sektionerna, men panelen delar höjd med rackets eget
        // scrollområde — då syns bara oscillatorn utan att rulla. Rätt lösning är att rackets
        // stegrutnät krymper först; bygg det när 1280 blir en skärm att stödja.
        let visible_w = ui.clip_rect().width().max(320.0);
        if visible_w < 1100.0 {
            synth_oscillator(self, ui);
            ui.add_space(6.0);
            synth_filter(self, ui);
            ui.add_space(6.0);
            synth_adsr(self, ui);
            ui.add_space(6.0);
            synth_graph(self, ui);
        } else {
            ui.columns(2, |cols| {
                synth_oscillator(self, &mut cols[0]);
                synth_filter(self, &mut cols[1]);
                synth_adsr(self, &mut cols[1]);
                synth_graph(self, &mut cols[1]);
            });
        }
    });
            });
}
}

/// Oscillatorsektionen: vågform, oktaver, unison och FM.
fn synth_oscillator(app: &mut SonixApp, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("OSCILLATOR")).strong());
        let waveforms = [
            (Waveform::Sine, crate::i18n::t("∿ Sinus")),
            (Waveform::Saw, crate::i18n::t("⩘ Sågtand")),
            (Waveform::Square, crate::i18n::t("⊓ Fyrkant")),
            (Waveform::Triangle, crate::i18n::t("⋀ Triangel")),
            (Waveform::Additive, crate::i18n::t("▥ Additiv")),
            (Waveform::Pluck, crate::i18n::t("𝄞 Sträng")),
            (Waveform::Bowed, crate::i18n::t("🎻 Båge")),
            (Waveform::Guitar, crate::i18n::t("🎸 Gitarr")),
            (Waveform::SixOp, crate::i18n::t("✱ 6-op")),
            (Waveform::Inharmonic, crate::i18n::t("⛭ Metall")),
        ];
        for (wf, name) in waveforms {
            let is_wf = app.waveform == wf;
            if ui.selectable_label(is_wf, name).clicked() {
                app.waveform = wf;
                let _ = app.engine.send_command(AudioCommand::SetWaveform(wf));
            }
        }
        if app.waveform == Waveform::SixOp {
            // Panelen är för hög och för bred för oscillatorkolumnen (36 matrisceller
            // + 12 rattar), så den öppnas i ett eget fönster. Inline blev både
            // matrisen och enveloperna avklippta.
            ui.separator();
            if ui
                .button(crate::i18n::t("⚙ 6-op-matris…"))
                .on_hover_text(crate::i18n::t(
                    "Operatorer, enveloper och 6×6-matrisen i ett eget fönster",
                ))
                .clicked()
            {
                app.show_six_op_window = true;
            }
            let mut open = app.show_six_op_window;
            let mut six_changed = false;
            egui::Window::new(crate::i18n::t("6-OP (SYTRUS)"))
                .open(&mut open)
                .resizable(true)
                .default_width(420.0)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .button(crate::i18n::t("∿ Sinus"))
                            .on_hover_text(crate::i18n::t("Nollställ matrisen"))
                            .clicked()
                        {
                            app.six_op = crate::audio::synth::SixOpParams::default();
                            six_changed = true;
                        }
                        if ui
                            .button(crate::i18n::t("⛓ Kedja"))
                            .on_hover_text(crate::i18n::t(
                                "Operator 1→2→3→4→5→6; bara den sista hörs",
                            ))
                            .clicked()
                        {
                            app.six_op = crate::audio::synth::SixOpParams::chain(1.2);
                            six_changed = true;
                        }
                        if ui
                            .button(crate::i18n::t("↺ Sågtand"))
                            .on_hover_text(crate::i18n::t(
                                "Operator 1 återkopplar till sig själv",
                            ))
                            .clicked()
                        {
                            let mut params = crate::audio::synth::SixOpParams::default();
                            params.matrix[0][0] = 1.5;
                            app.six_op = params;
                            six_changed = true;
                        }
                    });
                    ui.separator();
                    // Förhållande och nivå: en rad per operator, så ratten och dess
                    // betydelse står på samma rad i stället för i två glesa rader.
                    egui::Grid::new("six_op_ops").num_columns(3).show(ui, |ui| {
                        for i in 0..crate::audio::synth::SIX_OP_COUNT {
                            ui.label(format!("OP{}", i + 1));
                            six_changed |= ui
                                .add(
                                    egui::DragValue::new(&mut app.six_op.ratios[i])
                                        .range(0.0..=32.0)
                                        .speed(0.01)
                                        .max_decimals(2),
                                )
                                .on_hover_text(crate::i18n::t("Frekvensförhållande"))
                                .changed();
                            six_changed |= ui
                                .add(
                                    egui::DragValue::new(&mut app.six_op.levels[i])
                                        .range(0.0..=1.0)
                                        .speed(0.01)
                                        .max_decimals(2),
                                )
                                .on_hover_text(crate::i18n::t(
                                    "Utgångsnivå — 0 hörs inte men kan modulera",
                                ))
                                .changed();
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    let mut target = app.six_env_target.min(crate::audio::synth::SIX_OP_COUNT - 1);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("ENVELOP"))
                                .size(Theme::FONT_XS),
                        );
                        for i in 0..crate::audio::synth::SIX_OP_COUNT {
                            if ui
                                .selectable_label(target == i, format!("OP{}", i + 1))
                                .clicked()
                            {
                                target = i;
                            }
                        }
                    });
                    app.six_env_target = target;
                    let env = &mut app.six_op.envs[target];
                    ui.horizontal(|ui| {
                        for (label, value, range) in [
                            ("A", &mut env.attack, 0.0..=4.0),
                            ("D", &mut env.decay, 0.0..=4.0),
                            ("S", &mut env.sustain, 0.0..=1.0),
                            ("R", &mut env.release, 0.0..=4.0),
                        ] {
                            six_changed |= ui
                                .add(egui::Slider::new(value, range).text(label))
                                .changed();
                        }
                    });
                    // Tonhöjdsenvelop: samma ADSR formar tonhöjden, i halvtoner.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("PIT"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut app.six_op.pitch_amount[target], -24.0..=24.0)
                                    .suffix(" st")
                                    .text(crate::i18n::t("halvtoner")),
                            )
                            .on_hover_text(crate::i18n::t(
                                "Operadens tonhöjd följer dess envelop — 0 = still",
                            ))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("F-ENV"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut app.six_op.filter_env[target], -4.0..=4.0)
                                    .suffix(" oktaver")
                                    .text(crate::i18n::t("cutoff")),
                            )
                            .on_hover_text(crate::i18n::t(
                                "Operadens envelop öppnar dess filter — 0 = orörd",
                            ))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("KEY"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(egui::Slider::new(&mut app.six_op.key_track[target], 0.0..=1.0))
                            .on_hover_text(crate::i18n::t(
                                "Filtret följer tonhöjden — 1 = en oktav per oktav",
                            ))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("CRV"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(egui::Slider::new(&mut app.six_op.env_curve[target], -2.0..=2.0))
                            .on_hover_text(crate::i18n::t(
                                "Envelopens form — plus = snabb start, minus = mjuk",
                            ))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("PAN")
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(egui::Slider::new(&mut app.six_op.pan[target], -1.0..=1.0))
                            .on_hover_text(crate::i18n::t(
                                "Operadens plats i stereobilden — 0 = mitten",
                            ))
                            .changed();
                    });
                    // **Fri tidskurva:** åtta punkter över valfri tid. 0 s = ADSR.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("KURVA"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::DragValue::new(&mut app.six_op.shape_secs[target])
                                    .range(0.0..=10.0)
                                    .speed(0.05)
                                    .suffix(" s"),
                            )
                            .on_hover_text(crate::i18n::t(
                                "0 s = ADSR. Annars läses de åtta punkterna över den här tiden",
                            ))
                            .changed();
                        for point in 0..crate::audio::synth::six_op::SHAPE_POINTS {
                            six_changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut app.six_op.shape[target][point],
                                        0.0..=1.0,
                                    )
                                    .vertical(),
                                )
                                .changed();
                        }
                    });
                    // **Egen filterkurva:** fyra band, 1,0 = neutralt. Frekvensen står i
                    // verktygstipset — fyra etiketter sprängde panelens bredd.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("FILTERKURVA"))
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        for band in 0..crate::audio::synth::six_op::CURVE_BANDS {
                            six_changed |= ui
                                .add_sized(
                                    [58.0, 18.0],
                                    egui::Slider::new(
                                        &mut app.six_op.curve_gain[band],
                                        0.0..=4.0,
                                    ),
                                )
                                .on_hover_text(format!(
                                    "{} Hz",
                                    crate::audio::synth::six_op::CURVE_HZ[band]
                                ))
                                .changed();
                        }
                    });
                    let filter = &mut app.six_op.filters[target];
                    ui.horizontal(|ui| {
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut filter.cutoff, 20.0..=20_000.0)
                                    .logarithmic(true)
                                    .text("LP"),
                            )
                            .on_hover_text(crate::i18n::t("Lågpass (Hz), 20 000 = av"))
                            .changed();
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut filter.resonance, 0.1..=10.0)
                                    .text("Q"),
                            )
                            .on_hover_text(crate::i18n::t("Resonans"))
                            .changed();
                        // Filtertyp per operator. LP är standard; de andra tre är
                        // samma beräkning, bara en annan utgång ur filtret.
                        for (mode, label) in
                            [(0u8, "LP"), (1, "BP"), (2, "HP"), (3, "NO")]
                        {
                            six_changed |= ui
                                .selectable_label(filter.mode == mode, label)
                                .clicked()
                                && {
                                    filter.mode = mode;
                                    true
                                };
                        }
                        // Branthet: 12 eller 24 dB/oktav. 24 dB är samma filter en
                        // gång till, med eget tillstånd.
                        for (slope, label) in [(1u8, "12"), (2, "24")] {
                            six_changed |= ui
                                .selectable_label(
                                    app.six_op.filter_slope[target] == slope,
                                    label,
                                )
                                .on_hover_text(crate::i18n::t("dB per oktav"))
                                .clicked()
                                && {
                                    app.six_op.filter_slope[target] = slope;
                                    true
                                };
                        }
                    });
                    ui.separator();
                    // **LFO:** hastighet, djup och mål. Djup 0 = av, alltså påverkar
                    // reglaget ingenting förrän man drar i det.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("UNI")
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::DragValue::new(&mut app.six_op.unison)
                                    .range(1..=4)
                                    .speed(0.05),
                            )
                            .changed();
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut app.six_op.detune, 0.0..=50.0)
                                    .text("cent"),
                            )
                            .changed();
                        ui.label(
                            egui::RichText::new("LFO")
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut app.six_op.lfo_rate, 0.05..=40.0)
                                    .logarithmic(true)
                                    .text("Hz"),
                            )
                            .changed();
                        six_changed |= ui
                            .add(egui::Slider::new(&mut app.six_op.lfo_depth, 0.0..=1.0))
                            .changed();
                        for (target, label) in [(1u8, "PITCH"), (2, "CUT")] {
                            six_changed |= ui
                                .selectable_label(
                                    app.six_op.lfo_target == target,
                                    label,
                                )
                                .clicked()
                                && {
                                    app.six_op.lfo_target = if app.six_op.lfo_target
                                        == target
                                    {
                                        0
                                    } else {
                                        target
                                    };
                                    true
                                };
                        }
                    });
                    // **LFO 2** — egen hastighet och eget mål, så tonhöjd och cutoff kan
                    // röra sig olika fort (Sytrus har flera).
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("LFO 2")
                                .size(Theme::FONT_XS)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        six_changed |= ui
                            .add(
                                egui::Slider::new(&mut app.six_op.lfo2_rate, 0.05..=40.0)
                                    .logarithmic(true)
                                    .text("Hz"),
                            )
                            .changed();
                        six_changed |= ui
                            .add(egui::Slider::new(&mut app.six_op.lfo2_depth, 0.0..=1.0))
                            .changed();
                        for (target, label) in [(1u8, "PITCH"), (2, "CUT")] {
                            six_changed |= ui
                                .selectable_label(app.six_op.lfo2_target == target, label)
                                .clicked()
                                && {
                                    app.six_op.lfo2_target = if app.six_op.lfo2_target
                                        == target
                                    {
                                        0
                                    } else {
                                        target
                                    };
                                    true
                                };
                        }
                    });
                    ui.label(
                        egui::RichText::new(crate::i18n::t("MATRIS (källa → mål)"))
                            .size(Theme::FONT_XS)
                            .color(crate::ui::theme::palette().text_muted()),
                    );
                    // Sytrus matris har tre rader per cell (FM, RM, ADD). Samma rutnät
                    // redigerar en rad i taget — 108 rutor får inte plats.
                    ui.horizontal(|ui| {
                        for (mode, label) in
                            [(0u8, "FM"), (1, "RM"), (2, "ADD")]
                        {
                            if ui
                                .selectable_label(app.six_op_mode == mode, label)
                                .on_hover_text(crate::i18n::t(match mode {
                                    0 => "Fasmodulering (FM)",
                                    1 => "Ringmodulering: källan multiplicerar målets amplitud",
                                    _ => "Additiv blandning: källans ljud läggs till målets",
                                }))
                                .clicked()
                            {
                                app.six_op_mode = mode;
                            }
                        }
                    });
                    egui::Grid::new("six_op_matrix").num_columns(7).show(ui, |ui| {
                        ui.label("");
                        for target_col in 1..=crate::audio::synth::SIX_OP_COUNT {
                            ui.label(
                                egui::RichText::new(format!("→{target_col}"))
                                    .size(Theme::FONT_XS),
                            );
                        }
                        ui.end_row();
                        for source in 0..crate::audio::synth::SIX_OP_COUNT {
                            ui.label(format!("{}", source + 1));
                            for target_col in 0..crate::audio::synth::SIX_OP_COUNT {
                                // Samma ruta, men raden som redigeras väljs ovan.
                                let (value, range, speed) = match app.six_op_mode {
                                    0 => (
                                        &mut app.six_op.matrix[source][target_col],
                                        -16.0..=16.0,
                                        0.05,
                                    ),
                                    1 => (
                                        &mut app.six_op.ring[source][target_col],
                                        0.0..=1.0,
                                        0.01,
                                    ),
                                    _ => (
                                        &mut app.six_op.add[source][target_col],
                                        -1.0..=1.0,
                                        0.01,
                                    ),
                                };
                                six_changed |= ui
                                    .add_sized(
                                        [34.0, 16.0],
                                        egui::DragValue::new(value)
                                            .range(range)
                                            .speed(speed)
                                            .max_decimals(2),
                                    )
                                    .on_hover_text(crate::tstatus!(
                                        "OP{} → OP{}",
                                        source + 1,
                                        target_col + 1
                                    ))
                                    .changed();
                            }
                            ui.end_row();
                        }
                    });
                });
            app.show_six_op_window = open;
            if six_changed {
                let _ = app.engine.send_command(AudioCommand::SetSixOp {
                    params: app.six_op,
                });
            }
        }
        if app.waveform == Waveform::Additive {
            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("DRAGSTÄNGER (1–64)"))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                for i in 0..HARMONIC_COUNT {
                    ui.allocate_ui(egui::vec2(14.0, 44.0), |ui| {
                        ui.vertical(|ui| {
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut app.harmonics[i], 0.0..=1.0)
                                        .vertical()
                                        .show_value(false),
                                )
                                .changed();
                            ui.label(egui::RichText::new(format!("{}", i + 1)).size(Theme::FONT_XS));
                        });
                    });
                }
            });
            if changed {
                let _ = app
                    .engine
                    .send_command(AudioCommand::SetHarmonics { levels: app.harmonics });
            }
            // **Harmor: bild → spektrum.** Bildens nederkant blir grundtonen och
            // överkanten överton 16; knappen fyller dragstängerna, inget annat.
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(crate::i18n::t("🖼 Från bild…"))
                        .size(Theme::FONT_S),
                ))
                .on_hover_text(crate::i18n::t(
                    "Läs en PNG som spektrum — nedersta raden blir grundtonen",
                ))
                .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG", &["png"])
                    .pick_file()
            {
                match crate::audio::spectral_image::envelope_from_file(&path) {
                    Ok(env) => {
                        app.harmonics = env;
                        let _ = app.engine.send_command(AudioCommand::SetHarmonics {
                            levels: app.harmonics,
                        });
                        app.status_message = crate::tstatus!(
                            "🖼 Spektrum från '{}'",
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        );
                    }
                    Err(e) => {
                        app.status_message =
                            crate::tstatus!("⚠ Kunde inte läsa bilden: {}", e);
                    }
                }
            }
            // **Harmor: klang → spektrum.** Samma 64 nivåer som bilden och rattarna fyller, men
            // mätta ur den valda kanalens eget sampel — ingen ny motor, bara en annan källa.
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(crate::i18n::t("🔍 Analysera klang"))
                        .size(Theme::FONT_S),
                ))
                .on_hover_text(crate::i18n::t(
                    "Mät övertonerna i den valda kanalens sampel och fyll dragstängerna",
                ))
                .clicked()
            {
                let idx = app.selected_channel.min(app.channels.len().saturating_sub(1));
                match app.channels.get(idx).and_then(|c| c.pcm_audio.as_ref()) {
                    Some((left, _, rate)) => {
                        let note = app.channels[idx].sample_base_note;
                        let hz = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
                        let env = crate::audio::timbre::envelope_from_pcm(left, *rate as f32, hz);
                        app.harmonics = env;
                        let _ = app
                            .engine
                            .send_command(AudioCommand::SetHarmonics { levels: app.harmonics });
                        app.status_message = crate::tstatus!(
                            "🔍 Klang ur '{}' ({} Hz)",
                            app.channels[idx].name,
                            hz.round()
                        );
                    }
                    None => {
                        app.status_message =
                            crate::tstatus!("⚠ Kanal {} har inget sampel att analysera", idx + 1)
                    }
                }
            }
            ui.separator();
            let mut env_changed = ui
                .checkbox(
                    &mut app.partial_env_on,
                    crate::i18n::t("✦ Spektral envelop"),
                )
                .on_hover_text(crate::i18n::t(
                    "Låt varje partial ha sin egen attack och decay — höga partialer dör först",
                ))
                .changed();
            if app.partial_env_on {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Partial:"));
                    ui.add(
                        egui::DragValue::new(&mut app.partial_env_target)
                            .range(0..=HARMONIC_COUNT)
                            .speed(1.0),
                    )
                    .on_hover_text(crate::i18n::t(
                        "0 = reglagen gäller alla partialer, k = bara partial k",
                    ));
                });
                let mut env = app.edited_partial_env();
                let mut shaped = false;
                shaped |= ui
                    .add(
                        egui::Slider::new(&mut env[0], 0.0..=2.0)
                            .text(crate::i18n::t("Attack:"))
                            .suffix(" s"),
                    )
                    .changed();
                shaped |= ui
                    .add(
                        egui::Slider::new(&mut env[1], 0.0..=5.0)
                            .text(crate::i18n::t("Decay:"))
                            .suffix(" s"),
                    )
                    .changed();
                shaped |= ui
                    .add(
                        egui::Slider::new(&mut env[2], 0.0..=1.0)
                            .text(crate::i18n::t("Sustain:")),
                    )
                    .changed();
                if shaped {
                    app.store_partial_env(env);
                    env_changed = true;
                }
                // Lutningen hör till mallen, inte till en enskild partial.
                if app.partial_env_target == 0 {
                    env_changed |= ui
                        .add(
                            egui::Slider::new(&mut app.partial_env_tilt, 0.0..=2.0)
                                .text(crate::i18n::t("Lutning:")),
                        )
                        .on_hover_text(crate::i18n::t(
                            "0 = alla partialer dör lika fort, 1 = partial k dör k gånger snabbare",
                        ))
                        .changed();
                }
            }
            if env_changed {
                let _ = app.engine.send_command(AudioCommand::SetPartialEnvelopes {
                    envs: app.partial_envelopes(),
                });
            }
            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("SPEKTRALT FILTER"))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
            let mut formant_changed = false;
            formant_changed |= ui
                .add(
                    egui::Slider::new(&mut app.formant.center, 1.0..=32.0)
                        .text(crate::i18n::t("Formant:"))
                        .fixed_decimals(1),
                )
                .on_hover_text(crate::i18n::t(
                    "Vilken partial klangformen ligger på — 1 = grundtonen",
                ))
                .changed();
            formant_changed |= ui
                .add(
                    egui::Slider::new(&mut app.formant.width, 0.5..=16.0)
                        .text(crate::i18n::t("Bredd:"))
                        .fixed_decimals(1),
                )
                .changed();
            formant_changed |= ui
                .add(
                    egui::Slider::new(&mut app.formant.amount, 0.0..=1.0)
                        .text(crate::i18n::t("Mängd:")),
                )
                .on_hover_text(crate::i18n::t(
                    "0 = filtret av, 1 = bara bandet kvar",
                ))
                .changed();
            if formant_changed {
                let _ = app.engine.send_command(AudioCommand::SetSpectralFormant {
                    center: app.formant.center,
                    width: app.formant.width,
                    amount: app.formant.amount,
                });
            }
            ui.label(
                egui::RichText::new(crate::i18n::t("TIDSAXEL"))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
            let mut sweep_changed = false;
            sweep_changed |= ui
                .add(
                    egui::Slider::new(&mut app.sweep.to, 1.0..=32.0)
                        .text(crate::i18n::t("Till:"))
                        .fixed_decimals(1),
                )
                .on_hover_text(crate::i18n::t(
                    "Klangformen glider från Formant till det här partialnumret under noten",
                ))
                .changed();
            sweep_changed |= ui
                .add(
                    egui::Slider::new(&mut app.sweep.seconds, 0.0..=10.0)
                        .text(crate::i18n::t("Tid:"))
                        .suffix(" s"),
                )
                .on_hover_text(crate::i18n::t("Sekunder från anslaget — 0 = av"))
                .changed();
            if sweep_changed {
                let _ = app.engine.send_command(AudioCommand::SetSpectralSweep {
                    to: app.sweep.to,
                    seconds: app.sweep.seconds,
                });
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(crate::i18n::t("FM (DX10/Sytrus)")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        let mut fm_changed = false;
        ui.horizontal(|ui| {
            fm_changed |= rotary_knob(ui, &mut app.fm.ratio, 0.25, 8.0, "RATIO", crate::ui::theme::palette().cyan(), 34.0);
            fm_changed |= rotary_knob(ui, &mut app.fm.index, 0.0, 12.0, "INDEX", crate::ui::theme::palette().cyan(), 34.0);
        })
        .response
        .on_hover_text(crate::i18n::t("Modulatorns frekvens i förhållande till tonen (1 = samma, 2 = oktaven över) och fasdjupet i radianer. 0 = ren oscillator."));
        if fm_changed {
            let _ = app.engine.send_command(AudioCommand::SetFm(app.fm));
        }
        ui.separator();
        ui.label(
            egui::RichText::new(crate::i18n::t("UNISON"))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
        );
        let mut unison_changed = false;
        ui.horizontal(|ui| {
            let mut voices = app.unison_voices as i32;
            if ui
                .add(egui::Slider::new(&mut voices, 1..=8).show_value(true))
                .on_hover_text(crate::i18n::t("Antal röster per not"))
                .changed()
            {
                app.unison_voices = voices.clamp(1, 8) as u8;
                unison_changed = true;
            }
            ui.add(
                egui::Slider::new(&mut app.unison_detune_cents, 0.0..=100.0)
                    .suffix(" cent")
                    .show_value(true),
            )
            .on_hover_text(crate::i18n::t("Hur mycket rösterna stäms isär"));
        });
        if unison_changed {
            let _ = app.engine.send_command(AudioCommand::SetUnison {
                voices: app.unison_voices,
                detune_cents: app.unison_detune_cents,
            });
        }
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("Glide:"));
            if ui
                .add(
                    egui::Slider::new(&mut app.glide_time, 0.0..=2.0)
                        .suffix(" s")
                        .show_value(true),
                )
                .on_hover_text(crate::i18n::t(
                    "Tid för tonhöjden att glida till nästa not (0 = av)",
                ))
                .changed()
            {
                let _ = app
                    .engine
                    .send_command(AudioCommand::SetGlide { seconds: app.glide_time });
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Slider::new(&mut app.sub_level, 0.0..=1.0)
                        .text(crate::i18n::t("Sub:"))
                        .show_value(false),
                )
                .on_hover_text(crate::i18n::t(
                    "Suboscillator: en sinus en oktav under noten, efter filtret",
                ))
                .changed()
            {
                let _ = app
                    .engine
                    .send_command(AudioCommand::SetSub { level: app.sub_level });
            }
            let mut drive = app.filter_drive;
            let mut key_track = app.filter_key_track;
            let a = ui
                .add(
                    egui::Slider::new(&mut drive, 0.0..=1.0)
                        .text(crate::i18n::t("Drive:"))
                        .show_value(false),
                )
                .on_hover_text(crate::i18n::t(
                    "Hur mycket filtret klipper. 0 = rent, 1 = mjukt klippt",
                ));
            let b = ui
                .add(
                    egui::Slider::new(&mut key_track, 0.0..=1.0)
                        .text(crate::i18n::t("Key track:"))
                        .show_value(false),
                )
                .on_hover_text(crate::i18n::t(
                    "Hur mycket brytfrekvensen följer noten. 0 = fast filter",
                ));
            if a.changed() || b.changed() {
                app.filter_drive = drive;
                app.filter_key_track = key_track;
                let _ = app.engine.send_command(AudioCommand::SetFilterDrive {
                    drive,
                    key_track,
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("Oktav:"));
            if ui.button(crate::i18n::t("➖ (Z)")).clicked() && app.octave > 1 { app.octave -= 1; }
            ui.label(egui::RichText::new(format!("C{}", app.octave)).strong().color(crate::ui::theme::palette().cyan()));
            if ui.button(crate::i18n::t("➕ (X)")).clicked() && app.octave < 8 { app.octave += 1; }
        });
    });
}

/// Det resonanta SVF-filtret: cutoff, Q och envelopmängd.
fn synth_filter(app: &mut SonixApp, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("RESONANT SVF FILTER (12 dB)")).strong().color(crate::ui::theme::palette().orange()));
        ui.horizontal(|ui| {
            let mut flt_changed = false;
            let mut fenv_changed = false;
            flt_changed |= rotary_knob(ui, &mut app.filter.cutoff, 60.0, 18000.0, "CUTOFF", crate::ui::theme::palette().orange(), 40.0);
            flt_changed |= rotary_knob(ui, &mut app.filter.resonance, 0.5, 9.5, "RESO (Q)", crate::ui::theme::palette().yellow(), 40.0);
            fenv_changed |= rotary_knob(ui, &mut app.filter_env_amount, -6.0, 6.0, "ENV ±oct", crate::ui::theme::palette().orange(), 40.0);
            if flt_changed {
                let _ = app.engine.send_command(AudioCommand::SetFilter(app.filter));
            }
            if fenv_changed {
                let _ = app.engine.send_command(AudioCommand::SetFilterEnv {
                    amount: app.filter_env_amount,
                    adsr: app.filter_env,
                });
            }
        });
        // **Radbryts i stället för att trycka ut kolumnen.** Textslingan var bredare än
        // halva panelen och klipptes av kanten på en 1280-skärm.
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "Cutoff {:.0} Hz · Q {:.1} · Env {:+.1} oct",
                    app.filter.cutoff, app.filter.resonance, app.filter_env_amount
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
            )
            .wrap(),
        );
    });
}

/// ADSR-rattena.
fn synth_adsr(app: &mut SonixApp, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("ADSR ENVELOPE")).strong().color(crate::ui::theme::palette().purple()));
        let mut adsr_changed = false;
        ui.horizontal(|ui| {
            adsr_changed |= rotary_knob(ui, &mut app.adsr.attack, 0.002, 2.0, "ATTK", crate::ui::theme::palette().purple(), 32.0);
            adsr_changed |= rotary_knob(ui, &mut app.adsr.decay, 0.01, 3.0, "DECAY", crate::ui::theme::palette().purple(), 32.0);
            adsr_changed |= rotary_knob(ui, &mut app.adsr.sustain, 0.0, 1.0, "SUST", crate::ui::theme::palette().purple(), 32.0);
            adsr_changed |= rotary_knob(ui, &mut app.adsr.release, 0.01, 4.0, "REL", crate::ui::theme::palette().purple(), 32.0);
        });
        if adsr_changed {
            let _ = app.engine.send_command(AudioCommand::SetAdsr(app.adsr));
        }
    });
}

/// Envelopens graf.
fn synth_graph(app: &mut SonixApp, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t("ENVELOPE GRAF")).strong().color(crate::ui::theme::palette().green()));
        // Grafen var 130 px fast; i ett smalt fönster stack den ut ur sin kolumn och
        // klipptes av kanten. Nu tar den platsen som finns, upp till 130.
        let graph_w = ui.available_width().clamp(60.0, 130.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(graph_w, 68.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(3.0), crate::ui::theme::palette().bg_deep());
        painter.rect_stroke(rect, Rounding::same(3.0), Stroke::new(1.0_f32, crate::ui::theme::palette().panel_alt()));

        let p0 = Pos2::new(rect.min.x + 4.0, rect.max.y - 4.0);
        let p1 = Pos2::new(rect.min.x + 4.0 + (app.adsr.attack * 20.0).min(30.0), rect.min.y + 6.0);
        let p2 = Pos2::new(p1.x + (app.adsr.decay * 18.0).min(30.0), rect.max.y - 4.0 - (app.adsr.sustain * (rect.height() - 14.0)));
        let p3 = Pos2::new(p2.x + 25.0, p2.y);
        let p4 = Pos2::new((p3.x + (app.adsr.release * 16.0).min(35.0)).min(rect.max.x - 4.0), rect.max.y - 4.0);

        painter.line_segment([p0, p1], Stroke::new(2.0_f32, crate::ui::theme::palette().green()));
        painter.line_segment([p1, p2], Stroke::new(2.0_f32, crate::ui::theme::palette().green()));
        painter.line_segment([p2, p3], Stroke::new(2.0_f32, crate::ui::theme::palette().green()));
        painter.line_segment([p3, p4], Stroke::new(2.0_f32, crate::ui::theme::palette().green()));
    });
}

impl SonixApp {
    /// Konturerna som skickas till motorn. Partial k får `decay / (k+1)^lutning`, alltså
    /// dör höga partialer först; avstängd blir listan neutral och rösten går tabellvägen.
    /// Egna konturer ligger ovanpå mallen.
    pub(crate) fn partial_envelopes(&self) -> [[f32; 3]; HARMONIC_COUNT] {
        super::state::partial_envelopes_for(
            self.partial_env_on,
            self.partial_env,
            self.partial_env_tilt,
            &self.partial_env_overrides,
        )
    }

    /// Konturen reglagen visar: mallen, eller partialens egen (eller den den får ur mallen).
    fn edited_partial_env(&self) -> [f32; 3] {
        let target = self.partial_env_target;
        if target == 0 {
            return self.partial_env;
        }
        let k = target - 1;
        self.partial_envelopes()[k.min(HARMONIC_COUNT - 1)]
    }

    /// Sparar reglagens kontur på rätt ställe: mallen om ingen partial är vald, annars som
    /// undantag för den partialen.
    fn store_partial_env(&mut self, env: [f32; 3]) {
        if self.partial_env_target == 0 {
            self.partial_env = env;
        } else {
            let k = self.partial_env_target - 1;
            self.partial_env_overrides.retain(|(index, _)| *index != k);
            self.partial_env_overrides.push((k, env));
        }
    }
}
