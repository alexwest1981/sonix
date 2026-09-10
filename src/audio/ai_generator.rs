use eframe::egui::Color32;
use std::sync::{Arc, Mutex};

use super::ai_client::AiConfig;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AiGeneratedClip {
    pub title: String,
    pub genre: String,
    pub key_signature: String,
    pub bpm: f32,
    pub channel_steps: [bool; 16],
    pub notes: [u8; 16],
    pub color: Color32,
}

pub type AiRequestResult = Result<Vec<AiGeneratedClip>, String>;

/// Live project state the app feeds into the assistant so prompts can be
/// grounded in what the user is actually working on (Etapp D).
#[derive(Debug, Clone, Default)]
pub struct GenContext {
    pub project_name: String,
    pub bpm: f32,
    pub key_label: String,
    pub key_pc: Option<u8>,
    pub is_minor: bool,
    pub selected_track: String,
    pub selected_region: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AiMusicAssistant {
    pub prompt_input: String,
    pub selected_genre: usize,
    pub selected_key: usize,
    pub selected_target: usize, // 0: Bassline, 1: Synth Lead, 2: Drum Groove, 3: Full Chord Progression
    pub generated_history: Vec<AiGeneratedClip>,
    pub is_generating: bool,
    pub last_status: String,
    pub config: AiConfig,
    pub last_error: Option<String>,
    pub use_context: bool,
    pub context: GenContext,
    pending: Option<Arc<Mutex<Option<AiRequestResult>>>>,
}

impl Default for AiMusicAssistant {
    fn default() -> Self {
        let mut asst = Self {
            prompt_input: crate::i18n::t("80s Retrowave analog bassline med punchig attack och pulserande 16-delar").to_string(),
            selected_genre: 0,
            selected_key: 2, // A minor
            selected_target: 0,
            generated_history: Vec::new(),
            is_generating: false,
            last_status: crate::i18n::t("AI Music Assistant redo (regelbaserad generativ motor – musikteori, ingen extern modell)").to_string(),
            config: AiConfig::load(),
            last_error: None,
            use_context: true,
            context: GenContext::default(),
            pending: None,
        };
        asst.generate_preset_clips();
        asst
    }
}

impl AiMusicAssistant {
    pub fn generate_preset_clips(&mut self) {
        self.generated_history = vec![
            AiGeneratedClip {
                title: "⚡ Cyberpunk 303 Acid Pulse".to_string(),
                genre: "Synthwave / Cyberpunk".to_string(),
                key_signature: "A Minor".to_string(),
                bpm: 128.0,
                channel_steps: [true, false, true, true, false, true, true, false, true, false, true, true, false, true, true, true],
                notes: [45, 45, 48, 52, 45, 45, 57, 45, 45, 45, 48, 52, 45, 45, 55, 57],
                color: Color32::from_rgb(0, 220, 255),
            },
            AiGeneratedClip {
                title: "🎷 Neo-Soul Warm Chord Melodies".to_string(),
                genre: "Neo-Soul / Lo-Fi".to_string(),
                key_signature: "D Minor 9".to_string(),
                bpm: 90.0,
                channel_steps: [true, false, false, true, false, false, true, false, true, false, false, true, false, false, true, false],
                notes: [50, 50, 50, 53, 53, 53, 57, 57, 60, 60, 60, 64, 64, 64, 60, 57],
                color: Color32::from_rgb(255, 140, 0),
            },
            AiGeneratedClip {
                title: "💥 Drill & Trap 808 Sliding Bass".to_string(),
                genre: "Trap / Hip Hop".to_string(),
                key_signature: "F Minor".to_string(),
                bpm: 140.0,
                channel_steps: [true, false, false, false, true, false, true, false, false, true, false, false, true, false, false, true],
                notes: [41, 41, 41, 41, 41, 41, 48, 48, 41, 41, 41, 41, 46, 46, 48, 41],
                color: Color32::from_rgb(180, 100, 255),
            },
        ];
    }

    pub fn generate_from_prompt(&mut self) {
        let p = self.prompt_input.to_lowercase();
        let title = format!("AI Clip: {}", self.prompt_input.chars().take(28).collect::<String>());

        let (genre, scale, mut bpm) = Self::detect_genre(&p);
        let fallback_key = if self.use_context {
            self.context
                .key_pc
                .map(|k| k as usize)
                .unwrap_or(self.selected_key)
        } else {
            self.selected_key
        };
        let (root_pc, mut is_minor) = Self::parse_key(&p, fallback_key);
        // Project context wins when the prompt does not state a key/tempo.
        if self.use_context {
            if !p.contains("dur") && !p.contains("major") && !p.contains("moll") && !p.contains("minor") {
                is_minor = self.context.is_minor;
            }
            if self.context.bpm > 0.0 {
                bpm = self.context.bpm;
            }
        }
        let target = Self::detect_target(&p, self.selected_target);
        let base_note: u8 = 12 * 3 + root_pc; // root in octave 3 (36..47)

        let (steps, notes) = Self::compose_pattern(scale, base_note, target, &p);

        let key_signature = format!(
            "{}{}",
            Self::note_name(root_pc),
            if is_minor { " Minor" } else { " Major" }
        );

        self.last_status = crate::tstatus!(
            "✔ Genererade ett {} -mönster ({}) i {} från prompten.",
            target_name(target),
            genre,
            key_signature
        );
        self.generated_history.insert(0, AiGeneratedClip {
            title,
            genre,
            key_signature,
            bpm,
            channel_steps: steps,
            notes,
            color: Color32::from_rgb(46, 204, 113),
        });
        if self.generated_history.len() > 12 {
            self.generated_history.truncate(12);
        }
    }

    pub fn selected_genre_label(&self) -> &'static str {
        match self.selected_genre {
            1 => "Trap / Hip Hop",
            2 => "Neo-Soul / Lo-Fi",
            3 => "House / EDM",
            4 => "Acid / Techno",
            5 => "Funk / Disco",
            _ => "Synthwave / Cyberpunk",
        }
    }

    pub fn selected_key_label(&self) -> &'static str {
        const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        NAMES[self.selected_key % 12]
    }

    pub fn selected_target_label(&self) -> &'static str {
        match self.selected_target {
            1 => "lead",
            2 => "trum",
            3 => "ackord",
            _ => "bas",
        }
    }

    fn build_remote_prompt(&self) -> String {
        let mut prompt = format!(
            "Free-text idea: {}\nTarget instrument: {}\nGenre hint: {}\nKey hint: {}\n",
            self.prompt_input,
            self.selected_target_label(),
            self.selected_genre_label(),
            self.selected_key_label(),
        );
        if self.use_context {
            let ctx = &self.context;
            let mut lines = Vec::new();
            if !ctx.project_name.trim().is_empty() {
                lines.push(format!("- Project: {}", ctx.project_name));
            }
            if ctx.bpm > 0.0 {
                lines.push(format!("- Project tempo: {:.0} BPM", ctx.bpm));
            }
            if !ctx.key_label.trim().is_empty() {
                lines.push(format!("- Project key: {}", ctx.key_label));
            }
            if !ctx.selected_track.trim().is_empty() {
                lines.push(format!("- Selected track: {}", ctx.selected_track));
            }
            if let Some(region) = &ctx.selected_region {
                lines.push(format!("- Selected region: {region}"));
            }
            if !lines.is_empty() {
                prompt.push_str("\nAlign the result to this project context:\n");
                prompt.push_str(&lines.join("\n"));
                prompt.push('\n');
            }
        }
        prompt.push_str("Return 1 to 3 clip variations as JSON.");
        prompt
    }

    /// Reloads config from disk/env and generates. Uses the remote API when it
    /// is configured, otherwise falls back to the local rule-based engine.
    pub fn generate(&mut self) {
        self.config = AiConfig::load();
        if self.config.is_ready() {
            self.request_remote_generation();
        } else {
            self.last_error = None;
            self.generate_from_prompt();
        }
    }

    pub fn request_remote_generation(&mut self) {
        self.last_error = None;
        let config = self.config.clone();
        if !config.is_ready() {
            self.generate_from_prompt();
            return;
        }
        let prompt = self.build_remote_prompt();
        let slot: Arc<Mutex<Option<AiRequestResult>>> = Arc::new(Mutex::new(None));
        self.pending = Some(slot.clone());
        self.is_generating = true;
        self.last_status = crate::i18n::t("⏳ Genererar via AI-API...").to_string();
        std::thread::spawn(move || {
            let result = crate::audio::ai_client::generate_clips(&config, &prompt);
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(result);
            }
        });
    }

    /// Polls the background AI request. Returns a status message the moment it
    /// finishes (success or failure), otherwise `None`.
    pub fn poll_remote_generation(&mut self) -> Option<String> {
        let result = {
            let slot = self.pending.as_ref()?;
            let mut guard = slot.lock().ok()?;
            guard.take()?
        };
        self.pending = None;
        self.is_generating = false;
        match result {
            Ok(clips) => {
                let count = clips.len();
                for clip in clips.into_iter().rev() {
                    self.generated_history.insert(0, clip);
                }
                if self.generated_history.len() > 12 {
                    self.generated_history.truncate(12);
                }
                let msg = crate::tstatus!("✔ AI-API genererade {} klipp från prompten.", count);
                self.last_status = msg.clone();
                Some(msg)
            }
            Err(e) => {
                self.last_error = Some(e.clone());
                self.generate_from_prompt();
                let msg = format!("⚠ AI-API misslyckades ({e}) – använde lokal motor.");
                self.last_status = msg.clone();
                Some(msg)
            }
        }
    }

    fn note_name(pc: u8) -> &'static str {
        const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        NAMES[(pc % 12) as usize]
    }

    fn detect_genre(p: &str) -> (String, &'static [u8], f32) {
        let minor: &[u8] = &[0, 2, 3, 5, 7, 8, 10];
        let dorian: &[u8] = &[0, 2, 3, 5, 7, 9, 10];
        let mixolydian: &[u8] = &[0, 2, 4, 5, 7, 9, 10];
        if p.contains("trap") || p.contains("drill") || p.contains("808") || p.contains("hip hop") || p.contains("hip-hop") {
            ("Trap / Hip Hop".to_string(), minor, 140.0)
        } else if p.contains("neo-soul") || p.contains("neo soul") || p.contains("jazz") || p.contains("lofi") || p.contains("lo-fi") {
            ("Neo-Soul / Lo-Fi".to_string(), dorian, 88.0)
        } else if p.contains("house") || p.contains("techno") || p.contains("edm") {
            ("House / EDM".to_string(), dorian, 124.0)
        } else if p.contains("acid") || p.contains("303") {
            ("Acid / Techno".to_string(), minor, 130.0)
        } else if p.contains("disco") || p.contains("funk") || p.contains("groove") {
            ("Funk / Disco".to_string(), mixolydian, 110.0)
        } else {
            ("Synthwave / Cyberpunk".to_string(), minor, 118.0)
        }
    }

    /// Finds a key like "a-moll", "c minor", "f# dur" from the prompt tokens.
    fn parse_key(p: &str, fallback: usize) -> (u8, bool) {
        let is_minor = !(p.contains("dur") || p.contains("major"));
        let names: [(&str, u8); 17] = [
            ("c#", 1), ("db", 1), ("d#", 3), ("eb", 3), ("f#", 6), ("gb", 6),
            ("g#", 8), ("ab", 8), ("a#", 10), ("bb", 10),
            ("c", 0), ("d", 2), ("e", 4), ("f", 5), ("g", 7), ("a", 9), ("b", 11),
        ];
        let tokens: Vec<String> = p
            .split(|c: char| !(c.is_alphanumeric() || c == '#'))
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();
        for tok in &tokens {
            for (name, pc) in names {
                if tok == name {
                    return (pc, is_minor);
                }
            }
        }
        ((fallback % 12) as u8, is_minor)
    }

    /// 0: Bass, 1: Lead, 2: Drums, 3: Chords. Prompt keywords win over the UI value.
    fn detect_target(p: &str, fallback: usize) -> usize {
        if p.contains("bass") || p.contains("bas") || p.contains("808") {
            0
        } else if p.contains("lead") || p.contains("melodi") || p.contains("melody") || p.contains("solo") {
            1
        } else if p.contains("drum") || p.contains("trum") || p.contains("beat") || p.contains("kick") {
            2
        } else if p.contains("chord") || p.contains("ackord") || p.contains("pad") || p.contains("harmon") {
            3
        } else {
            fallback.min(3)
        }
    }

    fn compose_pattern(scale: &[u8], base: u8, target: usize, p: &str) -> ([bool; 16], [u8; 16]) {
        let mut steps = [false; 16];
        let mut notes = [base; 16];
        let syncop = p.contains("syncop") || p.contains("synkop");
        let sixteenth = p.contains("16") || p.contains("puls") || p.contains("pulse");

        let deg = |d: usize| -> u8 {
            let oct = (d / scale.len()) as u8;
            base + scale[d % scale.len()] + 12 * oct
        };

        match target {
            // Drums: classic kick/snare/hat grid (pitched down so it is audible on synth too).
            2 => {
                for i in [0usize, 4, 8, 12] {
                    steps[i] = true;
                    notes[i] = base;
                }
                for i in [4usize, 12] {
                    notes[i] = base + 12;
                }
                let hat_step = if sixteenth { 1 } else { 2 };
                for i in (2..16).step_by(hat_step) {
                    steps[i] = true;
                    notes[i] = base + 24;
                }
                if syncop {
                    for i in [3usize, 7, 11, 15] {
                        steps[i] = true;
                        notes[i] = base + 12;
                    }
                }
            }
            // Chords: root/third/fifth/seventh block hits on beats and off-beats.
            3 => {
                let hits: &[usize] = if syncop { &[0, 3, 6, 8, 11, 14] } else { &[0, 4, 8, 12] };
                for (h_i, &i) in hits.iter().enumerate() {
                    steps[i] = true;
                    let tone = match h_i % 4 {
                        0 => 0,
                        1 => 2,
                        2 => 4,
                        _ => 6,
                    };
                    notes[i] = deg(tone);
                }
            }
            // Lead: 16th-note melodic run through the scale.
            1 => {
                let step_every = if sixteenth { 1 } else { 2 };
                let mut d = 0usize;
                for i in (0..16).step_by(step_every) {
                    steps[i] = true;
                    notes[i] = deg(d);
                    d += if i % 4 == 3 { 2 } else { 1 };
                }
            }
            // Bass (default): root/fifth/octave groove with syncopation.
            _ => {
                let pattern: &[usize] = if syncop {
                    &[0, 3, 6, 8, 11, 14]
                } else if sixteenth {
                    &[0, 2, 4, 6, 8, 10, 12, 14]
                } else {
                    &[0, 4, 6, 8, 12, 14]
                };
                for (i_i, &i) in pattern.iter().enumerate() {
                    steps[i] = true;
                    let tone = match i_i % 4 {
                        0 => 0,
                        1 => 4,
                        2 => 0,
                        _ => 6,
                    };
                    notes[i] = deg(tone);
                }
            }
        }
        (steps, notes)
    }
}

fn target_name(target: usize) -> &'static str {
    match target {
        0 => "bas",
        1 => "lead",
        2 => "trum",
        _ => "ackord",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_from_prompt_uses_prompt_key_and_target() {
        let mut a = AiMusicAssistant::default();
        a.prompt_input = "80s synthwave bassline i a-moll".to_string();
        a.generate_from_prompt();
        let clip = &a.generated_history[0];
        assert!(clip.key_signature.starts_with('A'), "key was {}", clip.key_signature);
        assert_eq!(clip.genre, "Synthwave / Cyberpunk");
        // Root A3 = 45.
        assert_eq!(clip.notes[0], 45);
        assert!(clip.channel_steps.iter().any(|&s| s));
    }

    #[test]
    fn trap_prompt_produces_low_808_notes() {
        let mut a = AiMusicAssistant::default();
        a.prompt_input = "Mörk Cyberpunk 808 basgång".to_string();
        a.generate_from_prompt();
        let clip = &a.generated_history[0];
        assert!(clip.genre.contains("Trap"));
        assert!(clip.notes.iter().all(|&n| n >= 36 && n <= 72));
    }

    #[test]
    fn different_targets_produce_different_patterns() {
        let mut a = AiMusicAssistant::default();
        a.prompt_input = "melodi lead i c-dur".to_string();
        a.generate_from_prompt();
        let lead = a.generated_history[0].channel_steps;
        a.prompt_input = "ackord pad progression i c-dur".to_string();
        a.generate_from_prompt();
        let chords = a.generated_history[0].channel_steps;
        assert_ne!(lead, chords);
    }

    #[test]
    fn project_context_is_included_in_remote_prompt() {
        let mut a = AiMusicAssistant::default();
        a.use_context = true;
        a.context.project_name = "Neon Nights".to_string();
        a.context.bpm = 128.0;
        a.context.key_label = "F Moll".to_string();
        a.context.selected_track = "Bass".to_string();
        let prompt = a.build_remote_prompt();
        assert!(prompt.contains("Neon Nights"));
        assert!(prompt.contains("128 BPM"));
        assert!(prompt.contains("F Moll"));
        assert!(prompt.contains("Bass"));
    }

    #[test]
    fn context_key_and_tempo_drive_local_generation() {
        let mut a = AiMusicAssistant::default();
        a.use_context = true;
        a.context.key_pc = Some(5); // F
        a.context.is_minor = true;
        a.context.bpm = 150.0;
        a.prompt_input = "bassline utan tonart".to_string();
        a.generate_from_prompt();
        let clip = &a.generated_history[0];
        assert!(clip.key_signature.starts_with('F'), "key was {}", clip.key_signature);
        assert_eq!(clip.bpm, 150.0);
    }
}

/// Style of procedurally generated audio. Used by the Suno/AI track generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenStyle {
    Drums,
    Bass,
    Synth,
    Vocal,
}

struct Rng(u64);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

const MINOR: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const TAU: f32 = std::f32::consts::TAU;

fn midi_to_freq(note: i32) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Renders real stereo PCM for the given style. This is a deterministic
/// synthesis engine (not an external AI model), so the result is always
/// audible and reproducible from the seed.
pub fn render_generated_audio(
    style: GenStyle,
    bpm: f32,
    bars: usize,
    sample_rate: u32,
    seed: u64,
) -> (Vec<f32>, Vec<f32>) {
    let sr = sample_rate.max(8000) as f32;
    let bpm = bpm.max(40.0);
    let sec_per_beat = 60.0 / bpm;
    let total_secs = bars.max(1) as f32 * 4.0 * sec_per_beat;
    let n = (total_secs * sr) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut rng = Rng(seed | 1);

    match style {
        GenStyle::Drums => render_drums(&mut l, &mut r, sr, sec_per_beat, bars, &mut rng),
        GenStyle::Bass => render_bass(&mut l, &mut r, sr, sec_per_beat, bars, &mut rng),
        GenStyle::Synth => render_synth(&mut l, &mut r, sr, sec_per_beat, bars, &mut rng),
        GenStyle::Vocal => render_vocal(&mut l, &mut r, sr, sec_per_beat, bars, &mut rng),
    }

    // Normalize to a healthy peak without clipping.
    let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, &v| m.max(v.abs()));
    if peak > 0.0001 {
        let g = (0.9 / peak).min(1.0);
        for v in l.iter_mut() {
            *v *= g;
        }
        for v in r.iter_mut() {
            *v *= g;
        }
    }
    (l, r)
}

fn add_kick(l: &mut [f32], r: &mut [f32], start: usize, sr: f32, gain: f32) {
    let len = (0.35 * sr) as usize;
    let mut phase = 0.0f32;
    for i in 0..len {
        let idx = start + i;
        if idx >= l.len() {
            break;
        }
        let t = i as f32 / sr;
        let f = 45.0 + 115.0 * (-t * 25.0).exp();
        phase += f / sr;
        let env = (-t * 9.0).exp();
        let click = if t < 0.004 { (1.0 - t / 0.004) * 0.6 } else { 0.0 };
        let s = ((phase * TAU).sin() + click) * env * gain;
        l[idx] += s;
        r[idx] += s;
    }
}

fn add_snare(l: &mut [f32], r: &mut [f32], start: usize, sr: f32, gain: f32, rng: &mut Rng) {
    let len = (0.25 * sr) as usize;
    let mut phase = 0.0f32;
    for i in 0..len {
        let idx = start + i;
        if idx >= l.len() {
            break;
        }
        let t = i as f32 / sr;
        phase += 185.0 / sr;
        let tone = (phase * TAU).sin();
        let noise = rng.next_f32();
        let env = (-t * 16.0).exp();
        let s = (tone * 0.45 + noise * 0.75) * env * gain;
        l[idx] += s * 0.95;
        r[idx] += s;
    }
}

fn add_hat(l: &mut [f32], r: &mut [f32], start: usize, sr: f32, gain: f32, open: bool, rng: &mut Rng) {
    let dur = if open { 0.22 } else { 0.06 };
    let len = (dur * sr) as usize;
    let mut lp = 0.0f32;
    for i in 0..len {
        let idx = start + i;
        if idx >= l.len() {
            break;
        }
        let t = i as f32 / sr;
        let n = rng.next_f32();
        let hp = n - lp;
        lp += 0.4 * (n - lp);
        let env = (-t * if open { 18.0 } else { 70.0 }).exp();
        let s = hp * env * gain;
        l[idx] += s * 0.85;
        r[idx] += s;
    }
}

fn render_drums(l: &mut [f32], r: &mut [f32], sr: f32, spb: f32, bars: usize, rng: &mut Rng) {
    let step = spb / 4.0;
    for s in 0..bars * 16 {
        let t = s as f32 * step;
        let start = (t * sr) as usize;
        let in_bar = s % 16;
        if in_bar == 0 || in_bar == 6 || in_bar == 10 {
            add_kick(l, r, start, sr, 0.95);
        }
        if in_bar == 4 || in_bar == 12 {
            add_snare(l, r, start, sr, 0.7, rng);
        }
        add_hat(l, r, start, sr, if in_bar % 4 == 2 { 0.35 } else { 0.22 }, in_bar == 14, rng);
    }
}

fn render_bass(l: &mut [f32], r: &mut [f32], sr: f32, spb: f32, bars: usize, rng: &mut Rng) {
    let root = 33 + (rng.next_u32() % 5) as i32; // A1..E2
    let progression = [0usize, 5, 3, 6];
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let note_len = (spb * 0.9 * sr) as usize;
    for bar in 0..bars {
        let deg_root = progression[bar % progression.len()];
        for eighth in 0..8 {
            let note = root + MINOR[(deg_root + if eighth % 4 == 2 { 4 } else { 0 }) % 7];
            let freq = midi_to_freq(note);
            let start = ((bar as f32 * 4.0 + eighth as f32 * 0.5) * spb * sr) as usize;
            for i in 0..note_len {
                let idx = start + i;
                if idx >= l.len() {
                    break;
                }
                let t = i as f32 / sr;
                phase += freq / sr;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
                let saw = 2.0 * phase - 1.0;
                let env = (1.0 - (t / (note_len as f32 / sr)).min(1.0)).powf(0.6) * (-t * 4.0).exp().min(1.0);
                let raw = saw * env;
                lp += 0.18 * (raw - lp);
                let s = lp * 0.8;
                l[idx] += s;
                r[idx] += s;
            }
        }
    }
}

fn render_synth(l: &mut [f32], r: &mut [f32], sr: f32, spb: f32, bars: usize, rng: &mut Rng) {
    let root = 45 + (rng.next_u32() % 5) as i32;
    let progression = [0usize, 5, 3, 6];
    let chord_intervals = [0usize, 2, 4, 6]; // root, third, fifth, seventh
    let bar_len = (4.0 * spb * sr) as usize;
    let mut phases = [0.0f32; 8];
    for bar in 0..bars {
        let deg_root = progression[bar % progression.len()];
        let start = (bar as f32 * 4.0 * spb * sr) as usize;
        let notes: Vec<i32> = chord_intervals
            .iter()
            .map(|&ci| root + MINOR[(deg_root + ci) % 7] + 12 * ((deg_root + ci) / 7) as i32)
            .collect();
        for i in 0..bar_len {
            let idx = start + i;
            if idx >= l.len() {
                break;
            }
            let t = i as f32 / sr;
            let bar_secs = 4.0 * spb;
            let env = (t / (bar_secs * 0.15)).min(1.0) * ((bar_secs - t) / (bar_secs * 0.3)).clamp(0.0, 1.0);
            let mut sl = 0.0;
            let mut sr_ = 0.0;
            for (ni, &note) in notes.iter().enumerate() {
                let f = midi_to_freq(note);
                phases[ni * 2] += f / sr;
                if phases[ni * 2] >= 1.0 {
                    phases[ni * 2] -= 1.0;
                }
                phases[ni * 2 + 1] += (f * 1.006) / sr;
                if phases[ni * 2 + 1] >= 1.0 {
                    phases[ni * 2 + 1] -= 1.0;
                }
                let a = 2.0 * phases[ni * 2] - 1.0;
                let b = 2.0 * phases[ni * 2 + 1] - 1.0;
                sl += a;
                sr_ += b;
            }
            let pan = 0.5 + 0.15 * ((bar as f32 * 0.7).sin());
            l[idx] += sl * env * 0.12 * (1.0 - pan);
            r[idx] += sr_ * env * 0.12 * pan;
        }
    }
}

fn render_vocal(l: &mut [f32], r: &mut [f32], sr: f32, spb: f32, bars: usize, rng: &mut Rng) {
    let root = 57 + (rng.next_u32() % 5) as i32;
    let mut phase = 0.0f32;
    let note_len = (spb * 0.85 * sr) as usize;
    let mut degree = 0usize;
    for bar in 0..bars {
        for eighth in 0..8 {
            let degree_step = if eighth % 4 == 0 { 2 } else { 1 };
            degree = (degree + degree_step) % 7;
            let note = root + MINOR[degree];
            let freq = midi_to_freq(note);
            let start = ((bar as f32 * 4.0 + eighth as f32 * 0.5) * spb * sr) as usize;
            for i in 0..note_len {
                let idx = start + i;
                if idx >= l.len() {
                    break;
                }
                let t = i as f32 / sr;
                let vib = 1.0 + 0.006 * (TAU * 5.0 * t).sin();
                phase += (freq * vib) / sr;
                if phase >= 1.0 {
                    phase -= 1.0;
                }
                let base = (phase * TAU).sin();
                let harm = (phase * TAU * 2.0).sin() * 0.3 + (phase * TAU * 3.0).sin() * 0.15;
                let breath = rng.next_f32() * 0.05;
                let env = (t / 0.03).min(1.0) * ((note_len as f32 / sr - t) / 0.08).clamp(0.0, 1.0);
                let s = (base + harm + breath) * env * 0.5;
                l[idx] += s;
                r[idx] += s;
            }
        }
    }
}

#[cfg(test)]
mod audio_tests {
    use super::*;

    #[test]
    fn generated_audio_is_audible_and_stereo() {
        for style in [GenStyle::Drums, GenStyle::Bass, GenStyle::Synth, GenStyle::Vocal] {
            let (l, r) = render_generated_audio(style, 120.0, 2, 44100, 42);
            assert!(!l.is_empty());
            assert_eq!(l.len(), r.len());
            let peak = l.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
            assert!(peak > 0.1, "style {:?} produced near-silence (peak {})", style, peak);
            assert!(l.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn generated_audio_scales_with_length() {
        let (l1, _) = render_generated_audio(GenStyle::Drums, 120.0, 1, 44100, 7);
        let (l2, _) = render_generated_audio(GenStyle::Drums, 120.0, 2, 44100, 7);
        assert!(l2.len() > l1.len());
    }
}
