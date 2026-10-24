//! Modulationsmatrisen — FL:s Envelope/Peak/Keyboard/Voltage Controller i en form.
//!
//! En slot är **källa → mål med styrka**. Källor: en LFO (egen takt och vågform), ett
//! envelopföljare på masterns nivå, senaste notens anslag och senaste notens tonhöjd.
//! Mål: masterns volym och panorering, rösternas tonhöjd och deras filter.
//!
//! Matrisen räknas **per sampel** — fyra slots är några multiplikationer, och en
//! modulation som bara uppdateras per buffert hörs som steg när man drar i en ratt.

/// Vågform för en LFO-källa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LfoShape {
    Sine,
    Triangle,
    Square,
    Random,
}

impl LfoShape {
    pub const ALL: [LfoShape; 4] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::Square,
        LfoShape::Random,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LfoShape::Sine => "sinus",
            LfoShape::Triangle => "triangel",
            LfoShape::Square => "fyrkant",
            LfoShape::Random => "slump",
        }
    }

    /// Värdet för en fas 0–1, i −1…1.
    fn at(self, phase: f32, seed: u32) -> f32 {
        match self {
            LfoShape::Sine => (phase * std::f32::consts::TAU).sin(),
            LfoShape::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
            LfoShape::Square => {
                if phase < 0.5 { 1.0 } else { -1.0 }
            }
            // Slump per period, inte per sampel: annars blir det brus, inte en LFO.
            LfoShape::Random => {
                let x = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 9) as f32 / 8_388_608.0 - 1.0
            }
        }
    }
}

/// Vad en slot läser.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ModSource {
    Lfo { rate_hz: f32, shape: LfoShape },
    /// Envelopföljare på masterns nivå — FL:s Peak Controller.
    Env,
    /// Senaste notens anslag, 0–1 — FL:s Keyboard Controller med velocity.
    Velocity,
    /// Senaste notens tonhöjd, centrerad kring C4 — FL:s Keyboard Controller.
    Key,
    /// **VFX Envelope**: en ADSR som triggas av noter — FL:s Envelope Controller som källa.
    /// Tiderna är i sekunder; releasen kräver en not-av-signal, annars stannar envelopen på
    /// sustain för alltid.
    Adsr { attack: f32, decay: f32, sustain: f32, release: f32 },
    /// Ett fast värde som sätts för hand — FL:s Voltage Controller, och det en X-Y-pad är
    /// gjord av: två rattar som var och en styr något. Ingen rörelse, bara ett läge.
    Voltage { value: f32 },
}

impl ModSource {
    pub fn label(&self) -> String {
        match self {
            ModSource::Lfo { .. } => "LFO".to_string(),
            ModSource::Env => "Envelop".to_string(),
            ModSource::Velocity => "Anslag".to_string(),
            ModSource::Key => "Tonhöjd".to_string(),
            ModSource::Adsr { .. } => "ADSR".to_string(),
            ModSource::Voltage { value } => format!("Spänning {:.0} %", value * 100.0),
        }
    }
}

/// Vad en slot styr.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModTarget {
    Volume,
    Pan,
    /// Tonhöjd i semitoner. Alla deras instrument modulerar pitch; vår matris kunde
    /// bara volym och panorering, och det var den största funktionella luckan.
    Pitch,
    /// Filtrets brytfrekvens i oktaver.
    Cutoff,
}

impl ModTarget {
    pub const ALL: [ModTarget; 4] = [
        ModTarget::Volume,
        ModTarget::Pan,
        ModTarget::Pitch,
        ModTarget::Cutoff,
    ];

    /// Hur långt en styrka på 1,0 räcker, i målETS enhet.
    pub fn full_scale(self) -> f32 {
        match self {
            ModTarget::Pitch => 12.0,
            ModTarget::Cutoff => 4.0,
            _ => 1.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModTarget::Volume => "volym",
            ModTarget::Pan => "panorering",
            ModTarget::Pitch => "tonhöjd",
            ModTarget::Cutoff => "filter",
        }
    }
}

/// En rad i matrisen.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModSlot {
    pub source: ModSource,
    pub target: ModTarget,
    /// −1…1. Negativ styrka vänder riktningen.
    pub amount: f32,
    pub enabled: bool,
}

impl ModSlot {
    pub fn new(source: ModSource, target: ModTarget, amount: f32) -> Self {
        Self { source, target, amount, enabled: true }
    }
}

/// Resultatet för ett sampel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModOut {
    /// Multiplikator på masterns volym (1,0 = ingen påverkan).
    pub volume: f32,
    /// Panorering −1…1 som läggs till den befintliga.
    pub pan: f32,
    /// Tonhöjd i semitoner, lagd på rösternas frekvens.
    pub pitch: f32,
    /// Filtrets brytfrekvens i oktaver, lagd på rösternas filter.
    pub cutoff: f32,
}

/// Matrisen med sina källors tillstånd.
#[derive(Clone, Debug, Default)]
pub struct ModMatrix {
    pub slots: Vec<ModSlot>,
    /// En fas per slot, så varje LFO har sin egen takt utan omskalning.
    phases: Vec<f32>,
    /// ADSR-envelopens nivå och läge per slot.
    adsr_levels: Vec<f32>,
    adsr_stages: Vec<u8>,
    random_seed: u32,
    env: f32,
    /// Senaste notens anslag 0–1.
    pub last_velocity: f32,
    /// Senaste notens MIDI-nummer.
    pub last_key: f32,
    /// Om senaste noten hörts (annars är Velocity/Key 0).
    pub has_note: bool,
}

impl ModMatrix {
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| !s.enabled)
    }

    /// Byter slots och håller faslistan i takt med dem.
    pub fn set_slots(&mut self, slots: Vec<ModSlot>) {
        self.phases.resize(slots.len(), 0.0);
        self.adsr_levels.resize(slots.len(), 0.0);
        self.adsr_stages.resize(slots.len(), 0);
        self.slots = slots;
    }

    /// Noterar en spelad not, så Velocity/Key-källorna har något att läsa.
    pub fn note_on(&mut self, key: u8, velocity: f32) {
        self.last_key = key as f32;
        self.last_velocity = velocity.clamp(0.0, 1.0);
        self.has_note = true;
        // Nya noter börjar om i attacken, oavsett var envelopen stod.
        self.adsr_stages.fill(1);
    }

    /// **Not-av.** Utan den här kan en ADSR aldrig släppa: `has_note` sattes men nollställdes
    /// aldrig, så envelopen hade stannat på sustain för alltid.
    pub fn note_off(&mut self) {
        self.has_note = false;
    }

    /// Ett sampel: räknar fram volym- och panpåverkan.
    ///
    /// `level` är masterns nivå **före** volymstegen, som envelopföljaren lyssnar på.
    pub fn tick(&mut self, sample_rate: f32, level: f32) -> ModOut {
        // Följaren går snabbt upp och långsamt ner (som en peak meter), oberoende av
        // antalet slots — den räknas en gång per sampel.
        let l = level.abs().min(4.0);
        let coeff = if l > self.env { 0.01 } else { 0.0005 };
        self.env += (l - self.env) * coeff;

        if self.slots.is_empty() || self.slots.iter().all(|s| !s.enabled) {
            return ModOut { volume: 1.0, pan: 0.0, pitch: 0.0, cutoff: 0.0 };
        }
        if self.phases.len() != self.slots.len() {
            self.phases.resize(self.slots.len(), 0.0);
        }

        let mut volume = 0.0f32;
        let mut pan = 0.0f32;
        let mut pitch = 0.0f32;
        let mut cutoff = 0.0f32;

        for i in 0..self.slots.len() {
            let slot = self.slots[i];
            if !slot.enabled || slot.amount == 0.0 {
                continue;
            }
            let value = match slot.source {
                ModSource::Lfo { rate_hz, shape } => {
                    let phase = &mut self.phases[i];
                    *phase += rate_hz.abs().max(0.01) / sample_rate.max(1.0);
                    if *phase >= 1.0 {
                        *phase -= phase.floor();
                        self.random_seed = self.random_seed.wrapping_add(1);
                    }
                    shape.at(*phase, self.random_seed.wrapping_add(i as u32))
                }
                ModSource::Env => self.env.min(1.0),
                ModSource::Velocity => {
                    if self.has_note { self.last_velocity } else { 0.0 }
                }
                ModSource::Key => {
                    if self.has_note {
                        ((self.last_key - 60.0) / 48.0).clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                }
                ModSource::Adsr { attack, decay, sustain, release } => {
                    if self.adsr_levels.len() != self.slots.len() {
                        self.adsr_levels.resize(self.slots.len(), 0.0);
                        self.adsr_stages.resize(self.slots.len(), 0);
                    }
                    let dt = 1.0 / sample_rate.max(1.0);
                    let level = &mut self.adsr_levels[i];
                    let stage = &mut self.adsr_stages[i];
                    if !self.has_note {
                        // Släppet går mot noll oavsett var nivån stod.
                        *level = (*level - dt / release.max(0.001)).max(0.0);
                    } else if *level < 1.0 && *stage <= 1 {
                        *level = (*level + dt / attack.max(0.001)).min(1.0);
                        if *level >= 1.0 {
                            *stage = 2;
                        }
                    } else {
                        // Efter attacken: mot sustain, upp eller ner.
                        let target = sustain.clamp(0.0, 1.0);
                        *level += ((target - *level) * dt / decay.max(0.001)).clamp(-1.0, 1.0);
                        *level = level.clamp(0.0, 1.0);
                    }
                    level.clamp(0.0, 1.0)
                }
                ModSource::Voltage { value } => value.clamp(-1.0, 1.0),
            };
            let scaled = value * slot.amount;
            match slot.target {
                ModTarget::Volume => volume += scaled,
                ModTarget::Pan => pan += scaled,
                ModTarget::Pitch => pitch += scaled * slot.target.full_scale(),
                ModTarget::Cutoff => cutoff += scaled * slot.target.full_scale(),
            }
        }

        ModOut {
            volume: (1.0 + volume).clamp(0.0, 4.0),
            pan: pan.clamp(-1.0, 1.0),
            // Tolv semitoner upp och ner räcker för vibrato, drill och en oktavs svep.
            pitch: pitch.clamp(-12.0, 12.0),
            // Fyra oktaver upp och ner: från stängt till helt öppet filter.
            cutoff: cutoff.clamp(-4.0, 4.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// **Spänningen står still och gör precis det ratten säger** — den är ingen källa som rör
    /// sig, och det är hela poängen: en X-Y-pad ska hålla sitt läge mellan två drag.
    #[test]
    fn a_voltage_source_holds_its_value() {
        let mut m = ModMatrix {
            slots: vec![ModSlot::new(
                ModSource::Voltage { value: 0.5 },
                ModTarget::Cutoff,
                1.0,
            )],
            ..ModMatrix::default()
        };
        let first = m.tick(SR, 0.0);
        for _ in 0..1_000 {
            let again = m.tick(SR, 0.0);
            assert_eq!(again.cutoff, first.cutoff, "spänningen ska stå still");
        }
        // 0,5 × fullskalan (4 oktaver) = 2 oktaver upp.
        assert!((first.cutoff - 2.0).abs() < 1e-5, "cutoff: {}", first.cutoff);
        // Negativt värde vänder riktningen.
        m.set_slots(vec![ModSlot::new(
            ModSource::Voltage { value: -0.25 },
            ModTarget::Pitch,
            1.0,
        )]);
        let down = m.tick(SR, 0.0);
        assert!((down.pitch + 3.0).abs() < 1e-5, "pitch: {}", down.pitch);
    }

    #[test]
    fn an_lfo_at_one_hertz_completes_a_cycle_in_one_second() {
        let mut m = ModMatrix {
            slots: vec![ModSlot::new(
                ModSource::Lfo { rate_hz: 1.0, shape: LfoShape::Sine },
                ModTarget::Volume,
                1.0,
            )],
            ..Default::default()
        };
        // Ett kvarts varv in: sinusen är 1, alltså volymen 2.
        let mut out = ModOut { volume: 1.0, pan: 0.0, pitch: 0.0, cutoff: 0.0 };
        for _ in 0..(48_000 / 4) {
            out = m.tick(SR, 0.0);
        }
        assert!((out.volume - 2.0).abs() < 0.02, "volymen blev {}", out.volume);
        // Tre kvarts varv in är sinusen −1, alltså volymen 0 (ett halvt varv är nollgenomgången).
        for _ in 0..(48_000 / 2) {
            out = m.tick(SR, 0.0);
        }
        assert!(out.volume < 0.05, "volymen blev {}", out.volume);
    }

    #[test]
    fn a_disabled_slot_or_zero_amount_changes_nothing() {
        let mut off = ModSlot::new(ModSource::Env, ModTarget::Volume, 1.0);
        off.enabled = false;
        let mut m = ModMatrix { slots: vec![off], ..Default::default() };
        for _ in 0..1000 {
            let out = m.tick(SR, 1.0);
            assert_eq!((out.volume, out.pan), (1.0, 0.0));
        }
        let mut zero = ModMatrix {
            slots: vec![ModSlot::new(ModSource::Env, ModTarget::Volume, 0.0)],
            ..Default::default()
        };
        for _ in 0..1000 {
            assert_eq!(zero.tick(SR, 1.0).volume, 1.0);
        }
    }

    #[test]
    fn the_envelope_follower_rises_fast_and_falls_slowly() {
        let mut m = ModMatrix::default();
        for _ in 0..48_000 {
            m.tick(SR, 1.0);
        }
        let high = m.env;
        assert!(high > 0.8, "följaren kom bara till {high}");
        for _ in 0..48_000 {
            m.tick(SR, 0.0);
        }
        assert!(m.env < high * 0.5, "föll för långsamt: {}", m.env);
        assert!(m.env >= 0.0);
    }

    #[test]
    fn velocity_and_key_read_the_last_note_and_leave_center_at_c4() {
        let mut m = ModMatrix {
            slots: vec![
                ModSlot::new(ModSource::Velocity, ModTarget::Volume, 1.0),
                ModSlot::new(ModSource::Key, ModTarget::Pan, 1.0),
            ],
            ..Default::default()
        };
        // Ingen not än: båda källorna är 0.
        let out = m.tick(SR, 0.0);
        assert_eq!((out.volume, out.pan), (1.0, 0.0));

        m.note_on(72, 0.5);
        let out = m.tick(SR, 0.0);
        assert!((out.volume - 1.5).abs() < 0.01, "anslag 0,5 ska ge volym 1,5: {}", out.volume);
        assert!((out.pan - 0.25).abs() < 0.01, "C5 är en oktav över C4: {}", out.pan);
    }

    #[test]
    fn shapes_are_bipolar_and_bounded() {
        for shape in LfoShape::ALL {
            for i in 0..64 {
                let p = i as f32 / 64.0;
                let v = shape.at(p, 7);
                assert!((-1.0..=1.0).contains(&v), "{shape:?} gav {v}");
            }
        }
    }
    #[test]
    fn pitch_and_cutoff_have_their_own_scales() {
        let mut m = ModMatrix::default();
        m.set_slots(vec![
            ModSlot::new(ModSource::Velocity, ModTarget::Pitch, 1.0),
            ModSlot::new(ModSource::Velocity, ModTarget::Cutoff, 0.5),
        ]);
        m.note_on(60, 1.0);
        let out = m.tick(48_000.0, 0.0);
        // Styrka 1,0 på tonhöjd = en oktav; 0,5 på filter = två oktaver.
        assert!((out.pitch - 12.0).abs() < 1e-4, "tonhöjd: {}", out.pitch);
        assert!((out.cutoff - 2.0).abs() < 1e-4, "filter: {}", out.cutoff);
        assert!((out.volume - 1.0).abs() < 1e-4, "volymen ska vara orörd");
    }

    #[test]
    fn the_targets_are_clamped_to_their_range() {
        let mut m = ModMatrix::default();
        // Tre rader på samma mål: 3 × 12 semitoner får inte bli 36.
        m.set_slots(vec![
            ModSlot::new(ModSource::Velocity, ModTarget::Pitch, 1.0),
            ModSlot::new(ModSource::Velocity, ModTarget::Pitch, 1.0),
            ModSlot::new(ModSource::Velocity, ModTarget::Pitch, 1.0),
        ]);
        m.note_on(60, 1.0);
        let out = m.tick(48_000.0, 0.0);
        assert!((out.pitch - 12.0).abs() < 1e-4, "taket ska vara tolv semitoner: {}", out.pitch);
    }

    /// **VFX Envelope**: attack stiger till 1, decay faller mot sustain, och en not-av släpper
    /// den till noll. Provet visar också varför `note_off` måste finnas: utan den ligger nivån
    /// kvar på sustain hur länge som helst.
    #[test]
    fn an_adsr_modulation_source_rises_falls_and_releases_on_note_off() {
        let mut m = ModMatrix::default();
        m.set_slots(vec![ModSlot::new(
            ModSource::Adsr { attack: 0.01, decay: 0.02, sustain: 0.5, release: 0.02 },
            ModTarget::Volume,
            1.0,
        )]);
        m.note_on(60, 1.0);
        let sr = 48_000.0;
        let at = |m: &mut ModMatrix, samples: usize| {
            let mut v = 0.0;
            for _ in 0..samples {
                v = m.tick(sr, 0.0).volume - 1.0; // volymfaktorn är 1,0 utan påverkan
            }
            v
        };
        let early = at(&mut m, 240);            // 5 ms in i en 10 ms attack
        let peak = at(&mut m, 480);             // attacken klar
        let held = at(&mut m, 4_800);           // efter decay
        println!("adsr: {early:.2} → {peak:.2} → {held:.2}");
        assert!(early > 0.2 && early < 0.9, "attacken ska vara på väg: {early:.2}");
        assert!(peak > 0.85, "attacken ska nå toppen innan decay tar över: {peak:.2}");
        assert!((held - 0.5).abs() < 0.1, "decay ska landa på sustain 0,5: {held:.2}");

        m.note_off();
        let released = at(&mut m, 2_400);       // 50 ms efter not-av
        println!("adsr efter not-av: {released:.3}");
        assert!(released < 0.05, "releasen ska ha gått mot noll: {released:.3}");
    }

}
