use std::f32::consts::TAU;
use std::path::Path;
use super::wav_writer::write_pcm_f32_to_wav;

pub struct GeneratedSample {
    pub name: String,
    pub category: String,
    pub icon: String,
    pub default_note: u8,
    pub color_rgb: (u8, u8, u8),
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

impl GeneratedSample {
    pub fn new_stereo(
        name: impl Into<String>,
        category: impl Into<String>,
        icon: impl Into<String>,
        default_note: u8,
        color_rgb: (u8, u8, u8),
        left: Vec<f32>,
        right: Vec<f32>,
        sample_rate: u32,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            icon: icon.into(),
            default_note,
            color_rgb,
            left,
            right,
            sample_rate,
        }
    }

    pub fn compute_waveform_peaks(&self, num_points: usize) -> Vec<f32> {
        let n = num_points.max(32);
        let total = self.left.len().max(1);
        let chunk_size = (total / n).max(1);
        let mut peaks = Vec::with_capacity(n);

        for i in 0..n {
            let start = i * chunk_size;
            if start >= total {
                peaks.push(0.05);
                continue;
            }
            let end = ((i + 1) * chunk_size).min(total);
            let mut max_val: f32 = 0.0;
            for j in start..end {
                let val = (self.left[j].abs() + self.right[j].abs()) * 0.5;
                if val > max_val {
                    max_val = val;
                }
            }
            peaks.push(max_val.clamp(0.05, 1.0));
        }
        peaks
    }

    pub fn save_to_file(&self, dir: &Path) -> std::io::Result<String> {
        let safe_name = self.name
            .replace('/', "_")
            .replace(' ', "_")
            .replace(':', "_")
            .replace('(', "")
            .replace(')', "");
        let file_path = dir.join(format!("{}.wav", safe_name));
        let path_str = file_path.to_string_lossy().to_string();

        let mut interleaved = Vec::with_capacity(self.left.len() * 2);
        for i in 0..self.left.len() {
            interleaved.push(self.left[i]);
            interleaved.push(self.right[i]);
        }

        if let Err(e) = write_pcm_f32_to_wav(&path_str, &interleaved, self.sample_rate, 2) {
            eprintln!("[Sonix] Kunde inte spara factory sample {}: {}", path_str, e);
        }

        Ok(path_str)
    }
}

// Simple fast PRNG for noise generation
struct XorShift {
    state: u32,
}
impl XorShift {
    fn new(seed: u32) -> Self {
        Self { state: if seed == 0 { 0x12345678 } else { seed } }
    }
    fn next_f32(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        (self.state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

// Lowpass filter utility
struct SimpleLpf {
    y: f32,
    alpha: f32,
}
impl SimpleLpf {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (TAU * cutoff_hz.max(10.0));
        let alpha = dt / (rc + dt);
        Self { y: 0.0, alpha }
    }
    fn process(&mut self, x: f32) -> f32 {
        self.y += self.alpha * (x - self.y);
        self.y
    }
}

pub fn generate_all_factory_samples() -> Vec<GeneratedSample> {
    let sr = 44100_u32;
    let srf = sr as f32;
    let mut list = Vec::new();

    // =========================================================================
    // 1. DRUM LOOPS (4 BARS)
    // =========================================================================

    // 1.1 Boom Bap Hip-Hop Loop (90 BPM, 4 Bars)
    {
        let bpm = 90.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(42);

        let step_secs = (60.0 / bpm) / 4.0; // 16th note
        let total_steps = 64; // 4 bars * 16 steps

        for step in 0..total_steps {
            let step_start = (step as f32 * step_secs * srf) as usize;
            let step_in_bar = step % 16;

            // Kick pattern (1, 1.75, 2.5, 3.5 swing)
            let is_kick = step_in_bar == 0 || step_in_bar == 6 || step_in_bar == 10 || (step % 32 == 28);
            if is_kick {
                let len = ((0.32 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let freq = 45.0 + 110.0 * (-t * 30.0).exp();
                    let body = (TAU * freq * t).sin() * (-t * 8.5).exp() * 0.9;
                    let click = if t < 0.004 { rng.next_f32() * 0.4 } else { 0.0 };
                    let s = (body + click).clamp(-1.0, 1.0);
                    l[step_start + i] += s * 0.85;
                    r[step_start + i] += s * 0.85;
                }
            }

            // Snare pattern (beats 2 and 4 = steps 4 and 12, plus ghost notes)
            let is_snare = step_in_bar == 4 || step_in_bar == 12;
            let is_ghost = step_in_bar == 14 || (step % 32 == 30);
            if is_snare || is_ghost {
                let vel = if is_snare { 0.85 } else { 0.35 };
                let len = ((0.28 * srf) as usize).min(total_samples.saturating_sub(step_start));
                let mut snare_lpf = SimpleLpf::new(4500.0, srf);
                for i in 0..len {
                    let t = i as f32 / srf;
                    let tone = (TAU * 185.0 * t).sin() * (-t * 22.0).exp() * 0.55;
                    let noise = snare_lpf.process(rng.next_f32()) * (-t * 14.0).exp() * 0.75;
                    let s = (tone + noise) * vel;
                    l[step_start + i] += s * 0.9;
                    r[step_start + i] += s * 0.85;
                }
            }

            // Hi-Hat pattern (8th notes swung)
            let is_hat = step_in_bar % 2 == 0 || step_in_bar == 7 || step_in_bar == 15;
            if is_hat {
                let vel = if step_in_bar % 4 == 2 { 0.6 } else { 0.4 };
                let len = ((0.06 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let noise = rng.next_f32() * (-t * 80.0).exp() * vel;
                    l[step_start + i] += noise * 0.45;
                    r[step_start + i] += noise * 0.55;
                }
            }
        }

        // Soft limit & normalize
        for i in 0..total_samples {
            l[i] = (l[i] * 1.1).tanh();
            r[i] = (r[i] * 1.1).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Boom Bap 90 BPM Beat (4-Bar)",
            "Trumloopar",
            "🥁",
            60,
            (255, 140, 25),
            l, r, sr,
        ));
    }

    // 1.2 Trap 808 & Rolling Hats Loop (140 BPM, 4 Bars)
    {
        let bpm = 140.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(101);

        let step_secs = (60.0 / bpm) / 4.0;
        let total_steps = 64;

        for step in 0..total_steps {
            let step_start = (step as f32 * step_secs * srf) as usize;
            let step_in_bar = step % 16;

            // Trap Sub Kick on beat 1, 7, 11
            if step_in_bar == 0 || step_in_bar == 6 || step_in_bar == 10 || (step % 32 == 26) {
                let len = ((0.45 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let pitch_env = (-t * 24.0).exp();
                    let freq = 42.0 + 120.0 * pitch_env;
                    let body = (TAU * freq * t).sin() * (-t * 5.5).exp() * 1.1;
                    let sat = (body * 1.4).tanh();
                    l[step_start + i] += sat * 0.9;
                    r[step_start + i] += sat * 0.9;
                }
            }

            // Snappy Trap Clap / Snare on 8 (Double time beat 3)
            if step_in_bar == 8 {
                let len = ((0.30 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let pulse = if t < 0.010 { 1.0 } else if t < 0.022 { 0.8 } else if t < 0.034 { 1.2 } else { (- (t - 0.034) * 16.0).exp() };
                    let noise = rng.next_f32() * pulse * 0.85;
                    l[step_start + i] += noise * 0.8;
                    r[step_start + i] += noise * 0.9;
                }
            }

            // Rolling Trap Hats (16ths with 32nd triplet rolls at bar ends)
            let is_roll = (step_in_bar >= 12 && step_in_bar <= 15) && (step / 16 % 2 == 1);
            let sub_divisions = if is_roll { 2 } else { 1 };
            for sub in 0..sub_divisions {
                let sub_offset = (sub as f32 * (step_secs / sub_divisions as f32) * srf) as usize;
                let sub_start = (step_start + sub_offset).min(total_samples);
                let len = ((0.04 * srf) as usize).min(total_samples.saturating_sub(sub_start));
                let pan = if is_roll { 0.3 * (sub as f32 - 0.5) } else { 0.0 };
                for i in 0..len {
                    let t = i as f32 / srf;
                    let noise = rng.next_f32() * (-t * 90.0).exp() * 0.45;
                    l[sub_start + i] += noise * (0.5 - pan);
                    r[sub_start + i] += noise * (0.5 + pan);
                }
            }
        }

        for i in 0..total_samples {
            l[i] = (l[i] * 1.1).tanh();
            r[i] = (r[i] * 1.1).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Trap 808 & Rolls 140 BPM (4-Bar)",
            "Trumloopar",
            "🥁",
            60,
            (255, 80, 140),
            l, r, sr,
        ));
    }

    // 1.3 Synthwave 80s Cyber Groove (120 BPM, 4 Bars)
    {
        let bpm = 120.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(777);

        let step_secs = (60.0 / bpm) / 4.0;
        for step in 0..64 {
            let step_start = (step as f32 * step_secs * srf) as usize;
            let step_in_bar = step % 16;

            // 4-on-the-floor Kick
            if step_in_bar % 4 == 0 {
                let len = ((0.28 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let freq = 50.0 + 130.0 * (-t * 35.0).exp();
                    let s = (TAU * freq * t).sin() * (-t * 9.0).exp() * 0.95;
                    l[step_start + i] += s;
                    r[step_start + i] += s;
                }
            }

            // Gated Reverb 80s Snare on beats 2 and 4 (steps 4, 12)
            if step_in_bar == 4 || step_in_bar == 12 {
                let len = ((0.35 * srf) as usize).min(total_samples.saturating_sub(step_start));
                for i in 0..len {
                    let t = i as f32 / srf;
                    let tone = (TAU * 210.0 * t).sin() * (-t * 18.0).exp() * 0.45;
                    // Gated noise burst with abrupt cut at 0.22s
                    let gate = if t < 0.22 { 0.85 } else { (- (t - 0.22) * 50.0).exp() * 0.85 };
                    let noise = rng.next_f32() * gate;
                    l[step_start + i] += (tone + noise) * 0.8;
                    r[step_start + i] += (tone + noise) * 0.85;
                }
            }

            // Driving 16th Hi-Hats with Open Hat on the off-beats (steps 2, 6, 10, 14)
            let is_open = step_in_bar % 4 == 2;
            let len = (((if is_open { 0.25 } else { 0.05 }) * srf) as usize).min(total_samples.saturating_sub(step_start));
            let decay = if is_open { 16.0 } else { 85.0 };
            for i in 0..len {
                let t = i as f32 / srf;
                let noise = rng.next_f32() * (-t * decay).exp() * (if is_open { 0.55 } else { 0.35 });
                l[step_start + i] += noise * 0.4;
                r[step_start + i] += noise * 0.45;
            }
        }

        for i in 0..total_samples {
            l[i] = (l[i] * 1.05).tanh();
            r[i] = (r[i] * 1.05).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Synthwave 80s Retro Groove 120 BPM (4-Bar)",
            "Trumloopar",
            "🥁",
            60,
            (80, 220, 255),
            l, r, sr,
        ));
    }

    // 1.4 Lo-Fi Shaker & Percussion Loop (95 BPM, 4 Bars)
    {
        let bpm = 95.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(999);

        let step_secs = (60.0 / bpm) / 4.0;
        for step in 0..64 {
            let step_start = (step as f32 * step_secs * srf) as usize;
            let step_in_bar = step % 16;
            let accent = match step_in_bar % 4 {
                0 => 0.5,
                1 => 0.3,
                2 => 0.8,
                _ => 0.4,
            };
            let len = ((0.09 * srf) as usize).min(total_samples.saturating_sub(step_start));
            let pan = ((step as f32 * 0.3).sin() * 0.25).clamp(-0.4, 0.4);
            for i in 0..len {
                let t = i as f32 / srf;
                let env = (t * 50.0).sin().abs() * (-t * 35.0).exp();
                let noise = rng.next_f32() * env * accent * 0.7;
                l[step_start + i] += noise * (0.5 - pan);
                r[step_start + i] += noise * (0.5 + pan);
            }
        }

        list.push(GeneratedSample::new_stereo(
            "Lo-Fi Shaker Groove 95 BPM (4-Bar)",
            "Trumloopar",
            "🪘",
            60,
            (180, 140, 255),
            l, r, sr,
        ));
    }

    // =========================================================================
    // 2. LUSH PADS, AMBIENT & STRINGS (4-BAR SEAMLESS STEREO CHORDS)
    // =========================================================================

    // 2.1 Dreamy Neon Pad (Cmaj9)
    {
        let duration_secs = 8.0; // 2-4 bars
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        // Cmaj9: C3 (130.81), E3 (164.81), G3 (196.00), B3 (246.94), D4 (293.66)
        let freqs = [130.81, 164.81, 196.00, 246.94, 293.66];
        for (note_idx, &base_f) in freqs.iter().enumerate() {
            let pan = (note_idx as f32 / (freqs.len() - 1) as f32) * 0.6 - 0.3;
            for i in 0..total_samples {
                let t = i as f32 / srf;
                // Attack 1.2s, smooth sustain, release 1.5s
                let env = if t < 1.2 {
                    t / 1.2
                } else if t > duration_secs - 1.5 {
                    ((duration_secs - t) / 1.5).max(0.0)
                } else {
                    1.0
                };

                // Triple detuned saw oscillators
                let detune1 = 1.0025;
                let detune2 = 0.9975;
                let saw1 = ((t * base_f) % 1.0) * 2.0 - 1.0;
                let saw2 = ((t * base_f * detune1) % 1.0) * 2.0 - 1.0;
                let saw3 = ((t * base_f * detune2) % 1.0) * 2.0 - 1.0;

                // Subtle chorus LFO
                let chorus = (t * 0.4 * TAU).sin() * 0.15;
                let mix = (saw1 * 0.4 + saw2 * 0.3 + saw3 * 0.3) * env * 0.22;

                l[i] += mix * (0.5 - pan + chorus);
                r[i] += mix * (0.5 + pan - chorus);
            }
        }

        // Lowpass smooth filter sweep
        let mut lpf_l = SimpleLpf::new(1200.0, srf);
        let mut lpf_r = SimpleLpf::new(1200.0, srf);
        for i in 0..total_samples {
            l[i] = (lpf_l.process(l[i]) * 1.2).tanh();
            r[i] = (lpf_r.process(r[i]) * 1.2).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Dreamy Neon Pad (Cmaj9 Lush)",
            "Pads & Atmosfär",
            "✨",
            60,
            (100, 200, 255),
            l, r, sr,
        ));
    }

    // 2.2 Blade Runner Vangelis Brass (Dmin7)
    {
        let duration_secs = 7.5;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        // Dmin7: D3 (146.83), F3 (174.61), A3 (220.00), C4 (261.63)
        let freqs = [146.83, 174.61, 220.00, 261.63];
        for (note_idx, &base_f) in freqs.iter().enumerate() {
            let pan = (note_idx as f32 / 3.0) * 0.5 - 0.25;
            for i in 0..total_samples {
                let t = i as f32 / srf;
                let env = if t < 0.6 {
                    (t / 0.6).powf(1.4)
                } else if t > duration_secs - 1.2 {
                    ((duration_secs - t) / 1.2).max(0.0)
                } else {
                    1.0
                };

                // Warm saw with subtle pitch bend
                let pitch_mod = 1.0 + (-t * 3.0).exp() * 0.015;
                let f = base_f * pitch_mod;
                let saw1 = ((t * f) % 1.0) * 2.0 - 1.0;
                let saw2 = ((t * f * 1.004) % 1.0) * 2.0 - 1.0;
                let tri = ((t * f * 0.5) % 1.0).abs() * 4.0 - 1.0;

                let s = (saw1 * 0.45 + saw2 * 0.35 + tri * 0.2) * env * 0.25;
                l[i] += s * (0.5 - pan);
                r[i] += s * (0.5 + pan);
            }
        }

        let mut lpf_l = SimpleLpf::new(1800.0, srf);
        let mut lpf_r = SimpleLpf::new(1800.0, srf);
        for i in 0..total_samples {
            l[i] = (lpf_l.process(l[i]) * 1.3).tanh();
            r[i] = (lpf_r.process(r[i]) * 1.3).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Blade Runner CS-80 Brass (Dmin7)",
            "Pads & Atmosfär",
            "🪐",
            62,
            (255, 120, 60),
            l, r, sr,
        ));
    }

    // 2.3 Vintage Warm Tape Pad (Amin9)
    {
        let duration_secs = 7.0;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        // Amin9: A2 (110.00), C3 (130.81), E3 (164.81), G3 (196.00), B3 (246.94)
        let freqs = [110.00, 130.81, 164.81, 196.00, 246.94];
        for &base_f in &freqs {
            for i in 0..total_samples {
                let t = i as f32 / srf;
                let env = if t < 0.8 {
                    t / 0.8
                } else if t > duration_secs - 1.0 {
                    ((duration_secs - t) / 1.0).max(0.0)
                } else {
                    1.0
                };

                // Tape wow and flutter (0.35Hz + 4.2Hz LFO)
                let wow = (t * 0.35 * TAU).sin() * 0.003;
                let flutter = (t * 4.2 * TAU).sin() * 0.001;
                let f = base_f * (1.0 + wow + flutter);

                let tri = ((t * f) % 1.0).abs() * 4.0 - 1.0;
                let sine = (t * f * TAU).sin();
                let val = (tri * 0.6 + sine * 0.4) * env * 0.22;

                l[i] += val * 0.5;
                r[i] += val * 0.5;
            }
        }

        let mut lpf_l = SimpleLpf::new(950.0, srf);
        let mut lpf_r = SimpleLpf::new(950.0, srf);
        for i in 0..total_samples {
            l[i] = (lpf_l.process(l[i]) * 1.15).tanh();
            r[i] = (lpf_r.process(r[i]) * 1.15).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Vintage Tape Analog Pad (Amin9)",
            "Pads & Atmosfär",
            "🕯",
            57,
            (255, 180, 80),
            l, r, sr,
        ));
    }

    // =========================================================================
    // 3. BASSLINES & 808S
    // =========================================================================

    // 3.1 303 Resonant Acid Bassline Loop (120 BPM, 4-Bar)
    {
        let bpm = 120.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        let step_secs = (60.0 / bpm) / 4.0;
        // 16-step classic acid melody notes: C2, C2, D#2, C2, F2, D#2, G2, F2...
        let melody_midi = [
            36, 36, 39, 36, 41, 39, 43, 41,
            36, 48, 39, 36, 46, 43, 41, 38,
        ];

        for step in 0..64 {
            let note = melody_midi[step % 16];
            let base_f = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
            let step_start = (step as f32 * step_secs * srf) as usize;
            let len = ((step_secs * 0.95 * srf) as usize).min(total_samples.saturating_sub(step_start));
            let is_accent = step % 4 == 0 || step % 7 == 2;

            for i in 0..len {
                let t = i as f32 / srf;
                let saw = ((t * base_f) % 1.0) * 2.0 - 1.0;

                // 303 Filter envelope (sweeps from 4500Hz down to 400Hz with high resonance)
                let f_env = (-t * (if is_accent { 18.0 } else { 28.0 })).exp();
                let cutoff = 400.0 + (if is_accent { 3200.0 } else { 1800.0 }) * f_env;
                let reso_ring = (TAU * cutoff * t).sin() * 0.35 * f_env;

                let amp_env = (-t * 9.0).exp();
                let s = (saw * 0.7 + reso_ring) * amp_env * (if is_accent { 0.9 } else { 0.65 });
                let dist = (s * 1.8).tanh();
                l[step_start + i] += dist * 0.85;
                r[step_start + i] += dist * 0.85;
            }
        }

        list.push(GeneratedSample::new_stereo(
            "303 Acid Resonant Riff 120 BPM (4-Bar)",
            "Bas & 808",
            "🎸",
            36,
            (50, 255, 150),
            l, r, sr,
        ));
    }

    // 3.2 Deep 808 Sub Slide Bass
    {
        let duration_secs = 2.5;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        for i in 0..total_samples {
            let t = i as f32 / srf;
            // Slide: C1 (32.7) -> G1 (49.0) at t=0.6 -> D#1 (38.9) at t=1.4
            let freq = if t < 0.6 {
                32.7
            } else if t < 1.0 {
                32.7 + (49.0 - 32.7) * ((t - 0.6) / 0.4)
            } else if t < 1.4 {
                49.0
            } else {
                49.0 + (38.89 - 49.0) * ((t - 1.4) / 0.5).min(1.0)
            };

            let env = (-t * 1.2).exp();
            let body = (TAU * freq * t).sin();
            let sat = (body * 1.5).tanh() * env * 0.95;
            l[i] = sat;
            r[i] = sat;
        }

        list.push(GeneratedSample::new_stereo(
            "808 Slide Sub Bass Glide (C1-G1)",
            "Bas & 808",
            "💥",
            36,
            (255, 60, 60),
            l, r, sr,
        ));
    }

    // 3.3 Cyberpunk Reese Bass (D1 Detuned)
    {
        let duration_secs = 3.5;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let base_f = 36.71; // D1

        for i in 0..total_samples {
            let t = i as f32 / srf;
            let env = if t < 0.05 { t / 0.05 } else { (- (t - 0.05) * 0.8).exp() };

            let s1 = ((t * base_f * 0.992) % 1.0) * 2.0 - 1.0;
            let s2 = ((t * base_f * 1.008) % 1.0) * 2.0 - 1.0;
            let s3 = ((t * base_f * 2.0 * 0.996) % 1.0) * 2.0 - 1.0;
            let s4 = ((t * base_f * 2.0 * 1.004) % 1.0) * 2.0 - 1.0;

            let left_sig = (s1 * 0.5 + s3 * 0.35) * env;
            let right_sig = (s2 * 0.5 + s4 * 0.35) * env;

            l[i] = (left_sig * 1.6).tanh() * 0.9;
            r[i] = (right_sig * 1.6).tanh() * 0.9;
        }

        list.push(GeneratedSample::new_stereo(
            "Cyberpunk Reese Bass (D1 Detuned)",
            "Bas & 808",
            "🕹",
            38,
            (180, 50, 255),
            l, r, sr,
        ));
    }

    // =========================================================================
    // 4. KEYS, PLUCKS & MELODIES
    // =========================================================================

    // 4.1 Lo-Fi Rhodes EP Chord (Cmaj7)
    {
        let duration_secs = 4.0;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        // Cmaj7: C4 (261.63), E4 (329.63), G4 (392.00), B4 (493.88)
        let chord_freqs = [261.63, 329.63, 392.00, 493.88];
        for (idx, &carrier_f) in chord_freqs.iter().enumerate() {
            let pan = (idx as f32 / 3.0) * 0.4 - 0.2;
            for i in 0..total_samples {
                let t = i as f32 / srf;
                let env = (-t * 1.4).exp();
                let mod_env = (-t * 4.5).exp();

                // FM electric piano tine
                let mod_idx = 2.5 * mod_env;
                let modulator = (TAU * carrier_f * 3.0 * t).sin() * mod_idx;
                let carrier = (TAU * carrier_f * t + modulator).sin();

                // Soft tremolo
                let trem = 1.0 + 0.15 * (TAU * 4.5 * t).sin();
                let s = carrier * env * trem * 0.22;

                l[i] += s * (0.5 - pan);
                r[i] += s * (0.5 + pan);
            }
        }

        for i in 0..total_samples {
            l[i] = (l[i] * 1.2).tanh();
            r[i] = (r[i] * 1.2).tanh();
        }

        list.push(GeneratedSample::new_stereo(
            "Lo-Fi Rhodes EP Chord (Cmaj7)",
            "Keys & Melodier",
            "🎹",
            60,
            (255, 200, 100),
            l, r, sr,
        ));
    }

    // 4.2 Tropical Marimba Melody Loop (120 BPM, 4-Bar)
    {
        let bpm = 120.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        let step_secs = (60.0 / bpm) / 4.0;
        let pentatonic = [60, 62, 64, 67, 69, 72, 74, 76, 72, 69, 67, 64, 62, 64, 67, 60];

        for step in 0..64 {
            let note = pentatonic[step % 16];
            let f = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
            let step_start = (step as f32 * step_secs * srf) as usize;
            let len = ((0.35 * srf) as usize).min(total_samples.saturating_sub(step_start));
            let pan = ((step as f32 * 0.5).sin() * 0.3).clamp(-0.35, 0.35);

            for i in 0..len {
                let t = i as f32 / srf;
                // Wooden marimba transient + pure sine body
                let wood_click = if t < 0.003 { (t * 8000.0 * TAU).sin() * 0.5 } else { 0.0 };
                let body = (TAU * f * t).sin() * (-t * 12.0).exp();
                let harm = (TAU * f * 3.8 * t).sin() * (-t * 28.0).exp() * 0.25;

                let s = (body + harm + wood_click) * 0.4;
                l[step_start + i] += s * (0.5 - pan);
                r[step_start + i] += s * (0.5 + pan);
            }
        }

        list.push(GeneratedSample::new_stereo(
            "Tropical Marimba Loop 120 BPM (4-Bar)",
            "Keys & Melodier",
            "🏝",
            60,
            (80, 240, 180),
            l, r, sr,
        ));
    }

    // 4.3 8-Bit Chiptune Retro Arp Loop (128 BPM, 4-Bar)
    {
        let bpm = 128.0;
        let bar_secs = (60.0 / bpm) * 4.0;
        let total_secs = bar_secs * 4.0;
        let total_samples = (total_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        let arp_rate = 24.0; // 24 Hz rapid arpeggio
        let chords = [
            [60, 64, 67, 72], // C
            [57, 60, 64, 69], // Am
            [65, 69, 72, 77], // F
            [67, 71, 74, 79], // G
        ];

        for i in 0..total_samples {
            let t = i as f32 / srf;
            let bar_idx = ((t / bar_secs) as usize).min(3);
            let chord = chords[bar_idx];
            let note_idx = ((t * arp_rate) as usize) % 4;
            let note = chord[note_idx];
            let f = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);

            // 50% duty cycle square wave
            let sqr = if (t * f) % 1.0 < 0.5 { 0.35 } else { -0.35 };
            l[i] = sqr;
            r[i] = sqr;
        }

        list.push(GeneratedSample::new_stereo(
            "8-Bit Chiptune Retro Arp 128 BPM (4-Bar)",
            "Keys & Melodier",
            "👾",
            60,
            (255, 100, 220),
            l, r, sr,
        ));
    }

    // =========================================================================
    // 5. FX & TRANSITIONS
    // =========================================================================

    // 5.1 2-Bar Uplifter Noise Sweep (120 BPM)
    {
        let bpm = 120.0;
        let duration_secs = (60.0 / bpm) * 8.0; // 2 bars = 8 beats
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(333);

        for i in 0..total_samples {
            let t = i as f32 / srf;
            let progress = (t / duration_secs).clamp(0.0, 1.0);
            let cutoff = 200.0 + 10000.0 * progress.powf(2.2);
            let resonance = 0.4 * (TAU * cutoff * t).sin();
            let amp = progress.powf(1.5) * 0.85;

            let noise_l = rng.next_f32();
            let noise_r = rng.next_f32();
            l[i] = (noise_l * 0.7 + resonance) * amp;
            r[i] = (noise_r * 0.7 + resonance) * amp;
        }

        list.push(GeneratedSample::new_stereo(
            "Uplifter Noise Sweep (2-Bar)",
            "FX & Övergångar",
            "🌪",
            60,
            (140, 220, 255),
            l, r, sr,
        ));
    }

    // 5.2 Deep Sub Boom & Vinyl Impact
    {
        let duration_secs = 3.0;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];
        let mut rng = XorShift::new(555);

        for i in 0..total_samples {
            let t = i as f32 / srf;
            let freq = 75.0 * (-t * 1.8).exp().max(0.3);
            let env = (-t * 1.5).exp();
            let boom = (TAU * freq * t).sin() * env * 1.1;
            let vinyl_crackle = if rng.next_f32() > 0.985 { rng.next_f32() * 0.15 * env } else { 0.0 };

            let s = ((boom + vinyl_crackle) * 1.3).tanh();
            l[i] = s;
            r[i] = s;
        }

        list.push(GeneratedSample::new_stereo(
            "Sub Boom & Vinyl Impact Drop",
            "FX & Övergångar",
            "💥",
            36,
            (255, 80, 80),
            l, r, sr,
        ));
    }

    // 5.3 Sci-Fi Laser Zap
    {
        let duration_secs = 0.45;
        let total_samples = (duration_secs * srf) as usize;
        let mut l = vec![0.0_f32; total_samples];
        let mut r = vec![0.0_f32; total_samples];

        for i in 0..total_samples {
            let t = i as f32 / srf;
            let freq = 3200.0 * (-t * 22.0).exp() + 60.0;
            let env = (-t * 10.0).exp();
            let zap = (TAU * freq * t).sin() * env * 0.9;
            l[i] = zap;
            r[i] = zap;
        }

        list.push(GeneratedSample::new_stereo(
            "Sci-Fi Laser Zap One-Shot",
            "FX & Övergångar",
            "🚀",
            72,
            (80, 255, 200),
            l, r, sr,
        ));
    }

    list
}

pub fn ensure_factory_samples_directory() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::PathBuf::from(format!("{}/Music/Sonix/Factory_Samples", home));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[derive(Clone, Debug)]
pub struct ScannedSampleItem {
    pub name: String,
    pub category: String,
    pub icon: String,
    pub default_note: u8,
    pub color_rgb: (u8, u8, u8),
    pub waveform: Vec<f32>,
    pub file_path: String,
}

fn cache_fingerprint() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let roots = [
        format!("{}/Music/Sonix/Factory_Samples", home),
        format!("{}/Music/Sonix/Sample_Packs", home),
        format!("{}/Music/Sonix/User_Samples", home),
    ];
    let mut fp = String::new();
    for r in roots {
        fp.push_str(&r);
        fp.push(':');
        fp.push_str(&std::fs::metadata(&r).map(|m| m.len()).unwrap_or(0).to_string());
        fp.push(';');
    }
    fp
}

fn cache_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(format!("{}/Music/Sonix/library_cache.tsv", home))
}

fn write_library_cache(items: &[ScannedSampleItem]) {
    let mut s = String::new();
    s.push_str("SONIXLIB1\n");
    s.push_str(&cache_fingerprint());
    s.push('\n');
    for it in items {
        let size = std::fs::metadata(&it.file_path).map(|m| m.len()).unwrap_or(0);
        s.push_str("I\n");
        s.push_str(&it.file_path);
        s.push('\n');
        s.push_str(&size.to_string());
        s.push('\n');
        s.push_str(&it.name);
        s.push('\n');
        s.push_str(&it.category);
        s.push('\n');
        s.push_str(&it.icon);
        s.push('\n');
        s.push_str(&it.default_note.to_string());
        s.push('\n');
        s.push_str(&it.color_rgb.0.to_string());
        s.push('\n');
        s.push_str(&it.color_rgb.1.to_string());
        s.push('\n');
        s.push_str(&it.color_rgb.2.to_string());
        s.push('\n');
        for (i, v) in it.waveform.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&format!("{:.6}", v));
        }
        s.push('\n');
    }
    if let Some(parent) = cache_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(cache_path(), s);
}

fn read_library_cache() -> Option<Vec<ScannedSampleItem>> {
    let contents = std::fs::read_to_string(cache_path()).ok()?;
    let mut lines = contents.lines().peekable();
    if lines.next()? != "SONIXLIB1" {
        return None;
    }
    if lines.next()? != cache_fingerprint() {
        return None;
    }
    let mut items = Vec::new();
    while let Some(header) = lines.next() {
        if header != "I" {
            return None;
        }
        let file_path = lines.next()?.to_string();
        let size = lines.next()?.parse::<u64>().ok()?;
        let name = lines.next()?.to_string();
        let category = lines.next()?.to_string();
        let icon = lines.next()?.to_string();
        let default_note = lines.next()?.parse::<u8>().ok()?;
        let r = lines.next()?.parse::<u8>().ok()?;
        let g = lines.next()?.parse::<u8>().ok()?;
        let b = lines.next()?.parse::<u8>().ok()?;
        let waveform = lines
            .next()?
            .split_whitespace()
            .map(|t| t.parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()?;
        if std::fs::metadata(&file_path).map(|m| m.len()).ok()? != size {
            return None;
        }
        items.push(ScannedSampleItem {
            name,
            category,
            icon,
            default_note,
            color_rgb: (r, g, b),
            waveform,
            file_path,
        });
    }
    Some(items)
}

pub fn scan_and_load_all_samples() -> Vec<ScannedSampleItem> {
    let mut items = if let Some(cached) = read_library_cache() {
        eprintln!("🎵 Ljudbibliotek läst från cache: {} samplar", cached.len());
        cached
    } else {
        let fresh = perform_full_sample_scan();
        write_library_cache(&fresh);
        fresh
    };
    for it in &mut items {
        it.category = crate::i18n::translate_owned(crate::i18n::current(), &it.category);
    }
    items
}

fn perform_full_sample_scan() -> Vec<ScannedSampleItem> {
    let mut results = Vec::new();
    let scan_start = std::time::Instant::now();

    // 1. Generate & save Sonix Factory DSP Loops and Pads
    let factory_dir = ensure_factory_samples_directory();
    let generated = generate_all_factory_samples();
    eprintln!("🧪 DSP-genererade samplar: {}", generated.len());
    for gen_s in generated {
        let path_res = gen_s.save_to_file(&factory_dir);
        if let Ok(path_str) = path_res {
            let waveform = gen_s.compute_waveform_peaks(64);
            results.push(ScannedSampleItem {
                name: gen_s.name,
                category: gen_s.category,
                icon: gen_s.icon,
                default_note: gen_s.default_note,
                color_rgb: gen_s.color_rgb,
                waveform,
                file_path: path_str,
            });
        }
    }
    eprintln!("🧪 DSP klart på {:.1}s", scan_start.elapsed().as_secs_f32());

    // 2. Scan Sample_Packs directory
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let pack_dirs = [
        format!("{}/Music/Sonix/Sample_Packs", home),
        format!("{}/Music/Sonix/User_Samples", home),
    ];

    for p_dir in &pack_dirs {
        let path = Path::new(p_dir);
        if path.exists() {
            eprintln!("🧪 Skannar mapp: {}", p_dir);
            scan_dir_recursive(path, &mut results);
            eprintln!("🧪 Klar mapp ({:.1}s, totalt {} samplar)", scan_start.elapsed().as_secs_f32(), results.len());
        }
    }

    eprintln!(
        "🎵 Ljudbibliotek skannat: {} samplar på {:.1} s",
        results.len(),
        scan_start.elapsed().as_secs_f32()
    );

    results
}

fn scan_dir_recursive(dir: &Path, results: &mut Vec<ScannedSampleItem>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            // Avoid git directory
            if p.file_name().map(|n| n == ".git").unwrap_or(false) {
                continue;
            }
            scan_dir_recursive(&p, results);
        } else if p.is_file() {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if ext == "wav" {
                if let Some(item) = classify_and_parse_wav_sample(&p) {
                    results.push(item);
                }
            }
        }
    }
}

fn classify_and_parse_wav_sample(path: &Path) -> Option<ScannedSampleItem> {
    let path_str = path.to_string_lossy().to_string();
    let file_name = path.file_stem()?.to_string_lossy().to_string();
    let lower_path = path_str.to_lowercase();

    // Ignore tiny metadata files or temp files
    if file_name.starts_with('.') || file_name.contains("metadata") {
        return None;
    }

    let (category, icon, color_rgb, default_note) = if lower_path.contains("tr-808") || lower_path.contains("tr808") {
        ("💥 Roland TR-808", "🥁", (255, 140, 25), 36)
    } else if lower_path.contains("tr-909") || lower_path.contains("tr909") {
        ("⚡ Roland TR-909", "🥁", (255, 200, 50), 36)
    } else if lower_path.contains("tr-707") || lower_path.contains("lm-2") || lower_path.contains("casio") || lower_path.contains("drumtraks") || lower_path.contains("cr-8000") {
        ("📼 80s Drum Machines", "📼", (255, 100, 180), 38)
    } else if lower_path.contains("acoustic_kit") || lower_path.contains("crabacus") {
        ("🥁 Akustiskt Studiokit", "🥁", (80, 220, 255), 38)
    } else if lower_path.contains("rhodes") || lower_path.contains("wurlitzer") || lower_path.contains("cp80") || lower_path.contains("pianet") {
        ("🎹 Rhodes & Elpianon", "🎹", (255, 190, 80), 60)
    } else if lower_path.contains("mellotron") {
        ("🎻 Mellotron & Kör/Stråkar", "🎻", (180, 140, 255), 60)
    } else if lower_path.contains("grand-piano") || lower_path.contains("oldpiano") || lower_path.contains("piano") {
        ("🎹 Flygel & Akustiskt Piano", "🎹", (255, 220, 140), 60)
    } else if lower_path.contains("dsmolken") || lower_path.contains("meatbass") || lower_path.contains("swagbass") || lower_path.contains("double-bass") {
        ("🎸 Bas & Elbas", "🎸", (255, 80, 140), 36)
    } else if lower_path.contains("tx81z") || lower_path.contains("vcsl") {
        ("✨ TX81Z & Syntar", "✨", (100, 220, 255), 60)
    } else if lower_path.contains("factory_samples") {
        ("🎵 Sonix Factory", "🎵", (120, 200, 240), 60)
    } else {
        ("📂 Egna Samples", "⭐", (80, 240, 160), 60)
    };

    // Format clean display name
    let clean_name = format_clean_sample_name(&file_name, &lower_path);

    // Fast peak envelope
    let waveform = match super::wav_reader::read_wav_envelope(&path_str, 48) {
        Ok(peaks) => peaks,
        Err(_) => vec![0.5; 48],
    };

    Some(ScannedSampleItem {
        name: clean_name,
        category: category.to_string(),
        icon: icon.to_string(),
        default_note,
        color_rgb,
        waveform,
        file_path: path_str,
    })
}

fn format_clean_sample_name(stem: &str, lower_path: &str) -> String {
    let s = stem.replace('_', " ").replace('-', " ");

    if lower_path.contains("tr-808") || lower_path.contains("tr808") {
        if s.starts_with("BD") { return format!("808 Bass Drum ({})", s); }
        if s.starts_with("SD") { return format!("808 Snare Drum ({})", s); }
        if s.starts_with("CH") { return format!("808 Closed Hat ({})", s); }
        if s.starts_with("OH") { return format!("808 Open Hat ({})", s); }
        if s.starts_with("CP") { return format!("808 Handclap ({})", s); }
        if s.starts_with("CB") { return format!("808 Cowbell ({})", s); }
        if s.starts_with("RS") { return format!("808 Rimshot ({})", s); }
        if s.starts_with("CY") { return format!("808 Cymbal ({})", s); }
        if s.starts_with("LT") { return format!("808 Low Tom ({})", s); }
        if s.starts_with("MT") { return format!("808 Mid Tom ({})", s); }
        if s.starts_with("HT") { return format!("808 High Tom ({})", s); }
        if s.starts_with("MA") { return format!("808 Maracas ({})", s); }
        if s.starts_with("CL") { return format!("808 Claves ({})", s); }
    }

    if lower_path.contains("acoustic_kit") {
        return format!("Acoustic {}", s);
    }

    if lower_path.contains("rhodes") {
        return format!("Rhodes MKI {}", s);
    }
    if lower_path.contains("wurlitzer") {
        return format!("Wurlitzer EP {}", s);
    }
    if lower_path.contains("mellotron") {
        return format!("Mellotron {}", s);
    }

    s
}
