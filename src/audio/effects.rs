const MAX_DELAY_SAMPLES: usize = 96000; // ~2 seconds at 48kHz

#[derive(Debug, Clone, Copy)]
pub struct DelayParams {
    pub time_ms: f32,     // 20.0 .. 1000.0 ms
    pub feedback: f32,    // 0.0 .. 0.9
    pub mix: f32,         // 0.0 .. 1.0
}

impl Default for DelayParams {
    fn default() -> Self {
        Self {
            time_ms: 300.0,
            feedback: 0.45,
            mix: 0.25,
        }
    }
}

pub struct StereoDelay {
    pub sample_rate: f32,
    pub buffer_l: Vec<f32>,
    pub buffer_r: Vec<f32>,
    pub write_pos: usize,
}

impl StereoDelay {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            buffer_l: vec![0.0; MAX_DELAY_SAMPLES],
            buffer_r: vec![0.0; MAX_DELAY_SAMPLES],
            write_pos: 0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, in_l: f32, in_r: f32, params: &DelayParams) -> (f32, f32) {
        if params.mix <= 0.001 {
            return (in_l, in_r);
        }

        let delay_samples = ((params.time_ms * 0.001 * self.sample_rate) as usize).clamp(1, MAX_DELAY_SAMPLES - 1);
        let read_pos = (self.write_pos + MAX_DELAY_SAMPLES - delay_samples) % MAX_DELAY_SAMPLES;
        let read_pos_r = (self.write_pos + MAX_DELAY_SAMPLES - (delay_samples * 3 / 4)) % MAX_DELAY_SAMPLES;

        let delayed_l = self.buffer_l[read_pos];
        let delayed_r = self.buffer_r[read_pos_r];

        // Ping-pong feedback
        self.buffer_l[self.write_pos] = in_l + delayed_r * params.feedback;
        self.buffer_r[self.write_pos] = in_r + delayed_l * params.feedback;

        self.write_pos = (self.write_pos + 1) % MAX_DELAY_SAMPLES;

        let out_l = in_l * (1.0 - params.mix) + delayed_l * params.mix;
        let out_r = in_r * (1.0 - params.mix) + delayed_r * params.mix;

        (out_l, out_r)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReverbParams {
    pub room_size: f32,   // 0.0 .. 1.0
    pub damping: f32,     // 0.0 .. 1.0
    pub mix: f32,         // 0.0 .. 1.0
}

impl Default for ReverbParams {
    fn default() -> Self {
        Self {
            room_size: 0.75,
            damping: 0.35,
            mix: 0.2,
        }
    }
}

// Schroeder Reverb (Comb filters + All-pass filters)
pub struct SimpleReverb {
    comb_delays_l: [Vec<f32>; 4],
    comb_pos_l: [usize; 4],
    comb_filter_l: [f32; 4],
    allpass_delays_l: [Vec<f32>; 2],
    allpass_pos_l: [usize; 2],
}

impl SimpleReverb {
    pub fn new(sample_rate: f32) -> Self {
        // Schroeder comb/all-pass lengths are tuned for 44.1 kHz; scale them so
        // the room size stays consistent at any engine sample rate.
        let scale = (sample_rate / 44100.0).max(0.1);
        let comb_lens = [
            (1116.0 * scale).round().max(1.0) as usize,
            (1188.0 * scale).round().max(1.0) as usize,
            (1277.0 * scale).round().max(1.0) as usize,
            (1356.0 * scale).round().max(1.0) as usize,
        ];
        let allpass_lens = [
            (225.0 * scale).round().max(1.0) as usize,
            (556.0 * scale).round().max(1.0) as usize,
        ];

        let comb_delays_l = [
            vec![0.0; comb_lens[0]],
            vec![0.0; comb_lens[1]],
            vec![0.0; comb_lens[2]],
            vec![0.0; comb_lens[3]],
        ];
        let allpass_delays_l = [
            vec![0.0; allpass_lens[0]],
            vec![0.0; allpass_lens[1]],
        ];

        Self {
            comb_delays_l,
            comb_pos_l: [0; 4],
            comb_filter_l: [0.0; 4],
            allpass_delays_l,
            allpass_pos_l: [0; 2],
        }
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32, params: &ReverbParams) -> f32 {
        if params.mix <= 0.001 {
            return input;
        }

        let feedback = params.room_size.clamp(0.2, 0.95);
        let damp = params.damping.clamp(0.05, 0.9);

        let mut comb_sum = 0.0;
        for i in 0..4 {
            let len = self.comb_delays_l[i].len();
            let pos = self.comb_pos_l[i];
            let out = self.comb_delays_l[i][pos];
            self.comb_filter_l[i] = out * (1.0 - damp) + self.comb_filter_l[i] * damp;
            self.comb_delays_l[i][pos] = input + self.comb_filter_l[i] * feedback;
            self.comb_pos_l[i] = (pos + 1) % len;
            comb_sum += out;
        }

        let mut ap_out = comb_sum * 0.25;
        for i in 0..2 {
            let len = self.allpass_delays_l[i].len();
            let pos = self.allpass_pos_l[i];
            let buf_out = self.allpass_delays_l[i][pos];
            let new_buf = ap_out + buf_out * 0.5;
            self.allpass_delays_l[i][pos] = new_buf;
            self.allpass_pos_l[i] = (pos + 1) % len;
            ap_out = -ap_out * 0.5 + buf_out;
        }

        input * (1.0 - params.mix) + ap_out * params.mix
    }
}
