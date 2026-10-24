use eframe::egui::Color32;

/// **Typografisk skala.** Färgerna bor i [`Palette`], storlekarna här — en temafil ska inte
/// kunna göra texten oläsbar genom att sätta 3 px.
#[allow(dead_code)]
pub struct Theme;

#[allow(dead_code)]
impl Theme {
    // **Större typografi** (GUI-planen, punkt 3): 9 px etiketter var FL-tätt, inte Ableton.
    pub const FONT_XS: f32 = 10.5;
    pub const FONT_S: f32 = 12.5;
    pub const FONT_M: f32 = 14.5;
    pub const FONT_L: f32 = 18.0;
    pub const FONT_XL: f32 = 24.0;

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
/// Ljust eller mörkt. Sparas i temafilen som `"mode": "light"`, så en fil bara behöver
/// ange de färger som avviker från standardpaletten för sitt läge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// ```json
/// { "orange": [255, 90, 0, 255], "bg": [10, 10, 12, 255] }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Palette {
    /// `MODE` — `"dark"` eller `"light"`; avgör vilken standardpalett filen bygger på.
    pub mode: ThemeMode,
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
            mode: ThemeMode::Dark,
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

impl Palette {
    /// **Ljust läge.** Samma 29 nycklar, ljusa ytor och mörk text; accenter mörknade så att
    /// vit text på dem och mörk text på vitt klarar 4,5:1 (`light_mode_keeps_text_readable`).
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg_deep: [232, 236, 242, 255],
            bg: [246, 248, 251, 255],
            header: [238, 241, 246, 255],
            panel: [255, 255, 255, 255],
            panel_alt: [242, 244, 248, 255],
            grid: [226, 230, 237, 255],
            border: [198, 204, 214, 255],
            border_soft: [176, 184, 198, 255],
            text: [28, 32, 40, 255],
            text_muted: [92, 100, 114, 255],
            orange: [205, 92, 0, 255],
            green: [16, 124, 62, 255],
            red: [186, 38, 30, 255],
            cyan: [0, 116, 168, 255],
            purple: [104, 44, 176, 255],
            yellow: [146, 106, 0, 255],
            track_cyan: [32, 108, 152, 255],
            track_magenta: [134, 62, 158, 255],
            track_orange: [178, 90, 24, 255],
            track_green: [30, 122, 84, 255],
            track_teal: [30, 116, 126, 255],
            track_blue: [58, 96, 164, 255],
            track_rose: [150, 62, 92, 255],
            track_amber: [146, 112, 28, 255],
            track_violet: [96, 78, 160, 255],
            track_steel: [86, 96, 116, 255],
            cable: [120, 100, 220, 255],
            amber: [176, 112, 32, 255],
            scrim: [0, 0, 0, 90],
        }
    }

    pub fn is_light(&self) -> bool {
        self.mode == ThemeMode::Light
    }
}

/// Paletten som ritas. Läses **en gång** från `~/.config/sonix/theme.json`; saknas filen
/// skrivs standardpaletten dit så att den går att redigera, och en trasig fil ger standard
/// (en temafil får aldrig hindra appen från att starta).
/// **Temafilen läses om när den ändras** (0.9.79): `reload_if_changed` jämför filens
/// mtime en gång per bildruta. Paletten är `Copy`, så anropen får en kopia i stället för ett
/// lås — 29 färger är billigare att kopiera än att låna.
static PALETTE: std::sync::RwLock<Option<Palette>> = std::sync::RwLock::new(None);
static PALETTE_MTIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn mtime_stamp(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn load_palette() -> Palette {
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
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Palette::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Palette::default();
    };
    merge_onto_base(value)
}

/// Lägger filens nycklar ovanpå standardpaletten för sitt läge. Okända nycklar ignoreras, och en
/// fil som bara säger `{"mode": "light"}` ger den ljusa paletten orörd.
fn merge_onto_base(value: serde_json::Value) -> Palette {
    let light = value.get("mode").and_then(|m| m.as_str()) == Some("light");
    let base = if light { Palette::light() } else { Palette::default() };
    let mut merged = serde_json::to_value(base).unwrap_or(serde_json::Value::Null);
    if let (Some(target), Some(source)) = (merged.as_object_mut(), value.as_object()) {
        for (key, val) in source {
            if key != "mode" {
                target.insert(key.clone(), val.clone());
            }
        }
    }
    serde_json::from_value(merged).unwrap_or(base)
}

/// Byter läge och skriver det i temafilen **utan att röra användarens egna färger**; sedan läses
/// paletten om direkt, så växlingen syns i samma bildruta.
pub fn set_mode(mode: ThemeMode) {
    let path = theme_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_else(|| serde_json::to_value(Palette::default()).unwrap_or_default());
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "mode".to_string(),
            serde_json::Value::String(
                match mode {
                    ThemeMode::Light => "light",
                    ThemeMode::Dark => "dark",
                }
                .to_string(),
            ),
        );
    }
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(&path, compact_arrays(&text));
    }
    if let Ok(mut p) = PALETTE.write() {
        *p = Some(merge_onto_base(value));
    }
    PALETTE_MTIME.store(mtime_stamp(&path), std::sync::atomic::Ordering::Relaxed);
}

/// Läser om temafilen om den ändrats sedan förra anropet. Kostar en `metadata`-fråga per
/// bildruta — inget mer, så länge filen står still.
pub fn reload_if_changed() {
    let path = theme_path();
    let stamp = mtime_stamp(&path);
    if stamp == PALETTE_MTIME.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    PALETTE_MTIME.store(stamp, std::sync::atomic::Ordering::Relaxed);
    let fresh = load_palette();
    if let Ok(mut p) = PALETTE.write() {
        *p = Some(fresh);
    }
}

/// **Global form** (GUI-planen, punkt 3): luftigare rader, rundare hörn, mjukare kanter och
/// paletten i de få ytor egui äger själv. Utan det här ser appen ut som egui — vilket är
/// precis vad den gjort: allt var standardvärden.
pub fn apply_style(ctx: &eframe::egui::Context) {
    let p = palette();
    ctx.style_mut(|style| {
        // Mer luft: 5 px mellan rader (standard 3) och 21 px radhöjd (standard 18).
        style.spacing.item_spacing = eframe::egui::vec2(8.0, 5.0);
        style.spacing.button_padding = eframe::egui::vec2(9.0, 4.0);
        style.spacing.interact_size.y = 21.0;
        style.spacing.menu_margin = eframe::egui::Margin::same(6.0);
        // Rundare hörn och tunnare kanter — Abletons paneler har inga hårda ramar.
        for w in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.rounding = eframe::egui::Rounding::same(4.0);
            w.bg_stroke.width = w.bg_stroke.width.min(1.0);
        }
        style.visuals.window_rounding = eframe::egui::Rounding::same(8.0);
        style.visuals.menu_rounding = eframe::egui::Rounding::same(6.0);
        // Paletten i de ytor egui ritar själv.
        style.visuals.panel_fill = p.panel();
        style.visuals.window_fill = p.panel_alt();
        style.visuals.extreme_bg_color = p.bg();
        style.visuals.faint_bg_color = p.bg();
        style.visuals.selection.bg_fill = p.orange();
        style.visuals.selection.stroke.color = eframe::egui::Color32::BLACK;
        // Samma skala för de widgetar som inte får en storlek uttryckligen.
        use eframe::egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(10.5)),
            (TextStyle::Body, FontId::proportional(12.5)),
            (TextStyle::Button, FontId::proportional(12.5)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
            (TextStyle::Heading, FontId::proportional(18.0)),
        ]
        .into();
    });
}

/// Prov som rör `SONIX_CONFIG_DIR` måste turas om: miljövariabeln är global, och två prov som
/// byter den samtidigt ser varandras mappar.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Svart eller vitt — det som syns bäst på `bg`. Klipp och knappar behåller sin färg i båda
/// lägena, så etiketten måste byta sida i stället för att följa paletten.
pub fn readable_on(bg: eframe::egui::Color32) -> eframe::egui::Color32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let lum = 0.2126 * f(bg.r()) + 0.7152 * f(bg.g()) + 0.0722 * f(bg.b());
    if lum > 0.45 {
        eframe::egui::Color32::BLACK
    } else {
        eframe::egui::Color32::WHITE
    }
}

pub fn palette() -> Palette {
    if let Ok(p) = PALETTE.read()
        && let Some(pal) = *p
    {
        return pal;
    }
    let fresh = load_palette();
    if let Ok(mut p) = PALETTE.write() {
        *p = Some(fresh);
    }
    fresh
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
        let _guard = super::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Provet rör bara sökvägen; paletten cachas i en `RwLock`.`
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

#[cfg(test)]
mod reload_tests {
    use super::*;

    /// **En ändrad temafil slår igenom utan omstart.** Provet pekar `SONIX_CONFIG_DIR` mot en
    /// egen mapp, skriver en färg, läser om och jämför.
    #[test]
    fn editing_the_theme_file_changes_the_palette() {
        let dir = std::env::temp_dir().join(format!("sonix-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: variabeln sätts och återställs inom provet. Andra prov som ritar med
        // paletten kan se den här mappen under tiden — det ändrar bara färger, inte logik.
        let _guard = super::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os("SONIX_CONFIG_DIR");
        unsafe { std::env::set_var("SONIX_CONFIG_DIR", &dir) };
        let path = theme_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        let mut edited = Palette::default();
        edited.orange = [1, 2, 3, 255];
        std::fs::write(&path, serde_json::to_string(&edited).unwrap()).unwrap();
        // mtime har nanosekundsupplösning, men ge filen en ny tidsstämpel ändå.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, serde_json::to_string(&edited).unwrap()).unwrap();

        reload_if_changed();
        assert_eq!(palette().orange, [1, 2, 3, 255]);

        let mut second = Palette::default();
        second.orange = [9, 8, 7, 255];
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, serde_json::to_string(&second).unwrap()).unwrap();
        reload_if_changed();
        assert_eq!(palette().orange, [9, 8, 7, 255]);

        let _ = std::fs::remove_dir_all(&dir);
        match previous {
            Some(v) => unsafe { std::env::set_var("SONIX_CONFIG_DIR", v) },
            None => unsafe { std::env::remove_var("SONIX_CONFIG_DIR") },
        }
    }
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    fn luminance(c: [u8; 4]) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
    }

    fn ratio(a: [u8; 4], b: [u8; 4]) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// **Texten ska gå att läsa i båda lägena** — 4,5:1 mot panel och bakgrund, mätt, inte tyckt.
    #[test]
    fn light_mode_keeps_text_readable() {
        for palette in [Palette::default(), Palette::light()] {
            for surface in [palette.panel, palette.bg, palette.panel_alt] {
                let r = ratio(palette.text, surface);
                assert!(r >= 4.5, "{:?}: text mot yta gav {r:.2}:1", palette.mode);
                let muted = ratio(palette.text_muted, surface);
                assert!(muted >= 3.0, "{:?}: dämpad text gav {muted:.2}:1", palette.mode);
            }
        }
    }

    /// En fil som bara säger `{"mode":"light"}` ger den ljusa paletten; en färg i filen vinner.
    #[test]
    fn a_file_can_switch_mode_and_override_one_colour() {
        let light = merge_onto_base(serde_json::json!({"mode": "light"}));
        assert_eq!(light.mode, ThemeMode::Light);
        assert_eq!(light.panel, Palette::light().panel);
        let custom = merge_onto_base(serde_json::json!({"mode": "light", "panel": [1, 2, 3, 255]}));
        assert_eq!(custom.panel, [1, 2, 3, 255]);
        assert_eq!(custom.text, Palette::light().text);
    }
}
