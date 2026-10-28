//! Neural stem separation through ONNX models (Demucs, BS-RoFormer, Mel-Band RoFormer, SCNet).
//!
//! The actual inference is provided by the optional [`ort`] runtime and is only
//! compiled when the crate is built with `--features neural`. When no model is
//! present — or the feature is disabled — [`crate::audio::stem_separator`] falls
//! back to its genuine (but non-neural) spectral DSP separator, and the UI says
//! so honestly.
//!
//! Model discovery (in order):
//!   1. `$SONIX_DEMUCS_ONNX` — explicit path to an `.onnx` file (4-stem Demucs only).
//!   2. `$XDG_CONFIG_HOME/sonix/models/` (default `~/.config/sonix/models/`).
//!   3. `$XDG_DATA_HOME/sonix/models/` (default `~/.local/share/sonix/models/`).
//!
//! **Två sorters modeller.** Demucs-familjen tar vågform `[1,2,T]` in och lämnar `[1,S,2,T]`.
//! RoFormer-familjen och SCNet är exporterade *mellan* sin STFT och iSTFT: de tar
//! spektrogram-koefficienter och lämnar maskerade sådana, och fönstringen görs här
//! (`crate::audio::stft`). Layouten per export står i [`SpecLayout`], parametrarna i
//! [`ModelSpec::stft`] och katalogen i [`MODELS`].

use std::path::PathBuf;

use super::stem_separator::StemAudio;
use super::stft::StftParams;

/// Sample rate every model in the catalogue was trained on.
pub const MODEL_SAMPLE_RATE: u32 = 44_100;
/// Hur långt Demucs-segmentet är: HTDemucs har fasta positionella embeddings, så ett block är 7,8 s.
pub const DEFAULT_SEGMENT_SECONDS: f32 = 7.8;
/// Crossfade overlap between Demucs segments.
pub const DEFAULT_OVERLAP_SECONDS: f32 = 1.0;
/// Stereo.
pub const NUM_CHANNELS: usize = 2;
/// Hur många stämplatser en körning kan fylla: sång, trummor, bas, instrument, gitarr, piano.
pub const STEM_SLOTS: usize = 6;
/// Bytes per megabyte, för katalogens storlekar.
const MB: u64 = 1024 * 1024;

/// Hur modellen tar sitt ljud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// Vågform in och ut: `mix (1,2,T)` → `stems (1,S,2,T)` (Demucs-familjen).
    Waveform,
    /// Spektrogram in och ut; STFT/iSTFT görs i appen.
    Spectrogram,
}

/// Hur spektrogrammet packas i tensorerna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecLayout {
    /// Två tensorer `spec_real`/`spec_imag`, `[1,2,bins,T]` in; `out_spec_real`/`out_spec_imag`
    /// `[1,S,2,bins,T]` ut.
    RealImag,
    /// En packad `mix_spec` `[1,4,bins,T]` (L-re, L-im, R-re, R-im) in; `stems_spec`
    /// `[1,S*4,bins,T]` ut med samma packning per stämma.
    PackedChannels,
    /// En packad `mix_spec` in; samma fyra plan ut — modellen ger **en** stämma (sång).
    PackedSingle,
}

/// En modell i katalogen: var den hämtas, vad den ger, och hur den ska matas.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    /// Stabilt id (används i UI:t och i tester).
    pub id: &'static str,
    pub label: &'static str,
    /// Kvalitet, storlek och licens — det man vill veta innan man hämtar 350 MB.
    pub note: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    /// Källordningen i modellens utdata, med de namn vi mappar till stämmor.
    pub sources: &'static [&'static str],
    pub format: ModelFormat,
    /// Segmentets längd i sekunder (vågform).
    pub segment_secs: f32,
    /// Antal STFT-frames modellen är tracad med (spektrogram): en export har fast `T`.
    pub frames: usize,
    /// Överlapp mellan blocken, 0–1.
    pub overlap: f32,
    /// STFT-parametrar (spektrogram).
    pub stft: Option<StftParams>,
    pub layout: SpecLayout,
    /// Förväntad storlek i byte, så att en avbruten hämtning inte kan se ut som en modell.
    pub expected_bytes: u64,
}

/// **Katalogen.** Fyra familjer, sex modeller. SDR-talen i `note` är publicerade värden ur
/// modellkorten, inte skattningar — men de är mätta på olika dataset (MUSDB18-HQ, MoisesDB,
/// Multisong), så de är inte rad för rad jämförbara. Gitarr/piano-siffror finns bara för
/// Demucs 6s (MoisesDB) och BS-RoFormer-SW (MVSEP Multisong).
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "htdemucs",
        label: "HTDemucs (4 stämmor)",
        note: "MUSDB18-HQ 9,0 dB medel. 166 MB, MIT. Snabbast — standard.",
        file: "htdemucs.onnx",
        url: MODEL_URL,
        sources: &["drums", "bass", "other", "vocals"],
        format: ModelFormat::Waveform,
        segment_secs: DEFAULT_SEGMENT_SECONDS,
        frames: 0,
        overlap: 0.13,
        stft: None,
        layout: SpecLayout::RealImag,
        expected_bytes: 166 * MB,
    },
    ModelSpec {
        id: "htdemucs_6s",
        label: "HTDemucs 6s (6 stämmor)",
        note: "Gitarr 3,1 dB, piano 1,6 dB (MoisesDB) — svag på just de två. 136 MB, MIT.",
        file: "htdemucs_6s.onnx",
        url: "https://huggingface.co/StemSplitio/htdemucs-6s-onnx/resolve/main/htdemucs_6s_fp16weights.onnx",
        sources: &["drums", "bass", "other", "vocals", "guitar", "piano"],
        format: ModelFormat::Waveform,
        segment_secs: DEFAULT_SEGMENT_SECONDS,
        frames: 0,
        overlap: 0.13,
        stft: None,
        layout: SpecLayout::RealImag,
        expected_bytes: 136 * MB,
    },
    ModelSpec {
        id: "bs_roformer_sw6",
        label: "BS-RoFormer-SW 6 stämmor",
        note: "Gitarr 9,1 dB, piano 7,8 dB — bäst på sex. 353 MB. Kod MIT, men vikterna är re-hostade utan licens.",
        file: "bs_roformer_sw_6stem_fp16.onnx",
        url: "https://huggingface.co/Acapellas/bs-roformer-sw-6stem-FP16-FP32-onnx/resolve/main/bs_roformer_sw_6stem_fp16.onnx",
        sources: &["bass", "drums", "other", "vocals", "guitar", "piano"],
        format: ModelFormat::Spectrogram,
        segment_secs: 0.0,
        frames: 345,
        overlap: 0.25,
        stft: Some(StftParams {
            n_fft: 2048,
            hop: 512,
            hann: true,
            center: true,
            scale: 1.0,
        }),
        layout: SpecLayout::RealImag,
        expected_bytes: 353 * MB,
    },
    ModelSpec {
        id: "mel_roformer",
        label: "Mel-Band RoFormer (sång + rest)",
        note: "Sång 12,1 dB (MUSDB18-HQ) — bäst på sång. 464 MB, MIT.",
        file: "mel-roformer.fp16.onnx",
        url: "https://huggingface.co/audiojs/mel-roformer/resolve/main/mel-roformer.fp16.onnx",
        sources: &["vocals"],
        format: ModelFormat::Spectrogram,
        segment_secs: 0.0,
        frames: 801,
        overlap: 0.5,
        stft: Some(StftParams {
            n_fft: 2048,
            hop: 441,
            hann: true,
            center: true,
            scale: 1.0,
        }),
        layout: SpecLayout::PackedSingle,
        expected_bytes: 464 * MB,
    },
    ModelSpec {
        id: "scnet",
        label: "SCNet (4 stämmor, 13 MB)",
        note: "MUSDB18-HQ 9,0 dB. 13 MB, MIT — minst och snabbast av de neurala.",
        file: "scnet.int8.onnx",
        url: "https://huggingface.co/audiojs/scnet/resolve/main/scnet.int8.onnx",
        sources: &["drums", "bass", "other", "vocals"],
        format: ModelFormat::Spectrogram,
        segment_secs: 0.0,
        frames: 476,
        overlap: 0.75,
        stft: Some(StftParams {
            n_fft: 4096,
            hop: 1024,
            hann: false,
            center: true,
            scale: 1.0 / 64.0,
        }),
        layout: SpecLayout::PackedChannels,
        expected_bytes: 13 * MB,
    },
];

/// Var 4-stämmors Demucs hämtas (den som står först i katalogen).
///
/// **HTDemucs singelfil med fp16-lagrade vikter** (StemSplitios ONNX-export av
/// `facebookresearch/demucs`, MIT). Valet är gjort på tre mätvärden ur deras model card:
///
/// - 166 MB i stället för 316 MB (fp32-vikter), med max avvikelse **4,6 × 10⁻⁵** mot fp32 —
///   alltså hörbart identisk, och samma minne och tid vid körning (vikterna räknas i fp32 ändå).
/// - **En** session i stället för fyra: `htdemucs_ft`-säcken har bättre SDR (9,19 mot 8,8 dB
///   sång) men är 1,26 GB och fyra modeller — den passar inte vår inferens, som laddar en session
///   och läser ett enda tensor.
/// - Kontraktet är exakt vårt: `mix (1, 2, 343980)` in, `stems (1, 4, 2, 343980)` ut, ordning
///   `drums, bass, other, vocals`, 44,1 kHz, 7,8 s segment.
///
/// Modellen ligger **inte** i installationsfilen: den som vill köra neural separering hämtar
/// den, alla andra slipper 166 MB i skeppningslasten.
///
/// Attribution (MIT): Rouard, Massa & Défossez, *Hybrid Transformers for Music Source
/// Separation*, ICASSP 2023. ONNX-export och paritetskontroll av StemSplit.
pub const MODEL_URL: &str =
    "https://huggingface.co/StemSplitio/htdemucs-onnx/resolve/main/htdemucs_fp16weights.onnx";

/// Katalogen som lista (för UI:t).
pub fn models() -> &'static [ModelSpec] {
    MODELS
}

/// Slår upp en modell på id.
pub fn model_by_id(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

/// Standardmodellen: den som funnits längst och är snabbast.
pub fn default_model() -> &'static ModelSpec {
    &MODELS[0]
}

/// **Vilken stämplats en källa hör till.** Ordningen är appens egen (sång, trummor, bas,
/// instrument, gitarr, piano), oavsett i vilken ordning modellen skriver sina källor —
/// BS-RoFormer-SW börjar med basen, Demucs med trummorna.
pub fn stem_slot(source: &str) -> usize {
    match source {
        "vocals" => 0,
        "drums" => 1,
        "bass" => 2,
        "guitar" => 4,
        "piano" => 5,
        _ => 3, // other → instrument
    }
}

/// Environment variable that points directly at the 4-stem Demucs file.
pub const MODEL_ENV: &str = "SONIX_DEMUCS_ONNX";

/// File names accepted inside the model directories for the 4-stem Demucs.
pub const MODEL_FILE_NAMES: [&str; 3] = ["htdemucs.onnx", "demucs.onnx", "htdemucs_ft.onnx"];

/// Candidate directories searched for a model, most specific first:
/// the canonical data dir (`~/.local/share/sonix/models/`) and the older
/// configuration dir (`~/.config/sonix/models/`) which is still read.
pub fn model_dirs() -> Vec<PathBuf> {
    let paths = crate::paths::paths();
    vec![paths.models_dir(), paths.legacy_models_dir()]
}

/// Pure helper: first existing model candidate inside `dirs`.
pub fn find_model_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    find_model_in_with(dirs, &MODEL_FILE_NAMES)
}

pub fn find_model_in_with(dirs: &[PathBuf], names: &[&str]) -> Option<PathBuf> {
    for dir in dirs {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolves the model file for a spec, honouring [`MODEL_ENV`] for the 4-stem Demucs only.
pub fn find_model_for(spec: &'static ModelSpec) -> Option<PathBuf> {
    if spec.id == "htdemucs"
        && let Some(path) = std::env::var_os(MODEL_ENV).map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    // Demucs-filen får heta något av de gamla namnen också.
    let names: &[&str] = if spec.id == "htdemucs" {
        &MODEL_FILE_NAMES
    } else {
        std::slice::from_ref(&spec.file)
    };
    find_model_in_with(&model_dirs(), names)
}

/// **Är modellen hämtad?** En 4-stämmorsfil räknas inte som en 6-stämmorsmodell.
pub fn is_downloaded(spec: &'static ModelSpec) -> bool {
    find_model_for(spec).is_some()
}

pub fn find_model() -> Option<PathBuf> {
    find_model_for(default_model())
}

/// Human-readable status for the UI.
pub fn model_status() -> String {
    match find_model() {
        Some(path) => format!("{}", path.display()),
        None => model_dirs()
            .first()
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "~/.config/sonix/models".to_string()),
    }
}

/// Minsta godtagbara storlek för en hämtad modell: 90 % av den förväntade. Gränsen finns för att
/// en fellänk eller en HTML-sida aldrig ska se ut som en fungerande modell — och för att en
/// 13 MB-modell inte ska kräva 100 MB för att godkännas.
pub fn min_bytes(spec: &ModelSpec) -> u64 {
    spec.expected_bytes / 10 * 9
}

/// Är en hämtad fil komplett? En avbruten nedladdning får aldrig se ut som en fungerande
/// modell — hellre ett tydligt fel än en tyst halv fil som ger brus.
pub fn download_looks_complete(bytes: u64, expected: Option<u64>) -> Result<(), String> {
    match expected {
        Some(want) if bytes == want => Ok(()),
        Some(want) => Err(format!("hämtningen avbröts: {bytes} av {want} byte")),
        // Utan Content-Length finns bara en undre gräns: en riktig modell är aldrig liten.
        None if bytes >= 10 * MB => Ok(()),
        None => Err(format!("filen är bara {bytes} byte — ingen modell")),
    }
}

/// Laddar ner en modell till modellmappen och returnerar sökvägen.
///
/// Skriver till `<mål>.part` och byter namn först när storleken stämmer, så en avbruten
/// hämtning lämnar en `.part` efter sig och aldrig en halv modell med rätt namn.
pub fn download_model_for<F: FnMut(f32)>(
    spec: &'static ModelSpec,
    mut progress: F,
) -> Result<PathBuf, String> {
    let dir = model_dirs()
        .into_iter()
        .next()
        .ok_or_else(|| "ingen modellmapp".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("kunde inte skapa {}: {e}", dir.display()))?;
    let dest = dir.join(spec.file);
    let part = dir.join(format!("{}.part", spec.file));

    let response = ureq::get(spec.url)
        .call()
        .map_err(|e| format!("kunde inte hämta {}: {e}", spec.label))?;
    let expected = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&part)
        .map_err(|e| format!("kunde inte skriva {}: {e}", part.display()))?;
    let mut written: u64 = 0;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| format!("hämtningen bröts: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])
            .map_err(|e| format!("kunde inte skriva modellen: {e}"))?;
        written += n as u64;
        if let Some(want) = expected
            && want > 0
        {
            progress((written as f32 / want as f32).clamp(0.0, 1.0));
        }
    }
    drop(file);
    if let Err(e) = download_looks_complete(written, expected) {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &dest).map_err(|e| format!("kunde inte flytta modellen på plats: {e}"))?;
    progress(1.0);
    Ok(dest)
}

/// Whether a neural run is possible right now (feature enabled + any model found).
pub fn is_available() -> bool {
    cfg!(feature = "neural") && find_model().is_some()
}

pub fn is_available_for(spec: &'static ModelSpec) -> bool {
    cfg!(feature = "neural") && find_model_for(spec).is_some()
}

/// Linear-interpolation stereo resampler. Deliberately simple — it exists to
/// feed the model when the source is not already 44.1 kHz, not as an
/// audiophile-grade converter.
pub fn resample_linear(left: &[f32], right: &[f32], from: u32, to: u32) -> (Vec<f32>, Vec<f32>) {
    let n = left.len().min(right.len());
    if n == 0 || from == 0 || to == 0 || from == to {
        return (left[..n].to_vec(), right[..n].to_vec());
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((n as f64) * ratio).round() as usize;
    let mut out_l = Vec::with_capacity(out_len);
    let mut out_r = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = (src.floor() as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        let frac = (src - i0 as f64) as f32;
        out_l.push(left[i0] + (left[i1] - left[i0]) * frac);
        out_r.push(right[i0] + (right[i1] - right[i0]) * frac);
    }
    (out_l, out_r)
}

/// Start offsets for overlapping segments covering `total` samples. The final
/// segment always reaches the end of the signal.
pub fn plan_segments(total: usize, segment: usize, hop: usize) -> Vec<usize> {
    if total == 0 || segment == 0 {
        return Vec::new();
    }
    let hop = hop.max(1);
    let mut starts = Vec::new();
    let mut pos = 0usize;
    loop {
        starts.push(pos);
        if pos + segment >= total {
            break;
        }
        pos += hop;
    }
    starts
}

/// Triangular crossfade window for a segment. The first and last segments keep
/// a flat outer edge so no samples are attenuated at the signal boundaries.
pub fn segment_window(len: usize, overlap: usize, first: bool, last: bool) -> Vec<f32> {
    let mut window = vec![1.0f32; len];
    let overlap = overlap.min(len / 2);
    if overlap == 0 {
        return window;
    }
    for i in 0..overlap {
        let gain = (i + 1) as f32 / (overlap + 1) as f32;
        if !first {
            window[i] = gain;
        }
        if !last {
            window[len - 1 - i] = gain;
        }
    }
    window
}

/// Extract one `(source, channel)` sample slice from a model output tensor
/// shaped `[B, S, C, T]`, `[B, S*C, T]` or `[S*C, T]`.
pub fn source_channel_slice<'a>(
    shape: &[i64],
    data: &'a [f32],
    source: usize,
    channel: usize,
    channels: usize,
) -> Result<&'a [f32], String> {
    if channels == 0 {
        return Err("kanaler får inte vara 0".to_string());
    }
    let (source_channels, frames) = match shape {
        [b, s, c, t] if *b == 0 || *b == 1 => (*s as usize * *c as usize, *t as usize),
        [b, sc, t] if *b == 0 || *b == 1 => (*sc as usize, *t as usize),
        [sc, t] => (*sc as usize, *t as usize),
        _ => return Err(format!("oväntad utdata-form {shape:?}")),
    };
    if source_channels % channels != 0 {
        return Err(format!(
            "utdata-form {shape:?} stämmer inte med {channels} kanaler"
        ));
    }
    let index = source * channels + channel;
    if index >= source_channels {
        return Err("källindex utanför modellens utdata".to_string());
    }
    let start = index * frames;
    let end = start + frames;
    if end > data.len() {
        return Err("modellens utdata är kortare än formen anger".to_string());
    }
    Ok(&data[start..end])
}

/// En plan i en spektrogram-tensor: `index` räknas i de inledande dimensionerna, de två sista är
/// `(bins, frames)`. Exporten kan ha 0 i en dimension (dynamisk); då räknas planen ur datan.
pub fn spec_plane<'a>(
    shape: &[i64],
    data: &'a [f32],
    index: usize,
    bins: usize,
    frames: usize,
) -> Result<&'a [f32], String> {
    let plane = bins * frames;
    if plane == 0 {
        return Err("planet är tomt".to_string());
    }
    let start = index * plane;
    if start + plane > data.len() {
        return Err(format!(
            "modellens utdata har {} plan om {plane} värden, begärde plan {index} ({shape:?})",
            data.len() / plane
        ));
    }
    Ok(&data[start..start + plane])
}

/// Map a Demucs source index to our stem index
/// (`0` Vocals, `1` Drums, `2` Bass, `3` Instruments, `4` Guitar, `5` Piano).
pub fn demucs_source_to_stem(source: usize) -> usize {
    match source {
        0 => 1, // drums
        1 => 2, // bass
        2 => 3, // other -> instruments
        3 => 0, // vocals
        4 => 4, // guitar
        5 => 5, // piano
        _ => 3,
    }
}

/// Runs the neural separator with the default model. Returns `Err` with a human-readable reason
/// when no model or backend is available; the caller falls back to DSP.
pub fn separate_neural(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    progress: &dyn Fn(f32),
) -> Result<Vec<StemAudio>, String> {
    separate_neural_with(default_model(), left, right, sample_rate, progress)
}

/// Som [`separate_neural`], men med vald modell ur [`MODELS`].
pub fn separate_neural_with(
    spec: &'static ModelSpec,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    progress: &dyn Fn(f32),
) -> Result<Vec<StemAudio>, String> {
    #[cfg(feature = "neural")]
    {
        backend::separate(spec, left, right, sample_rate, progress)
    }
    #[cfg(not(feature = "neural"))]
    {
        let _ = (spec, left, right, sample_rate, progress);
        Err(crate::i18n::t(
            "Neural separation är inte inbyggd i den här builden (bygg med --features neural)",
        )
        .to_string())
    }
}

#[cfg(feature = "neural")]
mod backend {
    use super::*;
    use crate::audio::stft::{istft, stft};
    use ort::session::Session;
    use ort::value::Tensor;

    /// Kör en modell över ett stereopar och lägger ihop blocken med överlapp.
    pub fn separate(
        spec: &'static ModelSpec,
        left: &[f32],
        right: &[f32],
        sample_rate: u32,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<StemAudio>, String> {
        let path = find_model_for(spec).ok_or_else(|| {
            crate::tstatus!(
                "Modellen {} är inte hämtad. Hämta den i Stämmor-vyn ({} MB) — eller lägg filen i {}",
                spec.label,
                spec.expected_bytes / MB,
                model_status()
            )
        })?;

        let (l, r) = resample_linear(left, right, sample_rate, MODEL_SAMPLE_RATE);
        let n = l.len().min(r.len());
        if n == 0 {
            return Err("Ljudet är tomt".to_string());
        }

        let mut session = Session::builder()
            .map_err(|e| format!("Kunde inte skapa ONNX-session: {e}"))?
            .commit_from_file(&path)
            .map_err(|e| format!("Kunde inte ladda modellen {}: {e}", path.display()))?;

        // Global normalisation, matching the Demucs/MSST `demix()` path.
        let (mean, std) = mean_std(&l[..n], &r[..n]);

        match spec.format {
            ModelFormat::Waveform => run_waveform(&mut session, spec, &l, &r, mean, std, n, progress),
            ModelFormat::Spectrogram => {
                run_spectrogram(&mut session, spec, &l, &r, mean, std, n, progress)
            }
        }
    }

    fn mean_std(l: &[f32], r: &[f32]) -> (f32, f32) {
        let mut sum = 0.0f64;
        let mut sumsq = 0.0f64;
        for i in 0..l.len() {
            sum += l[i] as f64 + r[i] as f64;
            sumsq += (l[i] as f64 * l[i] as f64) + (r[i] as f64 * r[i] as f64);
        }
        let count = (l.len() * 2).max(1) as f64;
        let mean = (sum / count) as f32;
        let var = (sumsq / count) as f32 - mean * mean;
        (mean, var.max(1e-8).sqrt())
    }

    /// Samlar blocken i sex stämplatser och lämnar tillbaka dem som modellen fyllde.
    struct Accumulator {
        n: usize,
        acc: Vec<[Vec<f32>; NUM_CHANNELS]>,
        weight: Vec<f32>,
        used: [bool; STEM_SLOTS],
    }

    impl Accumulator {
        fn new(n: usize) -> Self {
            Self {
                n,
                acc: (0..STEM_SLOTS)
                    .map(|_| [vec![0.0; n], vec![0.0; n]])
                    .collect(),
                weight: vec![0.0; n],
                used: [false; STEM_SLOTS],
            }
        }

        /// Lägger ett redan fönstrat och avnormaliserat block på plats `slot`, kanal `channel`.
        fn add_windowed(&mut self, slot: usize, channel: usize, start: usize, piece: &[f32]) {
            if slot >= STEM_SLOTS {
                return;
            }
            self.used[slot] = true;
            let dst = &mut self.acc[slot][channel];
            for (i, &v) in piece.iter().enumerate() {
                if let Some(cell) = dst.get_mut(start + i) {
                    *cell += v;
                }
            }
        }

        /// Markerar platsen som använd utan att lägga något — för en stämma som finns i modellens
        /// utdata men är tyst i det här blocket.
        fn mark(&mut self, slot: usize) {
            if slot < STEM_SLOTS {
                self.used[slot] = true;
            }
        }

        fn finish(mut self) -> Vec<StemAudio> {
            let weight = self.weight;
            let mut stems = Vec::new();
            for slot in 0..STEM_SLOTS {
                if !self.used[slot] {
                    continue;
                }
                let left = std::mem::take(&mut self.acc[slot][0]);
                let right = std::mem::take(&mut self.acc[slot][1]);
                let mut out_l = vec![0.0f32; self.n];
                let mut out_r = vec![0.0f32; self.n];
                for i in 0..self.n {
                    let w = if weight[i] > 1e-6 { weight[i] } else { 1.0 };
                    out_l[i] = (left[i] / w).clamp(-1.0, 1.0);
                    out_r[i] = (right[i] / w).clamp(-1.0, 1.0);
                }
                stems.push(StemAudio {
                    left: out_l,
                    right: out_r,
                });
            }
            stems
        }
    }

    fn run_waveform(
        session: &mut Session,
        spec: &'static ModelSpec,
        l: &[f32],
        r: &[f32],
        mean: f32,
        std: f32,
        n: usize,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<StemAudio>, String> {
        let sources = spec.sources.len();
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .ok_or_else(|| "Modellen har inga indata".to_string())?;
        let output_name = session
            .outputs()
            .first()
            .map(|o| o.name().to_string())
            .ok_or_else(|| "Modellen har inga utdata".to_string())?;

        let segment = (MODEL_SAMPLE_RATE as f32 * spec.segment_secs.max(1.0)) as usize;
        let overlap = (MODEL_SAMPLE_RATE as f32 * DEFAULT_OVERLAP_SECONDS) as usize;
        let hop = segment.saturating_sub(overlap).max(1);
        let starts = plan_segments(n, segment, hop);
        let total = starts.len().max(1);
        let mut acc = Accumulator::new(n);

        for (segment_index, &start) in starts.iter().enumerate() {
            let end = (start + segment).min(n);
            let seg_len = end - start;

            // Always feed a full-length segment: HTDemucs has fixed positional
            // embeddings, so the final short chunk is zero-padded.
            let mut input = vec![0.0f32; NUM_CHANNELS * segment];
            for i in 0..seg_len {
                input[i] = (l[start + i] - mean) / std;
                input[segment + i] = (r[start + i] - mean) / std;
            }
            let tensor = Tensor::from_array((vec![1i64, 2, segment as i64], input))
                .map_err(|e| format!("Kunde inte skapa indata-tensor: {e}"))?;
            let outputs = session
                .run(ort::inputs![input_name.as_str() => tensor])
                .map_err(|e| format!("ONNX-körning misslyckades: {e}"))?;
            let value = outputs
                .get(output_name.as_str())
                .ok_or_else(|| format!("Modellens utdata '{output_name}' saknas"))?;
            let (shape, data) = value
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Kunde inte läsa utdata: {e}"))?;
            let shape: Vec<i64> = shape.iter().copied().collect();

            let first = segment_index == 0;
            let last = segment_index + 1 == total;
            let window = segment_window(seg_len, overlap.min(seg_len / 2), first, last);

            for source in 0..sources {
                let slot = stem_slot(spec.sources[source]);
                for channel in 0..NUM_CHANNELS {
                    let slice = source_channel_slice(&shape, data, source, channel, NUM_CHANNELS)?;
                    let usable = slice.len().min(seg_len);
                    let mut piece: Vec<f32> = Vec::with_capacity(usable);
                    for i in 0..usable {
                        piece.push((slice[i] * std + mean) * window[i]);
                    }
                    acc.add_windowed(slot, channel, start, &piece);
                }
            }
            for i in 0..seg_len {
                acc.weight[start + i] += window[i];
            }
            progress((segment_index + 1) as f32 / total as f32);
        }
        Ok(acc.finish())
    }

    /// Vad modellen faktiskt väntar — med i felmeddelandet, så ett formfel går att åtgärda utan
    /// att gissa (`Invalid rank for input` säger inget om vilken form som gäller).
    fn io_summary(session: &Session) -> String {
        let inputs: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| format!("{}: {:?}", i.name(), i.dtype()))
            .collect();
        let outputs: Vec<String> = session
            .outputs()
            .iter()
            .map(|o| format!("{}: {:?}", o.name(), o.dtype()))
            .collect();
        format!("in [{}] ut [{}]", inputs.join(", "), outputs.join(", "))
    }

    /// **Namnen ur sessionen, inte ur minnet.** Exporterna skriver inte alltid `mix_spec` /
    /// `stems_spec` (xycld-exporten har andra namn), och en gissning ger "Invalid input name".
    /// Först letas det dokumenterade namnet, annars tas platsen i sessionens egen ordning.
    fn spec_io_names(
        session: &Session,
        wanted_inputs: &[&str],
        wanted_outputs: &[&str],
    ) -> Result<(Vec<String>, Vec<String>), String> {
        let inputs: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();
        let outputs: Vec<String> = session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        if inputs.len() < wanted_inputs.len() || outputs.len() < wanted_outputs.len() {
            return Err(format!(
                "modellen har {} indata och {} utdata, väntade minst {} och {}",
                inputs.len(),
                outputs.len(),
                wanted_inputs.len(),
                wanted_outputs.len()
            ));
        }
        let pick = |have: &[String], wanted: &[&str]| -> Vec<String> {
            wanted
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    have.iter()
                        .find(|h| h.as_str() == *name)
                        .cloned()
                        .unwrap_or_else(|| have[i].clone())
                })
                .collect()
        };
        Ok((pick(&inputs, wanted_inputs), pick(&outputs, wanted_outputs)))
    }

    fn run_spectrogram(
        session: &mut Session,
        spec: &'static ModelSpec,
        l: &[f32],
        r: &[f32],
        mean: f32,
        std: f32,
        n: usize,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<StemAudio>, String> {
        let params = spec
            .stft
            .ok_or_else(|| format!("{} saknar STFT-parametrar", spec.id))?;
        let frames = spec.frames.max(2);
        let bins = params.bins();
        let plane = bins * frames;
        // Blocket måste ge exakt `frames` frames — en export är tracad med fast `T`.
        let block = params.samples_for(frames).max(params.hop * 2);
        let hop = ((block as f32) * (1.0 - spec.overlap)).round().max(1.0) as usize;
        let starts = plan_segments(n, block, hop);
        let total = starts.len().max(1);
        let sources = spec.sources.len();
        // Före körningen: sessionen lånas mutabelt av `run`, så sammanfattningen måste tas först.
        let io = io_summary(session);
        let mut acc = Accumulator::new(n);

        for (segment_index, &start) in starts.iter().enumerate() {
            let end = (start + block).min(n);
            let seg_len = end - start;
            let mut xl = vec![0.0f32; block];
            let mut xr = vec![0.0f32; block];
            for i in 0..seg_len {
                xl[i] = (l[start + i] - mean) / std;
                xr[i] = (r[start + i] - mean) / std;
            }
            let (re_l, im_l) = stft(&xl, &params);
            let (re_r, im_r) = stft(&xr, &params);

            // Modellens utdata som fyra plan per stämma: L-re, L-im, R-re, R-im.
            let planes: Vec<[Vec<f32>; 4]> = match spec.layout {
                SpecLayout::RealImag => {
                    let (ins, outs) = spec_io_names(
                        session,
                        &["spec_real", "spec_imag"],
                        &["out_spec_real", "out_spec_imag"],
                    )?;
                    let mut re = vec![0.0f32; 2 * plane];
                    let mut im = vec![0.0f32; 2 * plane];
                    re[..plane].copy_from_slice(&re_l);
                    im[..plane].copy_from_slice(&im_l);
                    re[plane..].copy_from_slice(&re_r);
                    im[plane..].copy_from_slice(&im_r);
                    let t_re = Tensor::from_array((vec![1i64, 2, bins as i64, frames as i64], re))
                        .map_err(|e| format!("indata: {e}"))?;
                    let t_im = Tensor::from_array((vec![1i64, 2, bins as i64, frames as i64], im))
                        .map_err(|e| format!("indata: {e}"))?;
                    let outputs = session
                        .run(ort::inputs![ins[0].as_str() => t_re, ins[1].as_str() => t_im])
                        .map_err(|e| format!("ONNX-körning misslyckades: {e} — {io}"))?;
                    let (shape_re, data_re) = outputs
                        .get(outs[0].as_str())
                        .ok_or_else(|| format!("utdata '{}' saknas", outs[0]))?
                        .try_extract_tensor::<f32>()
                        .map_err(|e| format!("utdata: {e}"))?;
                    let (_, data_im) = outputs
                        .get(outs[1].as_str())
                        .ok_or_else(|| format!("utdata '{}' saknas", outs[1]))?
                        .try_extract_tensor::<f32>()
                        .map_err(|e| format!("utdata: {e}"))?;
                    let shape: Vec<i64> = shape_re.iter().copied().collect();
                    let mut out = Vec::with_capacity(sources);
                    for source in 0..sources {
                        // `[1, S, 2, bins, T]` **per tensor**: re och im ligger i varsin tensor, så
                        // planet för (stämma, kanal) är `source*2 + kanal` i båda. Att räkna fyra
                        // plan per stämma i en och samma tensor läste förbi slutet på en 6-stämmors
                        // export (mätt: "[1, 6, 2, 1025, 345]", 12 plan).
                        let l = source * 2;
                        out.push([
                            spec_plane(&shape, data_re, l, bins, frames)?.to_vec(),
                            spec_plane(&shape, data_im, l, bins, frames)?.to_vec(),
                            spec_plane(&shape, data_re, l + 1, bins, frames)?.to_vec(),
                            spec_plane(&shape, data_im, l + 1, bins, frames)?.to_vec(),
                        ]);
                    }
                    out
                }
                SpecLayout::PackedChannels | SpecLayout::PackedSingle => {
                    let (ins, outs) = spec_io_names(session, &["mix_spec"], &["stems_spec"])?;
                    let mut packed = vec![0.0f32; 4 * plane];
                    packed[..plane].copy_from_slice(&re_l);
                    packed[plane..2 * plane].copy_from_slice(&im_l);
                    packed[2 * plane..3 * plane].copy_from_slice(&re_r);
                    packed[3 * plane..].copy_from_slice(&im_r);
                    let t = Tensor::from_array((vec![1i64, 4, bins as i64, frames as i64], packed))
                        .map_err(|e| format!("indata: {e}"))?;
                    let outputs = session
                        .run(ort::inputs![ins[0].as_str() => t])
                        .map_err(|e| format!("ONNX-körning misslyckades: {e} — {io}"))?;
                    let (shape, data) = outputs
                        .get(outs[0].as_str())
                        .ok_or_else(|| format!("utdata '{}' saknas", outs[0]))?
                        .try_extract_tensor::<f32>()
                        .map_err(|e| format!("utdata: {e}"))?;
                    let shape: Vec<i64> = shape.iter().copied().collect();
                    let mut out = Vec::new();
                    for source in 0..sources {
                        let base = source * 4;
                        out.push([
                            spec_plane(&shape, data, base, bins, frames)?.to_vec(),
                            spec_plane(&shape, data, base + 1, bins, frames)?.to_vec(),
                            spec_plane(&shape, data, base + 2, bins, frames)?.to_vec(),
                            spec_plane(&shape, data, base + 3, bins, frames)?.to_vec(),
                        ]);
                    }
                    out
                }
            };

            let first = segment_index == 0;
            let last = segment_index + 1 == total;
            let window = segment_window(seg_len, hop.min(seg_len / 2), first, last);

            for (source, [l_re, l_im, r_re, r_im]) in planes.iter().enumerate() {
                let slot = stem_slot(spec.sources[source]);
                let left_wave = istft(l_re, l_im, frames, block, &params);
                let right_wave = istft(r_re, r_im, frames, block, &params);
                for (channel, wave) in [&left_wave, &right_wave].into_iter().enumerate() {
                    let usable = wave.len().min(seg_len);
                    let mut piece: Vec<f32> = Vec::with_capacity(usable);
                    for i in 0..usable {
                        piece.push((wave[i] * std + mean) * window[i]);
                    }
                    acc.add_windowed(slot, channel, start, &piece);
                }
                // En modell med **en** stämma ger resten som instrument: mixen minus det modellen
                // tog ut. Utan det hade sångmodellerna lämnat tystnad efter sig.
                if sources == 1 && slot == 0 {
                    let residual = 3; // instruments
                    acc.mark(residual);
                    for (channel, (orig, wave)) in
                        [(l, &left_wave), (r, &right_wave)].into_iter().enumerate()
                    {
                        let usable = wave.len().min(seg_len);
                        let mut piece: Vec<f32> = Vec::with_capacity(usable);
                        for i in 0..usable {
                            let mix = (orig[start + i] - mean) / std;
                            piece.push((mix - wave[i]) * std * window[i]);
                        }
                        acc.add_windowed(residual, channel, start, &piece);
                    }
                }
            }
            for i in 0..seg_len {
                acc.weight[start + i] += window[i];
            }
            progress((segment_index + 1) as f32 / total as f32);
        }
        Ok(acc.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demucs_source_order_maps_to_sonix_stems() {
        // drums, bass, other, vocals -> Vocals, Drums, Bass, Instruments
        assert_eq!(demucs_source_to_stem(0), 1);
        assert_eq!(demucs_source_to_stem(1), 2);
        assert_eq!(demucs_source_to_stem(2), 3);
        assert_eq!(demucs_source_to_stem(3), 0);
        assert_eq!(demucs_source_to_stem(4), 4, "gitarr → gitarr");
        assert_eq!(demucs_source_to_stem(5), 5, "piano → piano");
    }

    #[test]
    fn resampling_upsamples_and_preserves_dc() {
        let left = vec![0.5f32; 100];
        let right = vec![-0.5f32; 100];
        let (l, r) = resample_linear(&left, &right, 1000, 2000);
        assert_eq!(l.len(), 200);
        assert_eq!(r.len(), 200);
        assert!(l.iter().all(|v| (*v - 0.5).abs() < 1e-6));
        assert!(r.iter().all(|v| (*v + 0.5).abs() < 1e-6));
    }

    #[test]
    fn resampling_downsample_length_and_identity() {
        let left: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let (l, r) = resample_linear(&left, &left, 48000, 44100);
        let expected = (1000.0_f64 * 44100.0 / 48000.0).round() as usize;
        assert_eq!(l.len(), expected);
        assert_eq!(r.len(), expected);
        // Same rate is an exact copy.
        let (same, _) = resample_linear(&left, &left, 44100, 44100);
        assert_eq!(same, left);
    }

    #[test]
    fn segments_cover_the_whole_signal() {
        let starts = plan_segments(10_000, 4_000, 3_000);
        assert_eq!(starts, vec![0, 3_000, 6_000]);
        assert_eq!(*starts.last().unwrap() + 4_000, 10_000);
        assert!(plan_segments(0, 4_000, 3_000).is_empty());
        // Shorter than one segment -> a single full-length start.
        assert_eq!(plan_segments(500, 4_000, 3_000), vec![0]);
    }

    #[test]
    fn segment_windows_do_not_attenuate_boundaries() {
        let first = segment_window(100, 20, true, false);
        let last = segment_window(100, 20, false, true);
        let middle = segment_window(100, 20, false, false);
        assert_eq!(first[0], 1.0);
        assert_eq!(last[99], 1.0);
        assert_eq!(middle[0], middle[99]);
        assert!(middle[0] < 1.0 && middle[0] > 0.0);
        // Crossfading two overlapping middle windows sums to 1 in the overlap.
        let a = segment_window(100, 40, false, false);
        let b = segment_window(100, 40, false, false);
        for j in 0..40 {
            assert!((a[100 - 40 + j] + b[j] - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn extracts_sources_from_batched_and_flat_layouts() {
        // [1, 4, 2, 4]: source 0 channel 1 should be samples 4..8.
        let data: Vec<f32> = (0..32).map(|i| i as f32).collect();
        let s = source_channel_slice(&[1, 4, 2, 4], &data, 0, 1, 2).unwrap();
        assert_eq!(s, &[4.0, 5.0, 6.0, 7.0]);
        // [1, 8, 4] (S*C flattened): source 3 channel 0 -> index 6.
        let s = source_channel_slice(&[1, 8, 4], &data, 3, 0, 2).unwrap();
        assert_eq!(s, &[24.0, 25.0, 26.0, 27.0]);
        // Unbatched [8, 4].
        let s = source_channel_slice(&[8, 4], &data, 1, 0, 2).unwrap();
        assert_eq!(s, &[8.0, 9.0, 10.0, 11.0]);
    }

    #[test]
    fn rejects_bad_shapes_and_short_buffers() {
        let data = vec![0.0f32; 8];
        assert!(source_channel_slice(&[1, 8, 4], &data, 3, 1, 2).is_err());
        assert!(source_channel_slice(&[1, 4, 2, 4], &data, 1, 0, 2).is_err());
        assert!(source_channel_slice(&[7], &data, 0, 0, 2).is_err());
    }

    #[test]
    fn finds_model_in_priority_order() {
        let base = std::env::temp_dir().join("sonix_model_discovery_test");
        let a = base.join("a");
        let b = base.join("b");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        assert!(find_model_in(&[a.clone(), b.clone()]).is_none());
        std::fs::write(b.join("demucs.onnx"), b"x").unwrap();
        assert_eq!(
            find_model_in(&[a.clone(), b.clone()]),
            Some(b.join("demucs.onnx"))
        );
        std::fs::write(a.join("htdemucs.onnx"), b"x").unwrap();
        assert_eq!(
            find_model_in(&[a.clone(), b.clone()]),
            Some(a.join("htdemucs.onnx"))
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn neural_requires_a_model_to_be_available() {
        // Without a model on disk this must be false regardless of the feature.
        if find_model().is_none() {
            assert!(!is_available());
        }
    }

    #[test]
    fn a_short_download_is_an_error_not_a_model() {
        assert!(download_looks_complete(100, Some(100)).is_ok());
        let err = download_looks_complete(40, Some(100)).unwrap_err();
        assert!(err.contains("avbröts"), "{err}");
        // Utan Content-Length finns bara en undre gräns.
        assert!(download_looks_complete(20 * MB, None).is_ok());
        assert!(
            download_looks_complete(2_000_000, None).is_err(),
            "2 MB är ingen modell"
        );
    }

    #[test]
    fn the_model_is_the_single_file_htdemucs_export() {
        // Samma kontrakt som inferensen: en fil, fyra stämmor, fp16-vikter för storleken.
        assert!(MODEL_URL.starts_with("https://huggingface.co/StemSplitio/htdemucs-onnx/"));
        assert!(MODEL_URL.ends_with("htdemucs_fp16weights.onnx"), "{MODEL_URL}");
        assert!(!MODEL_URL.starts_with("file:"));
    }

    /// **Varje modell i katalogen är komplett och unik.** Ett spektrogram utan STFT-parametrar
    /// eller två modeller med samma filnamn är fel som annars syns först när någon hämtar 350 MB.
    #[test]
    fn every_model_in_the_catalogue_is_complete_and_distinct() {
        // De modeller som är verifierade mot sin egen export (smoke-provet, docs/STEM-SEPARATION.md).
        let expected = ["htdemucs", "htdemucs_6s", "bs_roformer_sw6", "mel_roformer", "scnet"];
        assert_eq!(MODELS.len(), expected.len(), "katalogen ändrades utan att listan gjorde det");
        for id in expected {
            assert!(model_by_id(id).is_some(), "katalogen saknar {id}");
        }
        for (i, m) in MODELS.iter().enumerate() {
            assert!(!m.id.is_empty() && !m.label.is_empty(), "id och etikett krävs");
            assert!(!m.sources.is_empty(), "{} saknar källor", m.id);
            assert!(m.expected_bytes > MB, "{} saknar rimlig storlek", m.id);
            assert!(m.url.starts_with("https://"), "{} saknar URL", m.id);
            assert!(
                m.note.contains("MIT") || m.note.contains("licens"),
                "{} bör säga något om licensen",
                m.id
            );
            match m.format {
                ModelFormat::Spectrogram => {
                    assert!(
                        m.stft.is_some(),
                        "{} är ett spektrogram utan STFT-parametrar",
                        m.id
                    );
                    assert!(m.frames >= 2, "{} saknar frames", m.id);
                }
                ModelFormat::Waveform => assert!(m.segment_secs > 0.0, "{} saknar segment", m.id),
            }
            for other in &MODELS[i + 1..] {
                assert_ne!(m.id, other.id, "dubbelt id");
                assert_ne!(m.file, other.file, "dubbelt filnamn");
            }
        }
    }

    #[test]
    fn sources_map_to_the_six_stem_slots() {
        // Appens ordning: vocals, drums, bass, instruments, guitar, piano.
        assert_eq!(stem_slot("vocals"), 0);
        assert_eq!(stem_slot("drums"), 1);
        assert_eq!(stem_slot("bass"), 2);
        assert_eq!(stem_slot("other"), 3);
        assert_eq!(stem_slot("guitar"), 4);
        assert_eq!(stem_slot("piano"), 5);
        // Okänt namn hamnar i "instrument" i stället för att försvinna.
        assert_eq!(stem_slot("strings"), 3);
    }

    /// **BS-RoFormer-SW börjar med basen**, appens ordning börjar med sången — och blocket måste
    /// ge exakt de frames exporten är tracad med, annars får modellen fel form.
    #[test]
    fn the_spectrogram_models_get_their_own_order_and_block_length() {
        let sw = model_by_id("bs_roformer_sw6").expect("BS-RoFormer-SW ska finnas");
        assert_eq!(sw.sources[0], "bass");
        assert_eq!(sw.sources[3], "vocals");
        let params = sw.stft.expect("STFT");
        assert_eq!((params.n_fft, params.hop), (2048, 512));
        assert_eq!(params.frames_for(params.samples_for(sw.frames)), sw.frames);

        let scnet = model_by_id("scnet").expect("SCNet ska finnas");
        let p = scnet.stft.expect("STFT");
        assert_eq!(p.bins(), 2049, "4096 FFT ger 2049 bins");
        assert!(!p.hann, "SCNet-exporten är fönsterlös");
        assert_eq!(p.frames_for(p.samples_for(scnet.frames)), scnet.frames);

        // De två Demucs-modellerna är vågform och behåller sina gamla filnamn.
        assert_eq!(model_by_id("htdemucs").unwrap().file, "htdemucs.onnx");
        assert!(model_by_id("htdemucs_6s").unwrap().file.contains("6s"));
    }

    /// En 4-stämmorsfil i mappen får inte räknas som en sexa.
    #[test]
    fn the_six_stem_model_has_its_own_url_and_file() {
        let six = model_by_id("htdemucs_6s").unwrap();
        assert!(six.url.contains("htdemucs-6s-onnx"));
        assert!(six.url.ends_with("htdemucs_6s_fp16weights.onnx"));
        let dir = std::env::temp_dir().join(format!("sonix_6s_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("htdemucs.onnx"), b"x").unwrap();
        assert!(
            find_model_in_with(&[dir.clone()], &[six.file]).is_none(),
            "4-stämmorsfilen får inte användas som 6s-modell"
        );
        std::fs::write(dir.join(six.file), b"y").unwrap();
        assert!(find_model_in_with(&[dir.clone()], &[six.file]).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_download_floor_scales_with_the_model() {
        // En 13 MB-modell får inte kräva 100 MB för att godkännas.
        let scnet = model_by_id("scnet").unwrap();
        assert!(min_bytes(scnet) < 13 * MB);
        assert!(download_looks_complete(12_900_277, Some(12_900_277)).is_ok());
        assert!(download_looks_complete(1_000, Some(12_900_277)).is_err());
    }

    #[test]
    fn spec_planes_are_read_by_index_and_bad_indexes_are_reported() {
        let (bins, frames) = (3, 2);
        let data: Vec<f32> = (0..24).map(|i| i as f32).collect();
        // Två plan om 6 värden.
        assert_eq!(
            spec_plane(&[1, 4, 3, 2], &data, 1, bins, frames).unwrap(),
            &[6.0, 7.0, 8.0, 9.0, 10.0, 11.0]
        );
        let err = spec_plane(&[1, 4, 3, 2], &data, 4, bins, frames).unwrap_err();
        assert!(err.contains("4"), "{err}");
    }

    /// Kör **en riktig modell** på en syntetisk mix. Opt-in: `SONIX_NEURAL_SMOKE=<modell-id>`
    /// (eller `1` för standardmodellen).
    ///
    /// Provet svarar på frågan som filstorleken inte kan svara på: går det att köra modellen genom
    /// vår inferens, och summerar stämmorna tillbaka till källan?
    #[cfg(feature = "neural")]
    #[test]
    fn a_real_model_separates_a_synthetic_mix() {
        let Ok(which) = std::env::var("SONIX_NEURAL_SMOKE") else {
            return;
        };
        let spec = if which == "1" {
            default_model()
        } else {
            match model_by_id(&which) {
                Some(spec) => spec,
                None => return,
            }
        };
        if !is_available_for(spec) {
            return;
        }
        let sr = 44_100u32;
        let n = sr as usize * 4;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut seed: u32 = 12345;
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let bass = 0.30 * (2.0 * std::f32::consts::PI * 110.0 * t).sin();
            let vocal = 0.25 * (2.0 * std::f32::consts::PI * 660.0 * t).sin();
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
            let drum = if (t * 4.0).fract() < 0.05 { 0.25 * noise } else { 0.0 };
            let s = (bass + vocal + drum).clamp(-1.0, 1.0);
            l[i] = s;
            r[i] = s;
        }

        let stems = separate_neural_with(spec, &l, &r, sr, &|_| {}).expect("modellen ska köra");
        assert!(!stems.is_empty(), "modellen gav inga stämmor");

        // Summerar stämmorna tillbaka till källan? Det är modellens hela uppgift.
        let mut sum = vec![0.0f32; n];
        for stem in &stems {
            for (i, v) in stem.left.iter().enumerate().take(n) {
                sum[i] += v;
            }
        }
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let ratio = rms(&sum) / rms(&l).max(1e-9);
        println!(
            "{}: {} stämmor, summering {ratio:.3} av källan",
            spec.id,
            stems.len()
        );
        assert!(
            (0.5..1.7).contains(&ratio),
            "{}: stämmorna summerar till {ratio:.2} av källan — spektrogramvägen eller mappningen är fel",
            spec.id
        );
    }
}
