//! Motorns prov: tidmappning, sträckning, regioner, kantrampen och klockan.
//!
//! Flyttade ur `synth.rs` 2026-09-14 (motorn var 3 517 rader, varav proven 1 518).
//! Status: stabil — prov, inte kod; flyttar du en funktion, flytta dess prov.

use super::*;

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
        let sa = a.next_sample(Waveform::Saw, &adsr, &flt, &env, 0.0);
        let sb = b.next_sample(Waveform::Saw, &adsr, &flt, &other_env, 0.0);
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
        let a = dry.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 0.0);
        let b = wet.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 4.0);
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
        first.next_sample(Waveform::Saw, &adsr, &flt, &fenv, 2.0);
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

/// En send till en buss med lägre nivå **hörs**: spåret går till sin egen buss
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
        velocity: 1.0,
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
        hold_secs,
    });
    let _ = synth.process_stereo();
    synth
}

fn voice(synth: &SynthEngine) -> &SampleVoice {
    synth
        .sample_voices
        .iter()
        .find(|v| v.active)
        .expect("rösten ska vara igång")
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
