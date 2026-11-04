//! **Ikonerna behöver ett typsnitt som har dem.**
//!
//! Repot hade inga `FontDefinitions` alls: emoji ritades med typsnittets egen fallback, och
//! på en maskin som bara har `NotoColorEmoji` blir varje ikon i gränssnittet en tom ruta —
//! den fonten är CBDT-bitmap och kan inte rastreras av `ab_glyph`. Det är därför ikonerna
//! ibland syntes och ibland inte (designrunda 2: 197 ikoner i ytan).
//!
//! OpenMoji-black är en vanlig monokrom TTF med täckning för alla ikoner appen använder
//! (prövat mot typsnittets teckenkarta). Den läggs **sist** i fallback-kedjan, så text tar de
//! vanliga typsnitten och bara glyfer som saknas hamnar här.
//!
//! Licens: CC BY-SA 4.0, se `assets/fonts/OpenMoji-LICENSE.txt`.

use eframe::egui;

/// Kallas **en gång**, ur `CreationContext` — aldrig ur `update`. `apply_style` körs varje
/// bildruta, och att sätta typsnitt där skulle bygga om atlasen 60 gånger i sekunden.
pub fn install(cc: &eframe::CreationContext<'_>) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "openmoji".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/OpenMoji-black-glyf.ttf")),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("openmoji".to_owned());
    }
    cc.egui_ctx.set_fonts(fonts);
}
