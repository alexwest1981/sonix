//! **STFT och iSTFT** för de ONNX-modeller som tar ett spektrum i stället för vågform.
//!
//! RoFormer-familjen och SCNet är exporterade mellan sin egen STFT och iSTFT: grafen tar
//! `spec_real`/`spec_imag` (eller en packad `mix_spec`) och lämnar maskerade koefficienter.
//! Alltså måste vi göra fönstringen själva, med **exakt** de parametrar varje export är tracad med
//! — fel `n_fft` eller fel fönster ger inget felmeddelande, bara brus.
//!
//! Layouten är `[bin][frame]` (frekvens först), samma ordning som `torch.stft` ger och som
//! tensorerna `[1, kanal, 1025, T]` förväntar.
//!
//! Två saker skiljer sig mellan exporterna och är därför parametrar: fönstret (Hann eller inget)
//! och skalan (SCNet delar sin STFT med `1/√n_fft`, RoFormer-exporten är onormaliserad).

// Fönstringen används bara av neural-vägen; utan `--features neural` är den oanvänd.
#![cfg_attr(not(feature = "neural"), allow(dead_code))]

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

/// Hur en exports STFT ser ut. `Copy` så att katalogen kan vara en `const`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StftParams {
    /// Fönsterlängd och FFT-storlek. `n_fft / 2 + 1` frekvensbins ut.
    pub n_fft: usize,
    /// Hopp mellan fönstren i sampel.
    pub hop: usize,
    /// Hann-fönster (sant) eller rektangulärt (falskt).
    pub hann: bool,
    /// Reflektionspadding av `n_fft / 2` i båda ändar, som `torch.stft(center=True)`.
    pub center: bool,
    /// Delar insignalen med det här innan FFT:n, och utsignalerna multipliceras tillbaka med det
    /// i iSTFT:n. 1,0 = onormaliserad (RoFormer-exporterna).
    pub scale: f32,
}

impl StftParams {
    /// Antal frames för `samples` sampel, med centrering (samma räkning som `torch.stft`).
    pub fn frames_for(&self, samples: usize) -> usize {
        1 + samples / self.hop.max(1)
    }

    /// Antal sampel som ger exakt `frames` frames. Används för att klippa modellens block: en
    /// export är tracad med fast `T`, så blocket måste vara exakt så långt.
    pub fn samples_for(&self, frames: usize) -> usize {
        frames.saturating_sub(1) * self.hop.max(1)
    }

    /// Antal frekvensbins.
    pub fn bins(&self) -> usize {
        self.n_fft / 2 + 1
    }

    fn window(&self) -> Vec<f32> {
        if !self.hann {
            return vec![1.0; self.n_fft];
        }
        (0..self.n_fft)
            .map(|i| {
                let x = std::f32::consts::PI * i as f32 / self.n_fft as f32;
                x.sin() * x.sin()
            })
            .collect()
    }

    fn pad(&self, x: &[f32]) -> Vec<f32> {
        if !self.center || x.len() <= 1 {
            return x.to_vec();
        }
        let half = self.n_fft / 2;
        let mut out = Vec::with_capacity(x.len() + 2 * half);
        // Reflektion utan att upprepa kanten (torch:s `reflect`).
        for i in 0..half {
            out.push(x[(half - i).min(x.len() - 1)]);
        }
        out.extend_from_slice(x);
        for i in 0..half {
            let idx = x.len().saturating_sub(2 + i);
            out.push(x[idx]);
        }
        out
    }
}

/// STFT av en kanal. Returnerar `(real, imag)`, båda `bins × frames` i `[bin][frame]`-ordning.
pub fn stft(x: &[f32], params: &StftParams) -> (Vec<f32>, Vec<f32>) {
    let bins = params.bins();
    let padded = params.pad(x);
    let frames = params.frames_for(x.len());
    let window = params.window();
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(params.n_fft);

    let mut real = vec![0.0f32; bins * frames];
    let mut imag = vec![0.0f32; bins * frames];
    let mut buf = vec![Complex::new(0.0f32, 0.0f32); params.n_fft];

    for frame in 0..frames {
        let start = frame * params.hop;
        for i in 0..params.n_fft {
            let sample = padded.get(start + i).copied().unwrap_or(0.0);
            buf[i] = Complex::new(sample * window[i] / params.scale, 0.0);
        }
        fft.process(&mut buf);
        for bin in 0..bins {
            real[bin * frames + frame] = buf[bin].re;
            imag[bin * frames + frame] = buf[bin].im;
        }
    }
    (real, imag)
}

/// iSTFT: fönstrar, summerar med överlapp och delar med fönstrets kvadratsumma, så att
/// `istft(stft(x))` ger tillbaka `x` (provat i `stft_tests`).
pub fn istft(
    real: &[f32],
    imag: &[f32],
    frames: usize,
    samples: usize,
    params: &StftParams,
) -> Vec<f32> {
    let bins = params.bins();
    let window = params.window();
    let padded_len = if params.center {
        samples + params.n_fft
    } else {
        samples + params.n_fft
    };
    let mut out = vec![0.0f32; padded_len];
    let mut wsum = vec![0.0f32; padded_len];
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_inverse(params.n_fft);
    let mut buf = vec![Complex::new(0.0f32, 0.0f32); params.n_fft];

    for frame in 0..frames {
        for bin in 0..bins {
            let idx = bin * frames + frame;
            let re = real.get(idx).copied().unwrap_or(0.0);
            let im = imag.get(idx).copied().unwrap_or(0.0);
            buf[bin] = Complex::new(re * params.scale, im * params.scale);
        }
        // **Negativa frekvenser är spegelbilden.** Nollställs de blir inversen en analytisk
        // signal — halva amplituden, alltså en tystnad som ser ut som en modell som inte hittar
        // något. Spegla i stället, med DC och Nyquist reella.
        buf[0].im = 0.0;
        if bins >= 2 {
            buf[bins - 1].im = 0.0;
        }
        for bin in 1..bins.saturating_sub(1) {
            if params.n_fft >= 2 * bin {
                buf[params.n_fft - bin] = buf[bin].conj();
            }
        }
        fft.process(&mut buf);
        let norm = 1.0 / params.n_fft as f32;
        let start = frame * params.hop;
        for i in 0..params.n_fft {
            if let Some(slot) = out.get_mut(start + i) {
                *slot += buf[i].re * norm * window[i];
                wsum[start + i] += window[i] * window[i];
            }
        }
    }
    for (o, w) in out.iter_mut().zip(wsum.iter()) {
        if *w > 1e-8 {
            *o /= *w;
        }
    }
    let half = if params.center { params.n_fft / 2 } else { 0 };
    out.into_iter().skip(half).take(samples).collect()
}

#[cfg(test)]
mod stft_tests {
    use super::*;

    fn tone(n: usize, freq: f32, sr: f32) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin() * 0.5)
            .collect()
    }

    /// **Rundturen är kontraktet.** Kan vi inte få tillbaka det vi skickade in, är varje
    /// modellresultat brus hur bra modellen än är.
    #[test]
    fn stft_and_istft_round_trip_both_window_and_scale_variants() {
        let sr = 44_100.0;
        let x = tone(20_000, 440.0, sr);
        for (hann, scale) in [(true, 1.0f32), (false, 1.0f32 / 64.0), (true, 1.0 / 64.0)] {
            let params = StftParams {
                n_fft: 2048,
                hop: 512,
                hann,
                center: true,
                scale,
            };
            let (re, im) = stft(&x, &params);
            assert_eq!(re.len(), params.bins() * params.frames_for(x.len()));
            let back = istft(&re, &im, params.frames_for(x.len()), x.len(), &params);
            assert_eq!(back.len(), x.len());
            let err: f32 = x
                .iter()
                .zip(back.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);
            assert!(
                err < 1e-3,
                "rundturen ska ge tillbaka signalen (hann={hann}, scale={scale}), felet var {err}"
            );
        }
    }

    #[test]
    fn the_frame_count_matches_the_block_length() {
        let params = StftParams {
            n_fft: 2048,
            hop: 512,
            hann: true,
            center: true,
            scale: 1.0,
        };
        // 4 s @ 44,1 kHz → 345 frames, som bs-roformer-exporten är tracad med.
        assert_eq!(params.frames_for(176_400), 345);
        assert_eq!(params.samples_for(345), 176_128);
        // 8 s @ 44,1 kHz → 801 frames (mel-roformer), 11,03 s → 476 (SCNet).
        assert_eq!(params.frames_for(params.samples_for(801)), 801);
        let scnet = StftParams {
            n_fft: 4096,
            hop: 1024,
            hann: false,
            center: true,
            scale: 1.0 / 64.0,
        };
        assert_eq!(scnet.bins(), 2049);
        assert_eq!(scnet.frames_for(scnet.samples_for(476)), 476);
    }
}
