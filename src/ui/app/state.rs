//! Tillstånd och typer för app-ytan — utbrutet ur `app.rs` 2026-09-14.
//!
//! Det här är datamodellen: vad ett spår, ett klipp, ett mönster, en kanal och en
//! automatiseringskurva ÄR, plus deras standardvärden. Ingen rendering och ingen
//! inläsning — de delarna bor i sina egna moduler.
//!
//! Status: byggs — datamodellen; nya fält hör hit och ska ha ett ärligt standardvärde.
//! Rör inte: alla fält är `pub` (syskonmodulerna läser dem); döp inte om ett fält som står i en projektfil.

use super::*;

pub(crate) fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
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
    PluginManager,
    EffectsMixer,
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
    pub steps: [bool; 16],
    pub notes: [u8; 16],
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

#[derive(Clone, Debug)]
pub struct Pattern {
    pub name: String,
    pub color: Color32,
    pub channel_steps: Vec<[bool; 16]>,
    pub channel_notes: Vec<[u8; 16]>,
    pub piano_roll_grid: [[bool; 16]; 24],
    /// Den inspelade tagningen med sin faktiska tajming (Fas 6.4). Rutnätet
    /// säger *vilka* steg som spelar, tagningen säger *när* inom steget — det är
    /// den som gör kvantisering och humanisering möjliga i efterhand.
    pub take: crate::midi_take::Take,
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
}

pub(crate) fn default_region_color() -> Color32 {
    Color32::from_rgb(100, 180, 255)
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
    pub clips: [Option<usize>; 32],  // Pattern triggers
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
    pub pitch_semitones: f32,
    /// Per-parameter automation curves for this track (Fas 5.4).
    pub automation: Vec<AutomationLane>,
    /// Last automation value sent to the engine per `AutomationParam` index
    /// (NaN = never sent). Prevents command spam while playing.
    pub automation_last: [f32; AutomationParam::COUNT],
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
            waveform_cache: None,
            waveform_cache_pending: None,
            custom_clip_name: None,
            pcm_audio: None,
            frozen: None,
            frozen_pcm: None,
            eq: TrackEq::default(),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb_send: 0.0,
            delay_send: 0.0,
            pitch_semitones: 0.0,
            automation: Vec::new(),
            automation_last: [f32::NAN; AutomationParam::COUNT],
            bus: default_bus_for_kind(kind),
            vca: None,
            sidechain_from: None,
            sidechain_amount_db: 0.0,
            sidechain_threshold_db: -30.0,
            sends: Vec::new(),
        }
    }
}

