use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StemType {
    Vocals,
    Drums,
    Bass,
    Instruments,
}

impl StemType {
    pub fn name(&self) -> &'static str {
        match self {
            StemType::Vocals => "🎤 Vocals (Acapella)",
            StemType::Drums => "🥁 Drums & Beats",
            StemType::Bass => "🎸 Bass & Sub (808)",
            StemType::Instruments => "🎹 Instruments & Other",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            StemType::Vocals => Color32::from_rgb(255, 105, 180), // Pink
            StemType::Drums => Color32::from_rgb(255, 140, 0),   // Orange
            StemType::Bass => Color32::from_rgb(0, 220, 255),    // Cyan
            StemType::Instruments => Color32::from_rgb(46, 204, 113), // Green
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StemChannel {
    pub stem_type: StemType,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub pitch_shift: i8,
    pub time_stretch: f32,
    pub waveform_data: Vec<f32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StemProject {
    pub track_title: String,
    pub duration_seconds: f32,
    pub bpm: f32,
    pub model_name: &'static str,
    pub is_separating: bool,
    pub progress: f32,
    pub stems: Vec<StemChannel>,
    pub selected_sample_idx: usize,
}

impl Default for StemProject {
    fn default() -> Self {
        let mut proj = Self {
            track_title: "Retro_Synthwave_Summer_Hit.wav".to_string(),
            duration_seconds: 18.5,
            bpm: 126.0,
            model_name: "Demucs v4 Neural Hybrid Transformer (AI)",
            is_separating: false,
            progress: 1.0,
            stems: Vec::new(),
            selected_sample_idx: 0,
        };
        proj.load_demo_stems();
        proj
    }
}

impl StemProject {
    pub fn load_demo_stems(&mut self) {
        self.stems.clear();

        // Generate stylized waveforms for each stem
        let samples_count = 120;

        // 1. Vocals (Dynamic speech & sung melody bursts)
        let mut vox_wave = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let t = i as f32 / samples_count as f32;
            let phrase = if (0.15..0.45).contains(&t) || (0.55..0.90).contains(&t) {
                (t * 40.0).sin().abs() * 0.75 + (t * 85.0).sin().abs() * 0.25
            } else {
                0.04 * (t * 20.0).sin().abs()
            };
            vox_wave.push(phrase.clamp(0.02, 0.95));
        }
        self.stems.push(StemChannel {
            stem_type: StemType::Vocals,
            volume: 0.90,
            pan: 0.0,
            muted: false,
            solo: false,
            pitch_shift: 0,
            time_stretch: 1.0,
            waveform_data: vox_wave,
        });

        // 2. Drums (4-on-the-floor kick transients + sharp snare spikes)
        let mut drum_wave = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let beat_pos = i % 8;
            let peak = match beat_pos {
                0 => 0.95, // Kick transient
                4 => 0.85, // Snare spike
                2 | 6 => 0.45, // HiHat
                _ => 0.18,
            };
            drum_wave.push(peak);
        }
        self.stems.push(StemChannel {
            stem_type: StemType::Drums,
            volume: 0.95,
            pan: 0.0,
            muted: false,
            solo: false,
            pitch_shift: 0,
            time_stretch: 1.0,
            waveform_data: drum_wave,
        });

        // 3. Bass (Solid low frequency energy)
        let mut bass_wave = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let t = i as f32 / samples_count as f32;
            let w = (t * 16.0).sin().abs() * 0.65 + 0.25;
            bass_wave.push(w.clamp(0.1, 0.9));
        }
        self.stems.push(StemChannel {
            stem_type: StemType::Bass,
            volume: 0.88,
            pan: 0.0,
            muted: false,
            solo: false,
            pitch_shift: 0,
            time_stretch: 1.0,
            waveform_data: bass_wave,
        });

        // 4. Instruments (Synth chords, guitar riffs)
        let mut inst_wave = Vec::with_capacity(samples_count);
        for i in 0..samples_count {
            let t = i as f32 / samples_count as f32;
            let w = (t * 24.0).sin().abs() * 0.5 + (t * 6.0).cos().abs() * 0.35 + 0.1;
            inst_wave.push(w.clamp(0.08, 0.85));
        }
        self.stems.push(StemChannel {
            stem_type: StemType::Instruments,
            volume: 0.82,
            pan: 0.0,
            muted: false,
            solo: false,
            pitch_shift: 0,
            time_stretch: 1.0,
            waveform_data: inst_wave,
        });
    }

    pub fn trigger_ai_separation(&mut self, track_name: &str) {
        self.track_title = track_name.to_string();
        self.is_separating = true;
        self.progress = 0.0;
        self.load_demo_stems();
    }
}
