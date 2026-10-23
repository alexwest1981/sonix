//! **Två små instrument ur FL-gruppen** som inte behöver egna motorer:
//!
//! - **Autogun**: slumpar fram ett komplett ljud ur syntens egna parametrar. FL beskriver den som
//!   en synth med 4 miljarder ljud och ingen ratt — det är samma sak som att slumpa ett förval.
//! - **BeepMap**: gör deltonsnivåer av en bild. Här är "bilden" rådata: varje byte blir en
//!   deltons styrka. Det gör funktionen mätbar utan en bildavkodare i beroendelistan.

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
pub const OPERATOR_GLASS: [f32; HARMONIC_COUNT] = [
    1.0, 0.0, 0.55, 0.0, 0.32, 0.0, 0.20, 0.0, 0.12, 0.0, 0.08, 0.0, 0.05, 0.0, 0.03, 0.0,
];
pub const OPERATOR_REED: [f32; HARMONIC_COUNT] = [
    1.0, 0.70, 0.50, 0.38, 0.30, 0.24, 0.19, 0.15, 0.12, 0.10, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03,
];

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
        filter: FilterParams { cutoff: 9_000.0, resonance: 0.15, ..Default::default() },
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

}
