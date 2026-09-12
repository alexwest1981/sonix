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
    /// Grov översikt (512 punkter, `max(abs)`). Behålls för bakåtkompatibilitet —
    /// stämvyns nya väg använder `waveform_pairs` i stället.
    pub waveform_data: Vec<f32>,
    /// Exakt (min, max) per kolumn ur stämmanS EGNA SAMPLAR (8.3).
    ///
    /// Skillnaden mot `waveform_data` är hela poängen: `visual_peaks_from` trycker
    /// ihop filen till högst 512 punkter och behåller bara `max(abs)` — alltså ett
    /// symmetriskt hölje utan tecken. Här räknas både topp och botten per kolumn,
    /// en gång, vid separationen (några millisekunder för fyra minuter), så vyn
    /// kan rita den sanna formen utan att röra samplen per bildruta.
    pub waveform_pairs: Vec<(f32, f32)>,
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
    pub model_name: String,
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
    /// Stämfilerna på disk, i stämmornas ordning (Fas 8.5a steg 2).
    ///
    /// Fylls av [`StemProject::install_separation`] när stämmorna skrivs. Tom =
    /// ingen skrivning har lyckats, och då ska ingen region peka på en fil som
    /// inte finns.
    pub stem_files: Vec<String>,
}

impl Default for StemProject {
    fn default() -> Self {
        Self {
            track_title: crate::i18n::t("Ingen fil vald").to_string(),
            duration_seconds: 0.0,
            bpm: 126.0,
            model_name: "Sonix Spectral Separator (DSP)".to_string(),
            is_separating: false,
            progress: 0.0,
            stems: Vec::new(),
            selected_sample_idx: 0,
            stem_audio: Vec::new(),
            stem_audio_base: Vec::new(),
            sample_rate: 44100,
            source_path: None,
            stem_files: Vec::new(),
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

/// Result of a full separation run, independent of which backend produced it.
#[derive(Debug, Clone)]
pub struct SeparationResult {
    pub stems: Vec<StemAudio>,
    pub sample_rate: u32,
    pub bpm: f32,
    pub duration_seconds: f32,
    pub model_name: String,
    pub used_neural: bool,
    /// Why the neural backend was skipped, when it was (for honest reporting).
    pub fallback_reason: Option<String>,
}

/// Runs the best available separator: a real neural HTDemucs model when one is
/// installed (and the `neural` feature is compiled in), otherwise the built-in
/// spectral DSP separator. `progress` is called with `0.0..=1.0`.
pub fn run_separation(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    progress: &dyn Fn(f32),
) -> SeparationResult {
    let sr_f = sample_rate.max(1) as f32;
    let duration_seconds = left.len().min(right.len()) as f32 / sr_f;
    let bpm = estimate_bpm(left, right, sr_f);

    let mut fallback_reason = None;
    let neural = if crate::audio::neural_separator::is_available() {
        match crate::audio::neural_separator::separate_neural(left, right, sample_rate, progress) {
            Ok(stems) => Some(stems),
            Err(err) => {
                fallback_reason = Some(err);
                None
            }
        }
    } else {
        fallback_reason = Some(crate::i18n::t(
            "Ingen neural modell hittades — använder inbyggd DSP-separation",
        )
        .to_string());
        None
    };

    let (stems, model_name, used_neural) = match neural {
        Some(stems) => (stems, "HTDemucs (ONNX Neural)".to_string(), true),
        None => {
            progress(0.0);
            let stems = separate_stems(left, right, sr_f);
            progress(1.0);
            (stems, "Sonix Spectral Separator (DSP)".to_string(), false)
        }
    };

    SeparationResult {
        stems,
        sample_rate,
        bpm,
        duration_seconds,
        model_name,
        used_neural,
        fallback_reason,
    }
}

/// Filnamnen stämmorna får på disk, i samma ordning som [`separate_stems`]
/// lämnar dem (och som [`StemProject::install_separation`] mappar dem till
/// [`StemType`]).
pub const STEM_FILE_NAMES: [&str; 4] = ["vocals", "drums", "bass", "instruments"];

/// Skriver stämmorna som 32-bitars flyttals-WAV i `dir` och lämnar tillbaka
/// sökvägarna i stämmornas ordning.
///
/// **Varför stämmorna måste bli filer.** En stämma som bara finns i minnet blir
/// ett klipp vars `source_path` pekar på **originalet** — och kommer originalet
/// från en mp3, som appen inte kan avkoda, blir klippet tyst men ritas ändå ur
/// sin grova översikt. Precis den kombinationen (trovärdig vågform, inget ljud)
/// var det Alex såg. Med stämmorna som egna filer pekar klippet på sitt eget ljud,
/// och en omladdning av projektet spelar dem igen.
///
/// Filnamnet byggs av `base` genom [`crate::autosave::slug`], så ett snedstreck
/// eller `..` i en fil- eller låttitel kan inte lämna `dir`.
pub fn write_stems_to_dir(
    dir: &std::path::Path,
    base: &str,
    stems: &[StemAudio],
    sample_rate: u32,
) -> Result<Vec<std::path::PathBuf>, String> {
    if stems.is_empty() {
        return Err(crate::i18n::t("Inga stämmor att skriva till disk").to_string());
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| crate::tstatus!("Kunde inte skapa '{}': {}", dir.display(), e))?;
    let slug = crate::autosave::slug(base);
    let mut paths = Vec::with_capacity(stems.len());
    for (i, stem) in stems.iter().enumerate() {
        let name = STEM_FILE_NAMES
            .get(i)
            .copied()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("stem{}", i + 1));
        let path = dir.join(format!("{}-{}.wav", slug, name));
        // En väg för att skriva en stämfil, inte två: `write_stem_wav` skriver
        // 32-bitars flyttal (ett mellansteg ska inte kvantisera en enda gång
        // innan den riktiga exporten gör det) och är samma funktion som
        // separatören använder.
        crate::audio::exporter::write_stem_wav(
            path.to_string_lossy().as_ref(),
            &stem.left,
            &stem.right,
            sample_rate,
        )?;
        paths.push(path);
    }
    Ok(paths)
}

/// Skriver stämmorna och läser tillbaka **deras vågform ur filerna**.
///
/// Steget finns för att klippet ska byggas av det som faktiskt ligger på disk:
/// stämfilen skrivs, läses tillbaka, och vågformen som ritas är filens egen.
/// En trasig eller tom skrivning blir ett fel här — inte ett klipp som ser ut
/// att ha ljud men är tyst.
///
/// Lämnar `(sökväg, vågform)` i stämmornas ordning.
pub fn write_stems_and_read_envelopes(
    dir: &std::path::Path,
    base: &str,
    stems: &[StemAudio],
    sample_rate: u32,
    points: usize,
) -> Result<Vec<(std::path::PathBuf, Vec<f32>)>, String> {
    let paths = write_stems_to_dir(dir, base, stems, sample_rate)?;
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let shown = path.to_string_lossy().into_owned();
        let envelope = super::read_wav_envelope(&shown, points).map_err(|e| {
            crate::tstatus!("Kunde inte läsa tillbaka stämfilen '{}': {}", shown, e)
        })?;
        if envelope.is_empty() {
            return Err(crate::tstatus!("Stämfilen '{}' är tom", shown));
        }
        out.push((path, envelope));
    }
    Ok(out)
}

impl StemProject {
    /// Installerar en färdig separation och skriver stämmorna till disk.
    ///
    /// `stems_dir` och `base` kommer från anroparen (appen), så att en separation
    /// och en efterföljande export hamnar på **samma filnamn och samma plats** —
    /// då kan regionen peka på en fil som redan finns.
    ///
    /// `Err` betyder att skrivningen misslyckades, inte att separationen gjorde
    /// det: stämmorna finns kvar i minnet och går att spela, men de finns inte på
    /// disk. Anroparen ansvarar för att säga det högt — en tyst misslyckad
    /// skrivning är precis den sortens tystnad 8.5 stänger.
    pub fn install_separation(
        &mut self,
        result: SeparationResult,
        source_path: &str,
        stems_dir: &std::path::Path,
        base: &str,
    ) -> Result<(), String> {
        self.stem_files.clear();
        self.sample_rate = result.sample_rate;
        self.source_path = Some(source_path.to_string());
        self.track_title = std::path::Path::new(source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(source_path)
            .to_string();
        self.duration_seconds = result.duration_seconds;
        self.bpm = result.bpm;
        self.model_name = result.model_name;
        self.stem_audio = result.stems;
        self.stem_audio_base = self.stem_audio.clone();
        self.is_separating = false;
        self.progress = 1.0;
        self.stems.clear();

        let types = [StemType::Vocals, StemType::Drums, StemType::Bass, StemType::Instruments];
        let volumes = [0.90, 0.95, 0.88, 0.82];
        for (idx, stem_type) in types.iter().enumerate() {
            if idx >= self.stem_audio.len() {
                break;
            }
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
                // 2048 kolumner är tätare än någon skärmbredd för en 70 px-remsa
                // (2,5 kolumner per bildpunkt vid 800 px), och varje sample räknas.
                waveform_pairs: super::waveform::envelope_per_pixel(&mono, 2048),
            });
        }

        // Stämmorna skrivs till disk (8.5a steg 2). Tidigare lämnade en separation
        // antingen inga filer alls eller filer vars sökvägar **kastades**, och ett
        // misslyckande svaldes av `let _ =` — alltså samma tystnad som 8.5 stänger,
        // fast i skrivriktningen. Nu skrivs filerna en gång, sökvägarna sparas, och
        // ett fel går tillbaka till anroparen.
        //
        // Kostnaden: skrivningen sker på den tråd som anropar (UI-tråden, när
        // separationens resultat hämtas i `update`). Fyra 32-bitars stämmor av en
        // fyra minuters låt är några hundra MB. Att flytta skrivningen till
        // bakgrundstråden är nästa steg — det står i ROADMAP, inte här som en tyst
        // fördröjning.
        let written = write_stems_to_dir(stems_dir, base, &self.stem_audio, self.sample_rate);
        self.stem_files = written?
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        Ok(())
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

    /// Stämvyn ritade ett symmetriskt hölje ur en 512-punktsöversikt förut. Nu
    /// bär varje stämma exakta (min, max)-par ur sina egna samplar.
    ///
    /// Här prövas att paren FYLLS med riktigt innehåll. Att de bevarar osymmetri
    /// prövas i `waveform::tests`, på `envelope_per_pixel` — samma funktion den här
    /// vägen använder. Ett försök att pröva det här med en negativ halvvåg föll:
    /// stämmorna banddelas, och en banddelning tar bort just en sådan förskjutning.
    /// Testet hade fel, inte koden.
    #[test]
    fn stems_carry_exact_pairs_from_their_own_samples() {
        let sr = 44100.0f32;
        let n = 44100;
        // Bredbandigt och deterministiskt (LCG) — når alla fyra banden, till
        // skillnad från en enskild ton som bara hamnar i ett av dem.
        let mut seed: u32 = 12345;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / 8_388_608.0 - 1.0
        };
        let left: Vec<f32> = (0..n).map(|_| next() * 0.7).collect();
        let right: Vec<f32> = (0..n).map(|_| next() * 0.7).collect();

        let result = run_separation(&left, &right, sr as u32, &|_| {});
        let mut project = StemProject::default();
        let dir = std::env::temp_dir().join(format!("sonix_stem_pairs_{}", std::process::id()));
        // Skrivningen får inte fälla testet om katalogen inte går att skapa — det
        // som prövas här är paren, och det egna testet nedan prövar skrivningen.
        let _ = project.install_separation(result, "/tmp/prov.wav", &dir, "prov");
        let _ = std::fs::remove_dir_all(&dir);

        assert!(!project.stems.is_empty(), "separationen ska ge stämmor");
        for stem in &project.stems {
            assert!(
                !stem.waveform_pairs.is_empty(),
                "varje stämma ska bära exakta par"
            );
        }
        let loudest = project
            .stems
            .iter()
            .flat_map(|s| s.waveform_pairs.iter())
            .fold(0.0f32, |acc, (lo, hi)| acc.max(hi.abs()).max(lo.abs()));
        assert!(
            loudest > 0.01,
            "bredbandig insignal ska ge riktigt innehåll i något band, fick {loudest}"
        );
    }

    /// **Fas 8.5a steg 2:** en separation skriver sina stämmor till disk **och
    /// minns var de ligger**, och filen går att läsa tillbaka med appens egen
    /// avkodare — samma längd och samma ljud som stämman i minnet. Det är hela
    /// poängen: ett klipp som pekar på en fil som finns kan inte bli tyst.
    #[test]
    fn a_separation_writes_its_stems_and_remembers_where() {
        let sr = 44100u32;
        let n = 8_000;
        let left: Vec<f32> = (0..n).map(|i| (i as f32 / n as f32) * 0.6 - 0.3).collect();
        let right: Vec<f32> = left.iter().map(|s| s * 0.5).collect();
        let result = run_separation(&left, &right, sr, &|_| {});

        let dir = std::env::temp_dir().join(format!("sonix_stem_write_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut project = StemProject::default();
        project
            .install_separation(result, "/tmp/Min Låt.mp3", &dir, "Min Låt")
            .expect("skrivningen ska lyckas i en tom katalog");

        assert_eq!(
            project.stem_files.len(),
            project.stem_audio.len(),
            "en sökväg per stämma"
        );
        for path in &project.stem_files {
            assert!(
                std::path::Path::new(path).is_file(),
                "stämfilen ska finnas på disk: {path}"
            );
        }
        // Läs tillbaka den första stämman och jämför med stämman i minnet.
        let (read_l, read_r, read_sr) = crate::audio::wav_reader::load_wav_pcm(&project.stem_files[0])
            .expect("den skrivna filen ska gå att läsa");
        assert_eq!(read_sr, project.sample_rate, "samma samplerate");
        assert_eq!(
            read_l.len(),
            project.stem_audio[0].left.len(),
            "samma längd som stämman i minnet"
        );
        let stem = &project.stem_audio[0];
        let mid = read_l.len() / 2;
        assert!(
            (read_l[mid] - stem.left[mid]).abs() < 1e-4 && (read_r[mid] - stem.right[mid]).abs() < 1e-4,
            "samma ljud: ({}, {}) mot ({}, {})",
            read_l[mid],
            read_r[mid],
            stem.left[mid],
            stem.right[mid]
        );
        // Osymmetri ska bevaras — kanalerna får inte blandas ihop.
        assert!(
            (read_l[mid] - read_r[mid]).abs() > 1e-3,
            "vänster och höger ska vara olika, som de var"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En skrivning som inte går (målet är en **fil**, inte en katalog) ska komma
    /// tillbaka som ett fel — och stämmorna ska ändå finnas kvar i minnet, för
    /// separationen lyckades. Tystnaden var problemet, inte att disken är full.
    #[test]
    fn a_failed_write_is_reported_and_the_stems_stay_playable() {
        let sr = 44100u32;
        let n = 2_000;
        let left = vec![0.25f32; n];
        let right = vec![0.1f32; n];
        let result = run_separation(&left, &right, sr, &|_| {});

        // En fil där katalogen ska ligga: create_dir_all kan inte lyckas.
        let blocked = std::env::temp_dir().join(format!("sonix_stem_block_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"inte en katalog").expect("testfilen ska gå att skriva");

        let mut project = StemProject::default();
        let err = project
            .install_separation(result, "/tmp/Tyst.mp3", &blocked, "Tyst")
            .expect_err("skrivningen ska rapportera ett fel");
        assert!(!err.is_empty(), "felet ska förklara sig: {err}");
        assert!(
            project.stem_files.is_empty(),
            "inga sökvägar ska lovas när skrivningen misslyckades"
        );
        assert!(
            !project.stems.is_empty() && !project.stem_audio.is_empty(),
            "stämmorna ska finnas kvar i minnet och gå att spela"
        );
        let _ = std::fs::remove_file(&blocked);
    }

    #[test]
    fn run_separation_falls_back_to_dsp_without_a_model() {
        // With no ONNX model installed the neural backend is unavailable, so the
        // separator must still produce four finite stems and report DSP.
        if crate::audio::neural_available() {
            return;
        }
        let sr = 44100u32;
        let n = 44100usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let v = (t * 220.0 * std::f32::consts::TAU).sin() * 0.4
                + (t * 3000.0 * std::f32::consts::TAU).sin() * 0.1;
            l[i] = v;
            r[i] = v;
        }

        let saw_progress = std::sync::atomic::AtomicBool::new(false);
        let result = run_separation(&l, &r, sr, &|p| {
            if p > 0.5 {
                saw_progress.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });

        assert_eq!(result.stems.len(), 4);
        assert!(!result.used_neural);
        assert_eq!(result.model_name, "Sonix Spectral Separator (DSP)");
        assert!(result.fallback_reason.is_some());
        assert!((result.duration_seconds - 1.0).abs() < 0.01);
        assert!(saw_progress.load(std::sync::atomic::Ordering::Relaxed));
        for stem in &result.stems {
            assert_eq!(stem.left.len(), n);
            assert!(stem.left.iter().all(|v| v.is_finite()));
        }
    }

    /// En hörbar ton i `seconds` sekunder — används för att en stämma som skrivs
    /// till disk ska ha något att mäta i filen efteråt.
    fn tone(freq: f32, seconds: f32) -> Vec<f32> {
        let sr = 44100.0f32;
        let n = (sr * seconds) as usize;
        (0..n)
            .map(|i| (i as f32 / sr * freq * std::f32::consts::TAU).sin() * 0.8)
            .collect()
    }

    fn test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonix_stems_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn four_tone_stems() -> Vec<StemAudio> {
        [220.0, 880.0, 110.0, 1320.0]
            .iter()
            .map(|f| StemAudio { left: tone(*f, 0.2), right: tone(*f, 0.2) })
            .collect()
    }

    /// Det Alex såg: en stämma som bara finns i minnet blir ett klipp som pekar
    /// på originalet och är tyst. Skrivs stämmorna som filer ska filen gå att
    /// läsa tillbaka med **hela** ljudet i sig — inte bara finnas.
    #[test]
    fn stems_are_written_as_files_that_can_be_read_back() {
        let dir = test_dir("roundtrip");
        let sr = 44100u32;
        let stems = four_tone_stems();
        let paths = write_stems_to_dir(&dir, "Cyberpunk Odyssey (Suno AI)", &stems, sr)
            .expect("stämmorna ska gå att skriva");

        assert_eq!(paths.len(), 4);
        let mut names = std::collections::HashSet::new();
        for (path, stem) in paths.iter().zip(stems.iter()) {
            assert!(path.is_file(), "filen ska finnas: {}", path.display());
            assert!(
                names.insert(path.file_name().unwrap().to_string_lossy().to_string()),
                "två stämmor fick samma filnamn"
            );
            let (l, r, read_sr) = super::super::load_wav_pcm(&path.to_string_lossy())
                .expect("filen ska gå att läsa tillbaka");
            assert_eq!(read_sr, sr);
            assert_eq!(l.len(), stem.left.len(), "hela ljudet ska finnas i filen");
            assert_eq!(r.len(), stem.right.len());
            let peak = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.5, "stämman ska ha ljud i filen, inte bara en fil");
        }
        // Exakt fyra filer: ingen temp-fil eller delskrivning kvar i katalogen.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Filnamnet kommer från en fil- eller låttitel, alltså från användarens disk.
    /// Ett snedstreck eller `..` får inte kunna lägga stämmor utanför katalogen.
    #[test]
    fn a_title_with_a_slash_cannot_escape_the_stem_directory() {
        let dir = test_dir("escape");
        let paths = write_stems_to_dir(&dir, "../../etc/Min Låt", &four_tone_stems(), 44100)
            .expect("stämmorna ska gå att skriva");
        for path in &paths {
            assert_eq!(
                path.parent().unwrap(),
                dir.as_path(),
                "stämman hamnade utanför katalogen: {}",
                path.display()
            );
            assert!(!path.to_string_lossy().contains(".."));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Att skriva noll stämmor ska vara ett fel, inte en tyst tom uppsättning
    /// filer som anroparen tror är fyra.
    #[test]
    fn writing_no_stems_is_an_error() {
        let dir = test_dir("empty");
        let err = write_stems_to_dir(&dir, "Tom", &[], 44100).unwrap_err();
        assert!(!err.is_empty());
        assert!(!dir.exists(), "ingen katalog ska skapas för noll stämmor");
    }

    /// Vågformen klippet ritar ska komma ur **filen**, inte ur den grova
    /// översikten i minnet. Testet läser tillbaka och mäter: fyra vågformer som
    /// bär stämmornas ljud, i stämmornas ordning.
    #[test]
    fn the_waveform_comes_from_the_written_file() {
        let dir = test_dir("envelope");
        let written = write_stems_and_read_envelopes(
            &dir,
            "Min Låt",
            &four_tone_stems(),
            44100,
            64,
        )
        .expect("skrivning och återläsning ska gå igenom");

        assert_eq!(written.len(), 4);
        for (i, (path, envelope)) in written.iter().enumerate() {
            assert!(path.is_file(), "filen ska finnas: {}", path.display());
            assert_eq!(envelope.len(), 64, "en punkt per begärd punkt");
            assert!(envelope.iter().all(|v| v.is_finite()));
            let mean: f32 = envelope.iter().sum::<f32>() / envelope.len() as f32;
            assert!(
                mean > 0.3,
                "stämma {i}: vågformen är (nästan) tom ({mean}) — en tyst fil hade ritats som ljud"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En katalog som inte går att skapa ska bli ett fel. Annars hade anroparen
    /// fått fyra regioner som pekar på filer som inte finns — den tysta klassen
    /// som 8.5 stänger.
    #[test]
    fn a_directory_that_cannot_be_created_is_an_error() {
        let blocked = test_dir("blocked");
        std::fs::write(&blocked, b"en fil, inte en katalog").expect("tmpfil");
        let err =
            write_stems_to_dir(&blocked, "Min Låt", &four_tone_stems(), 44100).unwrap_err();
        assert!(!err.is_empty());
        let _ = std::fs::remove_file(&blocked);
    }
}
