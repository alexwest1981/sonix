//! Motorns prov: tidmappning, sträckning, regioner, kantrampen och klockan.
//!
//! Flyttade ur `synth.rs` 2026-09-14 (motorn var 3 517 rader, varav proven 1 518).
//! Status: stabil — prov, inte kod; flyttar du en funktion, flytta dess prov.

use super::*;
use crate::audio::filter::SamplerFilter;

/// Delad tabell för prov som bara behöver *någon* tabell (de flesta spelar Saw/Sine).
/// Tabellen byggs i elva oktavband — att skapa den per sampel gjorde enstaka prov minuter långa.
fn default_table() -> &'static AdditiveTable {
    static TABLE: std::sync::OnceLock<AdditiveTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| AdditiveTable::new(&HARMONICS_DEFAULT, 48_000.0))
}

/// **Rampen vid kanten.** 0 vid kanten, 1 en ramp in, rakt däremellan — och samma regel
/// används i båda ändarna av fönstret, så den behöver bara vara rätt en gång.
#[test]
fn the_edge_fade_is_zero_at_the_edge_and_one_a_ramp_in() {
    assert_eq!(slice_edge_gain(0.0, 88.0), 0.0, "vid kanten ska den vara tyst");
    assert!((slice_edge_gain(44.0, 88.0) - 0.5).abs() < 1e-6);
    assert_eq!(slice_edge_gain(88.0, 88.0), 1.0);
    assert_eq!(slice_edge_gain(1000.0, 88.0), 1.0, "långt in rörs inget");
}

/// **Avstängd dämpning ska vara identisk med den gamla vägen, inte nästan.** Det är
/// kontrollen som gör att en påslagen ramp aldrig kan ändra något den inte ska.
#[test]
fn a_zero_length_fade_is_the_old_behaviour_everywhere() {
    for frames in [0.0, 1.0, 17.0, 1e6] {
        assert_eq!(slice_edge_gain(frames, 0.0), 1.0, "{frames} ramar");
    }
}

/// Skräp ska inte tysta en röst: hellre odämpat än tyst.
#[test]
fn nonsense_inputs_leave_the_voice_audible() {
    assert_eq!(slice_edge_gain(f32::NAN, 88.0), 1.0);
    assert_eq!(slice_edge_gain(10.0, f32::NAN), 1.0);
    assert_eq!(slice_edge_gain(f32::INFINITY, 88.0), 1.0);
    assert_eq!(slice_edge_gain(-5.0, 88.0), 0.0, "före kanten är tyst");
}

/// En region för tidsmappningstesterna (Fas 8.10).
fn region_under_test(
    length_secs: f32,
    offset: f32,
    loop_length_secs: f32,
    is_reverse: bool,
    stretch_ratio: f32,
) -> StemRegionPlayback {
    StemRegionPlayback {
        start_time_secs: 0.0,
        length_secs,
        sample_offset_sec: offset,
        gain: 1.0,
        fade_in_sec: 0.0,
        fade_out_sec: 0.0,
        muted: false,
        is_reverse,
        loop_length_secs,
        stretch_ratio,
        source_audio: None,
    }
}

/// Klippet följer tempot **på riktigt**, genom mixern — inte bara i
/// aritmetiken (Fas 8.10).
///
/// Ett anslag som ligger 0,5 s in i källjudet ska höras efter 0,25 s ut-tid när
/// klossen spelas dubbelt så fort, och efter 0,5 s när den inte är sträckt.
/// Samma ljud, samma region, olika faktor.
#[test]
fn a_stretched_clip_plays_its_transient_earlier() {
    let sr = 48_000u32;
    let mut source = vec![0.0f32; sr as usize];
    source[24_000] = 1.0; // anslaget ligger på 0,5 s i källan
    let left = Arc::new(source.clone());
    let right = Arc::new(source);

    // Fram till 0,25 s ut-tid (12 100 block): anslaget hinns bara med när
    // klossen går dubbelt så fort.
    for (rate, expected) in [(1.0f32, false), (2.0f32, true)] {
        let mut synth = SynthEngine::new(sr as f32);
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: left.clone(),
            right: right.clone(),
            sample_rate: sr as f32,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        synth.handle_command(AudioCommand::SetStemTrackRegions {
            track_index: 0,
            regions: vec![region_under_test(1.0, 0.0, 0.0, false, rate)],
        });
        synth.handle_command(AudioCommand::SetSongPlayback(true));

        let mut peak = 0.0f32;
        for _ in 0..12_100 {
            let (l, _r) = synth.process_stereo();
            peak = peak.max(l.abs());
        }
        assert_eq!(
            peak > 0.3,
            expected,
            "rate={rate}: toppen blev {peak} — anslaget ska {}höras inom 0,25 s",
            if expected { "" } else { "INTE " }
        );
    }
}

/// Ljudklockan räknar **renderade bildrutor** — det är spelhuvudets klocka (8.13b).
///
/// Spelhuvudet läste förut UI:ts egen stegklocka, och de två gick isär: mätt
/// 2026-09-13 låg markören 0,66 s efter ljudet. Den här klockan står still i paus,
/// räknar exakt en bildruta per renderad bildruta när transporten går — och gör det
/// även i ett projekt utan stämmor, så att spelhuvudet aldrig kan fastna.
#[test]
fn the_song_clock_counts_rendered_frames() {
    let mut synth = SynthEngine::new(44_100.0);
    for _ in 0..4_410 {
        synth.process_stereo();
    }
    assert_eq!(synth.song_time_samples, 0, "en pausad transport ska inte röra klockan");

    synth.handle_command(AudioCommand::SetSongPlayback(true));
    for _ in 0..4_410 {
        synth.process_stereo();
    }
    assert_eq!(synth.song_time_samples, 4_410, "0,1 s ljud = 4 410 bildrutor i 44,1 kHz");
    let secs = synth.song_time_samples as f32 / synth.sample_rate;
    assert!((secs - 0.1).abs() < 1e-4, "sekunderna kommer ur samma räkning: {secs}");

    synth.handle_command(AudioCommand::SetSongPlayback(false));
    for _ in 0..1_000 {
        synth.process_stereo();
    }
    assert_eq!(synth.song_time_samples, 4_410, "paus håller kvar positionen");
}

/// Ett klipp med eget källjud spelar **filen**, inte spårets buffert (Fas 8.10 steg 2).
///
/// Det är hela poängen med att räkna offline: tidslinjen spelar en färdigsträckt fil
/// med faktor 1,0, och uppspelningsvägen får ingen ny felkälla. Testet mäter **var**
/// anslaget hörs — på filens egen sekund (0,1 s) i stället för spårets (0,5 s).
#[test]
fn a_region_with_its_own_audio_plays_that_file_instead_of_the_track() {
    let sr = 48_000u32;
    let with_ping = |at: usize| -> Arc<Vec<f32>> {
        let mut v = vec![0.0f32; sr as usize];
        v[at] = 1.0;
        Arc::new(v)
    };
    let track = with_ping(24_000); // spårets anslag: 0,5 s
    let file = with_ping(4_800); // den sträckta filens: 0,1 s

    for own_file in [false, true] {
        let mut synth = SynthEngine::new(sr as f32);
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: track.clone(),
            right: track.clone(),
            sample_rate: sr as f32,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
        let mut region = region_under_test(1.0, 0.0, 0.0, false, 1.0);
        if own_file {
            region.source_audio = Some((file.clone(), file.clone(), sr as f32));
        }
        synth.handle_command(AudioCommand::SetStemTrackRegions {
            track_index: 0,
            regions: vec![region],
        });
        synth.handle_command(AudioCommand::SetSongPlayback(true));

        // 6 000 block ≈ 0,125 s: filens anslag hinns med, spårets är 0,5 s bort.
        let mut peak = 0.0f32;
        for _ in 0..6_000 {
            let (l, _r) = synth.process_stereo();
            peak = peak.max(l.abs());
        }
        assert_eq!(
            peak > 0.3,
            own_file,
            "eget källjud={own_file}: toppen blev {peak} — {}",
            if own_file {
                "filens anslag på 0,1 s ska höras"
            } else {
                "spårets anslag ligger på 0,5 s och ska inte ha hunnit"
            }
        );
    }
}

/// Vid 1,0 är tidsmappningen **exakt** den som gällde före 8.10.
///
/// Det här är regressionsvakten: inför tempoföljningen får ingen region som
/// inte är sträckt spela ett sample annorlunda än förut.
/// **En omladdning får inte tömma klippen** (Fas 8.10c).
///
/// `LoadStemTrack` byggde ett nytt spår och ärvde eq, kompressor, sends och plugin —
/// men **inte** `regions`. Appen skickar `LoadStemTrack` vid varje uppspelningsstart,
/// så en tempoändring blev ohörbar: motorn föll tillbaka till originalet medan vyn
/// ritade den sträckta filen. Mätt på Alex' RAHP140 2026-09-13 (9 klipp, 140 → 100).
#[test]
fn a_track_reload_keeps_the_stretched_regions() {
    let sr = 48_000u32;
    let pcm = Arc::new(vec![0.0f32; sr as usize]);
    fn load(synth: &mut SynthEngine, pcm: &Arc<Vec<f32>>, sr: u32) {
        synth.handle_command(AudioCommand::LoadStemTrack {
            track_index: 0,
            left: pcm.clone(),
            right: pcm.clone(),
            sample_rate: sr as f32,
            volume: 1.0,
            pan: 0.0,
            start_time_secs: 0.0,
        });
    }
    let mut synth = SynthEngine::new(sr as f32);
    load(&mut synth, &pcm, sr);
    let mut region = region_under_test(1.0, 0.0, 0.0, false, 1.0);
    region.source_audio = Some((pcm.clone(), pcm.clone(), sr as f32));
    synth.handle_command(AudioCommand::SetStemTrackRegions {
        track_index: 0,
        regions: vec![region],
    });
    assert_eq!(
        synth.stretched_region_tracks, 1,
        "motorn ska ha ett sträckt spår efter beställningen"
    );

    load(&mut synth, &pcm, sr); // uppspelningsstart: det var här klippen tömdes
    assert_eq!(
        synth.stem_tracks[0].regions.len(),
        1,
        "klippet ska överleva omladdningen"
    );
    assert!(
        synth.stem_tracks[0].regions[0].source_audio.is_some(),
        "och det sträckta ljudet med"
    );
    assert_eq!(
        synth.stretched_region_tracks, 1,
        "räknaren ska stå kvar — den är vad appen läser"
    );
}

#[test]
fn a_region_at_rate_one_maps_time_exactly_as_before() {
    let plain = region_under_test(4.0, 0.0, 0.0, false, 1.0);
    for t in [0.0f32, 0.25, 1.0, 3.999] {
        assert!((region_source_secs(&plain, t) - t).abs() < 1e-6, "t={t}");
    }

    // Med offset: positionen flyttas med offset, precis som förut.
    let offset = region_under_test(4.0, 1.5, 0.0, false, 1.0);
    assert!((region_source_secs(&offset, 0.5) - 2.0).abs() < 1e-5);

    // Slinga: fasen är (offset + tid) modulo loopen — som förut.
    let looped = region_under_test(8.0, 0.0, 2.0, false, 1.0);
    assert!((region_source_secs(&looped, 0.5) - 0.5).abs() < 1e-5);
    assert!((region_source_secs(&looped, 2.5) - 0.5).abs() < 1e-5, "varv två börjar om");
    assert!((region_source_secs(&looped, 4.25) - 0.25).abs() < 1e-5);

    // Omvänt: räknar nedåt från slutet.
    let reversed = region_under_test(4.0, 0.0, 0.0, true, 1.0);
    assert!((region_source_secs(&reversed, 0.0) - 4.0).abs() < 1e-5);
    assert!((region_source_secs(&reversed, 1.0) - 3.0).abs() < 1e-5);
}

/// Sträckt: källan läses fortare, så samma ut-tid når längre in i ljudet.
#[test]
fn a_stretched_region_reads_further_into_the_audio() {
    // Dubbelt tempo: efter 0,5 s ut-tid har 1,0 s källjud gått.
    let fast = region_under_test(4.0, 0.0, 0.0, false, 2.0);
    assert!((region_source_secs(&fast, 0.5) - 1.0).abs() < 1e-5);
    assert!((region_source_secs(&fast, 2.0) - 4.0).abs() < 1e-5);

    // Halva tempot: efter 1,0 s har bara 0,5 s ljud gått.
    let slow = region_under_test(4.0, 0.0, 0.0, false, 0.5);
    assert!((region_source_secs(&slow, 1.0) - 0.5).abs() < 1e-5);
}

/// En slingad kloss behåller sin period i ut-tid men läser mer källjud per
/// varv när den är sträckt — annars hade slingan spelat fel stycke.
#[test]
fn a_looped_region_keeps_its_period_and_reads_a_longer_pass() {
    let looped = region_under_test(8.0, 0.0, 2.0, false, 2.0);
    // Perioden i ut-tid är 2 s: 2,5 s in i klossen är man 0,5 s in i varv två.
    assert!((region_source_secs(&looped, 2.5) - 1.0).abs() < 1e-5);
    // Ett helt varv täcker 4 s källjud (2 s × 2,0).
    assert!((region_source_secs(&looped, 1.999) - 3.998).abs() < 1e-3);
}

fn active_voices(synth: &SynthEngine) -> usize {
    synth.voices.iter().filter(|v| v.is_active()).count()
}

#[test]
fn a_delayed_note_fires_after_its_delay() {
    // Fas 6.4 steg 2: noten ska inte låta på steggränsen utan senare.
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::NoteOnDelayed {
        note: 64,
        freq: 329.63,
        velocity: 0.8,
        delay_samples: 200,
    });
    assert_eq!(synth.scheduled_notes.len(), 1);

    synth.process_stereo(); // räknar ned till 199
    assert_eq!(active_voices(&synth), 0, "noten ska inte ha låtit än");

    for _ in 0..200 {
        synth.process_stereo();
    }
    assert_eq!(active_voices(&synth), 1, "noten ska klinga efter fördröjningen");
    assert!(synth.scheduled_notes.is_empty());
}

#[test]
fn a_zero_delay_note_fires_immediately() {
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::NoteOnDelayed {
        note: 60,
        freq: 261.63,
        velocity: 0.8,
        delay_samples: 0,
    });
    synth.process_stereo();
    assert_eq!(active_voices(&synth), 1);
}

#[test]
fn a_scheduled_note_off_releases_the_note_after_its_length() {
    use crate::audio::envelope::EnvelopeStage;
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::NoteOn {
        note: 60,
        freq: 261.63,
        velocity: 0.8,
        channel: 1,
    });
    synth.handle_command(AudioCommand::NoteOffDelayed {
        note: 60,
        channel: 1,
        delay_samples: 100,
    });
    assert_eq!(synth.scheduled_note_offs.len(), 1, "släppet ska vänta i kön");

    for _ in 0..100 {
        synth.process_stereo();
    }
    assert_ne!(
        synth.voices[0].envelope.stage,
        EnvelopeStage::Release,
        "längden ska hålla noten kvar"
    );

    for _ in 0..2 {
        synth.process_stereo();
    }
    assert_eq!(synth.voices[0].envelope.stage, EnvelopeStage::Release);
    assert!(synth.scheduled_note_offs.is_empty());
}

#[test]
fn a_key_up_before_the_length_drops_the_scheduled_release() {
    use crate::audio::envelope::EnvelopeStage;
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::NoteOn {
        note: 60,
        freq: 261.63,
        velocity: 0.8,
        channel: 1,
    });
    synth.handle_command(AudioCommand::NoteOffDelayed {
        note: 60,
        channel: 1,
        delay_samples: 4_800,
    });
    synth.process_stereo();
    // Tangenten släpps långt före längden: släppet vinner, och det schemalagda tas bort —
    // annars hade noten fått två släpp och en omtriggad ton dött i förtid.
    synth.handle_command(AudioCommand::NoteOff { note: 60, channel: 1 });
    assert_eq!(synth.voices[0].envelope.stage, EnvelopeStage::Release);
    assert!(
        synth.scheduled_note_offs.is_empty(),
        "släppet ska ha tagit bort det schemalagda släppet"
    );
    for _ in 0..5_000 {
        synth.process_stereo();
    }
    assert!(synth.scheduled_note_offs.is_empty());
}

#[test]
fn strum_chord_schedules_notes_over_time() {
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::StrumChord {
        notes: vec![60, 64, 67],
        velocity: 0.8,
        start_samples: 0,
        spread_samples: 100,
        mode: 1,
    });
    assert_eq!(synth.scheduled_notes.len(), 3);

    // First note fires on the first processed frame.
    synth.process_stereo();
    assert_eq!(active_voices(&synth), 1);

    // Advance far enough for the remaining strummed notes.
    for _ in 0..250 {
        synth.process_stereo();
    }
    assert_eq!(active_voices(&synth), 3);
    assert!(synth.scheduled_notes.is_empty());
}

#[test]
fn strum_chord_block_mode_fires_together() {
    let mut synth = SynthEngine::new(48_000.0);
    synth.handle_command(AudioCommand::StrumChord {
        notes: vec![60, 64, 67, 71],
        velocity: 0.8,
        start_samples: 0,
        spread_samples: 500,
        mode: 0,
    });
    synth.process_stereo();
    assert_eq!(active_voices(&synth), 4);
}

#[test]
fn filter_env_zero_amount_is_transparent() {
    let adsr = AdsrParams::default();
    let flt = FilterParams { cutoff: 1200.0, resonance: 2.0 };
    let env = AdsrParams { attack: 0.001, decay: 0.05, sustain: 0.0, release: 0.05 };
    let other_env = AdsrParams { attack: 0.2, decay: 0.9, sustain: 0.5, release: 1.0 };
    let mut a = Voice::new(48_000.0);
    let mut b = Voice::new(48_000.0);
    a.trigger(60, 261.63, 1.0, 48_000.0);
    b.trigger(60, 261.63, 1.0, 48_000.0);
    for _ in 0..500 {
        let sa = a.next_sample(Waveform::Saw, &adsr, &flt, &env, 0.0, &crate::audio::command::FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
        let sb = b.next_sample(Waveform::Saw, &adsr, &flt, &other_env, 0.0, &crate::audio::command::FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
        assert!(
            (sa - sb).abs() < 1e-6,
            "amount 0 must ignore the filter envelope shape"
        );
    }
}

#[test]
fn filter_env_amount_changes_timbre() {
    let adsr = AdsrParams::default();
    let flt = FilterParams { cutoff: 400.0, resonance: 4.0 };
    let fenv = AdsrParams { attack: 0.001, decay: 0.2, sustain: 0.0, release: 0.2 };
    let mut dry = Voice::new(48_000.0);
    let mut wet = Voice::new(48_000.0);
    dry.trigger(60, 261.63, 1.0, 48_000.0);
    wet.trigger(60, 261.63, 1.0, 48_000.0);
    let mut diff = 0.0_f32;
    for _ in 0..2000 {
        let a = dry.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 0.0, &crate::audio::command::FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
        let b = wet.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 4.0, &crate::audio::command::FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
        diff += (a - b).abs();
    }
    assert!(diff > 1.0, "filter envelope should alter the timbre, diff={diff}");
}

#[test]
fn voices_have_independent_filter_envelopes() {
    let fenv = AdsrParams { attack: 0.5, decay: 0.5, sustain: 1.0, release: 0.5 };
    let adsr = AdsrParams::default();
    let flt = FilterParams::default();
    let mut first = Voice::new(48_000.0);
    let mut second = Voice::new(48_000.0);
    first.trigger(60, 261.63, 1.0, 48_000.0);
    for _ in 0..2000 {
        first.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 2.0, &crate::audio::command::FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
    }
    second.trigger(64, 329.63, 1.0, 48_000.0);
    let l1 = first.filter_env.current_level;
    let l2 = second.filter_env.current_level;
    assert!(
        l1 > l2,
        "earlier note's filter envelope must be further along: {l1} vs {l2}"
    );
}

use crate::audio::plugin_host_live::{
    DEFAULT_BLOCK_FRAMES, PluginInfo, PluginParameter, PluginProcessor,
};

struct SilentLatencyProcessor {
    info: PluginInfo,
    latency: u32,
}

impl PluginProcessor for SilentLatencyProcessor {
    fn backend(&self) -> &'static str {
        "Fake"
    }
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &[]
    }
    fn latency_frames(&self) -> u32 {
        self.latency
    }
    fn process_stereo(&mut self, _left: &mut [f32], _right: &mut [f32]) {}
    fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
        false
    }
    fn reset(&mut self) {}
}

/// **En plugin med en egen utbuss** (Fas 8.6), för att kunna mäta routningen utan en riktig
/// plugin. Huvudutgången är **tyst**: allt provet kan höra kommer från utbussen, så en buss som
/// läcker in i fel spår syns direkt. Bussens värde är en **räknare** — sample `n` bär talet `n` —
/// och det är den som gör en förskjutning mätbar: en kö som sackar efter en sample syns på
/// värdet, medan en konstant markör hade sett likadan ut hur fel den än var.
struct BusRampProcessor {
    info: PluginInfo,
    /// Nästa sampels värde. Både huvudutgången och bussen bär samma serie, så provet kan
    /// jämföra dem **mot varandra** — en kontroll på samma nivå — i stället för mot ett tal
    /// som bara stämmer om allt annat i kedjan är exakt 1,0.
    next: f32,
    /// Seriens första värde i det **pågående** blocket. `process_stereo` och
    /// `take_extra_output` får samma block, och ska därför bära samma tal.
    block_start: f32,
}

impl PluginProcessor for BusRampProcessor {
    fn backend(&self) -> &'static str {
        "Fake"
    }
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn parameters(&self) -> &[PluginParameter] {
        &[]
    }
    fn latency_frames(&self) -> u32 {
        0
    }
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.block_start = self.next;
        for (i, slot) in left.iter_mut().enumerate() {
            *slot = self.block_start + i as f32;
        }
        for (i, slot) in right.iter_mut().enumerate() {
            *slot = self.block_start + i as f32;
        }
        self.next += left.len() as f32;
    }
    fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
        false
    }
    fn reset(&mut self) {}
    fn extra_outputs(&self) -> usize {
        1
    }
    fn take_extra_output(&mut self, index: usize, left: &mut [f32], right: &mut [f32]) -> bool {
        if index != 0 {
            return false;
        }
        for (i, slot) in left.iter_mut().enumerate() {
            *slot = self.block_start + i as f32;
        }
        for (i, slot) in right.iter_mut().enumerate() {
            *slot = self.block_start + i as f32;
        }
        true
    }
}

/// Kör `samples` bildrutor med bussfixturen på spår 0 och returnerar
/// `(målspårets utgång, källspårets utgång)`. Med `routed = false` är kopplingen borta — samma
/// projekt i övrigt, alltså är skillnaden kopplingens och ingenting annats.
fn run_bus_route(samples: usize, routed: bool) -> (f32, Vec<f32>, usize) {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    let insert = PluginInsert::new(
        Box::new(BusRampProcessor {
            info: PluginInfo::default(),
            next: 0.0,
            block_start: 0.0,
        }),
        DEFAULT_BLOCK_FRAMES,
    );
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: Some(insert),
    });
    if routed {
        synth.handle_command(AudioCommand::SetPluginExtraOutputs {
            track_index: 0,
            targets: vec![Some(1)],
        });
    }
    let mut source_hist = Vec::with_capacity(samples);
    for _ in 0..samples {
        synth.process_stereo();
        source_hist.push(synth.stem_tracks[0].last_out_l);
    }
    (
        synth.stem_tracks[1].last_out_l,
        source_hist,
        synth.stem_tracks[1].pdc.delay(),
    )
}

/// **En pluginbuss når sitt eget spår — och bara dit** (Fas 8.6).
///
/// Målspåret är tyst i sig självt, så allt som hörs därifrån är bussen. Provet kör samma
/// projekt två gånger, med och utan kopplingen: utan den ska målspåret vara **tyst**, med den
/// ska det bära bussens värde. Att bara pröva att den kom fram vore halva provet — en buss som
/// läcker in i källspåret låter också.
#[test]
fn a_plugin_extra_bus_reaches_its_own_track() {
    // Längre än målspårets PDC-fördröjning (en blocklängd) **och** förbi insättningens egen
    // blockfördröjning: källans ljud börjar vid bildruta 128, så jämförelsen måste ligga efter
    // det — annars jämför provet mot tystnaden före musiken.
    let samples = 600;
    let (off_target, off_hist, _d) = run_bus_route(samples, false);
    let (on_target, on_hist, target_delay) = run_bus_route(samples, true);

    assert!(
        off_target.abs() < 1e-6,
        "utan koppling ska målspåret vara tyst, inte {off_target}"
    );
    // Målspåret kompenseras som alla andra spår: dess egen PDC-fördröjning ligger på allt som
    // går genom kedjan, också bussen. Alltså är rätt jämförelse bussens sample mot källans
    // ljud **så många bildrutor tidigare** — inte mot källans samtidiga värde.
    let expected = on_hist[samples - 1 - target_delay];
    assert!(expected.abs() > 1.0, "kontrollen måste bära musik: {expected}");
    assert!(
        (on_target - expected).abs() < 1e-3,
        "bussen ska nå målspåret i fas: {on_target} mot källans {expected} \
         ({target_delay} bildrutor tidigare)"
    );
    // Källspåret självt får inte påverkas av att bussen kopplas in: bussen får inte vända
    // tillbaka in i sitt eget spår.
    assert_eq!(
        on_hist, off_hist,
        "källspårets ljud ändrades av kopplingen"
    );
}

/// **Bussen sackar inte efter** (Fas 8.6). Köer läses en sample i taget; missas en läsning
/// hamnar bussen en sample fel och felet **växer**. Provet kör länge och jämför värdet mot
/// bildrutenumret, så en förskjutning i blockstorlek syns som hundratals sample.
#[test]
fn a_plugin_extra_bus_does_not_drift() {
    let samples = 3_000;
    let (target, hist, target_delay) = run_bus_route(samples, true);
    let expected = hist[samples - 1 - target_delay];
    assert!(expected.abs() > 2_000.0, "kontrollen måste bära musik: {expected}");
    assert!(
        (target - expected).abs() < 1e-3,
        "bussen har glidit ifrån källans ljud efter {samples} bildrutor: {target} mot {expected}"
    );
}
fn load_impulse_tracks(synth: &mut SynthEngine) {
    let impulse = Arc::new(vec![1.0_f32; 8]);
    let silent = Arc::new(vec![0.0_f32; 8]);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: impulse.clone(),
        right: impulse,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 1,
        left: silent.clone(),
        right: silent,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::SetSongPlayback(true));
}

#[test]
fn pdc_aligns_non_plugin_tracks_to_plugin_latency() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    let insert = PluginInsert::new(
        Box::new(SilentLatencyProcessor {
            info: PluginInfo::default(),
            latency: 20,
        }),
        DEFAULT_BLOCK_FRAMES,
    );
    let expected = insert.latency_frames();
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: Some(insert),
    });
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
    assert_eq!(synth.stem_tracks[1].pdc.delay(), expected);
}

#[test]
fn pdc_is_a_passthrough_without_plugins() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
    assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
}

#[test]
fn clearing_a_track_plugin_removes_its_pdc() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    let insert = PluginInsert::new(
        Box::new(SilentLatencyProcessor {
            info: PluginInfo::default(),
            latency: 12,
        }),
        DEFAULT_BLOCK_FRAMES,
    );
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: Some(insert),
    });
    synth.process_stereo();
    assert!(synth.stem_tracks[1].pdc.delay() > 0);
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: None,
    });
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
    assert!(synth.stem_tracks[0].plugin.is_none());
}

/// **Manuellt latens-offset: spåret fördröjs, och de andra följer med** (Fas 8.6).
///
/// Offsetet finns för den plugin som rapporterar fel — rapporterar den 0 men fördröjer 8
/// ramar är det bara en människa som kan veta det. Provet mäter att offsetet hamnar i
/// **både** spårets egen latens och i projektets maxvärde: spår 1 (utan plugin) ska
/// kompenseras för spår 0:s plugin **och** dess offset.
#[test]
fn a_manual_latency_offset_moves_the_track_and_the_others_follow() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    let insert = PluginInsert::new(
        Box::new(SilentLatencyProcessor {
            info: PluginInfo::default(),
            latency: 20,
        }),
        DEFAULT_BLOCK_FRAMES,
    );
    // Basen läses ur **inserten**, inte ur en hårdkodad 20: `latency_frames()` bär både
    // pluginens egen rapport och värdenas blockbuffert, och ett prov som räknar med bara
    // den ena prövar sin egen aritmetik i stället för motorn.
    let base = insert.latency_frames();
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: Some(insert),
    });
    synth.handle_command(AudioCommand::SetPluginLatencyOffset {
        track_index: 0,
        frames: 8,
    });
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0, "spåret självt är referensen");
    assert_eq!(synth.stem_tracks[1].pdc.delay(), base + 8);
}

/// **Ett negativt offset drar fram spåret** i stället: dess totala latens blir mindre, och
/// då är det de andra spåren som får delayen. Samma mekanik som ett positivt offset — men
/// det är den här riktningen som är lätt att få bakvänd.
#[test]
fn a_negative_offset_pulls_the_track_forward() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    let insert = PluginInsert::new(
        Box::new(SilentLatencyProcessor {
            info: PluginInfo::default(),
            latency: 20,
        }),
        DEFAULT_BLOCK_FRAMES,
    );
    // −12 ramar **mindre** än vad spåret har: projektets max blir base − 12, spår 0 behöver
    // ingen egen delay, och spår 1 kompenseras för det som är kvar.
    let base = insert.latency_frames();
    synth.handle_command(AudioCommand::SetTrackPlugin {
        track_index: 0,
        insert: Some(insert),
    });
    synth.handle_command(AudioCommand::SetPluginLatencyOffset {
        track_index: 0,
        frames: -12,
    });
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
    assert_eq!(synth.stem_tracks[1].pdc.delay(), base - 12);
}

/// **Offsetet överlever en omladdning av spåret.** `LoadStemTrack` skickas vid varje
/// uppspelningsstart, så ett offset som nollades där skulle bara höras som att spåret gled
/// igen varje gång man tryckte play — en tyst förlust mitt i en vanlig arbetsgång.
#[test]
fn a_manual_offset_survives_a_track_reload() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    synth.handle_command(AudioCommand::SetPluginLatencyOffset {
        track_index: 0,
        frames: 8,
    });
    let impulse = Arc::new(vec![1.0_f32; 8]);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: impulse.clone(),
        right: impulse,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    assert_eq!(synth.stem_tracks[0].manual_latency_frames, 8);
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[1].pdc.delay(), 8, "de andra kompenseras fortfarande");
}

#[test]
fn pitch_shift_engages_shifter_and_compensates_latency() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    synth.handle_command(AudioCommand::SetTrackMix {
        track_index: 0,
        comp_threshold_db: 0.0,
        comp_ratio: 1.0,
        reverb_send: 0.0,
        delay_send: 0.0,
        pitch_semitones: 12.0,
    });
    assert!(synth.stem_tracks[0].pitch_active);
    assert!((synth.stem_tracks[0].pitch_semitones - 12.0).abs() < 1e-6);
    let latency = synth.stem_tracks[0].pitch_shifter.latency_frames();
    assert!(latency > 0, "engaged shifter must report latency");
    synth.process_stereo();
    // The shifted track defines the maximum latency; the other is delayed.
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
    assert_eq!(synth.stem_tracks[1].pdc.delay(), latency);
}

#[test]
fn pitch_shift_zero_bypasses_shifter() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    synth.handle_command(AudioCommand::SetTrackMix {
        track_index: 0,
        comp_threshold_db: 0.0,
        comp_ratio: 1.0,
        reverb_send: 0.0,
        delay_send: 0.0,
        pitch_semitones: 0.0,
    });
    assert!(!synth.stem_tracks[0].pitch_active);
    synth.process_stereo();
    assert_eq!(synth.stem_tracks[0].pdc.delay(), 0);
    assert_eq!(synth.stem_tracks[1].pdc.delay(), 0);
}

#[test]
fn pitch_shift_survives_stem_reload() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_tracks(&mut synth);
    synth.handle_command(AudioCommand::SetTrackMix {
        track_index: 0,
        comp_threshold_db: 0.0,
        comp_ratio: 1.0,
        reverb_send: 0.0,
        delay_send: 0.0,
        pitch_semitones: -5.0,
    });
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: Arc::new(vec![1.0_f32; 8]),
        right: Arc::new(vec![1.0_f32; 8]),
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    assert!(synth.stem_tracks[0].pitch_active);
    assert!((synth.stem_tracks[0].pitch_semitones + 5.0).abs() < 1e-6);
}

// -- Fas 5.2: sub-mix bussar & VCA-grupper -------------------------------

fn route_track(synth: &mut SynthEngine, track: usize, bus: usize, vca: Option<usize>) {
    synth.handle_command(AudioCommand::SetStemTrackRouting {
        track_index: track,
        bus,
        vca,
    });
}

fn load_impulse_and_silence(synth: &mut SynthEngine) {
    // A 220 Hz tone (not DC): the master chain high-passes DC away, so a
    // constant buffer would read as silence regardless of group gain.
    let tone: Vec<f32> = (0..8192)
        .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.8)
        .collect();
    let impulse = Arc::new(tone);
    let silent = Arc::new(vec![0.0_f32; 8192]);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: impulse.clone(),
        right: impulse,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 1,
        left: silent.clone(),
        right: silent,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::SetSongPlayback(true));
}

/// Sums |L|+|R| over `frames` stereo frames.
fn output_energy(synth: &mut SynthEngine, frames: usize) -> f32 {
    let mut energy = 0.0_f32;
    for _ in 0..frames {
        let (l, r) = synth.process_stereo();
        energy += l.abs() + r.abs();
    }
    energy
}
#[test]
fn bus_volume_scales_group_output() {
    let mut unity = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut unity);
    route_track(&mut unity, 0, 0, None);
    let full = output_energy(&mut unity, 2048);

    let mut halved = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut halved);
    route_track(&mut halved, 0, 0, None);
    halved.handle_command(AudioCommand::SetBusState {
        bus: 0,
        volume: 0.5,
        muted: false,
        solo: false,
    });
    let half = output_energy(&mut halved, 2048);

    assert!(full > 0.05, "expected audible bus output, got {full}");
    assert!(
        half > 0.0 && half < full,
        "bus gain must attenuate: {half} vs {full}"
    );
}

#[test]
fn bus_mute_silences_group() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut synth);
    route_track(&mut synth, 0, 2, None);
    synth.handle_command(AudioCommand::SetBusState {
        bus: 2,
        volume: 1.0,
        muted: true,
        solo: false,
    });
    assert!(output_energy(&mut synth, 2048) < 1e-4, "muted bus must be silent");
}

#[test]
fn bus_solo_isolates_other_buses() {
    let mut reference = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut reference);
    route_track(&mut reference, 0, 0, None);
    route_track(&mut reference, 1, 1, None);
    assert!(output_energy(&mut reference, 2048) > 0.05, "reference should be audible");

    let mut soloed = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut soloed);
    route_track(&mut soloed, 0, 0, None);
    route_track(&mut soloed, 1, 1, None);
    soloed.handle_command(AudioCommand::SetBusState {
        bus: 1,
        volume: 1.0,
        muted: false,
        solo: true,
    });
    assert!(
        output_energy(&mut soloed, 2048) < 1e-4,
        "soloing the silent bus must silence the non-soloed impulse bus"
    );
}

#[test]
fn vca_volume_scales_group_output() {
    let mut unity = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut unity);
    route_track(&mut unity, 0, 0, Some(2));
    let full = output_energy(&mut unity, 2048);

    let mut halved = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut halved);
    route_track(&mut halved, 0, 0, Some(2));
    halved.handle_command(AudioCommand::SetVcaState {
        vca: 2,
        volume: 0.5,
        muted: false,
        solo: false,
    });
    let half = output_energy(&mut halved, 2048);

    assert!(full > 0.05, "expected audible VCA output, got {full}");
    assert!(
        half > 0.0 && half < full,
        "VCA gain must attenuate: {half} vs {full}"
    );
}

#[test]
fn vca_mute_silences_group() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut synth);
    route_track(&mut synth, 0, 0, Some(1));
    synth.handle_command(AudioCommand::SetVcaState {
        vca: 1,
        volume: 1.0,
        muted: true,
        solo: false,
    });
    assert!(output_energy(&mut synth, 2048) < 1e-4, "muted VCA must be silent");
}

#[test]
fn routing_clamps_out_of_range_assignments() {
    let mut synth = SynthEngine::new(48_000.0);
    load_impulse_and_silence(&mut synth);
    route_track(&mut synth, 0, 99, Some(99));
    assert_eq!(synth.stem_tracks[0].bus, NUM_BUSES - 1);
    assert_eq!(synth.stem_tracks[0].vca, None);
    // Out-of-range bus/VCA state must clamp to the last group, not panic.
    synth.handle_command(AudioCommand::SetBusState {
        bus: 99,
        volume: 0.3,
        muted: false,
        solo: false,
    });
    synth.handle_command(AudioCommand::SetVcaState {
        vca: 99,
        volume: 0.4,
        muted: false,
        solo: false,
    });
    assert!((synth.bus_volume[NUM_BUSES - 1] - 0.3).abs() < 1e-6);
    assert!((synth.vca_volume[NUM_VCAS - 1] - 0.4).abs() < 1e-6);
}

// -- Fas 8.3: sidokedjor -------------------------------------------------

/// Energin vid **en** frekvens (Goertzel). Behövs för sidokedjetestet: nyckeln
/// och målet ligger på olika toner, och då mäter den här bara målet — annars
/// hade nyckelns egen ton drunknat i mätningen.
fn goertzel(buf: &[f32], freq: f32, sr: f32) -> f32 {
    let n = buf.len() as f32;
    let w = 2.0 * std::f32::consts::PI * freq / sr;
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for &x in buf {
        let s0 = x + 2.0 * w.cos() * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    ((s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).max(0.0)).sqrt() / n
}

/// Ett spår med en ren ton på `freq` i 3 s — samma form som
/// `load_impulse_and_silence` använder, men på valfri frekvens (en
/// likströmston hade high-passats bort) och lång nog för ett test som duckar,
/// släpper och mäter efteråt.
fn load_tone_track(synth: &mut SynthEngine, track_index: usize, freq: f32) {
    // 3 s: ett sidokedjetest hinner både ducka, släppa och mäta efteråt.
    let tone: Vec<f32> = (0..144_000)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / 48_000.0).sin() * 0.6)
        .collect();
    let arc = Arc::new(tone);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index,
        left: arc.clone(),
        right: arc,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
}

fn render_left(synth: &mut SynthEngine, frames: usize) -> Vec<f32> {
    (0..frames).map(|_| synth.process_stereo().0).collect()
}

/// Två spår: nyckeln på 220 Hz (spår 0) och målet på 880 Hz (spår 1).
/// Ett par spår för send-testerna: spår 0 bär ljudet, spår 1 är tomt.
fn send_pair() -> SynthEngine {
    let mut synth = SynthEngine::new(48_000.0);
    let one = Arc::new(vec![1.0_f32; 4800]);
    let silent = Arc::new(vec![0.0_f32; 4800]);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: one.clone(),
        right: one,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: Arc::new(vec![1.0_f32; 4800]),
        right: Arc::new(vec![1.0_f32; 4800]),
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    let _ = silent;
    synth.handle_command(AudioCommand::SetStemTrackRegions {
        track_index: 0,
        regions: vec![region_under_test(1.0, 0.0, 0.0, false, 1.0)],
    });
    synth.handle_command(AudioCommand::SetSongPlayback(true));
    synth
}

/// Mäter toppen ur ett par block.
fn run_peak(synth: &mut SynthEngine, frames: usize) -> f32 {
    let mut peak = 0.0f32;
    for _ in 0..frames {
        let (l, _r) = synth.process_stereo();
        peak = peak.max(l.abs());
    }
    peak
}

/// **Cue A bär bara det som skickats dit.** Två spår med olika toner (400 Hz och 1 200 Hz),
/// där bara det första har en cue-send till A. Lyssnar monitorerna på mixen hörs båda; på
/// cue A hörs bara 400 Hz. Exportmotorn har alltid `Main`, så en cue kan aldrig hamna i en fil.
#[test]
fn cue_a_carries_only_the_track_that_was_sent_there() {
    use crate::audio::control_room::CueListen;

    fn engine(listen: CueListen) -> SynthEngine {
        let mut synth = SynthEngine::new(48_000.0);
        let tone = |f: f32| {
            Arc::new(
                (0..48_000)
                    .map(|i| 0.5 * (std::f32::consts::TAU * f * i as f32 / 48_000.0).sin())
                    .collect::<Vec<f32>>(),
            )
        };
        for (idx, freq) in [(0usize, 400.0f32), (1, 1_200.0)] {
            let left = tone(freq);
            let right = left.clone();
            synth.handle_command(AudioCommand::LoadStemTrack {
                track_index: idx,
                left,
                right,
                sample_rate: 48_000.0,
                volume: 1.0,
                pan: 0.0,
                start_time_secs: 0.0,
            });
            synth.handle_command(AudioCommand::SetStemTrackRegions {
                track_index: idx,
                regions: vec![region_under_test(1.0, 0.0, 0.0, false, 1.0)],
            });
        }
        // Bara 400 Hz-spåret går till cue A.
        synth.handle_command(AudioCommand::SetCueSend {
            track_index: 0,
            a: 1.0,
            b: 0.0,
        });
        synth.handle_command(AudioCommand::SetCueListen(listen));
        synth.handle_command(AudioCommand::SetSongPlayback(true));
        synth
    }

    fn levels(listen: CueListen) -> (f32, f32) {
        let mut synth = engine(listen);
        let mut buf = Vec::new();
        for _ in 0..9_600 {
            let (l, _) = synth.process_stereo();
            buf.push(l);
        }
        let body = &buf[2_400..];
        (mag_at(body, 400.0), mag_at(body, 1_200.0))
    }

    let (main_low, main_high) = levels(CueListen::Main);
    assert!(
        main_low > 0.01 && main_high > 0.01,
        "mixen ska bära båda spåren: {main_low:.4} / {main_high:.4}"
    );

    let (cue_low, cue_high) = levels(CueListen::A);
    assert!(cue_low > 0.01, "cue A ska bära spåret som skickats dit: {cue_low:.4}");
    assert!(
        cue_high < cue_low * 0.02,
        "cue A ska inte bära spåret utan send: {cue_high:.4} mot {cue_low:.4}"
    );

    // Cue B har ingen send alls och ska vara tyst.
    let (b_low, b_high) = levels(CueListen::B);
    assert!(
        b_low < 0.001 && b_high < 0.001,
        "cue B ska vara tyst utan sends: {b_low:.4} / {b_high:.4}"
    );
    // Exporten ser alltid mastern: en färsk motor lyssnar på mixen.
    assert_eq!(SynthEngine::new(48_000.0).cue_listen, CueListen::Main);
}

/// **En send till en buss med lägre nivå hörs**: spåret går till sin egen buss
/// som förut, och en del går en parallell väg.
///
/// Att skicka till en buss med samma nivå som spårets egen hörs däremot inte —
/// summan är linjär — och det är därför målets nivå är nedsatt i testet: annars
/// mätte det ingenting.
#[test]
fn a_send_into_a_quieter_bus_adds_signal() {
    let base = {
        let mut synth = send_pair();
        run_peak(&mut synth, 8)
    };

    let mut synth = send_pair();
    synth.handle_command(AudioCommand::SetBusState {
        bus: 3,
        volume: 0.5,
        muted: false,
        solo: false,
    });
    synth.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 0,
        sends: vec![StemSend { target: SendTarget::bus(3), level: 1.0 }],
    });
    let with_send = run_peak(&mut synth, 8);

    assert!(
        with_send > base * 1.05,
        "senden ska höras: utan {base}, med {with_send}"
    );
}

/// En send till en **tystad** buss hörs inte: bussens mute gäller målet, och
/// därför blir det ingen extra signal.
#[test]
fn a_send_into_a_muted_bus_adds_nothing() {
    let base = {
        let mut synth = send_pair();
        synth.handle_command(AudioCommand::SetBusState {
            bus: 3,
            volume: 1.0,
            muted: true,
            solo: false,
        });
        run_peak(&mut synth, 8)
    };
    let mut synth = send_pair();
    synth.handle_command(AudioCommand::SetBusState {
        bus: 3,
        volume: 1.0,
        muted: true,
        solo: false,
    });
    synth.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 0,
        sends: vec![StemSend { target: SendTarget::bus(3), level: 1.0 }],
    });
    let with_send = run_peak(&mut synth, 8);
    assert!(
        (with_send - base).abs() < 1e-4,
        "en send till en tystad buss ska inte höras: {base} mot {with_send}"
    );
}

/// Målet kläms till de bussar som finns, nivån till 0..2, och en send utan nivå
/// filtreras bort — ett projektfält kan innehålla vad som helst.
#[test]
fn a_send_is_clamped_to_the_buses_that_exist() {
    let mut synth = send_pair();
    synth.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 0,
        sends: vec![
            StemSend { target: SendTarget::bus(99), level: 9.0 },
            StemSend { target: SendTarget::bus(1), level: 0.0 },
            StemSend { target: SendTarget::bus(2), level: f32::NAN },
        ],
    });
    let sends = &synth.stem_tracks[0].sends;
    assert_eq!(sends.len(), 1, "bara den med en nivå ska bli kvar: {sends:?}");
    assert_eq!(sends[0].target.as_bus(), Some(NUM_BUSES - 1));
    assert!((sends[0].level - 2.0).abs() < 1e-6);
}

fn sidechain_pair() -> SynthEngine {
    let mut synth = SynthEngine::new(48_000.0);
    load_tone_track(&mut synth, 0, 220.0);
    load_tone_track(&mut synth, 1, 880.0);
    synth.handle_command(AudioCommand::SetSongPlayback(true));
    synth
}

/// Duckarens egen matematik: öppen under tröskeln, ned mot `amount_db` över
/// den, och tillbaka igen när nyckeln tystnar.
#[test]
fn a_ducker_stays_open_below_the_threshold_and_closes_above_it() {
    let mut d = Ducker::new(48_000.0);
    for _ in 0..4_800 {
        assert!((d.process(0.0, 0.0, -30.0, 12.0) - 1.0).abs() < 1e-6, "tyst nyckel ska inte ducka");
    }
    let mut gain = 1.0;
    for _ in 0..4_800 {
        gain = d.process(0.9, 0.9, -30.0, 12.0);
    }
    // −12 dB är 10^(−12/20) = 0.251. Mjuk övergång över 6 dB gör att en nyckel
    // 30 dB över tröskeln ligger vid full duckning.
    assert!(gain < 0.30 && gain > 0.20, "gainen ska ned mot −12 dB, fick {gain}");

    // Nyckeln tystnar → duckaren öppnar igen (release 120 ms).
    for _ in 0..96_000 {
        gain = d.process(0.0, 0.0, -30.0, 12.0);
    }
    assert!(gain > 0.99, "duckaren ska ha öppnat igen, fick {gain}");
}

/// Noll duckning ska vara samma sak som ingen duckning — gainen ska aldrig
/// röra signalen.
#[test]
fn a_zero_amount_ducker_never_touches_the_signal() {
    let mut d = Ducker::new(48_000.0);
    for _ in 0..4_800 {
        assert!((d.process(1.0, 1.0, -30.0, 0.0) - 1.0).abs() < 1e-4);
    }
}

/// **Det som hörs:** spår 1 (880 Hz) duckas av spår 0 (220 Hz). Energin vid
/// 880 Hz mäts med och utan sidokedja — nyckelns egen ton ligger på en annan
/// frekvens, så den blandas inte in.
#[test]
fn a_sidechain_ducks_the_target_track() {
    let own = {
        let mut synth = sidechain_pair();
        let buf = render_left(&mut synth, 48_000);
        goertzel(&buf[9_600..], 880.0, 48_000.0)
    };
    let ducked = {
        let mut synth = sidechain_pair();
        synth.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: 1,
            from: Some(0),
            amount_db: 18.0,
            threshold_db: -30.0,
        });
        let buf = render_left(&mut synth, 48_000);
        goertzel(&buf[9_600..], 880.0, 48_000.0)
    };

    assert!(own > 0.0, "målspåret ska höras utan sidokedja");
    assert!(
        ducked < own * 0.5,
        "880 Hz ska ha dämpats av nyckeln: {own} → {ducked}"
    );
    assert!(ducked > 0.0, "dämpat, inte tystat");
    // Och nyckeln själv ska vara kvar på sin frekvens.
    let mut synth = sidechain_pair();
    synth.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 1,
        from: Some(0),
        amount_db: 18.0,
        threshold_db: -30.0,
    });
    let buf = render_left(&mut synth, 48_000);
    assert!(goertzel(&buf[9_600..], 220.0, 48_000.0) > 0.01, "nyckeln ska höras som förut");
}

/// Ett `from` som pekar på spåret självt är en slinga och ignoreras — både när
/// det sätts och om spårlistan ändras efteråt. Ett `from` utanför listan duckar
/// ingen i stället för att få motorn att gå utanför bufferten.
#[test]
fn a_sidechain_never_points_at_the_track_itself_or_outside_the_list() {
    let mut synth = sidechain_pair();
    synth.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 0,
        from: Some(0),
        amount_db: 12.0,
        threshold_db: -30.0,
    });
    assert_eq!(synth.stem_tracks[0].sidechain_from, None, "självreferens ska inte sparas");

    synth.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 1,
        from: Some(99),
        amount_db: 12.0,
        threshold_db: -30.0,
    });
    assert_eq!(synth.stem_tracks[1].sidechain_from, Some(99));
    let buf = render_left(&mut synth, 4_800);
    assert!(buf.iter().all(|s| s.is_finite()), "utanför listan: ingen panik, inget NaN");
}

/// En sidokedja som **inte** är kopplad får inte färga signalen alls: samma
/// motor, samma spår, en med ett uttryckligt `from: None` och en utan kommandot
/// — utsignalerna ska vara bitvis lika.
#[test]
fn an_unset_sidechain_leaves_the_signal_untouched() {
    let mut plain = sidechain_pair();
    let mut unset = sidechain_pair();
    unset.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 1,
        from: None,
        amount_db: 24.0,
        threshold_db: -40.0,
    });

    let mut identical = true;
    for _ in 0..10_000 {
        let (a, _) = plain.process_stereo();
        let (b, _) = unset.process_stereo();
        if (a - b).abs() > 1e-9 {
            identical = false;
            break;
        }
    }
    assert!(identical, "utan kopplad sidokedja ska signalen vara orörd");
}

/// Och när en kopplad sidokedja tas bort ska nivån tillbaka. Här mäts **nivå**,
/// inte bitvis likhet: master-kedjans dynamik har minne av den duckade
/// perioden, så två motorer med olika historia kan inte jämföras sample för
/// sample. Nivån är det användaren hör.
#[test]
fn removing_a_sidechain_lets_the_level_come_back() {
    // Referensen: samma spår, ingen sidokedja alls, samma mätfönster.
    let mut plain = sidechain_pair();
    let plain_buf = render_left(&mut plain, 48_000);
    let expected = goertzel(&plain_buf[9_600..24_000], 880.0, 48_000.0);

    let mut synth = sidechain_pair();
    synth.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 1,
        from: Some(0),
        amount_db: 24.0,
        threshold_db: -40.0,
    });
    let ducked_buf = render_left(&mut synth, 48_000);
    let ducked = goertzel(&ducked_buf[9_600..24_000], 880.0, 48_000.0);

    synth.handle_command(AudioCommand::SetStemTrackSidechain {
        track_index: 1,
        from: None,
        amount_db: 24.0,
        threshold_db: -40.0,
    });
    // 0,5 s: duckarens release är 120 ms, och master-kedjan ska hinna med.
    // Räkningen måste stämma med källtonens längd (3 s = 144 000 samples):
    // 48 000 (duckat) + 24 000 (återhämtning) + 24 000 (mätning) = 96 000.
    let _ = render_left(&mut synth, 24_000);
    let after_buf = render_left(&mut synth, 24_000);
    let restored = goertzel(&after_buf, 880.0, 48_000.0);

    assert!(
        ducked < expected * 0.6,
        "före borttagningen ska nivån vara dämpad: {ducked} mot {expected}"
    );
    assert!(
        restored > expected * 0.8,
        "efter borttagningen ska nivån tillbaka: {restored} mot {expected}"
    );
}

// -- Fas 8.3: spår-till-spår-sends ----------------------------------------

/// Två spår: **spår 0 tar emot** (tyst buffert), **spår 1 sänder** (en 1 kHz-ton).
/// Spår 1 skickar sin utgång till spår 0 på nivån `level`. Båda ligger på buss 0, och
/// mottagaren är tyst — därför är utgången sändarens egen väg, plus senden när den är
/// inkopplad.
fn sender_and_receiver(level: f32) -> SynthEngine {
    let mut synth = SynthEngine::new(48_000.0);
    let silent = Arc::new(vec![0.0_f32; 144_000]);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 0,
        left: silent.clone(),
        right: silent,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    let tone: Vec<f32> = (0..144_000)
        .map(|i| (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 48_000.0).sin() * 0.6)
        .collect();
    let arc = Arc::new(tone);
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index: 1,
        left: arc.clone(),
        right: arc,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 1,
        sends: vec![StemSend {
            target: SendTarget::track(0),
            level,
        }],
    });
    synth.handle_command(AudioCommand::SetSongPlayback(true));
    synth
}

/// En 1 kHz-ton på 3 s — samma buffer som testerna ovan använder.
fn tone_buffer() -> Vec<f32> {
    (0..144_000)
        .map(|i| (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 48_000.0).sin() * 0.6)
        .collect()
}

/// Lägger ett spår på en buss med given buffert.
fn load_on_bus(synth: &mut SynthEngine, track_index: usize, bus: usize, buffer: Arc<Vec<f32>>) {
    synth.handle_command(AudioCommand::LoadStemTrack {
        track_index,
        left: buffer.clone(),
        right: buffer,
        sample_rate: 48_000.0,
        volume: 1.0,
        pan: 0.0,
        start_time_secs: 0.0,
    });
    synth.handle_command(AudioCommand::SetStemTrackRouting {
        track_index,
        bus,
        vca: None,
    });
}

/// **Fasen är hela poängen med en spår-send** (Fas 8.3): senden ska komma fram i
/// **samma sample** som sin källa, inte i nästa.
///
/// Provet jämför en send mot en **kontroll på samma nivå**: mottagaren (buss 1) får sin
/// egen ton *plus* en send med samma ton, och kontrollen har i stället en buffert som
/// redan är dubbelt så stark. Ligger senden rätt är de två renderingarna samma sak —
/// **sample för sample**. Kommer senden en sample sent blir skillnaden i stället
/// `x[n] − x[n−1]` ≈ 0,08 vid 1 kHz, alltså tusentals gånger över toleransen.
///
/// Sändarens egen buss skruvas ned till noll, så att det bara är mottagarens utgång som
/// hörs — senden tappas **före** bussnivån och påverkas inte.
#[test]
fn a_track_send_arrives_in_phase() {
    let tone = tone_buffer();
    let doubled: Vec<f32> = tone.iter().map(|v| v * 2.0).collect();

    let mut send = SynthEngine::new(48_000.0);
    load_on_bus(&mut send, 0, 1, Arc::new(tone.clone())); // mottagaren
    load_on_bus(&mut send, 1, 0, Arc::new(tone.clone())); // sändaren
    send.handle_command(AudioCommand::SetBusState {
        bus: 0,
        volume: 0.0,
        muted: false,
        solo: false,
    });
    send.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 1,
        sends: vec![StemSend {
            target: SendTarget::track(0),
            level: 1.0,
        }],
    });
    send.handle_command(AudioCommand::SetSongPlayback(true));

    let mut control = SynthEngine::new(48_000.0);
    load_on_bus(&mut control, 0, 1, Arc::new(doubled));
    control.handle_command(AudioCommand::SetSongPlayback(true));

    let a = render_left(&mut send, 2_000);
    let b = render_left(&mut control, 2_000);
    let largest = b.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
    assert!(largest > 0.5, "kontrollen ska låta (största {largest})");
    for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            (x - y).abs() < 1e-5,
            "sample {i}: senden gav {x}, kontrollen på samma nivå {y} — senden kom inte i fas"
        );
    }
}

/// ... och motprovet: samma uppställning **utan** send ger en helt annan kurva, så att
/// provet ovan inte kan bli grönt av att ingenting skickas.
#[test]
fn the_send_path_is_silent_without_the_send() {
    let mut synth = SynthEngine::new(48_000.0);
    load_on_bus(&mut synth, 0, 1, Arc::new(tone_buffer()));
    load_on_bus(&mut synth, 1, 0, Arc::new(tone_buffer()));
    synth.handle_command(AudioCommand::SetBusState {
        bus: 0,
        volume: 0.0,
        muted: false,
        solo: false,
    });
    synth.handle_command(AudioCommand::SetSongPlayback(true));
    let out = render_left(&mut synth, 2_000);
    let largest = out.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
    assert!(
        largest > 0.2,
        "mottagarens egen ton ska höras (största {largest}) — annars är send-vägen tyst av fel skäl"
    );
}

/// Ordningen syns i motorn: en send bakåt i spårlistan vänder den. (Det hörbara
/// beviset är fasprovet ovan; det här visar mekanismen när något går fel.)
#[test]
fn the_order_follows_the_sends() {
    // Spår 1 → spår 0: sändaren ligger *efter* mottagaren i listan och måste därför
    // räknas först — ordningen vänds.
    let mut synth = sender_and_receiver(0.5);
    assert_eq!(
        synth.stem_order,
        vec![1, 0],
        "en send bakåt i listan ska vända ordningen"
    );
    // Sedan en send tillbaka: nu är det en slinga, och då behålls den förra ordningen.
    synth.handle_command(AudioCommand::SetStemTrackSends {
        track_index: 0,
        sends: vec![StemSend {
            target: SendTarget::track(1),
            level: 0.5,
        }],
    });
    assert_eq!(
        synth.stem_order,
        vec![1, 0],
        "0 → 1 plus 1 → 0 är en slinga: förra ordningen behålls, inget gissas"
    );
}

/// **Målets eget läge gäller** — samma regel som för buss-sendarna: en tystad mottagare
/// tar inte emot. Utan det hade en tystad kanal ändå låtit, via någon annans send.
#[test]
fn a_send_into_a_muted_receiver_adds_nothing() {
    let mut without = sender_and_receiver(0.0);
    let reference = render_left(&mut without, 1_000);

    let mut muted = sender_and_receiver(1.0);
    muted.handle_command(AudioCommand::SetStemTrackState {
        track_index: 0,
        volume: 1.0,
        pan: 0.0,
        muted: true,
        solo: false,
    });
    let out = render_left(&mut muted, 1_000);
    for (i, (&x, &y)) in reference.iter().zip(out.iter()).enumerate() {
        assert!((x - y).abs() < 1e-6, "sample {i}: en tystad mottagare tog emot en send");
    }
}

/// Ett mål som inte finns (eller ett spår som skickar till sig självt) får inte
/// återkoppla, och får inte heller ändra ljudet: regeln är "ingenting", inte "närmast".
#[test]
fn a_send_to_a_track_that_is_not_there_does_nothing() {
    let mut reference_synth = sender_and_receiver(0.0);
    let reference = render_left(&mut reference_synth, 1_000);

    for target in [SendTarget::track(9), SendTarget::track(1)] {
        let mut synth = sender_and_receiver(0.0);
        synth.handle_command(AudioCommand::SetStemTrackSends {
            track_index: 1,
            sends: vec![StemSend { target, level: 1.0 }],
        });
        let out = render_left(&mut synth, 1_000);
        for (i, (&x, &y)) in reference.iter().zip(out.iter()).enumerate() {
            assert!(
                (x - y).abs() < 1e-6,
                "mål {target:?}, sample {i}: ett mål som inte finns ska inte höras"
            );
        }
    }
}

// -- Fas 8.4: samplern ----------------------------------------------------

/// Triggar en kanal med ett känt ljud och givna sampler-inställningar.
///
/// Ljudet är en **ramp** på 4 000 ramar, alltså ett värde per ram som går att känna
/// igen: om rösten spelar "fel" ram syns det direkt i utgången, och en loop går att
/// räkna på.
fn trigger_sampler(
    loop_mode: LoopMode,
    loop_span01: (f32, f32),
    ping_pong: bool,
    hold_secs: f32,
    amp_env: AdsrParams,
) -> SynthEngine {
    // **Fullt anslag och full känslighet** = den linjära faktor velocityn alltid har haft,
    // alltså precis vad varje prov före anslagsratten prövade.
    trigger_sampler_velocity(loop_mode, loop_span01, ping_pong, hold_secs, amp_env, 1.0, 1.0, SamplerFilter::default(), crate::audio::envelope::VelocityCurve::Linear)
}

/// Samma som `trigger_sampler`, men med **anslaget** och **kanalens känslighet** satta
/// (Fas 8.4/7).
fn trigger_sampler_velocity(
    loop_mode: LoopMode,
    loop_span01: (f32, f32),
    ping_pong: bool,
    hold_secs: f32,
    amp_env: AdsrParams,
    velocity: f32,
    velocity_sensitivity: f32,
    filter: SamplerFilter,
    velocity_curve: crate::audio::envelope::VelocityCurve,
) -> SynthEngine {
    let mut synth = SynthEngine::new(48_000.0);
    let n = 4_000usize;
    let ramp: Vec<f32> = (0..n).map(|i| i as f32 / n as f32 * 0.5).collect();
    let arc = Arc::new(ramp);
    synth.handle_command(AudioCommand::TriggerSampleVoice {
        left: arc.clone(),
        right: arc,
        sample_rate: 48_000,
        base_note: 60,
        note: 60, // samma not som basnoten: ingen transponering, steg = 1,0
        pitch_semitones: 0,
        pitch_cents: 0.0,
        velocity,
        velocity_sensitivity,
        velocity_curve,
        volume: 1.0,
        reverse: false,
        start01: 0.0,
        end01: 1.0,
        channel: 0,
        loop_mode,
        loop_start01: loop_span01.0,
        loop_end01: loop_span01.1,
        ping_pong,
        amp_env,
        filter,
        hold_secs,
        release: None,
    });
    let _ = synth.process_stereo();
    synth
}

/// **Panorering per operator når ända ut i mastern.** Operadens pan sätts till vänster;
/// då ska vänster kanal höras och höger vara tyst — genom röstens filter, envelop och FX.
#[test]
fn an_operator_pan_reaches_the_master() {
    let mut synth = SynthEngine::new(48_000.0);
    synth.reverb_params.mix = 0.0;
    synth.handle_command(AudioCommand::SetWaveform(crate::audio::synth::Waveform::SixOp));
    synth.handle_command(AudioCommand::SetSixOp {
        params: crate::audio::synth::SixOpParams {
            pan: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            ..crate::audio::synth::SixOpParams::default()
        },
    });
    synth.handle_command(AudioCommand::NoteOn {
        note: 60,
        freq: 261.63,
        velocity: 1.0,
        channel: 0,
    });

    let (mut peak_l, mut peak_r) = (0.0f32, 0.0f32);
    for _ in 0..4_000 {
        let (l, r) = synth.process_stereo();
        peak_l = peak_l.max(l.abs());
        peak_r = peak_r.max(r.abs());
    }
    assert!(peak_l > 0.05, "vänster ska höras: {peak_l:.4}");
    assert!(
        peak_r < peak_l * 0.02,
        "höger ska vara tyst när operatorn ligger helt till vänster: {peak_r:.5} mot {peak_l:.4}"
    );
}

/// **Tystnad ska vara tyst.** En motor utan noter får inte läcka brus — det var rapporterat
/// som brus vid uppspelning, och den enklaste mätningen är att räkna på en tom motor.
#[test]
fn an_idle_engine_is_silent() {
    let mut engine = SynthEngine::new(48_000.0);
    let mut peak = 0.0f32;
    for _ in 0..48_000 {
        let (l, r) = engine.process_stereo();
        peak = peak.max(l.abs()).max(r.abs());
    }
    assert!(peak < 1e-6, "tom motor ska vara tyst, fick {peak:.6}");
}

/// **En stereosampling behåller sina kanaler.** Vänster är en ramp, höger är tyst — förut
/// summerades de till mono, så höger fick samma ljud som vänster och bredden försvann.
#[test]
fn a_stereo_sample_keeps_its_channels() {
    let mut synth = SynthEngine::new(48_000.0);
    // Provet gäller samplingsvägen: reverben lägger en monovåt på båda kanalerna och skulle
    // annars dölja skillnaden vi mäter.
    synth.reverb_params.mix = 0.0;
    let n = 4_000usize;
    let ramp: Vec<f32> = (0..n).map(|i| i as f32 / n as f32 * 0.5).collect();
    synth.handle_command(AudioCommand::TriggerSampleVoice {
        left: std::sync::Arc::new(ramp),
        right: std::sync::Arc::new(vec![0.0; n]),
        sample_rate: 48_000,
        base_note: 60,
        note: 60,
        pitch_semitones: 0,
        pitch_cents: 0.0,
        velocity: 1.0,
        velocity_sensitivity: 1.0,
        velocity_curve: crate::audio::envelope::VelocityCurve::Linear,
        volume: 1.0,
        reverse: false,
        start01: 0.0,
        end01: 1.0,
        channel: 0,
        loop_mode: LoopMode::Off,
        loop_start01: 0.0,
        loop_end01: 1.0,
        ping_pong: false,
        amp_env: AdsrParams::default(),
        filter: SamplerFilter::default(),
        hold_secs: 0.0,
        release: None,
    });

    let (mut peak_l, mut peak_r) = (0.0f32, 0.0f32);
    // Rampen växer från 0, så fönstret måste vara en bit in i samplingen för att höras.
    for _ in 0..2_000 {
        let (l, r) = synth.process_stereo();
        peak_l = peak_l.max(l.abs());
        peak_r = peak_r.max(r.abs());
    }
    assert!(peak_l > 0.05, "vänster ska höras: {peak_l:.4}");
    assert!(peak_r < 0.001, "höger var tyst i samplingen: {peak_r:.6}");
}

/// Triggar en kanal med en **3 kHz-sin** (±0,5) — en ton mitt i registret, där ett lågpassfilter
/// hörs som skillnaden mellan "släpper igenom" och "dämpar".
///
/// **Proben var först ett växlande ljud (±0,5 varannan ram), och det var fel:** ett sådant ljud
/// ligger vid **Nyquist**, och i den här filterstrukturen (TPT/bilinjär) mappas digitala Nyquist
/// till **oändligheten** i förlagan — där är ett lågpass exakt noll, oavsett var cutoffen står.
/// Mätt: −78 dB vid 12 kHz, alltså dämpat även av ett "öppet" filter. En probe ska ligga i
/// passbandet eller stopbandet, inte i en punkt där varje svar är noll. En ramp (som
/// `trigger_sampler` använder) har sin energi i botten och säger nästan ingenting om ett filter.
fn trigger_sampler_filter(filter: SamplerFilter) -> SynthEngine {
    let mut synth = SynthEngine::new(48_000.0);
    let n = 4_000usize;
    let ton: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 3_000.0 * i as f32 / 48_000.0).sin())
        .collect();
    let arc = Arc::new(ton);
    synth.handle_command(AudioCommand::TriggerSampleVoice {
        left: arc.clone(),
        right: arc,
        sample_rate: 48_000,
        base_note: 60,
        note: 60,
        pitch_semitones: 0,
        pitch_cents: 0.0,
        velocity: 1.0,
        velocity_sensitivity: 1.0,
        velocity_curve: crate::audio::envelope::VelocityCurve::Linear,
        volume: 1.0,
        reverse: false,
        start01: 0.0,
        end01: 1.0,
        channel: 0,
        loop_mode: LoopMode::Off,
        loop_start01: 0.0,
        loop_end01: 1.0,
        ping_pong: false,
        amp_env: AdsrParams::identity(),
        filter,
        hold_secs: 0.0,
        release: None,
    });
    let _ = synth.process_stereo();
    synth
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn voice(synth: &SynthEngine) -> &SampleVoice {
    synth
        .sample_voices
        .iter()
        .find(|v| v.active)
        .expect("rösten ska vara igång")
}

/// **Avstängt är avstängt** (Fas 8.4/7): samma ljud två gånger, där den ena rösten har extrema
/// filtervärden men filtret **av**. Byte-identiskt — annars läcker inställningarna igenom ändå,
/// och ett projekt från före filtret låter inte som det gjorde. Identiteten är `on`, inte
/// siffrorna.
#[test]
fn a_filter_that_is_off_ignores_its_own_settings() {
    let mut av = trigger_sampler_filter(SamplerFilter::default());
    let mut av_men_installd = trigger_sampler_filter(SamplerFilter {
        on: false,
        cutoff_hz: 100.0,
        resonance: 9.0,
        env_amount_octaves: 6.0,
        ..SamplerFilter::default()
    });
    assert_eq!(
        render_left(&mut av, 512),
        render_left(&mut av_men_installd, 512),
        "ett filter som är av får inte påverka ljudet, vilka siffror det än står på"
    );
}

/// **Ett lågpassfilter dämpar det som ligger högt** (Fas 8.4/7), mätt på ett växlande ljud som
/// ligger vid Nyquist: 200 Hz ska dämpa det kraftigt mot 12 kHz.
#[test]
fn a_low_filter_darkens_the_sample() {
    let mut oppet = trigger_sampler_filter(SamplerFilter {
        on: true,
        cutoff_hz: 12_000.0,
        resonance: 0.707,
        env_amount_octaves: 0.0,
        ..SamplerFilter::default()
    });
    let mut stangt = trigger_sampler_filter(SamplerFilter {
        on: true,
        cutoff_hz: 200.0,
        resonance: 0.707,
        env_amount_octaves: 0.0,
        ..SamplerFilter::default()
    });
    let a = rms(&render_left(&mut oppet, 1_024));
    let b = rms(&render_left(&mut stangt, 1_024));
    // **Mätt:** 0,1153 genom det öppna filtret och 0,00092 genom det stängda — en kvot på 125
    // gånger, alltså −42 dB vid femton gånger cutoff. (Ingången är en 3 kHz-sin på ±0,5, RMS
    // 0,354; resten av vägen — panorering och master — ger en fast faktor, och den är samma för
    // båda renderingarna.)
    assert!(a > 0.05, "det öppna filtret ska släppa igenom 3 kHz ({a})");
    assert!(
        b < a * 0.05,
        "200 Hz ska dämpa 3 kHz kraftigt — två poler vid femton gånger cutoff: {b} mot {a}"
    );
}

/// **Envelopen sveper cutoffen över tiden** (Fas 8.4/7) — det som gör en "pluck". Provet kräver
/// att envelopens nivå läses **per sample**: hade cutoffen räknats ut en gång vid triggen vore
/// början och slutet av noten lika starka.
///
/// Fönstren ligger **inuti** ljudets 4 000 ramar (rösten tar slut med samplen), och attacken är
/// över efter 2 ms: 0,15 s decay vid 48 kHz är 7 200 ramar, så vid ram 3 200 har envelopen
/// fallit till ~0,55. Cutoffen går då från 150·2⁵ = 4 800 Hz (3 kHz ligger **under**: släpps
/// igenom) ned mot 150·2^(5·0,55) ≈ 1 kHz (3 kHz ligger **över**: dämpas).
#[test]
fn the_filter_envelope_sweeps_the_cutoff_over_time() {
    let mut synth = trigger_sampler_filter(SamplerFilter {
        on: true,
        cutoff_hz: 150.0,
        resonance: 0.707,
        env_amount_octaves: 5.0,
        env: AdsrParams {
            attack: 0.0,
            decay: 0.15,
            sustain: 0.0,
            release: 0.1,
        },
    });
    let out = render_left(&mut synth, 4_000);
    let tidig = rms(&out[120..520]);
    let sen = rms(&out[3_200..3_900]);
    assert!(
        tidig > sen * 3.0,
        "svepet syns inte: tidig {tidig}, sen {sen} (filtret öppnar och stänger)"
    );
}

/// **Anslaget hörs** (Fas 8.4/7): med full känslighet ger halv velocity halv nivå — samma
/// linjära faktor som `volume * velocity` alltid var, nu räknad **en gång** vid triggen och
/// buren av rösten. Provet jämför två renderingar på samma nivå, sample för sample.
#[test]
fn velocity_scales_the_sample_at_full_sensitivity() {
    let mut strong = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 1.0, 1.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let mut weak = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.5, 1.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let stark = render_left(&mut strong, 512);
    let svag = render_left(&mut weak, 512);
    // **Mätt, inte antaget:** kvoten är exakt 0,5 så länge mastern är linjär (de första
    // ramarna gav skillnaden 0,000000000), och böjer sig sedan — vid ram 500 är kvoten
    // 0,500056 och skillnaden 1,2·10⁻⁶. Det är **masterns** kurva som böjer, inte anslaget,
    // så provet kräver två saker: exakt halva i det linjära området, och svagare hela vägen.
    for i in 0..32 {
        assert!(
            (stark[i] * 0.5 - svag[i]).abs() < 1e-9,
            "ram {i}: mastern är linjär här, så halvt anslag ska ge exakt halva ({} → {})",
            stark[i],
            svag[i]
        );
    }
    for (i, (x, y)) in stark.iter().zip(svag.iter()).enumerate() {
        assert!(
            y.abs() <= x.abs() + 1e-9,
            "ram {i}: ett svagare anslag får aldrig bli starkare ({x} → {y})"
        );
    }
    assert!(
        svag.iter().any(|v| v.abs() > 1e-4),
        "ett halvt anslag ska fortfarande höras"
    );
}

/// **Känsligheten 0 är identiteten** (Fas 8.4/7): två helt olika anslag ger **byte-identisk**
/// utdata. Det är formen ett "det här får inte ändra ljudet"-prov ska ha — likhet, inte en
/// tolerans — för en känslighet som nästan är av är en ändring av ljudet.
/// **Kurvan når ljudet — och den är exakt** (Fas 8.4/7): vid halvt anslag är den kvadratiska
/// kurvans nivå **exakt hälften** av den rakas (0,25 mot 0,50), ram för ram.
///
/// Provet finns för att kurvan först var ett **val i gränssnittet utan en mätning**: en rullista
/// som skriver till ett fält ingen läser ser precis lika rätt ut som en som fungerar. Här mäts
/// skillnaden i **utdata från motorn**, med samma trigger och bara kurvan utbytt.
#[test]
fn the_squared_curve_reaches_the_audio_and_is_exactly_half() {
    let mut rak = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.5, 1.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let mut kvadrat = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.5, 1.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Squared,
    );
    let a = render_left(&mut rak, 512);
    let b = render_left(&mut kvadrat, 512);
    for i in 0..32 {
        assert!(
            (a[i] * 0.5 - b[i]).abs() < 1e-9,
            "ram {i}: 0,25 mot 0,50 ska ge exakt halva ({} → {})",
            a[i],
            b[i]
        );
    }
    assert!(
        b.iter().any(|v| v.abs() > 1e-4),
        "den kvadratiska kurvan ska fortfarande höras"
    );
    // Och den **raka** kurvan är oförändrad: samma utdata två gånger, alltså är det kurvan som
    // gör skillnaden och inte en sidoeffekt av triggen.
    let mut rak_igen = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.5, 1.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    assert_eq!(
        a,
        render_left(&mut rak_igen, 512),
        "den raka kurvan ska ge samma utdata varje gång"
    );
}

#[test]
fn zero_sensitivity_makes_the_velocity_inaudible() {
    let mut stark = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 1.0, 0.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let mut svag = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.25, 0.0,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let a = render_left(&mut stark, 512);
    let b = render_left(&mut svag, 512);
    assert_eq!(a, b, "känsligheten 0 ska göra anslaget ohörbart");
    assert!(
        a.iter().any(|v| v.abs() > 1e-4),
        "ljudet ska höras även när anslaget inte påverkar"
    );
}

/// **Kanalens känslighet når rösten** (Fas 8.4/7) — den ena änden mot den andra. Utan det
/// provet ser en ratt som skickas men aldrig läses precis ut som en som fungerar.
#[test]
fn the_channel_sensitivity_reaches_the_voice() {
    let mut synth = trigger_sampler_velocity(
        LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity(), 0.5, 0.5,
        SamplerFilter::default(),
        crate::audio::envelope::VelocityCurve::Linear,
    );
    let _ = synth.process_stereo();
    // (1 - 0,5) + 0,5 · 0,5 = 0,75 — halva anslaget får halva sin verkan.
    assert_eq!(voice(&synth).velocity_gain, 0.75);
}

/// **En-skottsprovspelaren är oförändrad** (Fas 8.4): med standardinställningarna
/// (inget loopläge, ingen envelop) tar rösten slut när ljudet tar slut — precis som
/// före samplern. Det är det provet som gör att ett sparat projekt från i går låter
/// likadant i dag.
#[test]
fn the_defaults_are_still_a_one_shot() {
    let mut synth = trigger_sampler(LoopMode::Off, (0.0, 1.0), false, 0.0, AdsrParams::identity());
    for _ in 0..4_100 {
        let _ = synth.process_stereo();
    }
    assert!(
        !synth.sample_voices.iter().any(|v| v.active),
        "en en-skottsröst ska ha tystnat efter ljudets slut"
    );
}

/// ... och en notlängd får **inte** klippa den: med identiteten som envelop finns
/// inget släpp att gå in i, så en en-skottsröst spelar sitt ljud till slutet även om
/// noten släpps på vägen.
#[test]
fn a_hold_does_not_cut_a_one_shot_without_an_envelope() {
    let mut synth = trigger_sampler(LoopMode::Off, (0.0, 1.0), false, 0.001, AdsrParams::identity());
    for _ in 0..2_000 {
        let _ = synth.process_stereo();
    }
    assert!(
        synth.sample_voices.iter().any(|v| v.active),
        "noten släpptes efter 1 ms, men utan envelop ska ljudet spela vidare"
    );
}

/// **Loopen är exakt periodisk** (Fas 8.4): rösten stannar kvar och positionen
/// upprepar sig med loopens längd — sample för sample, inte "ungefär".
#[test]
fn a_forever_loop_repeats_exactly() {
    // 4 000 ramar, loop 10 % .. 30 %. Perioden läses ur **rösten** — det är motorn som
    // äger spannet, och provet ska inte räkna om samma sak och sedan jämföra med sig
    // själv.
    let mut synth = trigger_sampler(LoopMode::Forever, (0.1, 0.3), false, 0.0, AdsrParams::identity());
    let (lo, hi) = voice(&synth).loop_span.expect("loopen ska finnas");
    assert!(hi - lo >= 1.0, "loopen ska vara minst en ram: {lo} .. {hi}");
    let period = (hi - lo).round() as usize;
    // Värm förbi loopens början: den *första* sträckan är inspelningen fram till
    // loopen, och den upprepar sig förstås inte.
    for _ in 0..(lo as usize + period) {
        let _ = synth.process_stereo();
    }
    let mut seen = Vec::new();
    for _ in 0..3 {
        let p = voice(&synth).pos;
        seen.push(p);
        for _ in 0..period {
            let _ = synth.process_stereo();
        }
    }
    assert!(synth.sample_voices.iter().any(|v| v.active), "loopen ska hålla rösten vid liv");
    for i in 1..seen.len() {
        assert!(
            (seen[i] - seen[i - 1]).abs() < 1.5,
            "varv {i}: positionen {seen:?} — loopen är inte periodisk"
        );
    }
}

/// **Not-av: `UntilRelease` spelar resten efter loopen** och tystnar sedan. Det är
/// skillnaden mot `Forever`, som loopar vidare förbi släppet.
#[test]
fn until_release_plays_the_tail_and_forever_does_not() {
    // Noten hålls 200 ramar, loopen ligger 10 % .. 30 %.
    let frames_hold = 200u32;
    let mut released = trigger_sampler(
        LoopMode::UntilRelease,
        (0.1, 0.3),
        false,
        frames_hold as f32 / 48_000.0,
        AdsrParams::identity(),
    );
    let mut forever = trigger_sampler(
        LoopMode::Forever,
        (0.1, 0.3),
        false,
        frames_hold as f32 / 48_000.0,
        AdsrParams::identity(),
    );
    for _ in 0..1_500 {
        let _ = released.process_stereo();
        let _ = forever.process_stereo();
    }
    let r = released.sample_voices.iter().find(|v| v.active);
    assert!(
        r.map(|v| v.pos > 1_199.0).unwrap_or(true),
        "efter not-avet ska rösten ha lämnat loopen och gått vidare"
    );
    let f = voice(&forever);
    assert!(
        (399.0..=1_199.5).contains(&f.pos),
        "Forever ska ligga kvar i loopen efter not-avet, men står på {}",
        f.pos
    );
}

/// **Ping-pong vänder i loopens ändar** i stället för att vrida tillbaka.
#[test]
fn ping_pong_turns_around() {
    let mut synth = trigger_sampler(LoopMode::Forever, (0.1, 0.3), true, 0.0, AdsrParams::identity());
    let mut directions = Vec::new();
    for _ in 0..2_000 {
        let _ = synth.process_stereo();
        if let Some(v) = synth.sample_voices.iter().find(|v| v.active) {
            directions.push(v.dir);
        }
    }
    assert!(
        directions.iter().any(|d| *d < 0.0) && directions.iter().any(|d| *d > 0.0),
        "ping-pong ska vända riktningen: {directions:?}"
    );
}

/// **En envelop gör rösten färdig**: efter not-avet fasar släppet ut den och rösten
/// tystnar när envelopen är klar — det är vad som gör en loopad not spelbar.
#[test]
fn an_envelope_ends_the_voice_after_the_release() {
    let env = AdsrParams {
        attack: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release: 0.005,
    };
    let mut synth = trigger_sampler(LoopMode::Forever, (0.1, 0.3), false, 0.001, env);
    for _ in 0..2_000 {
        let _ = synth.process_stereo();
    }
    assert!(
        !synth.sample_voices.iter().any(|v| v.active),
        "släppet är 5 ms — efter 2 000 ramar ska rösten ha tystnat"
    );
}

/// Amplituden hos en frekvens i en inspelad svit (en bin av en DFT).
// Hann-fönster: en rektangulär fönster på 50 ms läcker från grundtonen in i
// åttonde bin, och då mäter provet fönstret i stället för spektret.
fn hann_mag(buf: &[f32], freq: f32) -> f32 {
    let n = buf.len() as f32;
    let w = 2.0 * std::f32::consts::PI * freq / 48_000.0;
    let (mut re, mut im) = (0.0f32, 0.0f32);
    for (i, v) in buf.iter().enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n).cos();
        let ph = w * i as f32;
        re += v * hann * ph.cos();
        im += v * hann * ph.sin();
    }
    2.0 * (re * re + im * im).sqrt() / n
}
fn harmonic_centroid(buf: &[f32], f0: f32) -> f32 {
    let mut weighted = 0.0;
    let mut total = 0.0;
    for k in 1..=HARMONIC_COUNT {
        let m = hann_mag(buf, f0 * k as f32);
        weighted += m * k as f32;
        total += m;
    }
    if total <= 0.0 { 0.0 } else { weighted / total }
}

/// Grundtonen i Hz: scannar 60–200 Hz och tar toppen. Används där provet ska visa att
/// tonhöjden står stilla medan klangfärgen flyttas.
fn fundamental_hz(buf: &[f32]) -> f32 {
    let mut best = (0.0f32, 0.0f32);
    let mut f = 60.0;
    while f <= 200.0 {
        let m = hann_mag(buf, f);
        if m > best.1 {
            best = (f, m);
        }
        f += 0.5;
    }
    best.0
}

fn mag_at(x: &[f32], freq: f32) -> f32 {
    let sr = 48_000.0;
    let w = 2.0 * std::f32::consts::PI * freq / sr;
    let (mut re, mut im) = (0.0f32, 0.0f32);
    for (i, v) in x.iter().enumerate() {
        let ph = w * i as f32;
        re += v * ph.cos();
        im += v * ph.sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f32
}

#[test]
fn fm_at_zero_index_is_the_plain_oscillator() {
    // index 0 = ingen fasförskjutning: samma tal, inte "nästan samma".
    let adsr = AdsrParams::default();
    let flt = FilterParams { cutoff: 18_000.0, resonance: 0.7 };
    let env = AdsrParams::default();
    let mut a = Voice::new(48_000.0);
    let mut b = Voice::new(48_000.0);
    a.trigger(57, 220.0, 1.0, 48_000.0);
    b.trigger(57, 220.0, 1.0, 48_000.0);
    let fm = FmParams { ratio: 3.0, index: 0.0 };
    for i in 0..2_000 {
        let sa = a.next_sample(Waveform::Sine, &adsr, &flt, &env, 0.0, &fm, 0.0, 0.0, 0.0, 0.0, default_table());
        let sb = b.next_sample(Waveform::Sine, &adsr, &flt, &env, 0.0, &FmParams::default(), 0.0, 0.0, 0.0, 0.0, default_table());
        assert_eq!(sa, sb, "index 0 ska ge exakt samma sample (frame {i})");
    }
}

#[test]
fn fm_puts_sidebands_where_the_plain_oscillator_has_none() {
    // Bärvåg 110 Hz, modulator ratio 3 (330 Hz): sidobandet hamnar på 110+330 = 440 Hz,
    // där en ren sinuston inte har något alls. Det är skillnaden mellan FM och ingen FM.
    let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let flt = FilterParams { cutoff: 18_000.0, resonance: 0.7 };
    let env = AdsrParams::default();
    let render = |index: f32| {
        let mut v = Voice::new(48_000.0);
        v.trigger(45, 110.0, 1.0, 48_000.0);
        let fm = FmParams { ratio: 3.0, index };
        let mut out = Vec::with_capacity(9_600);
        for _ in 0..9_600 {
            out.push(v.next_sample(Waveform::Sine, &adsr, &flt, &env, 0.0, &fm, 0.0, 0.0, 0.0, 0.0, default_table()));
        }
        out
    };
    let plain = render(0.0);
    let modulated = render(4.0);
    let h_plain = mag_at(&plain[2_000..], 440.0);
    let h_fm = mag_at(&modulated[2_000..], 440.0);
    // Fönstret är inte ett heltal perioder av 440 Hz, så `h_plain` är läckage och inte noll —
    // därför mäts **kvoten**: sidobandet ska vara minst tio gånger starkare än läckaget.
    assert!(
        h_fm > 10.0 * h_plain,
        "sidobandet på 440 Hz ska dominera: utan FM {h_plain:.5}, med FM {h_fm:.5}"
    );
}

/// **Modulationen når fram till mastern.** En rad som drar ned volymen med senaste notens
/// anslag ska göra utfallet tystare — mätt på motorns egen utgång, inte på matrisen.
#[test]
fn a_velocity_slot_quiets_the_master_output() {
    use crate::audio::modulation::{ModSlot, ModSource, ModTarget};

    let mut plain = SynthEngine::new(48_000.0);
    let mut modulated = SynthEngine::new(48_000.0);
    modulated.modulation.set_slots(vec![ModSlot::new(ModSource::Velocity, ModTarget::Volume, -1.0)]);

    for engine in [&mut plain, &mut modulated] {
        engine.handle_command(AudioCommand::NoteOn { note: 60, freq: 261.63, velocity: 1.0, channel: 0 });
    }
    let mut peak_plain: f32 = 0.0;
    let mut peak_mod: f32 = 0.0;
    for _ in 0..4_800 {
        let (l, r) = plain.process_stereo();
        peak_plain = peak_plain.max(l.abs()).max(r.abs());
        let (l, r) = modulated.process_stereo();
        peak_mod = peak_mod.max(l.abs()).max(r.abs());
    }
    assert!(peak_plain > 0.01, "referensen ska höras: {peak_plain}");
    assert!(
        peak_mod < peak_plain * 0.35,
        "modulationen tog inte: {peak_mod} mot {peak_plain}"
    );
}

/// **Tonhöjdsmodulationen når rösterna.** En rad som drar upp tonhöjden en oktav med
/// senaste notens anslag ska göra utgången en oktav högre. Tonhöjden mäts med
/// autokorrelation, inte med nollgenomgångar: ett resonant filter lägger till
/// nollgenomgångar som inte hör till grundtonen.
#[test]
fn a_pitch_slot_transposes_the_oscillator() {
    use crate::audio::modulation::{ModSlot, ModSource, ModTarget};

    // Grundtonsskattning: den lag som ger högst normaliserad autokorrelation.
    let f0 = |samples: &[f32], sr: f32| -> f32 {
        let min_lag = (sr / 1200.0) as usize;
        let max_lag = (sr / 100.0) as usize;
        let mut best_lag = min_lag;
        let mut best = f32::NEG_INFINITY;
        for lag in min_lag..max_lag.min(samples.len() / 2) {
            let mut dot = 0.0f32;
            let mut energy = 0.0f32;
            for i in 0..samples.len() - lag {
                dot += samples[i] * samples[i + lag];
                energy += samples[i] * samples[i];
            }
            let score = dot / (energy.sqrt() * energy.sqrt() + 1e-9);
            if score > best {
                best = score;
                best_lag = lag;
            }
        }
        sr / best_lag as f32
    };

    let run = |modulated: bool| {
        let mut engine = SynthEngine::new(48_000.0);
        engine.filter_params.cutoff = 12_000.0;
        if modulated {
            engine
                .modulation
                .set_slots(vec![ModSlot::new(ModSource::Velocity, ModTarget::Pitch, 1.0)]);
        }
        engine.handle_command(AudioCommand::NoteOn { note: 57, freq: 220.0, velocity: 1.0, channel: 0 });
        let mut buf = Vec::new();
        for _ in 0..9_600 {
            let (l, _) = engine.process_stereo();
            buf.push(l);
        }
        // Hoppa över attacken: skattningen ska göras på det stationära partiet.
        f0(&buf[2_400..], 48_000.0)
    };

    let plain = run(false);
    let bent = run(true);
    assert!((plain - 220.0).abs() < 12.0, "referensen ska vara 220 Hz: {plain:.1}");
    let ratio = bent / plain;
    assert!(
        (ratio - 2.0).abs() < 0.12,
        "en oktav upp ska ge dubbel frekvens: {ratio:.2} ({plain:.1} Hz mot {bent:.1} Hz)"
    );
}

/// **Filtermodulationen når rösterna.** Samma not med ett lågt stängt filter ska bli
/// ljusare när modulationsraden öppnar det, mätt som energi i första differensen.
#[test]
fn a_cutoff_slot_opens_the_voice_filter() {
    use crate::audio::modulation::{ModSlot, ModSource, ModTarget};

    let mut closed = SynthEngine::new(48_000.0);
    let mut opened = SynthEngine::new(48_000.0);
    for engine in [&mut closed, &mut opened] {
        engine.waveform = crate::audio::Waveform::Saw;
        engine.filter_params.cutoff = 220.0;
        engine.handle_command(AudioCommand::NoteOn { note: 45, freq: 110.0, velocity: 1.0, channel: 0 });
    }
    opened
        .modulation
        .set_slots(vec![ModSlot::new(ModSource::Velocity, ModTarget::Cutoff, 0.75)]);
    let brightness = |engine: &mut SynthEngine| {
        let mut prev = 0.0f32;
        let mut sum = 0.0f64;
        for _ in 0..9_600 {
            let (l, _) = engine.process_stereo();
            let d = (l - prev) as f64;
            sum += d * d;
            prev = l;
        }
        sum.sqrt()
    };
    let a = brightness(&mut closed);
    let b = brightness(&mut opened);
    assert!(a > 1e-6, "referensen ska höras");
    assert!(
        b > a * 1.2,
        "öppnat filter ska ge mer diskant: {b:.4} mot {a:.4}"
    );
}

/// **Unison stämmer isär rösterna, och det hörs som rörelse.** Två eller flera röster
/// med några cents mellan sig svävar mot varandra; en enda röst står still. Måttet är
/// spridningen i blockvis RMS, inte en spektralbild.
#[test]
fn unison_detune_makes_the_level_move() {
    let spread = |voices: u8, detune: f32| {
        let mut engine = SynthEngine::new(48_000.0);
        engine.filter_params.cutoff = 12_000.0;
        // Ingen filterenvelope: annars sveper cutoffen och då rör nivån sig av det i
        // stället för av isärstämningen.
        engine.filter_env_amount = 0.0;
        // Platt envelope: spridningen ska komma av isärstämningen, inte av ett decay.
        engine.adsr = crate::audio::envelope::AdsrParams {
            attack: 0.005,
            decay: 0.001,
            sustain: 1.0,
            release: 0.05,
        };
        engine.handle_command(AudioCommand::SetUnison { voices, detune_cents: detune });
        engine.handle_command(AudioCommand::NoteOn { note: 57, freq: 220.0, velocity: 1.0, channel: 0 });
        // 50 ms-block: elva perioder av 220 Hz, så blockens RMS inte varierar av fasläget.
        let mut blocks = Vec::new();
        for _ in 0..40 {
            let mut sum = 0.0f32;
            for _ in 0..2_400 {
                let (l, _) = engine.process_stereo();
                sum += l * l;
            }
            blocks.push((sum / 2_400.0).sqrt());
        }
        // Hoppa över attacken.
        let tail = &blocks[4..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        let var = tail.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / tail.len() as f32;
        var.sqrt() / mean.max(1e-9)
    };
    let still = spread(1, 0.0);
    let wide = spread(3, 50.0);
    println!("RMS-spridning: en röst {still:.4}, tre röster {wide:.4}");
    // En enda röst har en liten rest (några procent) av att blocken inte ligger exakt på
    // perioden; det som ska synas är att de isärstämda rösterna rör sig **flera gånger** mer.
    assert!(still < 0.08, "en röst ska stå nästan still: {still:.4}");
    assert!(wide > still * 2.0, "tre isärstämda röster ska sväva: {wide:.4} mot {still:.4}");
}

/// Unisontaket: fler än åtta röster tas inte emot, och noll blir en.
#[test]
fn the_unison_count_is_clamped() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.handle_command(AudioCommand::SetUnison { voices: 99, detune_cents: 500.0 });
    assert_eq!(engine.unison_voices, 8);
    assert!(engine.unison_detune_cents <= 100.0);
    engine.handle_command(AudioCommand::SetUnison { voices: 0, detune_cents: -5.0 });
    assert_eq!(engine.unison_voices, 1);
    assert_eq!(engine.unison_detune_cents, 0.0);
}

/// **Portamento glider i stället för att hoppa.** 220 Hz, tyst, sedan 440 Hz: strax efter
/// den andra noten ligger tonhöjden kvar nära 220 och närmar sig 440 först efter glidtiden.
#[test]
fn glide_slides_between_notes() {
    // Grundtonsskattning med autokorrelation över ett kort fönster.
    let f0 = |samples: &[f32], sr: f32| -> f32 {
        let min_lag = (sr / 900.0) as usize;
        let max_lag = (sr / 150.0) as usize;
        let hi = max_lag.min(samples.len() / 2);
        let mut scores = Vec::new();
        for lag in min_lag..hi {
            let mut dot = 0.0f32;
            let mut energy = 0.0f32;
            for i in 0..samples.len() - lag {
                dot += samples[i] * samples[i + lag];
                energy += samples[i] * samples[i];
            }
            scores.push((lag, dot / (energy + 1e-9)));
        }
        let best = scores.iter().map(|(_, s)| *s).fold(f32::NEG_INFINITY, f32::max);
        // **Första** toppen, inte den högsta: en periodisk signal toppar också på två och tre
        // perioder, och då väljer den högsta toppen en oktav ned.
        let lag = scores
            .iter()
            .find(|(_, s)| *s >= best * 0.9)
            .map(|(l, _)| *l)
            .unwrap_or(min_lag);
        sr / lag as f32
    };

    let run = |glide: f32| {
        let mut engine = SynthEngine::new(48_000.0);
        engine.filter_params.cutoff = 12_000.0;
        engine.filter_env_amount = 0.0;
        engine.adsr = crate::audio::envelope::AdsrParams {
            attack: 0.002,
            decay: 0.001,
            sustain: 1.0,
            release: 0.002,
        };
        engine.handle_command(AudioCommand::SetGlide { seconds: glide });
        engine.handle_command(AudioCommand::NoteOn { note: 57, freq: 220.0, velocity: 1.0, channel: 0 });
        for _ in 0..4_800 {
            engine.process_stereo();
        }
        engine.handle_command(AudioCommand::NoteOff { note: 57, channel: 0 });
        for _ in 0..480 {
            engine.process_stereo();
        }
        engine.handle_command(AudioCommand::NoteOn { note: 69, freq: 440.0, velocity: 1.0, channel: 0 });
        // Första 40 ms efter den andra noten, och efter glidtiden.
        let mut early = Vec::new();
        for _ in 0..1_920 {
            let (l, _) = engine.process_stereo();
            early.push(l);
        }
        let mut late = Vec::new();
        for _ in 0..9_600 {
            let (l, _) = engine.process_stereo();
            late.push(l);
        }
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        println!(
            "  peak early {:.4} late {:.4}",
            rms(&early), rms(&late)
        );
        (f0(&early[240..], 48_000.0), f0(&late[2_400..], 48_000.0))
    };

    let (jump_early, jump_late) = run(0.0);
    let (glide_early, glide_late) = run(0.2);
    println!("utan glide: {jump_early:.0} → {jump_late:.0} Hz");
    println!("med glide:  {glide_early:.0} → {glide_late:.0} Hz");

    assert!((jump_early - 440.0).abs() < 25.0, "utan glide ska noten vara där direkt");
    // Gliden ska ha **börjat** röra sig (över 220) men inte vara framme (under målet).
    // Den exakta siffran beror på att fönstret är en chirp, därför bandet i stället för ett tal.
    assert!(
        glide_early > 220.0 && glide_early < jump_early - 20.0,
        "med glide ska tonhöjden vara på väg: {glide_early:.0} Hz mot målet {jump_early:.0}"
    );
    assert!(
        (glide_late - 440.0).abs() < 15.0,
        "efter glidtiden ska den ha nått 440: {glide_late:.0} Hz"
    );
}

/// **Filterdrive klipper mjukt.** Drivet mäts på **rösten**, inte på mastern: masterns
/// kedja komprimerar det drivna ljudet och skulle dölja vad drivet gör.
#[test]
fn filter_drive_compresses_the_crest() {
    let run = |drive: f32| {
        let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
        let flt = FilterParams { cutoff: 6_000.0, resonance: 0.7 };
        let env = AdsrParams::default();
        let mut v = Voice::new(48_000.0);
        v.trigger(57, 220.0, 1.0, 48_000.0);
        let mut buf = Vec::new();
        for _ in 0..9_600 {
            buf.push(v.next_sample(
                Waveform::Sine,
                &adsr,
                &flt,
                &env,
                0.0,
                &FmParams::default(),
                0.0,
                0.0,
                drive,
                0.0,
                default_table(),
            ));
        }
        let tail = &buf[2_400..];
        let peak = tail.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
        let mut d = 0.0f32;
        let mut e = 0.0f32;
        for w in tail.windows(2) {
            d += (w[1] - w[0]) * (w[1] - w[0]);
            e += w[1] * w[1];
        }
        (peak / rms.max(1e-9), d / e.max(1e-12), peak)
    };
    let (clean_crest, clean_tilt, clean_peak) = run(0.0);
    let (driven_crest, driven_tilt, driven_peak) = run(1.0);
    println!(
        "crest {clean_crest:.3} → {driven_crest:.3}, diskant {clean_tilt:.4} → {driven_tilt:.4}, topp {clean_peak:.3} → {driven_peak:.3}"
    );
    // En ren sinus har crest 1,414; driven ska den ner mot 1,0 (fyrkantigare).
    assert!((clean_crest - 1.414).abs() < 0.12, "oren referens: {clean_crest:.3}");
    assert!(
        driven_crest < clean_crest * 0.95,
        "drive ska ta ner crest: {driven_crest:.3} mot {clean_crest:.3}"
    );
    assert!(
        driven_tilt > clean_tilt * 3.0,
        "drive ska lägga till övertoner: {driven_tilt:.5} mot {clean_tilt:.5}"
    );
    // Normaliseringen ska hålla nivån: en fullskalig sinus får inte bli dubbelt så stark.
    assert!(
        driven_peak < clean_peak * 1.6,
        "drivet ska inte höja nivån nämnvärt: {driven_peak:.3} mot {clean_peak:.3}"
    );
}

/// **Key tracking följer klaviaturen.** Samma filter, samma not innehåll, två noter en
/// oktav isär: med tracking på flyttas brytfrekvensen med noten, så den högre noten får
/// proportionellt lika mycket av sitt spektrum som den lägre. Mätt som RMS på rösten.
#[test]
fn key_tracking_moves_the_cutoff_with_the_note() {
    let level = |key_track: f32, note: u8, freq: f32| {
        let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
        let flt = FilterParams { cutoff: 400.0, resonance: 0.7 };
        let env = AdsrParams::default();
        let mut v = Voice::new(48_000.0);
        v.trigger(note, freq, 1.0, 48_000.0);
        v.key_cutoff_ratio = if key_track > 0.0 {
            2.0_f32.powf((note as f32 - 60.0) / 12.0 * key_track)
        } else {
            1.0
        };
        let mut sum = 0.0f64;
        let mut n = 0usize;
        for i in 0..9_600 {
            let s = v.next_sample(
                Waveform::Saw,
                &adsr,
                &flt,
                &env,
                0.0,
                &FmParams::default(),
                0.0,
                0.0,
                0.0,
                0.0,
                default_table(),
            );
            if i > 2_400 {
                sum += (s as f64) * (s as f64);
                n += 1;
            }
        }
        (sum / n as f64).sqrt() as f32
    };
    let low_fixed = level(0.0, 57, 220.0);
    let high_fixed = level(0.0, 69, 440.0);
    let low_tracked = level(1.0, 57, 220.0);
    let high_tracked = level(1.0, 69, 440.0);
    let fixed_ratio = high_fixed / low_fixed;
    let tracked_ratio = high_tracked / low_tracked;
    println!("nivåförhållande hög/låg: {fixed_ratio:.3} utan tracking, {tracked_ratio:.3} med");
    assert!(
        tracked_ratio > fixed_ratio * 1.5,
        "tracking ska lyfta den högre noten: {tracked_ratio:.3} mot {fixed_ratio:.3}"
    );
    assert!(
        (low_tracked / low_fixed - 1.0).abs() < 0.35,
        "den lägre noten ska bara flyttas marginellt: {:.3}",
        low_tracked / low_fixed
    );
}

/// **Suboscillatorn lägger en oktav under noten, efter filtret.** Filtret står på 400 Hz,
/// alltså långt över 110 Hz: subben ska höras ändå, annars är den ingen subbas.
#[test]
fn the_sub_oscillator_adds_an_octave_below() {
    let run = |sub: f32| {
        let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
        let flt = FilterParams { cutoff: 400.0, resonance: 0.7 };
        let env = AdsrParams::default();
        let mut v = Voice::new(48_000.0);
        v.trigger(57, 220.0, 1.0, 48_000.0);
        let mut out = Vec::new();
        for _ in 0..9_600 {
            out.push(v.next_sample(
                Waveform::Sine,
                &adsr,
                &flt,
                &env,
                0.0,
                &FmParams::default(),
                0.0,
                0.0,
                0.0,
                sub,
                default_table(),
            ));
        }
        (mag_at(&out[2_400..], 110.0), mag_at(&out[2_400..], 220.0))
    };
    let (plain_sub, plain_main) = run(0.0);
    let (with_sub, with_main) = run(0.5);
    println!("110 Hz: {plain_sub:.5} → {with_sub:.5}, 220 Hz: {plain_main:.5} → {with_main:.5}");
    assert!(
        with_sub > plain_sub * 4.0,
        "subben ska synas på halva frekvensen: {with_sub:.5} mot {plain_sub:.5}"
    );
    assert!(
        (with_main - plain_main).abs() < plain_main * 0.2,
        "grundtonen ska vara orörd: {with_main:.5} mot {plain_main:.5}"
    );
}

/// **Varje förval når varje parameter.** `LoadPreset` ska ställa in allt som
/// `settings()` säger — inte bara vågform, envelope och filter, som var allt den gjorde
/// innan de nya parametrarna fanns.
#[test]
fn every_preset_reaches_every_parameter() {
    for preset in Preset::ALL {
        let s = preset.settings();
        let mut engine = SynthEngine::new(48_000.0);
        engine.handle_command(AudioCommand::LoadPreset(preset));
        assert_eq!(engine.waveform, s.waveform, "{}: vågform", preset.label());
        assert_eq!(engine.adsr.release, s.adsr.release, "{}: release", preset.label());
        assert_eq!(engine.filter_params.cutoff, s.filter.cutoff, "{}: cutoff", preset.label());
        assert_eq!(engine.filter_env_amount, s.filter_env_amount, "{}: filterenvelop", preset.label());
        assert_eq!(engine.sub_level, s.sub, "{}: sub", preset.label());
        assert_eq!(engine.unison_voices, s.unison, "{}: unison", preset.label());
        assert_eq!(engine.unison_detune_cents, s.detune, "{}: cent", preset.label());
        assert_eq!(engine.glide_time, s.glide, "{}: glide", preset.label());
        assert_eq!(engine.filter_drive, s.drive, "{}: drive", preset.label());
        assert_eq!(engine.harmonics, s.harmonics, "{}: dragstänger", preset.label());
        assert_eq!(engine.waveform, s.waveform, "{}: vågform igen", preset.label());
    }
    // Och förvalen ska faktiskt använda de nya parametrarna, annars mäter provet inget.
    let any_sub = Preset::ALL.iter().any(|p| p.settings().sub > 0.0);
    let any_unison = Preset::ALL.iter().any(|p| p.settings().unison > 1);
    let any_glide = Preset::ALL.iter().any(|p| p.settings().glide > 0.0);
    let any_drive = Preset::ALL.iter().any(|p| p.settings().drive > 0.0);
    let any_additive = Preset::ALL
        .iter()
        .any(|p| p.settings().waveform == Waveform::Additive && p.settings().harmonics[2] > 0.0);
    assert!(
        any_sub && any_unison && any_glide && any_drive && any_additive,
        "förvalen ska spänna över de nya parametrarna"
    );
}

/// **Additiv syntes: bara de övertoner som är utdragna hörs.** Med enbart tredje
/// dragstången utdragen ska 330 Hz finnas och grundtonen 110 Hz inte.
#[test]
fn additive_synthesis_plays_only_the_drawn_harmonics() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 18_000.0;
    engine.filter_env_amount = 0.0;
    engine.adsr = crate::audio::envelope::AdsrParams {
        attack: 0.001,
        decay: 10.0,
        sustain: 1.0,
        release: 0.1,
    };
    let mut levels = [0.0f32; crate::audio::synth::HARMONIC_COUNT];
    levels[2] = 1.0;
    engine.handle_command(AudioCommand::SetHarmonics { levels });
    engine.handle_command(AudioCommand::NoteOn { note: 45, freq: 110.0, velocity: 1.0, channel: 0 });
    let mut buf = Vec::new();
    for _ in 0..9_600 {
        let (l, _) = engine.process_stereo();
        buf.push(l);
    }
    let tail = &buf[2_400..];
    let fundamental = mag_at(tail, 110.0);
    let third = mag_at(tail, 330.0);
    println!("110 Hz: {fundamental:.4}, 330 Hz: {third:.4}");
    assert!(third > 0.2, "tredje övertonen ska höras: {third:.4}");
    assert!(
        third > fundamental * 20.0,
        "grundtonen ska vara borta: {fundamental:.4} mot {third:.4}"
    );
}

/// Tabellen ska vara samma summa som den direkta vägen — också för den 64:e partialen.
#[test]
fn the_additive_table_matches_the_direct_sum() {
    let mut levels = [0.0f32; HARMONIC_COUNT];
    levels[0] = 1.0;
    levels[5] = 0.5;
    levels[HARMONIC_COUNT - 1] = 0.25;
    let table = AdditiveTable::new(&levels, 48_000.0);
    // Tabellen är onormaliserad; rösten lägger på bandets vikt. Band 0 har alla 64 partialer.
    let gain = 1.0 / levels.iter().filter(|level| **level > 0.0).sum::<f32>().max(1.0);
    for j in 0..97 {
        let phase = std::f32::consts::TAU * j as f32 / 97.0;
        let mut direct: f32 = levels
            .iter()
            .enumerate()
            .map(|(i, level)| level * (phase * (i as f32 + 1.0)).sin())
            .sum::<f32>();
        direct *= gain;
        let got = table.at_phase(0, phase) * gain;
        assert!(
            (got - direct).abs() < 2e-3,
            "{phase}: tabell {got} mot direkt {direct}"
        );
    }
}

/// **Den 64:e partialen hörs**: bara sista dragstången utdragen ger 7 040 Hz ur en 110 Hz-not,
/// alltså inget tak vid 16 som förut.
#[test]
fn the_sixty_fourth_partial_is_heard() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 18_000.0;
    engine.filter_env_amount = 0.0;
    engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let mut levels = [0.0f32; HARMONIC_COUNT];
    levels[HARMONIC_COUNT - 1] = 1.0;
    engine.handle_command(AudioCommand::SetHarmonics { levels });
    engine.handle_command(AudioCommand::NoteOn {
        note: 45,
        freq: 110.0,
        velocity: 1.0,
        channel: 0,
    });
    let mut buf = Vec::new();
    for _ in 0..9_600 {
        let (l, _) = engine.process_stereo();
        buf.push(l);
    }
    let top = mag_at(&buf[2_400..], 110.0 * HARMONIC_COUNT as f32);
    let fundamental = mag_at(&buf[2_400..], 110.0);
    assert!(top > 0.2, "64:e partialen ska höras: {top:.4}");
    assert!(
        top > fundamental * 20.0,
        "grundtonen ska vara borta: {fundamental:.4} mot {top:.4}"
    );
}

/// **Bandbegränsningen:** en C7 med alla 64 dragstänger utdragna får bara sina elva första
/// partialer. Partial 20 ligger på 41,9 kHz och viker ner till 6,1 kHz vid 48 kHz — före
/// bandgränsen hördes den som en främmande ton mitt i klangen.
#[test]
fn a_high_note_is_band_limited() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 20_000.0;
    engine.filter_params.resonance = 0.0;
    engine.filter_env_amount = 0.0;
    engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    engine.handle_command(AudioCommand::SetHarmonics { levels: [1.0; HARMONIC_COUNT] });
    let freq = 2_093.0; // C7
    engine.handle_command(AudioCommand::NoteOn {
        note: 96,
        freq,
        velocity: 1.0,
        channel: 0,
    });
    let mut buf = Vec::new();
    for _ in 0..9_600 {
        let (l, _) = engine.process_stereo();
        buf.push(l);
    }
    let body = &buf[2_400..];
    let fundamental = mag_at(body, freq);
    let alias = mag_at(body, 48_000.0 - 20.0 * freq); // 6 140 Hz
    assert_eq!(engine.additive.limit(band_of(96)), 11, "C7-bandets gräns");
    assert!(
        alias < fundamental * 0.02,
        "partial 20 viker ner till 6,1 kHz: {alias:.4} mot grundtonen {fundamental:.4}"
    );
}

/// **Envelopp per partial: spektrum blir mörkare med tiden.** Bara partial 1 (full sustain)
/// och partial 8 (snabb decay till 0) utdragna — centroiden ska falla mot 1 när 8:an dör.
#[test]
fn per_partial_envelopes_darken_the_spectrum() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 18_000.0;
    engine.filter_env_amount = 0.0;
    // Utan detta mäter provet FX-svansen: delayens 300 ms-eko och reverbet bär 8:ans
    // energi långt efter decayen.
    engine.reverb_params.mix = 0.0;
    engine.delay_params.mix = 0.0;
    engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let mut levels = [0.0f32; HARMONIC_COUNT];
    levels[0] = 1.0;
    levels[7] = 1.0;
    engine.handle_command(AudioCommand::SetHarmonics { levels });
    let mut envs = PARTIAL_ENV_NEUTRAL;
    envs[7] = [0.001, 0.25, 0.0];
    engine.handle_command(AudioCommand::SetPartialEnvelopes { envs });
    assert!(engine.partial_envs_active);
    engine.handle_command(AudioCommand::NoteOn {
        note: 45,
        freq: 110.0,
        velocity: 1.0,
        channel: 0,
    });
    let mut buf = Vec::new();
    for _ in 0..24_000 {
        let (l, _) = engine.process_stereo();
        buf.push(l);
    }
    let early = harmonic_centroid(&buf[2_400..4_800], 110.0);
    let late = harmonic_centroid(&buf[19_200..21_600], 110.0);
    println!("centroid 50–100 ms: {early:.2}, 400–450 ms: {late:.2}");
    assert!(early > 2.5, "anslaget ska vara ljust: {early:.2}");
    assert!(late < 1.3, "bara grundtonen ska vara kvar: {late:.2}");
    assert!(early > late * 2.0, "spektret ska mörkna: {early:.2} -> {late:.2}");
}

/// Konturens form: linjär attack, linjär decay mot sustain, sedan stilla.
#[test]
fn a_partial_envelope_ramps_and_falls_as_written() {
    let env = [0.1, 0.2, 0.4];
    assert!((partial_env_level(0.05, env) - 0.5).abs() < 1e-6, "halva attacken");
    assert!((partial_env_level(0.2, env) - 0.7).abs() < 1e-6, "halva decayen");
    assert!((partial_env_level(5.0, env) - 0.4).abs() < 1e-6, "sustain");
}

/// **Spektralt filter: klangfärgen flyttar, tonhöjden står stilla.** Formanten ligger över
/// partialindex, så en förskjutning flyttar centroiden men grundtonen mäts fortfarande 110 Hz.
#[test]
fn a_spectral_formant_moves_the_centroid_but_not_the_pitch() {
    fn render(center: f32) -> Vec<f32> {
        let mut engine = SynthEngine::new(48_000.0);
        engine.waveform = Waveform::Additive;
        engine.filter_params.cutoff = 18_000.0;
        engine.filter_env_amount = 0.0;
        engine.reverb_params.mix = 0.0;
        engine.delay_params.mix = 0.0;
        engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
        let mut levels = [0.0f32; HARMONIC_COUNT];
        for level in levels.iter_mut().take(12) {
            *level = 0.6;
        }
        engine.handle_command(AudioCommand::SetHarmonics { levels });
        engine.handle_command(AudioCommand::SetSpectralFormant {
            center,
            width: 1.5,
            amount: 0.8,
        });
        engine.handle_command(AudioCommand::NoteOn {
            note: 45,
            freq: 110.0,
            velocity: 1.0,
            channel: 0,
        });
        let mut buf = Vec::new();
        for _ in 0..14_400 {
            let (l, _) = engine.process_stereo();
            buf.push(l);
        }
        buf
    }

    let low = render(2.0);
    let high = render(9.0);
    let c_low = harmonic_centroid(&low[4_800..], 110.0);
    let c_high = harmonic_centroid(&high[4_800..], 110.0);
    println!("centroid formant 2: {c_low:.2}, formant 9: {c_high:.2}");
    assert!(c_high > c_low * 1.5, "formanten ska flytta klangen: {c_low:.2} -> {c_high:.2}");

    let p_low = fundamental_hz(&low[4_800..]);
    let p_high = fundamental_hz(&high[4_800..]);
    println!("pitch formant 2: {p_low:.1} Hz, formant 9: {p_high:.1} Hz");
    assert!((p_low - 110.0).abs() < 2.0, "tonhöjden ska stå stilla: {p_low:.1} Hz");
    assert!((p_high - 110.0).abs() < 2.0, "tonhöjden ska stå stilla: {p_high:.1} Hz");
}

/// Formanten släpper igenom bandet och dämpar resten; `amount = 0` lämnar allt orört.
#[test]
fn the_spectral_formant_keeps_its_band_and_quietens_the_rest() {
    let formant = SpectralFormant { center: 5.0, width: 1.0, amount: 0.9 };
    assert!((formant.gain(5) - 1.0).abs() < 1e-3, "bandet ska vara orört");
    assert!(formant.gain(1) < 0.15, "grundtonen utanför bandet: {}", formant.gain(1));
    assert!(formant.gain(20) < 0.15, "hög partial utanför bandet: {}", formant.gain(20));
    let off = SpectralFormant { amount: 0.0, ..formant };
    assert_eq!(off.gain(1), 1.0, "avstängd formant ska inte röra något");
}

/// **Tidsaxeln: spektret glider under noten.** Två kurvor som startar på samma formant ska ge
/// samma centroid vid anslaget men olika vid 0,5 s — det är skillnaden mellan en envelop och
/// en tidsaxel.
#[test]
fn the_time_axis_moves_the_spectrum_after_the_attack() {
    fn render(to: f32) -> Vec<f32> {
        let mut engine = SynthEngine::new(48_000.0);
        engine.waveform = Waveform::Additive;
        engine.filter_params.cutoff = 18_000.0;
        engine.filter_env_amount = 0.0;
        engine.reverb_params.mix = 0.0;
        engine.delay_params.mix = 0.0;
        engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
        let mut levels = [0.0f32; HARMONIC_COUNT];
        for level in levels.iter_mut().take(12) {
            *level = 0.6;
        }
        engine.handle_command(AudioCommand::SetHarmonics { levels });
        engine.handle_command(AudioCommand::SetSpectralFormant {
            center: 1.0,
            width: 1.5,
            amount: 0.8,
        });
        engine.handle_command(AudioCommand::SetSpectralSweep { to, seconds: 1.0 });
        engine.handle_command(AudioCommand::NoteOn {
            note: 45,
            freq: 110.0,
            velocity: 1.0,
            channel: 0,
        });
        let mut buf = Vec::new();
        for _ in 0..26_400 {
            let (l, _) = engine.process_stereo();
            buf.push(l);
        }
        buf
    }

    let rising = render(9.0);
    let shallow = render(2.0);
    let start_a = harmonic_centroid(&rising[0..2_400], 110.0);
    let start_b = harmonic_centroid(&shallow[0..2_400], 110.0);
    let late_a = harmonic_centroid(&rising[24_000..26_400], 110.0);
    let late_b = harmonic_centroid(&shallow[24_000..26_400], 110.0);
    println!(
        "centroid vid anslag: {start_a:.2} / {start_b:.2}; vid 0,5 s: {late_a:.2} / {late_b:.2}"
    );
    assert!(
        (start_a - start_b).abs() < 0.25,
        "kurvorna ska starta lika: {start_a:.2} mot {start_b:.2}"
    );
    assert!(
        (late_a - late_b).abs() > 1.5,
        "efter 0,5 s ska de skilja: {late_a:.2} mot {late_b:.2}"
    );
    assert!(
        late_a > start_a + 1.0,
        "den stigande kurvan ska ljusna: {start_a:.2} -> {late_a:.2}"
    );
}

/// Svepets läge: linjärt från start till mål, klämt efter tiden; avstängt utan tid.
#[test]
fn the_sweep_reaches_its_target_and_then_stays() {
    let sweep = SpectralSweep { to: 9.0, seconds: 2.0 };
    assert!((sweep.center_at(0.0, 1.0) - 1.0).abs() < 1e-6);
    assert!((sweep.center_at(1.0, 1.0) - 5.0).abs() < 1e-6);
    assert!((sweep.center_at(9.0, 1.0) - 9.0).abs() < 1e-6);
    assert!(sweep.active(1.0));
    assert!(!SpectralSweep::default().active(1.0), "utan tid är svepet av");
    assert!(!SpectralSweep { to: 9.0, seconds: 0.0 }.active(1.0));
}

/// **Mätning av 6-op:s värsta fall.** Körs för hand:
/// `cargo test --release --bin sonix -- --ignored measure_six_op_worst_case --nocapture`
/// 16 röster, alla sex operatorer med filter (24 dB) och fyra unisonstämmor, plus filterkurvan
/// — det läge som kostar mest per sampel.
#[test]
#[ignore = "mätning: körs för hand med --ignored --nocapture"]
fn measure_six_op_worst_case() {
    use std::time::Instant;
    const VOICES: usize = 16;
    const SAMPLES: usize = 48_000;

    // Fyra lägen: ett typiskt (unison 1, två operatorer) och tre tak-lägen, så siffran säger
    // både vad de nya rattarna kostar och vad en vanlig patch landar på.
    for (label, slope, curve, unison) in [
        ("typisk patch (unison 1)", 1u8, false, 1u8),
        ("12 dB, ingen kurva", 1, false, 4),
        ("24 dB, ingen kurva", 2, false, 4),
        ("24 dB + filterkurva", 2, true, 4),
    ] {
        let mut engine = SynthEngine::new(48_000.0);
        engine.waveform = Waveform::SixOp;
        engine.handle_command(AudioCommand::SetSixOp { params: crate::audio::synth::SixOpParams {
            filters: std::array::from_fn(|_| crate::audio::synth::six_op::OpFilter {
                cutoff: 2_000.0,
                resonance: 2.0,
                mode: 0,
            }),
            filter_slope: [slope; 6],
            unison,
            detune: 12.0,
            curve_gain: if curve { [2.0, 2.0, 2.0, 2.0] } else { [1.0; 4] },
            ..crate::audio::synth::SixOpParams::default()
        } });
        for i in 0..VOICES {
            engine.handle_command(AudioCommand::NoteOn {
                note: 45 + i as u8,
                freq: 110.0 * 2.0f32.powf(i as f32 / 12.0),
                velocity: 0.5,
                channel: 0,
            });
        }

        let start = Instant::now();
        let mut peak = 0.0f32;
        for _ in 0..SAMPLES {
            let (l, _) = engine.process_stereo();
            peak = peak.max(l.abs());
        }
        let secs = start.elapsed().as_secs_f32();
        println!(
            "6-op, 16 röster, 6 operatorer, 4 stämmor, {label}: {secs:.4} s per sekund ljud ({:.1}× realtid), topp {peak:.3}",
            1.0 / secs
        );
    }
}

/// **Mätning, inte ett prov.** Körs för hand:
/// `cargo test --release --bin sonix -- --ignored measure_additive_table --nocapture`
/// 16 röster à 64 partialer, en sekund ljud; jämför tabellen mot en `sin()` per partial.
#[test]
#[ignore = "mätning: körs för hand med --ignored --nocapture"]
fn measure_additive_table_against_a_sine_per_partial() {
    use std::hint::black_box;
    use std::time::Instant;
    const VOICES: usize = 16;
    const SAMPLES: usize = 48_000;
    const TAU: f32 = std::f32::consts::TAU;
    let levels = [0.5f32; HARMONIC_COUNT];
    let phase_inc = TAU * 440.0 / 48_000.0;

    let mut acc = 0.0f32;
    let mut phases = [0.0f32; VOICES];
    let start = Instant::now();
    for _ in 0..SAMPLES {
        for phase in &mut phases {
            *phase = (*phase + phase_inc) % TAU;
            let p = black_box(*phase);
            let mut sum = 0.0;
            for (i, level) in levels.iter().enumerate() {
                sum += level * (p * (i as f32 + 1.0)).sin();
            }
            acc += sum;
        }
    }
    let direct = start.elapsed().as_secs_f32();

    let mut rebuilt = AdditiveTable::new(&levels, 48_000.0);
    let rebuild = Instant::now();
    for _ in 0..100 {
        rebuilt.rebuild(&levels, 48_000.0);
    }
    let rebuild_ms = rebuild.elapsed().as_secs_f32() * 1000.0 / 100.0;

    let table = AdditiveTable::new(&levels, 48_000.0);
    let mut phases = [0.0f32; VOICES];
    let start = Instant::now();
    for _ in 0..SAMPLES {
        for phase in &mut phases {
            *phase = (*phase + phase_inc) % TAU;
            acc += table.at_phase(0, black_box(*phase));
        }
    }
    let table_time = start.elapsed().as_secs_f32();
    black_box(acc);

    // Hela rösten, med filter och envelop: det som faktiskt spelar i realtid.
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 18_000.0;
    engine.filter_env_amount = 0.0;
    engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    engine.handle_command(AudioCommand::SetHarmonics { levels });
    for i in 0..VOICES {
        engine.handle_command(AudioCommand::NoteOn {
            note: 45 + i as u8,
            freq: 110.0 * 2.0f32.powf(i as f32 / 12.0),
            velocity: 0.5,
            channel: 0,
        });
    }
    let start = Instant::now();
    let mut peak = 0.0f32;
    for _ in 0..SAMPLES {
        let (l, _) = engine.process_stereo();
        peak = peak.max(l.abs());
    }
    let engine_time = start.elapsed().as_secs_f32();
    println!(
        "64 partialer, 16 röster, 1 s ljud: sin-summa {direct:.3} s ({:.1}× realtid), tabell {table_time:.4} s ({:.0}× realtid), {:.0}× snabbare; hel röst {engine_time:.4} s ({:.0}× realtid), topp {peak:.3}; ombyggnad {rebuild_ms:.2} ms",
        1.0 / direct,
        1.0 / table_time,
        direct / table_time,
        1.0 / engine_time,
    );

    // Den långsamma vägen: varje partial har sin egen kontur, så summan räknas per sampel.
    let mut engine = SynthEngine::new(48_000.0);
    engine.waveform = Waveform::Additive;
    engine.filter_params.cutoff = 18_000.0;
    engine.filter_env_amount = 0.0;
    engine.adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    engine.handle_command(AudioCommand::SetHarmonics { levels });
    let envs = [[0.002, 0.4, 0.2]; HARMONIC_COUNT];
    engine.handle_command(AudioCommand::SetPartialEnvelopes { envs });
    for i in 0..VOICES {
        engine.handle_command(AudioCommand::NoteOn {
            note: 45 + i as u8,
            freq: 110.0 * 2.0f32.powf(i as f32 / 12.0),
            velocity: 0.5,
            channel: 0,
        });
    }
    let start = Instant::now();
    for _ in 0..SAMPLES {
        let (l, _) = engine.process_stereo();
        peak = peak.max(l.abs());
    }
    let env_time = start.elapsed().as_secs_f32();
    println!(
        "samma 64 partialer med egen kontur per partial: {env_time:.4} s ({:.0}× realtid), topp {peak:.3}",
        1.0 / env_time
    );
    assert!(table_time < direct, "tabellen ska slå en sin() per partial");
}

/// **Plucked! är en sträng, inte en vågform.** Tonhöjden kommer ur fördröjningslinjens längd,
/// så provet mäter att 220 Hz in ger 220 Hz ut — och att strängen klingar av.
#[test]
fn a_plucked_string_rings_at_the_note_pitch_and_decays() {
    let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let flt = FilterParams { cutoff: 12_000.0, resonance: 0.0 };
    let env = AdsrParams::default();
    let mut v = Voice::new(48_000.0);
    v.trigger(57, 220.0, 1.0, 48_000.0);
    let mut out = Vec::new();
    for _ in 0..48_000 {
        out.push(v.next_sample(
            Waveform::Pluck,
            &adsr,
            &flt,
            &env,
            0.0,
            &FmParams::default(),
            0.0,
            0.0,
            0.0,
            0.0,
            default_table(),
        ));
    }
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let head = rms(&out[1_000..5_000]);
    let tail = rms(&out[40_000..48_000]);
    // Tonhöjd ur autokorrelation över det första halva sekunden.
    let f0 = crate::audio::recorder::detect_pitch_hz(&out[2_000..22_000], 48_000.0).unwrap_or(0.0);
    println!("sträng: f0 {f0:.1} Hz, RMS {head:.4} → {tail:.4}");
    assert!((f0 - 220.0).abs() < 12.0, "strängen ska stämmas till noten: {f0:.1} Hz");
    assert!(head > tail * 1.5, "strängen ska klinga av: {head:.4} → {tail:.4}");
}

/// **Sakura**: bågen exciterar strängen hela tiden, så tonen bär medan anslaget hålls —
/// till skillnad från Pluck, som är en burst och klingar av.
#[test]
fn a_bowed_string_sustains_at_the_note_pitch() {
    let adsr = AdsrParams { attack: 0.05, decay: 10.0, sustain: 1.0, release: 0.1 };
    let flt = FilterParams { cutoff: 12_000.0, resonance: 0.0 };
    let env = AdsrParams::default();
    let mut v = Voice::new(48_000.0);
    v.trigger(57, 220.0, 1.0, 48_000.0);
    let mut out = Vec::new();
    for _ in 0..96_000 {
        out.push(v.next_sample(
            Waveform::Bowed,
            &adsr,
            &flt,
            &env,
            0.0,
            &FmParams::default(),
            0.0,
            0.0,
            0.0,
            0.0,
            default_table(),
        ));
    }
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let early = rms(&out[10_000..30_000]);
    let late = rms(&out[80_000..96_000]);
    let f0 = crate::audio::recorder::detect_pitch_hz(&out[20_000..40_000], 48_000.0).unwrap_or(0.0);
    println!("båge: f0 {f0:.1} Hz, RMS {early:.4} → {late:.4}");
    assert!(late > 0.005, "bågen ska bära tonen: {late:.4}");
    assert!(
        late > early * 0.5,
        "den ska inte dö ut som en plockad sträng: {early:.4} → {late:.4}"
    );
    assert!((f0 - 220.0).abs() < 15.0, "strängen ska stämma: {f0:.1} Hz");
}

/// Renderar en plockad sträng och lämnar dess sampel.
fn render_plucked(note: u8, freq: f32, body_amount: f32) -> Vec<f32> {
    let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let flt = FilterParams { cutoff: 12_000.0, resonance: 0.0 };
    let env = AdsrParams::default();
    let mut v = Voice::new(48_000.0);
    v.body_amount = body_amount;
    v.trigger(note, freq, 1.0, 48_000.0);
    let mut out = Vec::new();
    for _ in 0..48_000 {
        out.push(v.next_sample(
            Waveform::Pluck,
            &adsr,
            &flt,
            &env,
            0.0,
            &FmParams::default(),
            0.0,
            0.0,
            0.0,
            0.0,
            default_table(),
        ));
    }
    out
}

/// **Dämpningen sitter per varv, inte per sampel.** En kort sträng gör fler varv per
/// sekund; låg förlusten per sampel hade 1760 Hz dött direkt medan 110 Hz klingade
/// vidare (0,545 mot 0,002 i det försök som reverterades). Här ska båda ha klingat av
/// i samma storleksordning efter 0,9 s — samma absoluta tid, inte samma antal varv.
#[test]
fn plucked_strings_decay_in_absolute_time_not_per_loop() {
    let tail_ratio = |out: &[f32]| {
        let rms = |s: &[f32]| (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt();
        rms(&out[40_000..48_000]) / rms(&out[1_000..5_000])
    };
    let low = tail_ratio(&render_plucked(45, 110.0, 1.0));
    let high = tail_ratio(&render_plucked(93, 1760.0, 1.0));
    println!("kvar efter 0,9 s: 110 Hz {low:.4}, 1760 Hz {high:.4}");
    assert!(low < 0.1 && high < 0.1, "båda ska ha klingat av: {low:.4} / {high:.4}");
    let spread = low.max(high) / low.min(high).max(1e-6);
    assert!(
        spread < 10.0,
        "efterklangen ska vara samma storleksordning, inte per varv: {low:.4} mot {high:.4} ({spread:.1}×)"
    );
}

/// **Kroppen hörs.** Med `body_amount` 1,0 ligger en resonans på 196 Hz i efterledet;
/// med 0,0 är strängen torr. Samma sträng, samma frö — bara kroppen skiljer.
#[test]
fn the_body_resonance_is_audible_and_can_be_switched_off() {
    let wet = mag_at(&render_plucked(57, 220.0, 1.0), 196.0);
    let dry = mag_at(&render_plucked(57, 220.0, 0.0), 196.0);
    println!("kropp vid 196 Hz: på {wet:.5}, av {dry:.5} ({:.2}×)", wet / dry);
    assert!(wet > dry * 1.3, "kroppen ska höras vid 196 Hz: {wet:.5} mot {dry:.5}");
}

/// **Ogun är inharmonisk.** Deltonerna ligger utanför heltalsmultiplarna: 2,76× ska höras
/// medan 2× ska vara svag — det är skillnaden mot varje vanlig oscillator i synten.
#[test]
fn the_inharmonic_wave_puts_its_partials_off_the_harmonic_series() {
    let adsr = AdsrParams { attack: 0.001, decay: 10.0, sustain: 1.0, release: 0.1 };
    let flt = FilterParams { cutoff: 20_000.0, resonance: 0.0 };
    let env = AdsrParams::default();
    let mut v = Voice::new(48_000.0);
    v.trigger(57, 220.0, 1.0, 48_000.0);
    let mut out = Vec::new();
    for _ in 0..24_000 {
        out.push(v.next_sample(
            Waveform::Inharmonic,
            &adsr,
            &flt,
            &env,
            0.0,
            &FmParams::default(),
            0.0,
            0.0,
            0.0,
            0.0,
            default_table(),
        ));
    }
    let body = &out[6_000..];
    let (fund, second, metal) = (mag_at(body, 220.0), mag_at(body, 440.0), mag_at(body, 607.2));
    println!("ogun: 220 Hz {fund:.4}, 440 Hz {second:.4}, 607 Hz {metal:.4}");
    assert!(fund > 0.05, "grundtonen ska höras: {fund:.4}");
    assert!(metal > second * 2.0, "2,76× ska dominera över 2×: {metal:.4} mot {second:.4}");
}

/// **MPE: kanalen följer med noten hela vägen till rösten.** En pitch bend på en
/// medlemskanal ska böja bara den kanalens noter. Utan `channel` i `NoteOn` faller
/// provet på att båda rösterna får samma `bend_ratio`.
#[test]
fn a_note_bend_reaches_only_its_note_on_a_shared_channel() {
    let mut engine = SynthEngine::new(48_000.0);
    for (note, freq) in [(60u8, 261.63f32), (67, 392.0)] {
        engine.handle_command(AudioCommand::NoteOn {
            note,
            freq,
            velocity: 1.0,
            channel: 2,
        });
    }

    engine.handle_command(AudioCommand::NoteBend {
        key: 67,
        semitones: 12.0,
    });

    let active: Vec<&Voice> = engine.voices.iter().filter(|v| v.is_active()).collect();
    assert_eq!(active.len(), 2, "båda noterna ska klinga");
    let n60 = active.iter().find(|v| v.note == 60).expect("not 60");
    let n67 = active.iter().find(|v| v.note == 67).expect("not 67");
    assert_eq!(n60.bend_ratio, 1.0, "not 60 ska stå orörd");
    assert!(
        (n67.bend_ratio - 2.0).abs() < 1e-5,
        "not 67 ska en oktav upp, fick {}",
        n67.bend_ratio
    );
}

#[test]
fn an_mpe_bend_reaches_only_the_voice_on_its_channel() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.handle_command(AudioCommand::NoteOn {
        note: 60,
        freq: 261.63,
        velocity: 1.0,
        channel: 2,
    });
    engine.handle_command(AudioCommand::NoteOn {
        note: 67,
        freq: 392.0,
        velocity: 1.0,
        channel: 3,
    });

    engine.handle_command(AudioCommand::ChannelPitchBend {
        channel: 3,
        semitones: 12.0,
    });

    let active: Vec<&Voice> = engine.voices.iter().filter(|v| v.is_active()).collect();
    assert_eq!(active.len(), 2, "båda noterna ska klinga");
    let ch2 = active.iter().find(|v| v.channel == 2).expect("kanal 2");
    let ch3 = active.iter().find(|v| v.channel == 3).expect("kanal 3");
    assert_eq!(ch2.bend_ratio, 1.0, "kanal 2 ska stå orörd");
    assert!(
        (ch3.bend_ratio - 2.0).abs() < 1e-5,
        "kanal 3 ska en oktav upp, fick {}",
        ch3.bend_ratio
    );

    // Tillbaka till mitten = ingen böjning.
    engine.handle_command(AudioCommand::ChannelPitchBend {
        channel: 3,
        semitones: 0.0,
    });
    let ch3 = engine.voices.iter().find(|v| v.channel == 3).expect("kanal 3");
    assert!((ch3.bend_ratio - 1.0).abs() < 1e-6);
}

/// **Scratchen vänder och tar slut.** Ett urklipp spelas fram, tillbaka, fram … och rösten
/// tystnar när de halva passen är slut i stället för att gnugga för evigt.
#[test]
fn a_scratch_ping_pongs_and_stops_after_its_passes() {
    use std::sync::Arc;

    let mut engine = SynthEngine::new(48_000.0);
    let pcm: Arc<Vec<f32>> = Arc::new((0..48_000).map(|i| i as f32 / 48_000.0).collect());
    engine.handle_command(AudioCommand::PlayScratch {
        left: pcm.clone(),
        right: pcm,
        sample_rate: 48_000,
        volume: 1.0,
        start_frame: 0.0,
        end_frame: 4_800.0,
        speed: 1.0,
        passes: 2,
    });
    assert!(engine.sample_voices[0].active, "scratchen ska starta");

    // Förbi klippets slut: handen vänder.
    for _ in 0..5_200 {
        engine.process_stereo();
    }
    let dir = engine.sample_voices[0].dir;
    assert!(dir < 0.0, "efter klippets slut ska handen gå tillbaka: dir {dir}");

    // Två halva pass är 9 600 samples; därefter spelar rösten ut resten och tystnar.
    for _ in 0..10_000 {
        engine.process_stereo();
    }
    assert!(
        !engine.sample_voices[0].active,
        "scratchen ska ta slut, inte loopa för evigt (pos {})",
        engine.sample_voices[0].pos
    );
}

/// **En fördröjd bend gäller först efter sin fördröjning** — det är det som gör att en
/// inspelad kurva kan ligga inne i ett steg utan att kvantiseras till sextondelen.
#[test]
fn a_delayed_bend_lands_after_its_delay() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.handle_command(AudioCommand::NoteOn {
        note: 69,
        freq: 440.0,
        velocity: 1.0,
        channel: 1,
    });
    engine.handle_command(AudioCommand::BendDelayed {
        channel: 1,
        semitones: 12.0,
        delay_samples: 100,
    });

    for _ in 0..50 {
        engine.process_stereo();
    }
    let voice = engine.voices.iter().find(|v| v.is_active()).expect("rösten");
    assert!(
        (voice.bend_ratio - 1.0).abs() < 1e-6,
        "före fördröjningen ska rösten vara oböjd, fick {}",
        voice.bend_ratio
    );

    for _ in 0..60 {
        engine.process_stereo();
    }
    let voice = engine.voices.iter().find(|v| v.is_active()).expect("rösten");
    assert!(
        (voice.bend_ratio - 2.0).abs() < 1e-5,
        "efter 100 samples ska rösten en oktav upp, fick {}",
        voice.bend_ratio
    );
}

/// Samma tangent på två kanaler är två röster, och note-off på den ena tar bara den.
#[test]
fn the_same_note_on_two_channels_is_two_voices_and_note_off_takes_one() {
    use crate::audio::envelope::EnvelopeStage;

    let mut engine = SynthEngine::new(48_000.0);
    for channel in [2, 3] {
        engine.handle_command(AudioCommand::NoteOn {
            note: 60,
            freq: 261.63,
            velocity: 1.0,
            channel,
        });
    }
    assert_eq!(engine.voices.iter().filter(|v| v.is_active()).count(), 2);

    engine.handle_command(AudioCommand::NoteOff { note: 60, channel: 3 });

    let ch2 = engine.voices.iter().find(|v| v.channel == 2).expect("kanal 2");
    let ch3 = engine.voices.iter().find(|v| v.channel == 3).expect("kanal 3");
    assert_ne!(ch2.envelope.stage, EnvelopeStage::Release, "kanal 2 ska hålla");
    assert_eq!(ch3.envelope.stage, EnvelopeStage::Release, "kanal 3 ska släppa");
}

/// Mastern (kanal 0) böjer allt — den gamla vägen för en vanlig klaviatur.
#[test]
fn a_master_bend_moves_every_voice() {
    let mut engine = SynthEngine::new(48_000.0);
    for (note, freq, channel) in [(60, 261.63, 0), (67, 392.0, 3)] {
        engine.handle_command(AudioCommand::NoteOn {
            note,
            freq,
            velocity: 1.0,
            channel,
        });
    }
    engine.handle_command(AudioCommand::ChannelPitchBend {
        channel: 0,
        semitones: -12.0,
    });
    for voice in engine.voices.iter().filter(|v| v.is_active()) {
        assert!(
            (voice.bend_ratio - 0.5).abs() < 1e-5,
            "röst utan böjning kvar: {}",
            voice.bend_ratio
        );
    }
}

/// **En hållen pitch wheel gäller även nästa anslag.** Böjningen sparas per kanal, så en
/// not som slås an medan hjulet står uppe startar böjd — annars tappas den vid varje ny not.
#[test]
fn a_note_played_while_the_wheel_is_up_starts_bent() {
    let mut engine = SynthEngine::new(48_000.0);
    engine.handle_command(AudioCommand::ChannelPitchBend {
        channel: 2,
        semitones: 12.0,
    });
    engine.handle_command(AudioCommand::NoteOn {
        note: 60,
        freq: 261.63,
        velocity: 1.0,
        channel: 2,
    });
    // Mastern böjd ned en oktav: gäller alla kanaler.
    engine.handle_command(AudioCommand::ChannelPitchBend {
        channel: 0,
        semitones: -12.0,
    });
    engine.handle_command(AudioCommand::NoteOn {
        note: 67,
        freq: 392.0,
        velocity: 1.0,
        channel: 3,
    });

    let ch2 = engine.voices.iter().find(|v| v.channel == 2).expect("kanal 2");
    assert!(
        (ch2.bend_ratio - 1.0).abs() < 1e-5,
        "kanal 2: +12 och master −12 ska ta ut varandra, fick {}",
        ch2.bend_ratio
    );
    let ch3 = engine.voices.iter().find(|v| v.channel == 3).expect("kanal 3");
    assert!(
        (ch3.bend_ratio - 0.5).abs() < 1e-5,
        "kanal 3 ska bara ha masterns −12, fick {}",
        ch3.bend_ratio
    );
}


/// **FL Slayer låter som en gitarr, inte som en plockad sträng.** Mätt på samma ton
/// (220 Hz): plektrumets två avläsningar och rörsteget gör signalen **ljusare** och mer
/// komprimerad — nollgenomgångar 0,0676 → 0,1298, crestfaktor 8,14 → 7,04.
#[test]
fn the_guitar_is_brighter_and_more_compressed_than_a_plucked_string() {
    let adsr = AdsrParams { attack: 0.002, decay: 10.0, sustain: 1.0, release: 0.2 };
    let flt = FilterParams { cutoff: 18_000.0, resonance: 0.0 };
    let env = AdsrParams::default();
    let render = |wave: Waveform| {
        let mut v = Voice::new(48_000.0);
        v.trigger(57, 220.0, 1.0, 48_000.0);
        let mut out = Vec::new();
        for _ in 0..48_000 {
            out.push(v.next_sample(
                wave,
                &adsr,
                &flt,
                &env,
                0.0,
                &FmParams::default(),
                0.0,
                0.0,
                0.0,
                0.0,
                default_table(),
            ));
        }
        out
    };
    let stats = |out: &[f32]| {
        let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
        let zcr = out.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f32
            / out.len() as f32;
        let peak = out.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        (zcr, peak / rms.max(1e-9))
    };

    let pluck = render(Waveform::Pluck);
    let guitar = render(Waveform::Guitar);
    let (zcr_pluck, crest_pluck) = stats(&pluck);
    let (zcr_guitar, crest_guitar) = stats(&guitar);

    println!(
        "pluck zcr {zcr_pluck:.4} crest {crest_pluck:.2} | gitarr zcr {zcr_guitar:.4} crest {crest_guitar:.2}"
    );
    assert!(
        zcr_guitar > zcr_pluck * 1.5,
        "plektrumet ska göra tonen ljusare: {zcr_pluck:.4} -> {zcr_guitar:.4}"
    );
    assert!(
        crest_guitar < crest_pluck * 0.95,
        "röret ska komprimera topparna: {crest_pluck:.2} -> {crest_guitar:.2}"
    );
    let pitch = crate::audio::recorder::detect_pitch_hz(&guitar[4800..9600], 48_000.0)
        .unwrap_or(0.0);
    assert!((pitch - 220.0).abs() < 12.0, "strängen ska stämma: {pitch:.1} Hz");
    let late = (guitar[38_400..].iter().map(|x| x * x).sum::<f32>() / 9600.0).sqrt();
    assert!(late > 0.005, "gitarren ska klinga ut, inte dö direkt: {late:.4}");
}

/// **Control Room rör bara den motor som fick kommandot.** Samma not ger ljud i en
/// motor som inte vet om monitorläget, och tystnad i en som är tystad — det är
/// skillnaden mellan monitor och export.
#[test]
fn the_control_room_silences_only_the_engine_that_was_told() {
    let mut live = SynthEngine::new(48_000.0);
    let render_peak = |engine: &mut SynthEngine| {
        engine.handle_command(AudioCommand::NoteOn {
            note: 60,
            freq: 261.63,
            velocity: 1.0,
            channel: 0,
        });
        let mut peak: f32 = 0.0;
        for _ in 0..4_800 {
            let (l, r) = engine.process_stereo();
            peak = peak.max(l.abs()).max(r.abs());
        }
        peak
    };
    let loud = render_peak(&mut live);
    assert!(loud > 0.01, "motorn ska låta först: {loud}");

    live.handle_command(AudioCommand::SetControlRoom(
        crate::audio::command::ControlRoomState {
            level: 1.0,
            dim: false,
            mono: false,
            mute: true,
        },
    ));
    let muted = render_peak(&mut live);
    assert_eq!(muted, 0.0, "tyst monitor ska ge tystnad, fick {muted}");

    // En ny motor (den exporten bygger) har aldrig fått kommandot.
    let mut export = SynthEngine::new(48_000.0);
    let exported = render_peak(&mut export);
    assert!(exported > 0.01, "exportmotorn ska vara opåverkad: {exported}");
}

/// **Talkback hörs genom en dimmad monitor — och går inte in i mastern.** Micken fyller
/// monitorringen; med talkback av dämpas den av DIM som allt annat, med talkback på läggs
/// den på efter control room-steget och hörs i full nivå.
#[test]
fn talkback_is_heard_through_a_dimmed_monitor() {
    fn render(talkback: bool) -> f32 {
        let mut synth = SynthEngine::new(48_000.0);
        let ring = Arc::new(Mutex::new(vec![0.25f32; 24_000]));
        synth.handle_command(AudioCommand::SetMonitorRing {
            ring: Arc::clone(&ring),
        });
        synth.handle_command(AudioCommand::SetTalkback(talkback));
        synth.handle_command(AudioCommand::SetControlRoom(
            crate::audio::command::ControlRoomState {
                level: 1.0,
                dim: true,
                mono: false,
                mute: false,
            },
        ));
        let out = render_left(&mut synth, 4_800);
        rms(&out[480..])
    }

    let normal = render(false);
    let talking = render(true);
    assert!(talking > 0.1, "talkback ska höras: {talking:.4}");
    assert!(
        talking > normal * 5.0,
        "DIM dämpar musiken men inte pratet: av {normal:.4}, på {talking:.4}"
    );
}


/// **Talkback kan gå till en cue i stället för till monitorerna.** Med pratet på cue A hörs
/// det i A-lyssningen och inte i MIX — alltså i sångarens lurar, aldrig i en inspelning.
#[test]
fn talkback_can_be_routed_to_a_cue() {
    use crate::audio::control_room::CueListen;
    fn render(route: u8, listen: CueListen) -> f32 {
        let mut synth = SynthEngine::new(48_000.0);
        let ring = Arc::new(Mutex::new(vec![0.25f32; 24_000]));
        synth.handle_command(AudioCommand::SetMonitorRing {
            ring: Arc::clone(&ring),
        });
        synth.handle_command(AudioCommand::SetTalkback(true));
        synth.handle_command(AudioCommand::SetTalkbackCue(route));
        synth.handle_command(AudioCommand::SetCueListen(listen));
        let out = render_left(&mut synth, 4_800);
        rms(&out[480..])
    }

    let on_cue = render(1, CueListen::A);
    let on_main_with_cue_route = render(1, CueListen::Main);
    assert!(on_cue > 0.1, "pratet ska höras i cue A: {on_cue:.4}");
    assert!(
        on_main_with_cue_route < 0.01,
        "det ska inte läcka till MIX: {on_main_with_cue_route:.4}"
    );
}

/// **Bärvågen hörs bara när modulatorn säger till.** Synten är bärvåg och en stam är
/// modulator: med tyst modulator stängs bärvågen (amount 1,0), med en stam som spelar
/// öppnar den. Skillnaden mot en volymratt är att modulatorns **spektrum** styr, inte dess nivå.
#[test]
fn the_vocoder_silences_the_carrier_when_the_modulator_is_silent() {
    fn engine(with_stem: bool) -> SynthEngine {
        let mut synth = SynthEngine::new(48_000.0);
        if with_stem {
            let tone: Arc<Vec<f32>> = Arc::new(
                (0..48_000)
                    .map(|i| 0.5 * (std::f32::consts::TAU * 400.0 * i as f32 / 48_000.0).sin())
                    .collect(),
            );
            synth.handle_command(AudioCommand::LoadStemTrack {
                track_index: 0,
                left: tone.clone(),
                right: tone,
                sample_rate: 48_000.0,
                volume: 1.0,
                pan: 0.0,
                start_time_secs: 0.0,
            });
            synth.handle_command(AudioCommand::SetStemTrackRegions {
                track_index: 0,
                regions: vec![region_under_test(1.0, 0.0, 0.0, false, 1.0)],
            });
        }
        synth.handle_command(AudioCommand::SetSongPlayback(true));
        // Bärvågen måste ha energi i modulatorns band: en sinus på 880 Hz har inget vid
        // 400 Hz, och då blir vocodern tyst hur mycket modulator det än finns.
        synth.handle_command(AudioCommand::SetWaveform(
            crate::audio::command::Waveform::Saw,
        ));
        synth.handle_command(AudioCommand::SetMasterVocoder { amount: 1.0 });
        synth.handle_command(AudioCommand::NoteOn {
            note: 45,
            freq: 110.0,
            velocity: 1.0,
            channel: 0,
        });
        synth
    }

    let mut dry = engine(false);
    let carrier_only = rms(&render_left(&mut dry, 9_600)[4_800..]);
    let mut modulated = engine(true);
    let with_modulator = rms(&render_left(&mut modulated, 9_600)[4_800..]);
    assert!(
        carrier_only < with_modulator * 0.5,
        "utan modulator ska bärvågen stängas: {carrier_only:.4} mot {with_modulator:.4}"
    );
    assert!(with_modulator > 0.02, "med modulator ska den höras: {with_modulator:.4}");
}

/// **Sustainpedalen håller tonen efter att tangenten släppts.**
///
/// Två körningar med samma anslag: en med pedalen nere, en utan. Efter 200 ms ska
/// pedalkörningen vara klart starkare — och när pedalen går upp ska den falla.
/// Jämförelsen är relativ eftersom masterns FX-svans annars kan lura ett absolut mått.
#[test]
fn the_sustain_pedal_holds_a_note_after_the_key_is_released() {
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();

    let run = |pedal: bool| {
        let mut engine = SynthEngine::new(48_000.0);
        engine.filter_params.cutoff = 12_000.0;
        engine.filter_env_amount = 0.0;
        engine.adsr = crate::audio::envelope::AdsrParams {
            attack: 0.002,
            decay: 0.001,
            sustain: 0.8,
            release: 0.05,
        };
        if pedal {
            engine.handle_command(AudioCommand::SetSustainPedal(true));
        }
        engine.handle_command(AudioCommand::NoteOn {
            note: 60,
            freq: 261.63,
            velocity: 1.0,
            channel: 0,
        });
        for _ in 0..4_800 {
            engine.process_stereo();
        }
        engine.handle_command(AudioCommand::NoteOff { note: 60, channel: 0 });

        // 200 ms efter släppet.
        let mut held = Vec::new();
        for _ in 0..9_600 {
            held.push(engine.process_stereo().0);
        }
        let held_level = rms(&held[7_200..]);

        // Pedalen upp: releasen (50 ms) ska hinna göra sitt.
        engine.handle_command(AudioCommand::SetSustainPedal(false));
        let mut after = Vec::new();
        for _ in 0..14_400 {
            after.push(engine.process_stereo().0);
        }
        (held_level, rms(&after[12_000..]))
    };

    let (pedal_held, pedal_after) = run(true);
    let (plain_held, _) = run(false);

    println!(
        "  pedal: hållen {:.4}, efter upp {:.4} | utan pedal: {:.4}",
        pedal_held, pedal_after, plain_held
    );
    assert!(
        pedal_held > plain_held * 3.0,
        "pedalen ska hålla tonen: {pedal_held:.4} mot {plain_held:.4} utan pedal"
    );
    assert!(
        pedal_after < pedal_held * 0.5,
        "när pedalen går upp ska tonen falla: {pedal_after:.4} av {pedal_held:.4}"
    );
}

/// **Pedalen håller också en sampling** (FL Keys: ett pianosampel ska klinga ut när pedalen är
/// nere, och dämpas när den går upp).
///
/// Utan pedal rör not-av inte en en-skottsröst — det är det gamla beteendet. Med pedalen nere
/// ska nivån stå kvar 200 ms efter släppet, och efter pedal-upp falla inom 40 ms-rampen.
#[test]
fn the_pedal_holds_a_sample_and_damps_it_when_it_lifts() {
    let mut synth = SynthEngine::new(48_000.0);
    let n = 96_000usize;
    let tone: Vec<f32> = (0..n)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin())
        .collect();
    let arc = Arc::new(tone);
    synth.handle_command(AudioCommand::SetSustainPedal(true));
    synth.handle_command(AudioCommand::TriggerSampleVoice {
        left: arc.clone(),
        right: arc,
        sample_rate: 48_000,
        base_note: 60,
        note: 60,
        pitch_semitones: 0,
        pitch_cents: 0.0,
        velocity: 1.0,
        velocity_sensitivity: 1.0,
        velocity_curve: crate::audio::envelope::VelocityCurve::Linear,
        volume: 1.0,
        reverse: false,
        start01: 0.0,
        end01: 1.0,
        channel: 0,
        loop_mode: LoopMode::Off,
        loop_start01: 0.0,
        loop_end01: 1.0,
        ping_pong: false,
        amp_env: crate::audio::envelope::AdsrParams::identity(),
        filter: SamplerFilter::default(),
        hold_secs: 0.0,
        release: None,
    });

    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let mut before = Vec::new();
    for _ in 0..4_800 {
        before.push(synth.process_stereo().0);
    }
    synth.handle_command(AudioCommand::NoteOff { note: 60, channel: 0 });
    let mut held = Vec::new();
    for _ in 0..9_600 {
        held.push(synth.process_stereo().0);
    }
    synth.handle_command(AudioCommand::SetSustainPedal(false));
    let mut after = Vec::new();
    for _ in 0..9_600 {
        after.push(synth.process_stereo().0);
    }

    let level_before = rms(&before[2_400..]);
    let level_held = rms(&held[7_200..]);
    let level_after = rms(&after[7_200..]);
    println!(
        "  före {:.4}, hållen {:.4}, efter pedal-upp {:.4}",
        level_before, level_held, level_after
    );
    assert!(
        level_held > level_before * 0.8,
        "samplingen ska klinga vidare med pedalen: {level_held:.4} av {level_before:.4}"
    );
    // Rösten ska vara **stoppad**, inte bara tystare: masterns FX-svans ligger kvar en stund
    // och gör ett absolut mått på "tyst" fel.
    assert_eq!(
        synth.sample_voices.iter().filter(|v| v.is_active()).count(),
        0,
        "pedalen upp ska stoppa rösten"
    );
    assert!(
        level_after < level_held * 0.5,
        "pedalen upp ska dämpa: {level_after:.4} av {level_held:.4}"
    );
}

/// **Släppljudet spelar när tonen släpps** — och utan släppljud händer ingenting, som förut.
///
/// Huvudsamplingen är **tyst** i det här provet, så allt som hörs efter not-av är dämparens ljud.
#[test]
fn a_release_sample_plays_after_the_key_is_let_go() {
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    let run = |with_release: bool| {
        let mut synth = SynthEngine::new(48_000.0);
        let main = Arc::new(vec![0.0f32; 4_800]);
        let rel = Arc::new(
            (0..4_800)
                .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 880.0 * i as f32 / 48_000.0).sin())
                .collect::<Vec<f32>>(),
        );
        synth.handle_command(AudioCommand::TriggerSampleVoice {
            left: main.clone(),
            right: main,
            sample_rate: 48_000,
            base_note: 60,
            note: 60,
            pitch_semitones: 0,
            pitch_cents: 0.0,
            velocity: 1.0,
            velocity_sensitivity: 1.0,
            velocity_curve: crate::audio::envelope::VelocityCurve::Linear,
            volume: 1.0,
            reverse: false,
            start01: 0.0,
            end01: 1.0,
            channel: 0,
            loop_mode: LoopMode::Off,
            loop_start01: 0.0,
            loop_end01: 1.0,
            ping_pong: false,
            amp_env: crate::audio::envelope::AdsrParams::identity(),
            filter: SamplerFilter::default(),
            hold_secs: 10.0,
            release: with_release.then(|| (rel.clone(), 48_000)),
        });
        for _ in 0..4_800 {
            synth.process_stereo();
        }
        synth.handle_command(AudioCommand::NoteOff { note: 60, channel: 0 });
        let mut after = Vec::new();
        for _ in 0..4_800 {
            after.push(synth.process_stereo().0);
        }
        println!("  släppljud {}: {:.4}", with_release, rms(&after[1_000..]));
        rms(&after[1_000..])
    };
    let with = run(true);
    let without = run(false);
    assert!(with > 0.1, "släppljudet ska höras: {with:.4}");
    assert!(without < 1e-4, "utan släppljud ska det vara tyst: {without:.6}");
}

/// Ett JSFX-instrument som källa: tyst utan not, hörs efter `NoteOn`, tystnar på `NoteOff` — och
/// syntens egna röster hörs inte alls medan instrumentet är satt.
#[test]
fn a_jsfx_instrument_can_be_the_synth_source() {
    let path = std::env::temp_dir().join(format!("sonix-src-{}.jsfx", std::process::id()));
    std::fs::write(
        &path,
        "@init\ng = 0;\n@sample\nwhile (midirecv(offset, m1, m2, m3)) (\n  st = m1 & 240;\n  st == 144 ? g = 0.5;\n  st == 128 ? g = 0;\n);\nspl0 = g; spl1 = g;\n",
    )
    .expect("skriv jsfx");

    let mut synth = SynthEngine::new(44_100.0);
    synth.handle_command(AudioCommand::SetInstrument { path: Some(path.display().to_string()) });
    // Ett block är 512 frames: noten gäller från nästa block, så mät över två.
    let peak = |s: &mut SynthEngine| {
        (0..1024).map(|_| s.process_stereo().0.abs()).fold(0.0f32, f32::max)
    };
    assert_eq!(peak(&mut synth), 0.0, "tyst utan not");

    synth.handle_command(AudioCommand::NoteOn { note: 60, freq: 261.6, velocity: 1.0, channel: 0 });
    // Instrumentet ligger på syntens masternivå: hörs, men inte på rå 0,5 (gain + tanh).
    let level = peak(&mut synth);
    assert!((0.2..0.5).contains(&level), "noten ska höras på masternivå, fick {level}");

    synth.handle_command(AudioCommand::NoteOff { note: 60, channel: 0 });
    let _ = peak(&mut synth); // blocket som redan renderats med noten klingar ut först
    assert_eq!(peak(&mut synth), 0.0, "tyst efter not av");

    synth.handle_command(AudioCommand::SetInstrument { path: None });
    let _ = std::fs::remove_file(path);
}
