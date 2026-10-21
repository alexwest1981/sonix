//! Stämningar — Scala `.ascl`-filer, och den **enda** vägen från MIDI-not till frekvens.
//!
//! Ableton skeppar 152 mikrotonala stämningar i det här formatet. Att läsa dem är litet;
//! det som gör saken är att alla not-till-frekvens-vägar måste gå genom samma funktion,
//! annars spelar uppspelningen en stämning och exporten en annan.

use std::sync::{OnceLock, RwLock};

/// En stämning: cent per skalsteg, och en period (oktaven, normalt 1200 cent).
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub name: String,
    /// Cent över grundtonen för varje skalsteg.
    pub cents: Vec<f32>,
    /// Perioden i cent — 1200 för en oktav, något annat för t.ex. Bohlen-Pierce.
    pub period_cents: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self::twelve_tet()
    }
}

impl Tuning {
    /// Vanlig liksvävande tolvton — appens beteende innan stämningarna fanns.
    pub fn twelve_tet() -> Self {
        Self {
            name: "12-TET".to_string(),
            cents: (0..12).map(|i| i as f32 * 100.0).collect(),
            period_cents: 1200.0,
        }
    }

    pub fn is_twelve_tet(&self) -> bool {
        self.period_cents == 1200.0
            && self.cents.len() == 12
            && self.cents.iter().enumerate().all(|(i, c)| (*c - i as f32 * 100.0).abs() < 0.01)
    }

    /// Cent över grundtonen för en MIDI-not.
    ///
    /// En MIDI-not är **ett skalsteg** — det är Scala-konventionen, och därför sträcker sig
    /// en 24-stegsstämning över två oktaver på klaviaturen i stället för att kvantiseras.
    /// Not 69 (A4) är alltid stämningens 1/1.
    pub fn cents_for_note(&self, note: f32) -> f32 {
        let n = self.cents.len();
        if n == 0 {
            return (note - 69.0) * 100.0;
        }
        let degree = note - 69.0;
        let nf = n as f32;
        let octaves = (degree / nf).floor();
        let within = degree - octaves * nf;
        let idx = (within.floor() as usize).min(n - 1);
        let frac = within - idx as f32;
        let next = (idx + 1) % n;
        let a = self.cents[idx];
        let b = if next == 0 { self.period_cents } else { self.cents[next] };
        octaves * self.period_cents + a + (b - a) * frac
    }

    /// Frekvensen för en not. A4 (69) är referensen, precis som i 12-TET.
    pub fn freq(&self, note: f32, a4_hz: f32) -> f32 {
        if self.is_twelve_tet() {
            return a4_hz * 2.0f32.powf((note - 69.0) / 12.0);
        }
        a4_hz * 2.0f32.powf(self.cents_for_note(note) / 1200.0)
    }
}

/// Läser en Scala `.ascl`-fil.
///
/// Formatet: `!`-rader är kommentarer, första raden som inte är det är beskrivningen,
/// nästa är antalet toner, sedan en cent-rad per ton, och sist perioden i cent.
pub fn parse_ascl(text: &str) -> Option<Tuning> {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('!'))
        .collect();
    // Beskrivningen kan vara **flera rader** (Abletons 24-EDO-fil har två). Antalet är den
    // första raden som är ett ensamt heltal.
    let count_idx = lines
        .iter()
        .position(|l| l.split_whitespace().count() == 1 && l.parse::<usize>().is_ok())?;
    let count: usize = lines[count_idx].parse().ok()?;
    if count < 2 || count > 128 {
        return None;
    }
    let name = lines[..count_idx].join(" ");
    let name = if name.is_empty() { "Stämning".to_string() } else { name };

    // Antalet räknar **med** perioden som sista rad, och första tonen (1/1) skrivs inte ut.
    let mut values = Vec::with_capacity(count);
    for line in lines.iter().skip(count_idx + 1).take(count) {
        // Raden kan vara "100.    ! C#|Db" eller "3/2 ! kvint" — kommentaren klipps bort.
        let token = line.split('!').next()?.split_whitespace().next()?;
        values.push(parse_pitch_value(token)?);
    }
    if values.len() != count {
        return None;
    }
    let period_cents = *values.last()?;
    if period_cents <= 0.0 {
        return None;
    }
    let mut cents = Vec::with_capacity(count);
    cents.push(0.0);
    cents.extend_from_slice(&values[..count - 1]);
    Some(Tuning { name, cents, period_cents })
}

/// En tonhöjd i Scala-format: cent (`100.0`) eller frekvensförhållande (`3/2`).
pub fn parse_pitch_value(token: &str) -> Option<f32> {
    if let Some((num, den)) = token.split_once('/') {
        let num: f64 = num.trim().parse().ok()?;
        let den: f64 = den.trim().parse().ok()?;
        if num <= 0.0 || den <= 0.0 {
            return None;
        }
        return Some((1200.0 * (num / den).log2()) as f32);
    }
    token.trim().parse().ok()
}

static CURRENT: OnceLock<RwLock<Tuning>> = OnceLock::new();

fn current_slot() -> &'static RwLock<Tuning> {
    CURRENT.get_or_init(|| RwLock::new(Tuning::twelve_tet()))
}

/// Sätter den aktiva stämningen. `None` återgår till 12-TET.
pub fn set_tuning(tuning: Option<Tuning>) {
    if let Ok(mut g) = current_slot().write() {
        *g = tuning.unwrap_or_else(Tuning::twelve_tet);
    }
}

/// Namnet på den aktiva stämningen, för statusraden.
pub fn current_name() -> String {
    current_slot()
        .read()
        .map(|g| g.name.clone())
        .unwrap_or_else(|_| "12-TET".to_string())
}

/// **Den enda vägen från MIDI-not till frekvens.**
///
/// De tre kopiorna (`state.rs`, `exporter.rs`, `ai_generator.rs`) räknade alla
/// `440 * 2^((n-69)/12)`. Nu går de genom den här, så uppspelning, export och
/// AI-generatorn alltid spelar samma stämning — annars exporterar man en låt i en
/// stämning man inte hörde.
pub fn midi_to_freq(note: u8) -> f32 {
    let a4 = 440.0;
    match current_slot().read() {
        Ok(g) => g.freq(note as f32, a4),
        Err(_) => a4 * 2.0f32.powf((note as f32 - 69.0) / 12.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samma form som Abletons egna filer: beskrivning, antal (inklusive perioden),
    /// cent-rader med kommentarer, perioden sist.
    const TWELVE_TET: &str = "! 12-tet.ascl\n12-tone equal temperament, just intonation-ish test\n12\n100.  ! C#\n200.\n300.\n400.\n500.\n600.\n700.\n800.\n900.\n1000.\n1100.\n1200.\n";

    #[test]
    fn twelve_tet_parses_and_matches_the_old_formula_exactly() {
        let t = parse_ascl(TWELVE_TET).expect("12-TET ska gå att läsa");
        assert_eq!(t.cents.len(), 12);
        assert!((t.freq(69.0, 440.0) - 440.0).abs() < 0.001, "A4 är referensen");
        for note in [0u8, 21, 45, 60, 69, 88, 108] {
            let old = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
            assert!(
                (t.freq(note as f32, 440.0) - old).abs() < 0.01,
                "not {note}: {} mot {old}",
                t.freq(note as f32, 440.0)
            );
        }
    }

    #[test]
    fn quarter_tones_give_twenty_four_steps_per_octave() {
        // 24-EDO: 24 steg per oktav, ett steg = 50 cent.
        let mut text = String::from("24-EDO\n24\n");
        for i in 1..24 {
            text.push_str(&format!("{}\n", i as f32 * 50.0));
        }
        text.push_str("1200.\n");
        let t = parse_ascl(&text).unwrap();
        assert_eq!(t.cents.len(), 24, "24 steg per oktav");
        assert!(!t.is_twelve_tet());
        // Ett skalsteg är 50 cent, och en oktav (24 steg) är 1200.
        let a4 = t.freq(69.0, 440.0);
        let step = 1200.0 * (t.freq(70.0, 440.0) / a4).log2();
        assert!((step - 50.0).abs() < 0.5, "steget blev {step} cent");
        let octave = 1200.0 * (t.freq(93.0, 440.0) / a4).log2();
        assert!((octave - 1200.0).abs() < 0.5, "oktaven blev {octave} cent");
    }

    #[test]
    fn a_file_that_does_not_start_at_the_root_is_rejected() {
        assert!(parse_ascl("trasig\n2\n1200.0\n-5.0").is_none(), "negativ period");
        assert!(parse_ascl("bara en rad").is_none());
        assert!(parse_ascl("").is_none());
    }

    #[test]
    fn the_global_switch_moves_the_shared_conversion() {
        // 12-TET först, så provet inte stör andra.
        set_tuning(None);
        let baseline = midi_to_freq(60);
        assert!((baseline - 261.6256).abs() < 0.01, "C4: {baseline}");
        let mut text = String::from("24-EDO\n24\n");
        for i in 1..24 {
            text.push_str(&format!("{}\n", i as f32 * 50.0));
        }
        text.push_str("1200.\n");
        set_tuning(parse_ascl(&text));
        let quarter = midi_to_freq(60);
        let cents = 1200.0 * (quarter / baseline).log2();
        assert!(cents.abs() > 1.0, "stämningen flyttade inte noten: {cents} cent");
        set_tuning(None);
        assert!((midi_to_freq(60) - baseline).abs() < 0.001, "tillbaka till 12-TET");
    }

    /// Mot en **riktig** Ableton-stämning, när en sådan pekas ut (`SONIX_ASCL`).
    #[test]
    fn a_real_ascl_from_ableton_parses_if_one_is_available() {
        let Ok(path) = std::env::var("SONIX_ASCL") else {
            return;
        };
        let text = std::fs::read(&path).expect("filen ska gå att läsa");
        let t = parse_ascl(&String::from_utf8_lossy(&text)).expect("Abletons .ascl ska gå att läsa");
        assert!(!t.cents.is_empty());
        assert!(t.period_cents > 0.0);
        println!("{path}: {} steg, period {} cent", t.cents.len(), t.period_cents);
    }
}
