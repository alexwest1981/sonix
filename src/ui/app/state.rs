//! Tillstånd och typer för app-ytan — utbrutet ur `app.rs` 2026-09-14.
//!
//! Det här är datamodellen: vad ett spår, ett klipp, ett mönster, en kanal och en
//! automatiseringskurva ÄR, plus deras standardvärden. Ingen rendering och ingen
//! inläsning — de delarna bor i sina egna moduler.
//!
//! Status: byggs — datamodellen; nya fält hör hit och ska ha ett ärligt standardvärde.
//! Rör inte: alla fält är `pub` (syskonmodulerna läser dem); döp inte om ett fält som står i en projektfil.

use super::*;
use crate::ui::theme::Theme;

/// Not → frekvens. Går genom `audio::tuning`, som är den enda vägen: annars kan
/// uppspelningen spela en stämning och exporten en annan.
pub(crate) fn midi_to_freq(note: u8) -> f32 {
    crate::audio::tuning::midi_to_freq(note)
}

pub(crate) fn note_name(note: u8) -> &'static str {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    names[(note % 12) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    ChannelRack,
    PianoRoll,
    PlaylistArranger,
    SessionDrummer,
    AlchemySynth,
    RemixFx,
    VocalStudio,
    AiMusicAssistant,
    ModularPatcher,
    StemSeparator,
    Video,
    PluginManager,
    EffectsMixer,
}

impl ViewMode {
    /// Ytorna i den ordning Tab går genom dem.
    pub const ALL: [ViewMode; 13] = [
        ViewMode::ChannelRack,
        ViewMode::PianoRoll,
        ViewMode::PlaylistArranger,
        ViewMode::SessionDrummer,
        ViewMode::AlchemySynth,
        ViewMode::RemixFx,
        ViewMode::VocalStudio,
        ViewMode::AiMusicAssistant,
        ViewMode::ModularPatcher,
        ViewMode::StemSeparator,
        ViewMode::Video,
        ViewMode::PluginManager,
        ViewMode::EffectsMixer,
    ];

    /// Nästa yta, med omslag från sista till första.
    pub fn next(self) -> ViewMode {
        let at = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }

    /// Föregående yta, med omslag från första till sista.
    pub fn prev(self) -> ViewMode {
        let at = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(at + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Pianorullens zoom: 10 % per tryck, klämt mellan 0,6 och 2,0.
pub const ZOOM_MIN: f32 = 0.6;
pub const ZOOM_MAX: f32 = 2.0;

pub fn zoom_step(current: f32, delta: f32) -> f32 {
    (current + delta).clamp(ZOOM_MIN, ZOOM_MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangerTool {
    Select, // ⇱ Markör / Flytta / Trimma
    Paint,  // ✎ Rita
    Slice,  // ✂ Klipp / Sax
    Erase,  // 🗑 Radera
    Mute,   // 🔇 Muta
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionDragMode {
    Move,
    TrimStart,
    TrimEnd,
}

#[derive(Debug, Clone)]
pub struct RegionDragState {
    pub track_idx: usize,
    pub region_idx: usize,
    pub mode: RegionDragMode,
    pub initial_start_bar: f32,
    pub initial_length_bars: f32,
    pub initial_sample_offset_sec: f32,
    pub initial_loop_length_bars: f32,
    pub drag_start_mouse_x: f32,
}

#[derive(Clone)]
pub struct LibrarySampleItem {
    #[allow(dead_code)]
    pub id: usize,
    pub name: String,
    pub category: String,
    pub icon: String,
    pub default_note: u8,
    pub color: Color32,
    pub waveform: Vec<f32>,
    pub file_path: Option<String>,
}

#[derive(Clone)]
pub struct ChannelStrip {
    pub name: String,
    pub icon: String,
    pub color: Color32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub steps: [bool; MAX_STEPS],
    pub notes: [u8; MAX_STEPS],
    pub pitch_semitones: i8, // -24 .. +24 st
    pub pitch_fine_cents: f32, // -100.0 .. +100.0 cents
    pub sample_start: f32, // 0.0 .. 1.0 (Chopping start)
    pub sample_end: f32, // 0.0 .. 1.0 (Chopping end)
    /// Slicekartan ur kanalens ljud (Fas 8.7): `(start, slut)` i 0..1, i den
    /// ordning slagen kommer. Tom = ingen detektering har gjorts (eller så
    /// hittades inga slag). Slicen som spelas är `sample_start`/`sample_end`.
    pub slices: Vec<(f32, f32)>,
    /// Den **gamla** attack/decay-ratten (0.0 .. 1.0). Ingen DSP läste den — den finns
    /// kvar därför att ett sparat projekt inte ska tappa ett värde, och den får sätta
    /// attacken första gången ett gammalt projekt öppnas i samplern (Fas 8.4).
    pub attack_decay: f32, // 0.0 .. 1.0
    /// **Samplern** (Fas 8.4): loopläge, loop-punkter som andelar av filen, riktning och
    /// amplitud-ADSR. `LoopMode::Off` + `AdsrParams::identity()` = en-skottsprovspelning,
    /// alltså exakt det kanalen gjorde innan samplern fanns.
    pub loop_mode: crate::audio::LoopMode,
    pub sample_loop_start: f32,
    pub sample_loop_end: f32,
    pub ping_pong: bool,
    pub amp_env: crate::audio::envelope::AdsrParams,
    /// **Velocitetskänsligheten** (Fas 8.4/7), 0,0–1,0: hur mycket en nots anslag får påverka
    /// nivån. `1,0` är den linjära faktor velocityn alltid har haft (alltså oförändrat ljud för
    /// ett projekt från före ratten), `0,0` stänger av den helt.
    pub velocity_sensitivity: f32,
    /// **Anslagets kurva** (Fas 8.4/7): rak (standarden — kanalen har alltid haft den, så ett
    /// projekt från före valet låter identiskt) eller kvadratisk ("curve 2", FL/Abletons kurva).
    pub velocity_curve: crate::audio::envelope::VelocityCurve,
    /// **Filterenvelopen** (Fas 8.4/7): kanalens lågpassfilter med sin **egen** ADSR. Standard
    /// är avstängt, alltså oförändrat ljud för ett projekt från före filtret.
    pub filter: crate::audio::filter::SamplerFilter,
    /// **Multi-samples** (Fas 8.4/7): en keymap av zoner — sampel med tonhöjds- och
    /// anslagsintervall. **Tom = kanalens eget sampel**, alltså exakt beteendet före keymappen.
    pub zones: Vec<crate::audio::keymap::SampleZone>,
    pub is_reverse: bool,
    pub waveform_preview: Vec<f32>,
    pub sample_path: Option<String>,
    pub pcm_audio: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    pub sample_base_note: u8,
}

pub use crate::audio::tempo::MAX_STEPS;

/// Antal steg som spelas i ett pattern: en takt är 16, och ett pattern är 1–4 takter.
/// Konturerna som skickas till motorn: mallen med lutning (partial k får `decay/(k+1)^lutning`,
/// alltså dör höga partialer först), och undantagen ovanpå. `overrides` är (partial, kontur) med
/// 0-baserat partialindex, så en enskild partial kan ha sin egen kurva utan att mallen tappas.
pub fn partial_envelopes_for(
    on: bool,
    template: [f32; 3],
    tilt: f32,
    overrides: &[(usize, [f32; 3])],
) -> [[f32; 3]; crate::audio::synth::HARMONIC_COUNT] {
    let mut envs = crate::audio::synth::PARTIAL_ENV_NEUTRAL;
    if on {
        for (k, env) in envs.iter_mut().enumerate() {
            env[0] = template[0];
            env[1] = template[1] / (k as f32 + 1.0).powf(tilt);
            env[2] = template[2];
        }
        for (k, env) in overrides {
            if let Some(slot) = envs.get_mut(*k) {
                *slot = *env;
            }
        }
    }
    envs
}

pub fn pattern_steps(bars: usize) -> usize {
    bars.clamp(1, 4) * crate::audio::tempo::STEPS_PER_BAR
}

/// Blocket som spelar i takt `bar`: `(pattern, stegförskjutning i patternet)`.
///
/// Startpunkterna ligger i `clips` och längden i `clip_bars`. En takt som ligger inne i
/// ett block är tom i `clips` — därför söks starten bakåt, högst tre takter (fyra takter
/// är det längsta ett pattern får vara).
pub fn clip_at_bar(track: &PlaylistTrack, bar: usize) -> Option<(usize, usize)> {
    clip_at(&track.clips, &track.clip_bars, bar)
}

/// Samma uppslag mot råa arrayer, för exportens ögonblicksbild av ett spår.
pub fn clip_at(
    clips: &[Option<usize>; 32],
    clip_bars: &[u8; 32],
    bar: usize,
) -> Option<(usize, usize)> {
    let first = bar.saturating_sub(3);
    for start in (first..=bar).rev() {
        if let Some(pat) = clips.get(start).copied().flatten() {
            let bars = (*clip_bars.get(start).unwrap_or(&1) as usize).clamp(1, 4);
            if bar < start + bars {
                return Some((pat, (bar - start) * crate::audio::tempo::STEPS_PER_BAR));
            }
        }
    }
    None
}

/// Bendpunkterna som ska skickas i ett steg: `(kanal, halvtoner, fördröjning i samples)`.
///
/// Punkten hör till det steg dess `pos` ligger i, och fördröjningen är resten — så en
/// böjning mitt i steget landar samplenoggrant i stället för på steggränsen.
pub fn bends_for_step(bends: &[BendPoint], step: usize, step_samples: u32) -> Vec<(u8, f32, u32)> {
    let mut out = Vec::new();
    for b in bends {
        if !b.pos.is_finite() || b.pos < 0.0 {
            continue;
        }
        let at = b.pos.floor() as usize;
        if at == step {
            out.push((
                b.channel,
                b.semitones,
                ((b.pos - at as f32) * step_samples as f32).max(0.0) as u32,
            ));
        }
    }
    out
}

/// Lägger en bendpunkt i kurvan. En ny punkt på samma steg och kanal **ersätter** den
/// gamla, så ett hjul som rör sig inte fyller kurvan med hundra punkter per steg.
pub fn push_bend(bends: &mut Vec<BendPoint>, point: BendPoint) {
    let at = point.pos.floor();
    if let Some(old) = bends.iter_mut().find(|b| {
        b.channel == point.channel && (b.pos.floor() - at).abs() < f32::EPSILON
    }) {
        *old = point;
    } else {
        bends.push(point);
        bends.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    }
}

/// Nästa steg i pattern-läget, med omslag vid patternets längd (inte vid 16).
pub fn next_pattern_step(step: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (step + 1) % len
    }
}

/// En punkt i patternets bendkurva (MPE). `pos` är i **steg** i patternet, `channel` är
/// motorns kanal (0 = master, 1–16 = medlemskanal) och `semitones` är böjningen där.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BendPoint {
    pub pos: f32,
    pub channel: u8,
    pub semitones: f32,
}

/// **Ett komp** (Ableton): från `from_step` till nästa komp spelas lane `take`
/// (0 = lane 1). Täckningen är implicit — listan sorteras och varje komp gäller fram till
/// nästa komps början, så att sätta ett komp mitt i ett annat delar det utan att räkna om
/// några gränser.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompSlice {
    pub from_step: f32,
    pub take: usize,
}

/// Tagningarna i ett pattern, som en lista: lane 1 först.
impl Pattern {
    /// Tagningen som spelas och redigeras just nu.
    pub fn active_take(&self) -> &crate::midi_take::Take {
        if self.active_take == 0 {
            &self.take
        } else {
            self.alt_takes.get(self.active_take - 1).unwrap_or(&self.take)
        }
    }

    pub fn active_take_mut(&mut self) -> &mut crate::midi_take::Take {
        let slot = self.active_take;
        if slot == 0 || slot > self.alt_takes.len() {
            return &mut self.take;
        }
        &mut self.alt_takes[slot - 1]
    }

    /// Alla lanes, lane 1 först.
    pub fn take_lanes(&self) -> Vec<&crate::midi_take::Take> {
        std::iter::once(&self.take)
            .chain(self.alt_takes.iter())
            .collect()
    }

    /// Låter `take` spela från `from_step` till nästa komp. Ett komp på samma steg ersätts.
    pub fn comp_from(&mut self, from_step: f32, take: usize) {
        let from_step = from_step.clamp(0.0, 15.99);
        let take = take.min(self.alt_takes.len());
        self.comp
            .retain(|slice| (slice.from_step - from_step).abs() > 0.001);
        self.comp.push(CompSlice { from_step, take });
        self.comp.sort_by(|a, b| a.from_step.total_cmp(&b.from_step));
    }

    /// Tagningen som spelas: den aktiva lane:n, eller — om patternet är kompat — noterna ur
    /// varje lane för de steg där den lane:n är vald. Ett komp som pekar utanför lanes
    /// faller tillbaka på den aktiva lane:n.
    pub fn comped_take(&self) -> crate::midi_take::Take {
        if self.comp.is_empty() {
            return self.active_take().clone();
        }
        let lanes = self.take_lanes();
        let last_lane = lanes.len().saturating_sub(1);
        let mut notes = Vec::new();
        for (lane, take) in lanes.iter().enumerate() {
            for note in &take.notes {
                let chosen = self
                    .comp
                    .iter()
                    .rev()
                    .find(|slice| slice.from_step <= note.pos)
                    .map(|slice| slice.take.min(last_lane))
                    .unwrap_or(self.active_take.min(last_lane));
                if chosen == lane {
                    notes.push(note.clone());
                }
            }
        }
        notes.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        crate::midi_take::Take { notes }
    }
}

#[derive(Clone, Debug)]
pub struct Pattern {
    pub name: String,
    pub color: Color32,
    /// 1–4 takter. Rutnätet rymmer alltid 64 steg, så en längdändring kastar aldrig noter —
    /// stegen utanför längden ligger kvar och hörs igen om patternet blir längre.
    pub bars: usize,
    /// MPE-bend som hör till patternet. Ligger i patternet (inte i tagningen), eftersom
    /// det är patternet som spelas — både i pattern-läget och från ett arrangemangsblock.
    pub bends: Vec<BendPoint>,
    /// **Tagningar** (Ableton take lanes): fler inspelningar av samma slinga. `take` är
    /// alltid lane 1; de här är lane 2 och framåt. `active_take` väljer vilken som spelas.
    pub alt_takes: Vec<crate::midi_take::Take>,
    /// 0 = `take`, 1.. = `alt_takes[n-1]`.
    pub active_take: usize,
    /// **Kompning:** vilka steg som spelas ur vilken lane. Tom = bara `active_take`.
    pub comp: Vec<CompSlice>,
    pub channel_steps: Vec<[bool; MAX_STEPS]>,
    pub channel_notes: Vec<[u8; MAX_STEPS]>,
    pub piano_roll_grid: [[bool; MAX_STEPS]; 24],
    /// Den inspelade tagningen med sin faktiska tajming (Fas 6.4). Rutnätet
    /// säger *vilka* steg som spelar, tagningen säger *när* inom steget — det är
    /// den som gör kvantisering och humanisering möjliga i efterhand.
    pub take: crate::midi_take::Take,
}

/// En grupp spår i spellistan (FL). Spåren i en grupp kan fällas ihop, och då syns bara det
/// första — annars gick gruppen inte att fälla upp igen utan att leta i menyer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TrackGroup {
    pub name: String,
    pub collapsed: bool,
}

/// Är spåret dolt därför att dess grupp är ihopfälld? Det **första** spåret i gruppen syns
/// alltid, så att gruppen har en rad att klicka på.
pub fn track_hidden(tracks: &[PlaylistTrack], groups: &[TrackGroup], idx: usize) -> bool {
    let Some(group_idx) = tracks.get(idx).and_then(|t| t.group) else {
        return false;
    };
    let Some(group) = groups.get(group_idx) else {
        return false;
    };
    if !group.collapsed {
        return false;
    }
    let first_member = tracks.iter().position(|t| t.group == Some(group_idx));
    first_member != Some(idx)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeSnapMode {
    FreeHundredth, // ⚡ 0.01s (Fri / Högsta precision)
    Snap16th,      // 1/16-dels takt
    SnapBeat,      // 1/4 Beat
    SnapBar,       // 1 Hel takt
}

pub fn format_time_hundredths(seconds: f32) -> String {
    let total_cs = (seconds.max(0.0) * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_sec = total_cs / 100;
    let s = total_sec % 60;
    let m = total_sec / 60;
    format!("{:02}:{:02}.{:02}", m, s, cs)
}

pub fn format_bar_subdivisions(bar_float: f32) -> String {
    let bar_idx = bar_float.floor() as i32 + 1;
    let rem_bar = (bar_float - bar_float.floor()).max(0.0);
    let beat = (rem_bar * 4.0).floor() as i32 + 1;
    let step = ((rem_bar * 16.0) % 4.0).floor() as i32 + 1;
    let cs = ((rem_bar * 100.0) % 25.0).floor() as i32;
    format!("{} {:03}.{}.{} (+{:02}cs)", crate::i18n::t("Takt"), bar_idx, beat, step, cs)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackKind {
    VocalAudio,
    CustomAudio,
    Drums,
    SynthLead,
    Bassline,
    Fx,
}

#[derive(Clone, Debug)]
pub struct SongMarker {
    pub start_bar: usize,
    pub length_bars: usize,
    pub name: String,
    pub color: Color32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AudioRegion {
    pub id: usize,
    pub name: String,
    pub start_bar: f32,
    pub length_bars: f32,
    pub sample_offset_sec: f32,
    pub source_path: Option<String>,
    pub waveform_peaks: Vec<f32>,
    pub volume: f32,
    pub fade_in_bars: f32,
    pub fade_out_bars: f32,
    pub muted: bool,
    #[serde(default)]
    pub is_reverse: bool,
    #[serde(skip, default = "default_region_color")]
    pub color: Color32,
    #[serde(default)]
    pub loop_length_bars: f32,
    /// Tempot klippets ljud spelades in i (Fas 8.10).
    ///
    /// **0,0 = okänt**, och då rörs ljudet inte när projektets tempo ändras. Det
    /// gäller varje projekt som sparades innan fältet fanns (fältet är
    /// `#[serde(default)]`) och allt som kommer från biblioteket — en källa utan
    /// känt tempo sträcks inte i smyg.
    #[serde(default)]
    pub source_bpm: f32,
    /// Klippet vill ha **bandspelaren** (Fas 8.10 steg 2).
    ///
    /// Ett **aktivt val, aldrig standard**: höjs tempot följer tonhöjden med (Abletons
    /// *Re-Pitch*, FL:s *Resample*). Standard är tonhöjdsbevarande sträckning, för en smurf
    /// som uppstår av misstag är värre än en effekt man får leta efter.
    #[serde(default)]
    pub tape: bool,
    /// **Warp-läget för det här klippet** (0.9.44). Läget hör till sträckningens nyckel, så
    /// två klipp med samma källa kan sträckas olika. Saknas i äldre filer — då gäller
    /// `Complex`, samma läge som var projektets enda förut.
    #[serde(default)]
    pub warp_mode: crate::audio::warp::WarpMode,
}

/// Flyttar låtsektioner när tid infogas eller tas bort. `delta` är positiv vid infogning.
/// Ackordkvalitet. Intervallen är halvtoner från grundtonen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Dom7,
    Min7,
    Sus4,
    Dim,
}

impl ChordQuality {
    pub const ALL: [ChordQuality; 6] = [
        ChordQuality::Major,
        ChordQuality::Minor,
        ChordQuality::Dom7,
        ChordQuality::Min7,
        ChordQuality::Sus4,
        ChordQuality::Dim,
    ];

    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordQuality::Major => &[0, 4, 7],
            ChordQuality::Minor => &[0, 3, 7],
            ChordQuality::Dom7 => &[0, 4, 7, 10],
            ChordQuality::Min7 => &[0, 3, 7, 10],
            ChordQuality::Sus4 => &[0, 5, 7],
            ChordQuality::Dim => &[0, 3, 6],
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            ChordQuality::Major => "",
            ChordQuality::Minor => "m",
            ChordQuality::Dom7 => "7",
            ChordQuality::Min7 => "m7",
            ChordQuality::Sus4 => "sus4",
            ChordQuality::Dim => "dim",
        }
    }
}

/// Ett ackord som gäller från och med en takt tills nästa ackord tar över.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChordEvent {
    pub bar: usize,
    /// Grundton som pch 0–11 (0 = C).
    pub root: u8,
    pub quality: ChordQuality,
}

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Notnamn för en pch (0–11), för fönster och listor.
pub fn note_name_of(pch: u8) -> &'static str {
    NOTE_NAMES[(pch % 12) as usize]
}

impl ChordEvent {
    pub fn name(&self) -> String {
        format!("{}{}", NOTE_NAMES[(self.root % 12) as usize], self.quality.suffix())
    }

    /// Ackordets toner som pch.
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.quality
            .intervals()
            .iter()
            .map(|i| (self.root + *i) % 12)
            .collect()
    }
}

/// Ackordet som gäller i en takt: det senaste ackord som börjar på eller före takten.
pub fn chord_at(chords: &[ChordEvent], bar: usize) -> Option<ChordEvent> {
    chords
        .iter()
        .filter(|c| c.bar <= bar)
        .max_by_key(|c| c.bar)
        .copied()
}

/// Flyttar noten till **närmaste** ackordston.
///
/// Det är valet "melodins karaktär": noten rör sig som mest en liten bit i stället för att
/// transponeras med ett fast intervall, så melodins rörelse och rytmiska profil behålls.
/// Vid lika avstånd uppåt och nedåt väljs den nedre — den låter minst påträngande.
pub fn snap_to_chord(midi: u8, chord: &ChordEvent) -> u8 {
    let classes = chord.pitch_classes();
    if classes.contains(&(midi % 12)) {
        return midi;
    }
    for step in 1..=6u8 {
        if midi >= step && classes.contains(&((midi - step) % 12)) {
            return midi - step;
        }
        if midi + step <= 127 && classes.contains(&((midi + step) % 12)) {
            return midi + step;
        }
    }
    midi
}

/// Nästa sektion i kedjan när spelhuvudet lämnar sin sista takt.
///
/// `sections` är `(start_bar, length_bars)` per markör och `chain` är markörindex i
/// spelordning — samma sektion får förekomma flera gånger, vilket är hela poängen med en
/// arranger track (A B A C). `None` betyder "spela vidare linjärt": antingen är takten inte
/// slutet på en sektion, eller så står sektionen utanför kedjan.
pub fn next_chain_bar(bar: usize, sections: &[(usize, usize)], chain: &[usize]) -> Option<usize> {
    if chain.is_empty() {
        return None;
    }
    let idx = sections
        .iter()
        .position(|(start, len)| *len > 0 && bar + 1 == start + len)?;
    let pos = chain.iter().position(|c| *c == idx)?;
    let next = chain[(pos + 1) % chain.len()];
    sections.get(next).map(|(start, _)| *start)
}

/// Vad som händer när takten tar slut.
///
/// Slingan gäller **bara när den är på**. Utan det här var slingan obetingad: låten
/// hoppade tillbaka vid loop-slutet även med slingan avstängd, och allt efter
/// loop-regionen gick aldrig att höra — det såg ut som att låten stannade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAdvance {
    /// Spela vidare linjärt.
    Continue,
    /// Hoppa till den här takten (slingan).
    Wrap(usize),
    /// Låten är slut: stanna där vi är.
    Stop,
}

/// `bar` är takten vi just lämnade (nästa takt är `bar + 1`).
pub fn after_bar_end(
    bar: usize,
    loop_enabled: bool,
    loop_start: usize,
    loop_end: usize,
    max_bars: usize,
) -> BarAdvance {
    let next = bar + 1;
    if loop_enabled && next >= loop_end.max(loop_start + 1) {
        return BarAdvance::Wrap(loop_start);
    }
    if next >= max_bars.max(1) {
        return BarAdvance::Stop;
    }
    BarAdvance::Continue
}

pub fn shift_song_markers(markers: &mut [SongMarker], at_bar: f32, delta: i64) -> usize {
    let mut moved = 0;
    for m in markers.iter_mut() {
        if m.start_bar as f32 >= at_bar - 1e-4 {
            let shifted = m.start_bar as i64 + delta;
            m.start_bar = shifted.max(0) as usize;
            moved += 1;
        }
    }
    moved
}

pub(crate) fn default_region_color() -> Color32 {
    Theme::TRACK_BLUE
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TrackEq {
    #[serde(default)]
    pub low_gain_db: f32,
    #[serde(default = "default_low_freq")]
    pub low_freq: f32,
    #[serde(default)]
    pub mid_gain_db: f32,
    #[serde(default = "default_mid_freq")]
    pub mid_freq: f32,
    #[serde(default = "default_mid_q")]
    pub mid_q: f32,
    #[serde(default)]
    pub high_gain_db: f32,
    #[serde(default = "default_high_freq")]
    pub high_freq: f32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_low_freq() -> f32 { 100.0 }

fn default_mid_freq() -> f32 { 1500.0 }

fn default_mid_q() -> f32 { 1.0 }

fn default_high_freq() -> f32 { 8000.0 }

fn default_true() -> bool { true }

pub(crate) fn default_comp_thresh() -> f32 { 0.0 }

pub(crate) fn default_comp_ratio() -> f32 { 1.0 }

/// Automatable per-track parameters. The order defines the cache index used by
/// `PlaylistTrack::automation_last`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum AutomationParam {
    Volume,
    Pan,
    ReverbSend,
    DelaySend,
    /// Kompressorns tröskel i dB. Fanns i `SetStemMixParams` hela tiden — det var bara
    /// automationen som inte nådde den (Fas 8.8).
    CompThreshold,
    /// Kompressorns förhållande.
    CompRatio,
    /// Transponering i halvtoner.
    Pitch,
    /// EQ: låg bandets förstärkning i dB (Fas 8.8). Banden ligger på spåret, som lane:n hör
    /// till — därför hör de hemma här och inte hos en buss.
    EqLow,
    /// EQ: mittenbandet.
    EqMid,
    /// EQ: hög bandet.
    EqHigh,
}

/// **Logotypen i appen** — samma fil som fönsterikonen (`assets/sonix.png`, och samma bytes
/// som `main.rs` ger `with_icon`).
///
/// Den avkodas **en gång** och hålls i en statisk cache: `include_bytes!` gör att bilden följer
/// med binären (ingen fil som kan saknas vid körning, och ingen sökväg att leta upp i), och
/// `OnceLock` ser till att avkodningen inte sker per bildruta. `None` betyder att bilden inte
/// gick att läsa — då ritas ingen bild i stället för en grå ruta, och det syns i `Om`-rutan
/// vilken väg som togs.
static SONIX_LOGO: std::sync::OnceLock<Option<egui::ColorImage>> = std::sync::OnceLock::new();

/// Logotypen som `ColorImage`, avkodad en gång. `None` om PNG:n inte gick att läsa.
pub fn sonix_logo() -> Option<&'static egui::ColorImage> {
    SONIX_LOGO
        .get_or_init(|| {
            let bytes = include_bytes!("../../../assets/sonix.png");
            let img = image::load_from_memory(bytes).ok()?;
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
        })
        .as_ref()
}

/// A single breakpoint on an automation curve.
///
/// **Positionen är i takter** (Fas 8.2), inte sekunder: en punkt hör till en plats i musiken.
/// Med ett enda tempo är det ingen skillnad, men efter ett tempobyte är takten den enda form
/// som är rätt — och sekunder-per-takt är inte ett tal över ett byte. Filformen skiljer sig
/// från den här typen: se [`AutomationPointOnDisk`], som också kan läsa gamla projekt.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationPoint {
    pub time_bars: f32,
    pub value: f32,
}

/// **Hur en punkt ser ut i en fil** — och varför den är en egen typ.
///
/// Projekt skrivna före Fas 8.2 har `time_secs`. Att bara byta namn på fältet hade fått en
/// gammal fil att läsas som *noll* takter (ett saknat fält med `#[serde(default)]` är tyst),
/// alltså hela kurvan hopklämd på takt 0 utan ett ord om saken. Därför bär filformen **båda**
/// namnen, och formen avgör vad som gäller: `time_bars` läses rakt av, `time_secs` räknas om
/// **genom tempokartan** när projektet har laddats (kartan behövs för omräkningen och finns
/// inte i filen).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationPointOnDisk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_bars: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_secs: Option<f32>,
    pub value: f32,
}

/// Filformen av en lane. Se [`AutomationPointOnDisk`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationLaneOnDisk {
    pub param: AutomationParam,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPointOnDisk>,
}

/// One automated parameter for a track. Points are kept sorted by time.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AutomationLane {
    pub param: AutomationParam,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
}

/// **Kurvans regel, på ett ställe** (Fas 8.2 och 8.8).
///
/// Linearly interpolated value at **`bar`**; constant before the first and after the last
/// point. `None` when there are no points. Både spårets lane och bussens lane går genom den
/// här funktionen — annars vore det två kurvor som ser likadana ut och beter sig olika, och
/// en fix i den ena skulle lämna den andra fel.
///
/// Argumentet är en takt, inte en sekund — samma typ, olika enhet, och därför inget som
/// kompilatorn kan vakta. Anroparen konverterar spelhuvudets sekunder **en gång** via
/// tempokartan (se `apply_automation`).
pub fn lane_value_at(points: &[AutomationPoint], bar: f32) -> Option<f32> {
    if points.is_empty() {
        return None;
    }
    let first = &points[0];
    if bar <= first.time_bars {
        return Some(first.value);
    }
    let last = points.last().unwrap();
    if bar >= last.time_bars {
        return Some(last.value);
    }
    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if bar >= a.time_bars && bar <= b.time_bars {
            let span = (b.time_bars - a.time_bars).max(1e-6);
            let f = (bar - a.time_bars) / span;
            return Some(a.value + (b.value - a.value) * f);
        }
    }
    Some(last.value)
}

/// **En buss äger sina egna kurvor** (Fas 8.8).
///
/// Industristandarden är entydig: i Ableton är gruppen ett eget spår med egna lanes, i FL
/// automatiseras *inserten* och klippet namnges efter bussen, i Reaper har folder-spåret sina
/// egna enveloper och i Logic har aux-strippen egen automation. Ingen av dem lägger bussens
/// kurva under ett spår som *skickar* till bussen, och skälet är enkelt: flera spår kan skicka
/// till samma buss, så en kurva under ett av dem hade gjort anspråk på ett globalt värde — och
/// vem som ägde bussen hade berott på vilket spår man råkade titta på.
///
/// Bussen har i dag bara **nivå** (och mute/solo), så lane:n bär punkter och inget parameterval.
/// Får bussen fler parametrar blir det här ett val — men inte förrän dess.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BusAutomationLane {
    /// Bussen kurvan hör till (`0..NUM_BUSES`). En fil med ett ogiltigt index ignoreras vid
    /// spelning i stället för att tyst styra buss 0.
    pub bus: usize,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
}

/// **En plugins kurva** (Fas 8.8) — bussens form, och av samma skäl.
///
/// `AutomationParam` kan inte bära den: den enumen är **statisk** (tio varianter kända vid
/// kompilering, `ALL`, och ett index in i en cache med fast längd), medan en plugins
/// parametrar är en **runtime-lista** som varierar per plugin och per instans, med `u32`-id:n
/// som bara finns efter att instansen laddats. Lane:n bär därför `param_id` — samma tal som
/// `AudioCommand::SetPluginParameter` tar — och **namnet**, så att kurvan fortfarande går att
/// läsa i ett projekt där pluginen inte är laddad: ett id utan namn säger ingenting.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginAutomationLane {
    /// Spåret vars plugin-insert kurvan styr.
    pub track: usize,
    pub param_id: u32,
    #[serde(default)]
    pub param_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
    /// Senast skickat värde (`None` = aldrig). **Inte en del av filen** — det är motorns
    /// tillstånd speglat, inte en inställning — och samma skäl som spårets `automation_last`:
    /// utan cachen hade varje bildruta under uppspelning skickat ett kommando. Bussens kurva
    /// behövde ingen cache eftersom bussens *eget* värde är UI-tillstånd; en plugin-parameters
    /// värde ägs av motorn och går inte att läsa tillbaka här.
    #[serde(skip)]
    pub last_sent: Option<f64>,
}

impl PluginAutomationLane {
    /// Pluginens kurva vid en takt — **samma regel som spårets och bussens**, se
    /// [`lane_value_at`]. Att de tre går genom samma funktion är vad som gör att en fix i
    /// kurvan gäller alla; två egna kopior hade sett likadana ut och betett sig olika.
    pub fn value_at(&self, bar: f32) -> Option<f32> {
        lane_value_at(&self.points, bar)
    }

    /// Klämmer ett kurvvärde till parameterns **egna** område.
    ///
    /// En CLAP-parameter mäter inte i 0..1: en nivå kan gå i dB och en frekvens i Hz, och
    /// pluginens `min_value`/`max_value` är det enda som vet. Utan klämningen hade en kurva
    /// ritad i fel skala skickat värden pluginen inte har — och tyst styrt fel.
    pub fn clamp_to_range(raw: f32, range: (f64, f64)) -> f64 {
        let (lo, hi) = if range.0 <= range.1 { range } else { (range.1, range.0) };
        (raw as f64).clamp(lo, hi)
    }
}

/// **En pedals ljudande fält** — se [`NonTrackSound::pedals`].
///
/// Namn, färg och kategori står kvar i `FxPedal`: de är etiketter som aldrig ändrar ljudet, och en
/// ångring ska inte skapas av att ett fönster byter flik eller en pedal byter namn.
#[derive(Clone, Debug, PartialEq)]
pub struct PedalSound {
    pub id: String,
    pub enabled: bool,
    pub p1_val: f32,
    pub p2_val: f32,
    pub p3_val: f32,
}

/// **Det som hörs men ligger utanför spåren** (Fas 6.2): masterns kedja och plugin-kurvorna.
///
/// Klumpen finns för att **snapshoten, digesten och återställningen ska läsa samma fältlista**.
/// Annars hade de varit tre egna listor som sett likadana ut och glidit isär — och felet hade
/// blivit tyst: en ångring som inte tar med mastern låter fel utan att något *ser* fel ut.
///
/// **Läget fram till 2026-09-15:** mastern och plugin-kurvorna låg utanför `playlist_tracks`,
/// och därmed utanför både ångringen och mixerdigesten. En ändrad master-FX gick inte att ångra
/// alls, och en ändrad plugin-kurva syntes i gränssnittet medan pluginen spelade vidare med det
/// gamla värdet — samma klass av fel som `sync_track_audio_state` löste för spåren i 6.2
/// ("ångringen hörs, inte bara syns").
#[derive(Clone, Debug)]
pub struct NonTrackSound {
    /// Plugin-kurvorna (Fas 8.8). `last_sent` följer med i klonen, men den **nollas** vid
    /// återställning — den är motorns spegel, inte en inställning, och nollningen är hela
    /// mekanismen som gör att en ångrad kurva hörs igen (se `apply_non_track_sound`).
    pub plugin_automation: Vec<PluginAutomationLane>,
    pub delay: crate::audio::DelayParams,
    pub reverb: crate::audio::ReverbParams,
    pub drive: f32,
    pub master_volume: f32,
    pub pedals: Vec<PedalSound>,
    pub eq_nodes: Vec<crate::ui::fx_rack_modal::VisualEqNode>,
    pub compressor_release_ms: f32,
}

/// **Vad en automationskurva styr** (Fas 8.8).
///
/// Spårets egna rattar är den statiska listan; pluginens är en runtime-lista. De två slagen
/// blandas inte i samma lista — men *valet* i gränssnittet är ett enda, och det är det här.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutomationTarget {
    /// Spårets egen ratt (`AutomationParam`).
    Track(AutomationParam),
    /// En parameter hos spårets plugin-insert: `(spår, pluginets parameter-id)`.
    Plugin { track: usize, param_id: u32 },
}

/// Det ritningen behöver av en kurva, oavsett vilken lista den bor i.
///
/// Spårkurvan och plugin-kurvan ligger i var sin `Vec` (olika typer, olika livstider), så en
/// gemensam *referens* är vad som låter ritningskoden förbli en enda väg i stället för två
/// som driver isär.
pub struct AutomationCurve<'a> {
    pub enabled: bool,
    pub points: &'a [AutomationPoint],
}

/// **Snäpp en taktposition** (Fas 8.2 steg 3). Ett sextondelssteg är en *plats i takten*,
/// inte ett antal sekunder — därför räknas snäppet i takter, och sekunden hämtas ur
/// tempokartan **efteråt**. Ren funktion med egna prov, för att de fem ställena som snäpper
/// (linjalen, släppet, spliten) ska svara likadant: med ett enda tempo är svaret detsamma
/// som när räkningen gick via sekunder, och efter ett tempobyte är det den enda formen som
/// är rätt.
///
/// `FreeHundredth` lämnas orörd: där *är* sekunden enheten, och avrundningen sker av
/// anroparen som har kartan.
pub(crate) fn snap_bar(mode: TimeSnapMode, bar: f32) -> f32 {
    match mode {
        TimeSnapMode::FreeHundredth => bar,
        TimeSnapMode::Snap16th => (bar * 16.0).round() / 16.0,
        TimeSnapMode::SnapBeat => (bar * 4.0).round() / 4.0,
        TimeSnapMode::SnapBar => bar.round(),
    }
}

#[derive(Clone, Debug)]
pub struct PlaylistTrack {
    pub name: String,
    #[allow(dead_code)]
    pub icon: &'static str,
    pub kind: TrackKind,
    pub color: Color32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    #[allow(dead_code)]
    pub is_rec_armed: bool,
    pub regions: Vec<AudioRegion>,   // Continuous multi-track audio stems and slices
    /// **Startpunkter**: `clips[bar] = Some(p)` betyder att pattern p *börjar* i takt bar.
    /// Längden står i `clip_bars[bar]`; taken efter starten är tomma i listan och ritas
    /// som en del av blocket. Ett block är alltså en post, inte en post per takt.
    pub clips: [Option<usize>; 32],  // Pattern triggers
    /// Längden i takter för blocket som börjar på samma index som `clips`. 1 = en takt.
    pub clip_bars: [u8; 32],
    #[allow(dead_code)]
    /// Vågformens flernivå-cache (Fas 8.3): `(nyckel, cache)`.
    ///
    /// Byggs vid behov när spårets regioner ritas, och görs om när ljudet byts —
    /// nyckeln är buffertens identitet, så en ny tagning eller en frysning ger en
    /// ny cache utan att någon behöver komma ihåg att säga till. Att bygga den på
    /// de tio ställen där `pcm_audio` sätts vore tio chanser att glömma ett.
    pub waveform_cache: Option<(u64, crate::audio::waveform::WaveformCache)>,
    /// Nyckeln som en arbetstråd håller på att bygga, om någon gör det.
    pub waveform_cache_pending: Option<u64>,
    #[allow(dead_code)]
    pub custom_clip_name: Option<String>,
    pub pcm_audio: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    /// Fruset spår (Tier 2): färdigrenderat ljud + fingeravtrycket av källan.
    pub frozen: Option<FrozenTrack>,
    /// Det frusna ljudet inläst, så att en omladdning (ClearAllStemTracks +
    /// resync, som vid projektinläsning) kan skicka det igen.
    pub frozen_pcm: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    pub eq: TrackEq,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    /// Cue-send till monitorbuss A/B (0–1). Hörs bara när Control Room lyssnar på bussen.
    pub cue_a: f32,
    pub cue_b: f32,
    pub pitch_semitones: f32,
    /// Per-parameter automation curves for this track (Fas 5.4).
    pub automation: Vec<AutomationLane>,
    /// Last automation value sent to the engine per `AutomationParam` index
    /// (NaN = never sent). Prevents command spam while playing.
    pub automation_last: [f32; AutomationParam::COUNT],
    /// Grupp i spellistan (`None` = ingen grupp). Sparas inte i projektet än.
    pub group: Option<usize>,
    /// Sub-mix bus this track feeds (`0..NUM_BUSES`) — Fas 5.2.
    pub bus: usize,
    /// Optional VCA control group (`0..NUM_VCAS`) — Fas 5.2.
    pub vca: Option<usize>,
    /// Sidokedja (Fas 8.3): spåret duckas av det här spårets ljud. `None` = ingen.
    pub sidechain_from: Option<usize>,
    /// Hur mycket spåret sänks (dB) när key-signalen är över tröskeln.
    pub sidechain_amount_db: f32,
    /// Tröskeln key-signalen måste över för att ducka (dB).
    pub sidechain_threshold_db: f32,
    /// Sends (Fas 8.13): en del av spårets signal till andra bussar, utöver den
    /// egna. Målet är en buss — en buss skickar inte vidare, så en send kan aldrig
    /// bli en slinga.
    pub sends: Vec<crate::audio::StemSend>,
    /// Anteckningar för kanalen — FL:s Fruity NoteBook. Följer med projektfilen.
    pub notes: String,
}

/// Default sub-mix bus for a track kind (Fas 5.2): drums → Trummor, bass and
/// melodic synths → Synth, vocals → Vocal, FX/other → FX.
pub fn default_bus_for_kind(kind: TrackKind) -> usize {
    match kind {
        TrackKind::Drums => 1,
        TrackKind::Bassline | TrackKind::SynthLead => 2,
        TrackKind::VocalAudio => 0,
        TrackKind::Fx | TrackKind::CustomAudio => 3,
    }
}

impl TrackEq {
    pub fn to_settings(&self) -> TrackEqSettings {
        TrackEqSettings {
            enabled: self.enabled,
            low_freq: self.low_freq,
            low_gain_db: self.low_gain_db,
            mid_freq: self.mid_freq,
            mid_gain_db: self.mid_gain_db,
            mid_q: self.mid_q,
            high_freq: self.high_freq,
            high_gain_db: self.high_gain_db,
        }
    }
}

impl Default for TrackEq {
    fn default() -> Self {
        Self {
            low_gain_db: 0.0,
            low_freq: 100.0,
            mid_gain_db: 0.0,
            mid_freq: 1500.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            high_freq: 8000.0,
            enabled: true,
        }
    }
}

impl AutomationParam {
    /// **Ordningen är identiteten:** indexet in i `PlaylistTrack::automation_last` härleds ur
    /// den här listan, inte ur en handskriven siffra per variant. Två listor som måste stämma
    /// överens driver isär förr eller senare — det är den här kodbasens mest återkommande fel —
    /// och då pekar en kurva på fel parameter utan att något larmar.
    pub const ALL: [AutomationParam; 10] = [
        AutomationParam::Volume,
        AutomationParam::Pan,
        AutomationParam::ReverbSend,
        AutomationParam::DelaySend,
        AutomationParam::CompThreshold,
        AutomationParam::CompRatio,
        AutomationParam::Pitch,
        AutomationParam::EqLow,
        AutomationParam::EqMid,
        AutomationParam::EqHigh,
    ];

    /// Antalet mål. Används som längd på cachen, så arrayen inte kan bli för kort när ett mål
    /// tillkommer — det var den andra listan som annars hade glidit isär.
    pub const COUNT: usize = AutomationParam::ALL.len();

    pub fn label(self) -> &'static str {
        match self {
            AutomationParam::Volume => crate::i18n::t("Volym"),
            AutomationParam::Pan => crate::i18n::t("Panorering"),
            AutomationParam::ReverbSend => crate::i18n::t("Reverb-send"),
            AutomationParam::DelaySend => crate::i18n::t("Delay-send"),
            AutomationParam::CompThreshold => crate::i18n::t("Kompressor: tröskel (dB)"),
            AutomationParam::CompRatio => crate::i18n::t("Kompressor: förhållande"),
            AutomationParam::Pitch => crate::i18n::t("Transponering (halvtoner)"),
            AutomationParam::EqLow => crate::i18n::t("EQ: låg (dB)"),
            AutomationParam::EqMid => crate::i18n::t("EQ: mitten (dB)"),
            AutomationParam::EqHigh => crate::i18n::t("EQ: hög (dB)"),
        }
    }

    /// Inclusive value range used both for editing and for clamping.
    pub fn range(self) -> (f32, f32) {
        match self {
            AutomationParam::Volume => (0.0, 1.5),
            AutomationParam::Pan => (-1.0, 1.0),
            AutomationParam::ReverbSend
            | AutomationParam::DelaySend
            | AutomationParam::CompRatio => (0.0, 1.0),
            AutomationParam::CompThreshold => (-60.0, 0.0),
            AutomationParam::Pitch => (-24.0, 24.0),
            // Ett standardområde för ett band: brett nog att vara ett verkligt ingrepp, inte så
            // brett att en kurva kan vrida sönder ljudet av misstag.
            AutomationParam::EqLow | AutomationParam::EqMid | AutomationParam::EqHigh => (-18.0, 18.0),
        }
    }

    /// Indexet i cachen, **härlett ur `ALL`** — inte skrivet för hand. Ett prov räknar dem.
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|p| *p == self)
            .expect("varje variant finns i ALL — annars vore listan och koden oense")
    }
}

impl AutomationLaneOnDisk {
    /// Räknar om en lane från fil till den levande formen.
    ///
    /// `tempo` är **projektets** tempokarta, alltså den som gäller när projektet är laddat —
    /// det är därför det här steget ligger vid inläsningen och inte i `Deserialize`.
    /// En punkt som redan står i takter rörs inte; en gammal punkt räknas om via kartan, så
    /// att den hamnar på samma **ställe i musiken** som den lät på när den skrevs.
    pub fn to_live(&self, tempo: &crate::audio::tempo::TempoMap) -> AutomationLane {
        let points = self
            .points
            .iter()
            .map(|p| AutomationPoint {
                time_bars: match (p.time_bars, p.time_secs) {
                    // Nya filer: rakt av.
                    (Some(bars), _) => bars,
                    // Gamla filer: sekunder genom kartan. Det är en *flytt*, inte en gissning.
                    (None, Some(secs)) => tempo.bar_at_secs(secs as f64) as f32,
                    // Varken eller (en handskriven fil): takt 0 är den enda ärliga tolkningen.
                    (None, None) => 0.0,
                },
                value: p.value,
            })
            .collect();
        AutomationLane {
            param: self.param,
            enabled: self.enabled,
            points,
        }
    }
}

impl AutomationLane {
    /// Skriver en lane till filformen: punkterna i **takter** (det nya formatet), sorterade.
    pub fn to_disk(&self) -> AutomationLaneOnDisk {
        AutomationLaneOnDisk {
            param: self.param,
            enabled: self.enabled,
            points: self
                .points
                .iter()
                .map(|p| AutomationPointOnDisk {
                    time_bars: Some(p.time_bars),
                    time_secs: None,
                    value: p.value,
                })
                .collect(),
        }
    }
}

impl BusAutomationLane {
    /// Bussens nivå vid en takt — **samma regel som spårets kurva**.
    pub fn level_at(&self, bar: f32) -> Option<f32> {
        lane_value_at(&self.points, bar)
    }
}

impl AutomationLane {
    /// Spårets kurva vid en takt — samma regel som bussens, se [`lane_value_at`].
    pub fn value_at(&self, bar: f32) -> Option<f32> {
        lane_value_at(&self.points, bar)
    }
}

/// **Punkterna i taktordning** (Fas 8.8) — en regel, inte tre kopior.
///
/// Kurvan interpolerar mellan *angränsande* punkter, så en osorterad lista ger en kurva som
/// hoppar. Spårkurvan, busskurvan och plugin-kurvan sorterar därför med samma funktion: tre
/// egna sorteringar hade sett likadana ut och varit tre chanser att glömma en.
pub(crate) fn sort_automation_points(points: &mut [AutomationPoint]) {
    points.sort_by(|a, b| {
        a.time_bars
            .partial_cmp(&b.time_bars)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

impl PlaylistTrack {
    /// Ett spår kan frysas om det spelas upp av patterns — ljudspår ligger
    /// redan som färdigt ljud och har inget att tjäna på det.
    pub fn can_freeze(&self) -> bool {
        self.frozen.is_none()
            && matches!(
                self.kind,
                TrackKind::Drums | TrackKind::SynthLead | TrackKind::Bassline
            )
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }

    /// **Infoga tid**: allt som börjar vid `at_bar` eller senare flyttas `bars` takter framåt.
    /// En region som spänner över insättningspunkten förlängs i stället — annars hade dess
    /// ljud hamnat i otakt med resten av låten. Källan loopas om den är kortare än regionen.
    /// Motsvarar Abletons `Insert Time`.
    pub fn insert_time(&mut self, at_bar: f32, bars: f32) -> usize {
        if bars <= 0.0 {
            return 0;
        }
        let mut touched = 0;
        for r in &mut self.regions {
            let end = r.start_bar + r.length_bars;
            if r.start_bar >= at_bar - 1e-4 {
                r.start_bar += bars;
                touched += 1;
            } else if end > at_bar + 1e-4 {
                r.length_bars += bars;
                touched += 1;
            }
        }
        touched
    }

    /// **Ta bort tid**: intervallet `[start_bar, end_bar)` försvinner och allt efter flyttas ihop.
    /// Regioner som bara delvis ligger inne kortas; de som ligger helt inne tas bort.
    /// Returnerar `(borttagna, ändrade)`.
    pub fn delete_time(&mut self, start_bar: f32, end_bar: f32) -> (usize, usize) {
        let span = (end_bar - start_bar).max(0.0);
        if span <= 0.0 {
            return (0, 0);
        }
        let mut removed = 0;
        let mut changed = 0;
        self.regions.retain_mut(|r| {
            let r_end = r.start_bar + r.length_bars;
            if r_end <= start_bar + 1e-4 {
                return true; // helt före intervallet
            }
            if r.start_bar >= end_bar - 1e-4 {
                r.start_bar -= span; // helt efter: flyttas ihop
                changed += 1;
                return true;
            }
            if r.start_bar >= start_bar - 1e-4 && r_end <= end_bar + 1e-4 {
                removed += 1; // helt inne: bort
                return false;
            }
            if r.start_bar < start_bar && r_end > end_bar {
                r.length_bars -= span; // spänner över hela: kortas
                changed += 1;
                return true;
            }
            if r.start_bar < start_bar {
                r.length_bars = start_bar - r.start_bar; // slutet skärs av
                changed += 1;
                return true;
            }
            // Börjar inne i intervallet och slutar efter: börja vid intervallets slut.
            let cut = end_bar - r.start_bar;
            r.start_bar = start_bar;
            r.length_bars = r_end - end_bar;
            r.sample_offset_sec += cut;
            changed += 1;
            true
        });
        (removed, changed)
    }

    /// **Duplicera tid**: det som ligger helt inne i intervallet kopieras, och kopian läggs
    /// direkt efter intervallet; allt som låg där flyttas fram lika mycket.
    /// Motsvarar Abletons `Duplicate Time`. Returnerar antalet kopierade regioner.
    pub fn duplicate_time(&mut self, start_bar: f32, end_bar: f32) -> usize {
        let span = (end_bar - start_bar).max(0.0);
        if span <= 0.0 {
            return 0;
        }
        let mut next_id = self.regions.iter().map(|r| r.id).max().unwrap_or(0) + 1;
        let mut copies: Vec<AudioRegion> = self
            .regions
            .iter()
            .filter(|r| {
                r.start_bar >= start_bar - 1e-4 && r.start_bar + r.length_bars <= end_bar + 1e-4
            })
            .cloned()
            .collect();
        if copies.is_empty() {
            return 0;
        }
        let n = copies.len();
        for c in &mut copies {
            c.start_bar += span;
            c.id = next_id;
            next_id += 1;
        }
        for r in &mut self.regions {
            if r.start_bar >= end_bar - 1e-4 {
                r.start_bar += span;
            }
        }
        self.regions.extend(copies);
        n
    }

    /// Skapar korsningar där två regioner på spåret överlappar varandra.
    ///
    /// Den tidigare regionen får tona ut och den senare tona in, båda lika långa som
    /// överlappet — dock högst halva respektive region, för då finns det ingen region kvar
    /// att tona på. Motsvarar FL:s `Automatic crossfades` / `Create crossfade`.
    /// Returnerar antalet korsningar som skapades.
    pub fn create_crossfades(&mut self) -> usize {
        let mut order: Vec<usize> = (0..self.regions.len()).collect();
        order.sort_by(|a, b| {
            self.regions[*a]
                .start_bar
                .partial_cmp(&self.regions[*b].start_bar)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut made = 0;
        for pair in order.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let end_a = self.regions[a].start_bar + self.regions[a].length_bars;
            let start_b = self.regions[b].start_bar;
            let overlap = end_a - start_b;
            if overlap <= 0.001 {
                continue;
            }
            let fade = overlap
                .min(self.regions[a].length_bars * 0.5)
                .min(self.regions[b].length_bars * 0.5);
            if fade <= 0.001 {
                continue;
            }
            self.regions[a].fade_out_bars = fade;
            self.regions[b].fade_in_bars = fade;
            made += 1;
        }
        made
    }

    pub fn new(name: String, icon: &'static str, kind: TrackKind, color: Color32) -> Self {
        Self {
            name,
            icon,
            kind,
            color,
            volume: 0.85,
            pan: 0.0,
            muted: false,
            solo: false,
            is_rec_armed: false,
            regions: Vec::new(),
            clips: [None; 32],
            clip_bars: [1; 32],
            waveform_cache: None,
            waveform_cache_pending: None,
            custom_clip_name: None,
            group: None,
            pcm_audio: None,
            frozen: None,
            frozen_pcm: None,
            eq: TrackEq::default(),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            cue_a: 0.0,
            cue_b: 0.0,
            pitch_semitones: 0.0,
            automation: Vec::new(),
            automation_last: [f32::NAN; AutomationParam::COUNT],
            bus: default_bus_for_kind(kind),
            vca: None,
            sidechain_from: None,
            sidechain_amount_db: 0.0,
            sidechain_threshold_db: -30.0,
            sends: Vec::new(),
            notes: String::new(),
        }
    }
}


#[cfg(test)]
mod crossfade_tests {
    use super::*;

    fn region(id: usize, start: f32, len: f32) -> AudioRegion {
        AudioRegion {
            id,
            name: format!("r{id}"),
            start_bar: start,
            length_bars: len,
            sample_offset_sec: 0.0,
            source_path: None,
            waveform_peaks: Vec::new(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: default_region_color(),
            loop_length_bars: 0.0,
            source_bpm: 0.0,
            tape: false,
            warp_mode: Default::default(),
        }
    }

    fn track_with(regions: Vec<AudioRegion>) -> PlaylistTrack {
        let mut t = PlaylistTrack::new(
            "test".to_string(),
            "🎵",
            TrackKind::CustomAudio,
            Theme::BG_DEEP,
        );
        t.regions = regions;
        t
    }

    #[test]
    fn a_crossfade_is_as_long_as_the_overlap() {
        // 0–4 och 3–7 överlappar en takt: den tidigare tonar ut, den senare in.
        let mut t = track_with(vec![region(1, 0.0, 4.0), region(2, 3.0, 4.0)]);
        assert_eq!(t.create_crossfades(), 1);
        assert_eq!(t.regions[0].fade_out_bars, 1.0);
        assert_eq!(t.regions[1].fade_in_bars, 1.0);
        // Kanterna utåt rörs inte: korsningen är mellan regionerna, inte en fade av allt.
        assert_eq!(t.regions[0].fade_in_bars, 0.0);
        assert_eq!(t.regions[1].fade_out_bars, 0.0);
    }

    #[test]
    fn a_huge_overlap_stops_at_half_of_each_region() {
        // 0–2 och 0,5–10: överlappet är 1,5 takter men halva den korta regionen är 1,0.
        let mut t = track_with(vec![region(1, 0.0, 2.0), region(2, 0.5, 9.5)]);
        assert_eq!(t.create_crossfades(), 1);
        assert_eq!(t.regions[0].fade_out_bars, 1.0);
        assert_eq!(t.regions[1].fade_in_bars, 1.0);
    }

    #[test]
    fn regions_that_do_not_touch_get_no_crossfade() {
        let mut t = track_with(vec![region(1, 0.0, 4.0), region(2, 4.0, 4.0)]);
        assert_eq!(t.create_crossfades(), 0);
        assert_eq!(t.regions[0].fade_out_bars, 0.0);
        assert_eq!(t.regions[1].fade_in_bars, 0.0);
    }
}

#[cfg(test)]
mod time_ops_tests {
    use super::*;

    fn region(id: usize, start: f32, len: f32, offset: f32) -> AudioRegion {
        AudioRegion {
            id,
            name: format!("r{id}"),
            start_bar: start,
            length_bars: len,
            sample_offset_sec: offset,
            source_path: None,
            waveform_peaks: Vec::new(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: default_region_color(),
            loop_length_bars: 0.0,
            source_bpm: 0.0,
            tape: false,
            warp_mode: Default::default(),
        }
    }

    fn track(regions: Vec<AudioRegion>) -> PlaylistTrack {
        let mut t = PlaylistTrack::new(
            "t".to_string(),
            "🎵",
            TrackKind::CustomAudio,
            Theme::BG_DEEP,
        );
        t.regions = regions;
        t
    }

    #[test]
    fn inserting_time_pushes_later_regions_and_stretches_the_one_it_cuts() {
        // 0–4 (spänner över takt 2), 6–8. Infoga 2 takter vid takt 2.
        let mut t = track(vec![region(1, 0.0, 4.0, 0.0), region(2, 6.0, 2.0, 1.5)]);
        assert_eq!(t.insert_time(2.0, 2.0), 2);
        assert_eq!(t.regions[0].length_bars, 6.0, "regionen som klipps ska förlängas");
        assert_eq!(t.regions[0].start_bar, 0.0);
        assert_eq!(t.regions[1].start_bar, 8.0, "den senare ska flyttas två takter");
        assert_eq!(t.regions[1].sample_offset_sec, 1.5, "offsetten rörs inte");
    }

    #[test]
    fn deleting_time_removes_what_is_inside_and_closes_the_gap() {
        // 0–1 före, 2–3 inne, 4–8 efter, 1–6 spänner över allt.
        let mut t = track(vec![
            region(1, 0.0, 1.0, 0.0),
            region(2, 2.0, 1.0, 0.0),
            region(3, 4.0, 4.0, 0.0),
            region(4, 1.0, 5.0, 2.0),
        ]);
        let (removed, changed) = t.delete_time(2.0, 3.0);
        assert_eq!(removed, 1, "2–3 ligger helt inne och ska bort");
        assert_eq!(changed, 2, "4–8 flyttas ihop och 1–6 kortas");
        assert!(t.regions.iter().any(|r| r.start_bar == 0.0), "det som låg före rörs inte");
        // 4–8 blir 3–7: en takt bort, resten ihop.
        let after = t.regions.iter().find(|r| r.id == 3).unwrap();
        assert_eq!(after.start_bar, 3.0);
        assert_eq!(after.length_bars, 4.0);
        // 1–6 spänner över hela intervallet: starten står kvar, längden kortas.
        let spanning = t.regions.iter().find(|r| r.id == 4).unwrap();
        assert_eq!(spanning.start_bar, 1.0);
        assert_eq!(spanning.length_bars, 4.0);
        assert_eq!(spanning.sample_offset_sec, 2.0, "offsetten rörs inte när starten står kvar");
    }

    #[test]
    fn deleting_the_start_of_a_region_trims_it_and_moves_the_source() {
        // 2,5–6 med borttagning 2–3: regionen börjar inne i intervallet, alltså ska den
        // flyttas till intervallets början och källan hoppa fram lika mycket.
        let mut t = track(vec![region(1, 2.5, 3.5, 0.0)]);
        let (_, changed) = t.delete_time(2.0, 3.0);
        assert_eq!(changed, 1);
        let r = &t.regions[0];
        assert_eq!(r.start_bar, 2.0);
        // Kvar av 2,5–6 är 3–6 = tre takter, lagda där intervallet låg.
        assert_eq!(r.length_bars, 3.0);
        assert_eq!(r.sample_offset_sec, 0.5, "ljudet ska börja en halv takt in i källan");
    }

    #[test]
    fn duplicating_time_copies_the_inside_and_pushes_the_rest() {
        // 0–2 (inne i 0–4), 4–6 (efter). Duplicera 0–4 → kopian hamnar 4–6, originalet 4–6 flyttas till 8–10.
        let mut t = track(vec![region(1, 0.0, 2.0, 0.0), region(2, 4.0, 2.0, 0.0)]);
        assert_eq!(t.duplicate_time(0.0, 4.0), 1);
        assert_eq!(t.regions.len(), 3);
        assert_eq!(t.regions[0].start_bar, 0.0, "originalet ligger kvar");
        let copy = t.regions.iter().find(|r| r.start_bar == 4.0).unwrap();
        assert_eq!(copy.length_bars, 2.0);
        assert_ne!(copy.id, 1, "kopian ska ha eget id");
        let moved = t.regions.iter().find(|r| r.id == 2).unwrap();
        assert_eq!(moved.start_bar, 8.0, "det som låg efter flyttas fram");
    }
}

#[cfg(test)]
mod chain_tests {
    use super::next_chain_bar;

    /// Vers 0–8, refräng 8–16, brygga 16–24.
    fn song() -> Vec<(usize, usize)> {
        vec![(0, 8), (8, 8), (16, 8)]
    }

    #[test]
    fn the_chain_decides_what_comes_after_a_section() {
        // Vers → Refräng → Vers → Refräng: en riktig kedja, inte bara framåt.
        let chain = vec![0, 1, 0, 1];
        assert_eq!(next_chain_bar(7, &song(), &chain), Some(8), "versen leder till refrängen");
        assert_eq!(next_chain_bar(15, &song(), &chain), Some(0), "refrängen leder tillbaka till versen");
        assert_eq!(next_chain_bar(3, &song(), &chain), None, "mitt i en sektion händer inget");
    }

    #[test]
    fn a_section_outside_the_chain_plays_on_linearly_and_the_last_one_wraps() {
        let chain = vec![0, 1];
        assert_eq!(next_chain_bar(23, &song(), &chain), None, "bryggan är inte med i kedjan");
        // Bara refrängen i kedjan: den loopar sig själv.
        assert_eq!(next_chain_bar(15, &song(), &[1]), Some(8));
        // Tom kedja stänger av helt.
        assert_eq!(next_chain_bar(7, &song(), &[]), None);
    }
}

#[cfg(test)]
mod chord_tests {
    use super::*;

    fn c_major() -> ChordEvent {
        ChordEvent { bar: 0, root: 0, quality: ChordQuality::Major }
    }

    #[test]
    fn snapping_moves_the_smallest_distance_and_keeps_the_shape() {
        // C-dur: C E G. F (5) ligger en halvton under E, H (11) en halvton över C.
        assert_eq!(snap_to_chord(60, &c_major()), 60, "ackordston står still");
        assert_eq!(snap_to_chord(64, &c_major()), 64, "E är med");
        assert_eq!(snap_to_chord(65, &c_major()), 64, "F snappar ned till E");
        assert_eq!(snap_to_chord(71, &c_major()), 72, "H snappar upp till C");
        // Ett hopp på en liten bit, aldrig ett fast intervall.
        assert!((snap_to_chord(66, &c_major()) as i32 - 66).abs() <= 2);
    }

    #[test]
    fn ties_go_down_and_notes_outside_midi_range_are_left_alone() {
        // Db (61) ligger en halvton från både C (60) och D som inte är med — bara C är nära.
        assert_eq!(snap_to_chord(61, &c_major()), 60);
        // F# (66) ligger en halvton under G: uppåt är närmast.
        assert_eq!(snap_to_chord(66, &c_major()), 67);
        // D (62) ligger två från både C och E — lika nära, och då väljs den nedre.
        assert_eq!(snap_to_chord(62, &c_major()), 60);
        assert_eq!(snap_to_chord(0, &c_major()), 0, "C i botten står still");
    }

    #[test]
    fn a_chord_lasts_until_the_next_one_starts() {
        let chords = vec![
            ChordEvent { bar: 0, root: 0, quality: ChordQuality::Major },
            ChordEvent { bar: 4, root: 9, quality: ChordQuality::Minor },
        ];
        assert_eq!(chord_at(&chords, 0).unwrap().name(), "C");
        assert_eq!(chord_at(&chords, 3).unwrap().name(), "C", "gäller till takt 4");
        assert_eq!(chord_at(&chords, 4).unwrap().name(), "Am");
        assert_eq!(chord_at(&chords, 40).unwrap().name(), "Am", "sista gäller vidare");
        assert!(chord_at(&[], 0).is_none());
    }

    #[test]
    fn a_minor_seventh_has_four_tones() {
        let c = ChordEvent { bar: 0, root: 2, quality: ChordQuality::Min7 };
        assert_eq!(c.pitch_classes(), vec![2, 5, 9, 0]);
        assert_eq!(c.name(), "Dm7");
    }
}

#[cfg(test)]
mod bar_advance_tests {
    use super::{BarAdvance, after_bar_end};

    #[test]
    fn without_the_loop_the_song_plays_past_the_loop_region() {
        // Slingan av: loop-slutet (18) får inte stoppa något — bara slutet på låten (28).
        assert_eq!(after_bar_end(17, false, 0, 18, 28), BarAdvance::Continue);
        assert_eq!(after_bar_end(18, false, 0, 18, 28), BarAdvance::Continue);
        assert_eq!(after_bar_end(26, false, 0, 18, 28), BarAdvance::Continue);
        assert_eq!(after_bar_end(27, false, 0, 18, 28), BarAdvance::Stop);
    }

    #[test]
    fn with_the_loop_on_it_wraps_at_the_loop_end() {
        assert_eq!(after_bar_end(16, true, 4, 18, 28), BarAdvance::Continue);
        assert_eq!(after_bar_end(17, true, 4, 18, 28), BarAdvance::Wrap(4));
    }

    #[test]
    fn a_backwards_or_collapsed_loop_becomes_one_bar_at_its_start() {
        // loop_end <= loop_start (trasig region): slingan blir **en takt** från start, i
        // stället för en radda hopp till samma takt — och den slår inte till innan vi är
        // framme vid starttakten.
        assert_eq!(after_bar_end(9, true, 9, 9, 28), BarAdvance::Wrap(9));
        assert_eq!(after_bar_end(11, true, 12, 8, 28), BarAdvance::Continue);
        assert_eq!(after_bar_end(12, true, 12, 8, 28), BarAdvance::Wrap(12));
    }

    #[test]
    fn the_loop_wins_over_the_song_end() {
        // Slingan slutar före sista takten: slingan ska gälla, inte slutet.
        assert_eq!(after_bar_end(15, true, 0, 16, 16), BarAdvance::Wrap(0));
    }
}

#[cfg(test)]
mod track_group_tests {
    use super::*;

    fn track(group: Option<usize>) -> PlaylistTrack {
        let mut t = PlaylistTrack::new("Spår".to_string(), "🎚", TrackKind::CustomAudio, Color32::GRAY);
        t.group = group;
        t
    }

    fn group(collapsed: bool) -> TrackGroup {
        TrackGroup { name: "G".to_string(), collapsed }
    }

    /// **En ihopfälld grupp visar sitt första spår och gömmer resten** — och det första syns
    /// även när gruppen är ihopfälld, annars gick den aldrig att fälla upp igen.
    #[test]
    fn a_collapsed_group_hides_all_but_its_first_track() {
        let tracks = vec![track(Some(0)), track(Some(0)), track(None), track(Some(0))];
        let open = vec![group(false)];
        let closed = vec![group(true)];

        for i in 0..tracks.len() {
            assert!(!track_hidden(&tracks, &open, i), "öppen grupp gömmer inget ({i})");
        }
        assert!(!track_hidden(&tracks, &closed, 0), "första spåret ska synas");
        assert!(track_hidden(&tracks, &closed, 1), "andra ska gömmas");
        assert!(!track_hidden(&tracks, &closed, 2), "spår utan grupp ska synas");
        assert!(track_hidden(&tracks, &closed, 3), "tredje medlemmen ska gömmas");

        // Ett gruppindex som inte finns får inte gömma något.
        let broken = vec![track(Some(7))];
        assert!(!track_hidden(&broken, &closed, 0));
    }
}
