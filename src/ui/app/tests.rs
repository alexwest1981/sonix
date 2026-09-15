//! Prov för app-ytan — flyttade ur `app.rs` 2026-09-14 (filen hade vuxit till 23 000 rader).
//!
//! Status: byggs — proven hör till sina funktioner; flyttar du en funktion, flytta dess prov.
//! Rör inte: proven läser privata fält genom `use super::*;`, så de måste ligga under `app`.

use super::*;
// Moduler vars item bara proven använder re-exporteras inte (det skulle vara en
// oanvänd import i binärbygget, och CI fäller på varningar).
use super::midi::{apply_bar_to_pattern, drum_channel_for_key, midi_channel_for_track, notes_by_bar,
                  pattern_bar_notes, ARRANGEMENT_BARS, STEPS_PER_BAR};
use super::transport::steps_elapsed;   // stegklockan (Fas 8.13b) — modulen re-exporteras inte

    /// Minimal projektfil — fälten utanför `serde(default)` måste anges.
    fn minimal_project_json(name: &str) -> String {
        format!(
            r#"{{"name":"{name}","bpm":128.0,"swing":0.0,"master_volume":1.0,"master_pan":0.0,"tracks":[]}}"#
        )
    }

    fn isolated_paths(tag: &str) -> crate::paths::Paths {
        let root = std::env::temp_dir().join(format!(
            "sonix_app_test_{}_{}_{}",
            tag,
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("tmpdir");
        crate::paths::Paths::with_home(root)
    }

    /// Kärnan i Fas 6.1: vid start ska bara arbete som *inte* finns i den
    /// manuella filen erbjudas — annars blir varningen brus.
    /// Mixer-undon (Fas 6.2): varje fält som motorn tar emot måste ingå i
    /// sammanfattningen, annars kan en mixerändring ske helt utan att en
    /// ångringspunkt skapas. Fälten räknas upp i samma ordning som
    /// `sync_track_audio_state` och `sync_group_state` skickar dem.
    /// Testpattern med åtta kanaler, som appen bygger dem.
    fn test_pattern() -> Pattern {
        Pattern {
            name: "Testpattern".to_string(),
            color: Color32::WHITE,
            channel_steps: vec![[false; 16]; 8],
            channel_notes: vec![[60; 16]; 8],
            piano_roll_grid: [[false; 16]; 24],
            take: crate::midi_take::Take::default(),
        }
    }

    #[test]
    fn pattern_bar_notes_maps_drums_synth_and_bass() {
        use crate::audio::smf::TICKS_PER_STEP_16TH;
        let mut pat = test_pattern();
        // Bastrumma på steg 0 och 8 (kanal 0), virvel på steg 4 (kanal 1).
        pat.channel_steps[0][0] = true;
        pat.channel_steps[0][8] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_notes[0][8] = 36;
        pat.channel_steps[1][4] = true;
        pat.channel_notes[1][4] = 38;
        // Synth: rutnätet, rad 12 = MIDI 60, på steg 2.
        pat.piano_roll_grid[12][2] = true;
        // Bas: kanal 7, MIDI 40, på steg 3.
        pat.channel_steps[7][3] = true;
        pat.channel_notes[7][3] = 40;

        let vels = [100u8; 16];
        let drums = pattern_bar_notes(&pat, TrackKind::Drums, 0, 0, &vels);
        assert_eq!(drums.len(), 3, "tre trumslag: {drums:?}");
        assert!(drums.iter().all(|n| n.channel == 9), "trummor ska ligga på kanal 9");
        assert!(drums.iter().any(|n| n.key == 36 && n.start == 0));
        assert!(drums.iter().any(|n| n.key == 36 && n.start == 8 * TICKS_PER_STEP_16TH));
        assert!(drums.iter().any(|n| n.key == 38 && n.start == 4 * TICKS_PER_STEP_16TH));

        let synth = pattern_bar_notes(&pat, TrackKind::SynthLead, 3, 0, &vels);
        assert_eq!(synth.len(), 1);
        assert_eq!(synth[0].key, 60);
        assert_eq!(synth[0].channel, 3);
        assert_eq!(synth[0].start, 2 * TICKS_PER_STEP_16TH);

        let bass = pattern_bar_notes(&pat, TrackKind::Bassline, 4, 0, &vels);
        assert_eq!(bass.len(), 1);
        assert_eq!(bass[0].key, 40);
        assert_eq!(bass[0].start, 3 * TICKS_PER_STEP_16TH);

        // En takt längre fram flyttar noterna i tid.
        let bar2 = pattern_bar_notes(&pat, TrackKind::Drums, 0, 16, &vels);
        assert!(bar2.iter().all(|n| n.start >= 16 * TICKS_PER_STEP_16TH));
    }

    #[test]
    fn exported_midi_can_be_imported_back_with_the_same_notes() {
        // "Klart när": en fil exporterad från Sonix ska kunna läsas tillbaka
        // till samma noter och längder. Här hela vägen genom kodningen.
        use crate::audio::smf;
        let mut pat = test_pattern();
        pat.channel_steps[0][0] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_steps[1][2] = true;
        pat.channel_notes[1][2] = 38;
        pat.piano_roll_grid[0][5] = true; // MIDI 48
        pat.piano_roll_grid[23][15] = true; // MIDI 71
        pat.channel_steps[7][9] = true;
        pat.channel_notes[7][9] = 43;

        let vels = [100u8; 16];
        let mut notes = pattern_bar_notes(&pat, TrackKind::Drums, 9, 0, &vels);
        notes.extend(pattern_bar_notes(&pat, TrackKind::SynthLead, 0, 0, &vels));
        notes.extend(pattern_bar_notes(&pat, TrackKind::Bassline, 1, 0, &vels));
        let exported = smf::write_midi(120.0, &[smf::MidiTrack {
            name: "Allt".to_string(),
            notes: notes.clone(),
        }]);

        let parsed = smf::parse_midi(&exported).expect("egen fil ska gå att läsa");
        let back: Vec<smf::MidiNote> = parsed
            .notes_with_track()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect();
        assert_eq!(parsed.bpm.map(|b| b.round()), Some(120.0));
        assert_eq!(back.len(), notes.len(), "samma antal noter tillbaka");

        // Tillbaka in i ett tomt pattern: samma rutor ska tändas igen.
        let mut target = test_pattern();
        let report = apply_bar_to_pattern(&back, parsed.ppq, &mut target);
        assert_eq!(report.dropped_beyond_arrangement, 0);
        assert_eq!(report.dropped_out_of_range, 0);
        assert_eq!(report.notes_placed, notes.len());
        assert!(target.channel_steps[0][0], "bastrumman tillbaka på steg 0");
        assert_eq!(target.channel_notes[0][0], 36);
        assert!(target.channel_steps[1][2], "virveln tillbaka på steg 2");
        assert!(target.piano_roll_grid[0][5], "MIDI 48 → rad 0");
        assert!(target.piano_roll_grid[23][15], "MIDI 71 → rad 23");
        assert!(target.channel_steps[7][9], "basen tillbaka på steg 9");
        assert_eq!(target.channel_notes[7][9], 43);
    }

    #[test]
    /// En not i en *annan* takt kastas inte längre — den hör till sin egen takt
    /// och blir ett eget mönster (se runtgångstestet). Det som fortfarande
    /// redovisas som tappat är det som inte får plats i rutnätet alls.
    fn midi_import_reports_notes_outside_the_grid() {
        use crate::audio::smf::MidiNote;
        let notes = vec![
            // Går bra: trumma på steg 0.
            MidiNote { start: 0, length: 10, channel: 9, key: 36, velocity: 100 },
            // Okänd trumtangent (claves 75).
            MidiNote { start: 0, length: 10, channel: 9, key: 75, velocity: 100 },
            // Ton under piano-rollen men spelbar av basspåret.
            MidiNote { start: 0, length: 10, channel: 0, key: 30, velocity: 100 },
            // Ton ovanför piano-rollens 48–71: tappas, och räknas.
            MidiNote { start: 0, length: 10, channel: 0, key: 90, velocity: 100 },
        ];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, crate::audio::smf::PPQ, &mut pat);
        assert_eq!(rep.notes_placed, 2, "trumman och den låga bastonen");
        assert_eq!(rep.dropped_out_of_range, 2, "trumtangenten 75 och tonen på 90");
        assert_eq!(rep.dropped_beyond_arrangement, 0, "allt låg i den här takten");
        assert!(pat.channel_steps[0][0]);
        assert!(
            pat.channel_steps[7][0] && pat.channel_notes[7][0] == 30,
            "tonen under 48 ska hamna på baskanalen"
        );
    }

    #[test]
    fn midi_import_follows_the_files_own_resolution() {
        // En fil med 96 PPQ (vanligt från andra DAW:er) ska hamna på rätt steg,
        // inte skalas fel.
        use crate::audio::smf::MidiNote;
        let notes = vec![MidiNote {
            start: 96, // = fjärdedelsnot = steg 4 vid 96 PPQ
            length: 24,
            channel: 0,
            key: 60,
            velocity: 100,
        }];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, 96, &mut pat);
        assert_eq!(rep.notes_placed, 1);
        assert!(!pat.piano_roll_grid[12][0]);
        assert!(pat.piano_roll_grid[12][4], "96 tick vid 96 PPQ är steg 4");
    }

    #[test]
    fn midi_channels_skip_percussion() {
        // Kanal 9 är percussion, så melodiska spår får aldrig den kanalen.
        assert_eq!(midi_channel_for_track(0), 0);
        assert_eq!(midi_channel_for_track(8), 8);
        assert_eq!(midi_channel_for_track(9), 10);
        for t in 0..16 {
            assert_ne!(midi_channel_for_track(t), 9, "spår {t} fick percussionkanalen");
        }
        assert_eq!(drum_channel_for_key(36), Some(0));
        assert_eq!(drum_channel_for_key(38), Some(1));
        assert_eq!(drum_channel_for_key(42), Some(3));
        assert_eq!(drum_channel_for_key(49), Some(5));
        assert_eq!(drum_channel_for_key(75), None);
    }

    /// Kanal med varje fält satt till ett omisskännligt värde, så att ett
    /// bortglömt fält i sparandet syns direkt.
    fn test_channel() -> ChannelStrip {
        ChannelStrip {
            name: "Virvel".to_string(),
            icon: "🥁".to_string(),
            color: Color32::from_rgba_premultiplied(10, 20, 30, 255),
            volume: 0.42,
            pan: -0.25,
            muted: true,
            solo: true,
            steps: [true; 16],
            notes: [42; 16],
            pitch_semitones: -7,
            pitch_fine_cents: 12.5,
            sample_start: 0.125,
            sample_end: 0.875,
            slices: vec![(0.0, 0.125), (0.125, 0.875), (0.875, 1.0)],
            attack_decay: 0.375,
            loop_mode: crate::audio::LoopMode::Off,
            sample_loop_start: 0.0,
            sample_loop_end: 1.0,
            ping_pong: false,
            amp_env: crate::audio::envelope::AdsrParams::identity(),
            is_reverse: true,
            waveform_preview: vec![0.5; 4],
            sample_path: Some("/tmp/sonix-finns-inte.wav".to_string()),
            pcm_audio: None,
            sample_base_note: 43,
        }
    }

    #[test]
    fn saved_channel_round_trip_keeps_every_field() {
        // Fullständighetsvakt (Fas 6.7): läggs ett fält till i ChannelStrip utan
        // att följa med i sparandet, failar det här testet i stället för att
        // tyst tappa inställningen när projektet öppnas igen.
        let ch = test_channel();
        let json = serde_json::to_string(&channel_to_saved(&ch)).unwrap();
        let back: SavedChannel = serde_json::from_str(&json).unwrap();
        let r = saved_to_channel(&back);

        assert_eq!(r.name, ch.name);
        assert_eq!(r.icon, ch.icon);
        assert_eq!(r.color.to_array(), ch.color.to_array());
        assert_eq!(r.volume, ch.volume);
        assert_eq!(r.pan, ch.pan);
        assert_eq!(r.muted, ch.muted);
        assert_eq!(r.solo, ch.solo);
        assert_eq!(r.steps, ch.steps);
        assert_eq!(r.notes, ch.notes);
        assert_eq!(r.pitch_semitones, ch.pitch_semitones);
        assert_eq!(r.pitch_fine_cents, ch.pitch_fine_cents);
        assert_eq!(r.sample_start, ch.sample_start);
        assert_eq!(r.sample_end, ch.sample_end);
        assert_eq!(r.slices, ch.slices, "slicekartan ska med i projektfilen");
        assert_eq!(r.attack_decay, ch.attack_decay);
        assert_eq!(r.is_reverse, ch.is_reverse);
        assert_eq!(r.sample_path, ch.sample_path);
        assert_eq!(r.sample_base_note, ch.sample_base_note);
        // Ljudet sparas inte utan läses från disk: sökvägen finns inte här, så
        // både PCM och vågform ska vara tomma — men sökvägen själv ska med.
        assert!(r.pcm_audio.is_none());
        assert!(r.waveform_preview.is_empty());
    }

    #[test]
    fn project_file_carries_the_notes_and_the_drum_rack() {
        // "Klart när" för Fas 6.7: en sparad fil ska ge tillbaka noterna, inte
        // bara spåren. Före den här ändringen fanns varken `patterns` eller
        // `channels` i projektformatet — musiken tappades vid varje sparning.
        let mut pat = test_pattern();
        pat.channel_steps[0][0] = true;
        pat.channel_notes[0][0] = 36;
        pat.channel_steps[3][7] = true;
        pat.piano_roll_grid[12][5] = true;
        pat.piano_roll_grid[0][15] = true;

        let mut ch = test_channel();
        ch.steps[2] = true;
        ch.notes[2] = 42;

        let mut step_velocities = [1.0f32; 16];
        step_velocities[3] = 0.25;

        let data = SonixProjectData {
            name: "Testprojekt".to_string(),
            bpm: 133.0,
            follow_tempo: false,
            tempo_points: Vec::new(),
            song_key_root: 3,
            song_key_scale: 0,
            swing: 0.2,
            master_volume: 0.8,
            master_pan: 0.0,
            tracks: Vec::new(),
            plugin_slots: Vec::new(),
            bus_volume: default_bus_volume(),
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_automation: Vec::new(),
            plugin_automation: Vec::new(),
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: default_vca_volume(),
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
            patterns: vec![pattern_to_saved(&pat)],
            selected_pattern: 0,
            step_velocities: Some(step_velocities),
            channels: vec![channel_to_saved(&ch)],
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(
            json.contains("piano_roll_grid"),
            "noterna ska stå i filen, inte bara i minnet"
        );
        assert!(json.contains("channel_steps"));

        let back: SonixProjectData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patterns.len(), 1);
        let restored = saved_to_pattern(&back.patterns[0]);
        assert_eq!(restored.channel_steps, pat.channel_steps, "trumstegen tillbaka");
        assert_eq!(restored.channel_notes, pat.channel_notes, "trumtonerna tillbaka");
        assert_eq!(restored.piano_roll_grid, pat.piano_roll_grid, "piano-rollen tillbaka");
        assert_eq!(restored.name, pat.name);
        assert_eq!(restored.color.to_array(), pat.color.to_array());
        assert_eq!(back.channels.len(), 1);
        assert_eq!(back.channels[0].steps, ch.steps);
        assert_eq!(back.channels[0].notes, ch.notes);
        assert_eq!(back.step_velocities.unwrap()[3], 0.25, "stegvolymen tillbaka");
        assert_eq!(back.selected_pattern, 0);
        assert!(!back.follow_tempo, "switchen ska med i filen, inte bara i minnet");
    }

    #[test]
    fn old_project_files_without_patterns_still_load() {
        // Bakåtkompatibilitet (Fas 6.7): en fil skriven före den här ändringen
        // har varken patterns, channels eller step_velocities. Den ska fortfarande
        // läsas, och då lämnas standardpatterns och standardracket orörda.
        let old = r#"{
            "name": "Gammalt",
            "bpm": 120.0,
            "swing": 0.0,
            "master_volume": 0.9,
            "master_pan": 0.0,
            "tracks": []
        }"#;
        let data: SonixProjectData = serde_json::from_str(old).expect("gammal fil ska gå att läsa");
        assert!(data.patterns.is_empty());
        assert!(data.channels.is_empty());
        assert_eq!(data.selected_pattern, 0);
        assert!(
            data.step_velocities.is_none(),
            "en gammal fil ska inte påstå något om stegvolymer"
        );
        assert_eq!(data.bpm, 120.0);
        assert!(
            data.follow_tempo,
            "en fil som sparades innan fältet fanns ska läsa som PÅ — annars hade en \
             uppgradering tyst stängt av tempoföljningen för alla gamla projekt"
        );
    }

    #[test]
    fn restoring_a_saved_project_puts_the_notes_back() {
        // Inläsningsvägen (Fas 6.7): appen står med tomma standardpatterns och
        // ett tomt rack, filen har noterna — efter återställningen ska arbetet
        // finnas i appen igen.
        let mut app_patterns = vec![test_pattern(), test_pattern()];
        let mut app_channels = vec![test_channel(), test_channel()];
        let mut app_selected = 0usize;
        let mut app_velocities = [1.0f32; 16];

        let mut saved_pattern = test_pattern();
        saved_pattern.name = "Beat".to_string();
        saved_pattern.channel_steps[2][3] = true; // bastrumma på steg 3
        saved_pattern.channel_notes[2][3] = 36;
        saved_pattern.piano_roll_grid[4][11] = true;
        let mut saved_channel = test_channel();
        saved_channel.steps[5] = true;
        saved_channel.notes[5] = 49;
        let mut velocities = [1.0f32; 16];
        velocities[6] = 0.4;

        let saved = SavedMusic {
            patterns: &[pattern_to_saved(&saved_pattern)],
            // Filen pekar på ett patternnummer som inte finns här: ska klämmas
            // till ett giltigt index i stället för att lämna appen utanför listan.
            selected_pattern: 99,
            step_velocities: Some(velocities),
            channels: &[channel_to_saved(&saved_channel)],
        };
        restore_saved_music(
            &saved,
            &mut app_patterns,
            &mut app_channels,
            &mut app_selected,
            &mut app_velocities,
        );

        assert_eq!(app_patterns[0].name, "Beat");
        assert!(app_patterns[0].channel_steps[2][3], "trumslaget tillbaka");
        assert_eq!(app_patterns[0].channel_notes[2][3], 36);
        assert!(app_patterns[0].piano_roll_grid[4][11], "piano-rollen tillbaka");
        assert_eq!(app_selected, app_patterns.len() - 1, "valt pattern kläms till listan");
        assert_eq!(app_velocities[6], 0.4, "stegvolymen tillbaka");
        assert!(app_channels[0].steps[5], "kanalens steg tillbaka");
        assert_eq!(app_channels[0].notes[5], 49);
    }

    #[test]
    fn restoring_an_old_project_leaves_the_defaults_alone() {
        // En fil skriven före Fas 6.7 har inga mönster. Då ska appens egna
        // pattern och rack stå kvar — inte nollställas.
        let mut app_patterns = vec![test_pattern()];
        app_patterns[0].name = "Mitt eget".to_string();
        app_patterns[0].channel_steps[1][1] = true;
        let mut app_channels = vec![test_channel()];
        app_channels[0].name = "Min kanal".to_string();
        let mut app_selected = 0usize;
        let mut app_velocities = [0.5f32; 16];

        let saved = SavedMusic {
            patterns: &[],
            selected_pattern: 0,
            step_velocities: None,
            channels: &[],
        };
        restore_saved_music(
            &saved,
            &mut app_patterns,
            &mut app_channels,
            &mut app_selected,
            &mut app_velocities,
        );

        assert_eq!(app_patterns.len(), 1);
        assert_eq!(app_patterns[0].name, "Mitt eget");
        assert!(app_patterns[0].channel_steps[1][1]);
        assert_eq!(app_channels[0].name, "Min kanal");
        assert_eq!(app_velocities[0], 0.5, "stegvolymerna rörs inte");
    }

    #[test]
    fn the_recorded_take_survives_the_project_round_trip() {
        // Fas 6.4: tagningens tajming är också arbete — den ska med i filen.
        let mut pat = test_pattern();
        pat.take.push(2.75, 60, 0.8);
        pat.take.push(6.1, 64, 0.6);
        assert!(pat.take.tightness() > 0.0);

        let json = serde_json::to_string(&pattern_to_saved(&pat)).unwrap();
        assert!(json.contains("take"), "tagningen ska stå i filen");
        let back: SavedPattern = serde_json::from_str(&json).unwrap();
        let restored = saved_to_pattern(&back);

        assert_eq!(restored.take, pat.take, "samma tagning tillbaka");
        assert!((restored.take.tightness() - pat.take.tightness()).abs() < 1e-6);
    }

    #[test]
    fn an_old_pattern_file_loads_with_an_empty_take() {
        // Bakåtkompatibilitet: ett pattern sparat före Fas 6.4 har ingen tagning.
        let old = r#"{
            "name": "Gammalt pattern",
            "channel_steps": [[true, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]],
            "channel_notes": [[36, 38, 39, 42, 46, 49, 36, 38, 39, 42, 46, 49, 36, 38, 39, 42]],
            "piano_roll_grid": [[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]]
        }"#;
        // piano_roll_grid är 24 rader i appen; testet nedan använder rätt form.
        let old = old.replace(
            r#""piano_roll_grid": [[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]]"#,
            &format!(
                r#""piano_roll_grid": [{}]"#,
                vec![
                    "[false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false]";
                    24
                ]
                .join(",")
            ),
        );
        let back: SavedPattern = serde_json::from_str(&old).expect("gammalt pattern ska gå att läsa");
        assert!(back.take.is_empty(), "ingen tagning i en gammal fil");
        let restored = saved_to_pattern(&back);
        assert_eq!(restored.name, "Gammalt pattern");
        assert!(restored.channel_steps[0][0], "stegen läses som förut");
        assert!(restored.take.is_empty());
    }

    #[test]
    fn a_late_scan_does_not_throw_away_a_fresh_import() {
        // Fas 7.4: skanningen tar minuter. Importerar användaren ett eget sample
        // under tiden får den färdiga skanningen inte skriva över den.
        let scanned = vec![
            test_library_item(0, "Kick", "/samples/kick.wav"),
            test_library_item(1, "Snare", "/samples/snare.wav"),
        ];
        let existing = vec![
            test_library_item(9, "Mitt eget", "/home/alex/Music/Sonix/Samples/mitt.wav"),
            // En post som skanningen också hittade: ska inte bli dubbel.
            test_library_item(1, "Snare", "/samples/snare.wav"),
        ];
        let merged = merge_library(scanned.clone(), &existing);

        assert_eq!(merged.len(), 3, "två skannade + en egen import");
        assert!(merged.iter().any(|i| i.name == "Mitt eget"));
        assert_eq!(
            merged
                .iter()
                .filter(|i| i.file_path.as_deref() == Some("/samples/snare.wav"))
                .count(),
            1,
            "ingen dubblett av det skanningen hittade"
        );
    }

    fn test_library_item(id: usize, name: &str, path: &str) -> LibrarySampleItem {
        LibrarySampleItem {
            id,
            name: name.to_string(),
            category: "Test".to_string(),
            icon: "🎵".to_string(),
            default_note: 60,
            color: Color32::WHITE,
            waveform: Vec::new(),
            file_path: Some(path.to_string()),
        }
    }

    /// De tre svaren i tidslinjen (Fas 8.10 steg 2), som en tabell.
    ///
    /// Regeln är rena funktioner just för att den ska kunna prövas utan fönster — och
    /// för att tidslinjen, ritningen och exporten ska kunna svara likadant.
    #[test]
    fn the_tempo_follow_mapping_has_three_answers() {
        use crate::audio::stretch::{decide, FollowMode};
        // Okänt inspelningstempo: rör inte ljudet.
        let unknown = decide(0.0, 150.0, true, false);
        assert_eq!(unknown.mode, FollowMode::Untouched);
        assert_eq!(playback_for(unknown, false), (1.0, false));
        // Samma tempo: inget att göra.
        let same = decide(120.0, 120.0, true, false);
        assert_eq!(same.mode, FollowMode::Untouched);
        assert_eq!(playback_for(same, false), (1.0, false));
        // Switchen av: inget klipp följer tempot, varken sträckt eller bandspelare.
        let off = decide(120.0, 150.0, false, false);
        assert_eq!(off.mode, FollowMode::Untouched);
        assert_eq!(playback_for(off, false), (1.0, false));
        // Bandspelarläget: faktorn, aldrig en fil.
        let tape = decide(120.0, 150.0, true, true);
        assert_eq!(tape.mode, FollowMode::Tape);
        let (rate, file) = playback_for(tape, true);
        assert!((rate - 1.25).abs() < 1e-5, "bandspelarens faktor: {rate}");
        assert!(!file, "bandspelarläget spelar aldrig en sträckt fil");
        // Sträckning med filen inte klar: originalet i sitt eget tempo — ingen smurf.
        let stretch = decide(120.0, 150.0, true, false);
        assert_eq!(stretch.mode, FollowMode::Stretch);
        assert_eq!(playback_for(stretch, false), (1.0, false));
        // ... och med filen klar: filen spelas med faktor 1,0.
        assert_eq!(playback_for(stretch, true), (1.0, true));
    }

    /// Och raden ska komma **vid inläsningen**, inte först när tempot rörs: ett projekt
    /// vars klipp saknar mått ser annars ut som ett projekt där kontrollen är död.
    #[test]
    fn the_load_message_carries_the_same_note() {
        let opened = "📂 Öppnade projekt 'Rock and Hard Place'!";
        let note = tempo_change_note(0, 9, None).expect("nio klipp utan mått ska sägas");
        assert!(
            format!("{opened} {note}").contains('9'),
            "inläsningsraden ska bära antalet"
        );
        // Och ett projekt där allt följer får ingen extra rad.
        assert_eq!(tempo_change_note(9, 0, None), None);
    }

    /// **Felet i Alex' "inget hände".**
    ///
    /// Rock and Hard Place: alla nio klipp saknar känt inspelningstempo, så inget följer
    /// tempot. Appen sade ingenting alls, och en kontroll som tiger ser ut att vara död.
    /// Nu säger den vad som gäller — och **vilken** åtgärd som finns.
    #[test]
    fn a_tempo_change_that_nothing_follows_is_never_silent() {
        // Alla klipp står still: det här är fallet som såg ut som en död kontroll.
        let all_stuck = tempo_change_note(0, 9, None).expect("nio stillastående klipp ska sägas högt");
        assert!(all_stuck.contains('9'), "antalet ska stå i raden: {all_stuck}");
        // Texten är i18n:ad (testkörningen får engelska), så provet håller sig till det som
        // är lika i båda: antalet och pekaren till åtgärden.
        assert!(
            all_stuck.contains('⏱'),
            "raden ska peka på åtgärden: {all_stuck}"
        );

        // Delat läge: både de som följer och de som står still ska räknas.
        let mixed = tempo_change_note(3, 2, None).expect("delat läge ska sägas");
        assert!(mixed.contains('3') && mixed.contains('2'), "{mixed}");

        // Och när allt följer finns inget att säga — ingen rad, ingen tystnad att förklara.
        assert_eq!(tempo_change_note(9, 0, None), None);
        assert_eq!(tempo_change_note(0, 0, None), None);

        // **Går tempot att räkna ur klippen ska talet stå i raden**, inte bara en
        // hänvisning: "öppna Tempokarta" är ett steg för mycket när svaret redan finns.
        let measured = tempo_change_note(0, 9, Some(120.0)).expect("nio klipp ska sägas");
        assert!(
            measured.contains("120") && measured.contains('⏱'),
            "talet ska stå tillsammans med åtgärden: {measured}"
        );
        let mixed_measured = tempo_change_note(3, 2, Some(120.0)).expect("delat läge");
        assert!(mixed_measured.contains("120"), "{mixed_measured}");
    }

    /// Ett trimmat klipp behåller sin plats i den sträckta filen.
    #[test]
    fn a_trimmed_clip_keeps_its_place_in_the_stretched_file() {
        // 12 s in i en källa i 120 BPM, projektet i 150: filen är 0,8 gånger så lång,
        // alltså ligger samma ställe 9,6 s in i den.
        assert!((stretched_offset_secs(12.0, 120.0, 150.0) - 9.6).abs() < 1e-4);
        // Och åt andra hållet: filen blir längre, stället flyttas framåt.
        assert!((stretched_offset_secs(12.0, 150.0, 120.0) - 15.0).abs() < 1e-4);
        // Utan känt tempo rörs utsnittet inte — 8.10:s regel.
        assert_eq!(stretched_offset_secs(12.0, 0.0, 150.0), 12.0);
        // Och ett projekt utan tempo är inget att räkna mot.
        assert_eq!(stretched_offset_secs(12.0, 120.0, 0.0), 12.0);
    }

    /// **"Sätt takt 1 här"** (Fas 8.14): ljudet under spelhuvudet blir klippets första
    /// sampel medan klippet står kvar, och högerkanten står still.
    ///
    /// Fallet är Alex' eget: en stämma där musiken börjar 0,30 s in i filen. Klippet
    /// ligger på takt 0 och är exakt filens längd i 140 BPM (259,28 s = 151,24667
    /// takter). Står spelhuvudet på det första slaget ska filen läsas från 0,30 s och
    /// klippet bli 0,30 s kortare — musiken flyttas 0,30 s bakåt på tidslinjen och
    /// slaget hamnar på rutnätet.
    #[test]
    fn setting_beat_one_puts_the_sound_on_the_grid() {
        let tempo = crate::audio::tempo::TempoMap::single(140.0);
        let bars = 151.24667_f32;
        let a = align_clip_start_to_point(0.0, bars, 0.0, 140.0, 0.30, &tempo)
            .expect("0,30 s in i filen ska gå att sätta som första sampel");
        assert!(
            (a.sample_offset_sec - 0.30).abs() < 1e-5,
            "filen ska läsas från 0,30 s: {}",
            a.sample_offset_sec
        );
        assert!(
            (a.moved_source_secs - 0.30).abs() < 1e-5,
            "beskedet ska bära flytten: {}",
            a.moved_source_secs
        );
        let expected = bars as f64 - tempo.bars_for_secs_at(0.0, 0.30);
        assert!(
            (a.length_bars as f64 - expected).abs() < 1e-4,
            "högerkanten ska stå still: längden {expected}, blev {}",
            a.length_bars
        );
    }

    /// Vid en sträckning flyttas **filens** tid, inte tidslinjens.
    ///
    /// 0,30 s på tidslinjen i ett 100-projekt med ett 140-klipp är 0,30 × 100/140 =
    /// 0,2143 s av filen — samma faktor som motorn spelar med. Att använda
    /// tidslinjens tal rakt av vore den inverterade konventionen som kostade tid i 8.10.
    #[test]
    fn setting_beat_one_uses_the_source_timebase() {
        let tempo = crate::audio::tempo::TempoMap::single(100.0);
        let a = align_clip_start_to_point(0.0, 151.24667, 0.0, 140.0, 0.30, &tempo)
            .expect("flytten ska gå");
        let expected = 0.30 * (100.0 / 140.0);
        assert!(
            (a.sample_offset_sec as f64 - expected).abs() < 1e-4,
            "0,30 s ut = {expected} s källa, blev {}",
            a.sample_offset_sec
        );
    }

    /// Klippet står kvar — bara innehållet flyttas. Ett klipp som redan är trimmat
    /// (offset > 0) lägger flytten ovanpå det, och längden räknas i takter ur samma
    /// tempokarta som klippet ligger i.
    #[test]
    fn setting_beat_one_keeps_the_clip_where_it_is() {
        let tempo = crate::audio::tempo::TempoMap::single(120.0);
        let secs_per_bar = tempo.secs_per_bar_at(0.0); // 2,0 s i 120 BPM
        let a = align_clip_start_to_point(4.0, 8.0, 0.25, 120.0, 10.0, &tempo)
            .expect("tio sekunder in i ett klipp som börjar på åtta ska gå");
        // Klippet börjar på 8,0 s; spelhuvudet står 2,0 s in — alltså en takt.
        assert!(
            (a.length_bars - 7.0).abs() < 1e-3,
            "en takt kapas: {}",
            a.length_bars
        );
        assert!(
            (a.sample_offset_sec as f64 - 2.25).abs() < 1e-4,
            "offsetten växer med 2,0 s: {}",
            a.sample_offset_sec
        );
        let _ = secs_per_bar;
    }

    /// Ett hittat slag räknas om till tidslinjen med **samma** faktor som motorn spelar
    /// med, så en sträckt fil hamnar rätt: 0,2143 s in i en källa som spelas i 100 av
    /// ett 140-klipp ligger 0,30 s in på tidslinjen.
    #[test]
    fn a_found_beat_maps_through_the_stretch() {
        let tempo = crate::audio::tempo::TempoMap::single(100.0);
        let p = timeline_point_for_source_secs(0.2143, 0.0, 0.0, 140.0, &tempo)
            .expect("slaget ligger efter klippets början");
        assert!(
            (p - 0.30).abs() < 1e-3,
            "0,2143 s källa ska bli 0,30 s ut i ett 100-projekt: {p}"
        );
        assert!(
            timeline_point_for_source_secs(
                0.5,
                4.0,
                0.9,
                120.0,
                &crate::audio::tempo::TempoMap::single(120.0)
            )
            .is_none(),
            "ett slag före klippets första sampel finns inte att sätta"
        );
    }

    /// **Hela kedjan "hitta första slaget"** — ur ljudet, inte ur en gissning: klicket i
    /// filen hittas av detektorn, räknas om till tidslinjen, och blir klippets första
    /// sampel. Provet binder ihop de tre rena funktionerna, så att kedjan kan prövas utan
    /// fönster, utan motor och utan att en enda sampel spelas.
    #[test]
    fn finding_the_first_beat_aligns_the_clip_on_the_found_onset() {
        let sr = 48_000.0f32;
        let mut pcm = vec![0.0f32; (sr * 2.0) as usize];
        let at = (sr * 0.42) as usize;
        pcm[at] = 0.9;
        pcm[at + 1] = -0.7;
        let onset = crate::audio::onset::music_start_source_secs(
            &pcm,
            sr,
            0.0,
            FIRST_BEAT_SEARCH_SECS,
            &crate::audio::onset::OnsetParams::default(),
        )
        .expect("klicket ska hittas");
        assert!(
            onset <= 0.42 && onset > 0.30,
            "detektorn backar strax före anslaget: {onset}"
        );

        let tempo = crate::audio::tempo::TempoMap::single(140.0);
        let point = timeline_point_for_source_secs(onset, 0.0, 0.0, 140.0, &tempo)
            .expect("slaget ligger efter klippets början");
        let a = align_clip_start_to_point(0.0, 32.0, 0.0, 140.0, point, &tempo)
            .expect("flytten går");
        assert!(
            (a.sample_offset_sec - onset).abs() < 1e-4,
            "offsetten ska bli det hittade slaget: {} mot {onset}",
            a.sample_offset_sec
        );
    }

    /// **Ett nej ska vara ett nej** (8.5): går flytten inte att göra ska ingenting
    /// hända. Här står spelhuvudet före klippets början — då finns inget ljud att
    /// sätta som första sampel, och då hittar vi inte på ett.
    #[test]
    fn setting_beat_one_refuses_when_the_playhead_is_outside() {
        let tempo = crate::audio::tempo::TempoMap::single(120.0);
        assert!(
            align_clip_start_to_point(4.0, 8.0, 0.0, 120.0, 0.0, &tempo).is_none(),
            "spelhuvudet står före klippet"
        );
        assert!(
            align_clip_start_to_point(0.0, 1.0, 0.0, 120.0, 20.0, &tempo).is_none(),
            "ett klipp kortare än ett hundradels slag är inget klipp"
        );
    }

    #[test]
    fn mixer_digest_covers_every_mixed_field() {
        let base_track = PlaylistTrack::new(
            "Testkanal".to_string(),
            "🎹",
            TrackKind::SynthLead,
            Color32::WHITE,
        );
        let bus_volume = [1.0_f32; crate::audio::synth::NUM_BUSES];
        let bus_muted = [false; crate::audio::synth::NUM_BUSES];
        let bus_solo = [false; crate::audio::synth::NUM_BUSES];
        let vca_faders = [1.0_f32; crate::audio::synth::NUM_VCAS];
        let vca_muted = [false; crate::audio::synth::NUM_VCAS];
        let vca_solos = [false; crate::audio::synth::NUM_VCAS];

        let dig = |tracks: &[PlaylistTrack]| {
            mixer_digest(
                tracks,
                &bus_volume,
                &bus_muted,
                &bus_solo,
                &vca_faders,
                &vca_muted,
                &vca_solos,
            )
        };
        let base = dig(&[base_track.clone()]);

        let probe = |name: &str, mutate: &dyn Fn(&mut PlaylistTrack)| {
            let mut t = base_track.clone();
            mutate(&mut t);
            assert_ne!(dig(&[t]), base, "fältet '{name}' saknas i mixer-digesten");
        };

        probe("volume", &|t| t.volume += 0.1);
        probe("pan", &|t| t.pan -= 0.2);
        probe("muted", &|t| t.muted = !t.muted);
        probe("solo", &|t| t.solo = !t.solo);
        probe("comp_threshold_db", &|t| t.comp_threshold_db += 1.0);
        probe("comp_ratio", &|t| t.comp_ratio += 0.5);
        probe("reverb_send", &|t| t.reverb_send += 0.1);
        probe("delay_send", &|t| t.delay_send += 0.1);
        probe("pitch_semitones", &|t| t.pitch_semitones += 1.0);
        probe("bus", &|t| t.bus += 1);
        probe("vca", &|t| t.vca = t.vca.map(|v| v + 1).or(Some(0)));
        probe("sends", &|t| {
            t.sends.push(crate::audio::StemSend { target: crate::audio::SendTarget::bus(1), level: 0.5 })
        });
        // Sends (Fas 8.13): målet och nivån måste ingå, inte bara antalet — annars
        // kunde en send byta buss utan att mixern såg det.
        let with_send = |bus: usize, level: f32| {
            let mut t = base_track.clone();
            t.sends = vec![crate::audio::StemSend { target: crate::audio::SendTarget::bus(bus), level }];
            dig(&[t])
        };
        assert_ne!(with_send(1, 0.5), base, "en send ska synas i mixern");
        assert_ne!(with_send(1, 0.5), with_send(2, 0.5), "målet ska synas");
        assert_ne!(with_send(1, 0.5), with_send(1, 0.9), "nivån ska synas");

        probe("eq.low_gain_db", &|t| t.eq.low_gain_db += 1.0);
        probe("eq.low_freq", &|t| t.eq.low_freq += 10.0);
        probe("eq.mid_gain_db", &|t| t.eq.mid_gain_db += 1.0);
        probe("eq.mid_freq", &|t| t.eq.mid_freq += 10.0);
        probe("eq.high_gain_db", &|t| t.eq.high_gain_db += 1.0);
        probe("eq.high_freq", &|t| t.eq.high_freq += 10.0);

        // Buss- och VCA-nivåerna ligger utanför spåren och måste också fångas,
        // annars går en bussändring att göra utan ångringspunkt.
        let digest_with = |bv: &[f32], bm: &[bool], bs: &[bool], vf: &[f32], vm: &[bool], vs: &[bool]| {
            mixer_digest(&[base_track.clone()], bv, bm, bs, vf, vm, vs)
        };
        let mut bv = bus_volume;
        bv[0] = 0.5;
        assert_ne!(digest_with(&bv, &bus_muted, &bus_solo, &vca_faders, &vca_muted, &vca_solos), base, "bussvolym saknas i mixer-digesten");
        let mut bm = bus_muted;
        bm[0] = true;
        assert_ne!(digest_with(&bus_volume, &bm, &bus_solo, &vca_faders, &vca_muted, &vca_solos), base, "buss-mute saknas i mixer-digesten");
        let mut bs = bus_solo;
        bs[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bs, &vca_faders, &vca_muted, &vca_solos), base, "buss-solo saknas i mixer-digesten");
        let mut vf = vca_faders;
        vf[0] = 0.25;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vf, &vca_muted, &vca_solos), base, "VCA-volym saknas i mixer-digesten");
        let mut vm = vca_muted;
        vm[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vca_faders, &vm, &vca_solos), base, "VCA-mute saknas i mixer-digesten");
        let mut vs = vca_solos;
        vs[0] = true;
        assert_ne!(digest_with(&bus_volume, &bus_muted, &bus_solo, &vca_faders, &vca_muted, &vs), base, "VCA-solo saknas i mixer-digesten");

        // Och det viktigaste: ångringspunkten bär hela ljudbilden. Glöms ett fält
        // i `current_snapshot`/`restore_snapshot` går ändringen att göra men inte
        // att ångra — då fångar jämförelsen här det.
        let snapshot = TimelineUndoSnapshot {
            playlist_tracks: vec![base_track.clone()],
            patterns: Vec::new(),
            bus_volume,
            bus_muted,
            bus_solo,
            // Provet jämför en ljudbilds-fingerprint som inte rymmer busskurvorna; tom lista
            // räcker, och att de inte är med i jämförelsen är ett eget beslut (se digesten).
            bus_automation: Vec::new(),
            vca_faders,
            vca_muted,
            vca_solos,
            selected_timeline_track: 0,
            selected_audio_region: None,
            song_time: 0.0,
            description: "test".to_string(),
        };
        assert_eq!(
            mixer_digest(
                &snapshot.playlist_tracks,
                &snapshot.bus_volume,
                &snapshot.bus_muted,
                &snapshot.bus_solo,
                &snapshot.vca_faders,
                &snapshot.vca_muted,
                &snapshot.vca_solos,
            ),
            base,
            "ångringspunktens ljudbild skiljer sig från den levande state:n"
        );
    }

    #[test]
    fn recovery_offers_newer_autosaves_and_skips_already_saved_work() {
        let paths = isolated_paths("recovery");
        let dir = paths.autosave_dir();
        let now = crate::autosave::now_stamp();

        // (a) Autosave som är nyare än sin manuella fil → erbjuds.
        let fresh = minimal_project_json("Färsk");
        let fresh_file = paths.project_file("Färsk");
        std::fs::create_dir_all(fresh_file.parent().unwrap()).unwrap();
        std::fs::write(&fresh_file, &fresh).expect("manuell fil");
        crate::autosave::save(&dir, "Färsk", fresh.as_bytes(), now + 5).expect("autosave");

        // (b) Autosave som är äldre än sin manuella fil → inget att rädda.
        let saved = minimal_project_json("Redan sparad");
        let saved_file = paths.project_file("Redan sparad");
        std::fs::create_dir_all(saved_file.parent().unwrap()).unwrap();
        std::fs::write(&saved_file, &saved).expect("manuell fil");
        crate::autosave::save(&dir, "Redan sparad", saved.as_bytes(), now.saturating_sub(600))
            .expect("autosave");

        // (c) Autosave utan manuell fil (krasch före första sparningen) → erbjuds.
        let unsaved = minimal_project_json("Aldrig sparad");
        crate::autosave::save(&dir, "Aldrig sparad", unsaved.as_bytes(), now).expect("autosave");

        let found = collect_recovery_candidates_in(&paths);
        let names: Vec<String> = found.iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"Färsk".to_string()), "saknas: {names:?}");
        assert!(
            names.contains(&"Aldrig sparad".to_string()),
            "saknas: {names:?}"
        );
        assert!(
            !names.contains(&"Redan sparad".to_string()),
            "redan sparad ska inte erbjudas: {names:?}"
        );
        // Namnet kommer ur filens JSON, inte ur filnamnets slug.
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "Färsk", "nyaste först");

        // Prenumererad (återställd) autosave försvinner ur listan men finns kvar.
        let restored = found[0].entry.path.clone();
        let retired = crate::autosave::retire(&restored).expect("pensionera");
        assert!(retired.exists() && !restored.exists());
        let after = collect_recovery_candidates_in(&paths);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "Aldrig sparad");

        let _ = std::fs::remove_dir_all(paths.state_dir());
    }

    /// Läslistan ska hålla sig till senaste projekt, tåla en trasig fil och
    /// aldrig peka på något som städats bort utanför appen.
    #[test]
    fn recent_list_keeps_the_newest_projects_and_drops_missing_files() {
        let paths = isolated_paths("recent");
        let mut entries: Vec<RecentProject> = Vec::new();

        // Tolv projekt, bara åtta ska minnas — och det senaste först.
        for i in 0..12 {
            let file = paths.project_file(&format!("Projekt {i}"));
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, minimal_project_json(&format!("Projekt {i}"))).unwrap();
            push_recent_project(&mut entries, &format!("Projekt {i}"), &file.to_string_lossy());
        }
        store_recent_projects_in(&paths, &entries);

        let loaded = load_recent_projects_in(&paths);
        assert_eq!(loaded.len(), RECENT_MAX);
        assert_eq!(loaded[0].name, "Projekt 11", "senaste först");
        assert!(
            !loaded.iter().any(|e| e.name == "Projekt 0"),
            "äldsta ska ha ramlat ur listan"
        );

        // Samma projekt igen: flyttas upp, blir inte dubbelt.
        let last_path = loaded[3].path.clone();
        let last_name = loaded[3].name.clone();
        push_recent_project(&mut entries, &last_name, &last_path);
        assert_eq!(entries.len(), RECENT_MAX, "ingen dubblett");
        assert_eq!(entries[0].path, last_path);

        // En fil som raderats utanför appen ska inte erbjudas.
        std::fs::remove_file(&last_path).unwrap();
        store_recent_projects_in(&paths, &entries);
        let reloaded = load_recent_projects_in(&paths);
        assert!(
            !reloaded.iter().any(|e| e.path == last_path),
            "borttagen fil ska filtreras bort"
        );

        // Trasig JSON får inte krascha starten — bara ge en tom lista.
        std::fs::write(paths.recent_file(), b"{ inte json").unwrap();
        assert!(load_recent_projects_in(&paths).is_empty());

        let _ = std::fs::remove_dir_all(paths.state_dir());
    }

    #[test]
    fn test_classify_all_stem_names_distinct_colors() {
        let stems = [
            ("0 Lead Vocals", "🎙", Color32::from_rgb(0, 225, 245)),
            ("1 Backing Vocals", "🗣", Color32::from_rgb(225, 75, 235)),
            ("2 Drums", "🥁", Color32::from_rgb(255, 140, 25)),
            ("3 Bass", "🎸", Color32::from_rgb(45, 225, 105)),
            ("4 Guitar", "🎸", Color32::from_rgb(255, 195, 30)),
            ("5 Keyboard", "🎹", Color32::from_rgb(65, 155, 255)),
            ("6 Percussion", "🪘", Color32::from_rgb(255, 105, 50)),
            ("7 Strings", "🎻", Color32::from_rgb(40, 220, 185)),
            ("8 Synth", "🎛", Color32::from_rgb(175, 95, 255)),
            ("9 Other", "✨", Color32::from_rgb(255, 80, 150)),
            (crate::i18n::t("🎤 Mic (Voice & Sång)"), "🎤", Color32::WHITE),
        ];

        let mut seen_colors = std::collections::HashSet::new();
        for (name, expected_icon, expected_color) in stems {
            let (_kind, icon, color) = classify_track_style(name);
            assert_eq!(icon, expected_icon, "Icon mismatch for stem: {}", name);
            assert_eq!(color, expected_color, "Color mismatch for stem: {}", name);
            assert!(seen_colors.insert((color.r(), color.g(), color.b())), "Duplicate color found for: {}", name);
        }
    }

    #[test]
    fn test_vagen_hit_project_loads_with_colors_and_mic_track() {
        // Maskinoberoende: läser projektet ur den kanoniska projektmappen om det
        // finns (hoppar över annars).
        let path = crate::paths::paths().project_file("Vägen hit");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let data: SonixProjectData = serde_json::from_str(&content).expect("Valid JSON");
            assert_eq!(data.tracks.len(), 10, "Original file has 10 tracks");

            let mut app_tracks = Vec::new();
            for st in data.tracks {
                let (kind, icon, color) = classify_track_style(&st.name);
                let mut t = PlaylistTrack::new(st.name, icon, kind, color);
                t.regions = st.regions;
                app_tracks.push(t);
            }

            // Simulate SonixApp state
            let mut app_tracks_copy = app_tracks;
            // 1. Check ensure_mic_track_exists logic
            let mic_pos = app_tracks_copy.iter().position(|t| {
                let n = t.name.to_lowercase();
                n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
            });
            assert!(mic_pos.is_none(), "Original file had no mic track");

            let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
            mic_track.volume = 1.0;
            mic_track.is_rec_armed = true;
            app_tracks_copy.push(mic_track);

            assert_eq!(app_tracks_copy.len(), 11, "Now has 11 tracks including Mic");
            assert_eq!(app_tracks_copy.last().unwrap().name, crate::i18n::t("🎤 Mic (Voice & Sång)"));
            assert_eq!(app_tracks_copy.last().unwrap().color, Color32::WHITE);

            // Re-classify colors
            for track in &mut app_tracks_copy {
                let is_mic = track.name.contains("Mic");
                if !is_mic {
                    let (_k, _icon, col) = classify_track_style(&track.name);
                    track.color = col;
                    for r in &mut track.regions {
                        r.color = col;
                    }
                }
            }

            // Verify lead vocals is Cyan
            assert_eq!(app_tracks_copy[0].color, Color32::from_rgb(0, 225, 245));
            assert_eq!(app_tracks_copy[0].regions[0].color, Color32::from_rgb(0, 225, 245));

            // Verify backing vocals is Magenta
            assert_eq!(app_tracks_copy[1].color, Color32::from_rgb(225, 75, 235));
            assert_eq!(app_tracks_copy[1].regions[0].color, Color32::from_rgb(225, 75, 235));

            // Verify drums is Electric Orange
            assert_eq!(app_tracks_copy[2].color, Color32::from_rgb(255, 140, 25));
            assert_eq!(app_tracks_copy[2].regions[0].color, Color32::from_rgb(255, 140, 25));
        }
    }

    #[test]
    fn test_sample_drag_and_drop_to_timeline_track() {
        let mut tracks = vec![
            PlaylistTrack::new(crate::i18n::t("🥁 Trummor").to_string(), "🥁", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            PlaylistTrack::new(crate::i18n::t("🎸 Bas").to_string(), "🎸", TrackKind::Bassline, Color32::from_rgb(45, 225, 105)),
        ];

        let sample_item = LibrarySampleItem {
            id: 42,
            name: "⚡ 808 Kick Impact".to_string(),
            category: "Kick".to_string(),
            icon: "🥁".to_string(),
            default_note: 36,
            color: Color32::from_rgb(255, 80, 50),
            waveform: vec![0.8, 0.6, 0.4, 0.2, 0.05],
            file_path: None,
        };

        // Add to track 0 at bar 4.5
        let region = AudioRegion {
            source_bpm: 0.0, // testklippet har inget känt inspelningstempo
            tape: false,
            id: 101,
            name: sample_item.name.clone(),
            start_bar: 4.5,
            length_bars: 1.0,
            sample_offset_sec: 0.0,
            source_path: sample_item.file_path.clone(),
            waveform_peaks: sample_item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: sample_item.color,
            loop_length_bars: 0.0,
        };
        tracks[0].regions.push(region);

        assert_eq!(tracks[0].regions.len(), 1);
        assert_eq!(tracks[0].regions[0].name, "⚡ 808 Kick Impact");
        assert_eq!(tracks[0].regions[0].start_bar, 4.5);
        assert_eq!(tracks[0].regions[0].waveform_peaks.len(), 5);

        // Add to a brand new track (track index 2)
        let mut new_track = PlaylistTrack::new(
            format!("{} {}", sample_item.icon, sample_item.name),
            "🥁",
            TrackKind::CustomAudio,
            sample_item.color,
        );
        let new_reg = AudioRegion {
            source_bpm: 0.0, // testklippet har inget känt inspelningstempo
            tape: false,
            id: 102,
            name: sample_item.name.clone(),
            start_bar: 8.0,
            length_bars: 1.0,
            sample_offset_sec: 0.0,
            source_path: sample_item.file_path.clone(),
            waveform_peaks: sample_item.waveform.clone(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: sample_item.color,
            loop_length_bars: 0.0,
        };
        new_track.regions.push(new_reg);
        tracks.push(new_track);

        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[2].regions.len(), 1);
        assert_eq!(tracks[2].regions[0].start_bar, 8.0);
    }

    #[test]
    fn test_vocal_studio_sample_take_isolated_properties() {
        let mut vocal_track = crate::audio::recorder::VocalStudioTrack::default();
        vocal_track.takes.clear();
        let dummy_pcm = vec![0.5, -0.5, 0.8, -0.8, 0.3, -0.3];
        let take_idx = vocal_track.load_sample_or_region_as_take("Lead Hook", dummy_pcm, 44100, Color32::from_rgb(0, 200, 240));

        assert_eq!(take_idx, 0);
        assert_eq!(vocal_track.takes.len(), 1);
        assert_eq!(vocal_track.takes[0].name, "Lead Hook");
        assert_eq!(vocal_track.takes[0].gain_linear, 1.0);
        assert_eq!(vocal_track.takes[0].time_stretch, 1.0);
        assert_eq!(vocal_track.takes[0].pitch_semitones, 0.0);
        assert_eq!(vocal_track.takes[0].is_reverse, false);

        // Modify sound shaping parameters
        vocal_track.takes[0].gain_linear = 1.25;
        vocal_track.takes[0].time_stretch = 1.5;
        vocal_track.takes[0].pitch_semitones = 3.0;
        vocal_track.takes[0].is_reverse = true;

        assert_eq!(vocal_track.takes[0].gain_linear, 1.25);
        assert_eq!(vocal_track.takes[0].time_stretch, 1.5);
        assert_eq!(vocal_track.takes[0].pitch_semitones, 3.0);
        assert_eq!(vocal_track.takes[0].is_reverse, true);
    }

    #[test]
    fn test_automation_value_at_interpolates() {
        let lane = AutomationLane {
            param: AutomationParam::Volume,
            enabled: true,
            points: vec![
                AutomationPoint { time_bars: 0.0, value: 0.0 },
                AutomationPoint { time_bars: 10.0, value: 1.0 },
            ],
        };
        assert_eq!(lane.value_at(-1.0), Some(0.0));
        assert_eq!(lane.value_at(0.0), Some(0.0));
        assert_eq!(lane.value_at(5.0), Some(0.5));
        assert_eq!(lane.value_at(10.0), Some(1.0));
        assert_eq!(lane.value_at(20.0), Some(1.0));

        let empty = AutomationLane {
            param: AutomationParam::Pan,
            enabled: true,
            points: Vec::new(),
        };
        assert_eq!(empty.value_at(1.0), None);
    }

    /// **Samma fil är ikon och applogga — och den ska gå att läsa.**
    /// `include_bytes!` gör ett *saknat* asset till ett byggfel, men en trasig eller helsvart
    /// PNG hade gått rakt genom bygget och gett en tom ruta i appen. Provet avkodar den riktiga
    /// filen och kräver rätt mått **och** att det finns något ritat i den.
    #[test]
    fn the_logo_is_a_readable_square_with_a_mark_in_it() {
        let img = sonix_logo().expect("assets/sonix.png ska gå att avkoda");
        assert_eq!(img.size, [512, 512], "ikonen ska vara 512×512 som förut");
        assert!(
            img.pixels.iter().any(|p| p.r() > 8 || p.g() > 8 || p.b() > 8),
            "bilden är helt svart — då syns ingen logga"
        );
    }

    /// **Migrationen, mätt på en fil som den ser ut.** Ett projekt skrivet före Fas 8.2 har
    /// `time_secs`. Läses det utan omräkning hamnar hela kurvan på takt 0 — tyst, för ett
    /// saknat fält är tyst. Provet läser en riktig gammal JSON och kräver att punkterna hamnar
    /// på samma ställe i *musiken* som de lät på.
    #[test]
    fn an_old_project_moves_its_automation_points_through_the_tempo_map() {
        let old = r#"[{"param":"Volume","enabled":true,"points":[
            {"time_secs":8.0,"value":0.5},{"time_secs":16.0,"value":1.0}]}]"#;
        let on_disk: Vec<AutomationLaneOnDisk> = serde_json::from_str(old).unwrap();
        // 120 BPM i 4/4: en takt är två sekunder, så 8 s = takt 4 och 16 s = takt 8.
        let tempo = crate::audio::tempo::TempoMap::single(120.0);
        let live = on_disk[0].to_live(&tempo);
        assert_eq!(live.points.len(), 2, "punkter tappades i migrationen");
        assert!((live.points[0].time_bars - 4.0).abs() < 1e-3, "{:?}", live.points);
        assert!((live.points[1].time_bars - 8.0).abs() < 1e-3, "{:?}", live.points);
        assert!((live.points[0].value - 0.5).abs() < 1e-6);
    }

    /// **Provet som är hela skälet till bytet.** Ett projekt med ett tempobyte: sekunder och
    /// takter är inte längre samma sak, och en punkt som står i sekunder hamnar fel i musiken.
    /// Här: 120 BPM fram till takt 4, sedan 60 (fyra sekunder per takt).
    #[test]
    fn a_tempo_change_separates_seconds_from_bars() {
        let old = r#"[{"param":"Pan","points":[{"time_secs":10.0,"value":0.25}]}]"#;
        let on_disk: Vec<AutomationLaneOnDisk> = serde_json::from_str(old).unwrap();
        let tempo = crate::audio::tempo::TempoMap::from_points(vec![
            crate::audio::tempo::TempoPoint { start_bar: 0, bpm: 120.0 },
            crate::audio::tempo::TempoPoint { start_bar: 4, bpm: 60.0 },
        ]);
        let live = on_disk[0].to_live(&tempo);
        // Takt 0–4 är 2 s styck (8 s), sedan 4 s per takt: 10 s är en halv takt in i takt 4.
        assert!(
            (live.points[0].time_bars - 4.5).abs() < 1e-2,
            "10 s blev takt {} i stället för 4,5",
            live.points[0].time_bars
        );
    }

    /// Nya filer bär `time_bars` och **inte** `time_secs` — annars hade nästa läsning räknat
    /// om en punkt som redan står rätt, och felet hade vuxit för varje sparande.
    #[test]
    fn a_new_project_writes_bars_and_not_seconds() {
        let lane = AutomationLane {
            param: AutomationParam::Volume,
            enabled: true,
            points: vec![
                AutomationPoint { time_bars: 2.5, value: 0.4 },
                AutomationPoint { time_bars: 7.0, value: 0.9 },
            ],
        };
        let json = serde_json::to_string(&lane.to_disk()).unwrap();
        assert!(json.contains("time_bars"), "{json}");
        assert!(!json.contains("time_secs"), "{json}");
        // Och den läses tillbaka exakt.
        let back: AutomationLaneOnDisk = serde_json::from_str(&json).unwrap();
        let live = back.to_live(&crate::audio::tempo::TempoMap::single(120.0));
        assert_eq!(live.points.len(), 2);
        assert!((live.points[0].time_bars - 2.5).abs() < 1e-6);
        assert!((live.points[1].time_bars - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_automation_lane_serde_roundtrip() {
        let lane = AutomationLane {
            param: AutomationParam::ReverbSend,
            enabled: true,
            points: vec![
                AutomationPoint { time_bars: 1.5, value: 0.25 },
                AutomationPoint { time_bars: 4.0, value: 0.9 },
            ],
        };
        let json = serde_json::to_string(&lane).unwrap();
        let back: AutomationLane = serde_json::from_str(&json).unwrap();
        assert_eq!(back.param, AutomationParam::ReverbSend);
        assert!(back.enabled);
        assert_eq!(back.points.len(), 2);
        assert!((back.value_at(2.0).unwrap() - 0.38).abs() < 1e-4);
    }

    #[test]
    fn test_automation_param_ranges_and_index() {
        assert_eq!(AutomationParam::Volume.index(), 0);
        assert_eq!(AutomationParam::Pan.index(), 1);
        assert_eq!(AutomationParam::ReverbSend.index(), 2);
        assert_eq!(AutomationParam::DelaySend.index(), 3);
        assert_eq!(AutomationParam::Volume.range(), (0.0, 1.5));
        assert_eq!(AutomationParam::Pan.range(), (-1.0, 1.0));
        assert_eq!(AutomationParam::DelaySend.range(), (0.0, 1.0));
    }

    #[test]
    fn test_plugin_slots_survive_project_json_roundtrip() {
        let data = SonixProjectData {
            name: "Plugin Test".into(),
            bpm: 120.0,
            follow_tempo: true,
            tempo_points: Vec::new(),
            song_key_root: 3,
            song_key_scale: 0,
            swing: 0.0,
            master_volume: 1.0,
            master_pan: 0.0,
            tracks: Vec::new(),
            plugin_slots: vec![
                None,
                Some(SavedPluginData {
                    path: "/plugins/Gain.clap".into(),
                    name: "Gain".into(),
                    state: vec![0, 1, 2, 250, 255],
                    sandboxed: false,
                    latency_offset_frames: -37,
                    smart_disable: true,
                    // **Utbussarna med i filen** (Fas 8.6): buss 2 → spår 3. Två bussar,
                    // och den första okopplad, för att formen `Option` per buss ska prövas
                    // och inte bara en lista av tal.
                    extra_out_targets: vec![None, Some(2)],
                }),
            ],
            bus_volume: default_bus_volume(),
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_automation: Vec::new(),
            plugin_automation: Vec::new(),
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: default_vca_volume(),
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
            patterns: Vec::new(),
            selected_pattern: 0,
            step_velocities: None,
            channels: Vec::new(),
        };
        let json = serde_json::to_string(&data).unwrap();
        let back: SonixProjectData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plugin_slots.len(), 2);
        assert!(back.plugin_slots[0].is_none());
        let slot = back.plugin_slots[1].as_ref().unwrap();
        assert_eq!(slot.path, "/plugins/Gain.clap");
        assert_eq!(slot.name, "Gain");
        assert_eq!(slot.state, vec![0, 1, 2, 250, 255]);
        // **Offsetet är med i rundturen, och det negativa tecknet överlever** (Fas 8.6).
        assert_eq!(slot.latency_offset_frames, -37);
        assert!(slot.smart_disable, "kryssrutan ska också med i filen");
        assert_eq!(
            slot.extra_out_targets,
            vec![None, Some(2)],
            "utbussarnas mål ska med i filen — och `None` ska förbli `None`, \
             inte bli spår 0"
        );
    }

    /// **En projektfil från före 8.6 läses som noll offset** — alltså exakt den
    /// kompensation som gällde då. Provet läser en **riktig gammal JSON** (utan fältet), inte
    /// en rundtur genom den nya formen: det är den enda formen som visar att
    /// `#[serde(default)]` gör sitt jobb i stället för att ge ett parsefel på ett projekt
    /// Alex redan har.
    #[test]
    fn an_old_project_file_gets_a_zero_latency_offset() {
        let json = r#"{
            "name": "Gammalt",
            "bpm": 120.0,
            "swing": 0.0,
            "master_volume": 1.0,
            "master_pan": 0.0,
            "tracks": [],
            "plugin_slots": [
                {"path": "/plugins/Gain.clap", "name": "Gain", "state": [], "sandboxed": false}
            ]
        }"#;
        let data: SonixProjectData = serde_json::from_str(json).unwrap();
        let slot = data.plugin_slots[0].as_ref().unwrap();
        assert_eq!(slot.name, "Gain");
        assert_eq!(slot.latency_offset_frames, 0, "en gammal fil har inget offset");
        assert!(!slot.smart_disable, "och smart disable var inte på då heller");
        assert!(
            slot.extra_out_targets.is_empty(),
            "och inga utbussar var kopplade då: bussarna lästes inte alls, alltså låter \
             filen exakt som den gjorde"
        );
    }

    /// Beviset för att hålet i 6.3 är stängt: en låt på fyra takter skrivs ut,
    /// läses tillbaka och delas upp per takt — och **varje not finns kvar i rätt
    /// takt**. Före det här var importen en första-takt-import som räknade in
    /// resten och kastade den.
    #[test]
    fn an_exported_song_comes_back_bar_by_bar_without_losing_a_note() {
        use crate::audio::smf::{parse_midi, write_midi, MidiNote, MidiTrack};
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        let bar_ticks = step * STEPS_PER_BAR as u32;

        // Fyra takter: en ton på steg 0 och ett trumslag på steg 8 i varje.
        let mut notes = Vec::new();
        for bar in 0..4u32 {
            notes.push(MidiNote {
                start: bar * bar_ticks,
                length: step / 2,
                channel: 0,
                key: 60 + bar as u8,
                velocity: 100,
            });
            notes.push(MidiNote {
                start: bar * bar_ticks + 8 * step,
                length: step / 2,
                channel: 9,
                key: 36,
                velocity: 96,
            });
        }
        let bytes = write_midi(
            120.0,
            &[MidiTrack {
                name: "Fyra takter".to_string(),
                notes: notes.clone(),
            }],
        );
        let parsed = parse_midi(&bytes).expect("egen fil ska gå att läsa");
        let back: Vec<MidiNote> = parsed
            .notes_with_track()
            .into_iter()
            .map(|(_, n)| n.clone())
            .collect();
        assert_eq!(back.len(), notes.len(), "alla noter ska komma tillbaka ur filen");

        let groups = notes_by_bar(&back, parsed.ppq);
        assert_eq!(groups.len(), 4, "fyra takter ska bli fyra grupper, inte en");
        assert_eq!(
            groups.iter().map(|(_, n)| n.len()).sum::<usize>(),
            notes.len(),
            "ingen not får tappas i uppdelningen"
        );

        for (bar, bar_notes) in &groups {
            let mut pat = test_pattern();
            let rep = apply_bar_to_pattern(bar_notes, parsed.ppq, &mut pat);
            assert_eq!(
                rep.notes_placed + rep.dropped_out_of_range,
                bar_notes.len(),
                "takt {bar} ska redovisas helt"
            );
            assert_eq!(rep.dropped_out_of_range, 0, "takt {bar} ska inte tappa något");
            let row = (60 + *bar as u8 - 48) as usize;
            assert!(
                pat.piano_roll_grid[row][0],
                "tonen i takt {bar} ska ligga på steg 0"
            );
            assert!(pat.channel_steps[0][8], "bastrumman i takt {bar} ska ligga på steg 8");
            assert_eq!(pat.channel_notes[0][8], 36);
        }
    }

    /// Notlängden följer med in (den var ett eget hål i 6.3): en ton som håller
    /// över ett steg tänder alla steg den klingar igenom, medan ett trumslag
    /// bara tänder sitt eget — ett slag är ett slag.
    #[test]
    fn a_held_note_fills_the_steps_it_sounds_but_a_drum_hit_does_not() {
        use crate::audio::smf::MidiNote;
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        let notes = vec![
            // Ton som håller i tre steg från steg 4.
            MidiNote { start: 4 * step, length: 3 * step, channel: 0, key: 64, velocity: 100 },
            // Trumslag med (orimligt) lång not: ska ändå bara tända sitt steg.
            MidiNote { start: 2 * step, length: 8 * step, channel: 9, key: 38, velocity: 100 },
        ];
        let mut pat = test_pattern();
        let rep = apply_bar_to_pattern(&notes, crate::audio::smf::PPQ, &mut pat);
        assert_eq!(rep.notes_placed, 2);
        let row = (64 - 48) as usize;
        for s in 4..7 {
            assert!(pat.piano_roll_grid[row][s], "tonen ska tona i steg {s}");
        }
        assert!(!pat.piano_roll_grid[row][7], "och tystna efter sin längd");
        assert!(pat.channel_steps[1][2], "virveln ska ligga på steg 2");
        assert!(
            !pat.channel_steps[1][3],
            "ett slag får inte bli flera för att noten är lång"
        );
    }

    /// Uppdelningen är ren och ska tåla kanter: inga noter, en not, och noter
    /// långt bortom arrangemanget.
    #[test]
    fn notes_by_bar_handles_the_edges() {
        use crate::audio::smf::MidiNote;
        let step = crate::audio::smf::TICKS_PER_STEP_16TH;
        assert!(notes_by_bar(&[], crate::audio::smf::PPQ).is_empty());

        let notes = vec![
            MidiNote { start: 0, length: step, channel: 0, key: 60, velocity: 100 },
            // Takt 40 — utanför arrangemangets 32 takter.
            MidiNote {
                start: 40 * STEPS_PER_BAR as u32 * step,
                length: step,
                channel: 0,
                key: 62,
                velocity: 100,
            },
        ];
        let groups = notes_by_bar(&notes, crate::audio::smf::PPQ);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, 0);
        assert_eq!(groups[1].0, 40);
        assert!(
            groups[1].0 >= ARRANGEMENT_BARS,
            "takten utanför arrangemanget ska gå att upptäcka och redovisas"
        );
    }

    /// Tempobyten ska överleva en tur genom projektfilen — och en gammal fil utan
    /// fältet ska läsas som ett enda tempo, exakt som den skrevs.
    ///
    /// Det andra är det viktigaste: `#[serde(default)]` är hela skälet till att
    /// fältet kan läggas till utan att röra en enda befintlig projektfil.
    #[test]
    fn tempo_points_survive_the_project_file() {
        let with_points = r#"{"name":"x","bpm":120.0,"swing":0.0,"master_volume":1.0,
            "master_pan":0.0,"tracks":[],
            "tempo_points":[{"start_bar":0,"bpm":120.0},{"start_bar":8,"bpm":90.0}]}"#;
        let data: SonixProjectData =
            serde_json::from_str(with_points).expect("fil med tempobyten ska gå att läsa");
        assert_eq!(data.tempo_points.len(), 2, "båda punkterna ska med");
        assert_eq!(data.tempo_points[1].start_bar, 8);

        // Kartan som byggs ur dem svarar rätt på båda sidor om bytet.
        let map = crate::audio::tempo::TempoMap::from_points(data.tempo_points.clone());
        assert!((map.bpm_at(4.0) - 120.0).abs() < 0.01, "före bytet");
        assert!((map.bpm_at(9.0) - 90.0).abs() < 0.01, "efter bytet");

        // En gammal projektfil har inte fältet alls.
        let old = r#"{"name":"x","bpm":128.0,"swing":0.0,"master_volume":1.0,
            "master_pan":0.0,"tracks":[]}"#;
        let data: SonixProjectData =
            serde_json::from_str(old).expect("gammal projektfil ska fortfarande gå att läsa");
        assert!(
            data.tempo_points.is_empty(),
            "en gammal fil har inga byten — då gäller bpm som förut"
        );
        assert!((data.bpm - 128.0).abs() < 0.01, "och tempot ska vara kvar");
    }

        /// Importens urvalsregel: en mp3 vars stämma redan finns som wav hoppas över,
    /// en mp3 utan wav-syskon tas med. (Alex' förslag om en dialog bygger på samma
    /// regel — dialogen blir en fråga om just detta val.)
    #[test]
    fn a_stem_is_imported_once_and_the_wav_wins() {
        use std::path::PathBuf;
        let f = |n: &str| PathBuf::from(format!("/stems/{n}"));
        let files = vec![
            f("Rock (FX).wav"),
            f("Rock (FX).mp3"),
            f("Rock (Bass).wav"),
            f("Rock (Bass).mp3"),
            f("Endast mp3 (Synth).mp3"),
        ];
        let (kept, skipped) = stem_files_for_import(files, false);
        assert_eq!(skipped, 2, "båda mp3:erna hade wav-syskon");
        assert_eq!(kept.len(), 3, "två wav + den ensamma mp3:n");
        assert!(
            kept.iter()
                .any(|p| p.to_string_lossy().contains("Endast mp3")),
            "en mp3 utan wav-syskon ÄR stämman och ska med"
        );
        assert!(
            !kept.iter()
                .any(|p| p.to_string_lossy().ends_with("(FX).mp3")),
            "men den som har ett wav-syskon ska bort"
        );
    }

    /// Skiftläge och "behåll allt" ska inte kunna överraska.
    #[test]
    fn the_import_rule_handles_case_and_the_keep_everything_choice() {
        use std::path::PathBuf;
        let f = |n: &str| PathBuf::from(format!("/stems/{n}"));
        let (kept, skipped) = stem_files_for_import(vec![f("A.WAV"), f("A.MP3")], false);
        assert_eq!((kept.len(), skipped), (1, 1), "versaler ska inte lura regeln");

        let all = vec![f("A.wav"), f("A.mp3"), f("B.mp3")];
        let (kept, skipped) = stem_files_for_import(all.clone(), true);
        assert_eq!(
            (kept.len(), skipped),
            (3, 0),
            "keep_mp3 = allt med, inget tyst bortfall"
        );
        assert_eq!(kept, all);
    }

#[test]
    fn a_frozen_track_plays_its_audio_and_not_its_patterns() {
        let mut track =
            PlaylistTrack::new("Trummor".to_string(), "🥁", TrackKind::Drums, Color32::BLACK);
        track.clips[0] = Some(1);
        track.clips[4] = Some(1);
        assert_eq!(
            render_clips_for(&track)[0],
            Some(1),
            "ett ofruset spår ska trigga sina klipp"
        );

        track.frozen = Some(FrozenTrack {
            path: "/tmp/trummor-0.wav".to_string(),
            digest: 7,
            stamp: 1_700_000_000,
        });
        assert!(
            render_clips_for(&track).iter().all(|c| c.is_none()),
            "ett fruset spår ska vara tyst i pattern-vägen — annars hörs det två gånger"
        );
    }

    /// **Snäppet räknas i takter, inte i sekunder** (Fas 8.2 steg 3). Ett sextondelssteg är
    /// en plats i takten: 1,03 takter snäpper till 1,0 — inte till "1,0 takter omräknat från
    /// sekunder", som blir ett annat tal så snart tempot inte är konstant.
    #[test]
    fn the_snap_counts_in_bars() {
        assert_eq!(snap_bar(TimeSnapMode::Snap16th, 1.03), 1.0);
        assert_eq!(snap_bar(TimeSnapMode::Snap16th, 0.97), 1.0);
        assert_eq!(snap_bar(TimeSnapMode::Snap16th, 0.03125), 0.0625);
        assert_eq!(snap_bar(TimeSnapMode::SnapBeat, 2.4), 2.5);
        assert_eq!(snap_bar(TimeSnapMode::SnapBar, 7.4), 7.0);
        assert_eq!(snap_bar(TimeSnapMode::SnapBar, 7.6), 8.0);
        // Hundradelarna rörs inte: sekunden är enheten, och avrundningen sker av anroparen
        // som har kartan.
        assert_eq!(snap_bar(TimeSnapMode::FreeHundredth, 1.037), 1.037);
        // Ett negativt larvigt värde får inte bli NaN eller panik.
        assert_eq!(snap_bar(TimeSnapMode::SnapBar, -0.4), 0.0);
    }

    #[test]
    fn a_frozen_track_is_left_out_of_a_pattern_render() {
        let mut frozen =
            PlaylistTrack::new("Trummor".to_string(), "🥁", TrackKind::Drums, Color32::BLACK);
        frozen.frozen = Some(FrozenTrack {
            path: "/tmp/x.wav".to_string(),
            digest: 1,
            stamp: 2,
        });
        frozen.frozen_pcm = Some((std::sync::Arc::new(vec![0.0; 8]), std::sync::Arc::new(vec![0.0; 8]), 44_100));

        // Sång-läget: spåret hörs som ljud, och det ska med.
        assert!(frozen_audio_in_render(&frozen, false));
        // Pattern-läget renderar kanalracket — spårets frysta ljud ska inte med.
        assert!(
            !frozen_audio_in_render(&frozen, true),
            "ett fruset spår får inte hamna i en pattern-export"
        );
        // Ett ofruset spår påverkas inte av läget (det har inget fryst ljud).
        let plain = PlaylistTrack::new("Bas".to_string(), "🎸", TrackKind::Bassline, Color32::BLACK);
        assert!(frozen_audio_in_render(&plain, true));
        assert!(frozen_audio_in_render(&plain, false));
    }

    #[test]
    fn only_pattern_tracks_can_be_frozen() {
        for kind in [TrackKind::Drums, TrackKind::SynthLead, TrackKind::Bassline] {
            let track = PlaylistTrack::new("Spår".to_string(), "🎛", kind, Color32::BLACK);
            assert!(track.can_freeze(), "{kind:?} spelas av patterns och kan frysas");
        }
        // Ett ljudspår ligger redan som färdigt ljud — det finns inget att tjäna.
        let audio = PlaylistTrack::new("Sång".to_string(), "🎤", TrackKind::VocalAudio, Color32::BLACK);
        assert!(!audio.can_freeze());
        assert!(!audio.is_frozen());
    }

    #[test]
    fn the_frozen_region_covers_the_whole_file() {
        // 1,5 sekunder vid 44100 Hz.
        let left = vec![0.25f32; 66_150];
        let region = frozen_region(&left, 44_100);
        assert!(
            (region.length_secs - 1.5).abs() < 1e-3,
            "längden ska räknas ur bufferten, fick {}",
            region.length_secs
        );
        assert_eq!(region.start_time_secs, 0.0);
        assert_eq!(region.gain, 1.0);
        assert!(!region.muted);
        assert!((frozen_region(&left, 0).length_secs - 66_150.0).abs() < 1.0, "noll samplingsfrekvens får inte ge oändlig längd");
    }

    #[test]
    fn a_frozen_track_survives_the_project_file() {
        let saved = SavedFrozenTrack {
            path: "/home/x/Music/Sonix/Projects/Beat/Frozen/trummor-0.wav".to_string(),
            digest: 42,
            stamp: 1_700_000_000,
        };
        let json = serde_json::to_string(&saved).unwrap();
        let back: SavedFrozenTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(back, saved);
    }

    #[test]
    fn an_old_project_file_without_a_frozen_field_loads_as_unfrozen() {
        // Exakt den form en fil hade innan fältet fanns: allt annat har defaults.
        let json = r#"{
            "name": "Trummor",
            "volume": 0.8,
            "pan": 0.0,
            "muted": false,
            "solo": false,
            "clips": [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,
                      null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],
            "regions": []
        }"#;
        let track: SavedTrackData = serde_json::from_str(json).expect("äldre fil ska läsas");
        assert!(track.frozen.is_none(), "utan fältet är spåret ofrusat");
        assert_eq!(track.name, "Trummor");
    }

    /// Fas 8.3: en gammal projektfil har inga sidokedjefält — den ska läsas som
    /// ett spår utan sidokedja, med samma standardtröskel som ett nytt spår får.
    #[test]
    fn an_old_project_file_without_a_sidechain_loads_with_none() {
        let json = r#"{
            "name": "Bas",
            "volume": 0.8,
            "pan": 0.0,
            "muted": false,
            "solo": false,
            "clips": [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,
                      null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],
            "regions": []
        }"#;
        let track: SavedTrackData = serde_json::from_str(json).expect("äldre fil ska läsas");
        assert!(track.sidechain_from.is_none(), "utan fältet finns ingen sidokedja");
        assert_eq!(track.sidechain_amount_db, 0.0);
        assert_eq!(track.sidechain_threshold_db, -30.0);
    }

    /// Och en fil **med** fältet ska få tillbaka exakt samma sidokedja: källspåret,
    /// duckningen och tröskeln, både läst och skrivet.
    #[test]
    fn a_sidechain_survives_the_project_file() {
        let json = r#"{
            "name": "Bas",
            "volume": 0.8,
            "pan": 0.0,
            "muted": false,
            "solo": false,
            "clips": [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,
                      null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],
            "regions": [],
            "sidechain_from": 2,
            "sidechain_amount_db": 9.5,
            "sidechain_threshold_db": -24.0
        }"#;
        let track: SavedTrackData = serde_json::from_str(json).expect("filen ska läsas");
        assert_eq!(track.sidechain_from, Some(2));
        assert_eq!(track.sidechain_amount_db, 9.5);
        assert_eq!(track.sidechain_threshold_db, -24.0);

        // Rundgång: det som skrivs ska vara det som läses tillbaka.
        let written = serde_json::to_value(&track).expect("projektfilen ska kunna skrivas");
        assert_eq!(written["sidechain_from"], serde_json::json!(2));
        assert_eq!(written["sidechain_amount_db"], serde_json::json!(9.5));
        assert_eq!(written["sidechain_threshold_db"], serde_json::json!(-24.0));
        let back: SavedTrackData = serde_json::from_value(written).expect("rundgången ska hålla");
        assert_eq!(back.sidechain_from, Some(2));
        assert_eq!(back.sidechain_amount_db, 9.5);
    }

    #[test]
    fn test_old_projects_without_plugin_slots_still_load() {
        let legacy = r#"{
            "name": "Legacy",
            "bpm": 100.0,
            "swing": 0.0,
            "master_volume": 1.0,
            "master_pan": 0.0,
            "tracks": []
        }"#;
        let data: SonixProjectData = serde_json::from_str(legacy).unwrap();
        assert!(data.plugin_slots.is_empty());
    }

    /// Namnregeln för stämfilerna (Fas 8.5): källfilen först, låtens titel som
    /// reserv — och aldrig ett tomt namn. En tom bas hade gett filer som heter
    /// `-vocals.wav`, alltså ett namn som inte går att skilja mellan två låtar.
    #[test]
    fn stem_files_are_named_after_the_source_and_never_after_nothing() {
        assert_eq!(
            stem_base_name(Some("/home/alex/Music/Min Låt.mp3"), "Titel"),
            "Min Låt"
        );
        assert_eq!(
            stem_base_name(Some("/home/alex/Music/../Sång.wav"), "Titel"),
            "Sång"
        );
        assert_eq!(stem_base_name(None, "  Min Låt  "), "Min Låt");
        assert_eq!(stem_base_name(Some("   "), "Titel"), "Titel");
        assert_eq!(stem_base_name(None, "   "), "stems");
        assert_eq!(stem_base_name(Some(""), ""), "stems");
    }

    /// Fas 8.5 i kanalracket: en sample vars fil **inte** går att läsa får inte
    /// flytta in sitt namn, sin färg och sin vågform i kanalen. Förut gjorde den
    /// det — kanalen såg ut att ha ett eget sample och bar bibliotekets grova
    /// vågform medan `pcm_audio` var tom, och stegen spelade den inbyggda synten.
    /// Testet mäter båda sidorna: en läsbar fil tas, en oläsbar rör ingenting.
    #[test]
    fn a_channel_keeps_its_sound_when_the_sample_cannot_be_read() {
        let root = std::env::temp_dir().join(format!(
            "sonix_channel_assign_{}_{}",
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("tmpdir");
        let wav = root.join("kick.wav");
        let samples: Vec<f32> = (0..4410)
            .flat_map(|i| {
                let v = (i as f32 * 0.05).sin() * 0.8;
                [v, v]
            })
            .collect();
        crate::audio::write_export_with(
            &wav.to_string_lossy(),
            crate::audio::ExportFormat::Wav16,
            &samples,
            44_100,
            &crate::audio::ExportMeta::default(),
            crate::audio::DitherSettings::default(),
        )
        .expect("testwav");

        let item = |name: &str, path: Option<String>| LibrarySampleItem {
            id: 1,
            name: name.to_string(),
            category: "Kicks".to_string(),
            icon: "🥁".to_string(),
            default_note: 36,
            color: Color32::from_rgb(255, 80, 50),
            waveform: vec![0.9, 0.6, 0.3],
            file_path: path,
        };

        // 1. Filen finns och går att läsa: samplen tas, ljudet följer med.
        let mut ch = test_channel();
        assert!(
            assign_library_sample_to_channel(
                &mut ch,
                &item("Kick 808", Some(wav.to_string_lossy().into_owned()))
            ),
            "en läsbar fil ska tas"
        );
        assert_eq!(ch.name, "Kick 808");
        assert!(ch.pcm_audio.is_some(), "ljudet ska ha följt med samplen");
        assert_eq!(ch.waveform_preview, vec![0.9, 0.6, 0.3]);

        // 2. Filen går inte att läsa (mp3 eller borta): kanalen ska vara orörd.
        let mut ch = test_channel();
        let before = (
            ch.name.clone(),
            ch.icon.clone(),
            ch.sample_path.clone(),
            ch.steps,
            ch.pcm_audio.is_some(),
            ch.waveform_preview.clone(),
        );
        assert!(
            !assign_library_sample_to_channel(
                &mut ch,
                &item(
                    "Sång (mp3)",
                    Some(root.join("finns-inte.mp3").to_string_lossy().into_owned())
                )
            ),
            "en oläsbar fil ska inte tas"
        );
        assert_eq!(
            (
                ch.name.clone(),
                ch.icon.clone(),
                ch.sample_path.clone(),
                ch.steps,
                ch.pcm_audio.is_some(),
                ch.waveform_preview.clone(),
            ),
            before,
            "kanalen ska vara orörd när filen inte går att läsa"
        );

        // 3. Sample utan fil: appens egen röst, ingen fil att läsa.
        let mut ch = test_channel();
        assert!(assign_library_sample_to_channel(&mut ch, &item("Synth Kick", None)));
        assert_eq!(ch.name, "Synth Kick");
        assert!(ch.pcm_audio.is_none());
        assert!(ch.sample_path.is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Samma familj, i projektinläsningen: en kanal vars sample inte går att läsa
    /// får inte bära en vågform som ser ut som ljud. Vågformen räknas ur ljudet —
    /// finns inget ljud blir den tom, och då syns det att kanalen är tyst. Det är
    /// den enda ärliga utvägen här: en projektinläsning kan inte avbrytas för att
    /// en samplefil saknas, men den får inte låtsas att ljudet finns.
    #[test]
    fn a_channel_with_an_unreadable_sample_carries_no_waveform() {
        let mut saved = channel_to_saved(&test_channel());
        saved.sample_path = Some("/tmp/sonix-finns-inte-alls.mp3".to_string());
        let ch = saved_to_channel(&saved);
        assert!(ch.pcm_audio.is_none());
        assert!(ch.waveform_preview.is_empty(), "ingen vågform utan ljud");
        assert_eq!(ch.sample_path.as_deref(), Some("/tmp/sonix-finns-inte-alls.mp3"));
        assert_eq!(ch.name, "Virvel", "inställningarna ska ändå med");
        assert_eq!(ch.steps, [true; 16]);
    }

    /// Frågan om mp3:erna (Fas 8.5) behöver en siffra, och siffran ska räknas med
    /// **samma** regel som importen använder — annars kan dialogen säga "3" och
    /// importen hoppa över 2.
    #[test]
    fn the_mp3_question_counts_with_the_same_rule_as_the_import() {
        let names = |v: &[&str]| -> Vec<String> { v.iter().map(|s| s.to_string()).collect() };

        let set = names(&[
            "vocals.wav",
            "vocals.mp3",
            "drums.WAV",
            "drums.MP3", // skiftläge ska inte spela roll
            "bass.mp3",  // ingen wav med samma namn — räknas inte
            "other.wav",
        ]);
        assert_eq!(duplicate_mp3_count(&set), 2);
        // Samma svar som importen ger: den hoppar över precis de två.
        let paths: Vec<std::path::PathBuf> = set.iter().map(std::path::PathBuf::from).collect();
        assert_eq!(stem_files_for_import(paths, false).1, 2);

        // ZIP-listan bär mappnamn; bara filnamnet avgör.
        let zip = names(&["Stems/vocals.mp3", "Stems/vocals.wav", "Stems/bass.mp3"]);
        assert_eq!(duplicate_mp3_count(&zip), 1);

        assert_eq!(duplicate_mp3_count(&[]), 0);
        assert_eq!(duplicate_mp3_count(&names(&["a.mp3", "b.mp3"])), 0);
    }

    /// Separatören: wav-syskonet hittas när det finns, och frågan ställs aldrig
    /// för en fil som redan är wav eller för en mp3 utan syskon.
    #[test]
    fn the_separator_finds_a_wav_sibling_and_never_asks_for_a_wav() {
        let dir = std::env::temp_dir().join(format!(
            "sonix_sibling_{}_{}",
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        for name in ["Sång 1.mp3", "Sång 1.wav", "Annan.mp3", "anteckningar.txt"] {
            std::fs::write(dir.join(name), b"x").expect("tmpfil");
        }

        let mp3 = dir.join("Sång 1.mp3").to_string_lossy().into_owned();
        let wav = dir.join("Sång 1.wav").to_string_lossy().into_owned();
        assert_eq!(wav_sibling(&mp3).as_deref(), Some(wav.as_str()));
        assert_eq!(wav_sibling(&wav), None, "en wav har ingen fråga");
        assert_eq!(
            wav_sibling(&dir.join("Annan.mp3").to_string_lossy()),
            None,
            "utan syskon finns inget att välja"
        );
        assert_eq!(wav_sibling("/finns/inte/alls.mp3"), None);

        // Listningen tar ljudfilerna och lämnar anteckningen.
        assert_eq!(list_stem_audio_files(&dir.to_string_lossy()).len(), 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Den enda delen av frågan som inte går att pröva på namn: att `unzip -Z1`
    /// verkligen lämnar arkivets namn (och bara dem). Ignorerad som standard
    /// eftersom den kräver `zip` på maskinen — samma hållning som den externa
    /// dither-mätningen, som kräver ffprobe.
    ///
    /// Körs med: `cargo test --bin sonix -- --ignored zip_question`
    #[test]
    #[ignore]
    fn zip_question_sees_the_duplicates_in_a_real_archive() {
        let dir = std::env::temp_dir().join(format!(
            "sonix_zip_question_{}_{}",
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Stems")).expect("tmpdir");
        for name in ["Stems/vocals.wav", "Stems/vocals.mp3", "Stems/bass.mp3", "Stems/drums.wav"] {
            std::fs::write(dir.join(name), b"x").expect("tmpfil");
        }
        let zip_path = dir.join("Stems (120BPM).zip");
        let made = std::process::Command::new("zip")
            .args(["-q", "-r", &zip_path.to_string_lossy(), "Stems"])
            .current_dir(&dir)
            .output()
            .expect("zip");
        assert!(made.status.success(), "zip misslyckades");

        // Arkivet innehåller en mp3 med wav-syskon (vocals) och en utan (bass).
        let names = zip_entry_names(&zip_path.to_string_lossy());
        assert!(names.iter().any(|n| n.ends_with("vocals.wav")), "namnen: {names:?}");
        assert_eq!(duplicate_mp3_count(&names), 1);

        // Ett arkiv som inte finns ger en tom lista — ingen fråga, ingen panik.
        assert!(zip_entry_names("/finns/inte/alls.zip").is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fas 8.2 steg 3, hela vägen: tempobyten skrivs **ut** i filen och läses
    /// **in** igen som projektets tempopunkter. Rundgången är beviset — inte att
    /// funktionen finns. (Skrivaren och läsaren är varandras motparter: tick =
    /// takt × PPQ × 4 ut, takt = tick ÷ (PPQ × 4) in.)
    #[test]
    fn midi_tempo_changes_survive_the_round_trip() {
        use crate::audio::smf::{self, MidiNote, MidiTrack};
        let bytes = smf::write_midi_with_tempo(
            &[(0.0, 128.0), (4.0, 90.0)],
            &[MidiTrack {
                name: "Takt".to_string(),
                notes: vec![MidiNote {
                    start: 0,
                    length: 120,
                    channel: 9,
                    key: 36,
                    velocity: 100,
                }],
            }],
        );
        let parsed = smf::parse_midi(&bytes).expect("egen fil ska gå att läsa");
        assert_eq!(
            parsed.tempo_events.len(),
            2,
            "båda tempobytena ska stå i filen: {:?}",
            parsed.tempo_events
        );

        let imported = crate::audio::tempo::tempo_points_for_import(&parsed.tempo_events, 120.0)
            .expect("kartan ska tas in i ett projekt som står på 120");
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].start_bar, 0);
        assert!((imported[0].bpm - 128.0).abs() < 0.5, "{:?}", imported[0]);
        assert_eq!(imported[1].start_bar, 4);
        assert!((imported[1].bpm - 90.0).abs() < 0.5, "{:?}", imported[1]);

        // Och ett projekt som redan har ett eget tempo behåller sitt.
        assert!(
            crate::audio::tempo::tempo_points_for_import(&parsed.tempo_events, 100.0).is_none()
        );
    }
    /// Ett projekt som sparades innan tonarten fanns läses exakt som förut: Eb och Dur
    /// (Fas 8.11) — och tonarten betyder samma sak i varje meny, för det finns bara en.
    #[test]
    fn an_old_project_without_a_key_reads_as_eb_major() {
        let data: SonixProjectData =
            serde_json::from_str(&minimal_project_json("Gammalt")).expect("filen ska gå att läsa");
        assert_eq!(data.song_key_root, 3, "Eb");
        assert_eq!(data.song_key_scale, 0, "Dur");
        assert_eq!(crate::audio::scale::scale_at(data.song_key_scale).name, "Dur");
        assert!(
            crate::audio::scale::in_scale(3, data.song_key_root, data.song_key_scale),
            "Eb ska finnas i Eb-dur"
        );
    }

    /// Gamla projekt: inspelningstemot går att MÄTA fram ur filen — men bara när
    /// måttet stämmer med projektets tempo (Fas 8.10).
    #[test]
    fn an_old_project_gets_its_recording_tempo_measured_when_the_file_agrees() {
        let region = |bars: f32, source_bpm: f32| AudioRegion {
            id: 1,
            name: "Stämma".to_string(),
            start_bar: 0.0,
            length_bars: bars,
            sample_offset_sec: 0.0,
            source_path: Some("/tmp/stam.wav".to_string()),
            waveform_peaks: Vec::new(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: Color32::WHITE,
            loop_length_bars: 0.0,
            source_bpm,
            tape: false,
        };

        // 120 takter ur 240 sekunder = 120 BPM, och projektet står i 120: klippet
        // importerades i projektets tempo, alltså vet vi det.
        assert_eq!(source_bpm_from_region(&region(120.0, 0.0), 240.0, 120.0), 120.0);

        // Trimmad kloss (60 takter av samma fil): måttet blir 60 BPM och stämmer
        // inte — då gissar vi inte.
        assert_eq!(source_bpm_from_region(&region(60.0, 0.0), 240.0, 120.0), 0.0);

        // Tempot har ändrats sedan importen (projektet står i 100, filen säger
        // 120): samma sak — okänt, ljudet rörs inte.
        assert_eq!(source_bpm_from_region(&region(120.0, 0.0), 240.0, 100.0), 0.0);

        // Redan känt: räknas inte om.
        assert_eq!(source_bpm_from_region(&region(120.0, 96.5), 240.0, 120.0), 96.5);

        // Utan ljud (ingen längd) finns inget att mäta.
        assert_eq!(source_bpm_from_region(&region(120.0, 0.0), 0.0, 120.0), 0.0);
        // Utan längd att mäta mot behålls det sparade värdet — det är bättre än 0,0.
        assert_eq!(source_bpm_from_region(&region(120.0, 118.0), 0.0, 120.0), 118.0);
    }

    /// **Felet Alex hörde 2026-09-12, som ett test.**
    ///
    /// Hans stämmor är 254,000 s och klippen 127 takter, alltså är innehållet
    /// 127 × 240 / 254 = **120,0000** BPM. Klippen bar 120,98828 (en siffra ur en
    /// tempoanalys). Sträckningen räknar `filsekunder × källa/projekt`, klossen är
    /// `takter × 240/projekt` — och med 120,98828 blev filen **2,3 sekunder för
    /// lång** vid 110 BPM. Det är inte en artefakt i en ton: det är hela låten som
    /// glider ur takt, mer ju längre den spelar.
    #[test]
    fn a_tempo_that_does_not_fill_the_clip_is_replaced_by_the_geometry() {
        let (file_secs, bars, stored) = (254.0_f32, 127.0_f32, 120.98828_f32);
        let geometry = bars * 240.0 / file_secs;
        assert!(
            (geometry - 120.0).abs() < 1e-4,
            "stämmorna är exakt 127 takter i 120,0000 BPM: {geometry}"
        );

        let region = |bars: f32, source_bpm: f32| AudioRegion {
            id: 1,
            name: "Stämma".to_string(),
            start_bar: 0.0,
            length_bars: bars,
            sample_offset_sec: 0.0,
            source_path: Some("/tmp/stam.wav".to_string()),
            waveform_peaks: Vec::new(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: Color32::WHITE,
            loop_length_bars: 0.0,
            source_bpm,
            tape: false,
        };
        let source = source_bpm_from_region(&region(bars, stored), file_secs, 120.0);
        assert!(
            (source - 120.0).abs() < 1e-4,
            "geometrin ska vinna över analystalet: {source}"
        );

        // Filen och klossen möts nu vid varje projekt-tempo — det är hela poängen.
        for project in [120.0_f32, 110.0, 102.0, 133.0] {
            let clip_secs = bars * 240.0 / project;
            let rendered = file_secs * (source / project);
            assert!(
                (clip_secs - rendered).abs() < 0.02,
                "vid {project} BPM: filen {rendered:.2} s mot klossen {clip_secs:.2} s"
            );
        }

        // Och med den gamla siffran kvar är felet sekunder, inte millisekunder.
        let wrong = file_secs * (stored / 110.0);
        let right = bars * 240.0 / 110.0;
        assert!(
            wrong - right > 2.0,
            "felet ska vara hörbart stort: {:.2} s",
            wrong - right
        );
    }

    /// Stämpeln: klipp utan känt tempo får projektets, och vid det tempot ändras
    /// ingenting — faktorn är 1,0 (Fas 8.10).
    #[test]
    fn stamping_the_unknown_tempo_changes_nothing_until_the_tempo_moves() {
        let mut regions = vec![
            AudioRegion {
                id: 1,
                name: "A".to_string(),
                start_bar: 0.0,
                length_bars: 4.0,
                sample_offset_sec: 0.0,
                source_path: None,
                waveform_peaks: Vec::new(),
                volume: 1.0,
                fade_in_bars: 0.0,
                fade_out_bars: 0.0,
                muted: false,
                is_reverse: false,
                color: Color32::WHITE,
                loop_length_bars: 0.0,
                source_bpm: 0.0,
                tape: false,
            },
            AudioRegion {
                id: 2,
                name: "B".to_string(),
                start_bar: 0.0,
                length_bars: 4.0,
                sample_offset_sec: 0.0,
                source_path: None,
                waveform_peaks: Vec::new(),
                volume: 1.0,
                fade_in_bars: 0.0,
                fade_out_bars: 0.0,
                muted: false,
                is_reverse: false,
                color: Color32::WHITE,
                loop_length_bars: 0.0,
                source_bpm: 96.0,
                tape: false,
            },
        ];
        // Utan fil att mäta mot: projektets tempo, som förut (faktorn 1,0 direkt).
        let (stamped, used) = stamp_source_tempo(regions.iter_mut(), None, 120.0);
        assert_eq!(stamped, 1, "bara klippet utan tempo ska stämplas");
        assert_eq!(used, 120.0, "och tempot som sattes ska gå att säga");
        assert_eq!(regions[0].source_bpm, 120.0);
        assert_eq!(regions[1].source_bpm, 96.0, "ett känt tempo rörs inte");
        // Och vid det tempot är faktorn 1,0 — ingenting hörs förrän man rör tempot.
        assert_eq!(stretch_ratio_for(regions[0].source_bpm, 120.0), 1.0);
        assert_eq!(stretch_ratio_for(regions[0].source_bpm, 140.0), 140.0 / 120.0);
    }

    /// **Felet bakom "tempot påverkar inte" och "låten skar av i slutet".**
    ///
    /// Alex' "Rock and Hard Place": Suno gav inget BPM (varken i arkivnamnet eller i
    /// taggarna), så alla nio klipp stod på 0 och rördes inte av tempot. Filerna är
    /// 259,28 s och klippen 129,64 takter — alltså byggdes de i **120,000** BPM, vilket
    /// är det tal sträckningen ska mätas mot. Vid 200 BPM räckte klippet bara 155,57 s
    /// och **103,71 s musik skars av**; vid 100 BPM blev klippet 311,14 s och filen tog
    /// slut i förtid. Båda felen är samma sak: ett klipp utan känt tempo varken sträcks
    /// eller fyller sin kloss.
    #[test]
    fn the_clip_measure_gives_the_tempo_suno_did_not() {
        // Hans egna tal: 129,64 takter, 259,28 s fil.
        let bpm = geometry_source_bpm(129.64, 259.28).expect("måttet ska gå att räkna");
        assert!((bpm - 120.0).abs() < 0.01, "nio stämmor ger 120,000: {bpm}");

        // Klippet räcker filen exakt i det tempot — och skär av i alla andra.
        let clip_secs = |bpm: f32| 129.64 * 240.0 / bpm;
        assert!((clip_secs(120.0) - 259.28).abs() < 0.01, "hela filen vid 120");
        assert!((clip_secs(200.0) - 155.57).abs() < 0.01, "klippet tar slut vid 155,57 s");
        assert!((259.28 - clip_secs(200.0) - 103.71).abs() < 0.01, "103,71 s skars av");

        // Med måttet satt sträcks filen till klossen i stället: 259,28 × 120/200 = 155,57.
        let factor = stretch_ratio_for(120.0, 200.0);
        assert!((259.28 / factor - 155.57).abs() < 0.01, "filen fyller klossen");

        // Skräp ger inget svar i stället för ett orimligt.
        assert_eq!(geometry_source_bpm(0.0, 259.28), None);
        assert_eq!(geometry_source_bpm(129.64, 0.0), None);
        assert_eq!(geometry_source_bpm(4.0, 600.0), None, "1,6 BPM är inget tempo");
    }

    /// Ett klipp som börjar en bit in i filen beskriver ett utsnitt, inte filen.
    #[test]
    fn a_clip_that_starts_inside_the_file_keeps_the_project_tempo() {
        // 129,64 takter ur en 259,28 s fil = 120 BPM — men bara om klippet är hela filen.
        assert_eq!(stamped_tempo(129.64, 0.0, Some(259.28), 200.0), 120.0);
        // Börjar det 30 s in är takterna ett utsnitt: projektets tempo gäller.
        assert_eq!(stamped_tempo(129.64, 30.0, Some(259.28), 200.0), 200.0);
        // Utan fil att mäta mot: projektets tempo.
        assert_eq!(stamped_tempo(129.64, 0.0, None, 200.0), 200.0);
    }

    /// Klippet följer projektets tempo — men bara när inspelningstemot är känt
    /// (Fas 8.10).
    #[test]
    fn the_stretch_ratio_follows_the_known_tempo_and_leaves_the_unknown_alone() {
        // Inspelad i 120, spelad i 240: dubbelt så fort.
        assert!((stretch_ratio_for(120.0, 240.0) - 2.0).abs() < 1e-5);
        // Samma tempo: ingen omsampling alls (bit-exakt väg).
        assert!((stretch_ratio_for(120.0, 120.0) - 1.0).abs() < 1e-6);
        // Inspelad i 180, spelad i 90: halva farten.
        assert!((stretch_ratio_for(180.0, 90.0) - 0.5).abs() < 1e-5);

        // Okänt inspelningstempo: rör inte ljudet. Det här är vägen för varje
        // projekt sparat före fältet, och för allt som kommer från biblioteket.
        assert_eq!(stretch_ratio_for(0.0, 240.0), 1.0);
        assert_eq!(stretch_ratio_for(120.0, 0.0), 1.0);

        // Och gränserna: ett orimligt fält i ett projekt ska inte göra klippet
        // oanvändbart.
        assert_eq!(stretch_ratio_for(1.0, 280.0), MAX_STRETCH_RATIO);
        assert_eq!(stretch_ratio_for(280.0, 40.0), MIN_STRETCH_RATIO);
    }

    /// Utsnittet en region täcker: mer ljud än tid när klippet spelades in
    /// långsammare än projektet (Fas 8.10).
    #[test]
    fn a_slower_source_clip_covers_more_audio_than_its_length() {
        // 2 sekunder på tidslinjen i 48 kHz, inspelad i halva tempot: 4 sekunder ljud.
        assert_eq!(region_source_span_samples(2.0, 2.0, 48_000), 192_000);
        // I samma tempo: precis sin egen längd.
        assert_eq!(region_source_span_samples(2.0, 1.0, 48_000), 96_000);
        // Aldrig noll — en region ska alltid gå att rita.
        assert_eq!(region_source_span_samples(0.0, 1.0, 48_000), 1);
    }

    /// Stegklockan räknar hela steg och ligger aldrig före ljudet (Fas 8.13b).
    #[test]
    fn the_step_clock_counts_whole_steps_and_never_runs_ahead() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + std::time::Duration::from_millis(ms);
        let step = std::time::Duration::from_micros(107_143); // sextondel i 140 BPM

        // Inget steg är inne än: noll.
        assert_eq!(steps_elapsed(t0, step, at(0)), 0);
        assert_eq!(steps_elapsed(t0, step, at(107)), 0);
        // Första hela steget, och sedan ett per steg — inte avrundat uppåt.
        assert_eq!(steps_elapsed(t0, step, at(108)), 1);
        assert_eq!(steps_elapsed(t0, step, at(214)), 1);
        assert_eq!(steps_elapsed(t0, step, at(215)), 2);
        // En bildruta som stått still länge tar med alla steg den hann förbi.
        assert_eq!(steps_elapsed(t0, step, at(1_000)), 9);
        // Ett steg utan längd kan inte ge något.
        assert_eq!(steps_elapsed(t0, std::time::Duration::ZERO, at(1_000)), 0);
    }

    /// Den gamla stegklockan sackade efter ljudet — felet växte med tiden.
    ///
    /// **Mätt i Alex' projekt 2026-09-13:** spelhuvudet låg 0,66 s efter ljudet
    /// 12,64 s in i Rock and Hard Place (Rock and a Hard Place (Vocals): tystnad
    /// till 5,0 s, första frasen 7,155 s, nästa fras 13,3 s — och spelhuvudet stod på
    /// 12,64 s när den frasen hördes). Den gamla regeln var
    /// `last_step_time = Instant::now()`: nästa deadline sattes en bildruta för sent,
    /// varje gång. Här simuleras en bildruta på 11,2 ms (≈89 Hz, det som ger just
    /// 0,66 s) steg för steg, och båda reglerna får samma bildrutor att arbeta med.
    #[test]
    fn the_old_step_clock_lagged_the_sound_and_the_error_grew() {
        let frame = std::time::Duration::from_micros(11_200);
        let step = std::time::Duration::from_micros(107_143); // sextondel i 140 BPM
        let song_secs = 12.64_f64; // där Alex stod
        let t0 = Instant::now();

        let (mut old_lag, mut new_lag) = (0.0_f64, 0.0_f64);
        for rule in ["gammal", "ny"] {
            let (mut anchor, mut song, mut frames) = (t0, 0.0_f64, 0u64);
            loop {
                let now = t0 + frame * frames as u32;
                if now.duration_since(t0).as_secs_f64() > song_secs {
                    break;
                }
                if rule == "gammal" {
                    if now.duration_since(anchor) >= step {
                        anchor = now; // ← ankaret till NU: felet summeras
                        song += step.as_secs_f64();
                    }
                } else {
                    let due = steps_elapsed(anchor, step, now); // ← hela steg från deadline
                    if due > 0 {
                        anchor += step * due as u32;
                        song += step.as_secs_f64() * due as f64;
                    }
                }
                frames += 1;
            }
            let lag = song_secs - song;
            if rule == "gammal" {
                old_lag = lag;
            } else {
                new_lag = lag;
            }
        }

        assert!(
            old_lag > 0.5,
            "den gamla klockan skulle sackat mer än en halv sekund: {old_lag:.3} s"
        );
        assert!(
            new_lag <= step.as_secs_f64(),
            "den nya klockan ska hålla sig inom ett steg: {new_lag:.3} s"
        );
    }

    /// Ett enstaka anslag får inte försvinna i översikten (Fas 8.3).
    ///
    /// Den gamla vägen läste **var 64:e sample** inom varje punkt, så ett anslag
    /// som ligger mellan två läsningar syntes inte alls: översikten visade
    /// tystnad där det fanns ljud. Cache-vägen lägger varje sample i ett fack och
    /// kan inte missa något, hur kort anslaget än är.
    #[test]
    fn a_single_sample_transient_is_visible_in_the_overview() {
        let length = 48_000usize;
        // Positioner valda så att de ligger mitt emellan två läsningar i den
        // gamla vägen (64-samplars steg), plus första och sista samplet.
        for position in [0usize, 1, 63, 64, 65, 12_345, 47_999] {
            let mut samples = vec![0.0f32; length];
            samples[position] = 0.9;
            let arc = std::sync::Arc::new(samples);
            let cache = crate::audio::waveform::WaveformCache::build(&arc);
            let peaks = overview_peaks_from(&cache, &arc, 480);
            assert_eq!(peaks.len(), 480, "en punkt per fack");
            assert!(
                peaks.iter().any(|p| *p > 0.5),
                "anslaget på sample {position} syntes inte i översikten"
            );
        }
    }

    /// MÄTNING: hur många anslag tappade den gamla översikten?
    ///
    /// Reproducerar den borttagna raka samplingen (`var 64:e sample` inom varje
    /// punkt) och räknar hur många enstaka anslag som föll mellan två läsningar,
    /// mot cache-vägens noll.
    ///
    /// Samma **steg** som en riktig stämma ger: en fyra minuter lång fil i 48 kHz
    /// blir, vid 120 BPM, 1920 punkter (`file_bars * 16`) — alltså 6000 samplar
    /// per punkt och ett steg på 6000/64 = **93**. Här är filen kortare men
    /// förhållandet detsamma (600 000 / 100 = 6000), så det är samma läckage som
    /// mäts — utan att testet behöver bygga en 46 MB-buffert 200 gånger.
    ///
    /// Kör med:
    ///   cargo test --locked --bin sonix the_old_overview -- --ignored --nocapture
    #[test]
    #[ignore = "mätning, inte en grind — körs manuellt"]
    fn the_old_overview_lost_the_shortest_transients() {
        let length = 600_000usize;
        let points = 100usize;
        let chunk_size = (length / points).max(1); // 6000, som för en stämma
        let mut samples = vec![0.0f32; length];
        let mut old_missed = 0usize;
        let mut new_missed = 0usize;
        let positions = 200usize;
        for i in 0..positions {
            let at = i * (length / positions);
            samples[at] = 0.9;

            let mut old_peaks = Vec::with_capacity(points);
            for p_i in 0..points {
                let s_start = p_i * chunk_size;
                let s_end = (s_start + chunk_size).min(length);
                let mut peak = 0.04f32;
                let step = ((s_end - s_start) / 64).max(1);
                let mut s = s_start;
                while s < s_end {
                    let v = samples[s].abs();
                    if v > peak {
                        peak = v;
                    }
                    s += step;
                }
                old_peaks.push(peak.clamp(0.04, 0.98));
            }

            let arc = std::sync::Arc::new(samples.clone());
            let cache = crate::audio::waveform::WaveformCache::build(&arc);
            let new_peaks = overview_peaks_from(&cache, &arc, points);

            if !old_peaks.iter().any(|p| *p > 0.5) {
                old_missed += 1;
            }
            if !new_peaks.iter().any(|p| *p > 0.5) {
                new_missed += 1;
            }
            samples[at] = 0.0;
        }
        eprintln!(
            "  av {positions} enstaka anslag: gamla översikten missade {old_missed}, cache-vägen missade {new_missed}"
        );
        assert_eq!(new_missed, 0, "cache-vägen får inte tappa ett anslag");
        assert!(
            old_missed > 0,
            "mätningen ska visa den gamla vägens förlust — annars mäter den inget"
        );
    }

    /// Cachen nycklas på **bufferten**, inte på Arc-handtaget.
    ///
    /// Motorn får ett eget handtag till samma buffert vid import
    /// (`LoadStemTrack`), och spårets cache måste kännas igen som giltig för den.
    #[test]
    fn the_waveform_key_follows_the_buffer_not_the_handle() {
        let left = std::sync::Arc::new(vec![0.1f32; 1000]);
        let engine_side = left.clone();
        assert_eq!(
            waveform_key(&left),
            waveform_key(&engine_side),
            "samma buffert genom två handtag är samma ljud"
        );
        let other = std::sync::Arc::new(vec![0.1f32; 1001]);
        assert_ne!(
            waveform_key(&left),
            waveform_key(&other),
            "olika buffertar får inte få samma nyckel"
        );
    }

    /// En importerad stämma får sin PCM och sin cache — och ingen cache när det
    /// inte finns något ljud att rita (Fas 8.3 + 8.5).
    #[test]
    fn an_imported_stem_gets_its_pcm_and_a_cache_that_belongs_to_it() {
        let left = std::sync::Arc::new(vec![0.25f32; 480]);
        let right = std::sync::Arc::new(vec![-0.25f32; 480]);
        let cache = crate::audio::waveform::WaveformCache::build(&left);

        let (pcm, keyed) = imported_track_waveform(
            Some((left.clone(), right.clone(), 48_000)),
            Some(cache.clone()),
        );
        let (l, r, sr) = pcm.expect("PCM ska med till spåret");
        assert_eq!(sr, 48_000);
        assert_eq!(r.len(), 480);
        // Samma buffert som motorn fick (`LoadStemTrack`), inte en kopia:
        // annars vore nyckeln en annan och cachen oanvändbar.
        assert_eq!(
            std::sync::Arc::as_ptr(&l),
            std::sync::Arc::as_ptr(&left),
            "spåret ska hålla motorns buffert"
        );
        let (key, cache_in_track) = keyed.expect("cachen ska följa med");
        assert_eq!(key, waveform_key(&l));
        assert_eq!(
            cache_in_track.envelope_at(&l, 0, l.len(), 40),
            cache.envelope_at(&left, 0, left.len(), 40),
            "cachen på spåret ska svara för samma ljud"
        );

        // Utan PCM: ingen cache alls.
        let (pcm, keyed) = imported_track_waveform(None, Some(cache));
        assert!(pcm.is_none());
        assert!(keyed.is_none(), "en cache utan ljud får inte följa med");
    }

    /// Importen ger tidslinjen en cache för **samma** samplar som motorn får
    /// (Fas 8.3).
    ///
    /// Det är skillnaden mot förut: spåret fick ingen PCM, så ritningen föll
    /// tillbaka på den grova översikten och blev kvar där. Nu byggs cachen i
    /// avkodningstråden och nycklas på bufferten motorn fick — `ensure_waveform_cache`
    /// känner igen den och ritar det exakta höljet redan i första bildrutan.
    ///
    /// Kör hela importvägen (avkodningstråden) mot en riktig wav-fil på disk, i
    /// en sandlåda under `/tmp` — Alex' egna stämmor rörs inte.
    #[test]
    fn the_import_hands_the_timeline_a_cache_for_the_samples_the_engine_gets() {
        let root = std::env::temp_dir().join(format!(
            "sonix_import_cache_{}_{}",
            std::process::id(),
            crate::autosave::now_stamp()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("sandlådan ska gå att skapa");

        let sr = 48_000u32;
        let frames = 4_800usize;
        let mut left = vec![0.0f32; frames];
        left[4_320] = 0.9; // ett anslag på 90 % av filen
        let right = left.clone();
        let wav = root.join("01_Drums.wav");
        crate::audio::exporter::write_stem_wav(&wav.to_string_lossy(), &left, &right, sr)
            .expect("stämman ska gå att skriva");

        let progress = std::sync::Arc::new(std::sync::Mutex::new(StemImportProgress::default()));
        SonixApp::background_decode_stems(
            &root.to_string_lossy(),
            "Cachetest",
            120.0,
            false,
            progress.clone(),
        );

        let res = {
            let p = progress.lock().expect("låset");
            p.completed_payload.clone().expect("importen ska bli klar")
        };
        assert_eq!(res.tracks.len(), 1, "en wav in, ett spår ut");
        let dt = &res.tracks[0];

        let pcm_left = dt.pcm_left.clone().expect("wav-filen ska avkodas");
        assert_eq!(pcm_left.len(), frames, "hela ljudet ska med");
        let cache = dt
            .waveform_cache
            .clone()
            .expect("cachen ska byggas i avkodningstråden");

        // Spåret nycklar cachen på den buffert motorn får: samma handtag ur
        // spårvyns perspektiv, alltså samma nyckel — ingen omslagning i onödan.
        let engine_side = pcm_left.clone();
        assert_eq!(waveform_key(&pcm_left), waveform_key(&engine_side));

        // Djupt inzoomad är höljet ur cachen det **exakta** höljet för samma
        // samplar (färre samplar per bildpunkt än finaste facket → ur samplen).
        let pixels = 200usize; // 4800/200 = 24 samplar per bildpunkt < 64
        assert_eq!(
            cache.envelope_at(&pcm_left, 0, pcm_left.len(), pixels),
            crate::audio::waveform::envelope_per_pixel(&pcm_left, pixels),
            "ur cachen ska samma svar komma som ur samplen"
        );

        // Och översikten som följer med regionen bär anslaget.
        assert!(!dt.wave_env.is_empty(), "regionen ska ha en översikt");
        assert!(
            dt.wave_env.iter().any(|p| *p > 0.5),
            "anslaget ska synas i översikten: {:?}",
            dt.wave_env
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Ett klipp att peka på i proven nedan: bara det geometrin bryr sig om.
    fn region_at(start_bar: f32, length_bars: f32) -> AudioRegion {
        AudioRegion {
            id: 0,
            name: String::new(),
            start_bar,
            length_bars,
            sample_offset_sec: 0.0,
            source_path: None,
            waveform_peaks: Vec::new(),
            volume: 1.0,
            fade_in_bars: 0.0,
            fade_out_bars: 0.0,
            muted: false,
            is_reverse: false,
            color: super::default_region_color(),
            loop_length_bars: 0.0,
            source_bpm: 0.0,
            tape: false,
        }
    }

    /// **Högerklick väljer klippet under pekaren** (Fas 8.15) — samma regel som
    /// dragstarten frågar, alltså kan de två dörrarna inte välja olika klipp. Det var
    /// fällan: menyn gäller det *valda* klippet, och högerklick valde inte, så
    /// "Radera region" kunde träffa ett annat klipp än det man pekade på.
    #[test]
    fn the_region_under_the_pointer_is_the_one_the_menu_acts_on() {
        let lane_min_x = 100.0;
        let bar_w = 40.0;
        let regions = vec![region_at(0.0, 2.0), region_at(4.0, 2.0)];

        // Rakt på klippet.
        assert_eq!(region_under_x(&regions, 140.0, lane_min_x, bar_w), Some(0));
        assert_eq!(region_under_x(&regions, 300.0, lane_min_x, bar_w), Some(1));

        // Åtta pixlars marginal utanför kanterna — handtagens marginal, så att ett
        // klipp som är några pixlar brett fortfarande går att peka på.
        assert_eq!(region_under_x(&regions, 96.0, lane_min_x, bar_w), Some(0));
        assert_eq!(region_under_x(&regions, 186.0, lane_min_x, bar_w), Some(0));

        // Glappet mellan klippen är ingens: då väljs ingenting (valet lämnas orört).
        assert_eq!(region_under_x(&regions, 220.0, lane_min_x, bar_w), None);
        assert_eq!(region_under_x(&regions, 80.0, lane_min_x, bar_w), None);

        // Tom lane, ingen bredd (får inte bli NaN) och en punkt utan tal.
        assert_eq!(region_under_x(&[], 140.0, lane_min_x, bar_w), None);
        assert_eq!(region_under_x(&regions, 140.0, lane_min_x, 0.0), None);
        assert_eq!(region_under_x(&regions, f32::NAN, lane_min_x, bar_w), None);
    }

    /// Klippgeometri för grupproven: bara det skiftet bryr sig om.
    fn geometry(start_bar: f32, length_bars: f32, sample_offset_sec: f32, source_bpm: f32) -> ClipGeometry {
        ClipGeometry { start_bar, length_bars, sample_offset_sec, source_bpm }
    }

    /// **Stämgruppen flyttar tillsammans** (Fas 8.15b): ett och samma skift i *tid* för
    /// varje klipp som börjar på ankarets takt — och ingenting för de andra.
    ///
    /// Fallet är Alex' eget: nio stämmor på takt 0, där mätningen på trummorna sa att
    /// musiken börjar 0,6008 s in i filen. Flyttades bara trumklippet hamnade det 0,60 s
    /// före bas och gitarr — som ligger låsta mot varandra på 0 ms.
    #[test]
    fn the_stem_group_shifts_by_the_same_time() {
        let tempo = crate::audio::tempo::TempoMap::single(140.0);
        let bars = 151.24667_f32;
        let clips = vec![
            (0, 0, geometry(0.0, bars, 0.0, 140.0)), // trummorna — ankaret
            (1, 0, geometry(0.0, bars, 0.0, 140.0)), // basen
            (2, 0, geometry(8.0, 4.0, 0.0, 140.0)),  // ett klipp på en annan takt
        ];
        let plan = plan_group_shift_by_head_secs(&clips, 0.0, 0.6008125, &tempo)
            .expect("hela gruppen ska gå att flytta");
        assert_eq!(plan.len(), 2, "bara de två som börjar på takt 0");
        assert!(plan.iter().all(|(t, r, _)| (*t, *r) == (0, 0) || (*t, *r) == (1, 0)));
        let expected_len = bars as f64 - tempo.bars_for_secs_at(0.0, 0.6008125);
        for (t, _, a) in &plan {
            assert!(
                (a.sample_offset_sec as f64 - 0.6008125).abs() < 1e-5,
                "spår {t}: filen ska läsas 0,6008 s längre in: {}",
                a.sample_offset_sec
            );
            assert!(
                (a.length_bars as f64 - expected_len).abs() < 1e-3,
                "spår {t}: högerkanten ska stå still: {}",
                a.length_bars
            );
        }
        assert!(
            plan.iter().all(|(t, _, _)| *t != 2),
            "klippet på takt 8 ska stå orört"
        );
    }

    /// Vid en sträckning är skiftet fortfarande samma **tid** på tidslinjen, men olika
    /// många sekunder av filen — samma faktor som motorn spelar med.
    #[test]
    fn a_stretched_stem_shifts_in_its_own_timebase() {
        let tempo = crate::audio::tempo::TempoMap::single(100.0);
        let clips = vec![
            (0, 0, geometry(0.0, 151.24667, 0.0, 100.0)), // klippet i projektets tempo
            (1, 0, geometry(0.0, 151.24667, 0.0, 140.0)), // ett 140-klipp i ett 100-projekt
        ];
        let plan =
            plan_group_shift_by_head_secs(&clips, 0.0, 0.30, &tempo).expect("flytten ska gå");
        assert_eq!(plan.len(), 2, "båda börjar på takt 0");
        assert!(
            (plan[0].2.sample_offset_sec as f64 - 0.30).abs() < 1e-4,
            "klippet i projektets tempo läser filen 0,30 s längre in: {}",
            plan[0].2.sample_offset_sec
        );
        // Faktorn är tempo/källa (samma som motorn spelar med, och samma som
        // `setting_beat_one_uses_the_source_timebase` pinnar): en 140-fil i ett
        // 100-projekt spelas långsammare, så 0,30 s tidslinje är 0,30 × 100/140 s fil.
        let stretched = 0.30 * (100.0 / 140.0);
        assert!(
            (plan[1].2.sample_offset_sec as f64 - stretched).abs() < 1e-3,
            "0,30 s på tidslinjen är {stretched} s av en 140-fil som spelas i 100: {}",
            plan[1].2.sample_offset_sec
        );
    }

    /// **Ett klipp som inte kan flyttas stoppar allt** (Fas 8.15b): en halvflyttad grupp är
    /// en sång ur fas, och det är värre än ingen flytt alls. `Err` namnger klippet, och
    /// anroparen skriver ingenting.
    #[test]
    fn a_clip_that_cannot_move_stops_the_whole_group() {
        let tempo = crate::audio::tempo::TempoMap::single(140.0);
        let clips = vec![
            (0, 0, geometry(0.0, 151.24667, 0.0, 140.0)),
            (7, 2, geometry(0.0, 0.02, 0.0, 140.0)), // 0,02 takter: skiftet tömmer den
        ];
        let err = plan_group_shift_by_head_secs(&clips, 0.0, 0.6008125, &tempo)
            .expect_err("en 0,02 takter lång stämma kan inte kapas 0,35 takter");
        assert_eq!(err, (7, 2), "klippet som stoppade ska namnges");
    }

    /// Ett skift som inte är positivt flyttar ingenting — det finns ingen början att sätta.
    #[test]
    fn nothing_moves_when_the_head_is_not_positive() {
        let tempo = crate::audio::tempo::TempoMap::single(140.0);
        let clips = vec![(0, 0, geometry(0.0, 151.24667, 0.0, 140.0))];
        assert!(plan_group_shift_by_head_secs(&clips, 0.0, 0.0, &tempo)
            .expect("noll ska gå")
            .is_empty());
        assert!(plan_group_shift_by_head_secs(&clips, 0.0, -0.5, &tempo)
            .expect("negativt ska gå")
            .is_empty());
    }

    /// **En tabell, ett index.** `ALL`, `index()` och cachen är tre listor för samma sak — den
    /// här kodbasens mest återkommande fel är att två av dem driver isär. Provet räknar dem mot
    /// varandra i stället för att lita på att de skrivs i takt: lägger någon till en variant och
    /// glömmer `ALL`, eller skriver ett index för hand som pekar fel, fälls det här.
    #[test]
    fn all_index_and_the_cache_agree() {
        assert_eq!(AutomationParam::ALL.len(), AutomationParam::COUNT);
        let mut sedda = std::collections::HashSet::new();
        for (plats, p) in AutomationParam::ALL.iter().enumerate() {
            assert_eq!(
                p.index(),
                plats,
                "{p:?} säger index {} men ligger på plats {plats} i ALL",
                p.index()
            );
            assert!(sedda.insert(p.index()), "{p:?} delar index med en annan variant");
            let (lo, hi) = p.range();
            assert!(lo < hi, "{p:?} har ett omöjligt område {lo}–{hi}");
            assert!(!p.label().is_empty(), "{p:?} saknar etikett");
        }
        // Cachen måste rymma alla mål — det var längden som annars hade blivit för kort.
        assert_eq!(AutomationParam::COUNT, sedda.len());
    }

    /// De nya målen i 8.8 är **samma fält som `SetStemMixParams` redan bär**. Provet binder
    /// listan till den sanningen: skulle ett mål läggas till utan att kommandot har fältet, är
    /// det en rad som ser automatiserad ut men inte gör något.
    #[test]
    fn the_new_goals_are_parameters_the_mix_command_already_carries() {
        for p in [
            AutomationParam::CompThreshold,
            AutomationParam::CompRatio,
            AutomationParam::Pitch,
            AutomationParam::EqLow,
            AutomationParam::EqMid,
            AutomationParam::EqHigh,
        ] {
            assert!(AutomationParam::ALL.contains(&p), "{p:?} saknas i ALL");
            let (lo, hi) = p.range();
            // Tröskeln är i dB och ska kunna nå ett verkligt ingrepp; transponeringen ska rymma
            // minst en oktav åt båda hållen — annars vore målet formellt med men oanvändbart.
            assert!(lo <= 0.0 && hi >= 0.0, "{p:?}: området {lo}–{hi} når inte noll");
        }
        assert_eq!(AutomationParam::CompThreshold.range(), (-60.0, 0.0));
        assert_eq!(AutomationParam::Pitch.range(), (-24.0, 24.0));
    }

    fn pt(bar: f32, value: f32) -> AutomationPoint {
        AutomationPoint { time_bars: bar, value }
    }

    /// **Regeln är en regel.** Spårets lane, bussens lane och plugin-kurvan ska ge *samma* svar
    /// för samma punkter — annars vore de tre kurvor som ser likadana ut och beter sig olika,
    /// och en fix i en av dem hade lämnat de andra fel.
    #[test]
    fn a_bus_lane_a_track_lane_and_a_plugin_lane_follow_the_same_curve() {
        let points = vec![pt(4.0, 0.2), pt(8.0, 1.0)];
        let spår = AutomationLane {
            param: AutomationParam::Volume,
            enabled: true,
            points: points.clone(),
        };
        let buss = BusAutomationLane { bus: 1, enabled: true, points: points.clone() };
        let plugin = PluginAutomationLane {
            track: 1,
            param_id: 3,
            param_name: "Cutoff".into(),
            enabled: true,
            points,
            last_sent: None,
        };
        for bar in [0.0, 4.0, 5.0, 6.0, 8.0, 12.0] {
            assert_eq!(spår.value_at(bar), buss.level_at(bar), "oense vid takt {bar}");
            assert_eq!(
                spår.value_at(bar),
                plugin.value_at(bar),
                "plugin-kurvan och spårkurvan är oense vid takt {bar}"
            );
        }
        // Och interpolationen är linjär mitt emellan.
        assert!((buss.level_at(6.0).unwrap() - 0.6).abs() < 1e-4);
        assert!((plugin.value_at(6.0).unwrap() - 0.6).abs() < 1e-4);
        // Konstant före första och efter sista punkten: en kurva ska inte börja på noll.
        assert_eq!(buss.level_at(0.0), Some(0.2));
        assert_eq!(plugin.value_at(99.0), Some(1.0));
    }

    /// En tom kurva ger inget värde — den får inte betyda "noll".
    #[test]
    fn an_empty_bus_lane_says_nothing_rather_than_zero() {
        let tom = BusAutomationLane { bus: 0, enabled: true, points: Vec::new() };
        assert_eq!(tom.level_at(3.0), None);
        assert_eq!(lane_value_at(&[], 3.0), None);
        let tom_plugin = PluginAutomationLane {
            track: 0,
            param_id: 1,
            param_name: String::new(),
            enabled: true,
            points: Vec::new(),
            last_sent: None,
        };
        assert_eq!(tom_plugin.value_at(3.0), None);
    }

    /// **`last_sent` hör till körningen, inte till filen.** Skrevs cachen ner skulle en öppnad
    /// fil påstå att ett värde redan skickats — och första värdet efter ett tempobyte hade
    /// uteblivit, tyst, eftersom jämförelsen då hade ett tal att jämföra mot.
    #[test]
    fn a_plugin_lane_is_written_without_the_last_sent_cache() {
        let lane = PluginAutomationLane {
            track: 1,
            param_id: 7,
            param_name: "Cutoff".into(),
            enabled: true,
            points: vec![pt(2.0, 0.5)],
            last_sent: Some(1234.5),
        };
        let json = serde_json::to_string(&lane).unwrap();
        assert!(!json.contains("last_sent"), "cachen ska inte med i filen: {json}");
        let back: PluginAutomationLane = serde_json::from_str(&json).unwrap();
        assert_eq!(back.last_sent, None, "en inläst kurva har inte skickat något än");
        assert_eq!(back.param_id, 7);
        assert_eq!(back.param_name, "Cutoff");
        assert_eq!(back.points.len(), 1);
        assert!((back.points[0].time_bars - 2.0).abs() < 1e-6);
    }

    /// **Filformen, läst som den ser ut.** En handskriven lane utan `enabled` ska vara **på**
    /// (en kurva som tystnar för att ett fält saknas är den värsta sortens standardvärde), och
    /// målet ska komma tillbaka med spår, id och namn intakt.
    #[test]
    fn a_plugin_lane_in_a_project_file_keeps_its_target_and_curve() {
        let mut value: serde_json::Value =
            serde_json::from_str(&minimal_project_json("Med kurva")).unwrap();
        value["plugin_automation"] = serde_json::json!([{
            "track": 2,
            "param_id": 41,
            "param_name": "Cutoff",
            "points": [{"time_bars": 8.0, "value": 0.25}]
        }]);
        let data: SonixProjectData = serde_json::from_value(value).unwrap();
        assert_eq!(data.plugin_automation.len(), 1);
        let lane = &data.plugin_automation[0];
        assert_eq!(lane.track, 2);
        assert_eq!(lane.param_id, 41);
        assert_eq!(lane.param_name, "Cutoff");
        assert!(lane.enabled, "en lane i en fil utan `enabled` ska vara på");
        assert_eq!(lane.points.len(), 1);
        assert!((lane.points[0].time_bars - 8.0).abs() < 1e-6);
    }

    /// **Fältet saknas i filer före 8.8** — då ska det läsas som en tom lista, och ett nytt
    /// sparande ska skriva det. Utan det andra halvan hade en kurva kunnat skapas i det tysta
    /// och aldrig kommit med i filen.
    #[test]
    fn an_old_project_file_without_plugin_automation_reads_as_an_empty_list() {
        let data: SonixProjectData =
            serde_json::from_str(&minimal_project_json("Gammalt")).unwrap();
        assert!(data.plugin_automation.is_empty());
        let ny = serde_json::to_string(&data).unwrap();
        assert!(
            ny.contains("\"plugin_automation\""),
            "fältet ska skrivas även när listan är tom: {ny}"
        );
    }

    /// **En plugin-parameter mäter inte i 0..1.** Området kommer från parametern, och ett värde
    /// utanför det kläms — en kurva ritad i fel skala ska inte kunna skicka ett tal pluginen
    /// inte har. Ett bakvänt område (hi < lo) från en slarvig plugin kläms också rätt.
    #[test]
    fn a_plugin_parameter_value_is_clamped_to_the_parameters_own_range() {
        // En nivå i dB: området är inte 0..1, och 40 är över taket.
        assert!((PluginAutomationLane::clamp_to_range(40.0, (-60.0, 12.0)) - 12.0).abs() < 1e-9);
        assert!((PluginAutomationLane::clamp_to_range(-99.0, (-60.0, 12.0)) + 60.0).abs() < 1e-9);
        // Mitt emellan rörs värdet inte.
        assert!((PluginAutomationLane::clamp_to_range(0.5, (0.0, 1.0)) - 0.5).abs() < 1e-9);
        // Bakvänt område: samma svar, inte ett panikvärde.
        assert!((PluginAutomationLane::clamp_to_range(0.5, (1.0, 0.0)) - 0.5).abs() < 1e-9);
        assert!((PluginAutomationLane::clamp_to_range(5.0, (1.0, 0.0)) - 1.0).abs() < 1e-9);
    }
    use crate::audio::tempo::{set_tempo_point, TempoMap, TempoPoint};

    /// **Ett enda tempo ger exakt ett stycke, med dagens faktor.** För projekt utan tempobyten
    /// ska ingenting ändras — den här vägen är då bit-identisk med den gamla.
    #[test]
    fn a_single_tempo_gives_exactly_one_piece_with_the_old_factor() {
        let tempo = TempoMap::single(100.0);
        let pieces = stretch_pieces(0.0, 4.0, 0.0, 140.0, &tempo);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].ratio, stretch_ratio_for(140.0, 100.0) as f64);
        assert_eq!(pieces[0].source_offset_secs, 0.0);
        // Fyra takter i 100 BPM är 9,6 ut-sekunder.
        assert!((pieces[0].out_secs - 9.6).abs() < 1e-6);
    }

    /// **Fysiken, inte en kommentar.** Fyra takter i 120 BPM är 8 s; i 150 BPM 6,4 s. Med ett
    /// byte vid takt 2 ska de två styckena tillsammans vara exakt 4 + 3,2 = 7,2 ut-sekunder —
    /// och varje stycke ska ha sin egen faktor.
    #[test]
    fn a_change_inside_the_clip_splits_it_and_the_pieces_add_up() {
        let mut punkter = vec![TempoPoint { start_bar: 0, bpm: 120.0 }];
        set_tempo_point(&mut punkter, 2, 150.0);
        let tempo = TempoMap::from_points(punkter);

        let pieces = stretch_pieces(0.0, 4.0, 0.0, 120.0, &tempo);
        assert_eq!(pieces.len(), 2, "bytet vid takt 2 ligger inuti spannet");

        let summa: f64 = pieces.iter().map(|p| p.out_secs).sum();
        let ur_kartan = tempo.secs_at_bar(4.0) - tempo.secs_at_bar(0.0);
        assert!(
            (summa - ur_kartan).abs() < 1e-9,
            "styckena summerar till {summa} men kartan säger {ur_kartan}"
        );
        assert!((summa - 7.2).abs() < 1e-6, "2 takter à 120 + 2 à 150 = 7,2 s, fick {summa}");

        // Enheten är projekt/källa: källan i 120 i ett projekt i 120 ger 1,0, och samma källa i
        // ett projekt i 150 ger 150/120 = 1,25 — filen spelas kortare, som ett högre tempo ska.
        assert!((pieces[0].ratio - 1.0).abs() < 1e-9);
        assert!(
            (pieces[1].ratio - 1.25).abs() < 1e-9,
            "en källa i 120 i ett projekt i 150 ger 150/120 = 1,25, fick {}",
            pieces[1].ratio
        );
        // Och källan fortsätter där det första slutade — inget hopp, inget glapp.
        let förväntat = pieces[0].source_offset_secs + pieces[0].out_secs * pieces[0].ratio;
        assert!((pieces[1].source_offset_secs - förväntat).abs() < 1e-9);
        assert!((pieces[1].out_start_secs - pieces[0].out_secs).abs() < 1e-9);
    }

    /// Ett byte **utanför** klippet ska inte dela något, och ett klipp som börjar *på* ett byte
    /// hör till det bytet — ändpunkterna är inte skär.
    #[test]
    fn changes_outside_the_clip_do_not_split_it() {
        let mut punkter = vec![TempoPoint { start_bar: 0, bpm: 120.0 }];
        set_tempo_point(&mut punkter, 8, 150.0);
        let tempo = TempoMap::from_points(punkter);
        assert_eq!(stretch_pieces(0.0, 4.0, 0.0, 120.0, &tempo).len(), 1);
        assert_eq!(stretch_pieces(8.0, 4.0, 0.0, 120.0, &tempo).len(), 1);

        let mut punkter2 = vec![TempoPoint { start_bar: 0, bpm: 120.0 }];
        set_tempo_point(&mut punkter2, 4, 150.0);
        let tempo2 = TempoMap::from_points(punkter2);
        assert_eq!(
            stretch_pieces(0.0, 4.0, 0.0, 120.0, &tempo2).len(),
            1,
            "bytet ligger precis vid klippets slut, inte inuti"
        );
    }

    /// Flera delar över tempobyten bevarar kontinuitet i källoffset och delar upp i rätt längder.
    #[test]
    fn multi_piece_playback_preserves_offsets_and_calculates_lengths() {
        let mut punkter = vec![TempoPoint { start_bar: 0, bpm: 120.0 }];
        set_tempo_point(&mut punkter, 2, 160.0);
        let tempo = TempoMap::from_points(punkter);

        let pieces = stretch_pieces(0.0, 4.0, 0.0, 120.0, &tempo);
        assert_eq!(pieces.len(), 2);

        // Piece 0: takter 0..2 @ 120 BPM = 4.0 sekunder
        assert!((pieces[0].out_start_secs - 0.0).abs() < 1e-6);
        assert!((pieces[0].out_secs - 4.0).abs() < 1e-6);
        assert!((pieces[0].source_offset_secs - 0.0).abs() < 1e-6);
        assert_eq!(pieces[0].bpm_here, 120.0);

        // Piece 1: takter 2..4 @ 160 BPM = 3.0 sekunder (2 * 60 / 160 * 4 = 3.0s)
        assert!((pieces[1].out_start_secs - 4.0).abs() < 1e-6);
        assert!((pieces[1].out_secs - 3.0).abs() < 1e-6);
        // Källan i piece 1 börjar exakt efter 4.0 källsekunder
        assert!((pieces[1].source_offset_secs - 4.0).abs() < 1e-6);
        assert_eq!(pieces[1].bpm_here, 160.0);
    }

