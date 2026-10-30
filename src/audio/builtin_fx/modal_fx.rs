//! **Modal resonator as a source-instrument.** The bank lives in `audio::modal`; this file is the
//! registry entry that turns the knobs into [`ModalParams`] and the note into a strike.
//!
//! The host sees one monophonic bank: a strike adds its mallet impulse to whatever is still
//! ringing, and `note_off` does nothing — a struck bar rings out, it does not stop when the key
//! goes up.

use crate::audio::modal::{ModalBank, ModalParams, MAX_MODES};
use crate::audio::plugin_host_live::{
    PluginInfo, PluginParameter, PluginProcessor,
};

/// Pseudo-path that makes this the synth source (`AudioCommand::SetInstrument`).
pub const MODAL_SOURCE_PATH: &str = "builtin:modal";

/// The knobs, in parameter-id order.
const PARAMS: [(&str, f64, f64, f64); 5] = [
    ("Decay (T60 s)", 0.05, 10.0, 1.0),
    ("Inharmonicity", 0.0, 0.1, 0.0),
    ("Material", 0.0, 1.0, 0.5),
    ("Mallet (0=soft, 1=hard)", 0.0, 1.0, 0.5),
    ("Modes", 1.0, MAX_MODES as f64, MAX_MODES as f64),
];

pub struct ModalSource {
    bank: ModalBank,
    params: ModalParams,
    /// Last struck frequency; the bank is retuned on it when a knob moves.
    freq: f32,
    info: PluginInfo,
    params_list: Vec<PluginParameter>,
}

impl ModalSource {
    pub fn new(sample_rate: f32) -> Self {
        let params_list = PARAMS
            .iter()
            .enumerate()
            .map(|(i, (name, min, max, default))| PluginParameter {
                id: i as u32,
                name: name.to_string(),
                module: String::new(),
                min_value: *min,
                max_value: *max,
                default_value: *default,
                flags: 0,
            })
            .collect();
        Self {
            bank: ModalBank::new(sample_rate),
            params: ModalParams::default(),
            freq: 0.0,
            info: PluginInfo {
                id: MODAL_SOURCE_PATH.to_string(),
                name: "Modal".to_string(),
                vendor: "Sonix".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: "Struck bar/membrane: modal resonator bank with a mallet exciter"
                    .to_string(),
                features: vec!["instrument".to_string()],
            },
            params_list,
        }
    }
}

impl PluginProcessor for ModalSource {
    fn backend(&self) -> &'static str {
        "modal"
    }

    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn parameters(&self) -> &[PluginParameter] {
        &self.params_list
    }

    fn latency_frames(&self) -> u32 {
        0
    }

    fn accepts_notes(&self) -> bool {
        true
    }

    fn note_on(&mut self, note: u8, velocity: u8) {
        self.freq = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
        self.bank.tune(self.freq, self.params);
        self.bank.strike(velocity as f32 / 127.0, self.params.hardness);
    }

    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        for i in 0..n {
            let s = self.bank.next();
            left[i] = s;
            right[i] = s;
        }
    }

    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        let v = value as f32;
        match id {
            0 => self.params.t60 = v.clamp(0.05, 10.0),
            1 => self.params.inharmonicity = v.clamp(0.0, 0.1),
            2 => self.params.material = v.clamp(0.0, 1.0),
            3 => self.params.hardness = v.clamp(0.0, 1.0),
            4 => self.params.modes = (v.clamp(1.0, MAX_MODES as f32)) as usize,
            _ => return false,
        }
        if self.freq > 0.0 {
            self.bank.tune(self.freq, self.params);
        }
        true
    }

    fn reset(&mut self) {
        self.bank.reset();
        self.freq = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::modal::mallet_seconds;

    #[test]
    fn a_strike_through_the_source_is_audible_and_finite() {
        let mut source = ModalSource::new(48_000.0);
        source.note_on(57, 100); // A3 = 220 Hz
        let mut l = vec![0.0; 9_600];
        let mut r = vec![0.0; 9_600];
        source.process_stereo(&mut l, &mut r);
        let peak = l.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        println!(
            "(source) A3 strike: peak {peak:.3}, pulse {:.2} ms, mallet knob default",
            1e3 * mallet_seconds(0.5)
        );
        assert!(l.iter().all(|s| s.is_finite()));
        assert!(peak > 0.05, "the strike should be audible, peak {peak}");
        // The modal path accepts every documented parameter id.
        for id in 0..PARAMS.len() as u32 {
            assert!(source.set_parameter(id, 0.5), "parameter {id} rejected");
        }
        assert!(!source.set_parameter(99, 0.0));
    }

    /// The pseudo-path really becomes the synth source: `SetInstrument` + `NoteOn` is heard.
    #[test]
    fn the_modal_source_is_reachable_through_set_instrument() {
        use crate::audio::synth::SynthEngine;
        let mut synth = SynthEngine::new(44_100.0);
        synth.handle_command(crate::audio::command::AudioCommand::SetInstrument {
            path: Some(MODAL_SOURCE_PATH.to_string()),
        });
        synth.handle_command(crate::audio::command::AudioCommand::NoteOn {
            note: 60,
            freq: 261.6,
            velocity: 1.0,
            channel: 0,
        });
        let level = (0..1024)
            .map(|_| synth.process_stereo().0.abs())
            .fold(0.0f32, f32::max);
        println!("(wiring) builtin:modal as the synth source: peak {level:.3} at C4");
        assert!(level > 0.02, "the modal source should be heard through the master, got {level}");
    }
}
