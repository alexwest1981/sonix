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
            color: Color32::from_rgb(0, 230, 255),
            category: AddTrackCategory::Vocal,
            tag: "Mikrofon",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎤 Rap / Trap Vocal",
            desc: "Tät, punchig röstinställning med de-esser och snabb gate.",
            icon: "🎤",
            kind: TrackKind::VocalAudio,
            color: Color32::from_rgb(255, 110, 200),
            category: AddTrackCategory::Vocal,
            tag: "Mikrofon",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎙 Stämmor & Backing Vocals",
            desc: "Bred stereobild med doubler och mjuk plate-reverb.",
            icon: "👥",
            kind: TrackKind::VocalAudio,
            color: Color32::from_rgb(180, 120, 255),
            category: AddTrackCategory::Vocal,
            tag: "Kör & Harmoni",
            is_rec_arm_default: true,
        },
        TrackTemplate {
            title: "🎙 Podcast & Tal",
            desc: "Ren och brusfri röstinspelning optimerad för podcasts och voiceovers.",
            icon: "📻",
            kind: TrackKind::VocalAudio,
            color: Color32::from_rgb(255, 180, 50),
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
            color: Theme::FL_GREEN,
            category: AddTrackCategory::Keys,
            tag: "Akustiskt",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎹 Vintage Rhodes Electric",
            desc: "Varmt elpiano med mjuk tremolo och analog chorus.",
            icon: "✨",
            kind: TrackKind::SynthLead,
            color: Color32::from_rgb(80, 210, 170),
            category: AddTrackCategory::Keys,
            tag: "Vintage",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎛 Analog Poly Synth Pad",
            desc: "Svävande varma synth-mattor och drömlika atmosfärer.",
            icon: "🌌",
            kind: TrackKind::SynthLead,
            color: Color32::from_rgb(140, 100, 255),
            category: AddTrackCategory::Keys,
            tag: "Synth",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ Pluck & Arp Lead",
            desc: "Snabb, krispig synth-pluck perfekt för melodier och arpeggion.",
            icon: "⚡",
            kind: TrackKind::SynthLead,
            color: Color32::from_rgb(255, 220, 40),
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
            color: Color32::from_rgb(255, 90, 90),
            category: AddTrackCategory::BassGuitar,
            tag: "Gitarr",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎸 Akustisk Nylon Gitarr",
            desc: "Varm och fyllig fingerpicking-gitarr med studiorumsklang.",
            icon: "🪕",
            kind: TrackKind::SynthLead,
            color: Color32::from_rgb(230, 160, 80),
            category: AddTrackCategory::BassGuitar,
            tag: "Akustiskt",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🔊 808 Sub Bass",
            desc: "Djup, mättad sub-bas som pumpar och skakar subwoofern.",
            icon: "🔊",
            kind: TrackKind::Bassline,
            color: Color32::from_rgb(255, 50, 120),
            category: AddTrackCategory::BassGuitar,
            tag: "Sub Bass",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ 303 Acid Bassline",
            desc: "Klassisk resonant bas med glide och distorsion.",
            icon: "🎛",
            kind: TrackKind::Bassline,
            color: Color32::from_rgb(255, 140, 20),
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
            color: Theme::FL_CYAN,
            category: AddTrackCategory::Drums,
            tag: "Trap / Hip-Hop",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🥁 Studio Akustiska Trummor",
            desc: "Naturligt trumset med punchig bastrumma och dynamisk virvel.",
            icon: "🥢",
            kind: TrackKind::Drums,
            color: Color32::from_rgb(100, 180, 255),
            category: AddTrackCategory::Drums,
            tag: "Rock / Pop",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "☕ Lo-Fi Vinyl Beat Kit",
            desc: "Knastriga samplingar, dämpade kicks och vintage vinylklang.",
            icon: "📻",
            kind: TrackKind::Drums,
            color: Color32::from_rgb(200, 150, 100),
            category: AddTrackCategory::Drums,
            tag: "Lo-Fi",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "⚡ 909 Electronic Techno Kit",
            desc: "Tung analog kick, öppna hi-hats och industriella claps.",
            icon: "⚡",
            kind: TrackKind::Drums,
            color: Color32::from_rgb(255, 100, 40),
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
            color: Color32::from_rgb(220, 140, 255),
            category: AddTrackCategory::Orchestra,
            tag: "Cinematic",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "🎺 Brass Section",
            desc: "Mäktiga trumpeter, tromboner och valthorn för drops och refränger.",
            icon: "🎺",
            kind: TrackKind::SynthLead,
            color: Color32::from_rgb(255, 200, 60),
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
            color: Theme::FL_ORANGE,
            category: AddTrackCategory::Import,
            tag: "WAV / MP3",
            is_rec_arm_default: false,
        },
        TrackTemplate {
            title: "📦 Suno AI Stems Spår",
            desc: "Separat ljudspår för Vocals, Drums, Bass eller Instrument från Suno AI.",
            icon: "📦",
            kind: TrackKind::CustomAudio,
            color: Color32::from_rgb(160, 60, 240),
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
) {
    if !*open {
        return;
    }

    let mut close = false;
    let mut created_track = None;

    egui::Window::new("➕ Skapa nytt spår i Sonix Studio")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(720.0, 520.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.add_space(2.0);

            // Top Header and Search Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Välj instrument eller ljudkälla för ditt nya spår:").strong().size(12.5).color(Theme::TEXT_BRIGHT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("🔍 Sök:").size(11.0).color(Theme::TEXT_MUTED));
                    ui.add_sized(Vec2::new(140.0, 20.0), egui::TextEdit::singleline(&mut state.search_filter).hint_text("T.ex. flygel, 808..."));
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
                    let fill = if is_active { Theme::FL_ORANGE } else { Color32::from_rgb(26, 32, 42) };
                    let fg = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                    if ui.add(egui::Button::new(egui::RichText::new(cat_name).strong().size(11.0).color(fg)).fill(fill)).clicked() {
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

                        let bg = if is_hover { Color32::from_rgb(32, 40, 54) } else { Color32::from_rgb(20, 24, 32) };
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
                            egui::FontId::proportional(16.0),
                            tmpl.color,
                        );
                        ui.painter().text(
                            Pos2::new(card_rect.min.x + 40.0, card_rect.min.y + 18.0),
                            egui::Align2::LEFT_CENTER,
                            tmpl.title,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );

                        // Tag Pill on Right
                        let tag_pos = Pos2::new(card_rect.max.x - 10.0, card_rect.min.y + 18.0);
                        ui.painter().text(
                            tag_pos,
                            egui::Align2::RIGHT_CENTER,
                            format!("[{}]", tmpl.tag),
                            egui::FontId::proportional(10.0),
                            tmpl.color,
                        );

                        // Description
                        ui.painter().text(
                            Pos2::new(card_rect.min.x + 14.0, card_rect.min.y + 48.0),
                            egui::Align2::LEFT_CENTER,
                            tmpl.desc,
                            egui::FontId::proportional(10.0),
                            Theme::TEXT_MUTED,
                        );

                        if card_resp.clicked() {
                            let new_id = playlist_tracks.len() + 1;
                            let mut track = PlaylistTrack::new(
                                format!("{} {}", tmpl.title, new_id),
                                tmpl.icon,
                                tmpl.kind,
                                tmpl.color,
                            );
                            track.is_rec_armed = tmpl.is_rec_arm_default;
                            created_track = Some((track, tmpl.title));
                            close = true;
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
                ui.label(egui::RichText::new("💡 Tips: Du kan byta färg, ljud och effekter när som helst i spårhuvudet.").size(10.5).color(Theme::TEXT_MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Stäng").clicked() {
                        close = true;
                    }
                });
            });
        });

    if let Some((track, title)) = created_track {
        *status_message = format!("✔ Skapade nytt spår: {}", title);
        playlist_tracks.push(track);
    }

    if close {
        *open = false;
    }
}
