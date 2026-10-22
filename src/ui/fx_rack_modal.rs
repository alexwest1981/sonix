use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::{
    AudioCommand, AudioEngine, CompressorParams, DeEsserParams, DoublerParams, GateParams,
    ConvolverParams, GranularParams, LimiterParams, MasterEqBand, MasterFilterParams, MasterFxParams,
    ModFxParams, MultibandParams, ReverbParams, SaturatorParams, TransientParams,
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
    pub compressor_release_ms: f32,
    /// Convolverns torr/våt och nivå. Impulsresponsen själv ligger i motorn (den är för stor
    /// för att skickas varje bildruta) — här finns bara namnet, för att veta om en är laddad.
    pub convolver_mix: f32,
    pub convolver_gain_db: f32,
    pub convolver_ir_name: Option<String>,
}

impl Default for FxRackState {
    fn default() -> Self {
        Self {
            pedals: vec![
                FxPedal {
                    id: "visual_eq",
                    name: "📊 VISUAL EQ",
                    category: "Clean Up",
                    enabled: true,
                    color: Color32::from_rgb(0, 200, 240),
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
                    color: Color32::from_rgb(255, 140, 0),
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
                    color: Color32::from_rgb(180, 100, 255),
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
                    color: Color32::from_rgb(60, 180, 255),
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
                    color: Color32::from_rgb(120, 240, 100),
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
                    color: Color32::from_rgb(255, 80, 120),
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
                    name: "📶 MULTIBAND COMP (3 band)",
                    category: "Glue Mix",
                    enabled: false,
                    color: Color32::from_rgb(120, 200, 255),
                    p1_name: "LOW",
                    p1_val: 0.0,
                    p2_name: "MID",
                    p2_val: 0.0,
                    p3_name: "HIGH",
                    p3_val: 0.0,
                    mix: 1.0,
                    description: "Tre band (250 Hz / 4 kHz, Linkwitz-Riley) med egen kompression per band.",
                    extra: Vec::new(),
                },
                FxPedal {
                    id: "transient",
                    name: "⚡ TRANSIENT SHAPER",
                    category: "Dynamics",
                    enabled: false,
                    color: Color32::from_rgb(255, 200, 60),
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
                    color: Color32::from_rgb(255, 120, 60),
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
                    color: Color32::from_rgb(200, 255, 120),
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
                    id: "chorus",
                    name: "🌊 CHORUS",
                    category: "Sound Design",
                    enabled: false,
                    color: Color32::from_rgb(90, 180, 255),
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
                    color: Color32::from_rgb(120, 255, 210),
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
                    color: Color32::from_rgb(180, 140, 255),
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
                    id: "analog_filter",
                    name: "🎛 SVF FILTER (12 dB)",
                    category: "Sound Design",
                    enabled: false,
                    color: Color32::from_rgb(240, 210, 40),
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
                    id: "brickwall_limiter",
                    name: "🛡 MASTER LIMITER",
                    category: "Loudness",
                    enabled: true,
                    color: Color32::from_rgb(255, 50, 50),
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
            search_query: String::new(),
            selected_category: "Alla",
            eq_nodes: vec![
                VisualEqNode { name: "Low (Bass)", freq_hz: 120.0, gain_db: 2.0, q: 0.7, color: Color32::from_rgb(255, 100, 100), is_active: true },
                VisualEqNode { name: "Low-Mid", freq_hz: 450.0, gain_db: -1.5, q: 1.2, color: Color32::from_rgb(255, 180, 50), is_active: true },
                VisualEqNode { name: "High-Mid", freq_hz: 2800.0, gain_db: 3.0, q: 1.0, color: Color32::from_rgb(80, 220, 120), is_active: true },
                VisualEqNode { name: "High (Air)", freq_hz: 11000.0, gain_db: 1.5, q: 0.8, color: Color32::from_rgb(0, 200, 255), is_active: true },
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
        let mut eq_bands = [MasterEqBand::flat(); 4];
        for (i, node) in self.eq_nodes.iter().take(4).enumerate() {
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
        let multiband = MultibandParams {
            amounts: [
                mb_p.map(|p| p.p1_val).unwrap_or(0.0),
                mb_p.map(|p| p.p2_val).unwrap_or(0.0),
                mb_p.map(|p| p.p3_val).unwrap_or(0.0),
            ],
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
            granular,
            granular_enabled: gr_p.map(|p| p.enabled).unwrap_or(false),
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
        let target = engine.sample_rate as u32;
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
        .default_size(Vec2::new(880.0, 620.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // ============================================================
                // 1. TOP HEADER & NAVIGATION TABS
                // ============================================================
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎛 SOUNDTRAP FX RACK")).strong().size(15.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("Modulär effektkedja  •  Visual Parametric EQ  •  Dynamisk kompressor  •  Vocal Doubler")).size(11.0).color(Theme::TEXT_MUTED));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let lib_btn = ui.selectable_label(state.active_tab == 2, crate::i18n::t("🗂 Effektbibliotek"));
                        if lib_btn.clicked() { state.active_tab = 2; }

                        let eq_btn = ui.selectable_label(state.active_tab == 1, "📊 Visual EQ");
                        if eq_btn.clicked() { state.active_tab = 1; }

                        let rack_btn = ui.selectable_label(state.active_tab == 0, crate::i18n::t("🎛 Effektkedja"));
                        if rack_btn.clicked() { state.active_tab = 0; }
                    });
                });

                ui.add_space(4.0);

                // Presets toolbar
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Kedjemallar (Presets):")).strong().color(Theme::FL_CYAN));
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
                            ui.label(egui::RichText::new(crate::tstatus!("{} aktiva effekter", enabled_count)).size(10.5).color(Theme::FL_GREEN));
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
                                .color(Theme::FL_CYAN),
                        );
                        let name = state
                            .convolver_ir_name
                            .clone()
                            .unwrap_or_else(|| crate::i18n::t("ingen fil laddad").to_string());
                        ui.label(egui::RichText::new(name).size(10.0).color(Theme::TEXT_MUTED));
                        if ui.button(crate::i18n::t("📥 Ladda .wav")).clicked() {
                            if let Some(msg) = state.load_convolver_ir(engine) {
                                *status_msg = msg;
                            }
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
                                .size(10.0)
                                .color(Theme::TEXT_MUTED),
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
                            .fill(Theme::FL_GREEN)
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
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);

                for pedal in pedals.iter_mut() {
                    let card_w = 175.0;
                    let card_h = 360.0;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());

                    let card_bg = if pedal.enabled {
                        Color32::from_rgb(22, 26, 35)
                    } else {
                        Color32::from_rgb(15, 17, 22)
                    };

                    ui.painter().rect_filled(rect, Rounding::same(8.0), card_bg);
                    ui.painter().rect_stroke(
                        rect,
                        Rounding::same(8.0),
                        Stroke::new(1.5_f32, if pedal.enabled { pedal.color } else { Color32::from_rgb(40, 46, 56) }),
                    );

                    // Top Header Strip
                    let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 26.0));
                    ui.painter().rect_filled(top_bar, Rounding::same(7.0), if pedal.enabled { pedal.color } else { Color32::from_rgb(30, 36, 46) });

                    ui.painter().text(
                        Pos2::new(rect.min.x + 8.0, rect.min.y + 13.0),
                        egui::Align2::LEFT_CENTER,
                        pedal.name,
                        egui::FontId::proportional(11.0),
                        if pedal.enabled { Color32::BLACK } else { Theme::TEXT_MUTED },
                    );

                    // Power LED
                    let led_pos = Pos2::new(rect.max.x - 12.0, rect.min.y + 13.0);
                    ui.painter().circle_filled(
                        led_pos,
                        4.5,
                        if pedal.enabled { Theme::FL_GREEN } else { Color32::from_rgb(80, 20, 20) },
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
                            ui.painter().rect_filled(eq_box, Rounding::same(4.0), Color32::from_rgb(12, 16, 24));
                            ui.painter().rect_stroke(eq_box, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(30, 42, 58)));
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
                            ui.painter().rect_filled(gr_box, Rounding::same(2.0), Color32::from_rgb(12, 16, 24));
                            // Real gain reduction reported by the audio thread (0..-20 dB).
                            let gr_frac = (master_gr_db.abs() / 20.0).clamp(0.0, 1.0);
                            let fill_w = gr_box.width() * gr_frac;
                            ui.painter().rect_filled(Rect::from_min_size(gr_box.min, Vec2::new(fill_w, gr_box.height())), Rounding::same(2.0), Theme::FL_ORANGE);
                            let gr_txt = if master_gr_db <= -0.05 { format!("GR {:.1} dB", master_gr_db) } else { "GR".to_string() };
                            ui.painter().text(gr_box.center(), egui::Align2::CENTER_CENTER, gr_txt, egui::FontId::proportional(8.5), Color32::BLACK);
                        } else if pedal.id == "vocal_doubler" {
                            let (orb_box, _) = ui.allocate_exact_size(Vec2::new(card_w - 20.0, 45.0), Sense::hover());
                            ui.painter().rect_filled(orb_box, Rounding::same(4.0), Color32::from_rgb(12, 16, 24));
                            let c1 = Pos2::new(orb_box.center().x - 14.0, orb_box.center().y);
                            let c2 = Pos2::new(orb_box.center().x + 14.0, orb_box.center().y);
                            ui.painter().circle_stroke(c1, 16.0, Stroke::new(1.5_f32, pedal.color));
                            ui.painter().circle_stroke(c2, 16.0, Stroke::new(1.5_f32, Color32::from_rgb(120, 180, 255)));
                        }

                        ui.add_space(6.0);

                        if pedal.id == "visual_eq" {
                            // Knobs drive the real 4-band EQ nodes (same state as
                            // the Visual EQ tab).
                            let short = ["LOW", "L-MID", "H-MID", "HIGH"];
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
                            rotary_knob(ui, &mut pedal.mix, 0.0, 1.0, "MIX", Color32::WHITE, 18.0);
                        }

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(pedal.category).size(9.0).color(Theme::TEXT_MUTED));
                    });
                }

                // Add Card (+) at end of chain
                let add_w = 140.0;
                let add_h = 360.0;
                let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(add_w, add_h), Sense::click());
                ui.painter().rect_filled(add_rect, Rounding::same(8.0), Color32::from_rgb(18, 22, 30));
                ui.painter().rect_stroke(add_rect, Rounding::same(8.0), Stroke::new(1.0_f32, Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 75)).color));
                ui.painter().text(add_rect.center() - Vec2::new(0.0, 15.0), egui::Align2::CENTER_CENTER, "➕", egui::FontId::proportional(26.0), Theme::FL_CYAN);
                ui.painter().text(add_rect.center() + Vec2::new(0.0, 18.0), egui::Align2::CENTER_CENTER, crate::i18n::t("Lägg till från\nEffektbiblioteket"), egui::FontId::proportional(11.0), Theme::TEXT_BRIGHT);

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
            ui.label(egui::RichText::new(crate::i18n::t("📊 4-BANDS PARAMETRISK EQUALIZER")).strong().size(13.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Dra i noderna för att justera frekvens och förstärkning (dB)")).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(4.0);

        let canvas_w = ui.available_width().max(300.0);
        let canvas_h = 240.0;
        let (canvas_rect, canvas_resp) = ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), Sense::click_and_drag());
        let painter = ui.painter();

        // Canvas Background & Grid
        painter.rect_filled(canvas_rect, Rounding::same(6.0), Color32::from_rgb(12, 16, 24));
        painter.rect_stroke(canvas_rect, Rounding::same(6.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 44, 60)));

        let mid_y = canvas_rect.center().y;

        // Draw dB horizontal grid lines (+12, +6, 0, -6, -12)
        for db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
            let y = mid_y - (db / 12.0) * (canvas_h * 0.42);
            let stroke_col = if db == 0.0 { Color32::from_rgb(50, 70, 95) } else { Color32::from_rgb(22, 30, 42) };
            painter.line_segment([Pos2::new(canvas_rect.min.x, y), Pos2::new(canvas_rect.max.x, y)], Stroke::new(1.0_f32, stroke_col));
            painter.text(Pos2::new(canvas_rect.min.x + 6.0, y - 2.0), egui::Align2::LEFT_BOTTOM, format!("{:+1.0} dB", db), egui::FontId::proportional(8.5), Color32::from_rgb(80, 100, 125));
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
            painter.line_segment([Pos2::new(x, canvas_rect.min.y), Pos2::new(x, canvas_rect.max.y)], Stroke::new(0.8_f32, Color32::from_rgb(20, 28, 40)));
            let label = if f >= 1000.0 { format!("{:.0}k", f / 1000.0) } else { format!("{:.0}", f) };
            painter.text(Pos2::new(x, canvas_rect.max.y - 4.0), egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(8.0), Color32::from_rgb(70, 85, 105));
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
            painter.line_segment([curve_pts[i], curve_pts[i + 1]], Stroke::new(2.2_f32, Theme::FL_CYAN));
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

        if canvas_resp.dragged() {
            if let Some(m_pos) = canvas_resp.hover_pos() {
                if let Some(idx) = hovered_node {
                    let norm_x = ((m_pos.x - canvas_rect.min.x) / canvas_rect.width()).clamp(0.01, 0.99);
                    let target_freq = 10.0_f32.powf(20.0_f32.log10() + norm_x * (20000.0_f32.log10() - 20.0_f32.log10()));
                    let norm_y = (mid_y - m_pos.y) / (canvas_h * 0.42);
                    let target_db = (norm_y * 12.0).clamp(-12.0, 12.0);

                    state.eq_nodes[idx].freq_hz = target_freq;
                    state.eq_nodes[idx].gain_db = target_db;
                }
            }
        }

        for (idx, node) in state.eq_nodes.iter().enumerate() {
            let node_x = freq_to_x(node.freq_hz);
            let node_y = mid_y - (node.gain_db / 12.0) * (canvas_h * 0.42);
            let n_pos = Pos2::new(node_x, node_y);
            let is_h = hovered_node == Some(idx);

            painter.circle_filled(n_pos, if is_h { 9.0 } else { 7.0 }, node.color);
            painter.circle_stroke(n_pos, if is_h { 9.0 } else { 7.0 }, Stroke::new(1.5_f32, Color32::WHITE));
            painter.text(n_pos + Vec2::new(0.0, -14.0), egui::Align2::CENTER_BOTTOM, format!("{:.0}Hz ({:+.1}dB)", node.freq_hz, node.gain_db), egui::FontId::proportional(9.0), Color32::WHITE);
        }

        ui.add_space(8.0);

        // Node Parameter controls row
        ui.horizontal(|ui| {
            for (idx, node) in state.eq_nodes.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut node.is_active, "");
                            ui.label(egui::RichText::new(format!("{}. {}", idx + 1, node.name)).strong().size(11.0).color(node.color));
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
            ui.label(egui::RichText::new(crate::i18n::t("🗂 EFFEKTBIBLIOTEK (EFFECTS LIBRARY)")).strong().size(13.0).color(Theme::FL_YELLOW));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Klicka på en effekt för att lägga till eller aktivera den i din aktiva kedja")).size(11.0).color(Theme::TEXT_MUTED));
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
                let chip_bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(28, 34, 46) };
                let text_col = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(cat)).size(10.5).color(text_col)).fill(chip_bg)).clicked() {
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

                            ui.painter().rect_filled(rect, Rounding::same(6.0), Color32::from_rgb(20, 24, 32));
                            ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, Color32::from_rgb(38, 48, 65)));

                            let left_stripe = Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height()));
                            ui.painter().rect_filled(left_stripe, Rounding::same(2.0), pedal.color);

                            let mut card_ui = ui.new_child(
                                egui::UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(rect.min.x + 10.0, rect.min.y + 6.0), rect.max - Vec2::new(8.0, 6.0)))
                            );

                            card_ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(pedal.name).strong().size(11.0).color(Theme::TEXT_BRIGHT));
                                    ui.label(egui::RichText::new(crate::i18n::t(pedal.description)).size(9.5).color(Theme::TEXT_MUTED));
                                    ui.label(egui::RichText::new(pedal.category).size(8.5).color(pedal.color));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let btn_txt = if pedal.enabled { crate::i18n::t("✔ På") } else { crate::i18n::t("➕ Lägg till") };
                                    let btn_bg = if pedal.enabled { Theme::FL_GREEN } else { Color32::from_rgb(45, 60, 85) };
                                    if ui.add(egui::Button::new(egui::RichText::new(btn_txt).size(10.0).color(Color32::WHITE)).fill(btn_bg)).clicked() {
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
}
