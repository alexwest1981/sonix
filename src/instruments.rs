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
}
