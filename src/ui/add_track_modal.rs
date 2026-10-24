use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use super::theme::Theme;
use super::app::{PlaylistTrack, TrackKind};

#[derive(Clone, Debug, PartialEq)]
pub enum AddTrackCategory {
    Vocal,
    Keys,
    BassGuitar,
    Drums,
    Orchestra,
    Import,
}

#[derive(Clone, Debug)]
pub struct TrackTemplate {
    pub title: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    pub kind: TrackKind,
    pub color: Color32,
    pub category: AddTrackCategory,
    pub tag: &'static str,
    pub is_rec_arm_default: bool,
}

pub fn get_track_templates() -> Vec<TrackTemplate> {
    vec![
        // VOCALS & MICROPHONE
        TrackTemplate {
            title: "🎙 Lead Vocal (Sång)",
            desc: "Krispig och fyllig mikrofonsignal med studiokompressor och equalizer.",
            icon: "🎙",
            kind: TrackKind::VocalAudio,
            color: crate::ui::theme::palette().cyan(),
            category: AddTrackCategory::Vocal,
            tag: "Mikrofon",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎤 Rap / Trap Vocal",
            desc: "Tät, punchig röstinställning med de-esser och snabb gate.",
            icon: "🎤",
            kind: TrackKind::VocalAudio,
            color: crate::ui::theme::palette().track_rose(),
            category: AddTrackCategory::Vocal,
            tag: "Mikrofon",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎙 Stämmor & Backing Vocals",
            desc: "Bred stereobild med doubler och mjuk plate-reverb.",
            icon: "👥",
            kind: TrackKind::VocalAudio,
            color: crate::ui::theme::palette().purple(),
            category: AddTrackCategory::Vocal,
            tag: "Kör & Harmoni",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎙 Podcast & Tal",
            desc: "Ren och brusfri röstinspelning optimerad för podcasts och voiceovers.",
            icon: "📻",
            kind: TrackKind::VocalAudio,
            color: crate::ui::theme::palette().amber(),
            category: AddTrackCategory::Vocal,
            tag: "Tal / Broadcast",
            is_rec_arm_default: true,
        },

        // KEYBOARDS & SYNTHS
        TrackTemplate {
            title: "🎹 Grand Concert Piano",
            desc: "Akustisk flygel med djup dynamik och resonant reverbklang.",
            icon: "🎹",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().green(),
            category: AddTrackCategory::Keys,
            tag: "Akustiskt",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎹 Vintage Rhodes Electric",
            desc: "Varmt elpiano med mjuk tremolo och analog chorus.",
            icon: "✨",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().track_teal(),
            category: AddTrackCategory::Keys,
            tag: "Vintage",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎛 Analog Poly Synth Pad",
            desc: "Svävande varma synth-mattor och drömlika atmosfärer.",
            icon: "🌌",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().purple(),
            category: AddTrackCategory::Keys,
            tag: "Synth",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ Pluck & Arp Lead",
            desc: "Snabb, krispig synth-pluck perfekt för melodier och arpeggion.",
            icon: "⚡",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().yellow(),
            category: AddTrackCategory::Keys,
            tag: "Elektronisk",
            is_rec_arm_default: false,
        },

        // GUITAR & BASS
        TrackTemplate {
            title: "🎸 Clean Strat Electric",
            desc: "Klar elgitarr med rörförstärkarsimulering och mjuk delay.",
            icon: "🎸",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().red(),
            category: AddTrackCategory::BassGuitar,
            tag: "Gitarr",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎸 Akustisk Nylon Gitarr",
            desc: "Varm och fyllig fingerpicking-gitarr med studiorumsklang.",
            icon: "🪕",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().amber(),
            category: AddTrackCategory::BassGuitar,
            tag: "Akustiskt",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🔊 808 Sub Bass",
            desc: "Djup, mättad sub-bas som pumpar och skakar subwoofern.",
            icon: "🔊",
            kind: TrackKind::Bassline,
            color: crate::ui::theme::palette().track_rose(),
            category: AddTrackCategory::BassGuitar,
            tag: "Sub Bass",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ 303 Acid Bassline",
            desc: "Klassisk resonant bas med glide och distorsion.",
            icon: "🎛",
            kind: TrackKind::Bassline,
            color: crate::ui::theme::palette().orange(),
            category: AddTrackCategory::BassGuitar,
            tag: "Acid",
            is_rec_arm_default: false,
        },

        // DRUMS & BEATS
        TrackTemplate {
            title: "🥁 808 Trap Drum Kit",
            desc: "Tunga kicks, snärtiga snares, hi-hat rolls och 808 claps.",
            icon: "🥁",
            kind: TrackKind::Drums,
            color: crate::ui::theme::palette().cyan(),
            category: AddTrackCategory::Drums,
            tag: "Trap / Hip-Hop",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🥁 Studio Akustiska Trummor",
            desc: "Naturligt trumset med punchig bastrumma och dynamisk virvel.",
            icon: "🥢",
            kind: TrackKind::Drums,
            color: crate::ui::theme::palette().track_blue(),
            category: AddTrackCategory::Drums,
            tag: "Rock / Pop",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "☕ Lo-Fi Vinyl Beat Kit",
            desc: "Knastriga samplingar, dämpade kicks och vintage vinylklang.",
            icon: "📻",
            kind: TrackKind::Drums,
            color: crate::ui::theme::palette().amber(),
            category: AddTrackCategory::Drums,
            tag: "Lo-Fi",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ 909 Electronic Techno Kit",
            desc: "Tung analog kick, öppna hi-hats och industriella claps.",
            icon: "⚡",
            kind: TrackKind::Drums,
            color: crate::ui::theme::palette().red(),
            category: AddTrackCategory::Drums,
            tag: "Techno / House",
            is_rec_arm_default: false,
        },

        // ORCHESTRAL & STRINGS
        TrackTemplate {
            title: "🎻 Cinematic String Ensemble",
            desc: "Episka filmiska stråkar med full orkesterbredd.",
            icon: "🎻",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().cable(),
            category: AddTrackCategory::Orchestra,
            tag: "Cinematic",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎺 Brass Section",
            desc: "Mäktiga trumpeter, tromboner och valthorn för drops och refränger.",
            icon: "🎺",
            kind: TrackKind::SynthLead,
            color: crate::ui::theme::palette().yellow(),
            category: AddTrackCategory::Orchestra,
            tag: "Orkester",
            is_rec_arm_default: false,
        },

        // AUDIO / IMPORT
        TrackTemplate {
            title: "📁 Ljudfil / Sample Import",
            desc: "Importera egna WAV-, MP3-, FLAC- eller AIFF-ljudspår direkt i tidslinjen.",
            icon: "📁",
            kind: TrackKind::CustomAudio,
            color: crate::ui::theme::palette().orange(),
            category: AddTrackCategory::Import,
            tag: "WAV / MP3",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "📦 Suno AI Stems Spår",
            desc: "Separat ljudspår för Vocals, Drums, Bass eller Instrument från Suno AI.",
            icon: "📦",
            kind: TrackKind::CustomAudio,
            color: crate::ui::theme::palette().purple(),
            category: AddTrackCategory::Import,
            tag: "AI Stem",
            is_rec_arm_default: false,
        },
    ]
}

#[derive(Default)]
pub struct AddTrackModalState {
    pub selected_category: Option<AddTrackCategory>,
    pub search_filter: String,
}

pub fn render_add_track_modal(
    open: &mut bool,
    state: &mut AddTrackModalState,
    playlist_tracks: &mut Vec<PlaylistTrack>,
    status_message: &mut String,
    ctx: &egui::Context,
) -> Option<TrackTemplate> {
    if !*open {
        return None;
    }

    let mut close = false;
    let mut created_track = None;
    let mut import_request: Option<TrackTemplate> = None;

    egui::Window::new(crate::i18n::t("➕ Skapa nytt spår i Sonix Studio"))
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(720.0, 520.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.add_space(2.0);

            // Top Header and Search Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Välj instrument eller ljudkälla för ditt nya spår:")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().text()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🔍 Sök:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    ui.add_sized(Vec2::new(140.0, 20.0), egui::TextEdit::singleline(&mut state.search_filter).hint_text(crate::i18n::t("T.ex. flygel, 808...")));
                });
            });

            ui.add_space(6.0);

            // Category Filter Bar
            ui.horizontal_wrapped(|ui| {
                let categories = [
                    (None, "🌟 Alla"),
                    (Some(AddTrackCategory::Vocal), "🎙 Röst & Mic"),
                    (Some(AddTrackCategory::Keys), "🎹 Tangenter & Synth"),
                    (Some(AddTrackCategory::BassGuitar), "🎸 Gitarr & Bas"),
                    (Some(AddTrackCategory::Drums), "🥁 Trummor & Beats"),
                    (Some(AddTrackCategory::Orchestra), "🎻 Stråkar & Brass"),
                    (Some(AddTrackCategory::Import), "📁 Ljudfil / Stems"),
                ];

                for (cat_opt, cat_name) in categories {
                    let is_active = state.selected_category == cat_opt;
                    let fill = if is_active { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().panel() };
                    let fg = if is_active { Color32::BLACK } else { crate::ui::theme::palette().text() };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(cat_name)).strong().size(Theme::FONT_S).color(fg)).fill(fill)).clicked() {
                        state.selected_category = cat_opt;
                    }
                }
            });

            ui.separator();
            ui.add_space(4.0);

            // Template Cards Grid in Scroll Area
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                let templates = get_track_templates();
                let filter_lower = state.search_filter.to_lowercase();

                let filtered: Vec<&TrackTemplate> = templates.iter().filter(|t| {
                    if let Some(ref cat) = state.selected_category {
                        if t.category != *cat {
                            return false;
                        }
                    }
                    if !filter_lower.is_empty() {
                        let matches_title = t.title.to_lowercase().contains(&filter_lower);
                        let matches_desc = t.desc.to_lowercase().contains(&filter_lower);
                        let matches_tag = t.tag.to_lowercase().contains(&filter_lower);
                        if !matches_title && !matches_desc && !matches_tag {
                            return false;
                        }
                    }
                    true
                }).collect();

                egui::Grid::new("track_templates_grid").num_columns(2).spacing(Vec2::new(10.0, 10.0)).show(ui, |ui| {
                    for (idx, tmpl) in filtered.iter().enumerate() {
                        let (card_rect, card_resp) = ui.allocate_exact_size(Vec2::new(330.0, 72.0), Sense::click());
                        let is_hover = card_resp.hovered();

                        let bg = if is_hover { crate::ui::theme::palette().grid() } else { crate::ui::theme::palette().header() };
                        ui.painter().rect_filled(card_rect, Rounding::same(6.0), bg);
                        ui.painter().rect_stroke(card_rect, Rounding::same(6.0), Stroke::new(if is_hover { 1.5_f32 } else { 1.0_f32 }, tmpl.color));

                        // Left Accent
                        let left_bar = Rect::from_min_size(card_rect.min, Vec2::new(4.0, card_rect.height()));
                        ui.painter().rect_filled(left_bar, Rounding::same(2.0), tmpl.color);

                        // Icon and Title
                        ui.painter().text(
                            Pos2::new(card_rect.min.x + 14.0, card_rect.min.y + 18.0),
                            egui::Align2::LEFT_CENTER,
                            tmpl.icon,
                            egui::FontId::proportional(Theme::FONT_L),
                            tmpl.color,
                        );
                        ui.painter().text(
                            Pos2::new(card_rect.min.x + 40.0, card_rect.min.y + 18.0),
                            egui::Align2::LEFT_CENTER,
                            crate::i18n::t(tmpl.title),
                            egui::FontId::proportional(Theme::FONT_S),
                            crate::ui::theme::palette().text(),
                        );

                        // Tag Pill on Right
                        let tag_pos = Pos2::new(card_rect.max.x - 10.0, card_rect.min.y + 18.0);
                        ui.painter().text(
                            tag_pos,
                            egui::Align2::RIGHT_CENTER,
                            format!("[{}]", crate::i18n::t(tmpl.tag)),
                            egui::FontId::proportional(Theme::FONT_S),
                            tmpl.color,
                        );

                        // Description
                        ui.painter().text(
                            Pos2::new(card_rect.min.x + 14.0, card_rect.min.y + 48.0),
                            egui::Align2::LEFT_CENTER,
                            crate::i18n::t(tmpl.desc),
                            egui::FontId::proportional(Theme::FONT_S),
                            crate::ui::theme::palette().text_muted(),
                        );

                        if card_resp.clicked() {
                            if tmpl.category == AddTrackCategory::Import {
                                // Audio-file templates need a real file: request a
                                // picker instead of creating an empty silent track.
                                import_request = Some((*tmpl).clone());
                                close = true;
                            } else {
                                let new_id = playlist_tracks.len() + 1;
                                let mut track = PlaylistTrack::new(
                                    format!("{} {}", crate::i18n::t(tmpl.title), new_id),
                                    tmpl.icon,
                                    tmpl.kind,
                                    tmpl.color,
                                );
                                track.is_rec_armed = tmpl.is_rec_arm_default;
                                created_track = Some((track, crate::i18n::t(tmpl.title)));
                                close = true;
                            }
                        }

                        if (idx + 1) % 2 == 0 {
                            ui.end_row();
                        }
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("💡 Tips: Du kan byta färg, ljud och effekter när som helst i spårhuvudet.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(crate::i18n::t("Stäng")).clicked() {
                        close = true;
                    }
                });
            });
        });

    if let Some((track, title)) = created_track {
        *status_message = crate::tstatus!("✔ Skapade nytt spår: {}", title);
        playlist_tracks.push(track);
    }

    if close {
        *open = false;
    }

    import_request
}
