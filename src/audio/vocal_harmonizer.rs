use eframe::egui::Color32;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PitchBlob {
    pub start_step: usize,
    pub length_steps: usize,
    pub midi_note: u8,
    pub original_pitch_freq: f32,
    pub corrected_pitch_freq: f32,
    pub text_lyric: &'static str,
    pub color: Color32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HarmonyVoice {
    pub name: &'static str,
    pub interval_semitones: i8,
    pub volume: f32,
    pub pan: f32,
    pub enabled: bool,
    pub formant_shift: f32, // -12 .. +12
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VocalHarmonizer {
    pub autotune_speed: f32, // 0.0 (Natural) .. 1.0 (Hard Robot T-Pain)
    pub autotune_scale_snap: bool,
    pub target_scale: usize, // 0: Chromatic, 1: Major, 2: Minor, etc.
    pub blobs: Vec<PitchBlob>,
    pub voices: Vec<HarmonyVoice>,
    pub selected_blob_idx: Option<usize>,
}

impl Default for VocalHarmonizer {
    fn default() -> Self {
        let mut harm = Self {
            autotune_speed: 0.75,
            autotune_scale_snap: true,
            target_scale: 2, // A Minor
            blobs: Vec::new(),
            voices: vec![
                HarmonyVoice { name: "🎤 Lead Sång (Center)", interval_semitones: 0, volume: 1.0, pan: 0.0, enabled: true, formant_shift: 0.0 },
                HarmonyVoice { name: "✨ Hög Ters (+3st)", interval_semitones: 3, volume: 0.75, pan: 0.35, enabled: true, formant_shift: 1.5 },
                HarmonyVoice { name: "🎶 Låg Kvint (-5st)", interval_semitones: -5, volume: 0.70, pan: -0.35, enabled: true, formant_shift: -2.0 },
                HarmonyVoice { name: "🌌 Oktav Högre (+12st)", interval_semitones: 12, volume: 0.55, pan: 0.0, enabled: false, formant_shift: 3.0 },
            ],
            selected_blob_idx: None,
        };
        harm.populate_demo_pitch_blobs();
        harm
    }
}

impl VocalHarmonizer {
    pub fn populate_demo_pitch_blobs(&mut self) {
        self.blobs = vec![
            PitchBlob { start_step: 0, length_steps: 3, midi_note: 60, original_pitch_freq: 260.0, corrected_pitch_freq: 261.63, text_lyric: "Cause", color: Color32::from_rgb(0, 220, 255) },
            PitchBlob { start_step: 3, length_steps: 2, midi_note: 62, original_pitch_freq: 295.0, corrected_pitch_freq: 293.66, text_lyric: "we", color: Color32::from_rgb(0, 220, 255) },
            PitchBlob { start_step: 5, length_steps: 3, midi_note: 64, original_pitch_freq: 331.0, corrected_pitch_freq: 329.63, text_lyric: "are", color: Color32::from_rgb(0, 220, 255) },
            PitchBlob { start_step: 8, length_steps: 4, midi_note: 67, original_pitch_freq: 390.0, corrected_pitch_freq: 392.00, text_lyric: "star-", color: Color32::from_rgb(255, 140, 0) },
            PitchBlob { start_step: 12, length_steps: 4, midi_note: 65, original_pitch_freq: 351.0, corrected_pitch_freq: 349.23, text_lyric: "dust", color: Color32::from_rgb(255, 140, 0) },
        ];
    }
}
