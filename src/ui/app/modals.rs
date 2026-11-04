//! Dialogerna — alla små fönster på ett ställe.
//!
//! Varje funktion här ritar **ett** fönster och gör ingenting annat: reglerna den vilar på
//! bor i sina egna moduler (import, projekt, export), så att de går att pröva utan fönster.
//!
//! Status: byggs — dialogerna; en ny dialog läggs här och håller sin regel utanför.
//! Rör inte: knapptexten är en instruktion, inte kosmetika — ändra texten när vägen ändras.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
/// Logotypen som textur, laddad **första gången den behövs**. `None` när bilden inte gick
/// att avkoda — anroparen ritar då ingen bild, aldrig en tom ruta.
pub(crate) fn logo_texture_for(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
    if self.logo_texture.is_none()
        && let Some(img) = sonix_logo()
    {
        self.logo_texture = Some(ctx.load_texture(
            "sonix-logo",
            img.clone(),
            egui::TextureOptions::LINEAR,
        ));
    }
    self.logo_texture.clone()
}
}

impl SonixApp {
pub(crate) fn render_help_manual_modal(&mut self, ctx: &egui::Context) {
    if !self.show_help_guide {
        return;
    }

    let mut close = false;
    let mut open = self.show_help_guide;

    egui::Window::new(crate::i18n::t("📖 Sonix Studio – Komplett Bruksanvisning & Master Manual"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        // **Mätta mått, inte gissade.** Manualen ritades 1919 × ~210 px: den längsta raden
        // satte bredden (ingen rad bröt) och `available_height()` i en horisontell rad var noll,
        // så allt kollapsade till en rad (designrunda 2). Bredden sätts nu på kolumnerna
        // *innan* texten läggs in, och höjden är ett tal i stället för en fråga till layouten.
        // **Fast storlek, inte `default_size`.** Egui mäter ett `default_size`-fönster ur
        // innehållet, och innehållet är ~200 etiketter som inte bröt — alltså blev fönstret
        // 1919 px brett och kollapsade på höjden (mätt i skärmdump, designrunda 2 och igen
        // 2026-11-04). Med `fixed_size` får kolumnerna en bredd att bryta mot.
        .fixed_size(Vec2::new(780.0, 520.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // Top Header with quick search and close
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("📖 SONIX STUDIO MASTER MANUAL")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().orange()));
                ui.label(egui::RichText::new(crate::i18n::t("• Komplett Referensguide & Handbok")).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✖ Stäng Manual")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().orange())).clicked() {
                        close = true;
                    }
                });
            });

            ui.separator();

            // 2-Column Layout: Left Chapters Sidebar + Right Scrollable Documentation
            ui.horizontal_top(|ui| {
                ui.set_min_height(380.0);
                // Brytningen gäller **hela** panelen: 200 etiketter skulle annars var och en
                // mäta sin egen rad och tvinga fönstret att växa i sidled.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                // Left Chapter Navigation Sidebar
                ui.group(|ui| {
                    ui.set_min_size(Vec2::new(220.0, 380.0));
                    ui.set_max_height(380.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("KAPITEL & AVSNITT:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🔍")).size(Theme::FONT_S));
                            ui.add(egui::TextEdit::singleline(&mut self.help_search_query).hint_text(crate::i18n::t("Sök i manualen...")).desired_width(170.0));
                        });
                        ui.add_space(4.0);

                        let chapters = [
                            (0, "🚀 1. Snabbstart & Översikt"),
                            (1, "⌨ 2. Tangentbord & Kommandon"),
                            (2, "📊 3. Tidslinje & 0.01s Snäpp"),
                            (3, "📦 4. Multi-Track Stämimport"),
                            (4, "🔍 5. Stämeditor & 3-Band EQ"),
                            (5, "🥁 6. Channel Rack & Beats"),
                            (6, "🎹 7. Piano Roll & Synth"),
                            (7, "🎤 8. Vocal Studio & Pitch"),
                            (8, "🎛 9. Mixer & WAV-Export"),
                            (9, "⚙ 10. PipeWire & Wayland"),
                        ];

                        egui::ScrollArea::vertical().id_salt("manual_nav_scroll").show(ui, |ui| {
                            for (ch_idx, title) in chapters {
                                let is_sel = self.help_manual_active_tab == ch_idx;
                                let bg = if is_sel { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().header() };
                                let fg = if is_sel { Color32::BLACK } else { crate::ui::theme::palette().text() };

                                if ui.add_sized(Vec2::new(205.0, 26.0), egui::Button::new(egui::RichText::new(crate::i18n::t(title)).strong().size(Theme::FONT_S).color(fg)).fill(bg)).clicked() {
                                    self.help_manual_active_tab = ch_idx;
                                }
                                ui.add_space(2.0);
                            }
                        });
                    });
                });

                // Right Main Content Scroll Area. **Bredden först**: egui bryter en `label`
                // bara när den har en maxbredd att bryta mot.
                ui.group(|ui| {
                    ui.set_min_size(Vec2::new(460.0, 380.0));
                    ui.set_max_width(460.0);
                    ui.set_max_height(380.0);

                    egui::ScrollArea::vertical()
                        .id_salt("manual_content_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            match self.help_manual_active_tab {
                                0 => {
                                    // 1. SNABBSTART & ÖVERSIKT
                                    ui.heading(egui::RichText::new(crate::i18n::t("🚀 1. Snabbstart & Översikt")).color(crate::ui::theme::palette().orange()));
                                    ui.label(egui::RichText::new(crate::i18n::t("Välkommen till Sonix Studio – en blixtsnabb Digital Audio Workstation skapad för Linux med äkta realtidsprestanda.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.add_space(8.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎯 Typiskt produktionsarbetsflöde:")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.add_space(4.0);
                                        ui.label(crate::i18n::t("1. Importera stämmor: Klicka på '📦 IMPORT STEMS' (Ctrl+I) för att läsa in ett ZIP-paket med sång, bas, trummor m.m."));
                                        ui.label(crate::i18n::t("2. Arrangera & Klipp: Använd saxverktyget (✂ Klipp) och zooma in djupt (Ctrl+Scroll) för att dela med 0.01s precision."));
                                        ui.label(crate::i18n::t("3. Fokusera & Förädla: Klicka på '🔍' på ett spår för att öppna Stämeditorn, isolera stämman med Solo och ratta 3-bands EQ."));
                                        ui.label(crate::i18n::t("4. Skapa trumkomp: Tryck F4 för att öppna Channel Rack och klicka in 16-stegs beats."));
                                        ui.label(crate::i18n::t("5. Spela in melodier: Tryck F5 för Piano Roll eller spela live med datortangentbordet."));
                                        ui.label(crate::i18n::t("6. Mixa & Exportera: Tryck F6 för Mixern och Ctrl+E för att exportera mastrad 48kHz WAV."));
                                    });

                                    ui.add_space(8.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("⚡ Systemarkitektur & Prestanda:")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• 100% Rust Audio DSP – Inget hack, noll skräpsamling (Garbage Collection), noll latens."));
                                        ui.label(crate::i18n::t("• PipeWire & ALSA Native – Ansluter direkt till Linux moderna ljudserver."));
                                        ui.label(crate::i18n::t("• Asynkron bakgrundsavkodning – Gränssnittet fryser aldrig vid inläsning av tunga ljudfiler."));
                                    });
                                }

                                1 => {
                                    // 2. TANGENTBORD & KOMMANDON
                                    ui.heading(egui::RichText::new(crate::i18n::t("⌨ 2. Tangentbord & Komplett Kortkommandoreferens")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("NAVIGERING MELLAN VYER (Funktionstangenter):")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.separator();
                                        let f_keys = [
                                            ("F1", "Bruksanvisning / Manual & Hjälpcenter"),
                                            ("F3", "Tidslinje / Multi-Track Arranger"),
                                            ("F4", "Sonix Channel Rack (16-stegs trummaskin)"),
                                            ("F5", "Piano Roll (Notinmatning & melodieditor)"),
                                            ("F6", "Mixer Console & Master Effektrack"),
                                            ("F7", "Analog Alchemy Synthesizer"),
                                            ("F8", "Vocal Studio (Melodyne & Harmonizer)"),
                                            ("F9", "AI Music Studio & Prompt Engine"),
                                            ("F10", "Modulär Synt & Patcher"),
                                        ];
                                        for (k, desc) in f_keys {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(format!("[ {:<3} ]", k)).monospace().strong().color(crate::ui::theme::palette().orange()));
                                                ui.label(crate::i18n::t(desc));
                                            });
                                        }
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("PROJEKT & REDIGERINGSKOMMANDON:")).strong().color(crate::ui::theme::palette().green()));
                                        ui.separator();
                                        let shortcuts = [
                                            ("Mellanslag (Space)", "Starta / Pausa uppspelning"),
                                            ("Ctrl + N", "Skapa nytt tomt projekt"),
                                            ("Ctrl + O / P", "Öppna Projektbläddrare"),
                                            ("Ctrl + S", "Spara projektfil"),
                                            ("Ctrl + I", "Importera Stämmor / Multi-Track Stems"),
                                            ("Ctrl + E", "Exportera projekt (WAV/FLAC/MP3/OGG/AAC)"),
                                            ("Del / Backspace", "Radera markerat ljudklipp"),
                                            ("Ctrl + Scroll", "Mjuk horisontell zoomning i tidslinjen"),
                                            ("A, W, S, E, D...", "Klaviatur – Spela synthen live med tangentbordet"),
                                            ("Tab / Shift + Tab", "Byt yta framåt / bakåt"),
                                            ("Ctrl + + / −", "Zooma pianorullens rutnät"),
                                            ("Ctrl + 0", "Nollställ zoomningen"),
                                        ];
                                        for (k, desc) in shortcuts {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(format!("{:<18}", k)).monospace().strong().color(crate::ui::theme::palette().cyan()));
                                                ui.label(crate::i18n::t(desc));
                                            });
                                        }
                                    });
                                }

                                2 => {
                                    // 3. TIDSLINJE & 0.01s PRECISION
                                    ui.heading(egui::RichText::new(crate::i18n::t("📊 3. Tidslinje, Snäpp & 0.01s Precision")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Tidslinjen hanterar obegränsat med ljudspår och regioner med precision ned till 0.01 sekunder (hundradelar).")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🔍 Dynamisk 4-Nivåers Tidslinjelinjal:")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("1. Takter (Bars): Stora vertikala streck med taktnummer (1, 2, 3...) och tidskod (MM:SS.cs)."));
                                        ui.label(crate::i18n::t("2. Beats: Fjärdedelstikar (.2, .3, .4) som syns vid normal zoom."));
                                        ui.label(crate::i18n::t("3. 1/16-delssteg: Tunt rutnät för exakt rytmisk klippning och placering."));
                                        ui.label(crate::i18n::t("4. Hundradelar (0.01s): Aktiveras vid djup inzoomning (200%–800%) för millimeterexakta snitt i sång och trummor."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("✂ Redigeringsverktyg & Snäpp:")).strong().color(crate::ui::theme::palette().orange()));
                                        ui.label(crate::i18n::t("• ⇱ Välj: Klicka på en region för att markera och se dess egenskaper i inspektorn."));
                                        ui.label(crate::i18n::t("• ✎ Rita: Klicka i tidslinjen för att rita ut mönster och aktiva klipp."));
                                        ui.label(crate::i18n::t("• ✂ Klipp (0.01s): Saxverktyg. Klicka var som helst på ett ljudspår för att klyva klippet i två."));
                                        ui.label(crate::i18n::t("• 🔇 Muta / 🗑 Radera: Tysta eller radera regioner direkt med ett klick."));
                                        ui.separator();
                                        ui.label(crate::i18n::t("• Snäppläge '⚡ 0.01s (Fri)': Frikopplar från musikaliska takter och låter dig klippa med 10ms precision."));
                                        ui.label(crate::i18n::t("• Snäpplägen 1/16, Beat, Takt: Snäpper automatiskt till det musikaliska tempot (BPM)."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🏃 Följ Tidslinje (Auto-Scroll):")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• '🏃 Följ tidslinje: PÅ': Tidslinjen rullar automatiskt och håller spelhuvudet centrerat på skärmen."));
                                        ui.label(crate::i18n::t("• '⏸ Följ tidslinje: AV': Tidslinjen står stilla så att du kan redigera i lugn och ro medan låten spelar."));
                                    });
                                }

                                3 => {
                                    // 4. MULTI-TRACK STÄMIMPORT
                                    ui.heading(egui::RichText::new(crate::i18n::t("📦 4. Multi-Track Stämimport (Stems)")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Importera kompletta stämpaket (WAV, MP3, FLAC, OGG) från Suno AI, FL Studio, Ableton, Logic m.fl.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("📥 Hur du importerar:")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("1. Tryck '📦 IMPORT STEMS' i verktygsfältet eller tryck Ctrl+I."));
                                        ui.label(crate::i18n::t("2. Välj bland automatiskt upptäckta paket i ~/Music / ~/Downloads eller välj ZIP-fil/mapp manuellt."));
                                        ui.label(crate::i18n::t("3. Dra & Släpp (Drag & Drop): Dra en .zip-fil direkt in i Sonix-fönstret."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎛 Automatisk Spåridentifiering & Routing:")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• Lead Vocals ➔ 🎙 Sångbuss med lila färgkod."));
                                        ui.label(crate::i18n::t("• Backing Vocals ➔ 🗣 Körbuss."));
                                        ui.label(crate::i18n::t("• Drums / Kick / Snare ➔ 🥁 Trumbuss med cyan färgkod."));
                                        ui.label(crate::i18n::t("• Bass ➔ 🎸 Basbuss med gul färgkod."));
                                        ui.label(crate::i18n::t("• Guitar / Keys / Synth ➔ 🎹 Synthbuss med grön/orange färgkod."));
                                        ui.label(crate::i18n::t("• FX / Other ➔ ✨ Effektsändning."));
                                        ui.separator();
                                        ui.label(crate::i18n::t("Inläsningen sker i bakgrunden med en förloppsindikator utan att programmet hänger sig."));
                                    });
                                }

                                4 => {
                                    // 5. STÄMEDITOR & 3-BANDS EQ
                                    ui.heading(egui::RichText::new(crate::i18n::t("🔍 5. Dedikerad Stämeditor & Ljudfokus")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Fokusera på en enda stämma med högprecisionskontroller, decibelnivåer, grafisk EQ och fading.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎧 1. Isolera stämma (Solo On):")).strong().color(crate::ui::theme::palette().orange()));
                                        ui.label(crate::i18n::t("Klicka på den stora knappen '🎧 ISOLERA STÄMMA (SOLO)' högst upp i editorn för att direkt tysta alla andra spår och lyssna enbart på den valda stämman."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎚 2. Volym, Pan & Dynamik (Flik 1):")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• Volymreglage med exakt dB-visning och '±0.1 dB' finjusteringsknappar."));
                                        ui.label(crate::i18n::t("• Stereopanorering med snabbcentrering ('Center')."));
                                        ui.label(crate::i18n::t("• Kompressor: Justerbar Threshold (-30 dB till 0 dB) och Ratio (1:1 till 8:1)."));
                                        ui.label(crate::i18n::t("• Pitch Shifter: Transponera stämman ±12 halvtoner med bevarade formanter (1-centprecision, Shift = finjustering, dubbelklick = nollställ)."));
                                        ui.label(crate::i18n::t("• Reverb & Delay sends."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("📈 3. 3-Bands Grafisk Parametrisk EQ (Flik 2):")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• Interaktiv frekvenskurva i realtid (20 Hz – 20 kHz) med dB-skala."));
                                        ui.label(crate::i18n::t("• Low Shelf (Bas): Gain ±12 dB, brytfrekvens 40–400 Hz."));
                                        ui.label(crate::i18n::t("• Mid Peak (Mellanregister): Gain ±12 dB, frekvens 200 Hz – 6 kHz, Q-faktor 0.5–3.0."));
                                        ui.label(crate::i18n::t("• High Shelf (Diskant): Gain ±12 dB, frekvens 3 kHz – 16 kHz."));
                                        ui.label(crate::i18n::t("• Snabbpresets för Sång, Trummor, Gitarr, Bas och Flat nollställning."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("⏱ 4. Fading & Envelope i hundradelar (Flik 3):")).strong().color(crate::ui::theme::palette().purple()));
                                        ui.label(crate::i18n::t("• Fade In och Fade Out med 0.01s precision."));
                                        ui.label(crate::i18n::t("• Stegknappar för '±0.01s' och '±0.10s'. Snabbval för 20ms, 50ms, 100ms, 500ms, 1.00s."));
                                        ui.label(crate::i18n::t("• Slå på alla: Sätter samma fading på alla klipp i just den stämman."));
                                    });
                                }

                                5 => {
                                    // 6. CHANNEL RACK & BEATS
                                    ui.heading(egui::RichText::new(crate::i18n::t("🥁 6. Sonix Channel Rack & Stegsequencer (F4)")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("16-Stegs Trummaskin & Mönster:")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• 8 Klassiska trumkanaler: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, 303 Bass och Lead."));
                                        ui.label(crate::i18n::t("• Mönster P1–P4: Skapa variationer för vers, refräng och stick."));
                                        ui.label(crate::i18n::t("• Swing: Skjutreglage för att ge trummorna ett naturligt sväng."));
                                        ui.label(crate::i18n::t("• ⚡ Slumpa Beats: Genererar omedelbart nya inspirerande trumkomp."));
                                        ui.label(crate::i18n::t("• Velocity-redigering: Justera anslagskraften per steg i den nedre velocity-raden."));
                                    });
                                }

                                6 => {
                                    // 7. PIANO ROLL & SYNTH
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎹 7. Piano Roll & Analog Synthesizer (F5 / F7)")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Piano Roll (F5):")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• Grafiskt 3-oktavigt notinmatningsfönster (C3 till B5)."));
                                        ui.label(crate::i18n::t("• Klicka på klaviaturet till vänster för att provlyssna toner i realtid."));
                                        ui.label(crate::i18n::t("• ⚡ Slumpa Melodi: Skapar harmoniska melodislingor automatiskt."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Analog Alchemy Synthesizer (F7):")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• 4 Vågformer: Sawtooth (sågtand), Square (fyrkant), Sine (sinus) och Noise (brus)."));
                                        ui.label(crate::i18n::t("• ADSR Envelope: Attack, Decay, Sustain och Release."));
                                        ui.label(crate::i18n::t("• Resonant SVF-filter (12 dB): Cutoff (20Hz–18kHz) och Resonans."));
                                        ui.label(crate::i18n::t("• Saturation & Drive: Analog rörvärme och distortion."));
                                    });
                                }

                                7 => {
                                    // 8. VOCAL STUDIO & PITCH
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎤 8. Vocal Studio, Melodyne & Harmonizer (F8)")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Funktioner i Vocal Studio:")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• Melodyne ARA2 Editor: Interaktiva tonhöjds-blobs för att justera sångens toner och timing."));
                                        ui.label(crate::i18n::t("• 4-Voice Harmonizer: Skapar fylliga sångarrangemang med kör, oktavdubbling eller vocoder."));
                                        ui.label(crate::i18n::t("• Comping & Tagningar: Spela in flera sångtagningar och klipp ihop den bästa versionen."));
                                        ui.label(crate::i18n::t("• Sampling & Recorder: Spela in egna ljud, klappar och instrument med din mikrofon."));
                                    });
                                }

                                8 => {
                                    // 9. MIXER & WAV-EXPORT
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎛 9. Mixer Console, Effekter & WAV-Export (F6 / Ctrl+E)")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Mixerbord & Effektrack (F6):")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• 8 Stereokanaler + Master Bus med analoga faders och VU peak meters."));
                                        ui.label(crate::i18n::t("• 3-Bands Parametrisk EQ per mixerkanal."));
                                        ui.label(crate::i18n::t("• Master Limiter & Maximizer för kommersiell ljudstyrka utan digital distorsion."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Export & Rendering (Ctrl+E):")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• Export Song: Renderar hela tidslinjen till en sammanslagen masterfil."));
                                        ui.label(crate::i18n::t("• Export Pattern: Exporterar det aktiva Channel Rack-mönstret."));
                                        ui.label(crate::i18n::t("• 📤 BATCH EXPORT: Exporterar alla aktiva stämmor som separata WAV-filer till ./exports/."));
                                        ui.label(crate::i18n::t("• Format: 32-bit float / 24-bit PCM WAV i 44.1 kHz eller 48 kHz."));
                                    });
                                }

                                9 => {
                                    // 10. PIPEWIRE & WAYLAND SETUP
                                    ui.heading(egui::RichText::new(crate::i18n::t("⚙ 10. Ljudmotor, PipeWire & Hyprland / Wayland")).color(crate::ui::theme::palette().orange()));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor (PipeWire / ALSA / JACK):")).strong().color(crate::ui::theme::palette().cyan()));
                                        ui.label(crate::i18n::t("• Sonix kommunicerar direkt med Linux professionella ljudserver i realtid."));
                                        ui.label(crate::i18n::t("• Buffertstorlek: 128/256 samples för noll latens vid live-spelning, 512/1024 samples för tunga projekt."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Linux Wayland & Hyprland Säkerhet:")).strong().color(crate::ui::theme::palette().green()));
                                        ui.label(crate::i18n::t("• Sonix är helt anpassat för Hyprland, GNOME Wayland och KDE."));
                                        ui.label(crate::i18n::t("• När fönstret täcks av ett annat fönster eller minimeras fortsätter ljudmotorn att spela utan avbrott samtidigt som GUI-uppritningen vilar för att förhindra krascher."));
                                        ui.label(crate::i18n::t("• Inbyggd Crash Logger sparar automatiskt eventuella problem till /tmp/sonix_crash.log."));
                                    });
                                }

                                _ => {}
                            }
                        });
                });
            });
        });

    if close || !open {
        self.show_help_guide = false;
    }
}
}

impl SonixApp {
    /// Sant när minst en dialog ligger ovanpå arrangören — används för att dämpa
    /// bakgrunden och för att släppa tangentbordet till dialogen.
    pub(crate) fn any_modal_open(&self) -> bool {
        self.show_notes_modal
            || self.show_import_modal
            || self.show_controller_modal
            || self.show_macros_modal
            || self.show_suno_import_modal
            || self.show_midi_import_modal
            || self.show_phrase_bank
            || self.show_about_modal
            || self.show_recovery_modal
            || self.show_project_manager_modal
            || self.show_ai_settings_modal
            || self.show_audio_settings_modal
            || self.show_mic_settings_modal
            || self.show_chord_generator_modal
            || self.show_tuner_modal
            || self.show_big_clock
            || self.show_dice_generator_modal
            || self.edison.is_some()
            || self.newtone.is_some()
            || self.newtime.is_some()
            || self.show_fx_rack_modal
            || self.show_tempo_modal
            || self.show_song_structure_modal
            || self.show_stem_focus_modal
            || self.show_help_guide
            || self.show_add_track_modal
            || self.show_render_queue_modal
            || self.show_undo_history_modal
    }

    /// **Frasbanken** (FL): en mapp med `.mid`-fraser, listade. Ett klick infogar frasen i det
    /// valda patternet genom samma väg som MIDI-importen — ingen egen notläsning.
    pub(crate) fn render_phrase_bank_modal(&mut self, ctx: &egui::Context) {
        if !self.show_phrase_bank {
            return;
        }
        let mut open = self.show_phrase_bank;
        let mut pick = false;
        let mut chosen: Option<std::path::PathBuf> = None;
        let mut preview: Option<std::path::PathBuf> = None;
        egui::Window::new(crate::i18n::t("🎼 Frasbank"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(340.0, 420.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("📂 Välj mapp")).clicked() {
                        pick = true;
                    }
                    if let Some(dir) = &self.phrase_folder {
                        let name = dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| dir.display().to_string());
                        ui.label(
                            egui::RichText::new(name)
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                    }
                });
                ui.separator();
                if self.phrases.is_empty() {
                    ui.label(
                        egui::RichText::new(crate::i18n::t("Inga fraser inlästa"))
                            .size(Theme::FONT_S)
                            .color(crate::ui::theme::palette().text_muted()),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            for (name, path) in &self.phrases {
                                ui.horizontal(|ui| {
                                    // Lyssna först, infoga sedan: ▶ rör inte patternet.
                                    if ui
                                        .small_button("▶")
                                        .on_hover_text(crate::i18n::t("Lyssna på frasen"))
                                        .clicked()
                                    {
                                        preview = Some(path.clone());
                                    }
                                    if ui
                                        .selectable_label(false, name)
                                        .on_hover_text(path.display().to_string())
                                        .clicked()
                                    {
                                        chosen = Some(path.clone());
                                    }
                                });
                            }
                        });
                }
            });
        self.show_phrase_bank = open;
        if !open {
            self.stop_phrase_preview();
        }
        if pick {
            self.load_phrase_bank();
        }
        if let Some(path) = preview {
            self.preview_phrase(&path);
        }
        if let Some(path) = chosen {
            self.stop_phrase_preview();
            self.insert_phrase(&path);
        }
    }

    /// **Stor klocka** (FL:s Big Clock): takt.slag.steg och spelad tid i stor stil.
    pub(crate) fn render_big_clock(&mut self, ctx: &egui::Context) {
        if !self.show_big_clock {
            return;
        }
        let mut open = self.show_big_clock;
        let position = crate::ui::big_clock::bar_beat_step(self.current_step);
        let time = crate::ui::big_clock::elapsed_time(self.current_step, self.bpm);
        egui::Window::new(crate::i18n::t("⏱ Stor klocka"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(420.0, 210.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(position)
                            .monospace()
                            .size(72.0)
                            .color(crate::ui::theme::palette().green()),
                    );
                    ui.label(
                        egui::RichText::new(time)
                            .monospace()
                            .size(40.0)
                            .color(crate::ui::theme::palette().text()),
                    );
                });
            });
        self.show_big_clock = open;
    }

    /// **Take lanes** (Ableton-kompning): varje inspelningsförsök blir en lane, och man
    /// väljer vilken som spelas. Redigering (kvantisera, transponera) gäller den valda.
    pub(crate) fn render_takes_modal(&mut self, ctx: &egui::Context) {
        if !self.show_takes_modal {
            return;
        }
        let mut open = self.show_takes_modal;
        let mut pick: Option<usize> = None;
        let mut delete: Option<usize> = None;
        let mut comp_at: Option<(usize, f32, usize)> = None;
        let mut remove_comp: Option<(Option<usize>, f32)> = None;
        let mut comp_step = self.comp_step;
        let mut comp_bar = self.comp_bar;
        egui::Window::new(crate::i18n::t("🧵 Tagningar"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(360.0, 260.0))
            .show(ctx, |ui| {
                let Some(pat) = self.patterns.get(self.selected_pattern) else {
                    ui.label(crate::i18n::t("Inget pattern valt"));
                    return;
                };
                let active = pat.active_take;
                let lanes: Vec<(usize, usize)> = pat
                    .take_lanes()
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (i, t.len()))
                    .collect();
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Välj tagningen som spelas — inspelning går alltid till lane 1",
                    ))
                    .size(Theme::FONT_XS)
                    .color(crate::ui::theme::palette().text_muted()),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Komp från steg:"));
                    ui.add(
                        egui::DragValue::new(&mut comp_step)
                            .range(0.0..=15.0)
                            .speed(0.25),
                    );
                    ui.label(crate::i18n::t("Takt:"));
                    ui.add(egui::DragValue::new(&mut comp_bar).range(0..=4))
                        .on_hover_text(crate::i18n::t(
                            "0 = kompningen gäller alla takter, k = bara takt k",
                        ));
                });
                for (i, notes) in lanes {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(active == i, format!("{} {}", crate::i18n::t("Lane"), i + 1)).clicked() {
                            pick = Some(i);
                        }
                        ui.label(
                            egui::RichText::new(crate::tstatus!("{} noter", notes))
                                .size(Theme::FONT_S),
                        );
                        if ui
                            .button("✂")
                            .on_hover_text(crate::i18n::t(
                                "Låt den här lanen spela från steget",
                            ))
                            .clicked()
                        {
                                            comp_at = Some((comp_bar, comp_step, i));
                        }
                        if i > 0 && ui.button("🗑").clicked() {
                            delete = Some(i);
                        }
                    });
                }
                if !pat.comp.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            crate::i18n::t("Komp:"),
                            pat.comp.len()
                        ))
                        .size(Theme::FONT_XS)
                        .color(crate::ui::theme::palette().text_muted()),
                    );
                    for slice in &pat.comp {
                        ui.horizontal(|ui| {
                            let scope = match slice.bar {
                                Some(bar) => crate::tstatus!("takt {} · ", bar + 1),
                                None => String::new(),
                            };
                            ui.label(format!(
                                "{scope}{}",
                                crate::tstatus!(
                                    "steg {:.0} → lane {}",
                                    slice.from_step,
                                    slice.take + 1
                                )
                            ));
                            if ui.button("🗑").clicked() {
                                remove_comp = Some((slice.bar, slice.from_step));
                            }
                        });
                    }
                }
            });
        if let Some(i) = pick
            && let Some(pat) = self.patterns.get_mut(self.selected_pattern)
        {
            pat.active_take = i;
            self.status_message = crate::tstatus!("🧵 Spelar lane {}", i + 1);
        }
        if let Some(i) = delete
            && let Some(pat) = self.patterns.get_mut(self.selected_pattern)
            && i > 0
            && i - 1 < pat.alt_takes.len()
        {
            pat.alt_takes.remove(i - 1);
            // Komp som pekade på den borttagna lanen får lane 1; de efter flyttas ner ett steg,
            // annars pekar de tyst på fel lane.
            for slice in &mut pat.comp {
                if slice.take == i {
                    slice.take = 0;
                } else if slice.take > i {
                    slice.take -= 1;
                }
            }
            pat.active_take = pat.active_take.min(pat.alt_takes.len());
            self.status_message = crate::tstatus!("🗑 Tog bort lane {}", i + 1);
        }
        self.comp_step = comp_step;
        self.comp_bar = comp_bar;
        if let Some((bar, step, lane)) = comp_at
            && let Some(pat) = self.patterns.get_mut(self.selected_pattern)
        {
            let bar = if bar == 0 { None } else { Some(bar - 1) };
            pat.comp_from_bar(bar, step, lane);
            self.status_message = match bar {
                Some(bar) => crate::tstatus!(
                    "✂ Lane {} spelar från steg {:.0} i takt {}",
                    lane + 1,
                    step,
                    bar + 1
                ),
                None => crate::tstatus!(
                    "✂ Lane {} spelar från steg {:.0} i alla takter",
                    lane + 1,
                    step
                ),
            };
        }
        if let Some((bar, step)) = remove_comp
            && let Some(pat) = self.patterns.get_mut(self.selected_pattern)
        {
            pat.comp.retain(|slice| {
                !((slice.from_step - step).abs() <= 0.001 && slice.bar == bar)
            });
            self.status_message = crate::tstatus!("🗑 Tog bort komp vid steg {:.0}", step);
        }
        self.show_takes_modal = open;
    }

    /// **Fruity Dance**: en streckfigur som dansar i takt. Posen räknas i `ui::dance`,
    /// ritningen här — ingen bild, ingen animationstråd, bara figurens leder per bildruta.
    pub(crate) fn render_dance_modal(&mut self, ctx: &egui::Context) {
        if !self.show_dance {
            return;
        }
        let mut open = self.show_dance;
        egui::Window::new(crate::i18n::t("💃 Fruity Dance"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(240.0, 300.0))
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 200.0),
                    Sense::hover(),
                );
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, Rounding::same(4.0), Color32::BLACK);
                let steps = crate::ui::app::state::pattern_steps(
                    self.patterns
                        .get(self.selected_pattern)
                        .map_or(1, |p| p.bars),
                );
                let phase = if self.is_playing {
                    (self.current_step % steps) as f32 / steps as f32
                } else {
                    0.0
                };
                let pose = crate::ui::dance::pose(phase);
                // −1…1 → rutan, med y uppåt.
                let to_px = |(x, y): (f32, f32)| {
                    let scale = rect.height() * 0.42;
                    Pos2::new(
                        rect.center().x + x * scale,
                        rect.center().y - (y + pose.bob) * scale,
                    )
                };
                let hip = to_px(pose.hip);
                let head = to_px(pose.head);
                let limb = |from: Pos2, to: Pos2| {
                    painter.line_segment([from, to], Stroke::new(3.0_f32, crate::ui::theme::palette().green()));
                };
                limb(hip, head);
                limb(hip, to_px(pose.arm_l));
                limb(hip, to_px(pose.arm_r));
                limb(hip, to_px(pose.leg_l));
                limb(hip, to_px(pose.leg_r));
                painter.circle_filled(head, 12.0, crate::ui::theme::palette().cyan());
            });
        self.show_dance = open;
    }

    /// **VFX Script (lite)**: formeln skrivs in i en automationsbana på det valda spåret.
    /// Kurvan blir vanliga punkter, så den går att rätta för hand efteråt.
    pub(crate) fn render_script_modal(&mut self, ctx: &egui::Context) {
        if !self.show_script_modal {
            return;
        }
        let mut open = self.show_script_modal;
        let mut apply = false;
        egui::Window::new(crate::i18n::t("📜 Skript"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(400.0, 200.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(crate::i18n::t("Formel över tid (t = takter)"))
                        .size(Theme::FONT_S),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.script_source)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Mål:"));
                    egui::ComboBox::from_id_salt("script_param")
                        .selected_text(crate::ui::app::state::AutomationParam::ALL
                            [self.script_param.min(
                                crate::ui::app::state::AutomationParam::COUNT - 1,
                            )]
                        .label())
                        .show_ui(ui, |ui| {
                            for (i, p) in
                                crate::ui::app::state::AutomationParam::ALL.iter().enumerate()
                            {
                                if ui.selectable_label(self.script_param == i, p.label()).clicked()
                                {
                                    self.script_param = i;
                                }
                            }
                        });
                    if ui.button(crate::i18n::t("✍ Skriv kurvan")).clicked() {
                        apply = true;
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Kurvan skrivs på det valda spåret, över patternets takter",
                    ))
                    .size(Theme::FONT_XS)
                    .color(crate::ui::theme::palette().text_muted()),
                );
            });
        self.show_script_modal = open;
        if apply {
            self.apply_script_curve();
        }
    }

    /// Räknar formeln och lägger punkterna i spårets bana — samma bana som handredigeringen
    /// använder, så inget särskilt format behöver läras.
    pub(crate) fn apply_script_curve(&mut self) {
        let bars = self
            .patterns
            .get(self.selected_pattern)
            .map_or(1, |p| p.bars);
        match crate::ui::app::script::points_from_formula(&self.script_source, bars, 16) {
            Ok(points) => {
                if self.playlist_tracks.is_empty() {
                    self.status_message =
                        crate::tstatus!("⚠ Det finns inget spår att skriva kurvan på");
                    return;
                }
                let track_idx = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
                let param = crate::ui::app::state::AutomationParam::ALL
                    [self.script_param.min(crate::ui::app::state::AutomationParam::COUNT - 1)];
                let track = &mut self.playlist_tracks[track_idx];
                match track.automation.iter_mut().find(|l| l.param == param) {
                    Some(lane) => {
                        lane.points = points;
                        lane.enabled = true;
                    }
                    None => track
                        .automation
                        .push(crate::ui::app::state::AutomationLane {
                            param,
                            enabled: true,
                            points,
                        }),
                }
                self.status_message = crate::tstatus!(
                    "📜 Skrev {} punkter på '{}' (spår {})",
                    bars * 16,
                    param.label(),
                    track_idx + 1
                );
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Formeln går inte att läsa: {}", e);
            }
        }
    }

    /// **Visualizer** — vågformen överst, spektrumstaplar under, med toppmarkör.
    /// Samma data som mixern ritar; ingen egen analys.
    pub(crate) fn render_visualizer_modal(&mut self, ctx: &egui::Context) {
        if !self.show_visualizer {
            return;
        }
        let mut open = self.show_visualizer;
        egui::Window::new(crate::i18n::t("🌊 Visualizer"))
            .open(&mut open)
            .collapsible(false)
            .default_size(Vec2::new(560.0, 460.0))
            .show(ctx, |ui| {
                let wave_h = 90.0;
                let (wave_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), wave_h),
                    Sense::hover(),
                );
                let painter = ui.painter_at(wave_rect);
                painter.rect_filled(wave_rect, Rounding::same(4.0), Color32::BLACK);
                if self.scope_history.len() > 1 {
                    let n = self.scope_history.len();
                    let pts: Vec<Pos2> = self
                        .scope_history
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            Pos2::new(
                                wave_rect.min.x
                                    + wave_rect.width() * i as f32 / (n - 1) as f32,
                                wave_rect.center().y
                                    - s.clamp(-1.0, 1.0) * wave_rect.height() * 0.45,
                            )
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, crate::ui::theme::palette().green())));
                }

                ui.add_space(6.0);

                // Goniometern: mitten och sidan ur skopet, ritade som L/R-par.
                let g_size = 132.0;
                let (g_rect, _) = ui.allocate_exact_size(Vec2::new(g_size, g_size), Sense::hover());
                let painter = ui.painter_at(g_rect);
                painter.rect_filled(g_rect, Rounding::same(4.0), Color32::BLACK);
                let c = g_rect.center();
                let rad = g_size * 0.45;
                let faint = crate::ui::theme::palette().border();
                painter.line_segment(
                    [Pos2::new(c.x, c.y - rad), Pos2::new(c.x, c.y + rad)],
                    Stroke::new(1.0_f32, faint),
                );
                painter.line_segment(
                    [Pos2::new(c.x - rad, c.y), Pos2::new(c.x + rad, c.y)],
                    Stroke::new(1.0_f32, faint),
                );
                let pairs = self.scope_history.len().min(self.scope_side.len());
                if pairs > 1 {
                    // Strecket ritas som en väg genom punkterna: en sinus blir en diagonal
                    // ellips, mono en lodrät linje och ur fas en vågrät. En punktsvärm på
                    // 600 cirklar per bildruta syntes knappt och kostade mer än den visade.
                    let step = (pairs / 900).max(1);
                    let path: Vec<Pos2> = (0..pairs)
                        .step_by(step)
                        .map(|i| {
                            let mid = self.scope_history[i];
                            let side = self.scope_side[i];
                            let (x, y) = crate::ui::visualizer::goniometer_xy(mid + side, mid - side);
                            Pos2::new(
                                c.x + x.clamp(-1.0, 1.0) * rad,
                                c.y - y.clamp(-1.0, 1.0) * rad,
                            )
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        path,
                        Stroke::new(1.2_f32, crate::ui::theme::palette().green()),
                    ));
                }

                ui.add_space(6.0);

                let (bar_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 120.0),
                    Sense::hover(),
                );
                let painter = ui.painter_at(bar_rect);
                painter.rect_filled(bar_rect, Rounding::same(4.0), Color32::BLACK);
                let bands = self.spectrum_levels.len().max(1);
                let bw = bar_rect.width() / bands as f32;
                if self.visualizer_peaks.len() != bands {
                    self.visualizer_peaks = vec![0.0; bands];
                }
                crate::ui::visualizer::decay_peak(
                    &mut self.visualizer_peaks,
                    &self.spectrum_levels,
                    0.004,
                );
                for (i, level) in self.spectrum_levels.iter().enumerate() {
                    let x = bar_rect.min.x + i as f32 * bw;
                    let h = crate::ui::visualizer::bar_height(*level, bar_rect.height());
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(x + 1.0, bar_rect.max.y - h),
                            Vec2::new((bw - 2.0).max(1.0), h),
                        ),
                        Rounding::same(1.0),
                        crate::ui::theme::palette().cyan(),
                    );
                    let peak = crate::ui::visualizer::bar_height(
                        self.visualizer_peaks[i],
                        bar_rect.height(),
                    );
                    let py = bar_rect.max.y - peak;
                    painter.line_segment(
                        [Pos2::new(x + 1.0, py), Pos2::new(x + bw - 1.0, py)],
                        Stroke::new(1.5_f32, Color32::WHITE),
                    );
                }
            });
        self.show_visualizer = open;
    }

    /// **Ångra-historiken** (REAPER: Ctrl+Alt+Z). Stegen listas nyast överst; ett klick
    /// ångrar allt ovanför, så man hoppar tillbaka i stället för att trycka Ctrl+Z tio gånger.
    pub(crate) fn render_undo_history_modal(&mut self, ctx: &egui::Context) {
        if !self.show_undo_history_modal {
            return;
        }
        let rows: Vec<String> = self
            .undo_stack
            .iter()
            .map(|s| s.description.clone())
            .collect();
        let mut jump: Option<usize> = None;
        let mut open = self.show_undo_history_modal;
        egui::Window::new("↶ Ångra-historik")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(440.0, 340.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Nyast överst. Ett klick ångrar allt ovanför steget.")
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().text_muted()),
                );
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if rows.is_empty() {
                        ui.label("Ingenting att ångra.");
                    }
                    for (i, label) in rows.iter().enumerate().rev() {
                        let text = format!("{}. {}", i + 1, label);
                        if ui.selectable_label(false, text).clicked() {
                            jump = Some(i);
                        }
                    }
                });
            });
        self.show_undo_history_modal = open;
        if let Some(i) = jump {
            self.undo_to(i);
            self.status_message = crate::tstatus!("↶ Hoppade tillbaka till steg {}", i + 1);
        }
    }

    /// Alla dialoger, i den ordning de läggs ovanpå varandra.
    pub(crate) fn render_all_modals(&mut self, ctx: &egui::Context) {
        // En dialog ska vara det enda som lyser: resten av ytan dämpas.
        if self.any_modal_open() {
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("modal_scrim"),
            ))
            .rect_filled(ctx.screen_rect(), 0.0, crate::ui::theme::palette().scrim());
        }
        // 4. Modals & Dialogs
        self.render_notes_modal(ctx);
        self.render_import_sample_modal(ctx);
        self.render_hardware_controller_modal(ctx);
        self.render_batch_export_modal(ctx);
        self.render_suno_stem_import_modal(ctx);
        self.render_wav_question_modal(ctx);
        self.render_stem_import_progress_modal(ctx);
        self.render_project_load_progress_modal(ctx);
        self.render_about_modal(ctx);
        self.render_midi_import_modal(ctx);
        self.render_phrase_bank_modal(ctx);
        self.render_recovery_modal(ctx);
        self.render_undo_history_modal(ctx);
        self.render_project_manager_modal(ctx);
        self.render_ai_settings_modal(ctx);
        self.render_audio_settings_modal(ctx);
        self.render_mic_settings_modal(ctx);
        self.render_chord_generator_modal_view(ctx);
        self.render_tuner_modal_view(ctx);
        self.render_big_clock(ctx);
        self.render_visualizer_modal(ctx);
        self.render_script_modal(ctx);
        self.render_dance_modal(ctx);
        self.render_takes_modal(ctx);
        self.render_dice_generator_modal_view(ctx);
        self.render_edison_modal_view(ctx);
        self.render_newtone_modal_view(ctx);
        self.render_newtime_modal_view(ctx);
        self.render_source_tempo_modal_view(ctx);
        self.render_fx_rack_modal_view(ctx);
        self.render_tempo_modal(ctx);
        self.render_song_structure_modal_view(ctx);
        self.render_arranger_chain_view(ctx);
        self.render_chord_track_view(ctx);
        self.render_mod_matrix_view(ctx);
        self.render_launcher_view(ctx);
        self.render_macros_modal_view(ctx);
        self.render_stem_focus_modal(ctx);
        self.render_help_manual_modal(ctx);
        self.render_add_track_modal(ctx);
    }
}
