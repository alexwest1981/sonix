use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::{
    AudioCommand, AudioEngine, BitCrushParams, CompressorParams, DeEsserParams, DoublerParams, GateParams,
    ControlRoomParams, ConvolverParams, FreqShiftParams, GranularParams, LimiterParams, LovePhilterParams, LuxeverbParams, EmphasisParams, MasterEqBand, MasterFilterParams, MasterFxParams,
    DelayBankParams, DistructorParams, EffectorParams, HyperChorusParams, TimeCurveParams, TimeShape, HardcoreParams, ModFxParams, ShelvesParams, StereoParams, MultibandDelayParams, MultibandParams, ReverbParams, SaturatorParams, TransientParams,
    IR_BANK, synthesize_ir,
};
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob;

#[derive(Clone, Debug)]
pub struct VisualEqNode {
    pub name: &'static str,
    pub freq_hz: f32, // 20.0 .. 20000.0
    pub gain_db: f32, // -12.0 .. +12.0
    pub q: f32,       // 0.5 .. 5.0
    pub color: Color32,
    pub is_active: bool,
}

#[derive(Clone, Debug)]
pub struct FxPedal {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub enabled: bool,
    pub color: Color32,
    pub p1_name: &'static str,
    pub p1_val: f32, // 0.0 .. 1.0
    pub p2_name: &'static str,
    pub p2_val: f32,
    pub p3_name: &'static str,
    pub p3_val: f32,
    pub mix: f32,
    /// Fler rattar än de tre fasta, för effekter som behöver fem. Namn och värde 0–1,
    /// ritade efter p3 och före MIX.
    pub extra: Vec<(&'static str, f32)>,
    pub description: &'static str,
}

#[derive(Clone, Debug)]
pub struct FxRackState {
    pub pedals: Vec<FxPedal>,
    pub selected_preset: usize,
    pub active_tab: usize, // 0 = FX Rack, 1 = Visual EQ, 2 = Effects Library
    pub search_query: String,
    pub selected_category: &'static str,
    pub eq_nodes: Vec<VisualEqNode>,
    /// Control Room: monokontroll, dämpning och lyssningsnivå.
    pub control_room: ControlRoomParams,
    pub compressor_release_ms: f32,
    /// Convolverns torr/våt och nivå. Impulsresponsen själv ligger i motorn (den är för stor
    /// för att skickas varje bildruta) — här finns bara namnet, för att veta om en är laddad.
    pub convolver_mix: f32,
    pub convolver_gain_db: f32,
    pub convolver_ir_name: Option<String>,
    /// Nivå per band i masterns vocoder (16 band, 100 Hz–8 kHz), 0…2 där 1,0 är rakt igenom.
    /// Det är formantbandredigeraren — Vocodex ratt, men för hela kedjans vocoder.
    pub vocoder_bands: [f32; 16],
}

impl Default for FxRackState {
    fn default() -> Self {
        Self {
            control_room: ControlRoomParams::default(),
            pedals: vec![
                FxPedal {
                    id: "visual_eq",
                    name: "📊 VISUAL EQ",
                    category: "Clean Up",
                    enabled: true,
                    color: crate::ui::theme::palette().cyan(),
                    p1_name: "LOW",
                    p1_val: 0.50,
                    p2_name: "MID",
                    p2_val: 0.50,
                    p3_name: "HIGH",
                    p3_val: 0.55,
                    mix: 1.0,
                    description: "4-bands parametrisk equalizer med interaktiv kurva.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "dyn_comp",
                    name: "🗜 DYNAMIC COMPRESSOR",
                    category: "Glue Mix",
                    enabled: true,
                    color: crate::ui::theme::palette().orange(),
                    p1_name: "THRESH",
                    p1_val: 0.65,
                    p2_name: "RATIO",
                    p2_val: 0.40,
                    p3_name: "ATTACK",
                    p3_val: 0.30,
                    mix: 0.85,
                    description: "Professionell studiokompressor med Gain Reduction-mätare.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "vocal_doubler",
                    name: "🪐 VOCAL DOUBLER",
                    category: "Vocals",
                    enabled: true,
                    color: crate::ui::theme::palette().purple(),
                    p1_name: "STRENGTH",
                    p1_val: 0.70,
                    p2_name: "MOD",
                    p2_val: 0.35,
                    p3_name: "WIDTH",
                    p3_val: 0.80,
                    mix: 0.75,
                    description: "Skapar fylliga stereostämmor och analog körklang.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "studio_reverb",
                    name: "🌌 STUDIO REVERB",
                    category: "Echo & Delay",
                    enabled: true,
                    color: crate::ui::theme::palette().track_blue(),
                    p1_name: "DECAY",
                    p1_val: 0.55,
                    p2_name: "LOW CUT",
                    p2_val: 0.20,
                    p3_name: "HIGH CUT",
                    p3_val: 0.75,
                    mix: 0.30,
                    description: "Plate, Room och Hall-akustik för djup och rymd.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "de_esser",
                    name: "🎯 DE-ESSER",
                    category: "Clean Up",
                    enabled: true,
                    color: crate::ui::theme::palette().green(),
                    p1_name: "THRESH",
                    p1_val: 0.45,
                    p2_name: "FREQ",
                    p2_val: 0.65,
                    p3_name: "REDUCT",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Dämpar automatiskt skarpa s- och t-ljud i sånginspelningar.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "noise_gate",
                    name: "🚪 NOISE GATE",
                    category: "Clean Up",
                    enabled: true,
                    color: crate::ui::theme::palette().track_rose(),
                    p1_name: "THRESH",
                    p1_val: 0.25,
                    p2_name: "ATTACK",
                    p2_val: 0.15,
                    p3_name: "RELEASE",
                    p3_val: 0.40,
                    mix: 1.0,
                    description: "Tystar bakgrundsbrus och sus när sångaren inte sjunger.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "multiband",
                    name: "📶 MULTIBAND COMP (3 band + master)",
                    category: "Glue Mix",
                    enabled: false,
                    color: crate::ui::theme::palette().cyan(),
                    p1_name: "LOW",
                    p1_val: 0.0,
                    p2_name: "MID",
                    p2_val: 0.0,
                    p3_name: "HIGH",
                    p3_val: 0.0,
                    mix: 1.0,
                    description: "Tre band (250 Hz / 4 kHz, Linkwitz-Riley). TRÖSKEL och RATIO gäller alla band; TRÖSK L/M/H flyttar bandets egen tröskel ±20 dB, och LOW/MID/HIGH hur hårt bandet komprimeras.",
                    extra: vec![
                        ("TRÖSKEL", 0.5),
                        ("RATIO", 0.27),
                        ("TRÖSK L", 0.5),
                        ("TRÖSK M", 0.5),
                        ("TRÖSK H", 0.5),
                        ("MASTER", 0.0),
                    ],
                },
                FxPedal {
                    id: "transient",
                    name: "⚡ TRANSIENT SHAPER",
                    category: "Dynamics",
                    enabled: false,
                    color: crate::ui::theme::palette().yellow(),
                    p1_name: "ATTACK",
                    p1_val: 0.5,
                    p2_name: "SUSTAIN",
                    p2_val: 0.5,
                    p3_name: "SENS",
                    p3_val: 0.5,
                    mix: 1.0,
                    description: "Formar anslaget och kroppen var för sig — mer punch utan att höja nivån.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "saturator",
                    name: "🔥 SATURATOR",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().orange(),
                    p1_name: "DRIVE",
                    p1_val: 0.0,
                    p2_name: "TONE",
                    p2_val: 0.70,
                    p3_name: "MIX",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Mjuk mättnad (tanh) med tonkontroll — aldrig över full skala.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "granular",
                    name: "🌾 GRANULIZER",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().green(),
                    p1_name: "KORN",
                    p1_val: 0.30,
                    p2_name: "TÄTHET",
                    p2_val: 0.20,
                    p3_name: "POSITION",
                    p3_val: 0.10,
                    mix: 0.60,
                    description: "Korn ur de senaste två sekunderna: KORN (20–200 ms), TÄTHET (2–60 per sekund), POSITION (20–1000 ms bakåt), SPRIDNING (0–200 ms) och MIX.",
                    extra: vec![("SPRIDNING", 0.20), ("HASTIGHET", 0.50)],
                },
                FxPedal {
                    id: "freqshift",
                    name: "📡 FREKVENSSKIFTE",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().track_rose(),
                    p1_name: "SKIFT",
                    p1_val: 0.50,
                    p2_name: "RIKTNING",
                    p2_val: 0.50,
                    p3_name: "STEREO",
                    p3_val: 0.0,
                    mix: 0.50,
                    description: "Flyttar hela spektrumet i hertz — SKIFT ±2 000 Hz, mitten är 0. Till skillnad från tonhöjdstransponering behåller intervallerna sitt avstånd.",                    extra: Vec::new(),
                },
                FxPedal {
                    id: "bitcrush",
                    name: "🗜️ SQUEEZE",
                    category: "Distortion",
                    enabled: false,
                    color: crate::ui::theme::palette().amber(),
                    p1_name: "BITAR",
                    p1_val: 1.00,
                    p2_name: "NEDSAMPLING",
                    p2_val: 0.0,
                    p3_name: "TON",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "BITAR kvantiserar amplituden (1–16), NEDSAMPLING håller varje sampel 1–32 steg, TON är ett lågpass efter krossen (200 Hz–20 kHz).",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "mbdelay",
                    name: "📶 MULTIBAND DELAY",
                    category: "Delay",
                    enabled: false,
                    color: crate::ui::theme::palette().cyan(),
                    p1_name: "LÅG",
                    p1_val: 0.10,
                    p2_name: "MID",
                    p2_val: 0.25,
                    p3_name: "HÖG",
                    p3_val: 0.45,
                    mix: 0.35,
                    description: "Tre band, tre tider: LÅG under 250 Hz, MID 250 Hz–4 kHz, HÖG över 4 kHz. Tiderna är 20–800 ms i samma skala som ratten.",                    extra: Vec::new(),
                },
                FxPedal {
                    id: "delaybank",
                    name: "🏦 DELAY BANK",
                    category: "Delay",
                    enabled: false,
                    color: crate::ui::theme::palette().track_teal(),
                    p1_name: "TID 1",
                    p1_val: 0.08,
                    p2_name: "TID 2",
                    p2_val: 0.23,
                    p3_name: "TID 3",
                    p3_val: 0.35,
                    mix: 0.30,
                    description: "Tre delaylinjor, 20–1500 ms, med egen nivå, gemensam återkoppling och stereospridning.",                    extra: vec![("NIVÅ 2", 0.7), ("NIVÅ 3", 0.5), ("ÅTERKOPPLING", 0.35), ("SPRIDNING", 0.6)],
                },
                FxPedal {
                    id: "distructor",
                    name: "💥 DISTRUCTOR",
                    category: "Distortion",
                    enabled: false,
                    color: crate::ui::theme::palette().red(),
                    p1_name: "DRIVE 1",
                    p1_val: 0.35,
                    p2_name: "DRIVE 2",
                    p2_val: 0.35,
                    p3_name: "DRIVE 3",
                    p3_val: 0.35,
                    mix: 0.50,
                    description: "Tre distorsioner: mjuk mättnad, hård klippning och vikning. SERIE staplar dem, PARALLELL blandar dem.",                    extra: vec![("PARALLELL", 0.0), ("TON", 0.70)],
                },
                FxPedal {
                    id: "hardcore",
                    name: "🎸 HARDCORE",
                    category: "Distortion",
                    enabled: false,
                    color: crate::ui::theme::palette().track_rose(),
                    p1_name: "KOMP",
                    p1_val: 0.30,
                    p2_name: "DRIVE",
                    p2_val: 0.35,
                    p3_name: "WAH",
                    p3_val: 0.60,
                    mix: 1.00,
                    description: "Pedalkedja i serie: KOMP → DRIVE → WAH → DELAY. Distorsion före wah ger den vokalliknande kurvan.",                    extra: vec![("WAH-DJUP", 0.0), ("DELAY", 0.2), ("EKON", 0.0)],
                },
                FxPedal {
                    id: "shelves",
                    name: "📉 TONKONTUR",
                    category: "Filter",
                    enabled: false,
                    color: crate::ui::theme::palette().green(),
                    p1_name: "BAS",
                    p1_val: 0.50,
                    p2_name: "DISKANT",
                    p2_val: 0.50,
                    p3_name: "BASFRQ",
                    p3_val: 0.35,
                    mix: 1.00,
                    description: "Två hyllor: BAS och DISKANT i dB (mitten = 0), BASFRQ väljer var den nedre knäcker. Low Lifter är samma sak med bara den nedre hyllan.",                    extra: Vec::new(),
                },
                FxPedal {
                    id: "stereo",
                    name: "↔ STEREO SHAPER",
                    category: "Utility",
                    enabled: false,
                    color: crate::ui::theme::palette().cyan(),
                    p1_name: "BREDD",
                    p1_val: 0.50,
                    p2_name: "BALANS",
                    p2_val: 0.50,
                    p3_name: "—",
                    p3_val: 0.0,
                    mix: 1.00,
                    description: "Bredd (mitt/sida), balans och polaritet per kanal. Bredd 0,5 = som inspelat.",                    extra: vec![("VÄND V", 0.0), ("VÄND H", 0.0)],
                },
                FxPedal {
                    id: "effector",
                    name: "🎛️ EFFECTOR",
                    category: "Performance",
                    enabled: false,
                    color: crate::ui::theme::palette().cable(),
                    p1_name: "X",
                    p1_val: 0.50,
                    p2_name: "Y",
                    p2_val: 0.30,
                    p3_name: "LÄGE",
                    p3_val: 0.0,
                    mix: 1.00,
                    description: "XY-effekt: LÄGE väljer krets (0 = svep, 1 = upprepning, 2 = port, 3 = kross), X styr hur mycket och Y hur hårt.",                    extra: Vec::new(),
                },
                FxPedal {
                    id: "chorus",
                    name: "🌊 CHORUS",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().track_blue(),
                    p1_name: "RATE",
                    p1_val: 0.25,
                    p2_name: "DEPTH",
                    p2_val: 0.50,
                    p3_name: "MIX",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Modulerad fördröjning 12 ms ± 8 ms med kvarts periods kanalskillnad — bredd utan grumlighet.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "flanger",
                    name: "🌀 FLANGER",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().track_teal(),
                    p1_name: "RATE",
                    p1_val: 0.12,
                    p2_name: "DEPTH",
                    p2_val: 0.70,
                    p3_name: "MIX",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Kort fördröjning (2,5 ms ± 2 ms) med 0,6 återkoppling — den vassa kammen.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "phaser",
                    name: "🌗 PHASER",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().cable(),
                    p1_name: "RATE",
                    p1_val: 0.30,
                    p2_name: "DEPTH",
                    p2_val: 0.70,
                    p3_name: "MIX",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Fyra allpass-steg med svepande brytfrekvens (200 Hz–2 kHz). Kammen ligger djupast vid mix 0,5.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "love_philter",
                    name: "💘 LOVE PHILTER",
                    category: "Filter",
                    enabled: false,
                    color: crate::ui::theme::palette().track_rose(),
                    p1_name: "CUTOFF",
                    p1_val: 0.55,
                    p2_name: "RESON",
                    p2_val: 0.40,
                    p3_name: "ENV",
                    p3_val: 0.0,
                    mix: 1.0,
                    description: "Tre band (lågpass/bandpass/högpass) kring CUTOFF. SPREAD är avståndet mellan banden i oktaver, ENV hur långt ett anslag öppnar banken, ATTACK/RELEASE är envelopens tid.",
                    extra: vec![("SPREAD", 0.45), ("ATTACK", 0.10), ("RELEASE", 0.45)],
                },
                FxPedal {
                    id: "analog_filter",
                    name: "🎛 SVF FILTER (12 dB)",
                    category: "Sound Design",
                    enabled: false,
                    color: crate::ui::theme::palette().yellow(),
                    p1_name: "CUTOFF",
                    p1_val: 0.85,
                    p2_name: "RESON",
                    p2_val: 0.30,
                    p3_name: "DRIVE",
                    p3_val: 0.20,
                    mix: 1.0,
                    description: "Resonant lågpass- och högpassfilter med analog värme.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "luxeverb",
                    name: "🌫 LUXEVERB",
                    category: "Echo & Delay",
                    enabled: false,
                    color: crate::ui::theme::palette().track_blue(),
                    p1_name: "DECAY",
                    p1_val: 0.20,
                    p2_name: "SIZE",
                    p2_val: 0.40,
                    p3_name: "DAMP",
                    p3_val: 0.35,
                    mix: 0.30,
                    description: "Modulerat fyrlinjigt reverb: DECAY är RT60 i sekunder, SIZE rummets storlek, DAMP hur mörk svansen blir, PREDELAY, MOD hur mycket linjerna rör sig och WIDTH bredden.",
                    extra: vec![("PREDELAY", 0.10), ("MOD", 0.35), ("WIDTH", 1.0)],
                },
                FxPedal {
                    id: "emphasis",
                    name: "✨ EMPHASIS",
                    category: "Color",
                    enabled: false,
                    color: crate::ui::theme::palette().orange(),
                    p1_name: "FREQ",
                    p1_val: 0.57,
                    p2_name: "DRIVE",
                    p2_val: 0.25,
                    p3_name: "—",
                    p3_val: 0.0,
                    mix: 0.40,
                    description: "Harmonisk exciter: FREQ är var övertonerna föds (1–10 kHz), DRIVE hur hårt bandet klipps, MIX hur mycket som blandas in. Ger både jämna och udda övertoner, till skillnad från en symmetrisk dist.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "hyper_chorus",
                    name: "🌊 HYPER CHORUS",
                    category: "Echo & Delay",
                    enabled: false,
                    color: crate::ui::theme::palette().cyan(),
                    p1_name: "RATE",
                    p1_val: 0.30,
                    p2_name: "DEPTH",
                    p2_val: 0.45,
                    p3_name: "WIDTH",
                    p3_val: 0.80,
                    mix: 0.35,
                    description: "Fyra chorusröster med egna LFO-hastigheter: RATE är svepets fart, DEPTH djupet, WIDTH bredden (0 = monokompatibelt) och MIX torrt/vått. Fyra röster glider isär, så bredden pumpar inte i takt med sig själv.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "time_curve",
                    name: "⏱ TIME CURVE",
                    category: "Performance",
                    enabled: false,
                    color: crate::ui::theme::palette().purple(),
                    p1_name: "FORM",
                    p1_val: 0.33,
                    p2_name: "TEMPO",
                    p2_val: 0.5,
                    p3_name: "—",
                    p3_val: 0.0,
                    mix: 1.0,
                    description: "Gross Beats tidskurva: masterns egen historik spelas tillbaka långsammare än i realtid. FORM väljer rak, stutter, tape stop eller halv fart; TEMPO styr hur snabbt kurvan går runt. Tonhöjden följer med — det är effekten.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "brickwall_limiter",
                    name: "🛡 MASTER LIMITER",
                    category: "Loudness",
                    enabled: true,
                    color: crate::ui::theme::palette().red(),
                    p1_name: "CEILING",
                    p1_val: 0.95,
                    p2_name: "RELEASE",
                    p2_val: 0.35,
                    p3_name: "BOOST",
                    p3_val: 0.50,
                    mix: 1.0,
                    description: "Förhindrar digital distorsion och maximerar ljudstyrkan.",
                    extra: Vec::new(),
                },
            ],
            selected_preset: 0,
            active_tab: 0,
            vocoder_bands: [1.0; 16],
            search_query: String::new(),
            selected_category: "Alla",
            eq_nodes: vec![
                // **Sju band**, samma mittfrekvenser som master-EQ:n räknar på (FL:s 7 Band EQ).
                VisualEqNode { name: "Low", freq_hz: 60.0, gain_db: 2.0, q: 0.7, color: crate::ui::theme::palette().red(), is_active: true },
                VisualEqNode { name: "Bass", freq_hz: 170.0, gain_db: -1.5, q: 1.0, color: crate::ui::theme::palette().amber(), is_active: true },
                VisualEqNode { name: "Low-Mid", freq_hz: 350.0, gain_db: 0.0, q: 1.2, color: crate::ui::theme::palette().orange(), is_active: true },
                VisualEqNode { name: "Mid", freq_hz: 1000.0, gain_db: 0.0, q: 1.0, color: crate::ui::theme::palette().green(), is_active: true },
                VisualEqNode { name: "High-Mid", freq_hz: 3500.0, gain_db: 3.0, q: 1.0, color: crate::ui::theme::palette().cyan(), is_active: true },
                VisualEqNode { name: "High", freq_hz: 8000.0, gain_db: 0.0, q: 0.9, color: crate::ui::theme::palette().purple(), is_active: true },
                VisualEqNode { name: "Air", freq_hz: 12000.0, gain_db: 1.5, q: 0.8, color: crate::ui::theme::palette().track_teal(), is_active: true },
            ],
            compressor_release_ms: 120.0,
            convolver_mix: 0.35,
            convolver_gain_db: 0.0,
            convolver_ir_name: None,
        }
    }
}

impl FxRackState {
    pub fn load_preset(&mut self, preset_idx: usize) {
        self.selected_preset = preset_idx;
        match preset_idx {
            0 => { // Warm Studio Vocal
                if let Some(p) = self.pedals.get_mut(0) { p.enabled = true; p.p1_val = 0.45; p.p2_val = 0.50; p.p3_val = 0.65; }
                if let Some(p) = self.pedals.get_mut(1) { p.enabled = true; p.p1_val = 0.60; p.p2_val = 0.45; }
                if let Some(p) = self.pedals.get_mut(2) { p.enabled = true; p.p1_val = 0.65; }
                if let Some(p) = self.pedals.get_mut(3) { p.enabled = true; p.p1_val = 0.40; p.mix = 0.25; }
                if let Some(p) = self.pedals.get_mut(4) { p.enabled = true; p.p1_val = 0.50; }
                if let Some(p) = self.pedals.get_mut(5) { p.enabled = true; p.p1_val = 0.30; }
            }
            1 => { // Punchy Modern Pop
                if let Some(p) = self.pedals.get_mut(0) { p.enabled = true; p.p1_val = 0.60; p.p3_val = 0.75; }
                if let Some(p) = self.pedals.get_mut(1) { p.enabled = true; p.p1_val = 0.75; p.p2_val = 0.60; }
                if let Some(p) = self.pedals.get_mut(2) { p.enabled = true; p.p1_val = 0.80; }
                if let Some(p) = self.pedals.get_mut(3) { p.enabled = true; p.mix = 0.35; }
            }
            2 => { // Lo-Fi Nostalgia & Warmth
                if let Some(p) = self.pedals.get_mut(0) { p.enabled = true; p.p1_val = 0.70; p.p3_val = 0.30; }
                if let Some(p) = self.pedals.get_mut(1) { p.enabled = true; p.p1_val = 0.50; }
                if let Some(p) = self.pedals.get_mut(6) { p.enabled = true; p.p1_val = 0.45; p.p2_val = 0.50; }
            }
            3 => { // 80s Gated Arena Reverb
                if let Some(p) = self.pedals.get_mut(1) { p.enabled = true; p.p1_val = 0.80; }
                if let Some(p) = self.pedals.get_mut(3) { p.enabled = true; p.p1_val = 0.85; p.mix = 0.55; }
                if let Some(p) = self.pedals.get_mut(5) { p.enabled = true; p.p1_val = 0.50; }
            }
            _ => {}
        }
    }

    fn pedal(&self, id: &str) -> Option<&FxPedal> {
        self.pedals.iter().find(|p| p.id == id)
    }

    /// Translate the UI rack state into the DSP parameter struct sent to the engine.
    pub fn build_master_fx_params(&self) -> MasterFxParams {
        let eq_pedal_on = self.pedal("visual_eq").map(|p| p.enabled).unwrap_or(false);
        let mut eq_bands = [MasterEqBand::flat(); 7];
        for (i, node) in self.eq_nodes.iter().take(7).enumerate() {
            eq_bands[i] = MasterEqBand {
                freq: node.freq_hz,
                gain_db: node.gain_db,
                q: node.q,
                active: node.is_active,
            };
        }

        let comp_p = self.pedal("dyn_comp");
        let comp = CompressorParams {
            threshold_db: -60.0 + comp_p.map(|p| p.p1_val).unwrap_or(0.65) * 60.0,
            ratio: 1.0 + comp_p.map(|p| p.p2_val).unwrap_or(0.4) * 11.0,
            attack_ms: 1.0 + comp_p.map(|p| p.p3_val).unwrap_or(0.3) * 99.0,
            release_ms: self.compressor_release_ms.max(5.0),
            makeup_db: comp_p.map(|p| (p.mix - 0.5) * 12.0).unwrap_or(0.0),
        };

        let dbl_p = self.pedal("vocal_doubler");
        let doubler = DoublerParams {
            strength: dbl_p.map(|p| p.p1_val).unwrap_or(0.7),
            modulation: dbl_p.map(|p| p.p2_val).unwrap_or(0.35),
            width: dbl_p.map(|p| p.p3_val).unwrap_or(0.8),
        };

        let ds_p = self.pedal("de_esser");
        let deesser = DeEsserParams {
            amount: ds_p.map(|p| p.p3_val).unwrap_or(0.45),
            freq: 4000.0 + ds_p.map(|p| p.p2_val).unwrap_or(0.65) * 8000.0,
        };

        let mb_p = self.pedal("multiband");
        let mb_extra = |i: usize| {
            mb_p.and_then(|p| p.extra.get(i)).map(|e| e.1.clamp(0.0, 1.0)).unwrap_or(0.5)
        };
        let multiband = MultibandParams {
            amounts: [
                mb_p.map(|p| p.p1_val).unwrap_or(0.0),
                mb_p.map(|p| p.p2_val).unwrap_or(0.0),
                mb_p.map(|p| p.p3_val).unwrap_or(0.0),
            ],
            // Den gemensamma tröskeln: ratten 0,5 = −18 dB, som är standardvärdet.
            threshold_db: -40.0 + mb_extra(0) * 44.0,
            ratio: 1.0 + mb_extra(1) * 11.0,
            // Bandets egen avvikelse: ratten 0,5 = ingen, ±1 på ratten är ±20 dB.
            offsets_db: [
                (mb_extra(2) - 0.5) * 40.0,
                (mb_extra(3) - 0.5) * 40.0,
                (mb_extra(4) - 0.5) * 40.0,
            ],
            master_amount: mb_extra(5),
            ..MultibandParams::default()
        };

        let tr_p = self.pedal("transient");
        let transient = TransientParams {
            attack_db: (tr_p.map(|p| p.p1_val).unwrap_or(0.5) * 2.0 - 1.0) * 12.0,
            sustain_db: (tr_p.map(|p| p.p2_val).unwrap_or(0.5) * 2.0 - 1.0) * 12.0,
            sensitivity: tr_p.map(|p| p.p3_val).unwrap_or(0.5),
        };

        let sat_p = self.pedal("saturator");
        let sat = SaturatorParams {
            drive: 1.0 + sat_p.map(|p| p.p1_val).unwrap_or(0.0).clamp(0.0, 1.0) * 19.0,
            tone: sat_p.map(|p| p.p2_val).unwrap_or(0.7),
            mix: sat_p.map(|p| p.p3_val).unwrap_or(0.5),
        };

        let gate_p = self.pedal("noise_gate");
        let gate = GateParams {
            threshold: {
                let thresh_db = -60.0 + gate_p.map(|p| p.p1_val).unwrap_or(0.25).clamp(0.0, 1.0) * 48.0;
                10.0_f32.powf(thresh_db / 20.0).clamp(0.0001, 0.5)
            },
            attack_ms: 1.0 + gate_p.map(|p| p.p2_val).unwrap_or(0.15) * 49.0,
            release_ms: 20.0 + gate_p.map(|p| p.p3_val).unwrap_or(0.4) * 480.0,
        };

        let filt_p = self.pedal("analog_filter");
        let filter = MasterFilterParams {
            cutoff: {
                let v = filt_p.map(|p| p.p1_val).unwrap_or(0.85).clamp(0.0, 1.0);
                20.0 * (20000.0_f32 / 20.0).powf(v)
            },
            resonance: 0.1 + filt_p.map(|p| p.p2_val).unwrap_or(0.30) * 7.9,
            drive: 1.0 + filt_p.map(|p| p.p3_val).unwrap_or(0.2) * 9.0,
            enabled: filt_p.map(|p| p.enabled).unwrap_or(false),
        };

        let lp_p = self.pedal("love_philter");
        let lp_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            lp_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let love_philter = LovePhilterParams {
            // 30 Hz–8 kHz, logaritmiskt: mitten av ratten hörs som mitten av registret.
            cutoff: 30.0 * (8000.0_f32 / 30.0).powf(lp_knob(|p| p.p1_val, 0.55)),
            spread: lp_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.45), 0.45) * 4.0,
            resonance: 0.2 + lp_knob(|p| p.p2_val, 0.40) * 7.8,
            attack_ms: 0.5 + lp_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.10), 0.10) * 199.5,
            release_ms: 5.0 + lp_knob(|p| p.extra.get(2).map(|e| e.1).unwrap_or(0.45), 0.45) * 995.0,
            env_octaves: lp_knob(|p| p.p3_val, 0.0) * 4.0,
            mix: lp_knob(|p| p.mix, 1.0),
            enabled: lp_p.map(|p| p.enabled).unwrap_or(false),
        };

        let lv_p = self.pedal("luxeverb");
        let lv_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            lv_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let luxeverb = LuxeverbParams {
            decay_s: 0.3 + lv_knob(|p| p.p1_val, 0.20) * 11.7,
            size: 0.5 + lv_knob(|p| p.p2_val, 0.40) * 1.5,
            damping: lv_knob(|p| p.p3_val, 0.35),
            modulation: lv_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.35), 0.35),
            predelay_ms: lv_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.10), 0.10) * 200.0,
            width: lv_knob(|p| p.extra.get(2).map(|e| e.1).unwrap_or(1.0), 1.0),
            mix: lv_knob(|p| p.mix, 0.30),
            enabled: lv_p.map(|p| p.enabled).unwrap_or(false),
        };

        let ex_p = self.pedal("emphasis");
        let ex_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            ex_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let hc_p = self.pedal("hyper_chorus");
        let hc_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            hc_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let hyper_chorus = HyperChorusParams {
            rate: 0.05 + hc_knob(|p| p.p1_val, 0.30) * 1.95,
            depth: hc_knob(|p| p.p2_val, 0.45),
            width: hc_knob(|p| p.p3_val, 0.80),
            mix: hc_knob(|p| p.mix, 0.35),
            enabled: hc_p.map(|p| p.enabled).unwrap_or(false),
        };

        let tc_p = self.pedal("time_curve");
        let tc_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            tc_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let shapes = TimeShape::ALL;
        let tc_shape = shapes
            .get((tc_knob(|p| p.p1_val, 0.33) * (shapes.len() - 1) as f32).round() as usize)
            .copied()
            .unwrap_or(TimeShape::Stutter);
        let time_curve = TimeCurveParams {
            curve: tc_shape.curve(),
            bpm: 60.0 + tc_knob(|p| p.p2_val, 0.5) * 120.0,
            mix: tc_knob(|p| p.mix, 1.0),
            enabled: tc_p.map(|p| p.enabled).unwrap_or(false),
        };

        let emphasis = EmphasisParams {
            // 1–10 kHz, logaritmiskt: mitten av ratten hörs som mitten av registret.
            freq_hz: 1_000.0 * 10.0_f32.powf(ex_knob(|p| p.p1_val, 0.57)),
            drive: 1.0 + ex_knob(|p| p.p2_val, 0.25) * 9.0,
            mix: ex_knob(|p| p.mix, 0.40),
            enabled: ex_p.map(|p| p.enabled).unwrap_or(false),
        };

        let lim_p = self.pedal("brickwall_limiter");
        let limiter = LimiterParams {
            ceiling: lim_p.map(|p| p.p1_val).unwrap_or(0.95).clamp(0.1, 1.0),
            release_ms: 5.0 + lim_p.map(|p| p.p2_val).unwrap_or(0.35) * 245.0,
            boost: 0.5 + lim_p.map(|p| p.p3_val).unwrap_or(0.5) * 2.5,
        };

        let gr_p = self.pedal("granular");
        let knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            gr_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let granular = GranularParams {
            size_ms: 10.0 + knob(|p| p.p1_val, 0.3) * 190.0,
            density: 2.0 + knob(|p| p.p2_val, 0.2) * 58.0,
            position_ms: 20.0 + knob(|p| p.p3_val, 0.1) * 980.0,
            spray_ms: knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.2), 0.2) * 200.0,
            // 0,25× till 4× (två oktaver ner, två upp) på en logaritmisk ratt: mitten är 1,0.
            speed: 2.0f32.powf(knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.5), 0.5) * 4.0 - 2.0),
            mix: knob(|p| p.mix, 0.6),
        };

        let fs_p = self.pedal("freqshift");
        let fs_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            fs_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let freq_shift = FreqShiftParams {
            // SKIFT är en magnitud; RIKTNING avgör tecknet.
            hz: fs_knob(|p| p.p1_val, 0.5)
                * 2000.0
                * if fs_knob(|p| p.p2_val, 0.5) >= 0.5 { 1.0 } else { -1.0 },
            mix: fs_knob(|p| p.mix, 0.5),
            spread: fs_knob(|p| p.p3_val, 0.0) >= 0.5,
        };

        let cr_p = self.pedal("bitcrush");
        let cr_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            cr_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let crush = BitCrushParams {
            // 1 bit i botten, 16 i toppen: ratten är bitdjupet, inte ett påslag.
            bits: 1.0 + cr_knob(|p| p.p1_val, 1.0) * 15.0,
            downsample: 1.0 + (cr_knob(|p| p.p2_val, 0.0) * 31.0).round(),
            tone: cr_knob(|p| p.p3_val, 0.5),
            mix: cr_knob(|p| p.mix, 1.0),
        };

        let mb_p = self.pedal("mbdelay");
        let mb_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            mb_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let mb_delay = MultibandDelayParams {
            times_ms: [
                20.0 + mb_knob(|p| p.p1_val, 0.1) * 780.0,
                20.0 + mb_knob(|p| p.p2_val, 0.25) * 780.0,
                20.0 + mb_knob(|p| p.p3_val, 0.45) * 780.0,
            ],
            xover_low: 250.0,
            xover_high: 4000.0,
            mix: mb_knob(|p| p.mix, 0.35),
        };

        let db_p = self.pedal("delaybank");
        let db_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            db_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let delay_bank = DelayBankParams {
            times_ms: [
                20.0 + db_knob(|p| p.p1_val, 0.08) * 1480.0,
                20.0 + db_knob(|p| p.p2_val, 0.23) * 1480.0,
                20.0 + db_knob(|p| p.p3_val, 0.35) * 1480.0,
            ],
            levels: [
                1.0,
                db_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.7), 0.7),
                db_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.5), 0.5),
            ],
            feedback: db_knob(|p| p.extra.get(2).map(|e| e.1).unwrap_or(0.35), 0.35),
            spread: db_knob(|p| p.extra.get(3).map(|e| e.1).unwrap_or(0.6), 0.6),
            mix: db_knob(|p| p.mix, 0.3),
        };

        let dist_p = self.pedal("distructor");
        let dist_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            dist_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let distructor = DistructorParams {
            drive: [
                dist_knob(|p| p.p1_val, 0.35),
                dist_knob(|p| p.p2_val, 0.35),
                dist_knob(|p| p.p3_val, 0.35),
            ],
            parallel: dist_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.0), 0.0),
            tone_hz: 300.0 * (10.0f32).powf(dist_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.7), 0.7) * 1.8),
            mix: dist_knob(|p| p.mix, 0.5),
        };

        let hc_p = self.pedal("hardcore");
        let hc_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            hc_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let hardcore = HardcoreParams {
            comp: hc_knob(|p| p.p1_val, 0.3),
            drive: hc_knob(|p| p.p2_val, 0.35),
            wah_hz: 300.0 * (10.0f32).powf(hc_knob(|p| p.p3_val, 0.6) * 1.5),
            wah_depth: hc_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.0), 0.0),
            delay_ms: 40.0 + hc_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.2), 0.2) * 460.0,
            delay_mix: hc_knob(|p| p.extra.get(2).map(|e| e.1).unwrap_or(0.0), 0.0),
            mix: hc_knob(|p| p.mix, 1.0),
        };

        let ef_p = self.pedal("effector");
        let ef_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            ef_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let effector = EffectorParams {
            x: ef_knob(|p| p.p1_val, 0.5),
            y: ef_knob(|p| p.p2_val, 0.3),
            // p3 väljer läge bland `EFFECTOR_MODES`.
            mode: (ef_knob(|p| p.p3_val, 0.0) * 3.0).round() as u8,
            mix: ef_knob(|p| p.mix, 1.0),
        };

        let st_p = self.pedal("stereo");
        let st_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            st_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let stereo = StereoParams {
            // BREDD 0–1 blir 0–2×, så mitten av ratten är "som inspelat".
            width: st_knob(|p| p.p1_val, 0.5) * 2.0,
            balance: st_knob(|p| p.p2_val, 0.5) * 2.0 - 1.0,
            invert_l: st_knob(|p| p.extra.first().map(|e| e.1).unwrap_or(0.0), 0.0) > 0.5,
            invert_r: st_knob(|p| p.extra.get(1).map(|e| e.1).unwrap_or(0.0), 0.0) > 0.5,
            // **Fruity Center** på tredje extra-ratten: 0,5 = neutralt, under = bort med
            // mitten, över = bort med sidorna.
            center: st_knob(|p| p.extra.get(2).map(|e| e.1).unwrap_or(0.5), 0.5) * 2.0 - 1.0,
            mix: st_knob(|p| p.mix, 1.0),
        };

        let sh_p = self.pedal("shelves");
        let sh_knob = |pick: fn(&FxPedal) -> f32, fallback: f32| {
            sh_p.map(|p| pick(p).clamp(0.0, 1.0)).unwrap_or(fallback)
        };
        let shelves = ShelvesParams {
            low_hz: 40.0 * (10.0f32).powf(sh_knob(|p| p.p3_val, 0.35) * 1.2),
            // Ratten står i mitten = 0 dB, och går ±15 dB.
            low_db: (sh_knob(|p| p.p1_val, 0.5) - 0.5) * 30.0,
            high_hz: 3_000.0,
            high_db: (sh_knob(|p| p.p2_val, 0.5) - 0.5) * 30.0,
            mix: sh_knob(|p| p.mix, 1.0),
        };

        let ch_p = self.pedal("chorus");
        let chorus = ModFxParams {
            rate: 0.05 + ch_p.map(|p| p.p1_val).unwrap_or(0.25) * 4.0,
            depth: ch_p.map(|p| p.p2_val).unwrap_or(0.5),
            mix: ch_p.map(|p| p.p3_val).unwrap_or(0.5),
            feedback: 0.0,
        };

        let fl_p = self.pedal("flanger");
        let flanger = ModFxParams {
            rate: 0.05 + fl_p.map(|p| p.p1_val).unwrap_or(0.12) * 2.0,
            depth: fl_p.map(|p| p.p2_val).unwrap_or(0.7),
            mix: fl_p.map(|p| p.p3_val).unwrap_or(0.5),
            // 0,6 är det som gör en modulerad fördröjning till en flanger i stället för chorus.
            feedback: 0.6,
        };

        let ph_p = self.pedal("phaser");
        let phaser = ModFxParams {
            rate: 0.05 + ph_p.map(|p| p.p1_val).unwrap_or(0.3) * 4.0,
            depth: ph_p.map(|p| p.p2_val).unwrap_or(0.7),
            mix: ph_p.map(|p| p.p3_val).unwrap_or(0.5),
            feedback: 0.2,
        };

        MasterFxParams {
            control: self.control_room,
            granular,
            granular_enabled: gr_p.map(|p| p.enabled).unwrap_or(false),
            freq_shift,
            freq_shift_enabled: fs_p.map(|p| p.enabled).unwrap_or(false),
            crush,
            crush_enabled: cr_p.map(|p| p.enabled).unwrap_or(false),
            mb_delay,
            mb_delay_enabled: mb_p.map(|p| p.enabled).unwrap_or(false),
            delay_bank,
            delay_bank_enabled: db_p.map(|p| p.enabled).unwrap_or(false),
            distructor,
            distructor_enabled: dist_p.map(|p| p.enabled).unwrap_or(false),
            hardcore,
            hardcore_enabled: hc_p.map(|p| p.enabled).unwrap_or(false),
            effector,
            effector_enabled: ef_p.map(|p| p.enabled).unwrap_or(false),
            stereo,
            stereo_enabled: st_p.map(|p| p.enabled).unwrap_or(false),
            shelves,
            shelves_enabled: sh_p.map(|p| p.enabled).unwrap_or(false),
            eq_bands,
            eq_enabled: eq_pedal_on,
            comp,
            comp_enabled: comp_p.map(|p| p.enabled).unwrap_or(false),
            doubler,
            doubler_enabled: dbl_p.map(|p| p.enabled).unwrap_or(false),
            deesser,
            deesser_enabled: ds_p.map(|p| p.enabled).unwrap_or(false),
            transient,
            transient_enabled: tr_p.map(|p| p.enabled).unwrap_or(false),
            sat,
            sat_enabled: sat_p.map(|p| p.enabled).unwrap_or(false),
            chorus,
            chorus_enabled: ch_p.map(|p| p.enabled).unwrap_or(false),
            flanger,
            flanger_enabled: fl_p.map(|p| p.enabled).unwrap_or(false),
            phaser,
            phaser_enabled: ph_p.map(|p| p.enabled).unwrap_or(false),
            multiband,
            multiband_enabled: mb_p.map(|p| p.enabled).unwrap_or(false),
            convolver: ConvolverParams {
                mix: self.convolver_mix,
                gain_db: self.convolver_gain_db,
            },
            // Utan impulsrespons finns inget att faltha: flaggan **är** filens närvaro.
            convolver_enabled: self.convolver_ir_name.is_some(),
            gate,
            gate_enabled: gate_p.map(|p| p.enabled).unwrap_or(false),
            filter,
            philter: love_philter,
            luxeverb,
            emphasis,
            hyper_chorus,
            time_curve,
            limiter,
            limiter_enabled: lim_p.map(|p| p.enabled).unwrap_or(false),
        }
    }

    /// Läser en `.wav` som impulsrespons och skickar den till motorn.
    ///
    /// Filen läses som mono (`(vänster + höger) / 2`) och skalas om linjärt om dess
    /// samplingsfrekvens skiljer sig från motorns — `shortcut:` linjär interpolering, vilket
    /// räcker för en rumsrespons; byt till `stretch`-modulens kvalitet om någon hör skillnad.
    pub fn load_convolver_ir(&mut self, engine: &mut AudioEngine) -> Option<String> {
        let path = rfd::FileDialog::new()
            .add_filter("WAV", &["wav"])
            .pick_file()?;
        let (l, r, sr) = match crate::audio::load_wav_pcm(&path.to_string_lossy()) {
            Ok(v) => v,
            Err(e) => {
                return Some(crate::tstatus!(
                    "⚠️ Kunde inte läsa '{}': {}",
                    path.display(),
                    e
                ));
            }
        };
        let mut mono: Vec<f32> = l
            .iter()
            .zip(r.iter())
            .map(|(a, b)| (a + b) * 0.5)
            .collect();
        let target = engine.sample_rate;
        if sr > 0 && target > 0 && sr != target {
            mono = resample_linear(&mono, sr, target);
        }
        if mono.is_empty() {
            return Some(crate::i18n::t("⚠️ Filen är tom — ingen impulsrespons laddad").to_string());
        }
        let len = mono.len();
        let secs = len as f32 / target.max(1) as f32;
        let _ = engine.send_command(AudioCommand::SetConvolverIr(Some(std::sync::Arc::new(mono))));
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.convolver_ir_name = Some(name.clone());
        Some(crate::tstatus!(
            "🌀 Impulsrespons '{}' laddad ({:.2} s, {} Hz → {} Hz)",
            name,
            secs,
            sr,
            target
        ))
    }

    /// Tar bort impulsresponsen (convolvern blir en bit-exakt genomgång).
    pub fn clear_convolver_ir(&mut self, engine: &mut AudioEngine) {
        let _ = engine.send_command(AudioCommand::SetConvolverIr(None));
        self.convolver_ir_name = None;
    }

    /// Genererar en impulsrespons ur banken och skickar in den.
    ///
    /// Banken är vår egen modell (brus + tre tal), inte inspelade rum — statusraden säger det,
    /// så ingen tror att en genererad hall är samma ljud som FL:s inspelade.
    pub fn load_bank_ir(&mut self, engine: &mut AudioEngine, index: usize) -> String {
        let Some(recipe) = IR_BANK.get(index) else {
            return crate::i18n::t("⚠️ Okänt bankindex").to_string();
        };
        let ir = synthesize_ir(recipe, engine.sample_rate.max(1) as f32);
        let secs = ir.len() as f32 / (engine.sample_rate.max(1) as f32);
        let len = ir.len();
        let _ = engine.send_command(AudioCommand::SetConvolverIr(Some(std::sync::Arc::new(ir))));
        self.convolver_ir_name = Some(format!("{} (genererad)", recipe.name));
        crate::tstatus!(
            "🏛 '{}' genererad: {:.1} s ({} sampel) — modell, inte en inspelning",
            recipe.name,
            secs,
            len
        )
    }

    /// Push the current rack to the audio engine in real time.
    pub fn sync_to_engine(&self, engine: &mut AudioEngine) {
        let _ = engine.send_command(AudioCommand::SetMasterFx(self.build_master_fx_params()));

        if let Some(p) = self.pedal("studio_reverb") {
            let params = ReverbParams {
                room_size: p.p1_val,
                damping: p.p2_val,
                mix: if p.enabled { p.mix } else { 0.0 },
            };
            let _ = engine.send_command(AudioCommand::SetReverb(params));
        }
    }
}


/// **Vocoderns band** — 16 reglage, ett per band, 0…2. Returen säger om något ändrades, så
/// kommandot bara skickas när det behövs.
fn render_vocoder_bands(ui: &mut Ui, state: &mut FxRackState) -> bool {
    let mut changed = false;
    ui.label(
        egui::RichText::new(crate::i18n::t(
            "Nivå per band i masterns vocoder: 1,0 är rakt igenom, 0 stänger bandet. Banden går från 100 Hz till 8 kHz.",
        ))
        .size(Theme::FONT_S)
        .color(crate::ui::theme::palette().text_muted()),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (i, gain) in state.vocoder_bands.iter_mut().enumerate() {
            ui.vertical(|ui| {
                if ui
                    .add(
                        egui::Slider::new(gain, 0.0..=2.0)
                            .vertical()
                            .show_value(false)
                            .text(format!("{i}")),
                    )
                    .changed()
                {
                    changed = true;
                }
                // Bandets ungefärliga mittfrekvens, så reglagen går att känna igen.
                let t = i as f32 / 15.0;
                let hz = 100.0 * (8_000.0f32 / 100.0).powf(t);
                ui.label(
                    egui::RichText::new(if hz >= 1_000.0 {
                        format!("{:.0}k", hz / 1_000.0)
                    } else {
                        format!("{hz:.0}")
                    })
                    .size(Theme::FONT_XS)
                    .color(crate::ui::theme::palette().text_muted()),
                );
            });
        }
    });
    ui.add_space(4.0);
    if ui.button(crate::i18n::t("↺ Nollställ banden")).clicked() {
        state.vocoder_bands = [1.0; 16];
        changed = true;
    }
    changed
}

pub fn render_fx_rack_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut FxRackState,
    engine: &mut AudioEngine,
    status_msg: &mut String,
) {
    if !*open {
        return;
    }

    // Push the current rack state to the DSP every frame for real-time auditioning.
    state.sync_to_engine(engine);

    // Real compressor gain reduction reported by the audio thread.
    let master_gr_db = engine.master_gain_reduction_db();

    let mut close = false;

    egui::Window::new(crate::i18n::t("🎛 Soundtrap Studio Effects & FX Rack"))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(1180.0, 680.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // ============================================================
                // 1. TOP HEADER & NAVIGATION TABS
                // ============================================================
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎛 SOUNDTRAP FX RACK")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().orange()));
                    ui.separator();
                    // Flikarna får hela raden: låg den dekorativa rubriken kvar tog fyra flikar
                    // slut på bredden och ritades ovanpå varandra (sett i skärmdump 37).
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let voc_btn = ui.selectable_label(state.active_tab == 3, crate::i18n::t("🎚 Vocoder"));
                        if voc_btn.clicked() { state.active_tab = 3; }
                        let lib_btn = ui.selectable_label(state.active_tab == 2, crate::i18n::t("🗂 Effektbibliotek"));
                        if lib_btn.clicked() { state.active_tab = 2; }

                        let eq_btn = ui.selectable_label(state.active_tab == 1, "📊 Visual EQ");
                        if eq_btn.clicked() { state.active_tab = 1; }

                        let rack_btn = ui.selectable_label(state.active_tab == 0, crate::i18n::t("🎛 Effektkedja"));
                        if rack_btn.clicked() { state.active_tab = 0; }
                    });
                });

                // Control Room ligger överst därför att den gäller **allt** som lämnar
                // mastern — den är lyssning, inte effekt.
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("🎧 CONTROL ROOM"))
                                .strong()
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().cyan()),
                        );
                        ui.separator();
                        ui.checkbox(&mut state.control_room.mono, crate::i18n::t("Mono"))
                            .on_hover_text(crate::i18n::t(
                                "Lägger ihop kanalerna: en mix som bara håller i stereo faller här.",
                            ));
                        ui.checkbox(&mut state.control_room.dim, crate::i18n::t("Dim"))
                            .on_hover_text(crate::i18n::t("Sänker lyssningen utan att röra mastern."));
                        ui.add(
                            egui::DragValue::new(&mut state.control_room.dim_db)
                                .range(-60.0..=0.0)
                                .speed(0.5)
                                .suffix(" dB"),
                        );
                        ui.separator();
                        ui.label(crate::i18n::t("Lyssning:"));
                        ui.add(
                            egui::Slider::new(&mut state.control_room.level_db, -40.0..=6.0)
                                .suffix(" dB")
                                .fixed_decimals(1),
                        );
                    });
                });

                ui.add_space(4.0);

                // Presets toolbar
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Kedjemallar (Presets):")).strong().color(crate::ui::theme::palette().cyan()));
                        let presets = ["Warm Studio Vocal", "Punchy Modern Pop", "Lo-Fi Nostalgia & Warmth", "80s Gated Arena Reverb", "Clean Master"];
                        egui::ComboBox::from_id_salt("fx_master_preset_picker")
                            .selected_text(*presets.get(state.selected_preset).unwrap_or(&"Standard"))
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for (pr_idx, &pr_name) in presets.iter().enumerate() {
                                    if ui.selectable_label(state.selected_preset == pr_idx, pr_name).clicked() {
                                        state.load_preset(pr_idx);
                                        *status_msg = crate::tstatus!("🎛 Laddade FX-preset: {}", pr_name);
                                    }
                                }
                            });

                        ui.separator();
                        if ui.button(crate::i18n::t("⚡ Aktivera alla")).clicked() {
                            for p in &mut state.pedals { p.enabled = true; }
                        }
                        if ui.button(crate::i18n::t("⚪ Förbikoppla (Bypass)")).clicked() {
                            for p in &mut state.pedals { p.enabled = false; }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let enabled_count = state.pedals.iter().filter(|p| p.enabled).count();
                            ui.label(egui::RichText::new(crate::tstatus!("{} aktiva effekter", enabled_count)).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                            // Kedjan är horisontell och rymmer inte nio pedaler i fönstret — säg det,
                            // i stället för att låta resten se ut som att den inte finns.
                            if enabled_count > 5 {
                                ui.label(egui::RichText::new(crate::i18n::t("· scrolla i kedjan →")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                            }
                        });
                    });
                });

                ui.add_space(6.0);

                // ============================================================
                // 2. MAIN CONTENT TABS
                // ============================================================
                match state.active_tab {
                    // TAB 0: MODULAR PEDAL CHAIN (Soundtrap Horizontal Cards)
                    0 => {
                        render_modular_pedal_chain(ui, state, master_gr_db);
                    }
                    // TAB 1: VISUAL PARAMETRIC EQ (Interactive Frequency Canvas)
                    1 => {
                        render_visual_parametric_eq(ui, state);
                    }
                    // TAB 2: CATEGORIZED EFFECTS LIBRARY DRAWER
                    2 => {
                        render_effects_library_drawer(ui, state, status_msg);
                    }
                    // TAB 3: VOCODERNS BAND
                    3 => {
                        let changed = render_vocoder_bands(ui, state);
                        if changed {
                            let _ = engine.send_command(crate::audio::AudioCommand::SetVocoderBands {
                                gains: state.vocoder_bands,
                            });
                        }
                    }
                    _ => {}
                }

                ui.add_space(8.0);

                // Convolvern har sin egen rad: två reglage och en fil, inte tre rattar som
                // pedalerna — en impulsrespons är inte en ratt.
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("🌀 CONVOLVER (impulsrespons)"))
                                .strong()
                                .color(crate::ui::theme::palette().cyan()),
                        );
                        let name = state
                            .convolver_ir_name
                            .clone()
                            .unwrap_or_else(|| crate::i18n::t("ingen fil laddad").to_string());
                        ui.label(egui::RichText::new(name).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        if ui.button(crate::i18n::t("📥 Ladda .wav")).clicked()
                            && let Some(msg) = state.load_convolver_ir(engine) {
                                *status_msg = msg;
                            }
                        if state.convolver_ir_name.is_some() && ui.button(crate::i18n::t("🗑 Rensa")).clicked() {
                            state.clear_convolver_ir(engine);
                            *status_msg = crate::i18n::t("🌀 Impulsresponsen är borttagen").to_string();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Slider::new(&mut state.convolver_mix, 0.0..=1.0)
                                .text(crate::i18n::t("Torrt/vått")),
                        );
                        ui.add(
                            egui::Slider::new(&mut state.convolver_gain_db, -24.0..=12.0)
                                .text(crate::i18n::t("Nivå (dB)")),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("🏛 Bank:"))
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                        for (i, recipe) in IR_BANK.iter().enumerate() {
                            if ui.small_button(recipe.name).clicked() {
                                *status_msg = state.load_bank_ir(engine, i);
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // ============================================================
                // 3. ACTION BAR & BOTTOM CONTROLS
                // ============================================================
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(egui::RichText::new(crate::i18n::t("✔ Tillämpa på Spår & Master")).strong().color(Color32::BLACK))
                            .fill(crate::ui::theme::palette().green())
                            .min_size(Vec2::new(220.0, 32.0)),
                    ).clicked() {
                        state.sync_to_engine(engine);
                        *status_msg = crate::i18n::t("✔ Effektinställningar applicerade i realtid!").to_string();
                        close = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(crate::i18n::t("Stäng")).clicked() {
                            close = true;
                        }
                    });
                });
            });
        });

    if close {
        *open = false;
    }
}

// ============================================================================
// MODULAR PEDAL CHAIN RENDERER
// ============================================================================
fn render_modular_pedal_chain(ui: &mut Ui, state: &mut FxRackState, master_gr_db: f32) {
    // Split the borrow so the Visual EQ pedal can read/write the real EQ nodes.
    let FxRackState { pedals, eq_nodes, active_tab, .. } = state;

    egui::ScrollArea::horizontal()
        .max_height(400.0)
        // Nio pedaler får inte plats i fönstret, och en osynlig skrollbar gör att resten ser ut
        // att inte finnas (sett i skärmdump 25: fem kort synliga, nio aktiva).
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);

                for pedal in pedals.iter_mut() {
                    let card_w = 175.0;
                    let card_h = 360.0;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());

                    let card_bg = if pedal.enabled {
                        crate::ui::theme::palette().header()
                    } else {
                        crate::ui::theme::palette().bg()
                    };

                    ui.painter().rect_filled(rect, Rounding::same(8.0), card_bg);
                    ui.painter().rect_stroke(
                        rect,
                        Rounding::same(8.0),
                        Stroke::new(1.5_f32, if pedal.enabled { pedal.color } else { crate::ui::theme::palette().border() }),
                    );

                    // Top Header Strip
                    let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 26.0));
                    ui.painter().rect_filled(top_bar, Rounding::same(7.0), if pedal.enabled { pedal.color } else { crate::ui::theme::palette().panel_alt() });

                    ui.painter().text(
                        Pos2::new(rect.min.x + 8.0, rect.min.y + 13.0),
                        egui::Align2::LEFT_CENTER,
                        pedal.name,
                        egui::FontId::proportional(Theme::FONT_S),
                        if pedal.enabled { Color32::BLACK } else { crate::ui::theme::palette().text_muted() },
                    );

                    // Power LED
                    let led_pos = Pos2::new(rect.max.x - 12.0, rect.min.y + 13.0);
                    ui.painter().circle_filled(
                        led_pos,
                        4.5,
                        if pedal.enabled { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().panel_alt() },
                    );

                    // Card Content
                    let mut child = ui.new_child(
                        egui::UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(rect.min.x + 8.0, rect.min.y + 32.0), rect.max - Vec2::new(8.0, 8.0)))
                    );

                    child.vertical(|ui| {
                        ui.checkbox(&mut pedal.enabled, crate::i18n::t("Aktiv"));
                        ui.add_space(4.0);

                        // Special visuals per pedal type
                        if pedal.id == "visual_eq" {
                            let (eq_box, _) = ui.allocate_exact_size(Vec2::new(card_w - 20.0, 50.0), Sense::hover());
                            ui.painter().rect_filled(eq_box, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                            ui.painter().rect_stroke(eq_box, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));
                            // Live mini curve derived from the real 4-band EQ nodes.
                            let mid = eq_box.center().y;
                            let span = eq_box.height() * 0.42;
                            let steps = 48;
                            let mut prev: Option<Pos2> = None;
                            for s in 0..=steps {
                                let norm = s as f32 / steps as f32;
                                let f = 20.0 * (20000.0_f32 / 20.0).powf(norm);
                                let mut gain = 0.0_f32;
                                for node in eq_nodes.iter() {
                                    if !node.is_active { continue; }
                                    let oct = (f / node.freq_hz).log2();
                                    gain += node.gain_db * (-oct.powi(2) * node.q * 1.8).exp();
                                }
                                let p = Pos2::new(
                                    eq_box.min.x + norm * eq_box.width(),
                                    mid - (gain.clamp(-12.0, 12.0) / 12.0) * span,
                                );
                                if let Some(q) = prev {
                                    ui.painter().line_segment([q, p], Stroke::new(1.8_f32, pedal.color));
                                }
                                prev = Some(p);
                            }
                        } else if pedal.id == "dyn_comp" {
                            let (gr_box, _) = ui.allocate_exact_size(Vec2::new(card_w - 20.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(gr_box, Rounding::same(2.0), crate::ui::theme::palette().bg_deep());
                            // Real gain reduction reported by the audio thread (0..-20 dB).
                            let gr_frac = (master_gr_db.abs() / 20.0).clamp(0.0, 1.0);
                            let fill_w = gr_box.width() * gr_frac;
                            ui.painter().rect_filled(Rect::from_min_size(gr_box.min, Vec2::new(fill_w, gr_box.height())), Rounding::same(2.0), crate::ui::theme::palette().orange());
                            let gr_txt = if master_gr_db <= -0.05 { format!("GR {:.1} dB", master_gr_db) } else { "GR".to_string() };
                            ui.painter().text(gr_box.center(), egui::Align2::CENTER_CENTER, gr_txt, egui::FontId::proportional(Theme::FONT_XS), Color32::BLACK);
                        } else if pedal.id == "vocal_doubler" {
                            let (orb_box, _) = ui.allocate_exact_size(Vec2::new(card_w - 20.0, 45.0), Sense::hover());
                            ui.painter().rect_filled(orb_box, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                            let c1 = Pos2::new(orb_box.center().x - 14.0, orb_box.center().y);
                            let c2 = Pos2::new(orb_box.center().x + 14.0, orb_box.center().y);
                            ui.painter().circle_stroke(c1, 16.0, Stroke::new(1.5_f32, pedal.color));
                            ui.painter().circle_stroke(c2, 16.0, Stroke::new(1.5_f32, crate::ui::theme::palette().track_blue()));
                        }

                        ui.add_space(6.0);

                        if pedal.id == "visual_eq" {
                            // **Sju rattar, sju band** — samma tillstånd som Visual EQ-fliken.
                            let short = ["LOW", "BASS", "L-MID", "MID", "H-MID", "HIGH", "AIR"];
                            for (i, node) in eq_nodes.iter_mut().enumerate() {
                                let label = short.get(i).copied().unwrap_or("BAND");
                                rotary_knob(ui, &mut node.gain_db, -12.0, 12.0, label, node.color, 14.0);
                            }
                        } else {
                            rotary_knob(ui, &mut pedal.p1_val, 0.0, 1.0, pedal.p1_name, pedal.color, 18.0);
                            rotary_knob(ui, &mut pedal.p2_val, 0.0, 1.0, pedal.p2_name, pedal.color, 18.0);
                            rotary_knob(ui, &mut pedal.p3_val, 0.0, 1.0, pedal.p3_name, pedal.color, 18.0);
                            for (name, val) in pedal.extra.iter_mut() {
                                rotary_knob(ui, val, 0.0, 1.0, name, pedal.color, 18.0);
                            }
                            rotary_knob(ui, &mut pedal.mix, 0.0, 1.0, "MIX", crate::ui::theme::palette().text(), 18.0);
                        }

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(pedal.category).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                    });
                }

                // Add Card (+) at end of chain
                let add_w = 140.0;
                let add_h = 360.0;
                let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(add_w, add_h), Sense::click());
                ui.painter().rect_filled(add_rect, Rounding::same(8.0), crate::ui::theme::palette().bg());
                ui.painter().rect_stroke(add_rect, Rounding::same(8.0), Stroke::new(1.0_f32, Stroke::new(1.0_f32, crate::ui::theme::palette().border()).color));
                ui.painter().text(add_rect.center() - Vec2::new(0.0, 15.0), egui::Align2::CENTER_CENTER, "➕", egui::FontId::proportional(Theme::FONT_XL), crate::ui::theme::palette().cyan());
                ui.painter().text(add_rect.center() + Vec2::new(0.0, 18.0), egui::Align2::CENTER_CENTER, crate::i18n::t("Lägg till från\nEffektbiblioteket"), egui::FontId::proportional(Theme::FONT_S), crate::ui::theme::palette().text());

                if add_resp.clicked() {
                    *active_tab = 2; // Switch to library
                }
            });
        });
}

// ============================================================================
// VISUAL PARAMETRIC EQ RENDERER
// ============================================================================
fn render_visual_parametric_eq(ui: &mut Ui, state: &mut FxRackState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("📊 4-BANDS PARAMETRISK EQUALIZER")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().cyan()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Dra i noderna för att justera frekvens och förstärkning (dB)")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        });

        ui.add_space(4.0);

        let canvas_w = ui.available_width().max(300.0);
        let canvas_h = 240.0;
        let (canvas_rect, canvas_resp) = ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), Sense::click_and_drag());
        let painter = ui.painter();

        // Canvas Background & Grid
        painter.rect_filled(canvas_rect, Rounding::same(6.0), crate::ui::theme::palette().bg_deep());
        painter.rect_stroke(canvas_rect, Rounding::same(6.0), Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

        let mid_y = canvas_rect.center().y;

        // Draw dB horizontal grid lines (+12, +6, 0, -6, -12)
        for db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
            let y = mid_y - (db / 12.0) * (canvas_h * 0.42);
            let stroke_col = if db == 0.0 { crate::ui::theme::palette().border_soft() } else { crate::ui::theme::palette().panel() };
            painter.line_segment([Pos2::new(canvas_rect.min.x, y), Pos2::new(canvas_rect.max.x, y)], Stroke::new(1.0_f32, stroke_col));
            painter.text(Pos2::new(canvas_rect.min.x + 6.0, y - 2.0), egui::Align2::LEFT_BOTTOM, format!("{:+1.0} dB", db), egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().text_muted());
        }

        // Draw frequency vertical grid lines (50Hz, 100Hz, 500Hz, 1kHz, 5kHz, 10kHz, 20kHz)
        let freq_ticks = [50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 20000.0];
        let freq_to_x = |f: f32| -> f32 {
            let min_log = 20.0_f32.log10();
            let max_log = 20000.0_f32.log10();
            let norm = (f.clamp(20.0, 20000.0).log10() - min_log) / (max_log - min_log);
            canvas_rect.min.x + norm * canvas_rect.width()
        };

        for &f in &freq_ticks {
            let x = freq_to_x(f);
            painter.line_segment([Pos2::new(x, canvas_rect.min.y), Pos2::new(x, canvas_rect.max.y)], Stroke::new(0.8_f32, crate::ui::theme::palette().panel()));
            let label = if f >= 1000.0 { format!("{:.0}k", f / 1000.0) } else { format!("{:.0}", f) };
            painter.text(Pos2::new(x, canvas_rect.max.y - 4.0), egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().border_soft());
        }

        // Compute EQ curve points
        let num_steps = 180;
        let mut curve_pts = Vec::with_capacity(num_steps);
        for step in 0..num_steps {
            let norm = step as f32 / num_steps as f32;
            let f = 10.0_f32.powf(20.0_f32.log10() + norm * (20000.0_f32.log10() - 20.0_f32.log10()));

            let mut total_gain_db = 0.0_f32;
            for node in &state.eq_nodes {
                if !node.is_active { continue; }
                let oct_dist = (f / node.freq_hz).log2();
                let bell_falloff = (-oct_dist.powi(2) * node.q * 1.8).exp();
                total_gain_db += node.gain_db * bell_falloff;
            }

            let x = canvas_rect.min.x + norm * canvas_rect.width();
            let y = mid_y - (total_gain_db.clamp(-14.0, 14.0) / 12.0) * (canvas_h * 0.42);
            curve_pts.push(Pos2::new(x, y));
        }

        // Draw EQ curve polygon & stroke
        for i in 0..curve_pts.len().saturating_sub(1) {
            painter.line_segment([curve_pts[i], curve_pts[i + 1]], Stroke::new(2.2_f32, crate::ui::theme::palette().cyan()));
        }

        // Draw interactive nodes & handle dragging
        let mut hovered_node = None;
        if let Some(m_pos) = canvas_resp.hover_pos() {
            for (idx, node) in state.eq_nodes.iter().enumerate() {
                let node_x = freq_to_x(node.freq_hz);
                let node_y = mid_y - (node.gain_db / 12.0) * (canvas_h * 0.42);
                let dist = m_pos.distance(Pos2::new(node_x, node_y));
                if dist < 18.0 {
                    hovered_node = Some(idx);
                }
            }
        }

        if canvas_resp.dragged()
            && let Some(m_pos) = canvas_resp.hover_pos()
                && let Some(idx) = hovered_node {
                    let norm_x = ((m_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.01, 0.99);
                    let target_freq = 10.0_f32.powf(20.0_f32.log10() + norm_x * (20000.0_f32.log10() - 20.0_f32.log10()));
                    let norm_y = (mid_y - m_pos.y) / (canvas_h * 0.42);
                    let target_db = (norm_y * 12.0).clamp(-12.0, 12.0);

                    state.eq_nodes[idx].freq_hz = target_freq;
                    state.eq_nodes[idx].gain_db = target_db;
                }

        for (idx, node) in state.eq_nodes.iter().enumerate() {
            let node_x = freq_to_x(node.freq_hz);
            let node_y = mid_y - (node.gain_db / 12.0) * (canvas_h * 0.42);
            let n_pos = Pos2::new(node_x, node_y);
            let is_h = hovered_node == Some(idx);

            painter.circle_filled(n_pos, if is_h { 9.0 } else { 7.0 }, node.color);
            painter.circle_stroke(n_pos, if is_h { 9.0 } else { 7.0 }, Stroke::new(1.5_f32, crate::ui::theme::palette().text()));
            painter.text(n_pos + Vec2::new(0.0, -14.0), egui::Align2::CENTER_BOTTOM, format!("{:.0}Hz ({:+.1}dB)", node.freq_hz, node.gain_db), egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().text());
        }

        ui.add_space(8.0);

        // Node Parameter controls row
        ui.horizontal(|ui| {
            for (idx, node) in state.eq_nodes.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut node.is_active, "");
                            ui.label(egui::RichText::new(format!("{}. {}", idx + 1, node.name)).strong().size(Theme::FONT_S).color(node.color));
                        });
                        ui.horizontal(|ui| {
                            rotary_knob(ui, &mut node.gain_db, -12.0, 12.0, "GAIN dB", node.color, 16.0);
                            rotary_knob(ui, &mut node.freq_hz, 20.0, 20000.0, "FREQ", node.color, 16.0);
                            rotary_knob(ui, &mut node.q, 0.3, 4.0, crate::i18n::t("Q-FAKTOR"), node.color, 16.0);
                        });
                    });
                });
            }
        });
    });
}

// ============================================================================
// CATEGORIZED EFFECTS LIBRARY DRAWER
// ============================================================================
fn render_effects_library_drawer(ui: &mut Ui, state: &mut FxRackState, status_msg: &mut String) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🗂 EFFEKTBIBLIOTEK (EFFECTS LIBRARY)")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().yellow()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Klicka på en effekt för att lägga till eller aktivera den i din aktiva kedja")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        });

        ui.add_space(4.0);

        // Search bar & Category filter chips
        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("🔍"));
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text(crate::i18n::t("Sök effekt (t.ex. Overdrive, EQ, Reverb, Chorus, Vocals...)"))
                    .desired_width(280.0),
            );

            if !state.search_query.is_empty() && ui.button(crate::i18n::t("✕")).clicked() {
                state.search_query.clear();
            }
        });

        ui.add_space(4.0);

        // Filter chips bar
        ui.horizontal_wrapped(|ui| {
            let categories = [
                "Alla", "Clean Up", "Glue Mix", "Enhance", "Sound Design",
                "Drums", "Bass", "Distortion", "Vocals", "Loudness", "Echo & Delay",
            ];

            for &cat in &categories {
                let is_sel = state.selected_category == cat;
                let chip_bg = if is_sel { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().grid() };
                let text_col = if is_sel { Color32::BLACK } else { crate::ui::theme::palette().text() };

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(cat)).size(Theme::FONT_S).color(text_col)).fill(chip_bg)).clicked() {
                    state.selected_category = cat;
                }
            }
        });

        ui.separator();
        ui.add_space(4.0);

        // Grid of available library items
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                let query = state.search_query.to_lowercase();
                let sel_cat = state.selected_category;

                egui::Grid::new("fx_lib_grid").spacing([10.0, 10.0]).show(ui, |ui| {
                    let mut col_count = 0;

                    for pedal in state.pedals.iter_mut() {
                        let matches_cat = sel_cat == "Alla" || pedal.category == sel_cat;
                        let matches_search = query.is_empty()
                            || pedal.name.to_lowercase().contains(&query)
                            || pedal.category.to_lowercase().contains(&query)
                            || pedal.description.to_lowercase().contains(&query);

                        if matches_cat && matches_search {
                            let card_w = 260.0;
                            let card_h = 75.0;
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());

                            ui.painter().rect_filled(rect, Rounding::same(6.0), crate::ui::theme::palette().header());
                            ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

                            let left_stripe = Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height()));
                            ui.painter().rect_filled(left_stripe, Rounding::same(2.0), pedal.color);

                            let mut card_ui = ui.new_child(
                                egui::UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(rect.min.x + 10.0, rect.min.y + 6.0), rect.max - Vec2::new(8.0, 6.0)))
                            );

                            card_ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(pedal.name).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.label(egui::RichText::new(crate::i18n::t(pedal.description)).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                                    ui.label(egui::RichText::new(pedal.category).size(Theme::FONT_XS).color(pedal.color));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let btn_txt = if pedal.enabled { crate::i18n::t("✔ På") } else { crate::i18n::t("➕ Lägg till") };
                                    let btn_bg = if pedal.enabled { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().border_soft() };
                                    if ui.add(egui::Button::new(egui::RichText::new(btn_txt).size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(btn_bg)).clicked() {
                                        pedal.enabled = !pedal.enabled;
                                        *status_msg = crate::tstatus!("🎛 Ändrade status för {}", pedal.name);
                                    }
                                });
                            });

                            col_count += 1;
                            if col_count % 3 == 0 {
                                ui.end_row();
                            }
                        }
                    }
                });
            });
    });
}

/// Linjär omskalning för en impulsrespons. `shortcut:` rak interpolering — tillräcklig för
/// ett rums frekvensgång, men den kortar/sträcker rumslängden exakt så mycket som
/// frekvenserna säger, alltså inte en tonhöjdsbevarande sträckning.
fn resample_linear(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 || from == to || x.is_empty() {
        return x.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((x.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = src.floor() as usize;
        let frac = (src - i0 as f64) as f32;
        let a = x[i0.min(x.len() - 1)];
        let b = x[(i0 + 1).min(x.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Granulizerns rattar når sina egna parametrar.** Fem rattar kan bytas om varandra
    /// utan att något låter fel i bygget — bara i ljudet. Därför mäts mappningen.
    #[test]
    fn the_granulizer_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "granular")
            .expect("granulizerpedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // kortaste kornet
        pedal.p2_val = 1.0; // tätast
        pedal.p3_val = 1.0; // längst bak
        pedal.extra[0].1 = 0.0; // ingen spridning
        pedal.extra[1].1 = 0.5; // hastighet 1,0
        pedal.mix = 0.25;

        let params = state.build_master_fx_params();
        assert!(params.granular_enabled, "pedalen ska slå på steget");
        assert!((params.granular.size_ms - 10.0).abs() < 0.01, "korn: {}", params.granular.size_ms);
        assert!((params.granular.density - 60.0).abs() < 0.01, "täthet: {}", params.granular.density);
        assert!(
            (params.granular.position_ms - 1000.0).abs() < 0.01,
            "position: {}",
            params.granular.position_ms
        );
        assert!((params.granular.spray_ms - 0.0).abs() < 0.01, "spridning: {}", params.granular.spray_ms);
        assert!((params.granular.mix - 0.25).abs() < 0.01, "mix: {}", params.granular.mix);
        assert!((params.granular.speed - 1.0).abs() < 0.01, "hastighet: {}", params.granular.speed);
    }
    /// **Frekvensskiftets rattar når sina egna fält** — magnitud, riktning och stereoläge är
    /// tre rattar som alla kan bytas om varandra utan att bygget klagar.
    #[test]
    fn the_frequency_shifter_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "freqshift")
            .expect("frekvensskiftet ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 1.0; // fullt skift
        pedal.p2_val = 0.0; // nedåt
        pedal.p3_val = 1.0; // stereoläge
        pedal.mix = 0.75;

        let params = state.build_master_fx_params();
        assert!(params.freq_shift_enabled, "pedalen ska slå på steget");
        assert!(
            (params.freq_shift.hz + 2000.0).abs() < 0.01,
            "skift: {}",
            params.freq_shift.hz
        );
        assert!((params.freq_shift.mix - 0.75).abs() < 0.01);
        assert!(params.freq_shift.spread, "stereoläget ska följa ratten");
    }

    /// **Krossens tre rattar når sina egna fält** — bitdjup, nedblandning och ton.
    #[test]
    fn the_bit_crusher_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "bitcrush")
            .expect("squeeze-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // 1 bit
        pedal.p2_val = 1.0; // 32 steg
        pedal.p3_val = 0.0; // stängt lågpass
        pedal.mix = 1.0;

        let params = state.build_master_fx_params();
        assert!(params.crush_enabled, "pedalen ska slå på steget");
        assert!((params.crush.bits - 1.0).abs() < 0.01, "bitar: {}", params.crush.bits);
        assert!(
            (params.crush.downsample - 32.0).abs() < 0.01,
            "nedblandning: {}",
            params.crush.downsample
        );
        assert!((params.crush.tone - 0.0).abs() < 0.01, "ton: {}", params.crush.tone);
    }

    /// **Multiband-delayns rattar når sina egna fält** — tre tider och en mix.
    #[test]
    fn the_multiband_delay_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "mbdelay")
            .expect("multiband-delayn ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // 20 ms
        pedal.p2_val = 0.5; // 410 ms
        pedal.p3_val = 1.0; // 800 ms
        pedal.mix = 0.5;

        let params = state.build_master_fx_params();
        assert!(params.mb_delay_enabled, "pedalen ska slå på steget");
        let t = params.mb_delay.times_ms;
        assert!((t[0] - 20.0).abs() < 0.1 && (t[1] - 410.0).abs() < 0.1 && (t[2] - 800.0).abs() < 0.1, "tider: {t:?}");
        assert!((params.mb_delay.mix - 0.5).abs() < 0.01);
    }

    /// **Delay-bankens rattar når sina egna fält** — tre tider, två nivåer, återkoppling,
    /// spridning och mix.
    #[test]
    fn the_delay_bank_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "delaybank")
            .expect("delay-banken ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // 20 ms
        pedal.p2_val = 0.5; // 760 ms
        pedal.p3_val = 1.0; // 1500 ms
        pedal.extra[0].1 = 0.25; // nivå 2
        pedal.extra[1].1 = 0.10; // nivå 3
        pedal.extra[2].1 = 0.80; // återkoppling
        pedal.extra[3].1 = 0.00; // spridning
        pedal.mix = 0.4;

        let params = state.build_master_fx_params();
        assert!(params.delay_bank_enabled, "pedalen ska slå på steget");
        let t = params.delay_bank.times_ms;
        assert!((t[0] - 20.0).abs() < 0.1 && (t[1] - 760.0).abs() < 0.1 && (t[2] - 1500.0).abs() < 0.1, "tider: {t:?}");
        assert!((params.delay_bank.levels[1] - 0.25).abs() < 0.01);
        assert!((params.delay_bank.feedback - 0.80).abs() < 0.01);
        assert!(params.delay_bank.spread.abs() < 0.01, "spridning: {}", params.delay_bank.spread);
        assert!((params.delay_bank.mix - 0.4).abs() < 0.01);
    }

    /// **Distructors rattar når sina egna fält** — tre drivar, kopplingen, tonen och mixen.
    #[test]
    fn the_distructor_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "distructor")
            .expect("distructor-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 1.0;
        pedal.p2_val = 0.5;
        pedal.p3_val = 0.0;
        pedal.extra[0].1 = 1.0; // parallellt
        pedal.extra[1].1 = 0.0; // lägsta ton
        pedal.mix = 1.0;

        let params = state.build_master_fx_params();
        assert!(params.distructor_enabled, "pedalen ska slå på steget");
        assert!((params.distructor.drive[0] - 1.0).abs() < 0.01);
        assert!((params.distructor.drive[1] - 0.5).abs() < 0.01);
        assert!(params.distructor.drive[2].abs() < 0.01);
        assert!(params.distructor.parallel > 0.99, "kopplingen: {}", params.distructor.parallel);
        assert!(
            (params.distructor.tone_hz - 300.0).abs() < 1.0,
            "tonen ska gå ner till 300 Hz: {}",
            params.distructor.tone_hz
        );
    }

    /// **Hardcores rattar når sina egna steg** — komp, drive, wah-centrum, wah-djup, delay och ekon.
    #[test]
    fn the_hardcore_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "hardcore")
            .expect("hardcore-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 1.0; // full komp
        pedal.p2_val = 0.0; // rent
        pedal.p3_val = 0.0; // lägsta wah
        pedal.extra[0].1 = 1.0;
        pedal.extra[1].1 = 0.0;
        pedal.extra[2].1 = 1.0;
        pedal.mix = 0.9;

        let params = state.build_master_fx_params();
        assert!(params.hardcore_enabled, "pedalen ska slå på kedjan");
        assert!((params.hardcore.comp - 1.0).abs() < 0.01);
        assert!(params.hardcore.drive.abs() < 0.01);
        assert!((params.hardcore.wah_hz - 300.0).abs() < 1.0, "wah: {}", params.hardcore.wah_hz);
        assert!((params.hardcore.wah_depth - 1.0).abs() < 0.01);
        assert!((params.hardcore.delay_ms - 40.0).abs() < 0.1, "delay: {}", params.hardcore.delay_ms);
        assert!((params.hardcore.delay_mix - 1.0).abs() < 0.01);
        assert!((params.hardcore.mix - 0.9).abs() < 0.01);
    }

    /// **Effectors tre rattar når sina egna fält** — X, Y och läget.
    #[test]
    fn the_effector_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "effector")
            .expect("effector-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.25;
        pedal.p2_val = 0.75;
        pedal.p3_val = 1.0; // högsta läget
        pedal.mix = 0.5;

        let params = state.build_master_fx_params();
        assert!(params.effector_enabled, "pedalen ska slå på steget");
        assert!((params.effector.x - 0.25).abs() < 0.01);
        assert!((params.effector.y - 0.75).abs() < 0.01);
        assert_eq!(params.effector.mode, 3, "läget ska bli det högsta");
    }

    /// **Love Philters sex rattar når sina egna fält** — CUTOFF, RESON, ENV, SPREAD,
    /// ATTACK och RELEASE. Rattar som byter plats låter fel utan att bygget klagar.
    #[test]
    fn the_love_philter_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "love_philter")
            .expect("love philter-pedalen ska finnas i racket");
        assert!(!state.build_master_fx_params().philter.enabled, "avstängd som standard");

        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 1.0; // högsta CUTOFF
        pedal.p2_val = 1.0; // högsta RESON
        pedal.p3_val = 0.5; // ENV halva vägen = 2 oktaver
        pedal.extra[0].1 = 0.0; // banden på varandra
        pedal.extra[1].1 = 0.0; // snabbaste attack
        pedal.extra[2].1 = 0.0; // kortaste release
        pedal.mix = 1.0;

        let p = state.build_master_fx_params().philter;
        assert!(p.enabled, "pedalen ska slå på steget");
        assert!((p.cutoff - 8000.0).abs() < 1.0, "cutoff: {}", p.cutoff);
        assert!((p.resonance - 8.0).abs() < 0.1, "resonans: {}", p.resonance);
        assert!((p.env_octaves - 2.0).abs() < 0.01, "env: {}", p.env_octaves);
        assert!(p.spread.abs() < 0.01, "spread: {}", p.spread);
        assert!(p.attack_ms < 1.0 && p.release_ms < 6.0, "tider: {} / {}", p.attack_ms, p.release_ms);
        assert!((p.mix - 1.0).abs() < 0.01);
    }

    /// **Distructor-pedalen slår på distorsionen, inte på de-essern.** De två delade
    /// variabelnamn i mappningen, så distructorns flagga hamnade på båda stegen.
    #[test]
    fn the_distructor_pedal_does_not_turn_on_the_de_esser() {
        let mut state = FxRackState::default();
        for pedal in &mut state.pedals {
            pedal.enabled = false;
        }
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "distructor")
            .expect("distructor-pedalen ska finnas i racket");
        state.pedals[idx].enabled = true;

        let params = state.build_master_fx_params();
        assert!(params.distructor_enabled, "distructorn ska vara på");
        assert!(!params.deesser_enabled, "de-essern ska vara av");
    }

    /// **Luxeverbs sju rattar når sina egna fält** — DECAY, SIZE, DAMP, PREDELAY, MOD,
    /// WIDTH och MIX.
    #[test]
    fn the_luxeverb_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "luxeverb")
            .expect("luxeverb-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // kortaste svansen
        pedal.p2_val = 1.0; // största rummet
        pedal.p3_val = 1.0; // mörkast
        pedal.extra[0].1 = 1.0; // längsta predelay
        pedal.extra[1].1 = 0.0; // ingen modulering
        pedal.extra[2].1 = 0.0; // mono
        pedal.mix = 0.5;

        let p = state.build_master_fx_params().luxeverb;
        assert!(p.enabled, "pedalen ska slå på steget");
        assert!((p.decay_s - 0.3).abs() < 0.01, "decay: {}", p.decay_s);
        assert!((p.size - 2.0).abs() < 0.01, "size: {}", p.size);
        assert!((p.damping - 1.0).abs() < 0.01);
        assert!((p.predelay_ms - 200.0).abs() < 0.1, "predelay: {}", p.predelay_ms);
        assert!(p.modulation.abs() < 0.01);
        assert!(p.width.abs() < 0.01);
        assert!((p.mix - 0.5).abs() < 0.01);
    }
    /// **Emphasis-rattarna når sina egna fält** — FREQ (1–10 kHz log), DRIVE (1–10) och MIX.
    #[test]
    fn the_emphasis_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "emphasis")
            .expect("emphasis-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // 1 kHz
        pedal.p2_val = 0.0; // drive 1
        pedal.mix = 1.0;

        let p = state.build_master_fx_params().emphasis;
        assert!(p.enabled, "pedalen ska slå på steget");
        assert!((p.freq_hz - 1_000.0).abs() < 1.0, "freq: {}", p.freq_hz);
        assert!((p.drive - 1.0).abs() < 0.01, "drive: {}", p.drive);
        assert!((p.mix - 1.0).abs() < 0.01);

        state.pedals[idx].p1_val = 1.0; // 10 kHz
        state.pedals[idx].p2_val = 1.0; // drive 10
        let p = state.build_master_fx_params().emphasis;
        assert!((p.freq_hz - 10_000.0).abs() < 5.0, "freq: {}", p.freq_hz);
        assert!((p.drive - 10.0).abs() < 0.01, "drive: {}", p.drive);
    }

    /// **Hyper Chorus-rattarna når sina egna fält** — RATE, DEPTH, WIDTH och MIX.
    #[test]
    fn the_hyper_chorus_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state
            .pedals
            .iter()
            .position(|p| p.id == "hyper_chorus")
            .expect("hyper_chorus-pedalen ska finnas i racket");
        let pedal = &mut state.pedals[idx];
        pedal.enabled = true;
        pedal.p1_val = 0.0; // 0,05 Hz
        pedal.p2_val = 1.0; // fullt djup
        pedal.p3_val = 0.0; // monokompatibel
        pedal.mix = 0.75;

        let p = state.build_master_fx_params().hyper_chorus;
        assert!(p.enabled, "pedalen ska slå på steget");
        assert!((p.rate - 0.05).abs() < 1e-4, "rate: {}", p.rate);
        assert!((p.depth - 1.0).abs() < 1e-4);
        assert!(p.width.abs() < 1e-4);
        assert!((p.mix - 0.75).abs() < 1e-4);
    }

    /// **Tröskelrattarna når var sitt band** — mitten (0,5) är ingen avvikelse.
    #[test]
    fn the_multiband_threshold_knobs_move_their_own_band() {
        let mut state = FxRackState::default();
        let idx = state.pedals.iter().position(|p| p.id == "multiband").expect("pedalen finns");
        let pedal = &mut state.pedals[idx];
        pedal.extra[2].1 = 0.0; // low −20 dB
        pedal.extra[3].1 = 1.0; // mid +20 dB
        pedal.extra[4].1 = 0.5; // high oförändrat
        pedal.extra[0].1 = 0.5; // tröskel −18 dB
        pedal.extra[1].1 = 0.27; // ratio ≈ 4
        let p = state.build_master_fx_params().multiband;
        assert!((p.offsets_db[0] + 20.0).abs() < 0.01, "low: {}", p.offsets_db[0]);
        assert!((p.offsets_db[1] - 20.0).abs() < 0.01, "mid: {}", p.offsets_db[1]);
        assert!(p.offsets_db[2].abs() < 0.01, "high: {}", p.offsets_db[2]);
        assert!((p.threshold_db + 18.0).abs() < 0.01, "tröskel: {}", p.threshold_db);
        assert!((p.ratio - 3.97).abs() < 0.05, "ratio: {}", p.ratio);
        // MASTER-rattens eget fält: banden står på 0, så bara master kan sänka summan.
        assert!(p.master_amount.abs() < 1e-6, "master: {}", p.master_amount);
        state.pedals[idx].extra[5].1 = 1.0;
        assert!((state.build_master_fx_params().multiband.master_amount - 1.0).abs() < 1e-6);
    }
    /// **Time Curves rattar når sina egna fält** — FORM väljer kurva, TEMPO sätter takten.
    #[test]
    fn the_time_curve_knobs_map_to_their_own_params() {
        let mut state = FxRackState::default();
        let idx = state.pedals.iter().position(|p| p.id == "time_curve").expect("pedalen finns");
        {
            let pedal = &mut state.pedals[idx];
            pedal.enabled = true;
            pedal.p1_val = 1.0; // sista formen: halv fart
            pedal.p2_val = 1.0; // 180 BPM
        }
        let p = state.build_master_fx_params().time_curve;
        assert!(p.enabled);
        assert_eq!(p.curve, TimeShape::Half.curve(), "formen hamnade inte på halv fart");
        assert!((p.bpm - 180.0).abs() < 0.01, "tempo: {}", p.bpm);

        state.pedals[idx].p1_val = 0.0; // rak
        state.pedals[idx].p2_val = 0.0; // 60 BPM
        let p = state.build_master_fx_params().time_curve;
        assert_eq!(p.curve, TimeShape::Flat.curve());
        assert!((p.bpm - 60.0).abs() < 0.01, "tempo: {}", p.bpm);
    }

    /// **Hela vägen: pedal → parametrar → ljud.** Ratten och DSP:n hade var sin test; det här
    /// prövar kopplingen mellan dem — att en påslagen Time Curve i racket faktiskt bromsar
    /// masterns ljud, inte bara fyller i ett fält ingen läser.
    #[test]
    fn an_enabled_time_curve_in_the_rack_halves_the_pitch_of_the_master() {
        let mut state = FxRackState::default();
        let idx = state.pedals.iter().position(|p| p.id == "time_curve").expect("pedalen finns");
        {
            let pedal = &mut state.pedals[idx];
            pedal.enabled = true;
            pedal.p1_val = 1.0; // halv fart
            pedal.p2_val = 0.5; // 120 BPM
            pedal.mix = 1.0;
        }
        let params = state.build_master_fx_params();
        let mut chain = crate::audio::MasterFxChain::new(48_000.0);
        chain.set_params(params);
        let sr = 48_000.0_f32;
        let mut out = Vec::with_capacity(24_000);
        for i in 0..24_000 {
            let t = i as f32 / sr;
            let x = 0.5 * (std::f32::consts::TAU * 220.0 * t).sin();
            let (l, _) = chain.process(x, x);
            out.push(l);
        }
        // Frekvensen i slutet av sekunden: kurvan behöver en stund på sig att bromsa in.
        let tail = &out[12_000..];
        let mut best = (0.0_f32, 0.0_f32);
        for hz in 80..160_u32 {
            let f = hz as f32;
            let (mut re, mut im) = (0.0_f32, 0.0_f32);
            for (i, v) in tail.iter().enumerate() {
                let ph = std::f32::consts::TAU * f * i as f32 / sr;
                re += v * ph.cos();
                im -= v * ph.sin();
            }
            let mag = (re * re + im * im).sqrt();
            if mag > best.1 {
                best = (f, mag);
            }
        }
        assert!(
            (best.0 - 110.0).abs() < 12.0,
            "masterkedjan spelade {:.0} Hz, väntade 110 (halv fart)", best.0
        );
    }

}
