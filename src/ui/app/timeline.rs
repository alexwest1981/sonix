//! Tidslinjen — klippens geometri, ångringen och klippoperationerna.
//!
//! Klippet är den enda sanningen om var ljudet ligger: \`AudioRegion\` med sin \`source_bpm\`,
//! sin längd i takter och sitt \`sample_offset_sec\`. Flyttar du en enhet (takter mot sekunder)
//! kompilerar det ändå — läs varje anropare och konvertera **en gång**, uttryckligt.
//!
//! Status: byggs — ångringen och klippoperationerna.
//! Rör inte: alla fält är \`pub\`; döp inte om ett fält som står i en projektfil.

use super::*;

/// Klippets ljud som editorn får det: namn, vänster, höger, samplingsfrekvens, starttakt.
type RegionAudio = (String, Vec<f32>, Vec<f32>, u32, f32);


/// Vad "sätt takt 1 här" gör med ett klipp (Fas 8.14).
///
/// Ljudet som ligger under spelhuvudet blir klippets **första sampel**. Klippet står
/// kvar där det står — **var** slaget ska landa är användarens val (flytta klippet
/// dit först), precis som i Abletons "Set 1.1.1 Here" — och det som ändras är
/// klippets innehåll: offsetten in i filen växer med sträckan fram till spelhuvudet,
/// och längden krymper lika mycket så att **högerkanten står still**.
///
/// Utan det här går en låt vars första slag inte ligger på takt 1 i filen inte att
/// få i takt med rutnätet: **inget tempo lagar det** (Alex 2026-09-13: "får inte
/// riktigt markören att matcha vågformerna oavsett bpm"). Manövern är densamma som
/// Audacitys kant-trim: början kapas, filen rörs inte, och en ångring tar tillbaka den.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipStartAlign {
    /// Nytt avstånd in i filen, i sekunder.
    pub sample_offset_sec: f32,
    /// Ny längd i takter, så att högerkanten hamnar där den var.
    pub length_bars: f32,
    /// Hur långt klippet flyttades in i filen, i sekunder (för beskedet i statusraden).
    pub moved_source_secs: f32,
}

/// Räknar fram [`ClipStartAlign`] — eller `None` när flytten inte går att göra.
///
/// **Ren funktion:** inget fönster, ingen motor, inget ljud. `None` betyder att
/// spelhuvudet står utanför klippets innehåll (före dess början eller så långt in att
/// klippet skulle försvinna). Då ska ingenting ändras och den som frågade ska få veta
/// varför — 8.5-regeln: hitta aldrig på ett ljud.
///
/// Faktorn är [`stretch_ratio_for`] = **källsekunder per utsekund**, samma tal motorn
/// spelar med. Flytten hamnar därför i filens tid och inte i tidslinjens: 0,30 s på
/// tidslinjen är 0,30 s av filen när tempot är detsamma, men 0,21 s av filen när
/// projektet står i 100 och klippet i 140.
pub fn align_clip_start_to_point(
    start_bar: f32,
    length_bars: f32,
    sample_offset_sec: f32,
    source_bpm: f32,
    point_sec: f64,
    tempo: &crate::audio::tempo::TempoMap,
) -> Option<ClipStartAlign> {
    let point_sec = point_sec.max(0.0);
    let start_sec = tempo.secs_at_bar(start_bar as f64);
    if point_sec <= start_sec {
        return None; // spelhuvudet står före klippets början: det finns inget att sätta
    }
    let head_secs = point_sec - start_sec; // ut-tid som kapas
    let ratio = stretch_ratio_for(source_bpm, tempo.bpm_at(start_bar as f64)) as f64;
    let new_offset = sample_offset_sec as f64 + head_secs * ratio;
    let new_length = length_bars as f64 - tempo.bars_for_secs_at(start_bar as f64, head_secs);
    if new_length < 0.01 {
        return None; // hela klippet skulle försvinna
    }
    Some(ClipStartAlign {
        sample_offset_sec: new_offset.max(0.0) as f32,
        length_bars: new_length as f32,
        moved_source_secs: (head_secs * ratio) as f32,
    })
}

/// Var på tidslinjen ett slag i **källan** ligger (Fas 8.14).
///
/// Omvändningen av [`align_clip_start_to_point`]: där *ges* punkten på tidslinjen,
/// här kommer den ur filens egen tid. Faktorn är [`stretch_ratio_for`] —
/// **källsekunder per utsekund** — så en sträckt fil räknas rätt: ett slag 0,2143 s in
/// i en källa som spelas i 100 av ett 140-klipp ligger 0,30 s in på tidslinjen.
///
/// `None` när slaget ligger **före** klippets första sampel: då finns det inget att
/// sätta, och då hittar vi inte på ett (8.5-regeln).
pub fn timeline_point_for_source_secs(
    onset_source_secs: f32,
    start_bar: f32,
    sample_offset_sec: f32,
    source_bpm: f32,
    tempo: &crate::audio::tempo::TempoMap,
) -> Option<f64> {
    let head_source = (onset_source_secs - sample_offset_sec) as f64;
    if head_source < 0.0 {
        return None;
    }
    let ratio = stretch_ratio_for(source_bpm, tempo.bpm_at(start_bar as f64)) as f64;
    if ratio <= 0.0 {
        return None;
    }
    Some(tempo.secs_at_bar(start_bar as f64) + head_source / ratio)
}

/// Ett klipps geometri — allt skiftet behöver veta, utan spår, fönster eller motor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipGeometry {
    pub start_bar: f32,
    pub length_bars: f32,
    pub sample_offset_sec: f32,
    pub source_bpm: f32,
}

/// Klippen som ska klippas: takt, kanal och hur starten justeras.
pub type TrimPlan = Vec<(usize, usize, ClipStartAlign)>;

/// Hur nära två klipp måste börja för att räknas som samma stämgrupp (Fas 8.15b).
///
/// En hundradels takt är det finaste rutnätet (`TimeSnapMode::FreeHundredth`). Klipp som
/// lagts på samma takt hamnar på exakt samma tal — toleransen finns bara för att flyttal
/// inte ska kunna sära på dem som hör ihop.
pub const STEM_GROUP_TOLERANCE_BARS: f32 = 0.01;

/// **Planen för hela stämgruppen** (Fas 8.15b): ett och samma skift i tid för varje klipp
/// som börjar på ankarets takt.
///
/// **Varför gruppen och inte klippet.** Ett importerat set (Suno-stämmor) läggs på samma
/// takt och *hör ihop* — stämmorna är sample-låsta mot varandra. Mätt i Alex' filer
/// 2026-09-13 låg bas och gitarr på 0 ms mot varandra (korrelation 0,71), medan ett ensamt
/// trumklipp som mätningen flyttade hamnade 0,60 s före resten: sången gick ur fas, och
/// han sa ifrån. Skiftet räknas därför i **tid**, inte i takter — med ett tempo som ändras
/// mitt i låten är det tiden som håller stämmorna ihop.
///
/// **Ren funktion:** geometri in, ny geometri ut. Indexen `(spår, klipp)` följer med
/// oförändrade så att anroparen kan skriva tillbaka dem; proven nedan behöver inga spår.
///
/// **Hela planen räknas fram innan något skrivs.** Ett klipp som inte kan flyttas (skiftet
/// skulle tömma det, eller det ligger före sin egen start) stoppar **allt** och namnges i
/// `Err((spår, klipp))` — en halvflyttad grupp är en sång ur fas, och det är värre än ingen
/// flytt alls.
pub fn plan_group_shift_by_head_secs(
    clips: &[(usize, usize, ClipGeometry)],
    anchor_bar: f32,
    head_secs: f64,
    tempo: &crate::audio::tempo::TempoMap,
) -> Result<TrimPlan, (usize, usize)> {
    let mut plan = Vec::new();
    if head_secs <= 0.0 {
        return Ok(plan);
    }
    for (t, r, g) in clips {
        if (g.start_bar - anchor_bar).abs() > STEM_GROUP_TOLERANCE_BARS {
            continue;
        }
        let target = tempo.secs_at_bar(g.start_bar as f64) + head_secs;
        match align_clip_start_to_point(
            g.start_bar,
            g.length_bars,
            g.sample_offset_sec,
            g.source_bpm,
            target,
            tempo,
        ) {
            Some(a) => plan.push((*t, *r, a)),
            None => return Err((*t, *r)),
        }
    }
    Ok(plan)
}

/// Vilket klipp en punkt i en lane träffar (Fas 8.15).
///
/// **Ren funktion, och en enda regel för två dörrar:** vänsterklickets dragstart och
/// högerklickets val frågar samma funktion. Utan det kunde "Radera region" träffa ett
/// annat klipp än det man pekade på — menyn gäller det *valda* klippet, och högerklick
/// valde inte.
///
/// Åtta pixlars marginal utanför kanterna är samma marginal som handtagen använder, så
/// att ett klipp som är några pixlar brett fortfarande går att peka på. Vid överlapp
/// vinner det **första** klippet i listan — samma ordning som dragstarten alltid har
/// haft.
pub fn region_under_x(
    regions: &[AudioRegion],
    mouse_x: f32,
    lane_min_x: f32,
    bar_w: f32,
) -> Option<usize> {
    const SLOP: f32 = 8.0;
    if bar_w <= 0.0 || !mouse_x.is_finite() {
        return None;
    }
    regions.iter().position(|r| {
        let x0 = lane_min_x + r.start_bar * bar_w;
        let x1 = x0 + r.length_bars * bar_w;
        mouse_x >= x0 - SLOP && mouse_x <= x1 + SLOP
    })
}

#[derive(Clone, Debug)]
pub struct TimelineUndoSnapshot {
    pub playlist_tracks: Vec<PlaylistTrack>,
    /// Mönstren med noterna och tagningen (Fas 6.4). Utan dem gick det inte att
    /// ångra en kvantisering: tagningen ligger i `patterns`, inte i spåren.
    pub patterns: Vec<Pattern>,
    /// Buss- och VCA-nivåer (Fas 6.2). De ligger utanför `playlist_tracks`, så
    /// utan dem kunde en bussändring inte ångras alls.
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    /// **Bussarnas egna kurvor** (Fas 8.8).
    pub bus_automation: Vec<BusAutomationLane>,
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    pub vca_faders: [f32; crate::audio::synth::NUM_VCAS],
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    pub vca_solos: [bool; crate::audio::synth::NUM_VCAS],
    /// **Det som hörs utanför spåren** (Fas 6.2): masterns kedja (delay, reverb, drive, volym,
    /// master-FX:et, pedalerna, EQ-noderna) och plugin-kurvorna. De låg utanför
    /// `playlist_tracks`, så en masterändring kunde inte ångras alls och en plugin-kurva
    /// ångrades i gränssnittet men inte i ljudet — se [`state::NonTrackSound`].
    pub non_track: state::NonTrackSound,
    pub selected_timeline_track: usize,
    pub selected_audio_region: Option<(usize, usize)>,
    pub song_time: f32,
    pub description: String,
}

impl SonixApp {
pub fn find_region_at_playhead(&self) -> Option<(usize, usize)> {
    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let current_playhead_bar = tempo.bar_at_secs(self.song_time as f64) as f32;

    // First check selected track
    if self.selected_timeline_track < self.playlist_tracks.len() {
        for (r_i, r) in self.playlist_tracks[self.selected_timeline_track].regions.iter().enumerate() {
            if current_playhead_bar >= r.start_bar && current_playhead_bar <= r.start_bar + r.length_bars {
                return Some((self.selected_timeline_track, r_i));
            }
        }
    }

    // Then check all tracks
    for (t_i, track) in self.playlist_tracks.iter().enumerate() {
        for (r_i, r) in track.regions.iter().enumerate() {
            if current_playhead_bar >= r.start_bar && current_playhead_bar <= r.start_bar + r.length_bars {
                return Some((t_i, r_i));
            }
        }
    }
    None
}
}

impl SonixApp {
/// Sekunder per takt vid projektets tempo — delningarna räknar i takter, ljudet i sekunder.
fn secs_per_bar(&self) -> f32 {
    crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_for_bars_at(0.0, 1.0) as f32
}

/// **Chop in bars:** delar valt klipp i bitar av `chunk_bars` takter.
pub fn chop_selected_region(&mut self, chunk_bars: f32) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        self.status_message = crate::tstatus!("Välj ett klipp först");
        return;
    };
    if t_idx >= self.playlist_tracks.len() {
        return;
    }
    let spb = self.secs_per_bar();
    let parts = self.playlist_tracks[t_idx].chop_region(r_idx, chunk_bars, spb);
    if parts > 1 {
        self.status_message = crate::tstatus!("✂ Delade klippet i {} delar", parts);
    } else {
        self.status_message = crate::tstatus!("Klippet är kortare än en del — inget att dela");
    }
    self.sync_track_regions(t_idx);
}

/// **Auto-slice:** hittar anslagen i klippets ljud och delar där. `sensitivity` är
/// tröskeln mot omgivningen — 1,5 är standard, lägre hittar svagare slag.
pub fn autoslice_selected_region(&mut self, sensitivity: f32) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        self.status_message = crate::tstatus!("Välj ett klipp först");
        return;
    };
    let Some(track) = self.playlist_tracks.get(t_idx) else {
        return;
    };
    let Some((pcm, _right, sr)) = track.pcm_audio.clone() else {
        self.status_message =
            crate::tstatus!("Auto-slice kräver att klippets ljud är inläst");
        return;
    };
    let Some(region) = track.regions.get(r_idx).cloned() else {
        return;
    };
    let params = crate::audio::onset::OnsetParams {
        sensitivity: sensitivity.clamp(1.05, 4.0),
        ..Default::default()
    };
    let onsets = crate::audio::onset::detect_onsets(&pcm, sr as f32, &params);
    let spb = self.secs_per_bar().max(1e-3);
    let points: Vec<f32> = onsets
        .iter()
        .map(|s| *s as f32 / sr.max(1) as f32 / spb - region.sample_offset_sec / spb)
        .filter(|p| *p > 1e-3 && *p < region.length_bars - 1e-3)
        .collect();
    let parts = self.playlist_tracks[t_idx].slice_region_at(r_idx, &points, spb);
    self.status_message = if parts > 1 {
        crate::tstatus!("🎯 Auto-slice hittade {} anslag", parts - 1)
    } else {
        crate::tstatus!("Inga anslag hittades — sänk känsligheten")
    };
    self.sync_track_regions(t_idx);
}

/// **Scale levels:** multiplicerar valt klipps nivå (FL:s `Scale levels…`).
pub fn scale_selected_region_level(&mut self, factor: f32) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        return;
    };
    if self.playlist_tracks[t_idx].scale_region_volume(r_idx, factor) {
        let db = 20.0 * factor.log10();
        self.status_message = crate::tstatus!("🔉 Klippets nivå {:+.1} dB", db);
        self.sync_track_regions(t_idx);
    }
}

pub fn split_selection_at_playhead(&mut self) {
    // **Flera markerade: dela var och en som spelhuvudet står inom.** En region som
    // inte når spelhuvudet lämnas orörd — annars skulle S kapa slumpmässiga kanter.
    if self.selected_regions.len() > 1 {
        let tempo = self.tempo_map();
        let bar = tempo.bar_at_secs(self.song_time as f64) as f32;
        let targets: Vec<(usize, usize)> = self
            .selected_regions
            .iter()
            .copied()
            .filter(|(t, r)| {
                self.playlist_tracks
                    .get(*t)
                    .and_then(|tr| tr.regions.get(*r))
                    .is_some_and(|reg| bar > reg.start_bar + 0.01 && bar < reg.start_bar + reg.length_bars - 0.01)
            })
            .collect();
        if targets.is_empty() {
            self.status_message = "✂️ Spelhuvudet står inte inom någon markerad region".to_string();
            return;
        }
        self.push_undo("Dela regioner vid spelhuvudet");
        let mut made = 0;
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for (t, r) in targets {
            let Some(reg) = self.playlist_tracks.get(t).and_then(|tr| tr.regions.get(r)).cloned() else {
                continue;
            };
            let end = reg.start_bar + reg.length_bars;
            let Some(right) = trimmed_region_copy(&reg, bar, end, &tempo, self.next_region_id()) else {
                continue;
            };
            let left = trimmed_region_copy(&reg, reg.start_bar, bar, &tempo, reg.id);
            if let Some(region) = self.playlist_tracks.get_mut(t).and_then(|tr| tr.regions.get_mut(r)) {
                match left {
                    Some(left) => *region = left,
                    None => region.length_bars = bar - reg.start_bar,
                }
            }
            self.playlist_tracks[t].regions.push(right);
            touched.insert(t);
            made += 1;
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.status_message = crate::tstatus!("✂️ Delade {} regioner vid spelhuvudet", made);
        return;
    }
}

pub fn split_selected_region_at_playhead(&mut self) {
    if self.selected_regions.len() > 1 {
        self.split_selection_at_playhead();
        return;
    }
    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let current_playhead_bar = tempo.bar_at_secs(self.song_time as f64) as f32;
    let cur_cut_sec = (self.song_time * 100.0).round() / 100.0;

    // Target region: either explicitly selected (and its target split point), or region under playhead, or selected track's region
    let target: Option<(usize, usize, f32)> = if let Some((t_idx, r_idx)) = self.selected_audio_region {
        if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            let orig = &self.playlist_tracks[t_idx].regions[r_idx];
            if current_playhead_bar >= orig.start_bar && current_playhead_bar <= (orig.start_bar + orig.length_bars) {
                Some((t_idx, r_idx, current_playhead_bar))
            } else if let Some((f_t, f_r)) = self.find_region_at_playhead() {
                Some((f_t, f_r, current_playhead_bar))
            } else {
                // Playhead is outside: split selected region at midpoint (50%)
                let mid_bar = orig.start_bar + orig.length_bars * 0.5;
                Some((t_idx, r_idx, mid_bar))
            }
        } else {
            self.find_region_at_playhead().map(|(t, r)| (t, r, current_playhead_bar))
        }
    } else if let Some((f_t, f_r)) = self.find_region_at_playhead() {
        Some((f_t, f_r, current_playhead_bar))
    } else if self.selected_timeline_track < self.playlist_tracks.len() && !self.playlist_tracks[self.selected_timeline_track].regions.is_empty() {
        let orig = &self.playlist_tracks[self.selected_timeline_track].regions[0];
        let mid_bar = orig.start_bar + orig.length_bars * 0.5;
        Some((self.selected_timeline_track, 0, mid_bar))
    } else {
        None
    };

    if let Some((t_idx, r_idx, cut_bar)) = target {
        if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
            let orig_start_sec = tempo.secs_at_bar(orig.start_bar as f64) as f32;
            let orig_len_sec =
                tempo.secs_for_bars_at(orig.start_bar as f64, orig.length_bars as f64) as f32;
            let raw_cut_sec = tempo.secs_at_bar(cut_bar as f64) as f32;
            let mut cut_sec = (raw_cut_sec * 100.0).round() / 100.0;

            // Ensure cut point is inside this region
            let min_cut = orig_start_sec + 0.02;
            let max_cut = orig_start_sec + orig_len_sec - 0.02;
            if cut_sec < min_cut || cut_sec > max_cut {
                cut_sec = orig_start_sec + orig_len_sec * 0.5;
            }

            let split_offset_sec = (cut_sec - orig_start_sec).clamp(0.01, orig_len_sec - 0.01);
            let split_offset_bar = (tempo.bars_for_secs_at(
                orig.start_bar as f64,
                split_offset_sec as f64,
            ) as f32
                * 100.0)
                .round()
                / 100.0;

            let split_points = if orig_len_sec > 0.001 {
                ((split_offset_sec / orig_len_sec) * orig.waveform_peaks.len() as f32) as usize
            } else {
                orig.waveform_peaks.len() / 2
            };
            let split_idx = split_points.clamp(1, orig.waveform_peaks.len().saturating_sub(1).max(1));
            let left_peaks = orig.waveform_peaks[..split_idx.min(orig.waveform_peaks.len())].to_vec();
            let right_peaks = orig.waveform_peaks[split_idx.min(orig.waveform_peaks.len())..].to_vec();

            let clean_title = orig.name
                .replace(" [Del 1]", "")
                .replace(" [Del 2]", "")
                .replace(" (Kopia)", "")
                .trim()
                .to_string();

            // Distinct alternating color for Part 2 so both slices stand out immediately
            let new_color = match orig.color {
                c if c == crate::ui::theme::palette().cyan() => crate::ui::theme::palette().orange(),
                c if c == crate::ui::theme::palette().orange() => crate::ui::theme::palette().green(),
                c if c == crate::ui::theme::palette().green() => crate::ui::theme::palette().purple(),
                c if c == crate::ui::theme::palette().purple() => crate::ui::theme::palette().track_blue(),
                _ => crate::ui::theme::palette().cyan(),
            };

            let r_left = AudioRegion {
                source_bpm: orig.source_bpm, // halvan ärver klippets källa
                tape: false,
                id: orig.id,
                name: format!("{} [Del 1]", clean_title),
                start_bar: orig.start_bar,
                length_bars: split_offset_bar,
                sample_offset_sec: orig.sample_offset_sec,
                source_path: orig.source_path.clone(),
                waveform_peaks: left_peaks.clone(),
                volume: orig.volume,
                fade_in_bars: orig.fade_in_bars.min(split_offset_bar * 0.5),
                fade_out_bars: 0.0,
                muted: orig.muted,
                is_reverse: orig.is_reverse,
                color: orig.color,
                loop_length_bars: 0.0,
                warp_mode: orig.warp_mode,
};

            let r_right = AudioRegion {
                source_bpm: orig.source_bpm, // halvan ärver klippets källa
                tape: false,
                id: orig.id + 1000 + (self.playlist_tracks[t_idx].regions.len() * 10),
                name: format!("{} [Del 2]", clean_title),
                start_bar: orig.start_bar + split_offset_bar,
                length_bars: ((orig.length_bars - split_offset_bar) * 100.0).round() / 100.0,
                sample_offset_sec: ((orig.sample_offset_sec + split_offset_sec) * 100.0).round() / 100.0,
                source_path: orig.source_path,
                waveform_peaks: if right_peaks.is_empty() { left_peaks } else { right_peaks },
                volume: orig.volume,
                fade_in_bars: 0.0,
                fade_out_bars: orig.fade_out_bars.min((orig.length_bars - split_offset_bar) * 0.5),
                muted: orig.muted,
                is_reverse: orig.is_reverse,
                color: new_color,
                loop_length_bars: 0.0,
                warp_mode: orig.warp_mode,
};

            self.push_undo(&format!("Klipp '{}' vid spelhuvud", clean_title));
            self.playlist_tracks[t_idx].regions.remove(r_idx);
            self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
            self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
            self.sync_track_regions(t_idx);
            self.selected_timeline_track = t_idx;
            self.selected_audio_region = Some((t_idx, r_idx + 1)); // Highlight Part 2
            self.status_message = crate::tstatus!("✂ Klippte '{}' vid {} (Takt {})! [Ångra: Ctrl+Z]", clean_title, format_time_hundredths(orig_start_sec + split_offset_sec), format_bar_subdivisions(orig.start_bar + split_offset_bar));
        }
    } else {
        self.status_message = crate::tstatus!("⚠ Inget ljudklipp markerat eller vid spelhuvudet ({}) att klippa.", format_time_hundredths(cur_cut_sec));
    }
}
}

impl SonixApp {
/// **Anpassa klippets längd** — sätt det tempo klippet ska spelas i så att det fyller
/// exakt `bars` takter. Tonhöjden bevaras (samma sträckning som tempoföljningen).
///
/// Längden räknas om ur det **uppnåeliga** resultatet, inte ur önskemålet: ligger målet
/// utanför sträckningens gränser blir klippet så långt ljudet räcker, i stället för att
/// sluta i tystnad.
pub fn fit_selected_region_to_bars(&mut self, bars: f32) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp först").to_string();
        return;
    };
    let Some(region) = self.playlist_tracks.get(t_idx).and_then(|t| t.regions.get(r_idx)) else {
        return;
    };
    if bars <= 0.0 {
        return;
    }
    let tempo = self.tempo_map();
    let bpm_here = tempo.bpm_at(region.start_bar as f64);
    let current_secs = tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32;
    let current_rate = stretch_ratio_for(region.source_bpm, bpm_here);
    let target_secs = tempo.secs_for_bars_at(region.start_bar as f64, bars as f64) as f32;
    let new_bpm = source_bpm_for_target_secs(bpm_here, current_secs, current_rate, target_secs);
    let new_rate = stretch_ratio_for(new_bpm, bpm_here);
    let audible = (current_secs * current_rate / new_rate).max(0.01);
    let new_bars = tempo.bars_for_secs_at(region.start_bar as f64, audible as f64) as f32;
    let name = region.name.clone();

    self.push_undo(crate::i18n::t("Anpassa klippets längd"));
    if let Some(region) = self.playlist_tracks.get_mut(t_idx).and_then(|t| t.regions.get_mut(r_idx)) {
        region.source_bpm = new_bpm;
        region.length_bars = new_bars.max(0.05);
    }
    self.sync_track_regions(t_idx);
    self.status_message = crate::tstatus!(
        "⌗ '{}' anpassat till {:.2} takter (källa {:.0} BPM, {:.2}×)",
        name,
        new_bars,
        new_bpm,
        new_rate
    );
}

/// Läser det valda klippets källjud till en egen stereobuffert — samma väg för Edison och
/// Newtone. `None` betyder att klippet saknar ljud eller är för kort att arbeta med.
fn selected_region_audio(&self) -> Option<RegionAudio> {
    let (t_idx, r_idx) = self.selected_audio_region?;
    let track = self.playlist_tracks.get(t_idx)?;
    let region = track.regions.get(r_idx)?.clone();

    // Klippets längd på tidslinjen och hur mycket källfilen är sträckt (Fas 8.10).
    let len_secs = self
        .tempo_map()
        .secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32;
    let rate = stretch_ratio_for(
        region.source_bpm,
        self.tempo_map().bpm_at(region.start_bar as f64),
    );

    let audio = if let Some((l, r, sr)) = self.playlist_tracks[t_idx].pcm_audio.clone() {
        Some((l, r, sr))
    } else if let Some(path) = region.source_path.clone() {
        load_audio_or_report(&path).map(|(l, r, sr)| {
            (std::sync::Arc::new(l), std::sync::Arc::new(r), sr)
        })
    } else {
        None
    };
    let (l, r, sr) = audio?;

    let start = ((region.sample_offset_sec.max(0.0) * sr as f32) as usize).min(l.len());
    let count = region_source_span_samples(len_secs, rate, sr)
        .min(l.len().saturating_sub(start))
        .min(r.len().saturating_sub(start));
    if count < 2 {
        return None;
    }
    Some((
        region.name.clone(),
        l[start..start + count].to_vec(),
        r[start..start + count].to_vec(),
        sr,
        region.start_bar,
    ))
}

/// **Edison:** kopierar in det valda klippets källjud i editorn.
///
/// Ingen fil skrivs och ingenting i projektet ändras här — rutan arbetar på en kopia,
/// och först *Skapa spår* lägger ett nytt spår bredvid. Källklippet kan alltså inte
/// förstöras av en redigering som blev fel.
pub fn open_edison_for_selected_region(&mut self) {
    let Some((name, l, r, sr, start_bar)) = self.selected_region_audio() else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp med ljud först").to_string();
        return;
    };
    self.edison = Some(crate::ui::edison_modal::EdisonState::new(name, l, r, sr, start_bar));
}

/// **Mät tempot i ett klipp som redan ligger på tidslinjen.**
///
/// Importvägen mäter numera själv, men klipp som kom in före dess har `source_bpm = 0` och står
/// still när projektets tempo ändras. Den här mäter om dem på plats — det är samma detektor, men
/// användaren bestämmer när den ska köras, och svaret syns i statusraden i stället för att gissas.
pub fn measure_selected_region_bpm(&mut self) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp med ljud först").to_string();
        return;
    };
    let Some((name, l, r, sr, _start_bar)) = self.selected_region_audio() else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp med ljud först").to_string();
        return;
    };
    match crate::audio::stem_separator::estimate_bpm(&l, &r, sr as f32) {
        bpm if bpm > 0.0 => {
            if let Some(region) = self
                .playlist_tracks
                .get_mut(t_idx)
                .and_then(|t| t.regions.get_mut(r_idx))
            {
                region.source_bpm = bpm;
            }
            self.sync_track_regions(t_idx);
            self.status_message =
                crate::tstatus!("🎵 '{}' går i {:.1} BPM — klippet följer nu tempot", name, bpm);
        }
        _ => {
            self.status_message = crate::tstatus!(
                "🎵 Hittade ingen takt i '{}' — klippet står still vid tempobyte",
                name
            );
        }
    }
}

/// **Newtone:** analyserar det valda klippets toner och öppnar tonhöjdseditorn.
pub fn open_newtone_for_selected_region(&mut self) {
    let Some((name, l, r, sr, start_bar)) = self.selected_region_audio() else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp med ljud först").to_string();
        return;
    };
    self.newtone = Some(crate::ui::newtone_modal::NewtoneState::new(name, l, r, sr, start_bar));
}

/// Skriver ett redigerat ljud som ett **nytt spår** — gemensam väg för Edison och Newtone.
///
/// Filen hamnar i projektets mapp och regionen pekar på den, så spåret finns kvar efter
/// omstart. Källklippet och dess fil rörs aldrig; returnerar `false` när inget skrevs.
pub fn add_edited_audio_track(
    &mut self,
    title: &str,
    left: Vec<f32>,
    right: Vec<f32>,
    sr: u32,
    start_bar: f32,
) -> bool {
    let frames = left.len().min(right.len());
    if frames < 2 {
        self.status_message = crate::i18n::t("Redigeringen är tom").to_string();
        return false;
    }
    let sr = sr.max(1);
    let dir = crate::paths::paths().project_assets_dir(&self.project_name);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!(
        "{}_edit_{}.wav",
        crate::autosave::slug(title),
        crate::autosave::now_stamp()
    ));
    let path_str = path.to_string_lossy().to_string();
    if let Err(e) = crate::audio::write_stem_wav(&path_str, &left[..frames], &right[..frames], sr) {
        self.status_message = crate::tstatus!("⚠️ Kunde inte skriva filen: {}", e);
        return false;
    }

    self.push_undo(crate::i18n::t("Redigerat klipp"));
    let name = format!("{} (redigerad)", title);
    let secs = frames as f32 / sr as f32;
    let bars = self
        .tempo_map()
        .bars_for_secs_at(start_bar as f64, secs as f64) as f32;
    let next_id = self
        .playlist_tracks
        .iter()
        .flat_map(|t| t.regions.iter())
        .map(|r| r.id)
        .max()
        .unwrap_or(0)
        + 1;
    let color = crate::ui::theme::palette().purple();
    let mut track = PlaylistTrack::new(name.clone(), "✂", TrackKind::CustomAudio, color);
    track.pcm_audio = Some((std::sync::Arc::new(left), std::sync::Arc::new(right), sr));
    track.regions = vec![AudioRegion {
        id: next_id,
        name: name.clone(),
        start_bar,
        length_bars: bars.max(0.25),
        sample_offset_sec: 0.0,
        source_path: Some(path_str),
        waveform_peaks: Vec::new(),
        volume: 1.0,
        fade_in_bars: 0.0,
        fade_out_bars: 0.0,
        muted: false,
        is_reverse: false,
        color,
        loop_length_bars: 0.0,
        // Redigeringen är rå ljuddata: okänt tempo betyder att tempot inte sträcker den.
        source_bpm: 0.0,
        tape: false,
        warp_mode: Default::default(),
    }];
    self.playlist_tracks.push(track);
    let idx = self.playlist_tracks.len() - 1;
    self.sync_track_stem_to_engine(idx);
    self.sync_track_regions(idx);
    self.selected_timeline_track = idx;
    self.status_message = crate::tstatus!("✂ '{}' skapad ur redigeringen ({:.1} s)", name, secs);
    true
}

/// **Newtime:** öppnar tidseditorn för det valda klippet.
pub fn open_newtime_for_selected_region(&mut self) {
    let Some((name, l, r, sr, start_bar)) = self.selected_region_audio() else {
        self.status_message = crate::i18n::t("Välj ett ljudklipp med ljud först").to_string();
        return;
    };
    self.newtime = Some(crate::ui::newtime_modal::NewtimeState::new(name, l, r, sr, start_bar));
}

/// Newtime: sträcker om ljudet med bevarad tonhöjd och lägger det som ett nytt spår.
pub fn newtime_create_track(&mut self) {
    let Some(state) = self.newtime.take() else { return };
    if (state.ratio() - 1.0).abs() < 1e-3 && state.warp.markers.is_empty() {
        self.status_message = crate::i18n::t("Tiden är oförändrad — inget att skapa").to_string();
        return;
    }
    let (l, r) = state.render();
    self.add_edited_audio_track(&state.title, l, r, state.sample_rate, state.start_bar);
}

/// Edison: skapar spåret av det redigerade ljudet.
pub fn edison_create_track(&mut self) {
    let Some(state) = self.edison.take() else { return };
    self.add_edited_audio_track(&state.title, state.work.0, state.work.1, state.sample_rate, state.start_bar);
}

/// Newtone: skriver den tonhöjdskorrigerade kopian som ett nytt spår.
pub fn newtone_create_track(&mut self) {
    let Some(state) = self.newtone.take() else { return };
    let (l, r) = state.render_corrected();
    self.add_edited_audio_track(&state.title, l, r, state.sample_rate, state.start_bar);
}

pub fn save_region_to_sound_browser(&mut self, track_idx: usize, region_idx: usize) {
    if track_idx >= self.playlist_tracks.len() || region_idx >= self.playlist_tracks[track_idx].regions.len() {
        return;
    }

    let region = self.playlist_tracks[track_idx].regions[region_idx].clone();
    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let reg_len_sec =
        (tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32)
            .max(0.05);
    let reg_offset_sec = region.sample_offset_sec.max(0.0);
    // Klippet kan vara sträckt (Fas 8.10): utsnittet som kopieras är det
    // vågformen visar, inte regionens tid på tidslinjen.
    // **Tempot där klippet ligger**, inte ett tal för hela projektet (8.10 punkt 1). Över ett
    // tempobyte är "projektets bpm" inte ett tal: klippet ska följa tempot på sin egen plats.
    // För ett projekt med ett enda tempo ger `bpm_at` exakt samma tal som förut.
    let reg_rate = stretch_ratio_for(
        region.source_bpm,
        self.tempo_map().bpm_at(region.start_bar as f64),
    );

    // Destination: the canonical user sample bank (Fas 6.0).
    let save_dir = crate::paths::paths().samples_dir();
    let _ = std::fs::create_dir_all(&save_dir);

    // Sanitize name for file
    let clean_name = region.name
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "_")
        .trim()
        .to_string();
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let filename = format!("{}_{}.wav", clean_name, timestamp);
    let dest_path = save_dir.join(&filename);
    let dest_path_str = dest_path.to_string_lossy().to_string();

    // Extract PCM audio
    let sr: u32 = 44100;
    let mut out_pcm: Vec<f32> = Vec::new();

    if let Some((ref l, ref r, track_sr)) = self.playlist_tracks[track_idx].pcm_audio {
        let start_sample = ((reg_offset_sec * track_sr as f32) as usize).min(l.len());
        let num_samples = region_source_span_samples(reg_len_sec, reg_rate, track_sr)
            .min(l.len().saturating_sub(start_sample));
        if num_samples > 0 {
            out_pcm.reserve(num_samples);
            for i in 0..num_samples {
                let s_l = l.get(start_sample + i).copied().unwrap_or(0.0);
                let s_r = r.get(start_sample + i).copied().unwrap_or(s_l);
                out_pcm.push((s_l + s_r) * 0.5 * region.volume);
            }
        }
    } else if let Some(ref src_p) = region.source_path
        && let Some((l, r, track_sr)) = load_audio_or_report(src_p) {
            let start_sample = ((reg_offset_sec * track_sr as f32) as usize).min(l.len());
            let num_samples = region_source_span_samples(reg_len_sec, reg_rate, track_sr)
                .min(l.len().saturating_sub(start_sample));
            if num_samples > 0 {
                out_pcm.reserve(num_samples);
                for i in 0..num_samples {
                    let s_l = l.get(start_sample + i).copied().unwrap_or(0.0);
                    let s_r = r.get(start_sample + i).copied().unwrap_or(s_l);
                    out_pcm.push((s_l + s_r) * 0.5 * region.volume);
                }
            }
    }

    // Fallback: If no raw PCM was cached, synthesize wave from waveform peaks
    if out_pcm.is_empty() {
        let total_samples = (reg_len_sec * sr as f32) as usize;
        out_pcm = Vec::with_capacity(total_samples);
        let num_peaks = region.waveform_peaks.len().max(1);
        for i in 0..total_samples {
            let p_idx = ((i as f32 / total_samples as f32) * num_peaks as f32) as usize;
            let amp = region.waveform_peaks.get(p_idx).copied().unwrap_or(0.5);
            let t = i as f32 / sr as f32;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin() * amp * region.volume;
            out_pcm.push(sample);
        }
    }

    // Write WAV file to disk
    let write_res = crate::audio::write_pcm_f32_to_wav(&dest_path_str, &out_pcm, sr, 1);
    if write_res.is_err() {
        let rel_path = format!("sample_{}_{}.wav", clean_name, timestamp);
        let _ = crate::audio::write_pcm_f32_to_wav(&rel_path, &out_pcm, sr, 1);
    }

    // Generate 32-point mini preview waveform
    let mut mini_wf = Vec::with_capacity(32);
    let chunk_sz = (out_pcm.len() / 32).max(1);
    for chunk in out_pcm.chunks(chunk_sz).take(32) {
        let max_val = chunk.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
        mini_wf.push(max_val.clamp(0.05, 1.0));
    }
    while mini_wf.len() < 32 { mini_wf.push(0.1); }

    let item_id = self.sample_library.len() + 1;
    let item = LibrarySampleItem {
        id: item_id,
        name: region.name.clone(),
        category: "Egna Samples".to_string(),
        icon: "🎵".to_string(),
        default_note: 60,
        color: region.color,
        waveform: mini_wf,
        file_path: Some(dest_path_str.clone()),
    };

    self.sample_library.push(item);
    self.status_message = crate::tstatus!("💾 Sparade sample '{}' till Sound Browser! Fil: {}", region.name, filename);
}
}

impl SonixApp {
pub fn add_sample_item_to_timeline(&mut self, item: &LibrarySampleItem) {
    let mut track = PlaylistTrack::new(format!("🎵 {}", item.name), "🎵", TrackKind::CustomAudio, item.color);
    track.volume = 0.90;

    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    let Some(ref path) = item.file_path else {
        self.status_message =
            crate::tstatus!("⚠ '{}' har ingen ljudfil — läggs inte till", item.name);
        return;
    };
    // Klippet får inte skapas utan sitt ljud. Den här vägen hittade på en längd
    // (fyra takter) när avkodningen misslyckades och skapade regionen ändå, med
    // bibliotekets grova översikt som vågform och originalfilen som källa. Kunde
    // filen inte läsas — t.ex. en mp3, som appen inte kan avkoda — blev klippet
    // tyst men ritades som om det hade ljud. Det var precis Alex' korta klipp.
    let Some((l, r, sr)) = load_audio_or_report(path) else {
        self.status_message = crate::tstatus!(
            "⚠ Kunde inte läsa '{}' — läggs inte till på tidslinjen (mp3 stöds inte, konvertera till wav)",
            item.name
        );
        return;
    };
    let duration_secs = l.len() as f32 / sr as f32;
    let pcm_opt = Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr));
    let reg_len_bars = (tempo.bars_for_secs_at(0.0, duration_secs as f64) as f32).max(0.25);

    let region = AudioRegion {
        source_bpm: 0.0, // bibliotekssamplens tempo är okänt → rör inte ljudet (8.10)
        tape: false,
        id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize,
        name: item.name.clone(),
        start_bar: 0.0,
        length_bars: reg_len_bars,
        sample_offset_sec: 0.0,
        source_path: item.file_path.clone(),
        waveform_peaks: item.waveform.clone(),
        volume: 1.0,
        fade_in_bars: 0.0,
        fade_out_bars: 0.0,
        muted: false,
        is_reverse: false,
        color: item.color,
        loop_length_bars: reg_len_bars,
        warp_mode: self.warp_mode,
};

    track.regions.push(region);

    if let Some((pcm_l, pcm_r, sr)) = pcm_opt {
        track.pcm_audio = Some((pcm_l, pcm_r, sr));
    }

    self.playlist_tracks.push(track);
    let new_t_idx = self.playlist_tracks.len() - 1;
    self.sync_track_regions(new_t_idx);
    self.selected_timeline_track = new_t_idx;
    self.selected_audio_region = Some((new_t_idx, 0));
    self.status_message = crate::tstatus!("➕ Lade till '{}' som nytt ljudspår på tidslinjen ({:.1} takter)!", item.name, reg_len_bars);
}
}

impl SonixApp {
pub fn add_sample_item_to_track_at_bar(&mut self, track_idx: usize, item: &LibrarySampleItem, start_bar: f32) {
    // Guarden ligger FÖRE spårskapandet nedan: annars skulle ett tomt spår bli
    // kvar när ljudet inte kan läsas. Klippet får inte skapas utan sitt ljud —
    // den här vägen hittade på en längd (fyra takter) när avkodningen misslyckades
    // och lade regionen på spåret ändå, med bibliotekets grova översikt som vågform
    // och originalfilen som källa. En mp3, som appen inte kan avkoda, blev ett tyst
    // klipp som ritades som om det hade ljud. Det var precis Alex' korta klipp.
    let Some(ref path) = item.file_path else {
        self.status_message =
            crate::tstatus!("⚠ '{}' har ingen ljudfil — läggs inte till", item.name);
        return;
    };
    let Some((l, r, sr)) = load_audio_or_report(path) else {
        self.status_message = crate::tstatus!(
            "⚠ Kunde inte läsa '{}' — läggs inte till på tidslinjen (mp3 stöds inte, konvertera till wav)",
            item.name
        );
        return;
    };
    let duration_secs = l.len() as f32 / sr as f32;
    let pcm_opt = Some((std::sync::Arc::new(l), std::sync::Arc::new(r), sr));

    if track_idx >= self.playlist_tracks.len() {
        let mut track = PlaylistTrack::new(format!("🎵 {}", item.name), "🎵", TrackKind::CustomAudio, item.color);
        track.volume = 0.90;
        self.playlist_tracks.push(track);
    }

    let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
    // Längden mäts från takten där klippet hamnar, inte från noll: över ett
    // tempobyte är antalet takter inte samma sak beroende på var man mäter.
    let reg_len_bars =
        (tempo.bars_for_secs_at(start_bar as f64, duration_secs as f64) as f32).max(0.25);

    let reg_id = self.next_region_id();
    let region = AudioRegion {
        source_bpm: 0.0, // bibliotekssamplens tempo är okänt → rör inte ljudet (8.10)
        tape: false,
        id: reg_id,
        name: item.name.clone(),
        start_bar: start_bar.max(0.0),
        length_bars: reg_len_bars,
        sample_offset_sec: 0.0,
        source_path: item.file_path.clone(),
        waveform_peaks: item.waveform.clone(),
        volume: 1.0,
        fade_in_bars: 0.0,
        fade_out_bars: 0.0,
        muted: false,
        is_reverse: false,
        color: item.color,
        loop_length_bars: reg_len_bars,
        warp_mode: self.warp_mode,
};

    let target_track = &mut self.playlist_tracks[track_idx];
    if target_track.pcm_audio.is_none()
        && let Some((pcm_l, pcm_r, sr)) = pcm_opt {
            target_track.pcm_audio = Some((pcm_l, pcm_r, sr));
        }

    target_track.regions.push(region);
    let new_reg_idx = target_track.regions.len() - 1;
    self.sync_track_regions(track_idx);
    self.selected_timeline_track = track_idx;
    self.selected_audio_region = Some((track_idx, new_reg_idx));
    self.status_message = crate::tstatus!("🎵 Placerade sample '{}' på spår {} vid takt {:.2}!", item.name, track_idx + 1, start_bar + 1.0);
}
}

impl SonixApp {
pub fn open_region_in_vocal_studio(&mut self, track_idx: usize, region_idx: usize) {
    if track_idx >= self.playlist_tracks.len() || region_idx >= self.playlist_tracks[track_idx].regions.len() {
        self.view_mode = ViewMode::VocalStudio;
        return;
    }
    let reg = &self.playlist_tracks[track_idx].regions[region_idx];
    let r_name = reg.name.clone();
    let r_color = reg.color;
    let r_source = reg.source_path.clone();

    let mut extracted_pcm = Vec::new();
    let mut sr = 44100;

    if let Some((ref l, _, srate)) = self.playlist_tracks[track_idx].pcm_audio {
        sr = srate;
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let start_sec = tempo.secs_at_bar(reg.start_bar as f64) as f32 + reg.sample_offset_sec;
        let len_sec =
            tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32;
        let start_idx = (start_sec * sr as f32) as usize;
        // Sträckt kloss (Fas 8.10): utsnittet är regionens tid gånger faktorn,
        // alltså samma stycke som vågformen visar.
        // Faktorn läses **där klippet ligger** (8.10 punkt 1): `tempo` ovan är en karta för ett
        // enda tempo, och över ett tempobyte skulle det talet vara fel för allt efter bytet.
        let end_idx = ((start_sec
            + len_sec * stretch_ratio_for(reg.source_bpm, self.tempo_map().bpm_at(reg.start_bar as f64)))
            * sr as f32) as usize;
        if start_idx < l.len() {
            extracted_pcm = l[start_idx..end_idx.min(l.len())].to_vec();
        }
    } else if let Some(ref path) = r_source && let Some((l, _, srate)) = load_audio_or_report(path) {
        sr = srate;
        extracted_pcm = l;
    }

    if extracted_pcm.is_empty() {
        // Ingen påhittad ton. Här låg förut en syntetisk 220 Hz-sinuston som
        // öppnades som en tagning: den som lyssnade hörde *något*, och det var
        // varken regionen eller källfilen. Ett spår vars källa är en mp3, och
        // som inte har någon inläst PCM än, gav en påkittad tagning i stället
        // för ett besked.
        let reason = if r_source.is_some() {
            crate::i18n::t("källfilen går inte att läsa (mp3 stöds inte, konvertera till wav)")
        } else {
            crate::i18n::t("regionen har ingen källfil")
        };
        self.status_message = crate::tstatus!(
            "⚠ Kunde inte öppna '{}' i Sångstudion — {}",
            r_name,
            reason
        );
        return;
    }

    let new_take_idx = self.vocal_studio.load_sample_or_region_as_take(&r_name, extracted_pcm, sr, r_color);
    self.view_mode = ViewMode::VocalStudio;
    self.status_message = crate::tstatus!("🎙 Öppnade region '{}' i Sångstudion (Tagning {})!", r_name, new_take_idx + 1);
}
}

impl SonixApp {
/// Bygger en ångringspunkt av läget just nu (Fas 6.2: mixern ingår).
pub(crate) fn current_snapshot(&self, description: &str) -> TimelineUndoSnapshot {
    TimelineUndoSnapshot {
        playlist_tracks: self.playlist_tracks.clone(),
        patterns: self.patterns.clone(),
        bus_volume: self.bus_volume,
        bus_muted: self.bus_muted,
        bus_automation: self.bus_automation.clone(),
        bus_solo: self.bus_solo,
        vca_faders: self.vca_faders,
        vca_muted: self.vca_muted,
        vca_solos: self.vca_solos,
        non_track: self.non_track_sound(),
        selected_timeline_track: self.selected_timeline_track,
        selected_audio_region: self.selected_audio_region,
        song_time: self.song_time,
        description: description.to_string(),
    }
}
}

impl SonixApp {
/// Mixerns ljudbild just nu — se [`mixer_digest`].
pub(crate) fn mixer_state_digest(&self) -> u64 {
    mixer_digest(
        &self.playlist_tracks,
        &self.bus_volume,
        &self.bus_muted,
        &self.bus_solo,
        &self.vca_faders,
        &self.vca_muted,
        &self.vca_solos,
        &self.non_track_sound(),
    )
}
}

impl SonixApp {
/// Lägger en färdig ångringspunkt på historiken.
pub(crate) fn push_undo_snapshot(&mut self, snapshot: TimelineUndoSnapshot) {
    self.undo_stack.push(snapshot);
    if self.undo_stack.len() > 60 {
        self.undo_stack.remove(0);
    }
    self.redo_stack.clear();
    // Ändringen skrivs av autosaven vid frame-gränsen (se `update`).
    self.autosave_pending = true;
}
}

impl SonixApp {
pub fn push_undo(&mut self, description: &str) {
    let snapshot = self.current_snapshot(description);
    self.push_undo_snapshot(snapshot);
}
}

impl SonixApp {
/// Lägger tillbaka ett snapshot i appens state **och** i ljudmotorn, så att
/// en ångring hörs och inte bara syns (Fas 6.2). `sync_track_regions` räcker
/// inte: volym, pan, mute/solo, EQ, kompressor, sends och routing ligger i
/// `sync_track_audio_state`, och buss/VCA i `sync_group_state`.
fn restore_snapshot(&mut self, snapshot: &TimelineUndoSnapshot) {
    self.playlist_tracks = snapshot.playlist_tracks.clone();
    // Mönstren först, sedan UI-spegeln: `load_pattern_into_ui` skriver inte
    // över `patterns` med det gamla UI-läget, vilket `select_pattern` hade gjort.
    self.patterns = snapshot.patterns.clone();
    self.load_pattern_into_ui(self.selected_pattern);
    self.bus_volume = snapshot.bus_volume;
    self.bus_muted = snapshot.bus_muted;
    self.bus_automation = snapshot.bus_automation.clone();
    self.bus_solo = snapshot.bus_solo;
    self.vca_faders = snapshot.vca_faders;
    self.vca_muted = snapshot.vca_muted;
    self.vca_solos = snapshot.vca_solos;
    // **Mastern och plugin-kurvorna** (Fas 6.2): `apply_non_track_sound` nollar kurvornas
    // `last_sent` och skickar hela kedjan till motorn, så ångringen **hörs** och inte bara syns.
    self.apply_non_track_sound(&snapshot.non_track);
    self.selected_timeline_track = snapshot.selected_timeline_track;
    self.selected_audio_region = snapshot.selected_audio_region;
    self.song_time = snapshot.song_time;

    for t_idx in 0..self.playlist_tracks.len() {
        self.sync_track_regions(t_idx);
        self.sync_track_audio_state(t_idx);
    }
    self.sync_group_state();
    // Historiens läge är nu "viloläge" för mixer-undon, annars skulle
    // ångringen själv registreras som en ny mixerändring.
    self.mixer_settled_digest = self.mixer_state_digest();
}
}

impl SonixApp {
pub fn undo(&mut self) {
    if let Some(snapshot) = self.undo_stack.pop() {
        let current = self.current_snapshot(&snapshot.description);
        self.redo_stack.push(current);

        self.restore_snapshot(&snapshot);
        // Även en ångring är en strukturell ändring som ska skyddas.
        self.autosave_pending = true;
        self.status_message = crate::tstatus!("↶ Ångrade: {} (Ctrl+Z)", snapshot.description);
    } else {
        self.status_message = crate::i18n::t("Ingenting att ångra.").to_string();
    }
}
}

impl SonixApp {
pub fn redo(&mut self) {
    if let Some(snapshot) = self.redo_stack.pop() {
        let current = self.current_snapshot(&snapshot.description);
        self.undo_stack.push(current);

        self.restore_snapshot(&snapshot);
        // Även en omgörning ändrar projektet — skyddas på samma sätt.
        self.autosave_pending = true;
        self.status_message = crate::tstatus!("↷ Gjorde om: {} (Ctrl+Y)", snapshot.description);
    } else {
        self.status_message = crate::i18n::t("Ingenting att göra om.").to_string();
    }
}
}

impl SonixApp {
pub fn next_region_id(&self) -> usize {
    let max_id = self.playlist_tracks.iter()
        .flat_map(|t| t.regions.iter())
        .map(|r| r.id)
        .max()
        .unwrap_or(100);
    max_id + 1
}
}

impl SonixApp {
pub fn duplicate_track(&mut self, track_idx: usize) {
    if track_idx >= self.playlist_tracks.len() {
        return;
    }
    let orig_name = self.playlist_tracks[track_idx].name.clone();
    ui_dbg(&format!(
        "duplicate_track idx={} name='{}' playing={} tracks_before={}",
        track_idx,
        orig_name,
        self.is_playing,
        self.playlist_tracks.len()
    ));
    self.push_undo(&crate::tstatus!("Duplicera spår '{}'", orig_name));

    let mut new_track = self.playlist_tracks[track_idx].clone();
    let clean_name = orig_name.replace(" (Kopia)", "");
    new_track.name = format!("{} (Kopia)", clean_name);
    for (i, r) in new_track.regions.iter_mut().enumerate() {
        r.id = self.next_region_id() + i * 10;
    }

    let insert_idx = track_idx + 1;
    self.playlist_tracks.insert(insert_idx, new_track);
    self.ensure_mic_track_exists();

    // Refresh all tracks in audio engine
    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    for i in 0..self.playlist_tracks.len() {
        self.sync_track_stem_to_engine(i);
        self.sync_track_audio_state(i);
        self.sync_track_regions(i);
    }

    self.selected_timeline_track = insert_idx.min(self.playlist_tracks.len().saturating_sub(1));
    if !self.playlist_tracks[self.selected_timeline_track].regions.is_empty() {
        self.selected_audio_region = Some((self.selected_timeline_track, 0));
    } else {
        self.selected_audio_region = None;
    }

    self.status_message = crate::tstatus!("📋 Duplicerade spår '{}' under originalspåret!", orig_name);
    ui_dbg(&format!(
        "duplicate_track done name='{}' tracks_after={} playing={} selected={}",
        orig_name,
        self.playlist_tracks.len(),
        self.is_playing,
        self.selected_timeline_track
    ));
}
}

impl SonixApp {
pub fn delete_track(&mut self, track_idx: usize) {
    if track_idx >= self.playlist_tracks.len() || self.playlist_tracks.len() <= 1 {
        return;
    }
    let orig_name = self.playlist_tracks[track_idx].name.clone();
    if orig_name.to_lowercase().contains("mic") && self.playlist_tracks.iter().filter(|t| t.name.to_lowercase().contains("mic")).count() <= 1 {
        self.status_message = crate::i18n::t("⚠️ Mikrofonspåret kan inte tas bort.").to_string();
        return;
    }
    self.push_undo(&crate::tstatus!("Ta bort spår '{}'", orig_name));
    self.playlist_tracks.remove(track_idx);
    self.ensure_mic_track_exists();

    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    for i in 0..self.playlist_tracks.len() {
        self.sync_track_stem_to_engine(i);
        self.sync_track_audio_state(i);
        self.sync_track_regions(i);
    }
    self.selected_timeline_track = track_idx.min(self.playlist_tracks.len().saturating_sub(1));
    self.selected_audio_region = None;
    self.status_message = crate::tstatus!("🗑 Raderade spår '{}'.", orig_name);
}
}

impl SonixApp {
    /// Vanligt klick på ett spår: bara det.
    pub fn select_only_track(&mut self, idx: usize) {
        self.selected_timeline_track = idx;
        self.selected_tracks.clear();
        self.selected_tracks.insert(idx);
        self.selected_regions.clear();
        self.selected_audio_region = None;
    }

    /// Klick på ett spår med Ctrl (växla) eller Shift (spann).
    pub fn click_track(&mut self, idx: usize, shift: bool, toggle: bool) {
        self.selected_tracks = crate::ui::app::state::clicked_selection(
            &self.selected_tracks,
            self.selected_timeline_track,
            idx,
            shift,
            toggle,
        );
        // Den primära posten ska alltid vara markerad; växlar man bort den blir
        // markeringens första spår primärt i stället.
        self.selected_timeline_track = if self.selected_tracks.contains(&idx) {
            idx
        } else {
            *self.selected_tracks.iter().next().unwrap_or(&idx)
        };
        self.selected_regions.clear();
        self.selected_audio_region = None;
    }

    /// Klick på en region: regionen och dess spår. Ctrl växlar, Shift tar spannet på
    /// samma spår — ett spann över spårgränser är svårt att förutse med musen.
    /// En grupperad region tar hela sin grupp med sig.
    pub fn click_region(&mut self, t: usize, r: usize, shift: bool, toggle: bool) {
        self.selected_timeline_track = t;
        self.selected_tracks.insert(t);
        let clicked_id = self
            .playlist_tracks
            .get(t)
            .and_then(|tr| tr.regions.get(r))
            .map(|reg| reg.id);
        if toggle {
            let was_selected = self.selected_regions.contains(&(t, r));
            if was_selected {
                self.selected_regions.remove(&(t, r));
                if let Some(id) = clicked_id {
                    for member in self.group_members_of(id) {
                        self.selected_regions.remove(&member);
                    }
                }
            } else {
                self.selected_regions.insert((t, r));
                if let Some(id) = clicked_id {
                    for member in self.group_members_of(id) {
                        self.selected_regions.insert(member);
                    }
                }
            }
            if self.selected_regions.is_empty() {
                self.selected_regions.insert((t, r));
            }
        } else if shift {
            match self.selected_audio_region.filter(|(pt, _)| *pt == t) {
                Some((_, pr)) => {
                    let (lo, hi) = if pr <= r { (pr, r) } else { (r, pr) };
                    self.selected_regions.clear();
                    for i in lo..=hi {
                        self.selected_regions.insert((t, i));
                    }
                }
                None => {
                    self.selected_regions.clear();
                    self.selected_regions.insert((t, r));
                }
            }
            self.selected_regions = self.expand_with_item_groups(&self.selected_regions);
        } else {
            self.selected_tracks.clear();
            self.selected_tracks.insert(t);
            self.selected_regions = std::iter::once((t, r)).collect();
            self.selected_regions = self.expand_with_item_groups(&self.selected_regions);
        }
        self.selected_audio_region = Some((t, r));
    }

    /// Håller markeringen inom projektet: borttagna spår och regioner försvinner, och den
    /// primära posten finns alltid med. Körs en gång per bildruta i stället för att jaga
    /// varje plats som kan ändra listorna.
    pub fn prune_selection(&mut self) {
        let track_count = self.playlist_tracks.len();
        if track_count == 0 {
            self.selected_tracks.clear();
            self.selected_regions.clear();
            self.selected_audio_region = None;
            return;
        }
        if self.selected_timeline_track >= track_count {
            self.selected_timeline_track = track_count - 1;
        }
        self.selected_tracks.retain(|t| *t < track_count);
        self.selected_tracks.insert(self.selected_timeline_track);
        let tracks = &self.playlist_tracks;
        self.selected_regions
            .retain(|(t, r)| tracks.get(*t).is_some_and(|tr| *r < tr.regions.len()));
        if let Some((t, r)) = self.selected_audio_region
            && tracks.get(t).is_none_or(|tr| r >= tr.regions.len())
        {
            self.selected_audio_region = None;
        }
    }

    /// Klistrar in en klunga på `base_track` vid `paste_bar`. Klungan behåller sitt
    /// inbördes avstånd i tid och spår; delar utan plats hoppas över.
    fn paste_cluster(&mut self, clip: &crate::ui::app::state::RegionClipboard, base_track: usize, paste_bar: f32) {
        self.push_undo("Klistra in regionklunga");
        // **Ripple vid inklistring:** gör plats först, klistra sedan — annars flyttas det
        // nyss inklistrade med när de efterföljande skjuts undan.
        let cluster_len = clip
            .items
            .iter()
            .map(|(_, r)| (r.start_bar - clip.anchor_bar) + r.length_bars)
            .fold(0.0f32, f32::max);
        if cluster_len > 1e-4 {
            self.ripple_shift(base_track, paste_bar - 1e-4, cluster_len);
        }
        let mut affected: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut pasted = 0;
        let mut first: Option<(usize, usize)> = None;
        for (item_idx, target, start_bar) in crate::ui::app::state::plan_region_paste(
            clip,
            base_track,
            paste_bar,
            self.playlist_tracks.len(),
        ) {
            let source = &clip.items[item_idx];
            let mut new_r = source.1.clone();
            new_r.id = self.next_region_id();
            new_r.start_bar = start_bar;
            new_r.name = format!("{} (Kopia)", new_r.name.replace(" (Kopia)", ""));
            // Kopian ärver grupptillhörigheten, så en klistrad klunga tetap grouped.
            if let Some(gid) = self.item_groups.get(&source.1.id).copied() {
                self.item_groups.insert(new_r.id, gid);
            }
            self.playlist_tracks[target].regions.push(new_r);
            let idx = self.playlist_tracks[target].regions.len() - 1;
            first.get_or_insert((target, idx));
            affected.insert(target);
            pasted += 1;
        }
        for t in affected {
            self.sync_track_regions(t);
        }
        if let Some(pos) = first {
            self.selected_regions.clear();
            self.selected_regions.insert(pos);
            self.selected_audio_region = Some(pos);
            self.selected_timeline_track = pos.0;
            self.selected_tracks.clear();
            self.selected_tracks.insert(pos.0);
        }
        self.status_message = crate::tstatus!(
            "📋 Klistrade in {} regioner vid takt {:.2}! [Ångra: Ctrl+Z]",
            pasted,
            paste_bar + 1.0
        );
    }

    /// **Ripple**: skjuter det som ligger vid eller efter `at_bar` med `delta` takter —
    /// negativt stänger ett gap, positivt gör plats för en inklistring. Returnerar antalet
    /// regioner som flyttades, och synkar de spår som ändrades.
    pub fn ripple_shift(&mut self, track_idx: usize, at_bar: f32, delta_bars: f32) -> usize {
        if self.ripple_mode == crate::ui::app::state::RippleMode::Off || delta_bars.abs() < 1e-6 {
            return 0;
        }
        let all_tracks = self.ripple_mode == crate::ui::app::state::RippleMode::All;
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut moved = 0;
        for (t, track) in self.playlist_tracks.iter_mut().enumerate() {
            if !all_tracks && t != track_idx {
                continue;
            }
            for reg in track.regions.iter_mut() {
                if reg.start_bar >= at_bar - 1e-4 {
                    reg.start_bar = (reg.start_bar + delta_bars).max(0.0);
                    moved += 1;
                    touched.insert(t);
                }
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        moved
    }

    /// **Klistra in på ursprunglig plats** (REAPER: `Paste to original position`).
    pub fn paste_cluster_at_origin(&mut self) {
        let Some(clip) = self.region_clipboard.clone() else {
            self.status_message = "Urklipp är tomt.".to_string();
            return;
        };
        self.paste_cluster(&clip, clip.origin_track, clip.anchor_bar);
    }

    /// Regioner som hör till samma objektgrupp som `region_id`.
    pub(crate) fn group_members_of(&self, region_id: usize) -> Vec<(usize, usize)> {
        let Some(gid) = self.item_groups.get(&region_id).copied() else {
            return Vec::new();
        };
        let mut members = Vec::new();
        for (t, track) in self.playlist_tracks.iter().enumerate() {
            for (r, reg) in track.regions.iter().enumerate() {
                if self.item_groups.get(&reg.id) == Some(&gid) {
                    members.push((t, r));
                }
            }
        }
        members
    }

    /// Utökar markeringen med alla grupper som redan ingår — klickar man en grupperad
    /// region markerar man hela gruppen, som i REAPER.
    fn expand_with_item_groups(
        &self,
        set: &std::collections::BTreeSet<(usize, usize)>,
    ) -> std::collections::BTreeSet<(usize, usize)> {
        crate::ui::app::state::expand_with_groups(&self.playlist_tracks, &self.item_groups, set)
    }

    /// **Objektgrupp** (REAPER: `Item grouping`, Ctrl+G på regioner).
    pub fn group_selected_regions(&mut self) -> usize {
        if self.selected_regions.len() < 2 {
            return 0;
        }
        let ids: Vec<usize> = self
            .selected_regions
            .iter()
            .filter_map(|(t, r)| self.playlist_tracks.get(*t).and_then(|tr| tr.regions.get(*r)))
            .map(|reg| reg.id)
            .collect();
        if ids.len() < 2 {
            return 0;
        }
        self.push_undo("Gruppera objekt");
        let gid = self.next_item_group_id.max(1);
        self.next_item_group_id = gid + 1;
        for id in ids.iter() {
            self.item_groups.insert(*id, gid);
        }
        ids.len()
    }

    /// Löser upp objektgrupperna för de markerade regionerna.
    pub fn ungroup_selected_regions(&mut self) -> usize {
        let ids: Vec<usize> = self
            .selected_regions
            .iter()
            .filter_map(|(t, r)| self.playlist_tracks.get(*t).and_then(|tr| tr.regions.get(*r)))
            .map(|reg| reg.id)
            .collect();
        let before = ids.len();
        for id in ids {
            self.item_groups.remove(&id);
        }
        before
    }

    /// Skjuter allt från `at_bar` med `delta` takter, utan hänsyn till ripple-läget.
    /// Används när ett kommando självt måste göra plats (razor-duplicera).
    fn shift_from(&mut self, tracks: &[usize], at_bar: f32, delta: f32) -> usize {
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut moved = 0;
        for t in tracks.iter().copied() {
            if let Some(track) = self.playlist_tracks.get_mut(t) {
                for reg in track.regions.iter_mut() {
                    if reg.start_bar >= at_bar - 1e-4 {
                        reg.start_bar = (reg.start_bar + delta).max(0.0);
                        moved += 1;
                        touched.insert(t);
                    }
                }
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        moved
    }

    /// **Skickar de markerade spåren till en ledig buss** — samma sak som att routa in dem
    /// i en mapp: de får en gemensam fader i mixern. Buss 3 är standardvägen och räknas
    /// därför inte som ledig. `None` när alla bussar redan används.
    pub fn send_selected_tracks_to_free_bus(&mut self) -> Option<usize> {
        let used: std::collections::BTreeSet<usize> = self
            .playlist_tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.selected_tracks.contains(i))
            .map(|(_, t)| t.bus.min(crate::audio::synth::NUM_BUSES - 1))
            .collect();
        let free = (0..crate::audio::synth::NUM_BUSES - 1).find(|b| !used.contains(b))?;
        self.push_undo("Skicka spår till buss");
        let mut touched = 0;
        for t in self.selected_tracks.clone() {
            if let Some(track) = self.playlist_tracks.get_mut(t) {
                track.bus = free;
                touched += 1;
            }
        }
        for t in self.selected_tracks.clone() {
            self.sync_track_audio_state(t);
        }
        self.status_message = crate::tstatus!(
            "🔀 Skickade {} spår till bussen '{}'",
            touched,
            crate::i18n::t(crate::audio::synth::BUS_NAMES[free])
        );
        Some(free)
    }

    /// Tillbaka till standardbussen (den sista), för de markerade spåren.
    pub fn send_selected_tracks_to_default_bus(&mut self) -> usize {
        let default_bus = crate::audio::synth::NUM_BUSES - 1;
        self.push_undo("Skicka spår till standardbuss");
        let mut touched = 0;
        for t in self.selected_tracks.clone() {
            if let Some(track) = self.playlist_tracks.get_mut(t) {
                track.bus = default_bus;
                touched += 1;
            }
        }
        for t in self.selected_tracks.clone() {
            self.sync_track_audio_state(t);
        }
        self.status_message = crate::tstatus!("⏏ Skickade {} spår tillbaka till standardbussen", touched);
        touched
    }

    /// **Flyttar det primära spåret upp eller ner** i listan och synkar om motorn.
    /// Markeringen följer med, så nästa kommando träffar samma spår som före flytten.
    pub fn move_selected_track(&mut self, delta: isize) -> bool {
        let from = self.selected_timeline_track;
        let to = from as isize + delta;
        if to < 0 {
            return false;
        }
        self.move_track(from, to as usize)
    }

    /// Flyttar spåret på `from` till platsen `to`. Motorn synkas om och markeringen
    /// följer med; en flytt till samma plats är inget kommando.
    pub fn move_track(&mut self, from: usize, to: usize) -> bool {
        if from >= self.playlist_tracks.len() || to >= self.playlist_tracks.len() || from == to {
            return false;
        }
        let name = self.playlist_tracks[from].name.clone();
        self.push_undo("Flytta spår");
        let track = self.playlist_tracks.remove(from);
        self.playlist_tracks.insert(to, track);

        // Markeringen flyttar med: index som låg mellan de två platserna skjuts ett steg.
        self.selected_tracks = self
            .selected_tracks
            .iter()
            .copied()
            .map(|i| crate::ui::app::state::remap_track_index(i, from, to))
            .collect();
        self.selected_timeline_track = to;
        self.selected_regions.clear();
        self.selected_audio_region = None;

        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        for i in 0..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
            self.sync_track_audio_state(i);
            self.sync_track_regions(i);
        }
        self.status_message = crate::tstatus!("↕ Flyttade '{}' till plats {}", name, to + 1);
        true
    }

    /// Flyttar en pågående dragnings regioner till `target`, med samma spåravstånd som
    /// klungan hade. Tas bort bakifrån så indexen håller, och läggs tillbaka i sitt
    /// ursprungliga spår om målet hamnar utanför projektet.
    pub(crate) fn move_drag_to_track(
        &mut self,
        drag: &crate::ui::app::state::RegionDragState,
        target: usize,
    ) -> usize {
        let mut refs: Vec<(usize, usize)> = vec![(drag.track_idx, drag.region_idx)];
        refs.extend(drag.multi_initial.iter().map(|(t, r, _)| (*t, *r)));
        refs.sort_by(|a, b| b.cmp(a));
        let base = drag.track_idx as isize;
        let mut lifted: Vec<(isize, AudioRegion)> = Vec::new();
        for (t, r) in refs {
            if let Some(track) = self.playlist_tracks.get_mut(t)
                && r < track.regions.len()
            {
                lifted.push((t as isize - base, track.regions.remove(r)));
            }
        }
        let mut moved = 0;
        let mut new_selection: std::collections::BTreeSet<(usize, usize)> =
            std::collections::BTreeSet::new();
        for (offset, reg) in lifted {
            let dest = target as isize + offset;
            let slot = if dest >= 0 && (dest as usize) < self.playlist_tracks.len() {
                Some(dest as usize)
            } else {
                let back = base + offset;
                (back >= 0 && (back as usize) < self.playlist_tracks.len()).then_some(back as usize)
            };
            if let Some(t) = slot {
                self.playlist_tracks[t].regions.push(reg);
                let idx = self.playlist_tracks[t].regions.len() - 1;
                new_selection.insert((t, idx));
                moved += 1;
            }
        }
        if !new_selection.is_empty() {
            self.selected_audio_region = new_selection.iter().next().copied();
            self.selected_regions = new_selection;
            self.selected_timeline_track = target.min(self.playlist_tracks.len().saturating_sub(1));
        }
        moved
    }

    /// **Razor: duplicera området** — kopierar det som ligger i tidsmarkeringen och lägger
    /// kopian direkt efter, med plats gjord så inget hamnar ovanpå något annat.
    pub fn razor_duplicate_area(&mut self) -> usize {
        let Some((a, b)) = self.time_selection else {
            return 0;
        };
        let (from, to) = if a <= b { (a, b) } else { (b, a) };
        let len = to - from;
        if len < 0.01 || self.playlist_tracks.is_empty() {
            return 0;
        }
        let tracks: Vec<usize> = if self.selected_tracks.is_empty() {
            vec![self.selected_timeline_track.min(self.playlist_tracks.len() - 1)]
        } else {
            self.selected_tracks
                .iter()
                .copied()
                .filter(|t| *t < self.playlist_tracks.len())
                .collect()
        };
        self.push_undo("Razor: duplicera område");
        let tempo = self.tempo_map();
        let sources: Vec<(usize, AudioRegion)> = tracks
            .iter()
            .flat_map(|t| {
                self.playlist_tracks
                    .get(*t)
                    .map(|tr| {
                        crate::ui::app::state::regions_in_bar_range(tr, from, to)
                            .into_iter()
                            .map(|r| (*t, tr.regions[r].clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        self.shift_from(&tracks, to - 1e-4, len);
        let mut made = 0;
        for (t, src) in sources {
            if let Some(mut copy) = trimmed_region_copy(&src, from, to, &tempo, self.next_region_id()) {
                copy.start_bar += len;
                copy.name = format!("{} (Kopia)", copy.name.replace(" (Kopia)", ""));
                self.playlist_tracks[t].regions.push(copy);
                made += 1;
            }
        }
        for t in tracks {
            self.sync_track_regions(t);
        }
        self.status_message = crate::tstatus!(
            "✂️ Razor: duplicerade takt {:.2}–{:.2} ({} regioner)",
            from + 1.0,
            to + 1.0,
            made
        );
        made
    }

    /// **Razor: ta bort tidsområdet** (REAPER). Regioner som ligger helt i området tas bort,
    /// de som bara sticker in kapas, och en region som spänner över hela området delas i två.
    /// Arbetar på de markerade spåren (annars det primära).
    pub fn razor_delete_area(&mut self) -> usize {
        let Some((a, b)) = self.time_selection else {
            return 0;
        };
        let (from, to) = if a <= b { (a, b) } else { (b, a) };
        if to - from < 0.01 || self.playlist_tracks.is_empty() {
            return 0;
        }
        let tempo = self.tempo_map();
        let tracks: Vec<usize> = if self.selected_tracks.is_empty() {
            vec![self.selected_timeline_track.min(self.playlist_tracks.len() - 1)]
        } else {
            self.selected_tracks
                .iter()
                .copied()
                .filter(|t| *t < self.playlist_tracks.len())
                .collect()
        };
        self.push_undo("Razor: ta bort område");
        let mut removed = 0;
        for t in tracks {
            let mut i = 0;
            while i < self.playlist_tracks[t].regions.len() {
                let reg = self.playlist_tracks[t].regions[i].clone();
                let start = reg.start_bar;
                let end = start + reg.length_bars;
                if end <= from + 1e-4 || start >= to - 1e-4 {
                    i += 1;
                    continue;
                }
                if start >= from - 1e-4 && end <= to + 1e-4 {
                    self.playlist_tracks[t].regions.remove(i);
                    removed += 1;
                    continue;
                }
                if start < from && end > to {
                    // Dela i två: vänster [start, from] och höger [to, end].
                    let right_offset_sec =
                        tempo.secs_for_bars_at(start as f64, (to - start) as f64) as f32;
                    let cut = ((to - start) / reg.length_bars).clamp(0.0, 1.0);
                    let peak_cut = ((cut * reg.waveform_peaks.len() as f32) as usize)
                        .clamp(1, reg.waveform_peaks.len().saturating_sub(1).max(1));
                    let mut right = reg.clone();
                    right.id = self.next_region_id();
                    right.start_bar = to;
                    right.length_bars = end - to;
                    right.sample_offset_sec = reg.sample_offset_sec + right_offset_sec;
                    right.waveform_peaks = reg.waveform_peaks[peak_cut.min(reg.waveform_peaks.len())..].to_vec();
                    right.fade_in_bars = 0.0;
                    right.fade_out_bars = reg.fade_out_bars.min(right.length_bars * 0.5);
                    let left = &mut self.playlist_tracks[t].regions[i];
                    left.length_bars = from - start;
                    left.fade_out_bars = 0.0;
                    left.waveform_peaks.truncate(peak_cut);
                    self.playlist_tracks[t].regions.insert(i + 1, right);
                    i += 2;
                    continue;
                }
                if start < from {
                    // Kapa högerkanten vid områdets början.
                    let new_len = from - start;
                    let keep = ((new_len / reg.length_bars) * reg.waveform_peaks.len() as f32) as usize;
                    let region = &mut self.playlist_tracks[t].regions[i];
                    region.length_bars = new_len;
                    region.fade_out_bars = reg.fade_out_bars.min(new_len * 0.5);
                    region.waveform_peaks.truncate(keep.max(1));
                } else {
                    // Kapa vänsterkanten vid områdets slut.
                    let shift_sec =
                        tempo.secs_for_bars_at(start as f64, (to - start) as f64) as f32;
                    let drop = ((to - start) / reg.length_bars).clamp(0.0, 1.0);
                    let keep_from = ((drop * reg.waveform_peaks.len() as f32) as usize)
                        .min(reg.waveform_peaks.len());
                    let region = &mut self.playlist_tracks[t].regions[i];
                    region.start_bar = to;
                    region.length_bars = end - to;
                    region.sample_offset_sec = reg.sample_offset_sec + shift_sec;
                    region.waveform_peaks = reg.waveform_peaks[keep_from..].to_vec();
                    region.fade_in_bars = 0.0;
                }
                i += 1;
            }
            self.sync_track_regions(t);
        }
        self.status_message = crate::tstatus!(
            "✂️ Razor: tog bort takt {:.2}–{:.2} ({} regioner)",
            from + 1.0,
            to + 1.0,
            removed
        );
        removed
    }

    /// **Ångra till ett visst steg** (REAPER: undo history). Steg 0 är botten.
    pub fn undo_to(&mut self, index: usize) {
        while self.undo_stack.len() > index {
            self.undo();
        }
    }

    /// **Gruppera de markerade spåren** i en ny grupp (FL:s spellistegrupper).
    /// Returnerar antalet spår som fick gruppen.
    pub fn group_selected_tracks(&mut self) -> usize {
        let members: Vec<usize> = self
            .selected_tracks
            .iter()
            .copied()
            .filter(|t| *t < self.playlist_tracks.len())
            .collect();
        if members.len() < 2 {
            return 0;
        }
        self.push_undo("Gruppera spår");
        let group_idx = self.track_groups.len();
        self.track_groups.push(crate::ui::app::state::TrackGroup {
            name: format!("Grupp {}", group_idx + 1),
            collapsed: false,
        });
        let mut count = 0;
        for t in members {
            if let Some(track) = self.playlist_tracks.get_mut(t) {
                track.group = Some(group_idx);
                count += 1;
            }
        }
        count
    }

    /// Tar bort grupptillhörigheten för de markerade spåren.
    pub fn ungroup_selected_tracks(&mut self) -> usize {
        let members: Vec<usize> = self
            .selected_tracks
            .iter()
            .copied()
            .filter(|t| *t < self.playlist_tracks.len())
            .collect();
        if members.is_empty() {
            return 0;
        }
        self.push_undo("Ta bort grupp");
        let mut count = 0;
        for t in members {
            if let Some(track) = self.playlist_tracks.get_mut(t)
                && track.group.is_some()
            {
                track.group = None;
                count += 1;
            }
        }
        count
    }
}

impl SonixApp {
pub fn copy_selected_region(&mut self) {
    // **Flera regioner → en klunga.** Spåravstånd och taktavstånd bevaras, så en
    // klistrad klunga hamnar rätt även om den sträcker sig över flera spår.
    if self.selected_regions.len() > 1 {
        let Some(clip) = crate::ui::app::state::build_region_clipboard(
            &self.playlist_tracks,
            &self.selected_regions,
        ) else {
            return;
        };
        let count = clip.items.len();
        self.region_clipboard = Some(clip);
        self.copied_region = None;
        self.track_clipboard.clear();
        self.status_message = crate::tstatus!(
            "📋 Kopierade {} regioner över {} spår! [Klistra in: Ctrl+V]",
            count,
            self.selected_regions.iter().map(|(t, _)| *t).collect::<std::collections::BTreeSet<_>>().len()
        );
        return;
    }

    // **Inga regioner men flera spår → spåren i sig.**
    if self.selected_regions.is_empty() && self.selected_tracks.len() > 1 {
        let mut copied: Vec<crate::ui::app::state::PlaylistTrack> = Vec::new();
        for t in self.selected_tracks.iter() {
            if let Some(track) = self.playlist_tracks.get(*t) {
                copied.push(track.clone());
            }
        }
        let count = copied.len();
        self.track_clipboard = copied;
        self.copied_region = None;
        self.region_clipboard = None;
        self.status_message = crate::tstatus!("📋 Kopierade {} spår! [Klistra in: Ctrl+V]", count);
        return;
    }

    if let Some((t_idx, r_idx)) = self.selected_audio_region {
        if t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            let reg = self.playlist_tracks[t_idx].regions[r_idx].clone();
            let name = reg.name.clone();
            self.copied_region = Some(reg);
            self.region_clipboard = None;
            self.track_clipboard.clear();
            self.selected_regions.clear();
            self.selected_regions.insert((t_idx, r_idx));
            self.status_message = crate::tstatus!("📋 Kopierade sample '{}' till urklipp! [Klistra in: Ctrl+V]", name);
        }
    } else {
        self.status_message = crate::i18n::t("Ingen ljudregion markerad att kopiera.").to_string();
    }
}
}

impl SonixApp {
pub fn paste_region(&mut self) {
    // **Klunga först:** regioner som kopierats tillsammans klistras in tillsammans.
    if let Some(clip) = self.region_clipboard.clone() {
        if self.playlist_tracks.is_empty() || clip.items.is_empty() {
            return;
        }
        let base_track = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let paste_bar = (tempo.bar_at_secs(self.song_time as f64) as f32).max(0.0);
        self.paste_cluster(&clip, base_track, paste_bar);
        return;
    }

    // **Spår:** kopierade spår läggs in efter det sista markerade.
    if !self.track_clipboard.is_empty() {
        let insert_at = self
            .selected_tracks
            .iter()
            .next_back()
            .map(|i| i + 1)
            .unwrap_or(self.playlist_tracks.len())
            .min(self.playlist_tracks.len());
        self.push_undo("Klistra in spår");
        let clips = self.track_clipboard.clone();
        let count = clips.len();
        for (k, src) in clips.iter().enumerate() {
            let mut copy = src.clone();
            copy.name = format!("{} (Kopia)", src.name.replace(" (Kopia)", ""));
            for r in copy.regions.iter_mut() {
                r.id = self.next_region_id();
            }
            self.playlist_tracks.insert(insert_at + k, copy);
        }
        let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
        for i in 0..self.playlist_tracks.len() {
            self.sync_track_stem_to_engine(i);
            self.sync_track_audio_state(i);
            self.sync_track_regions(i);
        }
        let last = (insert_at + count).min(self.playlist_tracks.len().saturating_sub(1));
        self.select_only_track(last);
        self.selected_audio_region = None;
        self.status_message = crate::tstatus!("📋 Klistrade in {} spår! [Ångra: Ctrl+Z]", count);
        return;
    }

    if let Some(ref copied) = self.copied_region.clone() {
        if self.playlist_tracks.is_empty() {
            return;
        }
        let target_track = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let paste_bar = (tempo.bar_at_secs(self.song_time as f64) as f32).max(0.0);

        self.push_undo(&format!("Klistra in '{}'", copied.name));
        let mut new_r = copied.clone();
        new_r.id = self.next_region_id();
        new_r.start_bar = paste_bar;
        let clean_name = copied.name.replace(" (Kopia)", "");
        new_r.name = format!("{} (Kopia)", clean_name);

        self.playlist_tracks[target_track].regions.push(new_r.clone());
        self.sync_track_regions(target_track);
        let new_r_idx = self.playlist_tracks[target_track].regions.len() - 1;
        self.selected_audio_region = Some((target_track, new_r_idx));
        self.status_message = crate::tstatus!("📋 Klistrade in sample '{}' på spår {} vid takt {:.2}! [Ångra: Ctrl+Z]", new_r.name, target_track + 1, paste_bar + 1.0);
    } else {
        self.status_message = crate::i18n::t("Urklipp är tomt. Kopiera en region först med Ctrl+C.").to_string();
    }
}
}

impl SonixApp {
pub fn repeat_selected_region_loop(&mut self, factor: f32) {
    if self.selected_regions.len() > 1 {
        let targets: Vec<(usize, usize)> = self.selected_regions.iter().copied().collect();
        self.push_undo("Repetera sample loop");
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for (t, r) in targets {
            if let Some(reg) = self.playlist_tracks.get_mut(t).and_then(|tr| tr.regions.get_mut(r)) {
                if reg.loop_length_bars <= 0.001 {
                    reg.loop_length_bars = reg.length_bars;
                }
                let base = reg.loop_length_bars;
                reg.length_bars = (reg.length_bars * factor).max(base);
                touched.insert(t);
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.status_message = crate::tstatus!("🔁 Loopade {} regioner", self.selected_regions.len());
        return;
    }
    if let Some((t_idx, r_idx)) = self.selected_audio_region
        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            self.push_undo("Repetera sample loop");
            let reg = &mut self.playlist_tracks[t_idx].regions[r_idx];
            if reg.loop_length_bars <= 0.001 {
                reg.loop_length_bars = reg.length_bars;
            }
            let base_loop = reg.loop_length_bars;
            reg.length_bars = (reg.length_bars * factor).max(base_loop);
            let new_len = reg.length_bars;
            let num_reps = (new_len / base_loop).round() as usize;
            let name = reg.name.clone();
            self.sync_track_regions(t_idx);
            self.status_message = crate::tstatus!("🔁 Loopade sample '{}': {} repetitioner ({:.1} takter)!", name, num_reps, new_len);
        }
}
}

impl SonixApp {
pub fn duplicate_selected_region(&mut self) {
    // **Hela markeringen**: varje region får en kopia direkt efter sig.
    if self.selected_regions.len() > 1 {
        let targets: Vec<(usize, usize)> = self.selected_regions.iter().copied().collect();
        self.push_undo("Duplicera ljudregioner");
        let mut made = 0;
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for (t, r) in targets {
            let Some(source) = self.playlist_tracks.get(t).and_then(|tr| tr.regions.get(r)) else {
                continue;
            };
            let mut dup = source.clone();
            dup.start_bar += dup.length_bars;
            dup.name = format!("{} (Kopia)", dup.name.replace(" (Kopia)", ""));
            dup.id = self.next_region_id();
            if let Some(track) = self.playlist_tracks.get_mut(t) {
                track.regions.push(dup);
                touched.insert(t);
                made += 1;
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.selected_regions.clear();
        self.selected_audio_region = None;
        self.status_message = crate::tstatus!("📋 Duplicerade {} regioner", made);
        return;
    }
    if let Some((t_idx, r_idx)) = self.selected_audio_region
        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            self.push_undo("Duplicera ljudregion");
            let mut dup = self.playlist_tracks[t_idx].regions[r_idx].clone();
            dup.start_bar += dup.length_bars;
            dup.name = format!("{} (Kopia)", dup.name.replace(" (Kopia)", ""));
            dup.id = self.next_region_id();
            self.playlist_tracks[t_idx].regions.push(dup);
            self.sync_track_regions(t_idx);
            let new_idx = self.playlist_tracks[t_idx].regions.len() - 1;
            self.selected_audio_region = Some((t_idx, new_idx));
            self.status_message = crate::i18n::t("📋 Duplicerade ljudregion till tidslinjen!").to_string();
        }
}
}

impl SonixApp {
pub fn delete_selected_region(&mut self) {
    // **Hela markeringen**, inte bara den primära: Delete ska ta det man ser markerat.
    if self.selected_regions.len() > 1 {
        let mut targets: Vec<(usize, usize)> = self.selected_regions.iter().copied().collect();
        targets.sort_by(|a, b| b.cmp(a)); // bakifrån, så indexen håller
        self.push_undo(&format!("Radera {} regioner", targets.len()));
        let mut removed = 0;
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for (t, r) in targets {
            if let Some(track) = self.playlist_tracks.get_mut(t)
                && r < track.regions.len()
            {
                let start = track.regions[r].start_bar;
                let len = track.regions[r].length_bars;
                track.regions.remove(r);
                removed += 1;
                touched.insert(t);
                self.ripple_shift(t, start + len - 1e-4, -len);
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.selected_regions.clear();
        self.selected_audio_region = None;
        self.status_message = crate::tstatus!("🗑 Raderade {} regioner. [Ångra: Ctrl+Z]", removed);
        return;
    }
    if let Some((t_idx, r_idx)) = self.selected_audio_region
        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            let name = self.playlist_tracks[t_idx].regions[r_idx].name.clone();
            let start = self.playlist_tracks[t_idx].regions[r_idx].start_bar;
            let len = self.playlist_tracks[t_idx].regions[r_idx].length_bars;
            self.push_undo(&format!("Radera '{}'", name));
            self.playlist_tracks[t_idx].regions.remove(r_idx);
            self.sync_track_regions(t_idx);
            self.ripple_shift(t_idx, start + len - 1e-4, -len);
            self.selected_regions.clear();
            self.selected_audio_region = None;
            self.status_message = crate::tstatus!("🗑 Raderade region '{}'. [Ångra: Ctrl+Z]", name);
        }
}
}

impl SonixApp {
pub fn toggle_mute_selected_region(&mut self) {
    // **Hela markeringen** mutas tillsammans; riktningen tas från den primära.
    if self.selected_regions.len() > 1 {
        let primary_muted = self
            .selected_audio_region
            .and_then(|(t, r)| self.playlist_tracks.get(t).and_then(|tr| tr.regions.get(r)))
            .map(|reg| reg.muted)
            .unwrap_or(false);
        let target = !primary_muted;
        let targets: Vec<(usize, usize)> = self.selected_regions.iter().copied().collect();
        self.push_undo("Muta/Avmuta regioner");
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for (t, r) in targets {
            if let Some(reg) = self.playlist_tracks.get_mut(t).and_then(|tr| tr.regions.get_mut(r)) {
                reg.muted = target;
                touched.insert(t);
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.status_message = if target {
            "🔇 Regioner mutade".to_string()
        } else {
            "🔊 Regioner aktiva".to_string()
        };
        return;
    }
    if let Some((t_idx, r_idx)) = self.selected_audio_region
        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            self.push_undo("Muta/Avmuta region");
            self.playlist_tracks[t_idx].regions[r_idx].muted = !self.playlist_tracks[t_idx].regions[r_idx].muted;
            let is_m = self.playlist_tracks[t_idx].regions[r_idx].muted;
            self.sync_track_regions(t_idx);
            self.status_message = if is_m { crate::i18n::t("🔇 Region mutad").to_string() } else { crate::i18n::t("🔊 Region aktiv").to_string() };
        }
}
}

impl SonixApp {
/// Klippets eget val: bandspelaren i stället för den tonhöjdsbevarande
/// sträckningen (Fas 8.10 steg 2).
///
/// Undantaget, aldrig standarden: en smurf som uppstår av misstag är värre än en
/// effekt man får leta efter (Abletons ordning — *Re-Pitch* är undantaget).
pub fn toggle_region_tape(&mut self, t_idx: usize, r_idx: usize) {
    if t_idx >= self.playlist_tracks.len()
        || r_idx >= self.playlist_tracks[t_idx].regions.len()
    {
        return;
    }
    self.push_undo("Klippets temoläge");
    let tape = {
        let r = &mut self.playlist_tracks[t_idx].regions[r_idx];
        r.tape = !r.tape;
        r.tape
    };
    self.sync_track_regions(t_idx);
    self.status_message = if tape {
        crate::i18n::t("📼 Klippet följer tempot som en bandspelare (tonhöjden följer med)")
            .to_string()
    } else {
        crate::i18n::t("🎚 Klippet sträcks med bevarad tonhöjd när tempot ändras").to_string()
    };
}
}

impl SonixApp {
pub fn reverse_selected_region(&mut self) {
    if self.selected_regions.len() > 1 {
        let targets: Vec<(usize, usize)> = self.selected_regions.iter().copied().collect();
        self.push_undo(crate::i18n::t("Vänd region baklänges (Reverse)"));
        let mut touched: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let mut flipped = 0;
        for (t, r) in targets {
            if let Some(reg) = self.playlist_tracks.get_mut(t).and_then(|tr| tr.regions.get_mut(r)) {
                reg.is_reverse = !reg.is_reverse;
                touched.insert(t);
                flipped += 1;
            }
        }
        for t in touched {
            self.sync_track_regions(t);
        }
        self.status_message = crate::tstatus!("🔄 Vände {} regioner baklänges", flipped);
        return;
    }
    if let Some((t_idx, r_idx)) = self.selected_audio_region
        && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
            self.push_undo(crate::i18n::t("Vänd region baklänges (Reverse)"));
            self.playlist_tracks[t_idx].regions[r_idx].is_reverse = !self.playlist_tracks[t_idx].regions[r_idx].is_reverse;
            self.sync_track_regions(t_idx);
            self.status_message = crate::i18n::t("🔄 Vände ljudregion baklänges (Reverse)!").to_string();
        }
}
}

impl SonixApp {
/// **"Sätt takt 1 här"** (Fas 8.14): ljudet under spelhuvudet blir klippets första
/// sampel. Klippet står kvar där det står och högerkanten står still — det är
/// början som kapas, som i Audacitys kant-trim.
///
/// Regeln bor i [`align_clip_start_to_point`] (ren funktion, fyra egna prov); det
/// här är bara inkopplingen: ångring, besked och en omsynk så att motorn hör
/// ändringen direkt.
/// Flyttar **hela stämgruppen** ett och samma skift (Fas 8.15b) — en regel, två dörrar.
///
/// Planen räknas fram först ([`plan_group_shift_by_head_secs`]) och skrivs sedan i ett
/// svep, så att ett klipp som inte kan flyttas stoppar allt i stället för att lämna
/// gruppen halvflyttad. Returnerar antalet klipp som flyttades, eller namnet på klippet
/// som stoppade.
fn shift_stem_group(
    &mut self,
    t_idx: usize,
    r_idx: usize,
    head_secs: f64,
    undo_label: &'static str,
) -> Result<usize, String> {
    let Some(anchor_bar) = self
        .playlist_tracks
        .get(t_idx)
        .and_then(|t| t.regions.get(r_idx))
        .map(|r| r.start_bar)
    else {
        return Ok(0);
    };
    let mut clips: Vec<(usize, usize, ClipGeometry)> = Vec::new();
    for (t, track) in self.playlist_tracks.iter().enumerate() {
        for (r, reg) in track.regions.iter().enumerate() {
            clips.push((
                t,
                r,
                ClipGeometry {
                    start_bar: reg.start_bar,
                    length_bars: reg.length_bars,
                    sample_offset_sec: reg.sample_offset_sec,
                    source_bpm: reg.source_bpm,
                },
            ));
        }
    }
    let tempo = self.tempo_map();
    let plan = match plan_group_shift_by_head_secs(&clips, anchor_bar, head_secs, &tempo) {
        Ok(plan) => plan,
        Err((t, r)) => {
            let name = self
                .playlist_tracks
                .get(t)
                .and_then(|track| track.regions.get(r))
                .map(|reg| reg.name.clone())
                .unwrap_or_default();
            return Err(name);
        }
    };
    if plan.is_empty() {
        return Ok(0);
    }
    self.push_undo(crate::i18n::t(undo_label));
    let mut touched: Vec<usize> = Vec::new();
    for (t, r, align) in &plan {
        if let Some(reg) = self
            .playlist_tracks
            .get_mut(*t)
            .and_then(|track| track.regions.get_mut(*r))
        {
            reg.sample_offset_sec = align.sample_offset_sec;
            reg.length_bars = align.length_bars;
        }
        if !touched.contains(t) {
            touched.push(*t);
        }
    }
    for t in touched {
        self.sync_track_regions(t);
    }
    Ok(plan.len())
}
}

impl SonixApp {
/// **"Sätt takt 1 här"** (Fas 8.14): ljudet under spelhuvudet blir klippets första
/// sampel medan klippet står kvar — och **hela stämgruppen följer med** (Fas 8.15b),
/// annars hamnar stämman ur fas med sina syskon.
///
/// Regeln bor i [`align_clip_start_to_point`] och gruppen i
/// [`plan_group_shift_by_head_secs`] (rena funktioner, egna prov); det här är bara
/// inkopplingen: ångring, besked och en omsynk så att motorn hör ändringen direkt.
pub fn set_beat_one_at_playhead(&mut self) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        return;
    };
    if t_idx >= self.playlist_tracks.len()
        || r_idx >= self.playlist_tracks[t_idx].regions.len()
    {
        return;
    }
    let tempo = self.tempo_map();
    let r = self.playlist_tracks[t_idx].regions[r_idx].clone();
    let head_secs = self.song_time as f64 - tempo.secs_at_bar(r.start_bar as f64);
    if head_secs <= 0.0 {
        // 8.5-regeln: säg varför i stället för att tiga eller gissa.
        self.status_message = crate::tstatus!(
            "⚠ Spelhuvudet står utanför '{}' — inget ändrat. Flytta det in i klippet först.",
            r.name
        );
        return;
    }
    match self.shift_stem_group(t_idx, r_idx, head_secs, "Sätt takt 1 här") {
        Err(name) => {
            self.status_message = crate::tstatus!(
                "⚠ Flytten skulle tömma '{}' — inget ändrat, ingenting flyttat.",
                name
            );
        }
        Ok(0) => {
            self.status_message = crate::tstatus!(
                "⚠ Hittade inget klipp på takt {:.2} att flytta.",
                r.start_bar
            );
        }
        Ok(n) => {
            self.status_message = crate::tstatus!(
                "🎯 {} klipp på takt {:.2} läser filen {:.3} s längre in — slaget ligger på rutnätet.",
                n,
                r.start_bar,
                head_secs
            );
        }
    }
}
}

impl SonixApp {
/// **"Hitta första slaget"** (Fas 8.14): mäter **var ljudet börjar** i klippets egen
/// fil i stället för att be användaren pricka slaget på pixeln — och sätter det sedan
/// som gruppens första sampel med samma regel som "Sätt takt 1 här".
///
/// Det är den etablerade DAW-vägen (Ableton visar detekterade transients och låter
/// dig sätta en av dem som klippets början), och den gör ett snap **ärligt**: talet
/// kommer ur en mätning i filen, inte ur ett antagande om rutnätet.
///
/// **Ankaret är ljudet, inte detektorn** — mätt på Alex' fem stämmor 2026-09-13:
/// träffen på 0,6008 s (backningen sa 0,502), sången 7,18 (mjuka attacken sågs inte
/// alls), basen 1,6885, gitarren **0,0** (inget att trimma), och en stämma som är
/// tyst i 20 s svarar `None`. Se [`crate::audio::onset::music_start_source_secs`].
///
/// **Gruppen följer med** (Fas 8.15b): ett klipp i ett set är en stämma, och en stämma
/// som flyttas ensam är en sång ur fas. Ankaret är det klipp du valde — där låg slaget.
///
/// **Ingen träff är ett giltigt svar.** En pad, en stråke eller ett tyst parti har
/// ingen början att sätta; då sägs det, och ingenting flyttas.
pub fn align_clip_to_first_beat(&mut self) {
    let Some((t_idx, r_idx)) = self.selected_audio_region else {
        return;
    };
    if t_idx >= self.playlist_tracks.len()
        || r_idx >= self.playlist_tracks[t_idx].regions.len()
    {
        return;
    }
    let tempo = self.tempo_map();
    let r = self.playlist_tracks[t_idx].regions[r_idx].clone();

    // Klippets eget ljud: **filen klippet pekar på** när den finns — då är svaret
    // rätt även när klippet flyttats in på ett annat spår. Annars spårets buffert,
    // och ett fel sägs högt i stället för att tigas (8.5).
    let (mono, sr) = match r.source_path.as_deref().filter(|p| !p.is_empty()) {
        Some(path) => match load_audio_or_report(path) {
            Some((l, _r, sr)) => (l, sr),
            None => {
                self.status_message = crate::tstatus!(
                    "⚠ Kunde inte läsa '{}' — inget ändrat.",
                    r.name
                );
                return;
            }
        },
        None => match self.playlist_tracks[t_idx].pcm_audio.as_ref() {
            Some((l, _r, sr)) => (l.as_ref().clone(), *sr),
            None => {
                self.status_message = crate::tstatus!(
                    "⚠ '{}' har ingen fil att mäta i — inget ändrat.",
                    r.name
                );
                return;
            }
        },
    };

    let params = crate::audio::onset::OnsetParams::default();
    let Some(onset_src) = crate::audio::onset::music_start_source_secs(
        &mono,
        sr as f32,
        r.sample_offset_sec,
        FIRST_BEAT_SEARCH_SECS,
        &params,
    ) else {
        self.status_message = crate::tstatus!(
            "🔍 Hörde inget ljud inom {:.0} s i början av '{}' — använd \"Sätt takt 1 här\" och peka själv.",
            FIRST_BEAT_SEARCH_SECS,
            r.name
        );
        return;
    };

    let Some(point) = timeline_point_for_source_secs(
        onset_src,
        r.start_bar,
        r.sample_offset_sec,
        r.source_bpm,
        &tempo,
    ) else {
        self.status_message = crate::tstatus!(
            "⚠ Slaget ligger före '{}' första sampel — inget ändrat.",
            r.name
        );
        return;
    };
    let head_secs = point - tempo.secs_at_bar(r.start_bar as f64);
    if head_secs <= 0.0 {
        self.status_message = crate::tstatus!(
            "⚠ '{}' börjar redan på slaget — inget ändrat.",
            r.name
        );
        return;
    }
    match self.shift_stem_group(t_idx, r_idx, head_secs, "Hitta första slaget") {
        Err(name) => {
            self.status_message = crate::tstatus!(
                "⚠ Flytten skulle tömma '{}' — inget ändrat, ingenting flyttat.",
                name
            );
        }
        Ok(n) => {
            self.status_message = crate::tstatus!(
                "🎯 Musiken börjar {:.3} s in i filen — {} klipp på takt {:.2} flyttades {:.3} s, så stämmorna håller ihop.",
                onset_src,
                n,
                r.start_bar,
                head_secs
            );
        }
    }
}
}


/// Kopia av `reg` beskuren till [from, to], med sampling-offset och vågform som följer
/// med kapningen. `None` när inget av regionen ligger i intervallet.
fn trimmed_region_copy(
    reg: &AudioRegion,
    from: f32,
    to: f32,
    tempo: &crate::audio::tempo::TempoMap,
    new_id: usize,
) -> Option<AudioRegion> {
    let start = reg.start_bar.max(from);
    let end = (reg.start_bar + reg.length_bars).min(to);
    if end - start < 0.01 {
        return None;
    }
    let mut copy = reg.clone();
    copy.id = new_id;
    copy.start_bar = start;
    copy.length_bars = end - start;
    if start > reg.start_bar {
        copy.sample_offset_sec = reg.sample_offset_sec
            + tempo.secs_for_bars_at(reg.start_bar as f64, (start - reg.start_bar) as f64) as f32;
    }
    let total = reg.length_bars.max(1e-6);
    let peaks = reg.waveform_peaks.len();
    if peaks > 1 {
        let p0 = (((start - reg.start_bar) / total) * peaks as f32) as usize;
        let p1 = (((end - reg.start_bar) / total) * peaks as f32) as usize;
        let p0 = p0.min(peaks - 1);
        let p1 = p1.min(peaks).max(p0 + 1);
        copy.waveform_peaks = reg.waveform_peaks[p0..p1].to_vec();
    }
    copy.fade_in_bars = 0.0;
    copy.fade_out_bars = 0.0;
    Some(copy)
}

#[cfg(test)]
mod razor_tests {
    use super::*;
    use crate::ui::app::state::AudioRegion;

    fn region(start: f32, len: f32) -> AudioRegion {
        AudioRegion {
            id: 1,
            name: "r".to_string(),
            start_bar: start,
            length_bars: len,
            sample_offset_sec: 0.0,
            source_path: None,
            waveform_peaks: vec![0.0; 100],
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: egui::Color32::WHITE,
            loop_length_bars: 0.0,
            source_bpm: 0.0,
            tape: false,
            warp_mode: Default::default(),
        }
    }

    /// **Kopian innehåller bara området.** Börjar regionen före `from` ska kopian börja på
    /// `from` med förskjuten källa; slutar den efter `to` ska den klippas där.
    #[test]
    fn a_trimmed_copy_covers_exactly_the_area() {
        let tempo = crate::audio::tempo::TempoMap::single(120.0);
        let copy = trimmed_region_copy(&region(4.0, 4.0), 5.0, 7.0, &tempo, 99).expect("kopia");
        assert_eq!(copy.id, 99);
        assert!((copy.start_bar - 5.0).abs() < 1e-4);
        assert!((copy.length_bars - 2.0).abs() < 1e-4);
        assert!(copy.sample_offset_sec > 0.0, "källan ska flyttas fram");
        assert!(copy.waveform_peaks.len() < 100, "vågformen ska kapas");
        assert!(copy.waveform_peaks.len() > 10);
    }

    /// Utanför området finns inget att kopiera.
    #[test]
    fn a_region_outside_the_area_is_not_copied() {
        let tempo = crate::audio::tempo::TempoMap::single(120.0);
        assert!(trimmed_region_copy(&region(0.0, 2.0), 5.0, 7.0, &tempo, 1).is_none());
        assert!(trimmed_region_copy(&region(8.0, 2.0), 5.0, 7.0, &tempo, 1).is_none());
    }
}
