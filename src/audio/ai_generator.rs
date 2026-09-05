use eframe::egui::Color32;

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
}

impl Default for AiMusicAssistant {
    fn default() -> Self {
        let mut asst = Self {
            prompt_input: "80s Retrowave analog bassline med punchig attack och pulserande 16-delar".to_string(),
            selected_genre: 0,
            selected_key: 2, // A minor
            selected_target: 0,
            generated_history: Vec::new(),
            is_generating: false,
            last_status: "AI Music Assistant redo (Lokal transformer + Suno/LALAL prompt engine)".to_string(),
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
        let base_note: u8 = if p.contains("c") { 48 } else if p.contains("d") { 50 } else if p.contains("f") { 53 } else { 45 };

        let mut steps = [false; 16];
        let mut notes = [base_note; 16];

        for i in 0..16 {
            if i % 2 == 0 || (i % 4 == 3 && p.contains("syncop")) || (p.contains("16") && i % 4 != 1) {
                steps[i] = true;
                let offset = match i % 4 {
                    0 => 0,
                    1 => 3,
                    2 => 7,
                    _ => 10,
                };
                notes[i] = base_note + offset;
            }
        }

        self.generated_history.insert(0, AiGeneratedClip {
            title,
            genre: "Generativ AI".to_string(),
            key_signature: "Auto Scale Snap".to_string(),
            bpm: 126.0,
            channel_steps: steps,
            notes,
            color: Color32::from_rgb(46, 204, 113),
        });
        self.last_status = format!("✔ Genererade nytt musikmönster från prompt: \"{}\"", self.prompt_input);
    }
}
