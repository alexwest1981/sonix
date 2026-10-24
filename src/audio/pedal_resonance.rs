//! **Pedalresonans** — sympatiska strängar när sustainpedalen är nere (FL Keys).
//!
//! Ett piano med pedalen nere låter större: de strängar som inte slås an svarar på de som gör
//! det, och hela soundboarden ringer med. Här är det en **bank av tre stämda kammar** (A2, E3,
//! A3) som matas med programmet bara medan pedalen är nere och läggs tillbaka i mastern.
//!
//! shortcut: tre kammar på fasta frekvenser, inte en modell av strängarna. Det ger efterklang
//! och kropp, inte en exakt kopia. Byt till modal syntes om skillnaden måste vara ohörbar.

/// Antal kammar. Tre räcker för att höra en kropp; fler låter som en kör av stämmor.
const COMBS: usize = 3;
/// Stämda frekvenser: en A-oktav med kvint — samma intervall som en öppen sträng låter i.
const FREQS: [f32; COMBS] = [110.0, 164.8, 220.0];
/// Efterklangens T60 i sekunder. Ett piano med pedalen nere ringer ungefär så här länge.
const T60: f32 = 0.9;
/// Hur mycket av kammarna som hörs i mastern. Lågt: det ska vara en kropp, inte en reverb.
const MIX: f32 = 0.18;

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
}

impl Comb {
    fn new(sample_rate: f32, freq: f32) -> Self {
        // Fördröjningen är en period av frekvensen; rundgången ger T60 sekunders efterklang.
        let len = ((sample_rate / freq).round() as usize).max(2);
        let feedback = 10f32.powf(-3.0 * len as f32 / (T60 * sample_rate));
        Self {
            buf: vec![0.0; len],
            idx: 0,
            feedback,
        }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let y = self.buf[self.idx];
        self.buf[self.idx] = x + y * self.feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        y
    }
}

pub struct PedalResonance {
    combs: [Comb; COMBS],
}

impl PedalResonance {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            combs: std::array::from_fn(|i| Comb::new(sample_rate, FREQS[i])),
        }
    }

    /// Matas med **instrumenten** (synt + samplingar), aldrig trummor eller stämmor: en
    /// sträng bankar inte på en bastrumma. Anroparen skickar 0 när pedalen är uppe, och då
    /// ringer kammarna bara ut.
    pub fn process(&mut self, x: f32) -> f32 {
        let mut wet = 0.0;
        for comb in &mut self.combs {
            wet += comb.tick(x);
        }
        wet * MIX / COMBS as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En kort puls ska lämna **efterklang** när pedalen är nere, och ingenting när den är uppe.
    #[test]
    fn the_combs_ring_only_while_the_pedal_is_down() {
        let sr = 48_000.0;
        let burst = |feed: bool| {
            let mut res = PedalResonance::new(sr);
            // 50 ms puls, sedan tystnad: mät hur mycket som finns kvar 300 ms senare.
            let mut tail = 0.0f32;
            for i in 0..14_400 {
                let x = if feed && i < 2_400 { 0.5 * (i as f32 / 2_400.0) } else { 0.0 };
                let l = res.process(x);
                if i >= 14_400 - 2_400 {
                    tail += l * l;
                }
            }
            tail
        };
        let pedal = burst(true);
        let dry = burst(false);
        println!("  pedalen nere {:.3e}, uppe {:.3e}", pedal, dry);
        assert!(pedal > 1e-4, "efterklang med pedalen nere: {pedal:.3e}");
        assert!(
            pedal > dry * 100.0,
            "utan pedal ska det vara tyst: {pedal:.3e} mot {dry:.3e}"
        );
    }
}
