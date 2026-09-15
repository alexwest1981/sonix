//! Rösterna och spåren motorn spelar: synth, duckning, stämspår, audition, samplingar.
//! Status: stabil — rösterna och spåren; en ny rösttyp hör hit och får eget prov.
//! Rör inte: `slice_edge_gain` — `fade_frames = 0` ska ge exakt den gamla vägen (1,0 överallt), inte nästan; det är den som skiljer ett klick från ett klipp.

use super::*;

#[derive(Debug, Clone, Copy)]
pub struct Voice {
    pub note: u8,
    pub freq: f32,
    pub phase: f32,
    pub phase_inc: f32,
    pub velocity: f32,
    pub envelope: AdsrVoice,
    /// Per-voice filter state so simultaneously sounding notes do not share a
    /// single filter (each note keeps its own resonant state).
    pub filter: StateVariableFilter,
    /// Per-voice filter envelope; its level modulates the cutoff per note.
    pub filter_env: AdsrVoice,
}

/// Sidokedje-duckare (Fas 8.3).
///
/// **Varför en egen liten DSP och inte en kompressor:** en kompressor med extern
/// nyckel gör samma sak som att köra kompressorn på key-signalen och multiplicera
/// den egna signalen med dess gain reduction. Det här är samma matematik, men
/// utskriven: ett envelopföljande steg på key-signalen och en gain som går mot
/// `-amount_db` när nyckeln är över tröskeln. Tidkonstanterna är fasta (5 ms
/// attack, 120 ms release) — det är de värden en duckare brukar ha, och ett
/// reglage mindre att förklara.
///
/// **Nyckeln är som mest ett sample gammal.** Spårloopen går i indexordning, så
/// ett key-spår med högre index än målet har redan passerat förra samplet när
/// målet räknas. 20 µs vid 48 kHz, mot tidkonstanter i millisekunder — att kräva
/// samma sample hade krävt en omsortering av hela loopen per sample, och det
/// hade varit en mycket dyrare konstruktion för en omätbar skillnad.
#[derive(Clone, Copy, Debug)]
pub struct Ducker {
    /// Utjämnad nyckelnivå (linjär).
    env: f32,
    attack: f32,
    release: f32,
}

/// Var i spårets ljud en region befinner sig vid en given tid in i klossen
/// (Fas 8.10).
///
/// **Ren funktion**, för att kunna prövas utan ljudmotor — motorn anropar den en
/// gång per sample. Utan sträckning (`stretch_ratio == 1.0`) ger den exakt samma
/// svar som före 8.10: samma uttryck, bara med tiden flyttad till källans
/// tidslinje. Vid 1.0 är vägen alltså bit-exakt som förut.
///
/// `rel_time` är sekunder in i regionen (ut-tid), svaret sekunder in i spårets ljud
/// (käll-tid). En slingad kloss upprepar ett stycke: perioden är
/// `loop_length_secs` i ut-tid, och källan som hinns igenom per varv är lika lång
/// gånger sträckningen.
pub fn region_source_secs(region: &StemRegionPlayback, rel_time: f32) -> f32 {
    let rate = region.stretch_ratio.max(0.05);
    let moved = rel_time * rate;
    if region.loop_length_secs > 0.02 {
        let loop_src = region.loop_length_secs * rate;
        let phase = (region.sample_offset_sec + moved) % loop_src;
        if region.is_reverse {
            (loop_src - phase).max(0.0)
        } else {
            phase
        }
    } else if region.is_reverse {
        // Samma uttryck som före 8.10, med tiden sträckt. (Ett omvänt klipp med
        // `sample_offset_sec > 0` hamnar utanför sitt eget utsnitt — det beteendet
        // är oförändrat här och står som en egen punkt i roadmapen.)
        (region.length_secs * rate - (region.sample_offset_sec + moved)).max(0.0)
    } else {
        region.sample_offset_sec + moved
    }
}

pub struct StemVoiceTrack {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: f32,
    pub volume: f32,
    pub pan: f32,
    pub pan_l: f32,
    pub pan_r: f32,
    pub muted: bool,
    pub solo: bool,
    pub start_time_secs: f32,
    pub regions: Vec<StemRegionPlayback>,
    pub eq: TrackEqSettings,
    pub eq_proc: StereoEq,
    pub comp: Compressor,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb: SimpleReverb,
    pub reverb_send: f32,
    pub delay: StereoDelay,
    pub delay_send: f32,
    /// Requested transposition in semitones (±12, cents resolution).
    pub pitch_semitones: f32,
    /// Real-time, formant-preserving pitch shifter for this stem.
    pub pitch_shifter: FormantPitchShifter,
    /// Whether the shifter is engaged (false = bit-exact bypass).
    pub pitch_active: bool,
    /// Optional CLAP insert on this track (Fas 4.2).
    pub plugin: Option<PluginInsert>,
    /// **Manuellt latens-offset för spårets plugin, i ramar** (Fas 8.6). Negativt = spåret
    /// dras fram. Se `compensated_latency` — regeln bor där, inte här.
    pub manual_latency_frames: i32,
    /// Delay line that aligns this track with the project's max plugin latency.
    pub pdc: PdcDelay,
    /// Sub-mix bus this track feeds (`0..NUM_BUSES`) — Fas 5.2.
    pub bus: usize,
    /// Optional VCA control group (`0..NUM_VCAS`) — Fas 5.2.
    pub vca: Option<usize>,
    /// Sends (Fas 8.13): parallella vägar till andra bussar.
    pub sends: Vec<StemSend>,
    /// **Pluginens egna utbussar till egna spår** (Fas 8.6): `extra_out_targets[port]` är
    /// spåret som pluginens utbuss `port` matar — den *första egna* bussen, alltså CLAP-port
    /// 1; port 0 är pluginens huvudutgång och går alltid till spårets egen kedja. `None` =
    /// bussen läses inte alls, precis som före den här punkten.
    pub extra_out_targets: Vec<Option<usize>>,
    /// Sidokedja (Fas 8.3): spåret duckas av det här spårets ljud.
    pub sidechain_from: Option<usize>,
    /// Hur mycket spåret sänks när key-signalen är över tröskeln.
    pub sidechain_amount_db: f32,
    /// Tröskeln key-signalen måste över för att ducka.
    pub sidechain_threshold_db: f32,
    /// Duckarens tillstånd (en per spår — annars styr ett spår ett annat).
    pub sidechain_ducker: Ducker,
    /// Spårets senaste utgångssample, för sidokedjor som pekar hit.
    pub last_out_l: f32,
    pub last_out_r: f32,
}

#[derive(Clone)]
pub struct AuditionVoice {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: f32,
    pub volume: f32,
    pub pitch_ratio: f32,
    pub time_stretch_ratio: f32,
    pub is_reverse: bool,
    pub loop_playback: bool,
    pub play_pos_samples: f32,
    pub is_playing: bool,
    /// Pitch-preserving time-stretch state (used when `time_stretch_ratio != 1`
    /// and playback is forward).
    pub wsola: Wsola,
}

/// A polyphonic one-shot voice that streams a preloaded WAV sample
/// (Channel Rack steps, drum machines & melodic sample playback).
/// **Kantdämpningen vid en slicekant** (Fas 8.7 steg 2).
///
/// `frames_from_edge` är hur långt in i slicen vi är — eller hur många ramar som återstår till
/// kanten — och `fade_frames` är rampens längd. Svaret är 0 **vid** kanten och 1 en ramp in.
/// Samma regel i båda ändarna, för det är samma fenomen: ett klick är en språngvis förändring
/// av amplituden, och en ramp mot noll är motsatsen.
///
/// Ren funktion med egen provsvit. Den är för liten att gömma i renderingsloopen och för
/// viktig att lämna oprövad — det är den som skiljer ett klick från ett klipp.
///
/// `fade_frames = 0` ger **1,0 överallt**, alltså exakt beteendet innan rampen fanns. Det är
/// avsiktligt: en avstängd dämpning ska vara identisk med den gamla vägen, inte nästan.
pub fn slice_edge_gain(frames_from_edge: f32, fade_frames: f32) -> f32 {
    if !frames_from_edge.is_finite() || !fade_frames.is_finite() {
        return 1.0;
    }
    if fade_frames <= 0.0 {
        return 1.0;
    }
    (frames_from_edge / fade_frames).clamp(0.0, 1.0)
}

/// Längden på kantdämpningen vid en slicekant (Fas 8.7 steg 2): 2 ms, inom det spann
/// (1–5 ms) Reapers fade pad och Abeltons per-slice-fade använder.
pub const SLICE_FADE_SECS: f32 = 0.002;

pub struct SampleVoice {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub src_sample_rate: f32,
    /// Playback cursor in *source* sample frames.
    pub pos: f32,
    /// Sample-frames advanced per output frame (pitch ratio * resample).
    pub step: f32,
    /// Inclusive source-frame range this voice may play within (slice/chop).
    pub start_frame: f32,
    pub end_frame: f32,
    pub volume: f32,
    pub pan_l: f32,
    pub pan_r: f32,
    pub reverse: bool,
    /// **Kantdämpningens längd i utramar** (Fas 8.7 steg 2): rampen vid *båda* kanterna av
    /// fönstret. Starten har haft en sedan tidigare (anti-klick); slutet fick sin när en slice
    /// som tar slut mitt i en ton visade sig klicka. 2 ms ligger inom det Reaper och Ableton
    /// använder för sin fade pad (1–5 ms).
    pub fade_frames: u32,
    pub frames_done: u32,
    pub active: bool,
    /// **Kanalen rösten tillhör** (Fas 8.4) — not-av gäller en kanal i taget, och poolen
    /// har ingen annan identitet.
    pub channel: u32,
    /// **Samplerns loop** (Fas 8.4): läge, loopens spann i källramar (redan klämt och
    /// ordnat av `loop_frames` — `None` = ingen loop, alltså en-skottsbeteendet) och om
    /// loopen vänder i sina ändar.
    pub loop_mode: LoopMode,
    pub loop_span: Option<(f32, f32)>,
    pub ping_pong: bool,
    /// Riktningen **just nu** (1,0 framåt, −1,0 bakåt). `reverse` är startläget; den här
    /// ändras av ping-pong.
    pub dir: f32,
    /// Noten har släppts (not-av, eller efter notens längd): `UntilRelease` spelar då
    /// resten efter loopen i stället för att loopa.
    pub released: bool,
    /// **Notens längd i utramar** (Fas 8.4). `None` = ingen not-av alls (dagens
    /// en-skottsbeteende).
    pub hold_frames: Option<u32>,
    /// Amplitud-ADSR på rösten. `env_on` är falskt när parametrarna är **identiteten**
    /// (`AdsrParams::identity`), och då rörs envelopen inte alls — det är vad som gör ett
    /// projekt från före samplern byte-identiskt.
    pub amp_env: AdsrVoice,
    pub env_params: AdsrParams,
    pub env_on: bool,
    /// **Velociteten som gain** (Fas 8.4/7): räknad **en gång** vid triggen ur notens anslag och
    /// kanalens känslighet (`envelope::velocity_gain`). `1,0` = ingen påverkan. Den ligger på
    /// rösten och inte i `volume` därför att den ska gå att stänga av per kanal — och för att
    /// envelopen, som ligger efter den i samplevägen, skalas av anslaget.
    pub velocity_gain: f32,
}

/// A note waiting to be triggered by the scheduler (chord strum/arpeggio).
pub struct ScheduledNote {
    pub samples_until: u32,
    pub note: u8,
    pub freq: f32,
    pub velocity: f32,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            note: 0,
            freq: 440.0,
            phase: 0.0,
            phase_inc: 0.0,
            velocity: 0.0,
            envelope: AdsrVoice::new(sample_rate),
            filter: StateVariableFilter::new(sample_rate),
            filter_env: AdsrVoice::new(sample_rate),
        }
    }

    pub fn trigger(&mut self, note: u8, freq: f32, velocity: f32, sample_rate: f32) {
        self.note = note;
        self.freq = freq;
        self.velocity = velocity;
        self.phase_inc = (2.0 * PI * freq) / sample_rate;
        self.filter.reset();
        self.envelope.gate_on();
        self.filter_env.gate_on();
    }

    pub fn release(&mut self) {
        self.envelope.gate_off();
        self.filter_env.gate_off();
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.filter_env.reset();
        self.velocity = 0.0;
        self.phase = 0.0;
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.envelope.is_active()
    }

    #[inline(always)]
    pub fn next_sample(
        &mut self,
        waveform: Waveform,
        adsr: &AdsrParams,
        filter_params: &FilterParams,
        filter_env: &AdsrParams,
        filter_env_amount: f32,
    ) -> f32 {
        if !self.envelope.is_active() {
            return 0.0;
        }

        let env_gain = self.envelope.next_sample(adsr);
        if env_gain <= 0.0 {
            return 0.0;
        }

        let raw_sample = match waveform {
            Waveform::Sine => self.phase.sin(),
            Waveform::Saw => (self.phase / PI) - 1.0,
            Waveform::Square => {
                if self.phase < PI {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Triangle => {
                let normalized = self.phase / (2.0 * PI);
                2.0 * (2.0 * (normalized - (normalized + 0.5).floor())).abs() - 1.0
            }
        };

        // Advance phase
        self.phase += self.phase_inc;
        if self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }

        // Each voice runs through its own resonant filter instance, and its own
        // filter envelope modulates the cutoff (in octaves) independently.
        let fenv = self.filter_env.next_sample(filter_env);
        let effective = FilterParams {
            cutoff: filter_params.cutoff * 2.0_f32.powf(filter_env_amount * fenv),
            resonance: filter_params.resonance,
        };
        self.filter.process_lowpass(raw_sample * self.velocity * env_gain, &effective)
    }
}

impl Ducker {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            env: 0.0,
            attack: 1.0 - (-1.0 / (0.005 * sr)).exp(),
            release: 1.0 - (-1.0 / (0.120 * sr)).exp(),
        }
    }

    /// Lämnar gainen för det här samplet (1.0 = orörd).
    pub fn process(&mut self, key_l: f32, key_r: f32, threshold_db: f32, amount_db: f32) -> f32 {
        let peak = key_l.abs().max(key_r.abs());
        let coeff = if peak > self.env { self.attack } else { self.release };
        self.env += (peak - self.env) * coeff;
        let env_db = 20.0 * self.env.max(1e-6).log10();
        if env_db <= threshold_db {
            return 1.0;
        }
        let target = 10.0f32.powf(-amount_db.clamp(0.0, 60.0) / 20.0);
        // Mjuk övergång: 6 dB över tröskeln ger hela duckningen. Utan den hade
        // gränsen blivit ett hörbart knäpp.
        let over = ((env_db - threshold_db) / 6.0).clamp(0.0, 1.0);
        1.0 - (1.0 - target) * over
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }
}

impl StemVoiceTrack {
    pub fn new(
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pan: f32,
        start_time_secs: f32,
        engine_sample_rate: f32,
    ) -> Self {
        let p = pan.clamp(-1.0, 1.0);
        let pan_l = ((1.0 - p) * 0.5).sqrt();
        let pan_r = ((1.0 + p) * 0.5).sqrt();
        Self {
            extra_out_targets: Vec::new(),
            left,
            right,
            sample_rate,
            volume,
            pan: p,
            pan_l,
            pan_r,
            muted: false,
            solo: false,
            start_time_secs,
            regions: Vec::new(),
            eq: TrackEqSettings::default(),
            eq_proc: StereoEq::new(engine_sample_rate),
            comp: Compressor::new(engine_sample_rate),
            comp_threshold_db: 0.0,
            comp_ratio: 1.0,
            reverb: SimpleReverb::new(engine_sample_rate),
            reverb_send: 0.0,
            delay: StereoDelay::new(engine_sample_rate),
            delay_send: 0.0,
            pitch_semitones: 0.0,
            pitch_shifter: FormantPitchShifter::new(engine_sample_rate),
            pitch_active: false,
            plugin: None,
            manual_latency_frames: 0,
            pdc: PdcDelay::new(),
            bus: 0,
            vca: None,
            sends: Vec::new(),
            sidechain_from: None,
            sidechain_amount_db: 0.0,
            sidechain_threshold_db: -30.0,
            sidechain_ducker: Ducker::new(engine_sample_rate),
            last_out_l: 0.0,
            last_out_r: 0.0,
        }
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        self.pan_l = ((1.0 - self.pan) * 0.5).sqrt();
        self.pan_r = ((1.0 + self.pan) * 0.5).sqrt();
    }
}

impl SampleVoice {
    pub fn new() -> Self {
        Self {
            left: Arc::new(Vec::new()),
            right: Arc::new(Vec::new()),
            src_sample_rate: 44100.0,
            pos: 0.0,
            step: 1.0,
            start_frame: 0.0,
            end_frame: 0.0,
            volume: 1.0,
            pan_l: 1.0,
            pan_r: 1.0,
            reverse: false,
            fade_frames: 0,
            frames_done: 0,
            active: false,
            channel: 0,
            loop_mode: LoopMode::Off,
            loop_span: None,
            ping_pong: false,
            dir: 1.0,
            released: false,
            hold_frames: None,
            amp_env: AdsrVoice::new(44_100.0),
            env_params: AdsrParams::identity(),
            env_on: false,
            velocity_gain: 1.0,
        }
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active && !self.left.is_empty()
    }
}
