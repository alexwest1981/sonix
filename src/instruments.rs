//! **Två små instrument ur FL-gruppen** som inte behöver egna motorer:
//!
//! - **Autogun**: slumpar fram ett komplett ljud ur syntens egna parametrar. FL beskriver den som
//!   en synth med 4 miljarder ljud och ingen ratt — det är samma sak som att slumpa ett förval.
//! - **BeepMap**: gör deltonsnivåer av en bild. Här är "bilden" rådata: varje byte blir en
//!   deltons styrka. Det gör funktionen mätbar utan en bildavkodare i beroendelistan.
//! - **Transmitter**: data blir ljud — händelser ur bytepar, och samma data som ett spelbart ljud.

#![allow(dead_code, reason = "transmittern anropas ännu bara från sina prov; kopplingen till UI:t kommer efter")]

use crate::audio::command::{PresetSettings, Waveform};
use crate::audio::filter::FilterParams;
use crate::audio::AdsrParams;
use crate::audio::synth::HARMONIC_COUNT;
use crate::rng::Rng;

/// **Morphine**: två additiva operatorkällor, och ett läge mellan dem.
///
/// Operatorkällorna är fasta spektra — en glasig (udda deltoner) och en rörledad (alla deltoner).
/// Det som gör Morphin till Morphine är att läget är en ratt: ljudet är *mellan* två klanger, inte
/// ett av tolv förval.
pub const OPERATOR_GLASS: [f32; HARMONIC_COUNT] = {
    let mut h = [0.0f32; HARMONIC_COUNT];
    h[0] = 1.0;
    h[2] = 0.55;
    h[4] = 0.32;
    h[6] = 0.20;
    h[8] = 0.12;
    h[10] = 0.08;
    h[12] = 0.05;
    h[14] = 0.03;
    h
};
pub const OPERATOR_REED: [f32; HARMONIC_COUNT] = {
    let mut h = [0.0f32; HARMONIC_COUNT];
    let levels = [
        1.0, 0.70, 0.50, 0.38, 0.30, 0.24, 0.19, 0.15, 0.12, 0.10, 0.08, 0.07, 0.06, 0.05, 0.04,
        0.03,
    ];
    let mut i = 0;
    while i < levels.len() {
        h[i] = levels[i];
        i += 1;
    }
    h
};

/// Läget `t` mellan två spektra. 0 = `a` ordagrant, 1 = `b` ordagrant, utanför kläms det.
pub fn morph_spectrum(
    a: &[f32; HARMONIC_COUNT],
    b: &[f32; HARMONIC_COUNT],
    t: f32,
) -> [f32; HARMONIC_COUNT] {
    let t = t.clamp(0.0, 1.0);
    let mut out = [0.0; HARMONIC_COUNT];
    for i in 0..HARMONIC_COUNT {
        out[i] = a[i] + (b[i] - a[i]) * t;
    }
    out
}

/// Ett Morphine-ljud på läget `morph` mellan de två operatorkällorna.
pub fn morphine(morph: f32) -> PresetSettings {
    let mut sound = PresetSettings {
        waveform: Waveform::Additive,
        adsr: AdsrParams { attack: 0.02, decay: 0.5, sustain: 0.75, release: 0.35 },
        filter: FilterParams { cutoff: 9_000.0, resonance: 0.15 },
        filter_env_amount: 0.2,
        sub: 0.15,
        unison: 1,
        detune: 0.0,
        glide: 0.0,
        drive: 0.0,
        harmonics: morph_spectrum(&OPERATOR_GLASS, &OPERATOR_REED, morph),
    };
    sound.harmonics[0] = 1.0;
    sound
}

/// **Autogun**: ett spelbart ljud ur ett frö.
///
/// Slumpen är begränsad där den skulle kunna göra ljudet oanvändbart: attacken får aldrig bli så
/// lång att noten inte hörs, och filtret aldrig så stängt att ingenting kommer igenom. Det är
/// skillnaden mellan en slumpgenerator och ett instrument.
pub fn autogun(seed: u64) -> PresetSettings {
    let mut rng = Rng::new(seed.max(1));
    let waveforms = [
        Waveform::Sine,
        Waveform::Saw,
        Waveform::Square,
        Waveform::Triangle,
        Waveform::Additive,
    ];
    let waveform = waveforms[(rng.next_f32() * waveforms.len() as f32) as usize % waveforms.len()];

    let mut harmonics = [0.0f32; HARMONIC_COUNT];
    for (i, level) in harmonics.iter_mut().enumerate() {
        // Dragstänger: oftast noll, ibland stark — som att någon dragit ut några av dem.
        let draw = rng.next_f32();
        *level = if draw < 0.45 { 0.0 } else { (1.0 / (i as f32 + 1.0)) * draw };
    }

    PresetSettings {
        waveform,
        adsr: AdsrParams {
            attack: 0.001 + rng.next_f32() * 0.4,
            decay: 0.05 + rng.next_f32() * 0.8,
            sustain: 0.2 + rng.next_f32() * 0.8,
            release: 0.05 + rng.next_f32() * 1.2,
        },
        filter: FilterParams {
            // 200 Hz–12 kHz: under 200 blir det tyst, över 12 kHz hörs ingen skillnad.
            cutoff: 200.0 * (60.0f32).powf(rng.next_f32()),
            resonance: rng.next_f32() * 0.8,
        },
        filter_env_amount: rng.next_f32() * 0.8,
        sub: if rng.next_f32() < 0.4 { rng.next_f32() * 0.5 } else { 0.0 },
        unison: 1 + (rng.next_f32() * 3.0) as u8,
        detune: rng.next_f32() * 25.0,
        glide: if rng.next_f32() < 0.3 { rng.next_f32() * 0.2 } else { 0.0 },
        drive: rng.next_f32() * 0.4,
        harmonics,
    }
}

/// **BeepMap**: deltonsnivåer ur rådata. Varje byte blir styrkan på sin delton, normaliserat så
/// att nivån inte beror på hur stark datan råkade vara.
pub fn beepmap_amplitudes(data: &[u8]) -> [f32; HARMONIC_COUNT] {
    let mut levels = [0.0f32; HARMONIC_COUNT];
    if data.is_empty() {
        return levels;
    }
    let mut sum = 0.0;
    for (i, level) in levels.iter_mut().enumerate() {
        let byte = data[i % data.len()] as f32 / 255.0;
        *level = byte;
        sum += byte;
    }
    if sum > 0.0 {
        for level in levels.iter_mut() {
            *level /= sum;
        }
    }
    levels
}

/// Transmitterns grundton när ingen annan anges (A3).
pub const TRANSMITTER_BASE_HZ: f32 = 220.0;

/// **Transmitter**: data blir ljud. Varje bytepar blir en händelse — tonhöjd ur den första byten
/// (12 steg per oktav, så 12 bytar = en oktav) och längd ur den andra. Händelserna lämnas som en
/// platt parlista `hz, sek, hz, sek, …` i stället för en egen struktur, så att en spelare som redan
/// tar tal kan spela dem direkt. En udda sista byte blir ingen händelse.
pub fn transmitter(data: &[u8], base_hz: f32) -> Vec<f32> {
    data.as_chunks::<2>().0.iter()
        .flat_map(|p| {
            [
                base_hz * 2.0f32.powf(p[0] as f32 / 12.0),
                // 10 ms–510 ms: kortare hörs inte som en ton, längre gör datan olidlig.
                0.01 + p[1] as f32 / 255.0 * 0.5,
            ]
        })
        .collect()
}

/// **Transmitter som förval**: spektret är `beepmap` av samma data, och filtret följer datans
/// medelvärde — starkare data öppnar filtret, som en mätare över materialet.
pub fn transmitter_to_settings(data: &[u8]) -> PresetSettings {
    let mean = if data.is_empty() {
        0.0
    } else {
        data.iter().map(|b| *b as f32).sum::<f32>() / data.len() as f32
    };
    PresetSettings {
        waveform: Waveform::Additive,
        adsr: AdsrParams { attack: 0.005, decay: 0.2, sustain: 0.6, release: 0.3 },
        // 200 Hz–12 kHz, samma spann som autogun: under 200 blir det tyst, över 12 kHz hörs inget.
        filter: FilterParams { cutoff: 200.0 * 60.0f32.powf(mean / 255.0), resonance: 0.2 },
        filter_env_amount: 0.3,
        sub: 0.0,
        unison: 1,
        detune: 0.0,
        glide: 0.0,
        drive: 0.0,
        harmonics: beepmap_amplitudes(data),
    }
}

/// **Transmitter som fil**: händelserna renderade till mono, en sinus per händelse med 5 ms attack
/// och 30 ms släpp så att bytena inte klickar. Händelsegränserna räknas i sampel ur den löpande
/// tiden, så längden blir summan av längderna och inte en summa av avrundningar.
pub fn transmitter_wav(data: &[u8], sample_rate: u32) -> Vec<f32> {
    let events = transmitter(data, TRANSMITTER_BASE_HZ);
    if sample_rate == 0 || events.is_empty() {
        return Vec::new();
    }
    let sr = sample_rate as f32;
    let (attack, release) = (0.005 * sr, 0.030 * sr);
    let mut out = Vec::new();
    let mut at = 0.0f32; // händelsens början i sekunder
    for e in events.as_chunks::<2>().0 {
        let (hz, secs) = (e[0], e[1]);
        let start = (at * sr).round() as usize;
        let end = ((at + secs) * sr).round() as usize;
        out.resize(end, 0.0);
        let len = end - start;
        for (i, s) in out[start..end].iter_mut().enumerate() {
            // Kortare händelser än attack+släpp får en triangel i stället för att klippas.
            let gain = (i as f32 / attack).min((len - i) as f32 / release).min(1.0);
            *s = (i as f32 / sr * hz * std::f32::consts::TAU).sin() * gain;
        }
        at += secs;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Autogun är repeterbar och spelbar.** Samma frö ger samma ljud (annars går det inte att
    /// komma tillbaka till ett man gillade), och attacken är alltid kort nog att noten hörs.
    #[test]
    fn autogun_is_repeatable_and_always_playable() {
        // Jämför fälten: `PresetSettings` bär typer som inte är `PartialEq`, och att lägga
        // `PartialEq` på hela filtertypen för ett provs skull vore att ändra ljudkedjan för att
        // testa den.
        let a = autogun(7);
        let b = autogun(7);
        let c = autogun(8);
        let same = |x: &PresetSettings, y: &PresetSettings| {
            x.waveform == y.waveform
                && x.adsr == y.adsr
                && x.filter.cutoff == y.filter.cutoff
                && x.filter.resonance == y.filter.resonance
                && x.harmonics == y.harmonics
                && x.unison == y.unison
                && x.detune == y.detune
                && x.sub == y.sub
                && x.glide == y.glide
                && x.drive == y.drive
        };
        assert!(same(&a, &b), "samma frö ska ge samma ljud");
        assert!(!same(&a, &c), "olika frön ska ge olika ljud");

        for seed in 1..200u64 {
            let p = autogun(seed);
            assert!(p.adsr.attack <= 0.41, "attacken ska vara spelbar: {}", p.adsr.attack);
            assert!(p.filter.cutoff >= 200.0, "filtret ska släppa igenom: {}", p.filter.cutoff);
            assert!(p.adsr.sustain > 0.0, "ljudet ska höras efter anslaget");
            assert!((1..=4).contains(&p.unison));
        }
    }

    /// **BeepMap blir en normaliserad spektral bild**, och tom data ger tystnad i stället för
    /// division med noll.
    #[test]
    fn beepmap_turns_bytes_into_normalised_partial_levels() {
        let data = [0u8, 64, 128, 255, 32, 16];
        let levels = beepmap_amplitudes(&data);
        let sum: f32 = levels.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "nivåerna ska summera till 1: {sum}");
        assert!(levels[3] > levels[5], "255 ska väga mer än 16");

        // Samma data ger samma nivåer.
        assert_eq!(levels, beepmap_amplitudes(&data));
        // Annan data ger andra nivåer.
        assert_ne!(levels, beepmap_amplitudes(&[1u8, 2, 3]));
        // Tom data: tyst, ingen panik.
        assert_eq!(beepmap_amplitudes(&[]), [0.0f32; HARMONIC_COUNT]);
    }
    #[test]
    fn a_morph_blend_is_exact_at_the_ends_and_linear_between_them() {
        let (a, b) = (OPERATOR_GLASS, OPERATOR_REED);
        assert_eq!(morph_spectrum(&a, &b, 0.0), a, "läge 0 ska vara operator A ordagrant");
        assert_eq!(morph_spectrum(&a, &b, 1.0), b, "läge 1 ska vara operator B ordagrant");
        assert_eq!(morph_spectrum(&a, &b, -3.0), a, "utanför intervallet kläms till A");
        let mid = morph_spectrum(&a, &b, 0.5);
        for i in 0..HARMONIC_COUNT {
            assert!(
                (mid[i] - (a[i] + b[i]) * 0.5).abs() < 1e-6,
                "delton {i} ligger inte mitt emellan"
            );
        }
        // Och förvalet använder samma väg — inte en egen kopia av spektret.
        let preset = morphine(0.5);
        assert_eq!(preset.waveform, Waveform::Additive);
        assert_eq!(preset.harmonics, mid);
    }

    /// **En händelse per bytepar**, och tolv bytar är en oktav — byte 12 ligger exakt en oktan
    /// över byte 0.
    #[test]
    fn transmitter_makes_one_event_per_byte_pair() {
        let ev = transmitter(&[12, 200, 0, 10, 60, 5], 220.0);
        assert_eq!(ev.len(), 6, "två tal per händelse, tre händelser");
        assert_eq!(ev[0], 2.0 * ev[2], "byte 12 ska vara dubbla byte 0: {} vs {}", ev[0], ev[2]);
        assert!((ev[2] - 220.0).abs() < 1e-3, "byte 0 ska vara grundtonen: {}", ev[2]);
        // En udda sista byte blir ingen händelse.
        assert_eq!(transmitter(&[12, 1, 3], 220.0).len(), 2);
    }

    /// **Förvalet lånar beepmap** i stället för att räkna spektret en gång till, och starkare data
    /// öppnar filtret.
    #[test]
    fn transmitter_settings_take_their_spectrum_from_beepmap() {
        let data = [0u8, 90, 200, 30];
        let p = transmitter_to_settings(&data);
        assert_eq!(p.waveform, Waveform::Additive);
        assert_eq!(p.harmonics, beepmap_amplitudes(&data), "spektret ska komma ur beepmap");
        let loud = transmitter_to_settings(&[200u8; 8]).filter.cutoff;
        let quiet = transmitter_to_settings(&[10u8; 8]).filter.cutoff;
        assert!(loud > quiet, "högre medelvärde ska öppna filtret: {loud} vs {quiet}");
        assert!(quiet >= 200.0);
    }

    /// **Wav-längden är summan av händelselängderna**, och ljudet ligger inom ±1.
    #[test]
    fn transmitter_wav_length_matches_event_durations() {
        let data: Vec<u8> = (0..40u8).collect();
        let ev = transmitter(&data, TRANSMITTER_BASE_HZ);
        let total: f32 = ev.iter().skip(1).step_by(2).sum();
        let wav = transmitter_wav(&data, 44_100);
        let expected = total * 44_100.0;
        assert!((wav.len() as f32 - expected).abs() <= 1.0, "{} sampel, väntade {expected}", wav.len());
        assert!(wav.iter().any(|s| s.abs() > 0.5), "filen ska innehålla ljud");
        assert!(wav.iter().all(|s| s.abs() <= 1.0), "ingen sampel över noll");
    }

    /// **Tom data ger tom vektor** — ingen panik, ingen division med noll.
    #[test]
    fn transmitter_is_empty_without_data() {
        assert!(transmitter(&[], 220.0).is_empty());
        assert!(transmitter_wav(&[], 44_100).is_empty());
        assert!(transmitter_wav(&[1, 2], 0).is_empty(), "noll samplerate ska inte panika");
        let p = transmitter_to_settings(&[]);
        assert_eq!(p.waveform, Waveform::Additive);
        assert_eq!(p.harmonics, [0.0f32; HARMONIC_COUNT]);
        assert!(p.filter.cutoff >= 200.0);
    }
}
