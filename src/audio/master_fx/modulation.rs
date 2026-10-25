use super::*;

// ---------------------------------------------------------------------------
// Stereo doubler / chorus
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoublerParams {
    pub strength: f32, // 0..1 wet amount
    pub modulation: f32, // 0..1 LFO depth
    pub width: f32, // 0..1 stereo spread
}

impl Default for DoublerParams {
    fn default() -> Self {
        Self { strength: 0.7, modulation: 0.35, width: 0.8 }
    }
}

pub struct Doubler {
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    phase: f32,
}

impl Doubler {
    pub fn new(sample_rate: f32) -> Self {
        let len = ((sample_rate * 0.1) as usize).max(1);
        Self {
            sample_rate,
            buffer_l: vec![0.0; len],
            buffer_r: vec![0.0; len],
            write_pos: 0,
            phase: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &DoublerParams) -> (f32, f32) {
        if p.strength <= 0.001 {
            return (l, r);
        }
        let len = self.buffer_l.len();
        self.buffer_l[self.write_pos] = l;
        self.buffer_r[self.write_pos] = r;

        self.phase += 0.35 / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo = (self.phase * 2.0 * PI).sin();

        let base_ms = 22.0;
        let depth_ms = 8.0 * p.modulation;
        let dl = ((base_ms + depth_ms * lfo) * 0.001 * self.sample_rate) as usize;
        let dr = ((base_ms - depth_ms * lfo) * 0.001 * self.sample_rate) as usize;

        let read_l = (self.write_pos + len - dl.min(len - 1)) % len;
        let read_r = (self.write_pos + len - dr.min(len - 1)) % len;
        let wet_l = self.buffer_l[read_l];
        let wet_r = self.buffer_r[read_r];

        self.write_pos = (self.write_pos + 1) % len;

        // Mid/side width widening of the wet signal.
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5 * (0.5 + p.width * 1.5);
        let (wet_l, wet_r) = (mid + side, mid - side);

        let mix = p.strength.clamp(0.0, 1.0);
        (l * (1.0 - mix) + wet_l * mix, r * (1.0 - mix) + wet_r * mix)
    }
}

// ---------------------------------------------------------------------------
// Modulation: chorus, flanger, phaser (FL Studio: Fruity/Hyper/Vintage Chorus,
// Fruity Flanger, Fruity Phaser — tio varianter av samma tre idéer)
// ---------------------------------------------------------------------------

/// Gemensamma reglage för de tre modulerande effekterna.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModFxParams {
    /// LFO-hastighet i Hz (0,02–8).
    pub rate: f32,
    /// 0–1: hur långt delaytiden (chorus/flanger) eller brytfrekvensen (phaser) sveper.
    pub depth: f32,
    /// Torr/våt. 0 = ren genomgång. Phaserns kam ligger djupast vid 0,5.
    pub mix: f32,
    /// −0,9–0,9 återkoppling: flangerns vassa kam, phaserns resonans.
    pub feedback: f32,
}

impl Default for ModFxParams {
    fn default() -> Self {
        Self { rate: 0.4, depth: 0.4, mix: 0.0, feedback: 0.0 }
    }
}

/// Modulerad fördröjningslinje. Chorus och flanger är samma krets med olika skalor:
/// chorus har lång bas och litet svep, flangern kort bas, stort svep och återkoppling.
pub struct ModDelay {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    phase: f32,
    base_ms: f32,
    swing_ms: f32,
    delay_ms: f32,
}

impl ModDelay {
    pub fn new(sample_rate: f32, base_ms: f32, swing_ms: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = ((base_ms + swing_ms) * 0.001 * sr).ceil() as usize + 4;
        Self {
            sample_rate: sr,
            buf_l: vec![0.0; cap],
            buf_r: vec![0.0; cap],
            write: 0,
            phase: 0.0,
            base_ms,
            swing_ms,
            delay_ms: base_ms,
        }
    }

    /// Aktuell fördröjning i millisekunder — går att mäta direkt i stället för att gissa ur ljudet.
    #[cfg(test)]
    pub fn delay_ms(&self) -> f32 {
        self.delay_ms
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &ModFxParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.001 {
            return (l, r);
        }
        self.phase += p.rate.clamp(0.02, 8.0) / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let depth = p.depth.clamp(0.0, 1.0);
        // Kvarts period mellan kanalerna: det är bredden i choruset.
        let lfo_l = 0.5 + 0.5 * (2.0 * PI * self.phase).sin();
        let lfo_r = 0.5 + 0.5 * (2.0 * PI * (self.phase + 0.25)).sin();
        self.delay_ms = self.base_ms + self.swing_ms * depth * lfo_l;
        let delay_r = self.base_ms + self.swing_ms * depth * lfo_r;

        let n = self.buf_l.len() as f32;
        let max_d = n - 2.0;
        let tap = |buf: &[f32], write: usize, d: f32| -> f32 {
            let d = d.clamp(1.0, max_d);
            let mut read = write as f32 - d;
            if read < 0.0 {
                read += n;
            }
            let i0 = read.floor() as usize % buf.len();
            let frac = read - read.floor();
            let i1 = (i0 + 1) % buf.len();
            buf[i0] + frac * (buf[i1] - buf[i0])
        };

        let wet_l = tap(&self.buf_l, self.write, self.delay_ms * 0.001 * self.sample_rate);
        let wet_r = tap(&self.buf_r, self.write, delay_r * 0.001 * self.sample_rate);

        // Återkopplingen skrivs in i bufferten (inte ut i signalen): kammen blir vass utan att nivån skenar.
        let fb = p.feedback.clamp(-0.9, 0.9);
        self.buf_l[self.write] = l + wet_l * fb;
        self.buf_r[self.write] = r + wet_r * fb;
        self.write = (self.write + 1) % self.buf_l.len();

        (l + mix * (wet_l - l), r + mix * (wet_r - r))
    }
}

/// Fyra allpass-steg med svepande brytfrekvens — phaser. Vid `mix = 1,0` är utgången
/// bara fasvriden (allpass), så kammen hörs först när torr och våt blandas: 0,5 ger djupast kam.
pub struct Phaser {
    sample_rate: f32,
    phase: f32,
    x1_l: [f32; 4],
    x1_r: [f32; 4],
    y1_l: [f32; 4],
    y1_r: [f32; 4],
    fb_l: f32,
    fb_r: f32,
    fc_hz: f32,
}

impl Phaser {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            phase: 0.0,
            x1_l: [0.0; 4],
            x1_r: [0.0; 4],
            y1_l: [0.0; 4],
            y1_r: [0.0; 4],
            fb_l: 0.0,
            fb_r: 0.0,
            fc_hz: 200.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &ModFxParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.001 {
            return (l, r);
        }
        self.phase += p.rate.clamp(0.02, 8.0) / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo = 0.5 + 0.5 * (2.0 * PI * self.phase).sin();
        self.fc_hz = 200.0 * 10.0_f32.powf(p.depth.clamp(0.0, 1.0) * lfo);
        let t = (PI * self.fc_hz / self.sample_rate).tan();
        // a = (tan−1)/(tan+1) hör till y = a·x + x1 − a·y1; den omvända kvoten ger inget filter alls.
        let c = (t - 1.0) / (t + 1.0);
        let fb = p.feedback.clamp(-0.9, 0.9);

        let mut xl = l + self.fb_l * fb;
        let mut xr = r + self.fb_r * fb;
        for s in 0..4 {
            let yl = c * xl + self.x1_l[s] - c * self.y1_l[s];
            self.x1_l[s] = xl;
            self.y1_l[s] = yl;
            xl = yl;
            let yr = c * xr + self.x1_r[s] - c * self.y1_r[s];
            self.x1_r[s] = xr;
            self.y1_r[s] = yr;
            xr = yr;
        }
        self.fb_l = xl;
        self.fb_r = xr;

        (l + mix * (xl - l), r + mix * (xr - r))
    }
}

// ---------------------------------------------------------------------------
// DJ performance FX: beat-repeat / reverse stutter and tape-stop varispeed
// ---------------------------------------------------------------------------

/// Beat-repeat / stutter / reverse effect for the Remix FX view.
///
/// When a mode is engaged the engine captures one subdivision of the master
/// bus (based on the project tempo) and loops it. Mode 4 plays the captured
/// slice backwards.
#[derive(Debug, Clone)]
pub struct RemixFx {
    pub mode: u8, // 0 = off, 1 = 1/4, 2 = 1/8, 3 = 1/16, 4 = reverse
    bpm: f32,
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    read_pos: usize,
    capture_len: usize,
    capturing: bool,
    active: bool,
}

impl RemixFx {
    pub fn new(sample_rate: f32) -> Self {
        let cap = 96_000; // ~2 s at 48 kHz, enough for 1 beat at 40 BPM
        Self {
            mode: 0,
            bpm: 120.0,
            sample_rate,
            buffer_l: vec![0.0; cap],
            buffer_r: vec![0.0; cap],
            write_pos: 0,
            read_pos: 0,
            capture_len: 0,
            capturing: false,
            active: false,
        }
    }

    pub fn set(&mut self, mode: u8, bpm: f32) {
        if bpm > 1.0 {
            self.bpm = bpm;
        }
        if mode != self.mode {
            self.mode = mode;
            self.active = mode != 0;
            self.capturing = self.active;
            self.write_pos = 0;
            self.read_pos = 0;
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.active {
            return (l, r);
        }
        let beat = 60.0 / self.bpm.max(20.0) * self.sample_rate;
        let seg = match self.mode {
            2 => beat * 0.5,
            3 => beat * 0.25,
            _ => beat,
        } as usize;
        self.capture_len = seg.clamp(64, self.buffer_l.len());

        if self.capturing {
            self.buffer_l[self.write_pos] = l;
            self.buffer_r[self.write_pos] = r;
            self.write_pos += 1;
            if self.write_pos >= self.capture_len {
                self.write_pos = 0;
                self.capturing = false;
                self.read_pos = 0;
            }
            return (l, r);
        }

        let (ol, or) = if self.mode == 4 {
            self.read_pos = if self.read_pos == 0 {
                self.capture_len - 1
            } else {
                self.read_pos - 1
            };
            (self.buffer_l[self.read_pos], self.buffer_r[self.read_pos])
        } else {
            let o = (self.buffer_l[self.read_pos], self.buffer_r[self.read_pos]);
            self.read_pos += 1;
            if self.read_pos >= self.capture_len {
                self.read_pos = 0;
            }
            o
        };
        (ol, or)
    }
}

impl Default for RemixFx {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

/// Tape-stop varispeed: ramps the master output playback rate to zero when
/// engaged and back to normal when released, producing the classic pitch-down
/// and stop.
#[derive(Debug, Clone)]
pub struct TapeStop {
    active: bool,
    rate: f32,
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    read_pos: f32,
}

impl TapeStop {
    pub fn new(sample_rate: f32) -> Self {
        let cap = 1 << 15;
        Self {
            active: false,
            rate: 1.0,
            sample_rate,
            buffer_l: vec![0.0; cap],
            buffer_r: vec![0.0; cap],
            write_pos: 0,
            read_pos: 0.0,
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let n = self.buffer_l.len();
        self.buffer_l[self.write_pos] = l;
        self.buffer_r[self.write_pos] = r;

        let target = if self.active { 0.0 } else { 1.0 };
        let step = 1.0 / (self.sample_rate * 0.8).max(1.0);
        if self.rate < target {
            self.rate = (self.rate + step).min(target);
        } else if self.rate > target {
            self.rate = (self.rate - step).max(target);
        }

        // Read at the current position *before* advancing it. `read_pos` starts
        // equal to `write_pos`, so at rate 1.0 the just-written sample passes
        // through with no added latency; as the rate falls, the read pointer
        // lags behind the write pointer and the pitch drops.
        let i0 = self.read_pos as usize % n;
        let i1 = (i0 + 1) % n;
        let frac = self.read_pos - self.read_pos.floor();
        let ol = self.buffer_l[i0] * (1.0 - frac) + self.buffer_l[i1] * frac;
        let or = self.buffer_r[i0] * (1.0 - frac) + self.buffer_r[i1] * frac;

        self.read_pos += self.rate;
        if self.read_pos >= n as f32 {
            self.read_pos -= n as f32;
        }

        self.write_pos = (self.write_pos + 1) % n;

        // Fade with the rate so a fully stopped tape is silent.
        let fade = self.rate.clamp(0.0, 1.0);
        (ol * fade, or * fade)
    }
}

impl Default for TapeStop {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

/// **Delay-bank** (FL: Fruity Delay Bank): tre delaylinor med egen tid och nivå, en gemensam
/// återkoppling och en stereospridning som flyttar linjerna åt varsitt håll. Skillnaden mot
/// den flerbandiga delayn är att den här delar på **tid**, inte på frekvens: alla tre linjerna
/// får samma spektrum.
pub struct DelayBank {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelayBankParams {
    pub times_ms: [f32; 3],
    pub levels: [f32; 3],
    pub feedback: f32,
    /// 0 = alla linjer i mitten, 1 = varannan vänster/höger.
    pub spread: f32,
    pub mix: f32,
}

impl Default for DelayBankParams {
    fn default() -> Self {
        Self {
            times_ms: [180.0, 360.0, 540.0],
            levels: [1.0, 0.7, 0.5],
            feedback: 0.35,
            spread: 0.6,
            mix: 0.3,
        }
    }
}

impl DelayBank {
    const MAX_MS: f32 = 1500.0;

    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = (Self::MAX_MS * 0.001 * sr).ceil() as usize + 4;
        Self {
            sample_rate: sr,
            buf_l: vec![0.0; cap],
            buf_r: vec![0.0; cap],
            write: 0,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &DelayBankParams) -> (f32, f32) {
        let len = self.buf_l.len();
        let tap = |from: usize, ms: f32| -> usize {
            let samples = ((ms.clamp(1.0, Self::MAX_MS) * 0.001 * self.sample_rate) as usize)
                .clamp(1, len - 1);
            (from + len - samples) % len
        };

        let (mut wet_l, mut wet_r) = (0.0, 0.0);
        for (i, &ms) in p.times_ms.iter().enumerate() {
            let level = p.levels[i].clamp(0.0, 1.0);
            let idx = tap(self.write, ms);
            // Varannan linje till vänster, varannan till höger, så mycket som spridningen
            // säger: 0 ger tre centrerade linjer.
            let side = if i % 2 == 0 { -1.0 } else { 1.0 } * p.spread.clamp(0.0, 1.0);
            wet_l += self.buf_l[idx] * level * 0.5 * (1.0 - side);
            wet_r += self.buf_r[idx] * level * 0.5 * (1.0 + side);
        }

        // Återkopplingen går tillbaka **in i linjen**, inte in i mixen: annars hade den torra
        // signalen påverkats av delayens egen utnivå.
        let fb = p.feedback.clamp(0.0, 0.95);
        self.buf_l[self.write] = l + wet_l * fb;
        self.buf_r[self.write] = r + wet_r * fb;
        self.write = (self.write + 1) % len;

        let mix = p.mix.clamp(0.0, 1.0);
        (l + wet_l * mix, r + wet_r * mix)
    }
}

// ---------------------------------------------------------------------------
// Hyper Chorus
// ---------------------------------------------------------------------------

/// **Hyper Chorus**: fyra chorusröster med egna, något olika LFO-hastigheter och
/// fördröjningar, panorera(de ut i sidorna. Att stapla röster är hela skillnaden mot ett
/// vanligt chorus: en röst ger en svepande kopia, fyra ger en bredd som inte pumpar i takt
/// med sig själv, eftersom rösterna glider isär.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperChorusParams {
    /// LFO-hastighet i Hz för första rösten; de andra följer med små avvikelser.
    pub rate: f32,
    /// 0–1: svepets djup.
    pub depth: f32,
    /// 0–1: hur mycket av sid-signalen som behålls. 0 = monokompatibelt.
    pub width: f32,
    pub mix: f32,
    pub enabled: bool,
}

impl Default for HyperChorusParams {
    fn default() -> Self {
        Self { rate: 0.35, depth: 0.45, width: 0.8, mix: 0.35, enabled: false }
    }
}

/// Fyra röster: basfördröjning i ms och LFO-faktor mot grundhastigheten.
const HYPER_BASE_MS: [f32; 4] = [9.0, 13.5, 19.0, 26.0];
const HYPER_RATE_MUL: [f32; 4] = [1.0, 1.13, 0.87, 1.27];
/// Varje rösts vikt mot vänster och höger kanal — bredden kommer från att de ligger olika.
const HYPER_PAN: [(f32, f32); 4] = [(0.9, 0.35), (0.45, 0.85), (0.25, 0.75), (0.75, 0.45)];

pub struct HyperChorus {
    voices: [ModDelay; 4],
}

impl HyperChorus {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: [
                ModDelay::new(sample_rate, HYPER_BASE_MS[0], 3.5),
                ModDelay::new(sample_rate, HYPER_BASE_MS[1], 3.0),
                ModDelay::new(sample_rate, HYPER_BASE_MS[2], 2.5),
                ModDelay::new(sample_rate, HYPER_BASE_MS[3], 2.0),
            ],
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &HyperChorusParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if !p.enabled || mix <= 0.001 {
            return (l, r);
        }
        let rate = p.rate.clamp(0.02, 8.0);
        let mut wl = 0.0_f32;
        let mut wr = 0.0_f32;
        for (i, voice) in self.voices.iter_mut().enumerate() {
            let vp = ModFxParams {
                rate: rate * HYPER_RATE_MUL[i],
                depth: p.depth.clamp(0.0, 1.0),
                mix: 1.0,
                feedback: 0.0,
            };
            let (a, b) = voice.process(l, r, &vp);
            wl += a * HYPER_PAN[i].0;
            wr += b * HYPER_PAN[i].1;
        }
        // Fyra röster summerade: 0,35 håller våten kvar i samma nivå som torrt.
        wl *= 0.35;
        wr *= 0.35;
        let mid = 0.5 * (wl + wr);
        let side = 0.5 * (wl - wr) * p.width.clamp(0.0, 1.0);
        let wet = (mid + side, mid - side);
        (l * (1.0 - mix) + wet.0 * mix, r * (1.0 - mix) + wet.1 * mix)
    }
}

#[cfg(test)]
mod hyper_chorus_tests {
    use super::*;

    fn mono(secs: f32, freq: f32, sr: f32) -> Vec<f32> {
        (0..(secs * sr) as usize)
            .map(|i| 0.4 * (std::f32::consts::TAU * freq * i as f32 / sr).sin())
            .collect()
    }

    #[test]
    fn a_hundred_percent_mix_is_the_dry_signal() {
        let sr = 48_000.0;
        let input = mono(0.3, 220.0, sr);
        let mut fx = HyperChorus::new(sr);
        let mut p = HyperChorusParams::default();
        p.enabled = true;
        p.mix = 0.0;
        for &x in &input {
            assert_eq!(fx.process(x, x, &p), (x, x));
        }
    }

    #[test]
    fn stacked_voices_widen_a_mono_signal() {
        let sr = 48_000.0;
        let input = mono(1.0, 220.0, sr);
        let mut fx = HyperChorus::new(sr);
        let mut p = HyperChorusParams::default();
        p.enabled = true;
        p.mix = 1.0;
        p.width = 1.0;
        let mut side_energy = 0.0_f64;
        let mut peak = 0.0_f32;
        for &x in input.iter().skip(24_000) {
            let (l, r) = fx.process(x, x, &p);
            side_energy += ((l - r) * 0.5).abs() as f64;
            peak = peak.max(l.abs()).max(r.abs());
        }
        let n = (input.len() - 24_000) as f64;
        assert!(side_energy / n > 0.01, "inget sidmaterial — ingen bredd: {}", side_energy / n);
        assert!(peak < 1.0, "fyra röster får inte dra över full skala: {peak:.3}");
    }

    #[test]
    fn zero_width_is_mono_compatible() {
        let sr = 48_000.0;
        let input = mono(0.5, 220.0, sr);
        let mut fx = HyperChorus::new(sr);
        let mut p = HyperChorusParams::default();
        p.enabled = true;
        p.mix = 1.0;
        p.width = 0.0;
        for &x in input.iter().skip(12_000) {
            let (l, r) = fx.process(x, x, &p);
            assert!((l - r).abs() < 1e-6, "bredd 0 ska vara monokompatibel: {} vs {}", l, r);
        }
    }
}
