use eframe::egui::Color32;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AudioTake {
    pub take_number: usize,
    pub name: String,
    pub waveform_data: Vec<f32>,
    pub is_selected: bool,
    pub color: Color32,
    pub start_bar: usize,
    pub length_bars: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CustomSoundClip {
    pub id: usize,
    pub name: String,
    pub category: String,
    pub waveform_data: Vec<f32>,
    pub duration_secs: f32,
    pub color: Color32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CompRegion {
    pub start_norm: f32, // 0.0 .. 1.0
    pub end_norm: f32,
    pub from_take: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    LeadVocals,
    CustomSounds,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VocalStudioTrack {
    pub name: String,
    pub input_channel: &'static str,
    pub is_armed: bool,
    pub is_recording: bool,
    pub recording_mode: RecordingMode,
    pub recording_elapsed_secs: f32,
    pub monitoring_on: bool,
    pub input_gain: f32,
    pub mic_vu_level: f32,
    pub custom_sample_name_input: String,
    pub takes: Vec<AudioTake>,
    pub custom_sounds: Vec<CustomSoundClip>,
    pub comp_regions: Vec<CompRegion>,
    pub active_comp_take: usize,
}

impl Default for VocalStudioTrack {
    fn default() -> Self {
        let mut track = Self {
            name: "🎤 Lead Sång (Mikrofon 1)".to_string(),
            input_channel: "PipeWire RT Mic In",
            is_armed: true,
            is_recording: false,
            recording_mode: RecordingMode::LeadVocals,
            recording_elapsed_secs: 0.0,
            monitoring_on: true,
            input_gain: 1.0,
            mic_vu_level: 0.65,
            custom_sample_name_input: "Mitt Akustiska Ljud 1".to_string(),
            takes: Vec::new(),
            custom_sounds: Vec::new(),
            comp_regions: Vec::new(),
            active_comp_take: 0,
        };
        track.populate_demo_takes();
        track.populate_demo_custom_sounds();
        track
    }
}

impl VocalStudioTrack {
    pub fn populate_demo_takes(&mut self) {
        self.takes.clear();
        let samples = 100;

        // Take 1: First full run-through
        let mut take1 = Vec::with_capacity(samples);
        for i in 0..samples {
            let t = i as f32 / samples as f32;
            let val = if (0.1..0.4).contains(&t) || (0.6..0.85).contains(&t) {
                (t * 30.0).sin().abs() * 0.7 + (t * 60.0).sin().abs() * 0.2
            } else {
                0.03
            };
            take1.push(val.clamp(0.02, 0.95));
        }
        self.takes.push(AudioTake {
            take_number: 1,
            name: "Tagning 1 (Intro & Vers)".to_string(),
            waveform_data: take1,
            is_selected: false,
            color: Color32::from_rgb(0, 200, 240),
            start_bar: 0,
            length_bars: 8,
        });

        // Take 2: Strong chorus
        let mut take2 = Vec::with_capacity(samples);
        for i in 0..samples {
            let t = i as f32 / samples as f32;
            let val = if (0.2..0.5).contains(&t) || (0.55..0.95).contains(&t) {
                (t * 35.0).sin().abs() * 0.85 + (t * 70.0).sin().abs() * 0.15
            } else {
                0.02
            };
            take2.push(val.clamp(0.02, 0.98));
        }
        self.takes.push(AudioTake {
            take_number: 2,
            name: "Tagning 2 (Stark Refräng)".to_string(),
            waveform_data: take2,
            is_selected: true,
            color: Color32::from_rgb(255, 140, 0),
            start_bar: 8,
            length_bars: 8,
        });

        // Take 3: Energetic ad-libs & high notes
        let mut take3 = Vec::with_capacity(samples);
        for i in 0..samples {
            let t = i as f32 / samples as f32;
            let val = if (0.65..0.98).contains(&t) {
                (t * 45.0).sin().abs() * 0.95
            } else {
                0.02
            };
            take3.push(val.clamp(0.02, 0.95));
        }
        self.takes.push(AudioTake {
            take_number: 3,
            name: "Tagning 3 (Ad-libs & Kör)".to_string(),
            waveform_data: take3,
            is_selected: false,
            color: Color32::from_rgb(180, 100, 255),
            start_bar: 16,
            length_bars: 8,
        });

        self.comp_regions = vec![
            CompRegion { start_norm: 0.0, end_norm: 0.45, from_take: 0 },
            CompRegion { start_norm: 0.45, end_norm: 0.75, from_take: 1 },
            CompRegion { start_norm: 0.75, end_norm: 1.0, from_take: 2 },
        ];
    }

    pub fn populate_demo_custom_sounds(&mut self) {
        self.custom_sounds.clear();

        // Sample 1: Hand Clap & Reverb
        let mut s1_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = (1.0 - t).powi(2) * (t * 50.0).sin().abs();
            s1_wave.push(val.clamp(0.05, 0.9));
        }
        self.custom_sounds.push(CustomSoundClip {
            id: 1,
            name: "👏 Akustisk Handklapp".to_string(),
            category: "Perkussion".to_string(),
            waveform_data: s1_wave,
            duration_secs: 1.2,
            color: Color32::from_rgb(255, 120, 80),
        });

        // Sample 2: Röstsample "Yeah!"
        let mut s2_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = ((t * std::f32::consts::PI).sin()) * 0.85;
            s2_wave.push(val.clamp(0.05, 0.95));
        }
        self.custom_sounds.push(CustomSoundClip {
            id: 2,
            name: "🗣 Röstchop 'Yeah!'".to_string(),
            category: "Vokal FX".to_string(),
            waveform_data: s2_wave,
            duration_secs: 0.8,
            color: Color32::from_rgb(100, 220, 255),
        });

        // Sample 3: Akustisk Gitarrknäpp
        let mut s3_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = (1.0 - t * 0.8) * (t * 80.0).sin().abs() * 0.75;
            s3_wave.push(val.clamp(0.05, 0.9));
        }
        self.custom_sounds.push(CustomSoundClip {
            id: 3,
            name: "🎸 Gitarrackord E-Moll".to_string(),
            category: "Instrument".to_string(),
            waveform_data: s3_wave,
            duration_secs: 2.4,
            color: Color32::from_rgb(120, 240, 140),
        });
    }

    pub fn add_new_take(&mut self, name: &str, start_bar: usize, length_bars: usize) {
        let count = self.takes.len() + 1;
        let mut new_wave = Vec::with_capacity(100);
        for i in 0..100 {
            let t = i as f32 / 100.0;
            let val = (t * 24.0).sin().abs() * 0.8 + (t * 48.0).cos().abs() * 0.15;
            new_wave.push(val.clamp(0.03, 0.95));
        }
        let color = match count % 4 {
            0 => Color32::from_rgb(0, 200, 240),
            1 => Color32::from_rgb(255, 140, 0),
            2 => Color32::from_rgb(180, 100, 255),
            _ => Color32::from_rgb(80, 240, 160),
        };
        self.takes.push(AudioTake {
            take_number: count,
            name: if name.is_empty() { format!("Tagning {}", count) } else { name.to_string() },
            waveform_data: new_wave,
            is_selected: true,
            color,
            start_bar,
            length_bars,
        });
        self.active_comp_take = self.takes.len() - 1;
    }

    pub fn add_custom_sound(&mut self, name: &str, category: &str) {
        let count = self.custom_sounds.len() + 1;
        let mut new_wave = Vec::with_capacity(60);
        for i in 0..60 {
            let t = i as f32 / 60.0;
            let val = ((1.0 - t) * (t * 36.0).sin().abs()).clamp(0.05, 0.95);
            new_wave.push(val);
        }
        self.custom_sounds.push(CustomSoundClip {
            id: count,
            name: if name.is_empty() { format!("Eget Ljud {}", count) } else { name.to_string() },
            category: if category.is_empty() { "Eget Ljud".to_string() } else { category.to_string() },
            waveform_data: new_wave,
            duration_secs: 1.5,
            color: Color32::from_rgb(255, 180, 60),
        });
    }
}
