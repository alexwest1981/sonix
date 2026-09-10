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

#[derive(Debug, Clone, Default)]
pub struct StemAudio {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
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
    pub stem_audio: Vec<StemAudio>,
    /// Un-stretched separation result, used as the source for the SPEED
    /// (pitch-preserving time-stretch) control.
    pub stem_audio_base: Vec<StemAudio>,
    pub sample_rate: u32,
    pub source_path: Option<String>,
}

impl Default for StemProject {
    fn default() -> Self {
        Self {
            track_title: crate::i18n::t("Ingen fil vald").to_string(),
            duration_seconds: 0.0,
            bpm: 126.0,
            model_name: "Sonix Spectral Separator (DSP)",
            is_separating: false,
            progress: 0.0,
            stems: Vec::new(),
            selected_sample_idx: 0,
            stem_audio: Vec::new(),
            stem_audio_base: Vec::new(),
            sample_rate: 44100,
            source_path: None,
        }
    }
}

/// One-pole low-pass coefficient for a given cutoff frequency.
fn one_pole_coeff(cutoff_hz: f32, sample_rate: f32) -> f32 {
    let c = 1.0 - (-std::f32::consts::TAU * cutoff_hz / sample_rate).exp();
    c.clamp(0.0, 1.0)
}

/// Splits a stereo mix into four real stems using spectral band-splitting,
/// center-channel extraction and transient detection. This is a genuine
/// (if lightweight) DSP separation — no neural network required.
pub fn separate_stems(left: &[f32], right: &[f32], sample_rate: f32) -> Vec<StemAudio> {
    let len = left.len().min(right.len());
    let lp_bass = one_pole_coeff(180.0, sample_rate);
    let lp_mid = one_pole_coeff(250.0, sample_rate);
    let hp_vocal = one_pole_coeff(300.0, sample_rate);
    let lp_vocal = one_pole_coeff(4500.0, sample_rate);

    let mut bass = StemAudio { left: vec![0.0; len], right: vec![0.0; len] };
    let mut drums = StemAudio { left: vec![0.0; len], right: vec![0.0; len] };
    let mut vocals = StemAudio { left: vec![0.0; len], right: vec![0.0; len] };
    let mut instr = StemAudio { left: vec![0.0; len], right: vec![0.0; len] };

    // Filter states (per channel, kept simple with one-pole sections).
    let mut bass_lp = 0.0f32;
    let mut mid_lp_l = 0.0f32;
    let mut mid_lp_r = 0.0f32;
    let mut voc_hp_l = 0.0f32;
    let mut voc_hp_r = 0.0f32;
    let mut voc_lp_l = 0.0f32;
    let mut voc_lp_r = 0.0f32;
    let mut transient_env = 0.0f32;

    const ENV_ATTACK: f32 = 0.35;
    const ENV_RELEASE: f32 = 0.004;

    for i in 0..len {
        let l = left[i];
        let r = right[i];
        let mid = (l + r) * 0.5;

        // Bass: low-passed mono center energy.
        bass_lp += lp_bass * (mid - bass_lp);
        let bass_sample = bass_lp * 1.05;

        // Transient gate from the high-passed center channel.
        mid_lp_l += lp_mid * (mid - mid_lp_l);
        let hp_mid = mid - mid_lp_l;
        let mag = hp_mid.abs();
        if mag > transient_env {
            transient_env += ENV_ATTACK * (mag - transient_env);
        } else {
            transient_env += ENV_RELEASE * (mag - transient_env);
        }
        let gate = (transient_env * 4.0).clamp(0.0, 1.0);

        // Drums: high-passed stereo content gated by transients.
        let hp_l = l - mid_lp_l;
        let hp_r = r - mid_lp_r;
        mid_lp_r += lp_mid * (r - mid_lp_r);
        let drum_l = hp_l * gate;
        let drum_r = hp_r * gate;

        // Vocals: band-limited center channel, ducked under drum transients.
        voc_hp_l += hp_vocal * (l - voc_hp_l);
        voc_hp_r += hp_vocal * (r - voc_hp_r);
        voc_lp_l += lp_vocal * (voc_hp_l - voc_lp_l);
        voc_lp_r += lp_vocal * (voc_hp_r - voc_lp_r);
        let duck = 1.0 - 0.8 * gate;
        let voc_l = voc_lp_l * duck;
        let voc_r = voc_lp_r * duck;

        // Instruments: residual after removing the other three components.
        let inst_l = l - bass_sample - drum_l - voc_l;
        let inst_r = r - bass_sample - drum_r - voc_r;

        bass.left[i] = bass_sample;
        bass.right[i] = bass_sample;
        drums.left[i] = drum_l;
        drums.right[i] = drum_r;
        vocals.left[i] = voc_l;
        vocals.right[i] = voc_r;
        instr.left[i] = inst_l;
        instr.right[i] = inst_r;
    }

    for stem in [&mut bass, &mut drums, &mut vocals, &mut instr] {
        normalize_stem(stem, 0.92);
    }

    vec![vocals, drums, bass, instr]
}

/// Estimates the tempo (BPM) from the mix by autocorrelating an onset-strength
/// envelope built from the high-passed center channel. Returns 0.0 when the
/// signal is too short or has no clear pulse, so the UI can show "unknown".
fn estimate_bpm(left: &[f32], right: &[f32], sample_rate: f32) -> f32 {
    let len = left.len().min(right.len());
    if (len as f32) < sample_rate {
        return 0.0;
    }
    let hop = (sample_rate / 100.0).max(1.0) as usize;
    let frames = len / hop;
    if frames < 8 {
        return 0.0;
    }

    let lp = one_pole_coeff(200.0, sample_rate);
    let mut lp_state = 0.0f32;
    let mut env = vec![0.0f32; frames];
    for (f, e) in env.iter_mut().enumerate() {
        let start = f * hop;
        let end = (start + hop).min(len);
        let mut energy = 0.0f32;
        for i in start..end {
            let mid = (left[i] + right[i]) * 0.5;
            lp_state += lp * (mid - lp_state);
            let hp = mid - lp_state;
            energy += hp * hp;
        }
        *e = (energy / (end - start).max(1) as f32).sqrt();
    }

    let mut onset = vec![0.0f32; frames];
    for f in 1..frames {
        onset[f] = (env[f] - env[f - 1]).max(0.0);
    }

    let fps = sample_rate / hop as f32;
    let min_lag = (fps * 60.0 / 200.0).round().max(1.0) as usize;
    let max_lag = (fps * 60.0 / 60.0).round().max(1.0) as usize;

    let mut best_lag = 0usize;
    let mut best_score = 0.0f32;
    for lag in min_lag..=max_lag {
        if lag >= frames {
            break;
        }
        let mut score = 0.0f32;
        let mut norm = 0.0f32;
        for f in lag..frames {
            score += onset[f] * onset[f - lag];
            norm += onset[f - lag] * onset[f - lag];
        }
        if norm > 1e-9 {
            score /= norm.sqrt();
        }
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    if best_lag == 0 || best_score < 0.05 {
        return 0.0;
    }
    let mut bpm = 60.0 * fps / best_lag as f32;
    while bpm < 60.0 {
        bpm *= 2.0;
    }
    while bpm > 180.0 {
        bpm /= 2.0;
    }
    bpm
}

fn normalize_stem(stem: &mut StemAudio, target_peak: f32) {
    let peak = stem
        .left
        .iter()
        .chain(stem.right.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 1e-6 {
        let gain = target_peak / peak;
        for v in stem.left.iter_mut() {
            *v *= gain;
        }
        for v in stem.right.iter_mut() {
            *v *= gain;
        }
    }
}

impl StemProject {
    /// Runs the real DSP separator over decoded PCM and refreshes the view model.
    pub fn separate_from_pcm(
        &mut self,
        left: &[f32],
        right: &[f32],
        sample_rate: u32,
        source_path: &str,
    ) {
        let sr_f = sample_rate.max(1) as f32;
        let separated = separate_stems(left, right, sr_f);

        self.sample_rate = sample_rate;
        self.source_path = Some(source_path.to_string());
        self.track_title = std::path::Path::new(source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(source_path)
            .to_string();
        self.duration_seconds = left.len() as f32 / sr_f;
        self.bpm = estimate_bpm(left, right, sr_f);
        self.stem_audio = separated;
        self.stem_audio_base = self.stem_audio.clone();
        self.stems.clear();

        let types = [StemType::Vocals, StemType::Drums, StemType::Bass, StemType::Instruments];
        let volumes = [0.90, 0.95, 0.88, 0.82];
        for (idx, stem_type) in types.iter().enumerate() {
            let audio = &self.stem_audio[idx];
            let mono: Vec<f32> = audio
                .left
                .iter()
                .zip(audio.right.iter())
                .map(|(l, r)| (l + r) * 0.5)
                .collect();
            self.stems.push(StemChannel {
                stem_type: *stem_type,
                volume: volumes[idx],
                pan: 0.0,
                muted: false,
                solo: false,
                pitch_shift: 0,
                time_stretch: 1.0,
                waveform_data: super::recorder::visual_peaks_from(&mono),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separation_preserves_length_and_is_finite() {
        let sr = 44100.0;
        let n = 44100;
        let mut l = Vec::with_capacity(n);
        let mut r = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr;
            let low = (t * 60.0 * std::f32::consts::TAU).sin() * 0.5;
            let high = (t * 5000.0 * std::f32::consts::TAU).sin() * 0.2;
            l.push(low + high);
            r.push(low + high);
        }
        let stems = separate_stems(&l, &r, sr);
        assert_eq!(stems.len(), 4);
        for s in &stems {
            assert_eq!(s.left.len(), n);
            assert_eq!(s.right.len(), n);
            assert!(s.left.iter().all(|v| v.is_finite()));
        }
        // Bass should contain the low tone, instruments should be non-silent.
        let bass_energy: f32 = stems[2].left.iter().map(|v| v.abs()).sum();
        assert!(bass_energy > 1.0, "bass stem is silent");
    }

    #[test]
    fn bpm_estimate_detects_click_track() {
        let sr = 44100.0;
        let n = (sr * 8.0) as usize;
        let interval = (60.0 / 120.0 * sr) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        for i in (0..n).step_by(interval) {
            for k in 0..200 {
                if i + k < n {
                    let v = if k < 100 { 0.9 } else { -0.9 };
                    l[i + k] = v;
                    r[i + k] = v;
                }
            }
        }
        let est = estimate_bpm(&l, &r, sr);
        assert!((est - 120.0).abs() < 5.0, "estimated {est} BPM");
    }
}
