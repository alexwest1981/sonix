use eframe::egui::Color32;

/// **Typografisk skala.** Färgerna bor i [`Palette`], storlekarna här — en temafil ska inte
/// kunna göra texten oläsbar genom att sätta 3 px.
#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    pub const FONT_XS: f32 = 9.0;
    pub const FONT_S: f32 = 11.0;
    pub const FONT_M: f32 = 13.0;
    pub const FONT_L: f32 = 16.0;
    pub const FONT_XL: f32 = 22.0;

    /// Samma färg med annan genomskinlighet — så att en ton inte behöver skrivas av för hand.
    pub fn alpha(c: Color32, a: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
    }
}

fn rgba([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// **Paletten: 29 roller, semantiska namn.** Färgerna är grupperade efter vad de *betyder*,
/// inte efter vilken skärm de kom ifrån, och en accent får bara betyda en sak — orange =
/// aktivt, grön = ok, röd = fel, cyan = klipp, lila = bas/modulering, gul = varning.
///
/// Fälten är `[r, g, b, a]` och inte `Color32`, eftersom egui inte kan spara sin färgtyp som
/// JSON. En temafil skriver bara de nycklar den vill ändra:
///
/// ```json
/// { "orange": [255, 90, 0, 255], "bg": [10, 10, 12, 255] }
/// ```
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Palette {
    /// `BG_DEEP`
    pub bg_deep: [u8; 4],
    /// `BG`
    pub bg: [u8; 4],
    /// `HEADER`
    pub header: [u8; 4],
    /// `PANEL`
    pub panel: [u8; 4],
    /// `PANEL_ALT`
    pub panel_alt: [u8; 4],
    /// `GRID`
    pub grid: [u8; 4],
    /// `BORDER`
    pub border: [u8; 4],
    /// `BORDER_SOFT`
    pub border_soft: [u8; 4],
    /// `TEXT`
    pub text: [u8; 4],
    /// `TEXT_MUTED`
    pub text_muted: [u8; 4],
    /// `ORANGE`
    pub orange: [u8; 4],
    /// `GREEN`
    pub green: [u8; 4],
    /// `RED`
    pub red: [u8; 4],
    /// `CYAN`
    pub cyan: [u8; 4],
    /// `PURPLE`
    pub purple: [u8; 4],
    /// `YELLOW`
    pub yellow: [u8; 4],
    /// `TRACK_CYAN`
    pub track_cyan: [u8; 4],
    /// `TRACK_MAGENTA`
    pub track_magenta: [u8; 4],
    /// `TRACK_ORANGE`
    pub track_orange: [u8; 4],
    /// `TRACK_GREEN`
    pub track_green: [u8; 4],
    /// `TRACK_TEAL`
    pub track_teal: [u8; 4],
    /// `TRACK_BLUE`
    pub track_blue: [u8; 4],
    /// `TRACK_ROSE`
    pub track_rose: [u8; 4],
    /// `TRACK_AMBER`
    pub track_amber: [u8; 4],
    /// `TRACK_VIOLET`
    pub track_violet: [u8; 4],
    /// `TRACK_STEEL`
    pub track_steel: [u8; 4],
    /// `CABLE`
    pub cable: [u8; 4],
    /// `AMBER`
    pub amber: [u8; 4],
    /// `SCRIM`
    pub scrim: [u8; 4],
}

impl Palette {
    pub fn bg_deep(&self) -> Color32 {
        rgba(self.bg_deep)
    }

    pub fn bg(&self) -> Color32 {
        rgba(self.bg)
    }

    pub fn header(&self) -> Color32 {
        rgba(self.header)
    }

    pub fn panel(&self) -> Color32 {
        rgba(self.panel)
    }

    pub fn panel_alt(&self) -> Color32 {
        rgba(self.panel_alt)
    }

    pub fn grid(&self) -> Color32 {
        rgba(self.grid)
    }

    pub fn border(&self) -> Color32 {
        rgba(self.border)
    }

    pub fn border_soft(&self) -> Color32 {
        rgba(self.border_soft)
    }

    pub fn text(&self) -> Color32 {
        rgba(self.text)
    }

    pub fn text_muted(&self) -> Color32 {
        rgba(self.text_muted)
    }

    pub fn orange(&self) -> Color32 {
        rgba(self.orange)
    }

    pub fn green(&self) -> Color32 {
        rgba(self.green)
    }

    pub fn red(&self) -> Color32 {
        rgba(self.red)
    }

    pub fn cyan(&self) -> Color32 {
        rgba(self.cyan)
    }

    pub fn purple(&self) -> Color32 {
        rgba(self.purple)
    }

    pub fn yellow(&self) -> Color32 {
        rgba(self.yellow)
    }

    pub fn track_cyan(&self) -> Color32 {
        rgba(self.track_cyan)
    }

    pub fn track_magenta(&self) -> Color32 {
        rgba(self.track_magenta)
    }

    pub fn track_orange(&self) -> Color32 {
        rgba(self.track_orange)
    }

    pub fn track_green(&self) -> Color32 {
        rgba(self.track_green)
    }

    pub fn track_teal(&self) -> Color32 {
        rgba(self.track_teal)
    }

    pub fn track_blue(&self) -> Color32 {
        rgba(self.track_blue)
    }

    pub fn track_rose(&self) -> Color32 {
        rgba(self.track_rose)
    }

    pub fn track_amber(&self) -> Color32 {
        rgba(self.track_amber)
    }

    pub fn track_violet(&self) -> Color32 {
        rgba(self.track_violet)
    }

    pub fn track_steel(&self) -> Color32 {
        rgba(self.track_steel)
    }

    pub fn cable(&self) -> Color32 {
        rgba(self.cable)
    }

    pub fn amber(&self) -> Color32 {
        rgba(self.amber)
    }

    pub fn scrim(&self) -> Color32 {
        rgba(self.scrim)
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bg_deep: [12, 15, 20, 255],
            bg: [18, 20, 24, 255],
            header: [26, 30, 36, 255],
            panel: [28, 32, 40, 255],
            panel_alt: [34, 38, 46, 255],
            grid: [32, 38, 48, 255],
            border: [45, 52, 65, 255],
            border_soft: [60, 75, 95, 255],
            text: [240, 244, 250, 255],
            text_muted: [140, 150, 168, 255],
            orange: [255, 140, 25, 255],
            green: [46, 204, 113, 255],
            red: [231, 76, 60, 255],
            cyan: [0, 215, 255, 255],
            purple: [172, 97, 245, 255],
            yellow: [255, 215, 0, 255],
            track_cyan: [44, 132, 180, 255],
            track_magenta: [148, 84, 176, 255],
            track_orange: [200, 112, 40, 255],
            track_green: [46, 148, 104, 255],
            track_teal: [52, 140, 150, 255],
            track_blue: [88, 128, 196, 255],
            track_rose: [168, 84, 112, 255],
            track_amber: [176, 140, 52, 255],
            track_violet: [124, 104, 190, 255],
            track_steel: [108, 120, 142, 255],
            cable: [160, 140, 255, 255],
            amber: [230, 160, 80, 255],
            scrim: [0, 0, 0, 120],
        }
    }
}

/// Paletten som ritas. Läses **en gång** från `~/.config/sonix/theme.json`; saknas filen
/// skrivs standardpaletten dit så att den går att redigera, och en trasig fil ger standard
/// (en temafil får aldrig hindra appen från att starta).
pub fn palette() -> &'static Palette {
    static PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| {
        let path = theme_path();
        if !path.exists() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_string_pretty(&Palette::default()) {
                let _ = std::fs::write(&path, compact_arrays(&json));
            }
            return Palette::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    })
}

/// `serde_json` lägger varje tal i en array på egen rad; en färg blir fem rader. Filen finns
/// för att redigeras för hand, så `[12, 15, 20, 255]` står på en rad i stället.
fn compact_arrays(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    // `färg = [12, 15, 20, 255]` byggs i en buffert och skrivs ut när stängningen kommer.
    let mut color: Option<String> = None;
    for line in json.lines() {
        let trimmed = line.trim();
        if let Some(values) = color.as_mut() {
            if trimmed == "]" || trimmed == "]," {
                out.push_str(values.trim_end_matches(' '));
                out.push(']');
                // Kommat hör till raden som stängde arrayen; annars blir sista färgen en
                // avslutande komma och filen går inte att läsa in igen.
                if trimmed.ends_with(',') {
                    out.push(',');
                }
                out.push('\n');
                color = None;
            } else {
                values.push_str(trimmed);
                values.push(' ');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if trimmed.ends_with('[') {
            out.pop(); // färgens värden ska stå på samma rad som `[`
            color = Some(String::new());
        }
    }
    out
}

pub fn theme_path() -> std::path::PathBuf {
    // Sökvägar byggs bara i `paths.rs` — även de här (SONIX_CONFIG_DIR och XDG styr).
    crate::paths::paths().config_dir().join("theme.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En temafil behöver bara skriva det den ändrar; resten är standard.
    #[test]
    fn a_theme_file_only_has_to_name_what_it_changes() {
        let p: Palette = serde_json::from_str(r#"{ "orange": [255, 90, 0, 255] }"#).unwrap();
        assert_eq!(p.orange(), Color32::from_rgb(255, 90, 0));
        assert_eq!(p.bg(), Palette::default().bg(), "orörd nyckel behåller standard");
    }

    /// Temat hamnar där `XDG_CONFIG_HOME` säger, inte alltid i `~/.config`.
    #[test]
    fn the_theme_path_follows_xdg_config_home() {
        // Provet rör bara sökvägen; `palette()` läses en gång och cachar.
        let path = theme_path();
        assert!(path.ends_with("sonix/theme.json"));
    }

    /// En färg skrivs på en rad, så filen går att läsa för hand.
    #[test]
    fn colors_are_written_on_one_line() {
        let json = compact_arrays(&serde_json::to_string_pretty(&Palette::default()).unwrap());
        assert!(json.contains("\"bg\": [18, 20, 24, 255]"), "{json}");
        // Den kompakterade filen måste fortfarande vara JSON, annars går den inte att läsa in.
        let back: Palette = serde_json::from_str(&json).expect("compact_arrays ska ge giltig JSON");
        assert_eq!(back, Palette::default());
    }

    /// Paletten går att spara och läsa tillbaka utan att tappa en nyans.
    #[test]
    fn the_palette_round_trips_through_json() {
        let mut p = Palette::default();
        p.scrim = [0, 0, 0, 77];
        p.track_teal = [1, 2, 3, 4];
        let json = serde_json::to_string(&p).unwrap();
        let back: Palette = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
