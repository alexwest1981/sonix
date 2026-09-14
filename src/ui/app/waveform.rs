//! Vågformer i vyn — nyckeln, översikten och den sanna cachen.
//!
//! Cachen nycklas på **buffertens identitet** (samma PCM ger samma cache) och byggs i den
//! tråd som ändå avkodar filen, så den första bildrutan har en sann vågform i stället för
//! en gissning. Representationen byts vid zoomtrösklar — se
//! `references/waveform-rendering.md` i sonix-skillen innan du rör ritningen.
//!
//! Status: stabil — exaktheten och nycklarna; ändra bara med ett prov som visar samma sak.
//! Rör inte: `imported_track_waveform` är regeln som ger spåret samma cache som motorn fick.

use super::*;

/// Identiteten hos en ljudbuffert: adress, längd och första/sista sample.
///
/// Räcker för att avgöra om vågformscachen hör till ljudet som ligger där nu, och
/// är billig nog att räkna varje bildruta. Innehållet kontrolleras i båda ändar
/// eftersom en frigjord buffert kan få samma adress igen.
pub(crate) fn waveform_key(buf: &std::sync::Arc<Vec<f32>>) -> u64 {
    let ptr = std::sync::Arc::as_ptr(buf) as usize as u64;
    let len = buf.len() as u64;
    let first = buf.first().copied().unwrap_or(0.0).to_bits() as u64;
    let last = buf.last().copied().unwrap_or(0.0).to_bits() as u64;
    ptr.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ len.rotate_left(17)
        ^ first.rotate_left(31)
        ^ last.rotate_left(47)
}

/// Den grova översikten som följer med en region — räknad **ur cachen**, ur en
/// genomgång som redan är gjord (Fas 8.3).
///
/// Symmetriskt toppvärde per punkt, samma form som regionritningens fallback och
/// projektfilen väntar sig. Skillnaden mot den gamla raka samplingen (`var 64:e
/// sample`) är att inget sample kan falla mellan två punkter: den som tittar på
/// översikten ser varje anslag som finns i filen.
pub(crate) fn overview_peaks_from(
    cache: &crate::audio::waveform::WaveformCache,
    samples: &[f32],
    points: usize,
) -> Vec<f32> {
    cache
        .envelope(samples, points)
        .into_iter()
        .map(|(lo, hi)| lo.abs().max(hi.abs()).clamp(0.04, 0.98))
        .collect()
}

/// Vad en importerad stämma lägger på spåret: PCM och den cache som hör till
/// **just den bufferten** (Fas 8.3).
///
/// Regeln ligger här och inte inuti `apply_imported_stems` för att kunna prövas
/// utan ett fönster (repots konvention). Cachen nycklas på bufferten — hör den
/// till ett annat ljud än spåret har lämnas den ifrån sig, för annars ritar
/// tidslinjen fel ljud och tror att det är rätt.
///
/// Utan PCM blir det ingen cache: en cache utan ljud är precis den sorts
/// trovärdiga bild som 8.5 handlade om.
pub(crate) fn imported_track_waveform(
    pcm: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    cache: Option<crate::audio::waveform::WaveformCache>,
) -> (
    Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    Option<(u64, crate::audio::waveform::WaveformCache)>,
) {
    match pcm {
        Some((left, right, sr)) => {
            let keyed = cache.map(|cache| (waveform_key(&left), cache));
            (Some((left, right, sr)), keyed)
        }
        None => (None, None),
    }
}

/// Vågform för kanalvisningen, räknad ur ljudet (Fas 6.7). Toppvärdet per
/// fönster räcker — det är samma sorts översikt kanalracket ritar.
pub(crate) fn waveform_preview_from_pcm(pcm: &[f32], points: usize) -> Vec<f32> {
    if pcm.is_empty() || points == 0 {
        return Vec::new();
    }
    let bucket = (pcm.len() / points).max(1);
    pcm.chunks(bucket)
        .map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())))
        .take(points)
        .collect()
}

/// Loads a WAV file once and returns Arc-wrapped stereo PCM for cheap cloning into audio commands.
pub(crate) fn load_sample_pcm_arcs(path: &str) -> Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)> {
    match crate::audio::load_wav_pcm(path) {
        Ok((l, r, sr)) => Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr)),
        // Samma tystnad som de sju andra ställena hade: en fil som inte går att
        // läsa blev None och den som frågade fick inget veta. Kanalernas ljud, i
        // den här vägen, kunde försvinna utan ett ord — det är samma familj.
        Err(_) => {
            report_unreadable(path);
            None
        }
    }
}

impl SonixApp {
pub(crate) fn ensure_waveform_cache(&mut self, t_idx: usize) {
    // Först: ta emot vad arbetstrådarna blivit klara med.
    if let Some(rx) = self.waveform_cache_rx.as_ref() {
        while let Ok((idx, key, cache)) = rx.try_recv() {
            if let Some(t) = self.playlist_tracks.get_mut(idx) {
                t.waveform_cache = Some((key, cache));
                t.waveform_cache_pending = None;
            }
        }
    }

    let Some(track) = self.playlist_tracks.get(t_idx) else {
        return;
    };
    let Some((left, _right, _sr)) = track.frozen_pcm.as_ref().or(track.pcm_audio.as_ref())
    else {
        return;
    };
    let key = waveform_key(left);
    if track.waveform_cache.as_ref().map(|(k, _)| *k) == Some(key) {
        return; // redan byggd för det här ljudet
    }
    if track.waveform_cache_pending == Some(key) {
        return; // en tråd bygger den redan
    }

    // Kanalen skapas vid behov, så att konstruktorn inte behöver röras.
    if self.waveform_cache_rx.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        self.waveform_cache_tx = Some(tx);
        self.waveform_cache_rx = Some(rx);
    }
    let Some(tx) = self.waveform_cache_tx.as_ref().cloned() else {
        return;
    };
    let left = left.clone();
    // Den gamla cachen (för annat ljud) får inte användas medan den nya byggs.
    self.playlist_tracks[t_idx].waveform_cache = None;
    self.playlist_tracks[t_idx].waveform_cache_pending = Some(key);
    std::thread::spawn(move || {
        let cache = crate::audio::waveform::WaveformCache::build(&left);
        let _ = tx.send((t_idx, key, cache));
    });
}
}

