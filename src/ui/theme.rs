use eframe::egui::Color32;

#[allow(dead_code)]
pub struct Theme;

/// **Paletten.** 23 roller, inga literala färger utanför den här filen.
///
/// Färgerna är grupperade efter vad de *betyder*, inte efter vilken skärm de kom ifrån:
/// ytor, kanter, text, accenter, spårfärger och datafärger. En accent får bara betyda en sak —
/// orange = aktivt/markerat/primär handling, grön = ok/redo, röd = fel/rec, cyan = klipp och vald
/// rad, lila = bas och modulering, gul = varning.
#[allow(dead_code)]
impl Theme {
    // --- Ytor, från mörkast till ljusast -------------------------------------
    /// appens botten
    pub const BG_DEEP: Color32 = Color32::from_rgb(12, 15, 20);
    /// panelbakgrund
    pub const BG: Color32 = Color32::from_rgb(18, 20, 24);
    /// meny- och verktygsrad
    pub const HEADER: Color32 = Color32::from_rgb(26, 30, 36);
    /// förhöjd panel
    pub const PANEL: Color32 = Color32::from_rgb(28, 32, 40);
    /// kanalremsa
    pub const PANEL_ALT: Color32 = Color32::from_rgb(34, 38, 46);

    // --- Kanter och rutnät --------------------------------------------------
    /// rutnät och avdelare inuti paneler
    pub const GRID: Color32 = Color32::from_rgb(32, 38, 48);
    /// kant
    pub const BORDER: Color32 = Color32::from_rgb(45, 52, 65);
    /// kant på mörk yta
    pub const BORDER_SOFT: Color32 = Color32::from_rgb(60, 75, 95);

    // --- Text ---------------------------------------------------------------
    /// primär text
    pub const TEXT: Color32 = Color32::from_rgb(240, 244, 250);
    /// sekundär text
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(140, 150, 168);

    // --- Accenter: en betydelse var -----------------------------------------
    /// primär accent: aktivt läge, markering, CTA
    pub const ORANGE: Color32 = Color32::from_rgb(255, 140, 25);
    /// ok/redo/export
    pub const GREEN: Color32 = Color32::from_rgb(46, 204, 113);
    /// fel/rec/mute
    pub const RED: Color32 = Color32::from_rgb(231, 76, 60);
    /// sekundär accent: klipp, vald rad
    pub const CYAN: Color32 = Color32::from_rgb(0, 215, 255);
    /// tredje accent: bas, modulering
    pub const PURPLE: Color32 = Color32::from_rgb(172, 97, 245);
    /// varning/badge
    pub const YELLOW: Color32 = Color32::from_rgb(255, 215, 0);

    // --- Spårfärger (innehåll, inte krom) ----------------------------------


    // --- Spårfärger (innehåll, inte krom) ----------------------------------
    // Dämpade med flit: en spårton får aldrig se ut som en accent, annars betyder
    // orange både "aktiverad" och "trumspår".
    /// sång — dämpad cyanblå
    pub const TRACK_CYAN: Color32 = Color32::from_rgb(44, 132, 180);
    /// kör — dämpad magenta
    pub const TRACK_MAGENTA: Color32 = Color32::from_rgb(148, 84, 176);
    /// trummor — dämpad orange
    pub const TRACK_ORANGE: Color32 = Color32::from_rgb(200, 112, 40);
    /// bas — dämpad grön
    pub const TRACK_GREEN: Color32 = Color32::from_rgb(46, 148, 104);
    /// lead — dämpad teal
    pub const TRACK_TEAL: Color32 = Color32::from_rgb(52, 140, 150);
    /// synth — dämpad blå
    pub const TRACK_BLUE: Color32 = Color32::from_rgb(88, 128, 196);
    /// fx — dämpad rosa
    pub const TRACK_ROSE: Color32 = Color32::from_rgb(168, 84, 112);

    /// percussion — dämpad guld
    pub const TRACK_AMBER: Color32 = Color32::from_rgb(176, 140, 52);
    /// gitarr — dämpad violett
    pub const TRACK_VIOLET: Color32 = Color32::from_rgb(124, 104, 190);
    /// synth — dämpat stål
    pub const TRACK_STEEL: Color32 = Color32::from_rgb(108, 120, 142);

    // --- Data: kablar, kurvor, badge --------------------------------------
    /// patchkabel
    pub const CABLE: Color32 = Color32::from_rgb(160, 140, 255);
    /// varm badge/text
    pub const AMBER: Color32 = Color32::from_rgb(230, 160, 80);

    // --- Typografisk skala: fem steg ---------------------------------------
    pub const FONT_XS: f32 = 9.0;
    pub const FONT_S: f32 = 11.0;
    pub const FONT_M: f32 = 13.0;
    pub const FONT_L: f32 = 16.0;
    pub const FONT_XL: f32 = 22.0;

    /// Samma färg med annan genomskinlighet — så att en ton inte behöver skrivas av för hand.
    pub fn alpha(c: Color32, a: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
    }

    /// Dämpning bakom dialoger: en dialog ska vara det enda som lyser.
    pub const SCRIM: Color32 = Color32::from_black_alpha(120);
}
