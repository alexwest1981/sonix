use eframe::egui::Color32;

#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // FL Studio Dark Metallic Charcoal Backgrounds
    pub const BG_DARK: Color32 = Color32::from_rgb(18, 20, 24);
    pub const HEADER_BG: Color32 = Color32::from_rgb(26, 30, 36);
    pub const PANEL_BG: Color32 = Color32::from_rgb(28, 32, 40);
    pub const CHANNEL_BG: Color32 = Color32::from_rgb(34, 38, 46);
    pub const LCD_BG: Color32 = Color32::from_rgb(10, 14, 18);
    pub const LCD_TEXT: Color32 = Color32::from_rgb(0, 240, 255);
    pub const LCD_ORANGE: Color32 = Color32::from_rgb(255, 150, 20);

    // FL Studio Iconic Accents
    pub const FL_ORANGE: Color32 = Color32::from_rgb(255, 128, 0);
    pub const FL_GREEN: Color32 = Color32::from_rgb(46, 204, 113);
    pub const FL_RED: Color32 = Color32::from_rgb(231, 76, 60);
    pub const FL_CYAN: Color32 = Color32::from_rgb(0, 210, 255);
    pub const FL_PURPLE: Color32 = Color32::from_rgb(165, 94, 234);
    pub const FL_YELLOW: Color32 = Color32::from_rgb(255, 215, 0);

    // Text & Muted
    pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(240, 244, 250);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(140, 150, 168);
    pub const BORDER_DARK: Color32 = Color32::from_rgb(45, 52, 65);
}
