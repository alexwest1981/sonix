//! Makron-modalen i appen (Fas 8.9 steg 2): inkopplingen till `ui::macros_modal`.
//!
//! Modalen äger sin egen bildruta och sitt tillstånd (som de andra dialogerna); den här filen
//! äger det enda som kräver `SonixApp` — **projektkörningen**: rendera projektet offline och kör
//! kedjans steg på bufferten. Det är Audacitys andra halva: en makrokedja ska kunna köras på
//! det öppna projektet, inte bara på filer.
//!
//! Status: byggs — steg 2.
//! Rör inte: exportens väg. Renderingen och skrivaren är **samma** som exportens
//! (`build_render_spec`/`render_buffer`/`write_export_with`), inte en parallell.

use super::*;

impl SonixApp {
    /// **Kedjan på det öppna projektet.** Renderar som exporten gör, kör kedjans buffertsteg
    /// (trimning, normalisering — samma funktioner som filvägen använder) och skriver med
    /// kedjans exportsteg.
    ///
    /// Tyst-ljud-doktrinen gäller även här: en kedja som gjorde renderingen tyst är ett fel, och
    /// då skrivs ingen fil.
    pub(crate) fn run_macro_on_project(
        &mut self,
        chain: &crate::audio::macro_chain::MacroChain,
    ) -> Result<Vec<crate::audio::macro_chain::StepReport>, String> {
        let sample_rate = self.export_sample_rate();
        let spec = self.build_render_spec(None, sample_rate);
        let mut buf = self.render_buffer(&spec, false)?;
        if buf.is_empty() {
            return Err(crate::i18n::t("Renderingen gav inget ljud — inget skrevs").to_string());
        }

        let reports = crate::audio::macro_chain::apply_steps_to_buffer(chain, &mut buf, sample_rate)?;

        let Some((format, suffix, dither, noise_shaping)) = chain.steps.iter().find_map(|s| match s {
            crate::audio::macro_chain::MacroStep::Export {
                format,
                suffix,
                dither,
                noise_shaping,
            } => Some((*format, suffix.clone(), *dither, *noise_shaping)),
            _ => None,
        }) else {
            return Err(crate::i18n::t("Kedjan har inget exportsteg — inget skrevs").to_string());
        };

        if crate::audio::loudness::true_peak_db(&buf) < -120.0 {
            return Err(crate::i18n::t("Kedjan gjorde projektet tyst — inget skrevs").to_string());
        }

        let dir = crate::paths::paths()
            .project_assets_dir(&self.project_name)
            .join("macro-output");
        std::fs::create_dir_all(&dir)
            .map_err(|e| crate::tstatus!("Kunde inte skapa utmappen {}: {}", dir.display(), e))?;
        let name = format!("{}{}.{}", self.project_name, suffix, format.ext());
        let path = dir.join(name);
        let settings = crate::audio::exporter::DitherSettings {
            enabled: dither,
            noise_shaping,
            seed: crate::audio::dither::DEFAULT_SEED,
        };
        let meta = crate::audio::exporter::ExportMeta {
            title: self.project_name.clone(),
            comment: "Renderad genom en makrokedja (Sonix)".to_string(),
            ..Default::default()
        };
        crate::audio::write_export_with(
            &path.to_string_lossy(),
            format,
            &buf,
            sample_rate,
            &meta,
            settings,
        )
        .map_err(|e| crate::tstatus!("Kunde inte skriva {}: {}", path.display(), e))?;
        // Läs tillbaka storleken: en skrivning som \"lyckades\" men lämnade en tom fil får inte se
        // ut som ett resultat.
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if bytes == 0 {
            return Err(crate::tstatus!(
                "Utdata {} är tom — filen skrevs inte korrekt",
                path.display()
            ));
        }
        let mut reports = reports;
        reports.push(crate::audio::macro_chain::StepReport {
            step: crate::i18n::t("Exportera").to_string(),
            detail: crate::tstatus!(
                "{} ({} byte, {} sampel)",
                path.display(),
                bytes,
                buf.len()
            ),
        });
        self.status_message = crate::tstatus!("✅ Makrokedjan skrev {}", path.display());
        Ok(reports)
    }

    /// Makron-modalen. En projektbegäran plockas ut här och utförs **medan `self` finns** —
    /// modalen ritar bara och kan inte rendera ett projekt själv.
    pub(crate) fn render_macros_modal_view(&mut self, ctx: &egui::Context) {
        let chains_dir = crate::paths::paths().macros_dir();
        let request = self.macro_state.project_request.take();
        let project_reports = request.map(|chain| self.run_macro_on_project(&chain));
        crate::ui::macros_modal::render_macros_modal(
            ctx,
            &mut self.show_macros_modal,
            &mut self.macro_state,
            &chains_dir,
            project_reports,
        );
    }
}
