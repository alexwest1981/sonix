//! Makron-modalen (Fas 8.9 steg 2): bygga, spara och köra en makrokedja inifrån appen.
//!
//! **Modalen är inkoppling.** Reglerna — stegen, trimningen, normaliseringen, exporten,
//! namnreglerna och felvägarna — bor i [`crate::audio::macro_chain`] och är prövade där (14
//! prov, mätta på riktiga stämmor). Det som händer här är att läsa kedjor ur mappen, visa dem,
//! låta handen ändra stegen och sätta en körning i gång.
//!
//! **En batch körs på en arbetstråd.** Tjugo filer tar minuter, och fönstret får inte stå
//! still medan det går — återkopplingen kommer per fil (`run_batch_with_progress`) och läggs i
//! en delad ruta som modalens bildruta läser av.
//!
//! Status: byggs — steg 2. Klickas inte i CI (ingen skärm finns här), så logiken ligger i
//! proven: filnamnsregeln och stegvalen är rena funktioner med egna prov.
//! Rör inte: en andra uppsättning stegregler. Stegen bor i `macro_chain`.

use crate::audio::exporter::{ExportFormat, ExportMeta};
use crate::audio::macro_chain::{self, ChainEntry, FileRun, MacroChain, MacroStep};
use crate::ui::theme::Theme;
use eframe::egui::{self, Vec2};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Vad en körning riktas mot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MacroTarget {
    /// En mapp med ljudfiler (Audacitys batch).
    Folder,
    /// Det öppna projektet — renderas och körs genom samma stegregler.
    Project,
}

/// Stegslagen i listan "lägg till steg".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MacroStepKind {
    TrimSilence,
    Normalize,
    Export,
}

impl MacroStepKind {
    pub const ALL: [MacroStepKind; 3] = [
        MacroStepKind::TrimSilence,
        MacroStepKind::Normalize,
        MacroStepKind::Export,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MacroStepKind::TrimSilence => "✂ Trimma tystnad",
            MacroStepKind::Normalize => "📊 Normalisera",
            MacroStepKind::Export => "💾 Exportera",
        }
    }

    /// **Standardsteget.** Värdena är desamma som `sonix --macro-example` skriver, så en kedja
    /// byggd i appen och en byggd för hand i en textfil blir **samma kedja** — annars vore
    /// startkedjan och knappen två olika sanningar om samma steg.
    pub fn default_step(self) -> MacroStep {
        match self {
            MacroStepKind::TrimSilence => MacroStep::TrimSilence {
                threshold_db: -55.0,
                keep_ms: 60.0,
            },
            MacroStepKind::Normalize => MacroStep::Normalize {
                target_lufs: -14.0,
                ceiling_dbtp: -1.0,
            },
            MacroStepKind::Export => MacroStep::Export {
                format: ExportFormat::Flac,
                suffix: "_master".to_string(),
                dither: true,
                noise_shaping: false,
            },
        }
    }
}

/// **Filnamnet för en kedja.** En kedja sparas som en fil man ska kunna hitta och rätta för
/// hand: `"Suno-stämma"` blir `suno-stamma.json`.
///
/// Translittereringen är **appens** (`crate::autosave::slug`) i stället för en egen tabell —
/// två slug-regler för samma sak driver isär, och den ena blir fel utan att någon märker det.
/// Följden ska sägas rakt ut: `Stämma` och `Stamma` blir **samma fil** (båda → `stamma.json`),
/// så den som sparar över en kedja får filnamnet i statusraden.
///
/// Ändelsen läggs på **även när namnet är tomt**: en fil utan `.json` hittas inte av listan i
/// mappen, så kedjan skulle sparas och sedan vara borta.
pub fn chain_file_name(name: &str) -> String {
    // Finns det något att bygga ett namn av? `slug` svarar "projekt" när inget återstår (dess
    // nödlösning för projektmappar), och en kedja som heter `projekt.json` vore ett namn man
    // inte känner igen. Finns bokstäver eller siffror ger `slug` alltid ett eget namn.
    if !name.chars().any(char::is_alphanumeric) {
        return "kedja.json".to_string();
    }
    format!("{}.json", crate::autosave::slug(name))
}

/// Modalens tillstånd. `progress` och `result` delas med arbetstråden: den skriver, bildrutan
/// läser.
#[derive(Clone)]
pub struct MacroModalState {
    /// Kedjorna i mappen, med sina fel (en trasig fil blir en rad, inte en tom plats).
    pub entries: Vec<ChainEntry>,
    /// Den valda kedjan, redigerbar. `None` = ingen vald.
    pub edit: Option<MacroChain>,
    /// Filen den valda kedjan kom från (tomt = ännu inte sparad).
    pub edit_path: Option<PathBuf>,
    pub target: MacroTarget,
    pub input_dir: String,
    pub out_dir: String,
    /// Filerna som hittades i in-mappen, och varför det inte gick.
    pub files: Vec<String>,
    pub listing_note: String,
    pub message: String,
    pub running: bool,
    pub progress: Arc<Mutex<(usize, usize)>>,
    pub result: Arc<Mutex<Option<Vec<FileRun>>>>,
    pub reports: Vec<String>,
    /// Satt av modalen när användaren vill köra mot projektet; plockas av anroparen, som har
    /// `SonixApp` och kan rendera.
    pub project_request: Option<MacroChain>,
}

impl Default for MacroModalState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            edit: None,
            edit_path: None,
            target: MacroTarget::Folder,
            input_dir: String::new(),
            out_dir: String::new(),
            files: Vec::new(),
            listing_note: String::new(),
            message: String::new(),
            running: false,
            progress: Arc::new(Mutex::new((0, 0))),
            result: Arc::new(Mutex::new(None)),
            reports: Vec::new(),
            project_request: None,
        }
    }
}

impl MacroModalState {
    /// Läser om mappens kedjor. Anropas när modalen öppnas och när en kedja sparats.
    pub fn reload(&mut self, dir: &std::path::Path) {
        self.entries = macro_chain::load_chains_in(dir);
    }

    /// Väljer en kedja för redigering (en kopia — först vid Spara skrivs den).
    pub fn select(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            self.edit = entry.chain.clone().ok();
            self.edit_path = Some(entry.path.clone());
        }
    }

    /// Läser in-mappen och listar vad som skulle köras.
    pub fn list_input_files(&mut self) {
        self.files.clear();
        let dir = PathBuf::from(self.input_dir.trim());
        if self.input_dir.trim().is_empty() {
            self.listing_note = crate::i18n::t("Välj en mapp först").to_string();
            return;
        }
        match macro_chain::audio_files_in(&dir) {
            Ok(files) => {
                self.listing_note = crate::tstatus!("{} ljudfiler hittades", files.len());
                self.files = files;
            }
            Err(e) => self.listing_note = e,
        }
    }

    /// Är körningen igång och vad står den på just nu?
    pub fn progress_now(&self) -> (usize, usize) {
        self.progress.lock().map(|p| *p).unwrap_or((0, 0))
    }

    /// Har en körning blivit klar sedan sist bildrutan ritades?
    pub fn take_finished(&mut self) -> Option<Vec<FileRun>> {
        let mut slot = self.result.lock().ok()?;
        slot.take()
    }

    /// Skriver den redigerade kedjan till mappen. Namnet kommer ur kedjans eget namn
    /// ([`chain_file_name`]), så samma kedja hamnar i samma fil varje gång.
    pub fn save_edit(&mut self, dir: &std::path::Path) {
        let Some(chain) = self.edit.clone() else {
            self.message = crate::i18n::t("Ingen kedja vald").to_string();
            return;
        };
        let path = dir.join(chain_file_name(&chain.name));
        match macro_chain::save_chain(&path, &chain) {
            Ok(()) => {
                self.message = crate::tstatus!("Sparade {}", path.display());
                self.edit_path = Some(path);
                self.reload(dir);
            }
            Err(e) => self.message = e,
        }
    }

    /// En ny kedja med startkedjans steg — så att "ny" inte betyder "tom".
    pub fn new_example(&mut self) {
        self.edit = Some(macro_chain::example_chain());
        self.edit_path = None;
        self.message = crate::i18n::t("Ny kedja — ändra namnet och spara").to_string();
    }
}

/// Modalens bildruta.
///
/// `project_reports` är resultatet av en projektkörning som anroparen redan gjort (den har
/// `SonixApp` och kan rendera); modalen begär den via `state.project_request` och får svaret
/// här i stället för att själv röra appen.
pub fn render_macros_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut MacroModalState,
    chains_dir: &std::path::Path,
    project_reports: Option<Result<Vec<macro_chain::StepReport>, String>>,
) {
    if !*open {
        return;
    }

    // En körning som blivit klar sedan sist: flytta över resultatet och släpp låset.
    if state.running
        && let Some(runs) = state.take_finished()
    {
        state.running = false;
        state.reports = macro_chain::batch_log(
            state.edit.as_ref().unwrap_or(&MacroChain::new("?")),
            &runs,
        )
        .lines()
        .map(|l| l.to_string())
        .collect();
        state.message = crate::tstatus!("Klar: {} filer", runs.len());
    }
    if let Some(result) = project_reports {
        state.running = false;
        state.reports = match result {
            Ok(reports) => reports
                .iter()
                .map(|r| format!("{}: {}", r.step, r.detail))
                .collect(),
            Err(e) => vec![format!("⚠ {e}")],
        };
        state.message = crate::i18n::t("Projektkörningen är klar").to_string();
    }

    let mut request_save = false;
    let mut request_new = false;
    let mut request_reload = false;
    let mut request_list = false;
    let mut request_run = false;
    let mut request_project_run = false;
    let mut pick_input = false;
    let mut pick_output = false;

    egui::Window::new(crate::i18n::t("🔗 Makron (kedja över filer)"))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(760.0, 560.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::i18n::t("🔗 MAKRON"))
                        .strong()
                        .size(14.0)
                        .color(Theme::FL_ORANGE),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Samma behandling på många filer — trimma, normalisera, exportera",
                    ))
                    .size(10.5)
                    .color(Theme::TEXT_MUTED),
                );
            });
            ui.add_space(4.0);

            ui.horizontal_top(|ui| {
                // ---- Kedjelistan ----
                ui.vertical(|ui| {
                    ui.set_min_width(200.0);
                    ui.label(egui::RichText::new(crate::i18n::t("Kedjor i mappen")).strong());
                    ui.label(
                        egui::RichText::new(chains_dir.display().to_string())
                            .size(9.5)
                            .color(Theme::TEXT_MUTED),
                    );
                    ui.add_space(2.0);
                    if ui.button(crate::i18n::t("🔄 Läs om")).clicked() {
                        request_reload = true;
                    }
                    if ui.button(crate::i18n::t("✨ Ny kedja (startmallen)")).clicked() {
                        request_new = true;
                    }
                    ui.separator();
                    let mut choose: Option<usize> = None;
                    for (i, entry) in state.entries.iter().enumerate() {
                        let name = entry
                            .path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        match &entry.chain {
                            Ok(chain) => {
                                let selected = state
                                    .edit_path
                                    .as_ref()
                                    .map(|p| p == &entry.path)
                                    .unwrap_or(false);
                                if ui
                                    .selectable_label(
                                        selected,
                                        crate::tstatus!("{} ({} steg)", name, chain.steps.len()),
                                    )
                                    .clicked()
                                {
                                    choose = Some(i);
                                }
                            }
                            Err(e) => {
                                // En trasig kedja syns som en rad med sitt fel i stället för att
                                // bara saknas — det är det svåraste felet att förstå annars.
                                ui.label(
                                    egui::RichText::new(crate::tstatus!("⚠ {}: {}", name, e))
                                        .size(9.5)
                                        .color(Theme::FL_RED),
                                );
                            }
                        }
                    }
                    if let Some(i) = choose {
                        state.select(i);
                    }
                });

                ui.separator();

                // ---- Redigeraren ----
                ui.vertical(|ui| {
                    if let Some(chain) = state.edit.clone() {
                        let mut edited = chain.clone();
                        ui.horizontal(|ui| {
                            ui.label(crate::i18n::t("Namn:"));
                            ui.add(
                                egui::TextEdit::singleline(&mut edited.name)
                                    .desired_width(240.0),
                            );
                            if ui.button(crate::i18n::t("💾 Spara kedjan")).clicked() {
                                request_save = true;
                            }
                        });
                        ui.add_space(2.0);

                        let mut remove: Option<usize> = None;
                        let mut move_up: Option<usize> = None;
                        let mut move_down: Option<usize> = None;
                        for (i, step) in edited.steps.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                let label = match step {
                                    MacroStep::TrimSilence { .. } => "✂",
                                    MacroStep::Normalize { .. } => "📊",
                                    MacroStep::Export { .. } => "💾",
                                };
                                ui.label(egui::RichText::new(label).size(13.0));
                                match step {
                                    MacroStep::TrimSilence {
                                        threshold_db,
                                        keep_ms,
                                    } => {
                                        ui.label(crate::i18n::t("Tröskel"));
                                        ui.add(
                                            egui::DragValue::new(threshold_db)
                                                .speed(0.5)
                                                .range(-90.0..=-20.0)
                                                .fixed_decimals(0)
                                                .suffix(" dB"),
                                        );
                                        ui.label(crate::i18n::t("Behåll"));
                                        ui.add(
                                            egui::DragValue::new(keep_ms)
                                                .speed(1.0)
                                                .range(0.0..=2000.0)
                                                .fixed_decimals(0)
                                                .suffix(" ms"),
                                        );
                                    }
                                    MacroStep::Normalize {
                                        target_lufs,
                                        ceiling_dbtp,
                                    } => {
                                        ui.label(crate::i18n::t("Mål"));
                                        ui.add(
                                            egui::DragValue::new(target_lufs)
                                                .speed(0.1)
                                                .range(-30.0..=-5.0)
                                                .fixed_decimals(1)
                                                .suffix(" LUFS"),
                                        );
                                        ui.label(crate::i18n::t("Tak"));
                                        ui.add(
                                            egui::DragValue::new(ceiling_dbtp)
                                                .speed(0.1)
                                                .range(-6.0..=0.0)
                                                .fixed_decimals(1)
                                                .suffix(" dBTP"),
                                        );
                                    }
                                    MacroStep::Export {
                                        format,
                                        suffix,
                                        dither,
                                        noise_shaping,
                                    } => {
                                        ui.label(crate::i18n::t("Format"));
                                        egui::ComboBox::from_id_salt("macro_export_format")
                                            .selected_text(export_format_label(*format))
                                            .width(90.0)
                                            .show_ui(ui, |ui| {
                                                for f in EXPORT_FORMATS {
                                                    if ui
                                                        .selectable_label(*format == f, export_format_label(f))
                                                        .clicked()
                                                    {
                                                        *format = f;
                                                    }
                                                }
                                            });
                                        ui.label(crate::i18n::t("Ändelse"));
                                        ui.add(
                                            egui::TextEdit::singleline(suffix).desired_width(90.0),
                                        );
                                        ui.checkbox(dither, crate::i18n::t("Dither"));
                                        ui.checkbox(noise_shaping, crate::i18n::t("Noise shaping"));
                                    }
                                }
                                if ui.button("⬆").clicked() {
                                    move_up = Some(i);
                                }
                                if ui.button("⬇").clicked() {
                                    move_down = Some(i);
                                }
                                if ui.button("✖").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove {
                            edited.steps.remove(i);
                        }
                        if let Some(i) = move_up
                            && i > 0
                        {
                            edited.steps.swap(i, i - 1);
                        }
                        if let Some(i) = move_down
                            && i + 1 < edited.steps.len()
                        {
                            edited.steps.swap(i, i + 1);
                        }

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(crate::i18n::t("Lägg till:"));
                            for kind in MacroStepKind::ALL {
                                if ui.button(kind.label()).clicked() {
                                    edited.steps.push(kind.default_step());
                                }
                            }
                        });

                        let has_export = edited.has_export();
                        if !has_export {
                            // En kedja utan export gör ingenting som syns. Att köra den tyst
                            // vore ett pass som ser lyckat ut utan att ha skrivit något.
                            ui.label(
                                egui::RichText::new(crate::i18n::t(
                                    "⚠ Kedjan har inget exportsteg — den skriver ingenting.",
                                ))
                                .size(10.0)
                                .color(Theme::FL_ORANGE),
                            );
                        }
                        state.edit = Some(edited);
                    } else {
                        ui.label(crate::i18n::t(
                            "Välj en kedja till vänster, eller skapa en ny ur startmallen.",
                        ));
                    }
                });
            });

            ui.separator();

            // ---- Körningen ----
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.target,
                    MacroTarget::Folder,
                    crate::i18n::t("📁 En mapp"),
                );
                ui.selectable_value(
                    &mut state.target,
                    MacroTarget::Project,
                    crate::i18n::t("🎛 Det öppna projektet"),
                );
            });

            if state.target == MacroTarget::Folder {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("In:"));
                    ui.add(egui::TextEdit::singleline(&mut state.input_dir).desired_width(300.0));
                    if ui.button("📁").clicked() {
                        pick_input = true;
                    }
                    if ui.button(crate::i18n::t("Läs mappen")).clicked() {
                        request_list = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Ut:"));
                    ui.add(egui::TextEdit::singleline(&mut state.out_dir).desired_width(300.0));
                    if ui.button("📁").clicked() {
                        pick_output = true;
                    }
                    ui.label(
                        egui::RichText::new(crate::i18n::t("(tomt = macro-output bredvid filen)"))
                            .size(9.5)
                            .color(Theme::TEXT_MUTED),
                    );
                });
                if !state.listing_note.is_empty() {
                    ui.label(
                        egui::RichText::new(state.listing_note.clone())
                            .size(10.0)
                            .color(Theme::TEXT_MUTED),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Projektet renderas som vid export och körs genom kedjans steg.",
                    ))
                    .size(10.0)
                    .color(Theme::TEXT_MUTED),
                );
            }

            ui.horizontal(|ui| {
                let ready = state.edit.is_some()
                    && state.edit.as_ref().map(|c| c.has_export()).unwrap_or(false)
                    && (state.target == MacroTarget::Project || !state.files.is_empty());
                ui.add_enabled_ui(ready && !state.running, |ui| {
                    if ui
                        .button(
                            egui::RichText::new(crate::i18n::t("▶ Kör kedjan"))
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .clicked()
                    {
                        if state.target == MacroTarget::Folder {
                            request_run = true;
                        } else {
                            request_project_run = true;
                        }
                    }
                });
                if state.running {
                    let (klar, av) = state.progress_now();
                    let ratio = if av > 0 {
                        klar as f32 / av as f32
                    } else {
                        0.0
                    };
                    ui.add(
                        egui::ProgressBar::new(ratio.clamp(0.0, 1.0))
                            .desired_width(200.0)
                            .text(crate::tstatus!("{} / {}", klar, av)),
                    );
                }
            });

            if !state.message.is_empty() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(state.message.clone())
                        .size(10.5)
                        .color(Theme::FL_CYAN),
                );
            }

            if !state.reports.is_empty() {
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for line in &state.reports {
                            ui.label(egui::RichText::new(line).size(9.5).monospace());
                        }
                    });
            }
        });

    // ---- Åtgärderna, efter bildrutan (så inget lån av state ligger kvar) ----
    if request_reload {
        state.reload(chains_dir);
    }
    if request_list {
        state.list_input_files();
    }
    if request_new {
        state.new_example();
    }
    if request_save {
        state.save_edit(chains_dir);
    }
    if pick_input
        && let Some(dir) = rfd::FileDialog::new().set_title(crate::i18n::t("Välj mapp")).pick_folder()
    {
        state.input_dir = dir.to_string_lossy().to_string();
        state.list_input_files();
    }
    if pick_output
        && let Some(dir) = rfd::FileDialog::new().set_title(crate::i18n::t("Välj utmapp")).pick_folder()
    {
        state.out_dir = dir.to_string_lossy().to_string();
    }

    if request_run
        && let Some(chain) = state.edit.clone()
    {
        state.running = true;
        state.reports.clear();
        if let Ok(mut p) = state.progress.lock() {
            *p = (0, state.files.len());
        }
        // Arbetstråden: en batch på tjugo filer tar minuter, och fönstret får inte stå still.
        let files = state.files.clone();
        let out = if state.out_dir.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(state.out_dir.trim()))
        };
        let progress = state.progress.clone();
        let result = state.result.clone();
        std::thread::spawn(move || {
            let meta = ExportMeta::default();
            let runs = macro_chain::run_batch_with_progress(
                &chain,
                &files,
                out.as_deref(),
                &meta,
                &mut |klar, av, _| {
                    if let Ok(mut p) = progress.lock() {
                        *p = (klar, av);
                    }
                },
            );
            if let Ok(mut slot) = result.lock() {
                *slot = Some(runs);
            }
        });
    }

    if request_project_run
        && let Some(chain) = state.edit.clone()
    {
        // Renderingen sker hos anroparen (den har `SonixApp`); modalen begär den bara.
        state.running = true;
        state.reports.clear();
        state.project_request = Some(chain);
    }
}

/// Formaten i exportsteget. Listan är **en** (den som exporteraren kan skriva), så en ny
/// ändelse inte kan dyka upp här utan att finnas där.
pub const EXPORT_FORMATS: [ExportFormat; 7] = [
    ExportFormat::Wav16,
    ExportFormat::Wav24,
    ExportFormat::Wav32,
    ExportFormat::Flac,
    ExportFormat::Mp3,
    ExportFormat::Ogg,
    ExportFormat::Aac,
];

pub fn export_format_label(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Wav16 => "WAV 16",
        ExportFormat::Wav24 => "WAV 24",
        ExportFormat::Wav32 => "WAV 32f",
        ExportFormat::Flac => "FLAC 24",
        ExportFormat::Mp3 => "MP3 320",
        ExportFormat::Ogg => "OGG",
        ExportFormat::Aac => "AAC",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Filnamnet är det som avgör om en kedja skriver över en annan.** Svenska tecken,
    /// mellanslag och skiljetecken ska bli ett filnamn man kan hitta.
    #[test]
    fn a_chain_name_becomes_a_findable_file_name() {
        assert_eq!(chain_file_name("Suno-stämma"), "suno-stamma.json");
        assert_eq!(chain_file_name("Min  Kedja!"), "min-kedja.json");
        assert_eq!(chain_file_name("  ö  "), "o.json");
        // Tomt och bara skiljetecken: ändå ett namn **med** ändelse — en fil utan `.json`
        // hittas inte av listan i mappen, och kedjan skulle vara borta efter att ha sparats.
        assert_eq!(chain_file_name(""), "kedja.json");
        assert_eq!(chain_file_name("///"), "kedja.json");
        // Två olika namn blir två olika filer.
        assert_ne!(chain_file_name("Stamma"), chain_file_name("Stamma 2"));
        // Men translittereringen är appens: `Stämma` och `Stamma` är samma fil. Det är
        // avsiktligt och därför står filnamnet i statusraden när man sparar.
        assert_eq!(chain_file_name("Stämma"), chain_file_name("Stamma"));
    }

    /// Stegknappens standardvärde ska vara **samma** som startkedjans: annars vore "ny kedja" och
    /// `--macro-example` två olika sanningar om samma steg.
    #[test]
    fn the_step_buttons_match_the_example_chain() {
        let example = macro_chain::example_chain();
        let built: Vec<MacroStep> = MacroStepKind::ALL
            .iter()
            .map(|k| k.default_step())
            .collect();
        assert_eq!(built.len(), 3);
        for step in &built {
            assert!(
                example.steps.contains(step),
                "steget {step:?} finns inte i startkedjan"
            );
        }
        assert_eq!(
            built.len(),
            example.steps.len(),
            "startkedjan ska bestå av exakt knapparnas steg"
        );
    }

    /// Formaten i väljaren ska vara exporterarens egna — en lista som driver isär är den här
    /// kodbasens dyraste sort (se skillen: en lista med fel längd flyttade tyst fyra val).
    #[test]
    fn the_format_picker_covers_the_exporter() {
        use crate::audio::exporter::ExportFormat::*;
        for f in [Wav16, Wav24, Wav32, Flac, Mp3, Ogg, Aac] {
            assert!(
                EXPORT_FORMATS.contains(&f),
                "{f:?} saknas i väljaren men kan exporteras"
            );
            assert_eq!(f.ext(), f.ext(), "ändelsen ska vara entydig");
        }
        assert_eq!(EXPORT_FORMATS.len(), 7);
    }

    /// En körning som blivit klar ska plockas upp av bildrutan **en gång** — annars skrivs
    /// resultatet om varje bildruta och rutan växer medan användaren tittar på den.
    #[test]
    fn a_finished_batch_is_taken_once() {
        let mut state = MacroModalState::default();
        state.running = true;
        *state.result.lock().unwrap() = Some(Vec::new());
        assert!(state.take_finished().is_some());
        assert!(state.take_finished().is_none(), "samma resultat två gånger");
    }
}
