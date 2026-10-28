//! Tillstånd och typer för app-ytan — utbrutet ur `app.rs` 2026-09-14.
//!
//! Det här är datamodellen: vad ett spår, ett klipp, ett mönster, en kanal och en
//! automatiseringskurva ÄR, plus deras standardvärden. Ingen rendering och ingen
//! inläsning — de delarna bor i sina egna moduler.
//!
//! Status: byggs — datamodellen; nya fält hör hit och ska ha ett ärligt standardvärde.
//! Rör inte: alla fält är `pub` (syskonmodulerna läser dem); döp inte om ett fält som står i en projektfil.

use super::*;

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
    /// Övriga markerade regioner (spår, index, starttakt) vid dragningens början.
    /// De följer den dragna regionen med samma förskjutning.
    pub multi_initial: Vec<(usize, usize, f32)>,
    /// Spåret pekaren står över under dragningen — dit flyttas regionerna vid släpp.
    pub target_track: Option<usize>,
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

/// En namngiven del av arrangemanget (intro, vers, refräng …). Starttakt är nollbaserad, som
/// `clips`-indexet; `bars` är antalet takter sektionen sträcker sig.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub name: String,
    pub start_bar: usize,
    pub bars: usize,
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
    /// **Släppljud** (FL Keys: dämparens ljud): sökväg för projektfilen och det färdiglästa
    /// ljudet för motorn. `None` = inget släppljud, och då är röstens beteende som förut.
    pub release_path: Option<String>,
    pub release_pcm: Option<(std::sync::Arc<Vec<f32>>, u32)>,
    pub sample_path: Option<String>,
    pub pcm_audio: Option<crate::audio::StereoPcm>,
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
/// Bends som bara gäller en ton: `(key, semitones, delay_samples)`. Samma tidsregel som
/// `bends_for_step`, men nycklad på notnummer i stället för kanal.
pub fn note_bends_for_step(
    bends: &[NoteBendPoint],
    step: usize,
    step_samples: u32,
) -> Vec<(u8, f32, u32)> {
    let mut out = Vec::new();
    for b in bends {
        if !b.pos.is_finite() || b.pos < 0.0 {
            continue;
        }
        let at = b.pos.floor() as usize;
        if at == step {
            out.push((
                b.key,
                b.semitones,
                ((b.pos - at as f32) * step_samples as f32).max(0.0) as u32,
            ));
        }
    }
    out
}

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

/// **En bend som bara gäller en ton** (MPE per ton): `key` pekar ut rösten, inte kanalen.
/// En kanalbend böjer allt som spelar på kanalen; den här böjer bara den noten, även om
/// två noter delar kanal.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NoteBendPoint {
    pub pos: f32,
    pub key: u8,
    pub semitones: f32,
}

/// **En punkt i pedalkurvan** (CC64): `pos` är i steg i patternet, `down` är pedalläget.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PedalPoint {
    pub pos: f32,
    pub down: bool,
}

/// Pedalläget som ska gälla i det här steget, om någon punkt ligger där.
pub fn pedal_at_step(points: &[PedalPoint], step: usize) -> Option<bool> {
    points
        .iter()
        .rev()
        .find(|p| p.pos.is_finite() && p.pos >= 0.0 && p.pos.floor() as usize == step)
        .map(|p| p.down)
}

/// Lägger en pedalpunkt. En ny punkt på samma steg **ersätter** den gamla, så ett trampande
/// inte fyller kurvan med hundra punkter per steg.
pub fn push_pedal(points: &mut Vec<PedalPoint>, point: PedalPoint) {
    let at = point.pos.floor();
    points.retain(|p| p.pos.floor() != at);
    points.push(point);
    points.sort_by(|a, b| a.pos.total_cmp(&b.pos));
}

/// **Ett komp** (Ableton): från `from_step` till nästa komp spelas lane `take`
/// (0 = lane 1). Täckningen är implicit — listan sorteras och varje komp gäller fram till
/// nästa komps början, så att sätta ett komp mitt i ett annat delar det utan att räkna om
/// några gränser.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompSlice {
    /// `None` = kompningen gäller varje takt, `Some(bar)` = bara den takten (0-baserad).
    /// Äldre filer har inget fält och läses som `None`, alltså precis som förut.
    #[serde(default)]
    pub bar: Option<usize>,
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

    /// Låter `take` spela från `from_step` till nästa komp. `None` = varje takt,
    /// `Some(bar)` = bara den takten.
    /// Samma sak, men bara för en takt (`Some(bar)`, 0-baserad). Ett komp på samma steg och
    /// takt ersätts; ett komp för en takt vinner över ett globalt komp på samma steg.
    pub fn comp_from_bar(&mut self, bar: Option<usize>, from_step: f32, take: usize) {
        let from_step = from_step.clamp(0.0, 15.99);
        let take = take.min(self.alt_takes.len());
        self.comp.retain(|slice| {
            let same_step = (slice.from_step - from_step).abs() <= 0.001;
            !(same_step && slice.bar == bar)
        });
        self.comp.push(CompSlice {
            bar,
            from_step,
            take,
        });
        self.comp.sort_by(|a, b| {
            a.from_step
                .total_cmp(&b.from_step)
                .then(a.bar.is_some().cmp(&b.bar.is_some()))
        });
    }

    /// Tagningen som spelas: den aktiva lane:n, eller — om patternet är kompat — noterna ur
    /// varje lane för de steg där den lane:n är vald. Ett komp som pekar utanför lanes
    /// faller tillbaka på den aktiva lane:n.
    pub fn comped_take(&self) -> crate::midi_take::Take {
        self.comped_take_at(None)
    }

    /// Vilken lane som spelar i en takt — det lane-vyn i spellistan markerar. Regeln är
    /// **lanen vid taktens första steg**: ett taktkomp vinner över ett globalt, och utan komp
    /// gäller den aktiva lane:n. Ett komp mitt i takten syns därför inte i lane-vyn (det hörs).
    pub fn lane_for_bar(&self, bar: usize) -> usize {
        let mut chosen = self.active_take;
        let mut best_specific = false;
        for slice in &self.comp {
            if slice.from_step > 0.001 {
                continue;
            }
            if slice.bar.is_some() && slice.bar != Some(bar) {
                continue;
            }
            let specific = slice.bar.is_some();
            if !best_specific || specific {
                best_specific = specific;
                chosen = slice.take;
            }
        }
        chosen.min(self.alt_takes.len())
    }

    /// Kompningen för en viss takt (`Some(bar)`) eller för alla (`None`). Ett komp som pekar
    /// ut en takt vinner över ett globalt på samma steg; noter före första kompningen följer
    /// den aktiva lane:n, som förut.
    pub fn comped_take_at(&self, bar: Option<usize>) -> crate::midi_take::Take {
        if self.comp.is_empty() {
            return self.active_take().clone();
        }
        let lanes = self.take_lanes();
        let last_lane = lanes.len().saturating_sub(1);
        let mut slices: Vec<(f32, bool, usize)> = self
            .comp
            .iter()
            .filter(|slice| slice.bar.is_none() || slice.bar == bar)
            .map(|slice| (slice.from_step, slice.bar.is_some(), slice.take))
            .collect();
        slices.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut notes = Vec::new();
        for (lane, take) in lanes.iter().enumerate() {
            for note in &take.notes {
                let chosen = slices
                    .iter()
                    .rev()
                    .find(|(from_step, _, _)| *from_step <= note.pos)
                    .map(|(_, _, take)| (*take).min(last_lane))
                    .unwrap_or(self.active_take.min(last_lane));
                if chosen == lane {
                    notes.push(*note);
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
    /// **Bend per ton** (MPE): samma kurva som `bends`, men nycklad på notnummer.
    pub note_bends: Vec<NoteBendPoint>,
    /// **Pedalkurvan** (CC64). Ligger i patternet av samma skäl som benden: det är patternet
    /// som spelas, i pattern-läget och från ett arrangemangsblock.
    pub pedals: Vec<PedalPoint>,
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrackGroup {
    pub name: String,
    pub collapsed: bool,
}

/// **Ripple-redigering** (REAPER): när något tas bort eller klistras in följer det som
/// ligger efter med, antingen på samma spår eller på alla spår.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RippleMode {
    #[default]
    Off,
    Track,
    All,
}

impl RippleMode {
    pub fn label(self) -> &'static str {
        match self {
            RippleMode::Off => "Ripple: av",
            RippleMode::Track => "Ripple: spår",
            RippleMode::All => "Ripple: alla",
        }
    }

    pub fn next(self) -> Self {
        match self {
            RippleMode::Off => RippleMode::Track,
            RippleMode::Track => RippleMode::All,
            RippleMode::All => RippleMode::Off,
        }
    }
}

/// **Flera kopierade regioner.** `items` bär spåravståndet från det första markerade
/// spåret, `anchor_bar` den tidigaste starttakt som varje `start_bar` mäts mot — så
/// klistras klungor in med sitt inbördes avstånd i både tid och spår.
#[derive(Clone, Debug, Default)]
pub struct RegionClipboard {
    pub items: Vec<(usize, AudioRegion)>,
    pub anchor_bar: f32,
    /// Spåret klungan kopierades från (minsta spårindex), så den kan klistras tillbaka
    /// på sin ursprungliga plats.
    pub origin_track: usize,
}

/// Utökar en markering med alla regioner som hör till samma objektgrupp som något i
/// markeringen. Klickar man en grupperad region markerar man hela gruppen (REAPER).
pub fn expand_with_groups(
    tracks: &[PlaylistTrack],
    groups: &std::collections::HashMap<usize, u32>,
    set: &std::collections::BTreeSet<(usize, usize)>,
) -> std::collections::BTreeSet<(usize, usize)> {
    let mut next = set.clone();
    let gids: std::collections::BTreeSet<u32> = set
        .iter()
        .filter_map(|(t, r)| tracks.get(*t).and_then(|tr| tr.regions.get(*r)))
        .filter_map(|reg| groups.get(&reg.id).copied())
        .collect();
    if gids.is_empty() {
        return next;
    }
    for (t, track) in tracks.iter().enumerate() {
        for (r, reg) in track.regions.iter().enumerate() {
            if groups.get(&reg.id).is_some_and(|g| gids.contains(g)) {
                next.insert((t, r));
            }
        }
    }
    next
}

/// Regioner i ett spår vars tid överlappar [from, to] — grunden för rammarkering.
pub fn regions_in_bar_range(track: &PlaylistTrack, from: f32, to: f32) -> Vec<usize> {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    track
        .regions
        .iter()
        .enumerate()
        .filter(|(_, r)| r.start_bar < hi && (r.start_bar + r.length_bars) > lo)
        .map(|(i, _)| i)
        .collect()
}

/// Bygger en klunga ur de markerade regionerna: spåravstånd från det första spåret och
/// den tidigaste starttakten som ankare. `None` när inget av urvalet finns kvar.
pub fn build_region_clipboard(
    tracks: &[PlaylistTrack],
    selected: &std::collections::BTreeSet<(usize, usize)>,
) -> Option<RegionClipboard> {
    let min_track = selected.iter().map(|(t, _)| *t).min()?;
    let mut items: Vec<(usize, AudioRegion)> = Vec::new();
    for (t, r) in selected.iter() {
        if let Some(reg) = tracks.get(*t).and_then(|tr| tr.regions.get(*r)) {
            items.push((*t - min_track, reg.clone()));
        }
    }
    if items.is_empty() {
        return None;
    }
    let anchor_bar = items.iter().map(|(_, r)| r.start_bar).fold(f32::INFINITY, f32::min);
    Some(RegionClipboard {
        items,
        anchor_bar,
        origin_track: min_track,
    })
}

/// Var klungan hamnar: (index i `clip.items`, spår, starttakt). Delar som skulle hamna
/// utanför projektets spår hoppas över — att skapa spår i smyg är värre än att tappa en del.
pub fn plan_region_paste(
    clip: &RegionClipboard,
    base_track: usize,
    paste_bar: f32,
    track_count: usize,
) -> Vec<(usize, usize, f32)> {
    clip.items
        .iter()
        .enumerate()
        .filter_map(|(i, (offset, reg))| {
            let target = base_track + offset;
            (target < track_count).then(|| (i, target, paste_bar + (reg.start_bar - clip.anchor_bar)))
        })
        .collect()
}

/// Markeringsläget efter ett klick på ett spår eller en region.
///
/// `toggle` (Ctrl/Cmd) växlar posten, `shift` tar spannet från den primära posten,
/// annars blir det bara den klickade. Markeringen får aldrig bli tom: då tystnar
/// kommandona utan att någon förstår varför.
pub fn clicked_selection(
    current: &std::collections::BTreeSet<usize>,
    primary: usize,
    clicked: usize,
    shift: bool,
    toggle: bool,
) -> std::collections::BTreeSet<usize> {
    if toggle {
        let mut next = current.clone();
        if !next.insert(clicked) {
            next.remove(&clicked);
        }
        if next.is_empty() {
            next.insert(clicked);
        }
        return next;
    }
    if shift {
        let (lo, hi) = if primary <= clicked {
            (primary, clicked)
        } else {
            (clicked, primary)
        };
        return (lo..=hi).collect();
    }
    std::iter::once(clicked).collect()
}

/// **Fäll ihop eller upp alla grupper** (FL: `Collapse all grouped tracks`). Ren funktion så
/// regeln går att pröva utan ett fönster; returnerar antalet grupper som faktiskt ändrades.
pub fn set_all_groups_collapsed(groups: &mut [TrackGroup], collapsed: bool) -> usize {
    let mut changed = 0;
    for g in groups.iter_mut() {
        if g.collapsed != collapsed {
            g.collapsed = collapsed;
            changed += 1;
        }
    }
    changed
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

impl AudioRegion {
    /// **Delar klippet vid `at_bars`** (relativt klippets början) och returnerar den högra
    /// delen. Vänster del behåller id:t; anroparen sätter nya id:n på delarna.
    ///
    /// `secs_per_bar` behövs för `sample_offset_sec`: positionerna räknas i takter men
    /// ljudet i sekunder, så en delning måste flytta läsningen lika långt.
    pub fn split_at(&mut self, at_bars: f32, secs_per_bar: f32) -> Option<AudioRegion> {
        if at_bars <= 1e-4 || at_bars >= self.length_bars - 1e-4 {
            return None;
        }
        let mut right = self.clone();
        right.start_bar = self.start_bar + at_bars;
        right.length_bars = self.length_bars - at_bars;
        right.sample_offset_sec = self.sample_offset_sec + at_bars * secs_per_bar.max(0.0);
        // Faderna hör till var sin ände: vänster behåller in, höger behåller ut.
        right.fade_in_bars = 0.0;
        right.fade_in_bars = right.fade_in_bars.min(right.length_bars);
        right.fade_out_bars = right.fade_out_bars.min(right.length_bars);

        self.length_bars = at_bars;
        self.fade_out_bars = 0.0;
        self.fade_in_bars = self.fade_in_bars.min(self.length_bars);
        Some(right)
    }
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
    crate::ui::theme::palette().track_blue()
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

/// **En klippa i arrangemanget** (AI-klipporna): ett objekt på tidslinjen med egna noter.
///
/// Skillnaden mot ett patternblock (`clips`/`clip_bars`) är att klippan bär sin egen notlista:
/// att flytta den rör bara `start_bar`, och att byta kanalens ljud rör inte noterna. Ett
/// patternblock pekar på rutnätet, alltså på något som delas med varje annat block.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PatternClip {
    pub id: usize,
    /// Mönstret klippan hör till. En klippa **utan** noter spelar dess rutnät på `lane`.
    pub pattern: usize,
    /// Kanalen i racket som klippan spelar — det är den som bestämmer ljudet.
    pub lane: usize,
    pub start_bar: f32,
    pub length_bars: f32,
    pub name: String,
    /// Noterna, med `pos` i steg **från klippans start** (0–64) — inte från taktens början som
    /// i en inspelad tagning, för en klippa kan vara flera takter.
    #[serde(default)]
    pub notes: Vec<crate::midi_take::TakeNote>,
}

impl PatternClip {
    /// Steget inuti klippan för en songtakt och ett steg; `None` utanför klippan.
    ///
    /// Räknat i absoluta sextondelar, så en klippa som börjar mitt i en takt spelar från
    /// taktens andra hälft i stället för att runda ner till taktens början.
    pub fn step_at(&self, song_bar: usize, step: usize) -> Option<usize> {
        const SPB: f32 = crate::audio::tempo::STEPS_PER_BAR as f32;
        let rel = song_bar as f32 * SPB + step as f32 - self.start_bar * SPB;
        let len = (self.length_bars * SPB).round().max(1.0);
        (rel >= -0.001 && rel < len).then(|| rel.max(0.0) as usize)
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
    /// **Klipporna** (AI-klipporna): egna objekt på tidslinjen med egna noter. Start och längd
    /// är `f32` takter som en ljudregion, så de går att dra och längdändra fritt.
    pub pattern_clips: Vec<PatternClip>,
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
    pub pcm_audio: Option<crate::audio::StereoPcm>,
    /// Fruset spår (Tier 2): färdigrenderat ljud + fingeravtrycket av källan.
    pub frozen: Option<FrozenTrack>,
    /// Det frusna ljudet inläst, så att en omladdning (ClearAllStemTracks +
    /// resync, som vid projektinläsning) kan skicka det igen.
    pub frozen_pcm: Option<crate::audio::StereoPcm>,
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
        let next_id = self.regions.iter().map(|r| r.id).max().unwrap_or(0) + 1;
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
        for (offset, c) in copies.iter_mut().enumerate() {
            c.start_bar += span;
            c.id = next_id + offset;
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
        let mut made = 0;
        for (a, b) in self.overlapping_pairs() {
            if self.crossfade_pair(a, b) {
                made += 1;
            }
        }
        made
    }

    /// **Korsning vid spelhuvudet** (FL: `Create crossfade`): den skarv som ligger närmast
    /// `bar` får korsningen — inte hela spåret. Returnerar 1 när en korsning skapades.
    pub fn create_crossfade_at(&mut self, bar: f32) -> usize {
        let mut best: Option<(usize, usize, f32)> = None;
        for (a, b) in self.overlapping_pairs() {
            let distance = (self.regions[b].start_bar - bar).abs();
            if best.is_none_or(|(_, _, d)| distance < d) {
                best = Some((a, b, distance));
            }
        }
        match best {
            Some((a, b, _)) if self.crossfade_pair(a, b) => 1,
            _ => 0,
        }
    }

    /// Regionparen som överlappar, i tidsordning — den enda definitionen av "en skarv".
    fn overlapping_pairs(&self) -> Vec<(usize, usize)> {
        let mut order: Vec<usize> = (0..self.regions.len()).collect();
        order.sort_by(|a, b| {
            self.regions[*a]
                .start_bar
                .partial_cmp(&self.regions[*b].start_bar)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order
            .windows(2)
            .filter(|pair| {
                let end_a = self.regions[pair[0]].start_bar + self.regions[pair[0]].length_bars;
                end_a - self.regions[pair[1]].start_bar > 0.001
            })
            .map(|pair| (pair[0], pair[1]))
            .collect()
    }

    /// Lägger in- och ut-toning över överlappet, högst halva respektive region.
    fn crossfade_pair(&mut self, a: usize, b: usize) -> bool {
        let end_a = self.regions[a].start_bar + self.regions[a].length_bars;
        let overlap = end_a - self.regions[b].start_bar;
        let fade = overlap
            .min(self.regions[a].length_bars * 0.5)
            .min(self.regions[b].length_bars * 0.5);
        if fade <= 0.001 {
            return false;
        }
        self.regions[a].fade_out_bars = fade;
        self.regions[b].fade_in_bars = fade;
        true
    }

    /// **Delar ett klipp på en taktgräns** och lägger den högra delen efter.
    /// `chunk_bars` är delens längd; sista delen blir resten. Returnerar antalet delar.
    pub fn chop_region(&mut self, index: usize, chunk_bars: f32, secs_per_bar: f32) -> usize {
        if self.regions.get(index).is_none() || chunk_bars <= 0.001 {
            return 0;
        }
        let mut parts = 0;
        // Varje varv delar den **sista** biten, inte den första — annars stannar loopen efter
        // en del, eftersom den första redan är kort nog.
        while self.regions[index + parts].length_bars > chunk_bars + 1e-3 {
            let Some(right) = self.regions[index + parts].split_at(chunk_bars, secs_per_bar)
            else {
                break;
            };
            self.regions.insert(index + 1 + parts, right);
            parts += 1;
        }
        let len = self.regions.len();
        self.reassign_region_ids(index..len);
        parts + 1
    }

    /// **Auto-slice:** delar klippet vid givna punkter (i takter, relativa klippets start).
    /// Punkter utanför klippet ignoreras, och dubbletter slås ihop.
    pub fn slice_region_at(
        &mut self,
        index: usize,
        points_bars: &[f32],
        secs_per_bar: f32,
    ) -> usize {
        let Some(region) = self.regions.get(index).cloned() else {
            return 0;
        };
        let mut cuts: Vec<f32> = points_bars
            .iter()
            .copied()
            .filter(|p| *p > 1e-3 && *p < region.length_bars - 1e-3)
            .collect();
        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-3);

        let mut made = 0;
        for cut in cuts {
            // Varje snitt ligger i den **sista** delen, så punkterna behöver räknas om
            // mot hur mycket som redan klippts bort.
            let consumed: f32 = self.regions[index..index + made]
                .iter()
                .map(|r| r.length_bars)
                .sum();
            let at = cut - consumed;
            if at <= 1e-3 {
                continue;
            }
            let Some(right) = self.regions[index + made].split_at(at, secs_per_bar) else {
                continue;
            };
            self.regions.insert(index + 1 + made, right);
            made += 1;
        }
        let len = self.regions.len();
        self.reassign_region_ids(index..len);
        made + 1
    }

    /// Skalar nivån på ett klipp. Faktorn kläms så att volymen håller sig inom 0–2.
    pub fn scale_region_volume(&mut self, index: usize, factor: f32) -> bool {
        let Some(region) = self.regions.get_mut(index) else {
            return false;
        };
        region.volume = (region.volume * factor).clamp(0.0, 2.0);
        true
    }

    fn reassign_region_ids(&mut self, range: std::ops::Range<usize>) {
        let mut next = self
            .regions
            .iter()
            .enumerate()
            .filter(|(i, _)| !range.contains(i))
            .map(|(_, r)| r.id)
            .max()
            .unwrap_or(0)
            + 1;
        for i in range {
            if let Some(region) = self.regions.get_mut(i) {
                region.id = next;
                next += 1;
            }
        }
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
            pattern_clips: Vec::new(),
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
mod region_edit_tests {
    use super::*;

    fn track_with_region(len_bars: f32, offset_sec: f32) -> PlaylistTrack {
        let mut t = PlaylistTrack::new(
            "Test".into(),
            "🎤",
            TrackKind::VocalAudio,
            Color32::WHITE,
        );
        t.regions.push(AudioRegion {
            id: 7,
            name: "Loop".into(),
            start_bar: 4.0,
            length_bars: len_bars,
            sample_offset_sec: offset_sec,
            source_path: None,
            waveform_peaks: Vec::new(),
            volume: 0.8,
            fade_in_bars: 0.1,
            fade_out_bars: 0.1,
            muted: false,
            is_reverse: false,
            color: default_region_color(),
            loop_length_bars: 0.0,
            source_bpm: 0.0,
            tape: false,
            warp_mode: Default::default(),
        });
        t
    }

    /// **Chop in bars:** fyra takter i ettor ger fyra delar som ligger i följd, och varje
    /// del läser ljudet en takt längre fram.
    #[test]
    fn chopping_in_bars_keeps_the_timeline_contiguous() {
        let mut t = track_with_region(4.0, 0.0);
        let parts = t.chop_region(0, 1.0, 2.0);
        assert_eq!(parts, 4);
        assert_eq!(t.regions.len(), 4);
        for (i, r) in t.regions.iter().enumerate() {
            assert!((r.start_bar - (4.0 + i as f32)).abs() < 1e-4, "del {i} börjar fel");
            assert!((r.length_bars - 1.0).abs() < 1e-4);
            assert!((r.sample_offset_sec - i as f32 * 2.0).abs() < 1e-3, "fel ljudoffset");
        }
        let mut ids: Vec<usize> = t.regions.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4, "alla delar ska ha eget id");
    }

    /// **Chop i 8-delar:** en takt i fjärdedelar ger fyra delar.
    #[test]
    fn chopping_in_eighths_uses_the_chunk_size() {
        let mut t = track_with_region(1.0, 0.0);
        assert_eq!(t.chop_region(0, 0.25, 2.0), 4);
        assert!((t.regions[3].start_bar - 4.75).abs() < 1e-4);
    }

    /// **Auto-slice:** snitten hamnar där punkterna säger, och en punkt utanför ignoreras.
    #[test]
    fn slicing_at_onsets_splits_where_the_points_are() {
        let mut t = track_with_region(4.0, 1.0);
        let parts = t.slice_region_at(0, &[0.5, 2.0, 9.0, 1.0], 2.0);
        assert_eq!(parts, 4);
        let lens: Vec<f32> = t.regions.iter().map(|r| r.length_bars).collect();
        assert!((lens[0] - 0.5).abs() < 1e-4, "{lens:?}");
        assert!((lens[1] - 0.5).abs() < 1e-4, "{lens:?}");
        assert!((lens[2] - 1.0).abs() < 1e-4, "{lens:?}");
        assert!((lens[3] - 2.0).abs() < 1e-4, "{lens:?}");
        assert!((t.regions[3].sample_offset_sec - (1.0 + 2.0 * 2.0)).abs() < 1e-3);
    }

    /// **Scale levels:** faktorn gäller bara valt klipp och volymen kläms till 0–2.
    #[test]
    fn scaling_levels_multiplies_and_clamps() {
        let mut t = track_with_region(2.0, 0.0);
        assert!(t.scale_region_volume(0, 0.5));
        assert!((t.regions[0].volume - 0.4).abs() < 1e-5);
        assert!(t.scale_region_volume(0, 100.0));
        assert!((t.regions[0].volume - 2.0).abs() < 1e-5);
        assert!(!t.scale_region_volume(9, 0.5));
    }

    /// Ett snitt utanför klippet eller på kanten gör ingenting.
    #[test]
    fn edges_do_not_produce_empty_parts() {
        let mut t = track_with_region(2.0, 0.0);
        assert!(t.regions[0].split_at(0.0, 2.0).is_none());
        assert!(t.regions[0].split_at(2.0, 2.0).is_none());
        assert_eq!(t.slice_region_at(0, &[0.0, 2.0], 2.0), 1);
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
            crate::ui::theme::palette().bg_deep(),
        );
        t.regions = regions;
        t
    }

    /// Ramarkeringens urval: bara regioner som faktiskt överlappar intervallet, och
    /// omvänd riktning (man drar åt vänster) ska ge samma svar.
    #[test]
    fn a_bar_range_finds_the_regions_it_touches() {
        let t = track_with(vec![
            region(1, 0.0, 2.0),
            region(2, 4.0, 2.0),
            region(3, 8.0, 2.0),
        ]);
        assert_eq!(regions_in_bar_range(&t, 1.5, 4.5), vec![0, 1]);
        assert_eq!(regions_in_bar_range(&t, 4.5, 1.5), vec![0, 1]);
        assert!(regions_in_bar_range(&t, 20.0, 21.0).is_empty());
    }

    /// Ripple-knappen cyklar av → spår → alla och tillbaka.
    #[test]
    fn the_ripple_switch_cycles() {
        assert_eq!(RippleMode::Off.next(), RippleMode::Track);
        assert_eq!(RippleMode::Track.next(), RippleMode::All);
        assert_eq!(RippleMode::All.next(), RippleMode::Off);
    }

    /// **En grupperad region drar med sig hela gruppen** — även en medlem på ett annat spår,
    /// och aldrig en region som inte är grupperad.
    #[test]
    fn selection_expands_to_whole_item_groups() {
        use std::collections::BTreeSet;
        let tracks = vec![
            track_with(vec![region(1, 0.0, 2.0), region(2, 4.0, 2.0)]),
            track_with(vec![region(3, 0.0, 2.0)]),
        ];
        let groups = std::collections::HashMap::from([(1usize, 7u32), (3usize, 7u32)]);
        let clicked: BTreeSet<(usize, usize)> = [(0, 0)].into_iter().collect();
        let expanded = expand_with_groups(&tracks, &groups, &clicked);
        assert_eq!(expanded.len(), 2);
        assert!(expanded.contains(&(1, 0)));
        assert!(!expanded.contains(&(0, 1)), "ogrupperad region ska inte följa med");
    }

    /// **Klungan behåller både spår- och taktavstånd.** Det är hela poängen med att
    /// kopiera flera regioner: den inbördes formen ska överleva klistret.
    #[test]
    fn a_cluster_keeps_track_and_time_offsets() {
        use std::collections::BTreeSet;
        let tracks = vec![
            track_with(vec![region(1, 4.0, 2.0)]),
            track_with(vec![region(2, 5.5, 1.0)]),
        ];
        let selected: BTreeSet<(usize, usize)> = [(1, 0), (0, 0)].into_iter().collect();
        let clip = build_region_clipboard(&tracks, &selected).expect("klunga");
        assert!((clip.anchor_bar - 4.0).abs() < 1e-4);

        let plan = plan_region_paste(&clip, 0, 10.0, 2);
        assert_eq!(plan.len(), 2);
        assert!(plan.iter().any(|(_, t, s)| *t == 0 && (*s - 10.0).abs() < 1e-3));
        assert!(plan.iter().any(|(_, t, s)| *t == 1 && (*s - 11.5).abs() < 1e-3));

        // Finns inte spåret hoppas delen över i stället för att ett nytt spår skapas.
        assert_eq!(plan_region_paste(&clip, 0, 10.0, 1).len(), 1);
    }

    /// **Bara skarven vid spelhuvudet.** Två skarvar, spelhuvudet vid den första: den
    /// andra ska stå orörd, annars är kommandot samma sak som "korsa allt".
    #[test]
    fn a_crossfade_at_the_playhead_touches_only_the_nearest_seam() {
        let mut t = track_with(vec![
            region(1, 0.0, 4.2),  // slutar 4,2
            region(2, 4.0, 4.2),  // skarv vid 4,0 (överlapp 0,2)
            region(3, 8.0, 4.2),  // skarv vid 8,0 (överlapp 0,2)
        ]);
        assert_eq!(t.create_crossfade_at(4.1), 1);
        assert!(t.regions[0].fade_out_bars > 0.0, "skarven vid 4,0");
        assert!(t.regions[1].fade_in_bars > 0.0);
        assert_eq!(t.regions[1].fade_out_bars, 0.0, "skarven vid 8,0 rörs inte");
        assert_eq!(t.regions[2].fade_in_bars, 0.0);
    }

    /// Utan överlapp finns ingen skarv att korsa — kommandot ska säga noll, inte gissa.
    #[test]
    fn a_crossfade_at_the_playhead_needs_an_overlap() {
        let mut t = track_with(vec![region(1, 0.0, 4.0), region(2, 4.0, 4.0)]);
        assert_eq!(t.create_crossfade_at(4.0), 0);
        assert_eq!(t.regions[0].fade_out_bars, 0.0);
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
            crate::ui::theme::palette().bg_deep(),
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
    /// **Skift tar spannet, Ctrl växlar, vanligt klick börjar om.** Reglerna är hela
    /// flödet när man markerar flera spår; därför prövas de utan fönster.
    #[test]
    fn clicks_build_a_multi_selection() {
        use std::collections::BTreeSet;
        let start: BTreeSet<usize> = [1].into_iter().collect();

        // Vanligt klick: bara den klickade.
        assert_eq!(clicked_selection(&start, 1, 4, false, false), [4].into_iter().collect::<BTreeSet<_>>());

        // Skift: spannet från primär till klickad, åt båda hållen.
        assert_eq!(clicked_selection(&start, 1, 3, true, false), [1, 2, 3].into_iter().collect::<BTreeSet<_>>());
        assert_eq!(clicked_selection(&[5].into_iter().collect(), 5, 2, true, false), [2, 3, 4, 5].into_iter().collect::<BTreeSet<_>>());

        // Ctrl växlar in och ut.
        let two = clicked_selection(&start, 1, 4, false, true);
        assert_eq!(two, [1, 4].into_iter().collect::<BTreeSet<_>>());
        assert_eq!(clicked_selection(&two, 4, 1, false, true), [4].into_iter().collect::<BTreeSet<_>>());

        // Sista posten kan inte växlas bort — markeringen får aldrig bli tom.
        assert_eq!(clicked_selection(&[2].into_iter().collect(), 2, 2, false, true), [2].into_iter().collect::<BTreeSet<_>>());
    }

    /// Alla grupper på en gång: bara de som faktiskt ändras räknas, så statusraden kan säga
    /// sanningen även när hälften redan stod rätt.
    #[test]
    fn all_groups_can_be_collapsed_and_expanded_at_once() {
        let mut groups = vec![
            TrackGroup { name: "A".to_string(), collapsed: false },
            TrackGroup { name: "B".to_string(), collapsed: true },
            TrackGroup { name: "C".to_string(), collapsed: false },
        ];
        assert_eq!(set_all_groups_collapsed(&mut groups, true), 2);
        assert!(groups.iter().all(|g| g.collapsed));
        assert_eq!(set_all_groups_collapsed(&mut groups, false), 3);
        assert!(groups.iter().all(|g| !g.collapsed));
    }

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

#[cfg(test)]
mod clip_tests {
    use super::*;

    fn clip(start_bar: f32, length_bars: f32) -> PatternClip {
        PatternClip {
            id: 3,
            pattern: 0,
            lane: 6,
            start_bar,
            length_bars,
            name: "bas".to_string(),
            notes: Vec::new(),
        }
    }

    /// Klippan spelar bara sina egna takter, och **flytten rör inte noterna** — det är hela
    /// skillnaden mot ett patternblock, som pekar på rutnätet.
    #[test]
    fn a_clip_plays_its_own_bars_and_keeps_its_notes_when_moved() {
        let mut c = clip(0.0, 2.0);
        c.notes.push(crate::midi_take::TakeNote {
            pos: 17.0,
            key: 40,
            velocity: 0.7,
            probability: 1.0,
            length_steps: 3.0,
            articulation: crate::midi_take::Articulation::Normal,
            applied: false,
        });
        let notes_before = c.notes.clone();

        assert_eq!(c.step_at(0, 0), Some(0));
        assert_eq!(c.step_at(1, 1), Some(17));
        assert_eq!(c.step_at(2, 0), None, "klippan är slut efter två takter");

        c.start_bar = 8.0;
        assert_eq!(c.step_at(8, 0), Some(0));
        assert_eq!(c.step_at(9, 1), Some(17));
        assert_eq!(c.step_at(7, 15), None, "inget före klippans start");
        assert_eq!(c.notes, notes_before, "flytten rör inte noterna");
    }

    /// En klippa som börjar mitt i en takt spelar från taktens andra hälft — inte från dess
    /// början, som en avrundning till hela takter hade gett.
    #[test]
    fn a_clip_that_starts_mid_bar_plays_from_there() {
        let c = clip(8.5, 1.0);
        assert_eq!(c.step_at(8, 7), None);
        assert_eq!(c.step_at(8, 8), Some(0));
        assert_eq!(c.step_at(9, 7), Some(15));
        assert_eq!(c.step_at(9, 8), None);
    }
}
