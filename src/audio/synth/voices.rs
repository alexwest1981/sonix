//! Rösterna och spåren motorn spelar: synth, duckning, stämspår, audition, samplingar.
//! Status: stabil — rösterna och spåren; en ny rösttyp hör hit och får eget prov.
//! Rör inte: `slice_edge_gain` — `fade_frames = 0` ska ge exakt den gamla vägen (1,0 överallt), inte nästan; det är den som skiljer ett klick från ett klipp.

use super::*;

/// Strängens lågpass i återkopplingen (Plucked!): 0,9 lämnar grundtonen nästan orörd
/// ett varv men tar diskanten, som en sträng som tappar sina övertoner först.
const PLUCK_LOWPASS: f32 = 0.9;
/// Efterklangens längd i **sekunder**, samma för alla tonhöjder — förlusten räknas om
/// per varv, så en kort sträng (som gör fler varv per sekund) inte dör i förtid.
const PLUCK_T60: f32 = 2.5;
/// Elgitarrens sträng klingar längre än ett plockat piano.
const GUITAR_T60: f32 = 3.2;
/// Var på strängen plektrumet träffar, som andel av längden. 0,18 ger en ljus,
/// "nära stallet"-karaktär; 0,5 hade gett en dov ton med släckt andra överton.
const GUITAR_PICK: f32 = 0.18;
/// Hur mycket den andra avläsningen släcker. 1,0 = fullständig släckning.
const GUITAR_PICK_MIX: f32 = 0.85;
/// Rörets arbetsområde: 1,0 = ren sträng, högre = mer klippning.
const GUITAR_DRIVE: f32 = 3.0;
/// Kroppens bidrag: 0,2 av strängen, men resonanserna ligger ~+6 dB i toppen (2,0).
/// En kropp som bara är 0 dB vid moden hörs inte — det var felet förra gången.
const BODY_MIX: f32 = 0.2 * 2.0;
/// Kroppens två moder. Låga Q (10 är taket i `StateVariableFilter`) ger en bred
/// resonans, som en gitarrkropp, och filtret är stabilt för alla Q.
const BODY_LO: FilterParams = FilterParams { cutoff: 196.0, resonance: 10.0 };
const BODY_HI: FilterParams = FilterParams { cutoff: 470.0, resonance: 10.0 };

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
    /// Modulatorns fas (FM). `index == 0` lämnar bärvågen orörd — då är röstens ljud
    /// bit-identiskt med oscillatorn före FM-vägen.
    pub mod_phase: f32,
    /// Portamento: fasökningen glider mot `target_inc` med `glide_coeff` per sampel.
    /// `glide_coeff == 0.0` betyder ingen glide, och då är rösten exakt som förut.
    pub target_inc: f32,
    pub glide_coeff: f32,
    /// Suboscillatorns fas (en oktav under noten), med egen ackumulator så att den inte
    /// hoppar när huvudoscillatorns fas slår om.
    pub sub_phase: f32,
    /// Fas per delton för den inharmoniska vågformen (Ogun).
    pub inh_phases: [f32; 6],
    /// Karplus-Strong-linjen (Plucked!): bruset som blir en sträng.
    pub pluck_buf: [f32; 4096],
    pub pluck_pos: usize,
    /// Sant när linjen fyllts för den här noten. Fylls vid första samplet, inte vid
    /// triggningen, så att en röst som aldrig spelar inte kostar något.
    pub pluck_started: bool,
    /// Förlusten per **varv** i linjen (Plucked!), räknad ur varvets längd när noten
    /// triggas — se `next_sample`.
    pub pluck_gain: f32,
    /// Fröet för bågens brus (Sakura). Per röst, så två stråkar inte får samma brus.
    pub bow_seed: u32,
    /// Kroppens två resonanser (Plucked!): egna filter med eget tillstånd, kopplade
    /// **efter** strängen så att de aldrig ligger i återkopplingen.
    pub body_lo: StateVariableFilter,
    pub body_hi: StateVariableFilter,
    /// Hur mycket av kroppen som mixas in. 0,0 = torr sträng; standard 1,0 = kropp på.
    pub body_amount: f32,
    /// Key tracking: filtrets brytfrekvens förskjuts med notens avstånd från C4.
    /// 1,0 = ingen förskjutning (så beter sig gamla projekt).
    pub key_cutoff_ratio: f32,
    /// MIDI-kanal + 1 (1–16) för en not som kom in över MIDI, annars 0. MPE:s
    /// pitch bend letar upp röster på samma kanal.
    pub channel: u8,
    /// Pitch bend som multiplikator på fasökningen (1,0 = ingen böjning).
    pub bend_ratio: f32,
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
    /// **Filterenvelopen** (Fas 8.4/7): kanalens filter (av/på, cutoff, resonans, envelopens
    /// djup i oktaver och envelopen själv) plus **tillståndet** — filtret och envelopen lever
    /// per röst. Är filtret av rörs samplarna inte alls, och ett projekt från före filtret är
    /// därför byte-identiskt.
    pub filter: crate::audio::filter::SamplerFilter,
    pub filter_state: StateVariableFilter,
    pub filter_env: AdsrVoice,
}

/// En bend som väntar på sin sample (inspelade bendkurvor).
pub struct ScheduledBend {
    pub samples_until: u32,
    pub channel: u8,
    pub semitones: f32,
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
            inh_phases: [0.0; 6],
            pluck_buf: [0.0; 4096],
            pluck_pos: 0,
            pluck_started: false,
            pluck_gain: 0.0,
            bow_seed: 0x2545_F491,
            body_lo: StateVariableFilter::new(sample_rate),
            body_hi: StateVariableFilter::new(sample_rate),
            body_amount: 1.0,
            note: 0,
            freq: 440.0,
            phase: 0.0,
            phase_inc: 0.0,
            velocity: 0.0,
            envelope: AdsrVoice::new(sample_rate),
            filter: StateVariableFilter::new(sample_rate),
            filter_env: AdsrVoice::new(sample_rate),
            mod_phase: 0.0,
            target_inc: 0.0,
            glide_coeff: 0.0,
            key_cutoff_ratio: 1.0,
            sub_phase: 0.0,
            channel: 0,
            bend_ratio: 1.0,
        }
    }

    /// Startar rösten. `from_freq` skiljer sig från `freq` bara när glide är på: då börjar
    /// rösten på den förra notens frekvens och glider till den nya.
    pub fn trigger_from(
        &mut self,
        note: u8,
        freq: f32,
        velocity: f32,
        sample_rate: f32,
        from_freq: f32,
        glide_coeff: f32,
    ) {
        self.trigger(note, freq, velocity, sample_rate);
        self.target_inc = (2.0 * PI * freq) / sample_rate.max(1.0);
        self.glide_coeff = glide_coeff.clamp(0.0, 1.0);
        if self.glide_coeff > 0.0 && from_freq > 0.0 {
            self.phase_inc = (2.0 * PI * from_freq) / sample_rate.max(1.0);
            self.freq = from_freq;
        }
    }

    pub fn trigger(&mut self, note: u8, freq: f32, velocity: f32, sample_rate: f32) {
        self.note = note;
        self.freq = freq;
        self.channel = 0;
        self.bend_ratio = 1.0;
        self.target_inc = (2.0 * PI * freq) / sample_rate.max(1.0);
        self.glide_coeff = 0.0;
        self.velocity = velocity;
        self.phase_inc = (2.0 * PI * freq) / sample_rate;
        self.filter.reset();
        // En ny plockning är en ny sträng: bruset fylls på nytt i linjen (annars
        // återanvänds den förra notens utklingning när röstplatsen lånas ut igen),
        // och kroppens tillstånd nollas så den inte bär med sig förra noten.
        self.pluck_started = false;
        // Ny not = nytt brus: bågen ska inte återanvända förra notens frö.
        self.bow_seed = 0x2545_F491 ^ (self.note as u32) << 8;
        self.body_lo.reset();
        self.body_hi.reset();
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
        self.mod_phase = 0.0;
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
        fm: &FmParams,
        mod_pitch: f32,
        mod_cutoff: f32,
        drive: f32,
        sub_level: f32,
        harmonics: &[f32; HARMONIC_COUNT],
    ) -> f32 {
        if !self.envelope.is_active() {
            return 0.0;
        }

        let env_gain = self.envelope.next_sample(adsr);
        if env_gain <= 0.0 {
            return 0.0;
        }

        // FM: modulatorn förskjuter bärvågens fas. Vid index 0 är förskjutningen noll och
        // `phase` ligger redan i [0, 2π), så båda while-slingorna hoppas över — samma tal,
        // samma ljud som oscillatorn utan FM.
        let two_pi = 2.0 * PI;
        let mut phase = self.phase;
        if fm.index != 0.0 {
            phase += self.mod_phase.sin() * fm.index;
            while phase >= two_pi {
                phase -= two_pi;
            }
            while phase < 0.0 {
                phase += two_pi;
            }
        }

        let raw_sample = match waveform {
            Waveform::Sine => phase.sin(),
            Waveform::Saw => (phase / PI) - 1.0,
            Waveform::Square => {
                if phase < PI {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Pluck => {
                let sr = self.string_rate();
                let len = self.string_len();
                if !self.pluck_started {
                    // Brus med fast frö per not: samma not låter likadant varje gång.
                    let mut seed = 0x9E37_79B9u32 ^ (self.note as u32) << 16 | 1;
                    for i in 0..self.pluck_buf.len() {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        self.pluck_buf[i] = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                    }
                    // Noll medelvärde i det som ligger i linjen: en likströmsrest har
                    // förstärkning 1 genom lågpasset och ligger kvar nästan för evigt —
                    // den hörs som ett dunk under strängen i stället för som en sträng.
                    let avg = self.pluck_buf[..len].iter().sum::<f32>() / len as f32;
                    for x in self.pluck_buf[..len].iter_mut() {
                        *x -= avg;
                    }
                    self.pluck_pos = 0;
                    self.pluck_started = true;
                    // **Dämpningen räknas per varv, inte per sampel.** Ett varv tar
                    // `len/sr` sekunder, så för att alla toner ska tappa lika många dB
                    // per sekund skalas förlusten med varvets längd: en kort sträng gör
                    // fler varv per sekund och hade annars dött direkt (det var felet
                    // förra gången, 110 Hz kvar men 1760 Hz borta).
                    // Lågpasset tar också en bit av grundtonen varje varv (mest för korta
                    // strängar, där grundtonen ligger högt i varvets spektrum). Den biten
                    // kompenseras bort, så att det är övertonerna som tappas fortare och
                    // inte tonhöjden. Kompensationen får aldrig ge loopförstärkning ≥ 1.
                    self.pluck_gain = Self::string_gain(len, sr);
                }
                let pos = self.pluck_pos;
                let next = if pos + 1 >= len { 0 } else { pos + 1 };
                let out = self.pluck_buf[pos];
                // Lågpass + förlust **inuti** linjen: det är där en sträng tappar energi.
                self.pluck_buf[pos] = (PLUCK_LOWPASS * out
                    + (1.0 - PLUCK_LOWPASS) * self.pluck_buf[next])
                    * self.pluck_gain;
                self.pluck_pos = next;
                // Kroppen är ett eget efterled utanför linjen: två resonanser som färgar
                // det strängen redan lämnat ifrån sig och därför inte kan påverka dess
                // återkoppling (ingen väg tillbaka in i `pluck_buf`).
                out + self.body_amount
                    * BODY_MIX
                    * (self.body_lo.process_bandpass(out, &BODY_LO)
                        + self.body_hi.process_bandpass(out, &BODY_HI))
            }
            Waveform::Guitar => {
                // **FL Slayer**: samma sträng som Pluck, men plektrumet sitter en bit in
                // på strängen. Signalen läses av på två ställen och differensen släcker
                // de övertoner som har en nod vid plektrumet — det är skillnaden mellan
                // en sträng och en sågtand. Efter strängen sitter ett mjukt rörsteg.
                let sr = self.string_rate();
                let len = self.string_len();
                if !self.pluck_started {
                    let mut seed = 0x51ED_2701u32 ^ (self.note as u32) << 16 | 1;
                    for i in 0..self.pluck_buf.len() {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        self.pluck_buf[i] = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                    }
                    let avg = self.pluck_buf[..len].iter().sum::<f32>() / len as f32;
                    // Högpassera bursten: plektrumets kant är ljusare än ett finger.
                    let mut prev = 0.0;
                    for x in self.pluck_buf[..len].iter_mut() {
                        let n = *x - avg;
                        *x = n - 0.85 * prev;
                        prev = n;
                    }
                    self.pluck_pos = 0;
                    self.pluck_started = true;
                    self.pluck_gain = Self::string_gain_with(len, sr, GUITAR_T60);
                }
                let pos = self.pluck_pos;
                let next = if pos + 1 >= len { 0 } else { pos + 1 };
                let raw = self.pluck_buf[pos];
                self.pluck_buf[pos] = (PLUCK_LOWPASS * raw
                    + (1.0 - PLUCK_LOWPASS) * self.pluck_buf[next])
                    * self.pluck_gain;
                self.pluck_pos = next;

                let tap = (pos + ((len as f32 * GUITAR_PICK) as usize).max(1)) % len;
                let comb = raw - GUITAR_PICK_MIX * self.pluck_buf[tap];
                // Mjuk klippning, normaliserad så att svaga noter inte blir rena och
                // starka inte blir kvadratvågor.
                let driven = (comb * GUITAR_DRIVE).tanh() / GUITAR_DRIVE.tanh();
                driven
                    + self.body_amount
                        * BODY_MIX
                        * (self.body_lo.process_bandpass(comb, &BODY_LO)
                            + self.body_hi.process_bandpass(comb, &BODY_HI))
            }
            Waveform::Bowed => {
                // Samma linje som Pluck, men exciterad **hela tiden**: en båge tillför energi
                // varje varv i stället för en burst vid anslaget. Bruset är litet — strängen
                // väljer själv ut sin grundton, och för mycket brus hade dränkt den.
                let sr = self.string_rate();
                let len = self.string_len();
                if !self.pluck_started {
                    for x in self.pluck_buf[..len].iter_mut() {
                        *x = 0.0;
                    }
                    self.pluck_pos = 0;
                    self.pluck_started = true;
                    self.pluck_gain = Self::string_gain(len, sr);
                }
                let pos = self.pluck_pos;
                let next = if pos + 1 >= len { 0 } else { pos + 1 };
                let out = self.pluck_buf[pos];
                self.bow_seed = self
                    .bow_seed
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                let noise = (self.bow_seed >> 8) as f32 / 8_388_608.0 - 1.0;
                // Bågens tryck följer anslaget: ett hårt anslag ger starkare ton.
                let excitation = noise * 0.06 * self.velocity.max(0.05);
                self.pluck_buf[pos] = (PLUCK_LOWPASS * out
                    + (1.0 - PLUCK_LOWPASS) * self.pluck_buf[next])
                    * self.pluck_gain
                    + excitation;
                self.pluck_pos = next;
                out + self.body_amount
                    * BODY_MIX
                    * (self.body_lo.process_bandpass(out, &BODY_LO)
                        + self.body_hi.process_bandpass(out, &BODY_HI))
            }
            Waveform::Inharmonic => {
                // Metalliska deltoner: inga heltalsmultiplar, och nivåerna faller snabbt.
                //
                // **Varje delton har sin egen fas.** Att multiplicera den redan omslagna
                // huvudfasen med 2,76 ger inte 2,76 gånger frekvensen: fasen hoppar vid
                // omslaget, och då hamnar deltonen någon annanstans än på 607 Hz. Provet
                // avslöjade det — 2× hördes starkt, 2,76× nästan inte alls.
                const RATIOS: [f32; 6] = [1.0, 2.76, 5.40, 8.93, 13.34, 18.64];
                let mut sum = 0.0;
                let mut weight = 0.0;
                for (i, ratio) in RATIOS.iter().enumerate() {
                    let v = &mut self.inh_phases[i];
                    *v += self.phase_inc * ratio;
                    if *v >= two_pi {
                        *v -= two_pi * (*v / two_pi).floor();
                    }
                    let level = 1.0 / (i as f32 + 1.0);
                    sum += level * v.sin();
                    weight += level;
                }
                sum / weight.max(0.001)
            }
            Waveform::Additive => {
                // Summan av övertonerna, normaliserad så att nivån inte beror på hur
                // många dragstänger som är utdragna.
                let mut sum = 0.0;
                let mut weight = 0.0;
                for (i, level) in harmonics.iter().enumerate() {
                    if *level > 0.0 {
                        sum += level * (phase * (i as f32 + 1.0)).sin();
                        weight += level;
                    }
                }
                if weight > 0.0 {
                    sum / weight.max(1.0)
                } else {
                    0.0
                }
            }
            Waveform::Triangle => {
                let normalized = phase / two_pi;
                2.0 * (2.0 * (normalized - (normalized + 0.5).floor())).abs() - 1.0
            }
        };

        // Portamento: fasökningen glider mot målet. Utan glide är koefficienten noll och
        // det här är en addition med noll — exakt samma ljud som förut.
        if self.glide_coeff > 0.0 {
            self.phase_inc += (self.target_inc - self.phase_inc) * self.glide_coeff;
            if (self.target_inc - self.phase_inc).abs() < 1e-9 {
                self.phase_inc = self.target_inc;
            }
        }

        // Advance phase. Tonhöjdsmodulationen skalas in här i stället för att räkna om
        // `phase_inc`, så en tremolo/vibrato följer med även genom FM. Pitch bend från
        // MIDI (MPE) är samma multiplikation: 1,0 när ingen böjer.
        let pitch_ratio = if mod_pitch.abs() > 1e-6 {
            2.0_f32.powf(mod_pitch / 12.0)
        } else {
            1.0
        } * self.bend_ratio;
        self.phase += self.phase_inc * pitch_ratio;
        if self.phase >= two_pi {
            self.phase -= two_pi;
        }
        if fm.index != 0.0 {
            self.mod_phase += self.phase_inc * pitch_ratio * fm.ratio.max(0.01);
            if self.mod_phase >= two_pi {
                self.mod_phase -= two_pi;
            }
        }

        // Each voice runs through its own resonant filter instance, and its own
        // filter envelope modulates the cutoff (in octaves) independently.
        let fenv = self.filter_env.next_sample(filter_env);
        let effective = FilterParams {
            cutoff: filter_params.cutoff
                * self.key_cutoff_ratio
                * 2.0_f32.powf(filter_env_amount * fenv + mod_cutoff.clamp(-4.0, 4.0)),
            resonance: filter_params.resonance,
        };
        let filtered = self
            .filter
            .process_lowpass(raw_sample * self.velocity * env_gain, &effective);
        // Filterdrive: mjuk klippning i stället för en hård gräns, normaliserad så att en
        // fullskalig sinus behåller sin nivå. drive 0 är exakt den gamla vägen.
        let filtered = if drive > 0.0 {
            let g = 1.0 + drive.clamp(0.0, 1.0) * 7.0;
            (filtered * g).tanh() / g.tanh()
        } else {
            filtered
        };
        // Suboscillatorn läggs på **efter** filtret: en subbas som försvinner när filtret
        // stängs är inte en subbas.
        if sub_level > 0.0 {
            self.sub_phase += self.phase_inc * pitch_ratio * 0.5;
            if self.sub_phase >= 2.0 * PI {
                self.sub_phase -= 2.0 * PI;
            }
            filtered + self.sub_phase.sin() * sub_level.clamp(0.0, 1.0) * self.velocity * env_gain
        } else {
            filtered
        }
    }
}

impl Voice {
    /// Samplade frekvensen — används för strängens dämpning, inte för tonhöjden.
    fn string_rate(&self) -> f32 {
        if self.phase_inc.abs() > 1e-9 {
            2.0 * PI * self.freq / self.phase_inc
        } else {
            48_000.0
        }
    }

    /// Strängens längd i sampel: `sr / f`, klämd till fördröjningslinjens storlek.
    fn string_len(&self) -> usize {
        (self.string_rate() / (self.freq * self.bend_ratio).max(20.0)).clamp(2.0, 4095.0) as usize
    }

    /// Förlusten per **varv** i linjen, kompenserad för lågpassets dämpning av grundtonen.
    ///
    /// Ett varv tar `len/sr` sekunder, så för att alla toner ska tappa lika många dB per
    /// sekund skalas förlusten med varvets längd: en kort sträng gör fler varv per sekund
    /// och hade annars dött direkt. Kompensationen får aldrig ge loopförstärkning ≥ 1.
    fn string_gain(len: usize, sr: f32) -> f32 {
        Self::string_gain_with(len, sr, PLUCK_T60)
    }

    /// Samma räkning med egen T60 — gitarren klingar längre än det plockade pianot.
    fn string_gain_with(len: usize, sr: f32, t60: f32) -> f32 {
        let w = 2.0 * PI / len as f32;
        let d = 1.0 - PLUCK_LOWPASS;
        let h = PLUCK_LOWPASS / (1.0 - 2.0 * d * w.cos() + d * d).sqrt();
        let per_loop = 0.001_f32.powf(len as f32 / sr / t60.max(0.05));
        (per_loop / h).min(0.9995)
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
            filter: crate::audio::filter::SamplerFilter::default(),
            filter_state: StateVariableFilter::new(44_100.0),
            filter_env: AdsrVoice::new(44_100.0),
        }
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active && !self.left.is_empty()
    }
}
