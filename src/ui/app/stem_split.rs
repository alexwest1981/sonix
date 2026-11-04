//! **Separera ett spår på plats.** Högerklicka på ett ljudspår → *Separera stämmor*: separationen
//! kör i bakgrunden, framsteget ritas på spårets egen rad, och när den är klar läggs varje stämma
//! in som ett eget spår **där originalet stod** — originalet tas bort.
//!
//! Maskineriet fanns redan: `stem_separator::run_separation_with` (neural modell med DSP-fallback),
//! `write_stems_and_read_envelopes` (stämmorna *måste* bli filer — annars pekar klippet på
//! originalfilen, och en mp3 som appen inte kan avkoda blir ett tyst klipp med trovärdig vågform)
//! och `sync_track_stem_to_engine`. Det som saknades var vägen **från ett spår i arrangemanget**
//! och att resultatet hamnade i stället för spåret, i stället för i en egen vy.
//!
//! Allt som kan gå fel sägs högt: spåret har inget ljud, ett annat jobb kör, filerna kunde inte
//! skrivas, modellen saknades och DSP-vägen användes. En separation som tyst blir något annat än
//! användaren bad om är samma familj som de tysta klippen (8.5-regeln).

use std::sync::{Arc, Mutex};

use eframe::egui::Color32;

use super::state::{AudioRegion, PlaylistTrack, TrackKind};
use super::SonixApp;

/// En färdig stämma, redo att bli ett spår.
pub struct StemPart {
    pub name: String,
    pub icon: &'static str,
    pub color: Color32,
    pub kind: TrackKind,
    pub path: String,
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub envelope: Vec<f32>,
    /// Klippets starttakt hos originalet: stämmorna ska ligga där spåret låg, inte på takt 1.
    pub start_bar: f32,
    pub bars: f32,
    pub volume: f32,
    /// Källans tempo (samma som originalets region), så en tempoändring sträcker stämmorna rätt.
    pub source_bpm: f32,
}

pub struct StemSplitJob {
    /// Raden som separeras. **Indexet är inte identitet** — spår kan läggas till och tas bort
    /// medan jobbet kör — så bufferten jämförs med `Arc::ptr_eq` innan något byts ut.
    pub track_index: usize,
    pub source: Arc<Vec<f32>>,
    pub progress: Arc<Mutex<f32>>,
    pub result: Arc<Mutex<Option<Result<SplitOutcome, String>>>>,
}

pub struct SplitOutcome {
    pub parts: Vec<StemPart>,
    pub used_neural: bool,
    pub fallback: Option<String>,
    pub model: String,
}

impl SonixApp {
    /// Startar separationen för ett spår. Returnerar en förklaring när den inte kunde starta —
    /// anroparen visar den; en knapp som inte gör något är värre än ingen knapp.
    pub fn start_stem_split(&mut self, track_idx: usize) -> Result<(), String> {
        if self.stem_split.is_some() {
            return Err(crate::i18n::t("En separation pågår redan — vänta tills den är klar")
                .to_string());
        }
        let Some(track) = self.playlist_tracks.get(track_idx) else {
            return Err(crate::i18n::t("Spåret finns inte").to_string());
        };
        let Some((left, right, sample_rate)) = track.pcm_audio.clone() else {
            return Err(crate::i18n::t("Spåret har inget ljud att separera").to_string());
        };
        if left.is_empty() || right.is_empty() {
            return Err(crate::i18n::t("Spåret är tomt").to_string());
        }

        // **Ett klipp i taget.** Ett spår med flera klipp är en klippt komposition; stämmorna
        // blir *hela* filen, så en enda region per stämma skulle påstå något om ljudet som
        // inte är sant. Säg det i stället för att lägga in en gissning.
        if track.regions.len() > 1 {
            return Err(crate::tstatus!(
                "Spåret har {} klipp — separera ett klipp i taget",
                track.regions.len()
            ));
        }
        // Tempot, längden och starttakten fryses här: de hör till spåret som det ser ut nu, och
        // tråden får inte läsa `self`.
        let bpm = self.bpm.max(20.0);
        let (start_bar, length_bars, source_bpm) = track
            .regions
            .first()
            .map(|r| (r.start_bar, r.length_bars, r.source_bpm))
            .unwrap_or_else(|| {
                let secs = left.len().min(right.len()) as f32 / sample_rate.max(1) as f32;
                (0.0, secs * bpm / 60.0 / 4.0, 0.0)
            });
        let dir = crate::paths::paths()
            .project_assets_dir(&self.project_name)
            .join("Stems");
        let base = super::import::stem_base_name(None, &track.name);
        let model = crate::audio::neural_separator::models()
            .get(self.stem_model)
            .unwrap_or_else(|| crate::audio::neural_separator::default_model());

        let progress: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
        let result: Arc<Mutex<Option<Result<SplitOutcome, String>>>> = Arc::new(Mutex::new(None));
        let progress_thread = progress.clone();
        let result_thread = result.clone();
        let (l, r) = (left.clone(), right.clone());

        std::thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let separation = crate::audio::stem_separator::run_separation_with(
                    &model,
                    &l,
                    &r,
                    sample_rate,
                    &|value| {
                        if let Ok(mut p) = progress_thread.lock() {
                            *p = value.clamp(0.0, 1.0);
                        }
                    },
                );
                let written = crate::audio::stem_separator::write_stems_and_read_envelopes(
                    &dir,
                    &base,
                    &separation.stems,
                    separation.sample_rate,
                    2048,
                )?;
                let mut parts = Vec::new();
                for (idx, (path, envelope)) in written.into_iter().enumerate() {
                    let Some(stem) = separation.stems.get(idx) else {
                        break;
                    };
                    let (name, icon, color, kind) = stem_identity(idx);
                    parts.push(StemPart {
                        name: name.to_string(),
                        icon,
                        color,
                        kind,
                        path: path.to_string_lossy().into_owned(),
                        left: Arc::new(stem.left.clone()),
                        right: Arc::new(stem.right.clone()),
                        sample_rate: separation.sample_rate,
                        envelope,
                        start_bar,
                        bars: length_bars.max(0.25),
                        volume: stem_volume(idx),
                        source_bpm,
                    });
                }
                Ok(SplitOutcome {
                    parts,
                    used_neural: separation.used_neural,
                    fallback: separation.fallback_reason.clone(),
                    model: separation.model_name.clone(),
                })
            }));
            let value = match outcome {
                Ok(inner) => inner,
                Err(_) => Err(crate::i18n::t("Separationen kraschade oväntat").to_string()),
            };
            if let Ok(mut slot) = result_thread.lock() {
                *slot = Some(value);
            }
        });

        self.stem_split = Some(StemSplitJob {
            track_index: track_idx,
            source: left,
            progress,
            result,
        });
        self.status_message = crate::tstatus!(
            "⏳ Separerar '{}' i bakgrunden — framsteget syns på spåret",
            self.playlist_tracks[track_idx].name
        );
        Ok(())
    }

    /// Framsteget för en rad, om det är den som separeras just nu.
    pub(crate) fn stem_split_progress(&self, track_index: usize) -> Option<f32> {
        let job = self.stem_split.as_ref()?;
        if job.track_index != track_index {
            return None;
        }
        job.progress.lock().ok().map(|p| *p)
    }

    /// Hämtar resultatet och lägger in stämmorna i stället för spåret. Ett anrop per bildruta.
    pub(crate) fn poll_stem_split(&mut self) {
        let Some(job) = self.stem_split.as_ref() else {
            return;
        };
        let taken = job.result.lock().ok().and_then(|mut slot| slot.take());
        let Some(result) = taken else {
            return;
        };
        let StemSplitJob {
            track_index,
            source,
            ..
        } = self.stem_split.take().expect("jobbet fanns nyss");

        let result = match result {
            Ok(outcome) => outcome,
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Separationen misslyckades: {}", e);
                return;
            }
        };
        if result.parts.is_empty() {
            self.status_message = crate::tstatus!("⚠ Separationen gav inga stämmor");
            return;
        }

        // **Samma spår som när jobbet startade?** Ett index kan ha flyttat sig medan tråden körde.
        let still_there = self
            .playlist_tracks
            .get(track_index)
            .and_then(|t| t.pcm_audio.as_ref().map(|(l, _, _)| Arc::ptr_eq(l, &source)))
            .unwrap_or(false);
        if !still_there {
            self.status_message = crate::tstatus!(
                "⚠ Spåret flyttades eller togs bort under separationen — stämmorna skrevs till disk men lades inte in"
            );
            return;
        }

        let original_name = self.playlist_tracks[track_index].name.clone();
        self.push_undo(&crate::tstatus!("Separera stämmor: {}", original_name));

        let mut next_id = self
            .playlist_tracks
            .iter()
            .flat_map(|t| t.regions.iter().map(|r| r.id))
            .max()
            .unwrap_or(0)
            + 1;
        let warp_mode = self.warp_mode;
        let inserted = result.parts.len();
        let mut new_tracks = Vec::with_capacity(inserted);
        for part in result.parts.iter() {
            let mut track = PlaylistTrack::new(part.name.clone(), part.icon, part.kind, part.color);
            track.volume = part.volume;
            track.regions = vec![AudioRegion {
                id: next_id,
                name: part.name.clone(),
                start_bar: part.start_bar,
                length_bars: part.bars,
                sample_offset_sec: 0.0,
                source_path: Some(part.path.clone()),
                waveform_peaks: part.envelope.clone(),
                volume: 1.0,
                fade_in_bars: 0.0,
                fade_out_bars: 0.0,
                muted: false,
                is_reverse: false,
                color: part.color,
                loop_length_bars: 0.0,
                source_bpm: part.source_bpm,
                tape: false,
                warp_mode,
            }];
            next_id += 1;
            // Ingen `waveform_cache` här: den är en nyckel till en delad cache, och regionens
            // egen `waveform_peaks` (2048 punkter ur den skrivna filen) räcker för att rita
            // vågformen i första bildrutan.
            track.pcm_audio = Some((
                part.left.clone(),
                part.right.clone(),
                part.sample_rate,
            ));
            new_tracks.push(track);
        }
        splice_stems(&mut self.playlist_tracks, track_index, new_tracks);

        // Spåren efter den här raden har flyttat ett steg: motorn måste få sina index igen,
        // annars spelar den före detta spår 5 ur spår 4:s buffer.
        for i in track_index..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
            self.sync_track_regions(i);
        }
        self.selected_timeline_track = self
            .selected_timeline_track
            .min(self.playlist_tracks.len().saturating_sub(1));

        // **DSP-vägen är inte samma sak som den neurala.** Står det inte, tror användaren att
        // stämmorna är modellseparation när de är spektral gissning.
        let backend = if result.used_neural {
            result.model.clone()
        } else {
            match &result.fallback {
                Some(reason) => crate::tstatus!("DSP — {}", reason),
                None => "DSP".to_string(),
            }
        };
        self.status_message = crate::tstatus!(
            "🎚 {} stämmor ur '{}' ({}) — originalet borttaget, Ctrl+Z tar tillbaka",
            inserted,
            original_name,
            backend
        );
    }
}

/// **Byter ut spåret mot sina stämmor, på samma plats.** En fri funktion, för det är hela
/// önskemålet: stämmorna ska ligga där originalet stod och originalet ska bort — inte läggas
/// sist, och inte lämna en lucka. Returnerar indexet för den sista insatta raden.
pub(crate) fn splice_stems(
    tracks: &mut Vec<PlaylistTrack>,
    index: usize,
    stems: Vec<PlaylistTrack>,
) -> usize {
    if index >= tracks.len() || stems.is_empty() {
        return index;
    }
    let count = stems.len();
    tracks.remove(index);
    for (offset, track) in stems.into_iter().enumerate() {
        tracks.insert(index + offset, track);
    }
    index + count - 1
}

/// Namn, ikon, färg och spårtyp per stämma, i den ordning separationen lämnar dem.
fn stem_identity(index: usize) -> (&'static str, &'static str, Color32, TrackKind) {
    let palette = crate::ui::theme::palette();
    match index {
        0 => (
            crate::i18n::t("🎤 Vocals"),
            "🎤",
            palette.track_rose(),
            TrackKind::VocalAudio,
        ),
        1 => (
            crate::i18n::t("🥁 Drums"),
            "🥁",
            palette.orange(),
            TrackKind::Drums,
        ),
        2 => (
            crate::i18n::t("🎸 Bass"),
            "🎸",
            palette.cyan(),
            TrackKind::Bassline,
        ),
        _ => (
            crate::i18n::t("🎹 Instruments"),
            "🎹",
            palette.green(),
            TrackKind::CustomAudio,
        ),
    }
}

/// Samma volymer som stämmixern använder (`install_separation`), så en separation låter likadant
/// oavsett vilken väg den kom in genom.
fn stem_volume(index: usize) -> f32 {
    [0.90, 0.95, 0.88, 0.82].get(index).copied().unwrap_or(0.9)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Ordningen är kontraktet.** Separationen lämnar vocals, drums, bass, instruments — i den
    /// ordningen. Glider den isär hamnar sången i trumspåret, och ingenting annat larmar.
    #[test]
    fn the_stem_order_matches_the_separator() {
        assert_eq!(stem_identity(0).0, "🎤 Vocals");
        assert_eq!(stem_identity(1).0, "🥁 Drums");
        assert_eq!(stem_identity(2).0, "🎸 Bass");
        assert_eq!(stem_identity(3).0, "🎹 Instruments");
        assert!(matches!(stem_identity(0).3, TrackKind::VocalAudio));
        assert!(matches!(stem_identity(1).3, TrackKind::Drums));
        assert!(matches!(stem_identity(2).3, TrackKind::Bassline));
        assert!(matches!(stem_identity(3).3, TrackKind::CustomAudio));
        // Fler stämmor än fyra (6-stämmorsmodellen) ska inte krascha: de blir "instruments".
        assert_eq!(stem_identity(5).0, "🎹 Instruments");
    }

    fn track(name: &str) -> PlaylistTrack {
        PlaylistTrack::new(name.to_string(), "🎚", TrackKind::CustomAudio, Color32::WHITE)
    }

    /// **Stämmorna tar originalets plats.** Inte sist i listan, inte med en lucka efter sig —
    /// och spåren runt omkring är orörda.
    #[test]
    fn the_stems_replace_the_original_where_it_stood() {
        let mut tracks = vec![track("A"), track("B (original)"), track("C")];
        let last = splice_stems(&mut tracks, 1, vec![track("B1"), track("B2"), track("B3"), track("B4")]);
        let names: Vec<&str> = tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B1", "B2", "B3", "B4", "C"]);
        assert_eq!(last, 4, "sista insatta raden är index 4");
        assert!(!names.iter().any(|n| n.contains("original")));
    }

    /// Ett index utanför listan eller tomma stämmor ska inte tappa ett spår.
    #[test]
    fn a_splice_that_cannot_happen_leaves_the_list_alone() {
        let mut tracks = vec![track("A")];
        assert_eq!(splice_stems(&mut tracks, 5, vec![track("B")]), 5);
        assert_eq!(tracks.len(), 1);
        assert_eq!(splice_stems(&mut tracks, 0, Vec::new()), 0);
        assert_eq!(tracks.len(), 1);
    }

    /// Samma volymer som `StemProject::install_separation` — en separation ska låta likadant
    /// oavsett vilken väg den kom in genom.
    #[test]
    fn the_stem_volumes_match_the_stem_mixer() {
        assert_eq!(stem_volume(0), 0.90);
        assert_eq!(stem_volume(1), 0.95);
        assert_eq!(stem_volume(2), 0.88);
        assert_eq!(stem_volume(3), 0.82);
    }
}
