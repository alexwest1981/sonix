//! Status: byggs — kommando-protokollet (AI-vägen)
//! Rör inte: nya kommandon ska gå genom `Command` så att agenten och UI:t gör samma sak
use super::drum::DrumType;
use super::effects::{DelayParams, ReverbParams};
use super::envelope::AdsrParams;
use super::filter::FilterParams;
use super::master_fx::{MasterFxParams, TrackEqSettings};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

#[allow(dead_code)]
impl Waveform {
    pub fn name(&self) -> &'static str {
        match self {
            Waveform::Sine => "Sinus (Sine)",
            Waveform::Saw => "Sågtand (Saw)",
            Waveform::Square => "Fyrkant (Square)",
            Waveform::Triangle => "Triangel (Triangle)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preset {
    CleanPluck,
    WarmPad,
    AcidBass,
    ChiptuneLead,
    CosmicBrass,
}

#[allow(dead_code)]
impl Preset {
    pub fn settings(&self) -> (Waveform, AdsrParams, FilterParams) {
        match self {
            Preset::CleanPluck => (
                Waveform::Saw,
                AdsrParams { attack: 0.005, decay: 0.35, sustain: 0.1, release: 0.25 },
                FilterParams { cutoff: 3800.0, resonance: 1.8 },
            ),
            Preset::WarmPad => (
                Waveform::Saw,
                AdsrParams { attack: 0.45, decay: 0.5, sustain: 0.75, release: 0.9 },
                FilterParams { cutoff: 1400.0, resonance: 0.9 },
            ),
            Preset::AcidBass => (
                Waveform::Saw,
                AdsrParams { attack: 0.005, decay: 0.18, sustain: 0.15, release: 0.1 },
                FilterParams { cutoff: 750.0, resonance: 5.0 },
            ),
            Preset::ChiptuneLead => (
                Waveform::Square,
                AdsrParams { attack: 0.002, decay: 0.08, sustain: 0.7, release: 0.08 },
                FilterParams { cutoff: 12000.0, resonance: 0.7 },
            ),
            Preset::CosmicBrass => (
                Waveform::Triangle,
                AdsrParams { attack: 0.08, decay: 0.3, sustain: 0.65, release: 0.45 },
                FilterParams { cutoff: 4500.0, resonance: 2.5 },
            ),
        }
    }
}

use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub enum AudioCommand {
    /// Replaces (or clears, with `None`) the CLAP insert on a stem track. The
    /// insert is built on the UI thread and handed to the audio thread here.
    SetTrackPlugin {
        track_index: usize,
        insert: Option<crate::audio::plugin_host_live::PluginInsert>,
    },
    /// Sets a parameter on a track's live plugin insert.
    SetPluginParameter {
        track_index: usize,
        param_id: u32,
        value: f64,
    },
    NoteOn {
        note: u8,
        freq: f32,
        velocity: f32,
    },
    NoteOff {
        note: u8,
    },
    /// En not som ska klinga en bit in i steget (Fas 6.4 steg 2), för tagningens
    /// mikro-tajming. Motorn har redan kön `scheduled_notes` (samma som
    /// `StrumChord` använder), så noten triggas samplenoggrant efter
    /// `delay_samples` i stället för att tvingas fram på steggränsen.
    NoteOnDelayed {
        note: u8,
        freq: f32,
        velocity: f32,
        delay_samples: u32,
    },
    /// Schedules a chord/arpeggio: each note in `notes` is triggered after
    /// `spread_samples * i` samples. `mode`: 0 = Block, 1 = Up, 2 = Down,
    /// 3 = Random. Used by the Chord Matrix audition/strum.
    StrumChord {
        notes: Vec<u8>,
        velocity: f32,
        start_samples: u32,
        spread_samples: u32,
        mode: u8,
    },
    TriggerDrum(DrumType),
    SetWaveform(Waveform),
    SetAdsr(AdsrParams),
    SetFilter(FilterParams),
    /// Per-voice filter envelope: `amount` is the cutoff modulation depth in
    /// octaves (positive = brighter, negative = darker); `adsr` shapes the
    /// per-note sweep. Default amount 0 disables it.
    SetFilterEnv {
        amount: f32,
        adsr: AdsrParams,
    },
    SetDelay(DelayParams),
    SetReverb(ReverbParams),
    SetDrive(f32),
    SetMasterFx(MasterFxParams),
    SetTrackEq {
        track_index: usize,
        settings: TrackEqSettings,
    },
    /// Per-track dynamics, aux sends and pitch for the selected channel strip.
    SetTrackMix {
        track_index: usize,
        comp_threshold_db: f32,
        comp_ratio: f32,
        reverb_send: f32,
        delay_send: f32,
        pitch_semitones: f32,
    },
    SetRemixFx {
        mode: u8,
        bpm: f32,
    },
    SetTapeStop {
        active: bool,
    },
    LoadPreset(Preset),
    SetMasterVolume(f32),
    StopAll,
    // Multi-Track Stem Audio Streaming
    LoadStemTrack {
        track_index: usize,
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pan: f32,
        start_time_secs: f32,
    },
    ClearAllStemTracks,
    SetStemTrackState {
        track_index: usize,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    },
    SetStemTrackRegions {
        track_index: usize,
        regions: Vec<StemRegionPlayback>,
    },
    /// Routes a stem track to a sub-mix bus and an optional VCA group (Fas 5.2).
    /// The bus is clamped to `0..NUM_BUSES`, the VCA to `0..NUM_VCAS` (`None`
    /// = the track is not in any VCA group).
    SetStemTrackRouting {
        track_index: usize,
        bus: usize,
        vca: Option<usize>,
    },
    /// Kopplar en sidokedja (Fas 8.3): `track_index` duckas av `from`s ljud.
    ///
    /// `amount_db` är hur mycket spåret sänks när key-signalen är över
    /// `threshold_db`. `from = None` kopplar bort sidokedjan. Ett `from` som
    /// pekar på spåret självt (eller utanför listan) ignoreras av motorn — en
    /// slinga skulle aldrig kunna beräknas.
    SetStemTrackSidechain {
        track_index: usize,
        from: Option<usize>,
        amount_db: f32,
        threshold_db: f32,
    },
    /// Skickar en del av spårets signal till andra bussar (Fas 8.13).
    ///
    /// En **send** är en parallell väg: spåret går fortfarande till sin egen buss,
    /// och det här är extra. Nivån är linjär (1,0 = lika starkt som spårets egen
    /// utgång), och målet kläms till `0..NUM_BUSES`.
    SetStemTrackSends {
        track_index: usize,
        sends: Vec<StemSend>,
    },
    /// Sets a sub-mix bus's group gain, mute and solo (Fas 5.2). The bus is
    /// clamped to `0..NUM_BUSES`.
    SetBusState {
        bus: usize,
        volume: f32,
        muted: bool,
        solo: bool,
    },
    /// Sets a VCA group's gain, mute and solo (Fas 5.2). The VCA is clamped to
    /// `0..NUM_VCAS`.
    SetVcaState {
        vca: usize,
        volume: f32,
        muted: bool,
        solo: bool,
    },
    SeekSongPosition(f32),
    SetSongPlayback(bool),
    // Isolated Audition for Vocal Studio & Sample Preview (does not affect song timeline)
    PlayAudition {
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: f32,
        volume: f32,
        pitch_ratio: f32,
        time_stretch_ratio: f32,
        is_reverse: bool,
        loop_playback: bool,
    },
    StopAudition,
    /// Registers the microphone's post-auto-tune mono monitor buffer. The
    /// audio thread mixes it into the master bus scaled by
    /// [`AudioCommand::SetMonitorLevel`] (default 1.0, i.e. unity).
    /// Sent again after every reconfigure.
    SetMonitorRing {
        ring: Arc<Mutex<Vec<f32>>>,
    },
    /// Nivå för direktlyssningen (0.0–1.0). Utan den är mastervolymen enda
    /// reglaget för monitor-signalen, vilket gör återkoppling (rundgång) svår
    /// att bryta utan att sänka hela mixen.
    SetMonitorLevel(f32),
    SetAuditionParams {
        volume: f32,
        pitch_ratio: f32,
        time_stretch_ratio: f32,
    },
    // Polyphonic WAV one-shot playback (Channel Rack sample player)
    TriggerSampleVoice {
        left: Arc<Vec<f32>>,
        right: Arc<Vec<f32>>,
        sample_rate: u32,
        base_note: u8,
        note: u8,
        pitch_semitones: i8,
        pitch_cents: f32,
        velocity: f32,
        volume: f32,
        reverse: bool,
        start01: f32,
        end01: f32,
    },
    // Modular Patcher (node graph) — real DSP graph evaluated per sample
    SetPatcherGraph(crate::audio::patcher::PatchSpec),
    SetPatcherEnabled(bool),
    PatcherNoteOn {
        freq: f32,
        velocity: f32,
    },
    PatcherNoteOff,
}

/// En send: en del av spårets signal till en annan buss (Fas 8.13).
///
/// `level` är linjär (1,0 = lika starkt som spårets egen utgång). Målet är en
/// buss, inte ett spår: en buss skickar inte vidare, så en send kan aldrig bli en
impl SendTarget {
    /// En buss som mål.
    pub fn bus(target_bus: usize) -> Self {
        SendTarget::Bus { target_bus }
    }
    /// Ett spår som mål.
    pub fn track(target_track: usize) -> Self {
        SendTarget::Track { target_track }
    }
    /// Buss-numret, om målet är en buss.
    pub fn as_bus(self) -> Option<usize> {
        match self {
            SendTarget::Bus { target_bus } => Some(target_bus),
            SendTarget::Track { .. } => None,
        }
    }
    /// Spår-numret, om målet är ett spår.
    pub fn as_track(self) -> Option<usize> {
        match self {
            SendTarget::Track { target_track } => Some(target_track),
            SendTarget::Bus { .. } => None,
        }
    }
}

/// **Ordningen spåren måste räknas i** (Fas 8.3) — topologisk, så att ett spår som tar emot
/// en send alltid räknas **efter** sina sändare.
///
/// Annars läser mottagaren en utgång som ännu inte finns för det samplet, och senden kommer
/// fram en sample sent. Det hörs inte som ett klick — det hörs som att signalen tar ut sig
/// själv mot mottagarens egen signal, för en förskjuten kopia av samma ljud släcker sig själv
/// i diskanten. Roadmapen pekade ut just det som det svåra med den här punkten.
///
/// **Ren funktion:** antal spår och varje spårs spår-mål in, ordningen ut. `Err((a, b))`
/// betyder att en slinga går genom `a` och `b` — då finns ingen giltig ordning, och motorn
/// behåller sin förra i stället för att gissa. Ett spår som skickar till **sig självt** är en
/// slinga och svarar `Err((a, a))`.
///
/// **Stabil:** spår utan inbördes beroende behåller sin naturliga ordning, så att ljudet inte
/// ändras av att ordningen råkar bli en annan. Mål utanför spårlistan ignoreras (de kan gälla
/// ett spår som ännu inte laddats), och en dubblerad kant räknas två gånger — precis som
/// grafen säger.
pub fn plan_track_order(
    track_count: usize,
    track_sends: &[Vec<usize>],
) -> Result<Vec<usize>, (usize, usize)> {
    let mut indegree = vec![0usize; track_count];
    for (from, targets) in track_sends.iter().enumerate() {
        if from >= track_count {
            break;
        }
        for &to in targets {
            if to == from {
                return Err((from, from)); // ett spår kan inte skicka till sig självt
            }
            if to < track_count {
                indegree[to] += 1;
            }
        }
    }
    let mut order = Vec::with_capacity(track_count);
    let mut done = vec![false; track_count];
    while order.len() < track_count {
        let Some(next) = (0..track_count).find(|&i| !done[i] && indegree[i] == 0) else {
            // Alla kvarvarande spår väntar på någon annan: en slinga. Namnge två av dem.
            let a = (0..track_count)
                .find(|&i| !done[i])
                .unwrap_or(0);
            let b = track_sends
                .get(a)
                .and_then(|v| {
                    v.iter()
                        .copied()
                        .find(|&t| t < track_count && !done[t])
                })
                .unwrap_or(a);
            return Err((a, b));
        };
        done[next] = true;
        order.push(next);
        if let Some(targets) = track_sends.get(next) {
            for &to in targets {
                if to < track_count {
                    indegree[to] = indegree[to].saturating_sub(1);
                }
            }
        }
    }
    Ok(order)
}

/// **Vart en send går** (Fas 8.13 bussar, Fas 8.3 spår).
///
/// En send är *ljud*, inte en mätning: den måste komma fram **exakt i fas**, annars tar den
/// ut sig själv mot mottagarens egen signal. Därför går en spår-send in i mottagarens kedja
/// **före** dess pitch, EQ och kompressor — samma sak som FL:s *Track Send* och Abletons
/// *Audio To* — och motorn räknar spåren i en ordning där sändaren alltid är klar först
/// ([`plan_track_order`]).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SendTarget {
    /// En buss (`0..NUM_BUSES`). Bussen har ingen egen bearbetning — bara nivå, mute och
    /// solo — och den skickar inte vidare.
    Bus { target_bus: usize },
    /// Ett annat spår (`0..antal spår`). Signalen hamnar i mottagarens kedja, och följer
    /// sedan med till mottagarens buss.
    Track { target_track: usize },
}

/// En send från ett spår: vart den går och hur starkt (`1,0` = lika starkt som spårets
/// egen utgång).
///
/// **Post-fader**, och samma signal som går till spårets egen buss — efter volym, EQ,
/// kompressor och sidokedja. En send tappar alltså inte EQ:n eller duckningen på vägen, och
/// den kan inte smyga sig förbi spårets egen mute.
///
/// **Serde-formen är flat och bakåtkompatibel:** en buss-send skrivs `{target_bus, level}`
/// precis som förut, så en gammal projektfil läses oförändrat, och en spår-send skrivs
/// `{target_track, level}`. Provet `old_project_files_read_a_bus_send_as_before` håller
/// den formen kvar — den är inte kosmetisk.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StemSend {
    #[serde(flatten)]
    pub target: SendTarget,
    pub level: f32,
}

#[derive(Debug, Clone)]
pub struct StemRegionPlayback {
    pub start_time_secs: f32,
    pub length_secs: f32,
    pub sample_offset_sec: f32,
    pub gain: f32,
    pub fade_in_sec: f32,
    pub fade_out_sec: f32,
    pub muted: bool,
    pub is_reverse: bool,
    pub loop_length_secs: f32,
    /// Källsekunder per utsekund (Fas 8.10). 1.0 = spela som inspelat.
    ///
    /// Räknas av appen ur regionens `source_bpm` och projektets tempo; motorn
    /// multiplicerar bara tiden med den. Se `region_source_secs`.
    pub stretch_ratio: f32,
    /// Eget källjud för det här klippet (Fas 8.10 steg 2): en **färdigsträckt fil**.
    ///
    /// Finns den läses den i stället för spårets buffert, och `stretch_ratio` är 1,0 —
    /// tidslinjen spelar då en vanlig fil och uppspelningsvägen får ingen ny
    /// felkälla. `None` = spela spårets eget ljud med faktorn, som förut.
    pub source_audio: Option<super::stretch::StretchedAudio>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Fas 8.3: sändningarnas form och ordning -------------------------------

    /// **En gammal projektfil läses exakt som förut.** En buss-send skrevs
    /// `{target_bus, level}` före den här punkten, och den formen får inte ha ändrats —
    /// annars tappar ett sparat projekt sin send utan ett ord.
    #[test]
    fn old_project_files_read_a_bus_send_as_before() {
        let old = r#"[{"target_bus":2,"level":0.5}]"#;
        let sends: Vec<StemSend> = serde_json::from_str(old).expect("gammal form ska läsas");
        assert_eq!(sends.len(), 1);
        assert_eq!(sends[0].target, SendTarget::bus(2));
        assert!((sends[0].level - 0.5).abs() < 1e-6);
        assert_eq!(sends[0].target.as_bus(), Some(2));
        assert_eq!(sends[0].target.as_track(), None);
    }

    /// **En spår-send skrivs och läses i samma flata form** — `{target_track, level}` —
    /// så att en ny fil är lika läsbar som en gammal, och rundgången är exakt.
    #[test]
    fn a_track_send_round_trips_in_the_same_flat_shape() {
        let sends = vec![
            StemSend { target: SendTarget::track(3), level: 0.25 },
            StemSend { target: SendTarget::bus(1), level: 0.75 },
        ];
        let json = serde_json::to_string(&sends).expect("ska gå att skriva");
        assert!(json.contains("\"target_track\":3"), "spårmålet ska synas i filen: {json}");
        assert!(json.contains("\"target_bus\":1"), "bussmålet ska synas i filen: {json}");
        let back: Vec<StemSend> = serde_json::from_str(&json).expect("rundgång");
        assert_eq!(back, sends);
    }

    /// **Ordningen: en sändare räknas alltid före sin mottagare** (Fas 8.3). Provet som
    /// fångar en kedja som ligger baklänges i spårlistan — den naturliga ordningen hade
    /// gett senden en sample sent, och då tar signalen ut sig själv i diskanten.
    #[test]
    fn the_order_puts_senders_before_receivers() {
        // 2 → 1 → 0: spåren ligger baklänges, ordningen måste vända dem.
        let order = plan_track_order(3, &[vec![], vec![0], vec![1]]).expect("en kedja är ingen slinga");
        assert_eq!(order, vec![2, 1, 0]);
        // Utan sends behålls den naturliga ordningen (stabilitet: ljudet ska inte ändras
        // av att en ordning råkar bli en annan).
        assert_eq!(plan_track_order(3, &[vec![], vec![], vec![]]).expect("tom graf"), vec![0, 1, 2]);
        // Diamant: 0 → 1, 0 → 2, 1 → 3, 2 → 3.
        assert_eq!(
            plan_track_order(4, &[vec![1, 2], vec![3], vec![3], vec![]]).expect("diamant"),
            vec![0, 1, 2, 3]
        );
        // Noll spår är en giltig (tom) ordning, inte ett fel.
        assert_eq!(plan_track_order(0, &[]).expect("tomt"), Vec::<usize>::new());
    }

    /// **En slinga kan inte bli en ordning** — och den namnges i stället för att gissas.
    /// Ett spår som skickar till sig självt är också en slinga (och svarar `(a, a)`).
    #[test]
    fn a_loop_is_named_instead_of_guessed() {
        assert_eq!(plan_track_order(2, &[vec![1], vec![0]]), Err((0, 1)));
        assert_eq!(plan_track_order(3, &[vec![1], vec![2], vec![0]]), Err((0, 1)));
        assert_eq!(plan_track_order(1, &[vec![0]]), Err((0, 0)));
        // Slingan får inte döljas av att en del av grafen är acyklisk: 3 → 0 är fritt,
        // men 0 ↔ 1 är en slinga och ska fällas.
        assert_eq!(plan_track_order(3, &[vec![1], vec![0], vec![0]]), Err((0, 1)));
    }

    /// **Mål utanför spårlistan ignoreras** (spåret kan laddas senare), och en dubblerad
    /// kant räknas två gånger — precis som grafen säger. Ingen av dem får panikera.
    #[test]
    fn targets_that_do_not_exist_yet_are_ignored() {
        let order = plan_track_order(2, &[vec![7, 7], vec![99]]).expect("okända mål är inga slingor");
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], 0, "inget beroende kvar: naturlig ordning");
        // En dubblerad kant från ett senare spår vänder fortfarande ordningen.
        assert_eq!(plan_track_order(2, &[vec![], vec![0, 0]]).expect("dubbel kant"), vec![1, 0]);
    }
}

