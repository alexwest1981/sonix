use std::collections::VecDeque;

use eframe::egui::{Color32, Pos2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    MidiIn,
    Oscillator,
    Filter,
    Envelope,
    Lfo,
    Delay,
    Reverb,
    Distortion,
    AudioOut,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PatcherNode {
    pub id: usize,
    pub title: String,
    pub node_type: NodeType,
    pub pos: Pos2,
    pub color: Color32,
    pub inputs: Vec<&'static str>,
    pub outputs: Vec<&'static str>,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatchCable {
    pub from_node: usize,
    pub from_pin: usize,
    pub to_node: usize,
    pub to_pin: usize,
    pub color: Color32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ModularGraph {
    pub nodes: Vec<PatcherNode>,
    pub cables: Vec<PatchCable>,
    pub next_id: usize,
    pub selected_node: Option<usize>,
    pub connecting_from: Option<(usize, usize)>, // (node_id, pin_idx)
}

impl Default for ModularGraph {
    fn default() -> Self {
        let mut graph = Self {
            nodes: Vec::new(),
            cables: Vec::new(),
            next_id: 1,
            selected_node: None,
            connecting_from: None,
        };
        graph.load_default_preset();
        graph
    }
}

impl ModularGraph {
    pub fn load_default_preset(&mut self) {
        self.nodes.clear();
        self.cables.clear();

        // 1. MIDI In
        self.nodes.push(PatcherNode {
            id: 1,
            title: "🎹 MIDI / Note IN".to_string(),
            node_type: NodeType::MidiIn,
            pos: Pos2::new(25.0, 25.0),
            color: Color32::from_rgb(255, 120, 60),
            inputs: vec![],
            outputs: vec!["Pitch", "Gate", "Velocity"],
            param1: 60.0,
            param2: 1.0,
            param3: 0.0,
        });

        // 2. LFO Modulator
        self.nodes.push(PatcherNode {
            id: 2,
            title: "🌀 LFO Modulator".to_string(),
            node_type: NodeType::Lfo,
            pos: Pos2::new(25.0, 195.0),
            color: Color32::from_rgb(180, 100, 255),
            inputs: vec!["Rate CV"],
            outputs: vec!["LFO Out"],
            param1: 2.5, // 2.5 Hz
            param2: 0.75, // Depth
            param3: 0.0,
        });

        // 3. Oscillator
        self.nodes.push(PatcherNode {
            id: 3,
            title: "🔊 Oscillator".to_string(),
            node_type: NodeType::Oscillator,
            pos: Pos2::new(215.0, 25.0),
            color: Color32::from_rgb(0, 200, 240),
            inputs: vec!["Pitch", "Sync", "PWM"],
            outputs: vec!["Audio Out"],
            param1: 1.0, // Saw
            param2: 0.0, // Fine tune
            param3: 0.5,
        });

        // 4. ADSR Envelope
        self.nodes.push(PatcherNode {
            id: 4,
            title: "📈 ADSR Envelope".to_string(),
            node_type: NodeType::Envelope,
            pos: Pos2::new(215.0, 195.0),
            color: Color32::from_rgb(255, 200, 40),
            inputs: vec!["Gate"],
            outputs: vec!["Env Out"],
            param1: 0.01, // Attack
            param2: 0.25, // Decay
            param3: 0.60, // Sustain
        });

        // 5. State Variable Filter (SVF)
        self.nodes.push(PatcherNode {
            id: 5,
            title: "🌊 SVF Filter (12 dB)".to_string(),
            node_type: NodeType::Filter,
            pos: Pos2::new(405.0, 25.0),
            color: Color32::from_rgb(46, 204, 113),
            inputs: vec!["Audio In", "Cutoff CV", "Res CV"],
            outputs: vec!["Lowpass", "Highpass", "Bandpass"],
            param1: 3200.0, // Cutoff
            param2: 3.5,    // Res
            param3: 1.0,
        });

        // 6. Tube Distortion
        self.nodes.push(PatcherNode {
            id: 6,
            title: "🔥 Tube-style Drive".to_string(),
            node_type: NodeType::Distortion,
            pos: Pos2::new(595.0, 25.0),
            color: Color32::from_rgb(255, 80, 50),
            inputs: vec!["Audio In", "Drive CV"],
            outputs: vec!["Wet Audio"],
            param1: 2.8,
            param2: 0.85,
            param3: 0.0,
        });

        // 7. Space Reverb
        self.nodes.push(PatcherNode {
            id: 7,
            title: "✨ Space Reverb".to_string(),
            node_type: NodeType::Reverb,
            pos: Pos2::new(595.0, 195.0),
            color: Color32::from_rgb(160, 120, 255),
            inputs: vec!["Audio In", "Mix CV"],
            outputs: vec!["Wet Out L", "Wet Out R"],
            param1: 0.85, // Room Size
            param2: 0.35, // Mix
            param3: 0.40, // Damp
        });

        // 8. Master Audio Output
        self.nodes.push(PatcherNode {
            id: 8,
            title: "🎚 Master Audio OUT".to_string(),
            node_type: NodeType::AudioOut,
            pos: Pos2::new(795.0, 95.0),
            color: Color32::from_rgb(255, 140, 0),
            inputs: vec!["Left In", "Right In"],
            outputs: vec![],
            param1: 0.85, // Volume
            param2: 0.0,  // Pan
            param3: 0.0,
        });

        self.next_id = 9;

        // Connect Default Patch Cables
        // MIDI Pitch -> OSC Pitch
        self.cables.push(PatchCable { from_node: 1, from_pin: 0, to_node: 3, to_pin: 0, color: Color32::from_rgb(0, 220, 255) });
        // MIDI Gate -> ADSR Gate
        self.cables.push(PatchCable { from_node: 1, from_pin: 1, to_node: 4, to_pin: 0, color: Color32::from_rgb(255, 200, 40) });
        // OSC Audio -> Filter Audio In
        self.cables.push(PatchCable { from_node: 3, from_pin: 0, to_node: 5, to_pin: 0, color: Color32::from_rgb(0, 200, 240) });
        // LFO Out -> Filter Cutoff CV
        self.cables.push(PatchCable { from_node: 2, from_pin: 0, to_node: 5, to_pin: 1, color: Color32::from_rgb(180, 100, 255) });
        // Filter Lowpass -> Tube Distortion Audio In
        self.cables.push(PatchCable { from_node: 5, from_pin: 0, to_node: 6, to_pin: 0, color: Color32::from_rgb(46, 204, 113) });
        // Tube Audio -> Reverb Audio In
        self.cables.push(PatchCable { from_node: 6, from_pin: 0, to_node: 7, to_pin: 0, color: Color32::from_rgb(255, 100, 60) });
        // Reverb Out L/R -> Master In L/R
        self.cables.push(PatchCable { from_node: 7, from_pin: 0, to_node: 8, to_pin: 0, color: Color32::from_rgb(160, 140, 255) });
        self.cables.push(PatchCable { from_node: 7, from_pin: 1, to_node: 8, to_pin: 1, color: Color32::from_rgb(160, 140, 255) });
    }

    pub fn add_node(&mut self, node_type: NodeType, pos: Pos2) {
        let (title, color, in_pins, out_pins, p1, p2, p3) = match node_type {
            NodeType::MidiIn => ("🎹 MIDI / Note IN", Color32::from_rgb(255, 120, 60), vec![], vec!["Pitch", "Gate", "Velocity"], 60.0, 1.0, 0.0),
            NodeType::Oscillator => ("🔊 Oscillator", Color32::from_rgb(0, 200, 240), vec!["Pitch", "Sync", "PWM"], vec!["Audio Out"], 1.0, 0.0, 0.5),
            NodeType::Filter => ("🌊 SVF Filter (12 dB)", Color32::from_rgb(46, 204, 113), vec!["Audio In", "Cutoff CV", "Res CV"], vec!["Lowpass", "Highpass", "Bandpass"], 3000.0, 2.5, 1.0),
            NodeType::Envelope => ("📈 ADSR Envelope", Color32::from_rgb(255, 200, 40), vec!["Gate"], vec!["Env Out"], 0.01, 0.25, 0.60),
            NodeType::Lfo => ("🌀 LFO Modulator", Color32::from_rgb(180, 100, 255), vec!["Rate CV"], vec!["LFO Out"], 2.0, 0.8, 0.0),
            NodeType::Delay => ("🌊 Delay", Color32::from_rgb(0, 230, 220), vec!["Audio In", "Time CV"], vec!["Wet Out"], 320.0, 0.45, 0.3),
            NodeType::Reverb => ("✨ Space Reverb", Color32::from_rgb(160, 120, 255), vec!["Audio In", "Mix CV"], vec!["Wet Out L", "Wet Out R"], 0.8, 0.35, 0.4),
            NodeType::Distortion => ("🔥 Tube Saturation", Color32::from_rgb(255, 80, 50), vec!["Audio In", "Drive CV"], vec!["Wet Audio"], 2.5, 0.8, 0.0),
            NodeType::AudioOut => ("🎚 Master Audio OUT", Color32::from_rgb(255, 140, 0), vec!["Left In", "Right In"], vec![], 0.85, 0.0, 0.0),
        };

        let node = PatcherNode {
            id: self.next_id,
            title: title.to_string(),
            node_type,
            pos,
            color,
            inputs: in_pins,
            outputs: out_pins,
            param1: p1,
            param2: p2,
            param3: p3,
        };
        self.next_id += 1;
        self.nodes.push(node);
    }
}

// ---------------------------------------------------------------------------
// Real-time DSP graph (used by the audio engine)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct PatchNodeSpec {
    pub id: usize,
    pub node_type: NodeType,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PatchSpec {
    pub nodes: Vec<PatchNodeSpec>,
    pub cables: Vec<(usize, usize, usize, usize)>, // from_node, from_pin, to_node, to_pin
}

impl ModularGraph {
    /// Flattens the UI graph into a spec that can be sent to the audio thread.
    pub fn to_spec(&self) -> PatchSpec {
        PatchSpec {
            nodes: self
                .nodes
                .iter()
                .map(|n| PatchNodeSpec {
                    id: n.id,
                    node_type: n.node_type,
                    param1: n.param1,
                    param2: n.param2,
                    param3: n.param3,
                })
                .collect(),
            cables: self
                .cables
                .iter()
                .map(|c| (c.from_node, c.from_pin, c.to_node, c.to_pin))
                .collect(),
        }
    }

    /// True when the patch cables form a feedback loop.
    pub fn has_cycle(&self) -> bool {
        let ids: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        let cables: Vec<(usize, usize, usize, usize)> = self
            .cables
            .iter()
            .map(|c| (c.from_node, c.from_pin, c.to_node, c.to_pin))
            .collect();
        topo_order(&ids, &cables).1
    }
}

fn node_io(node_type: NodeType) -> (usize, usize) {
    match node_type {
        NodeType::MidiIn => (0, 3),
        NodeType::Oscillator => (3, 1),
        NodeType::Filter => (3, 3),
        NodeType::Envelope => (1, 1),
        NodeType::Lfo => (1, 1),
        NodeType::Delay => (2, 1),
        NodeType::Reverb => (2, 2),
        NodeType::Distortion => (2, 1),
        NodeType::AudioOut => (2, 0),
    }
}

/// Kahn topological sort over the patch graph.
///
/// Returns the evaluation order as indices into `node_ids` plus a flag telling
/// whether a cycle was found. Cables may therefore be drawn in any order: an
/// upstream node is always evaluated before the nodes that read from it. If the
/// graph contains a feedback loop, the nodes that cannot be ordered are appended
/// in their original order so they still run (using the previous sample's output
/// for the feedback path).
fn topo_order(node_ids: &[usize], cables: &[(usize, usize, usize, usize)]) -> (Vec<usize>, bool) {
    let n = node_ids.len();
    let index_of = |id: usize| node_ids.iter().position(|x| *x == id);

    let mut indegree = vec![0usize; n];
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (from_node, _from_pin, to_node, _to_pin) in cables {
        if let (Some(from), Some(to)) = (index_of(*from_node), index_of(*to_node)) {
            adjacency[from].push(to);
            indegree[to] += 1;
        }
    }

    let mut queue: VecDeque<usize> = (0..n).filter(|i| indegree[*i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &next in &adjacency[i] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    let has_cycle = order.len() < n;
    if has_cycle {
        let mut placed = vec![false; n];
        for &i in &order {
            placed[i] = true;
        }
        for (i, was_placed) in placed.iter().enumerate() {
            if !was_placed {
                order.push(i);
            }
        }
    }

    (order, has_cycle)
}

struct RtNode {
    node_type: NodeType,
    p1: f32,
    p2: f32,
    p3: f32,
    inputs: Vec<Option<(usize, usize)>>,
    phase: f32,
    lfo_phase: f32,
    prev_sync: f32,
    svf_low: f32,
    svf_band: f32,
    env_stage: u8,
    env_level: f32,
    delay: Vec<f32>,
    delay_w: usize,
    rev: Vec<f32>,
    rev_w: usize,
}

impl RtNode {
    fn new(spec: &PatchNodeSpec, sample_rate: f32) -> Self {
        let (in_count, _) = node_io(spec.node_type);
        Self {
            node_type: spec.node_type,
            p1: spec.param1,
            p2: spec.param2,
            p3: spec.param3,
            inputs: vec![None; in_count],
            phase: 0.0,
            lfo_phase: 0.0,
            prev_sync: 0.0,
            svf_low: 0.0,
            svf_band: 0.0,
            env_stage: 0,
            env_level: 0.0,
            delay: if spec.node_type == NodeType::Delay {
                vec![0.0; (sample_rate * 1.2) as usize]
            } else {
                Vec::new()
            },
            delay_w: 0,
            rev: if spec.node_type == NodeType::Reverb {
                vec![0.0; (sample_rate * 0.4) as usize]
            } else {
                Vec::new()
            },
            rev_w: 0,
        }
    }
}

fn read_input(
    outs: &[f32],
    offsets: &[usize],
    nodes: &[RtNode],
    node_idx: usize,
    pin: usize,
) -> f32 {
    match nodes[node_idx].inputs.get(pin).and_then(|o| *o) {
        Some((src, sp)) => outs.get(offsets[src] + sp).copied().unwrap_or(0.0),
        None => 0.0,
    }
}

/// Evaluates a `PatchSpec` sample by sample and produces stereo audio.
pub struct PatchProcessor {
    nodes: Vec<RtNode>,
    offsets: Vec<usize>,
    outs: Vec<f32>,
    order: Vec<usize>,
    sample_rate: f32,
    note_freq: f32,
    gate: bool,
    velocity: f32,
    pub last_l: f32,
    pub last_r: f32,
}

impl PatchProcessor {
    pub fn new(spec: &PatchSpec, sample_rate: f32) -> Self {
        let nodes: Vec<RtNode> = spec.nodes.iter().map(|n| RtNode::new(n, sample_rate)).collect();
        let mut offsets = Vec::with_capacity(nodes.len());
        let mut total = 0usize;
        for n in &nodes {
            offsets.push(total);
            let (_, out_count) = node_io(n.node_type);
            total += out_count;
        }
        let node_ids: Vec<usize> = spec.nodes.iter().map(|n| n.id).collect();
        let (order, _has_cycle) = topo_order(&node_ids, &spec.cables);
        let id_index = |id: usize| spec.nodes.iter().position(|n| n.id == id);
        let mut processor = Self {
            nodes,
            offsets,
            outs: vec![0.0; total],
            order,
            sample_rate,
            note_freq: 220.0,
            gate: false,
            velocity: 0.8,
            last_l: 0.0,
            last_r: 0.0,
        };
        // Wire cables (needs indices resolved against the id list).
        for (from_node, from_pin, to_node, to_pin) in &spec.cables {
            let fi = match id_index(*from_node) {
                Some(i) => i,
                None => continue,
            };
            let ti = match id_index(*to_node) {
                Some(i) => i,
                None => continue,
            };
            if let Some(node) = processor.nodes.get_mut(ti)
                && *to_pin < node.inputs.len()
            {
                node.inputs[*to_pin] = Some((fi, *from_pin));
            }
        }
        processor
    }

    pub fn note_on(&mut self, freq: f32, velocity: f32) {
        self.note_freq = freq;
        self.velocity = velocity;
        self.gate = true;
    }

    pub fn note_off(&mut self) {
        self.gate = false;
    }

    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        self.gate
            || self
                .nodes
                .iter()
                .any(|n| n.node_type == NodeType::Envelope && n.env_level > 0.0005)
    }

    pub fn process(&mut self) -> (f32, f32) {
        let sr = self.sample_rate;
        let mut acc_l = 0.0f32;
        let mut acc_r = 0.0f32;

        {
            let Self {
                nodes,
                offsets,
                outs,
                order,
                note_freq,
                gate,
                velocity,
                ..
            } = self;
            let note_freq = *note_freq;
            let gate = *gate;
            let velocity = *velocity;

            for &i in order.iter() {
                let base = offsets[i];
                let (_, out_count) = node_io(nodes[i].node_type);
                match nodes[i].node_type {
                    NodeType::MidiIn => {
                        if out_count >= 3 {
                            outs[base] = note_freq;
                            outs[base + 1] = if gate { 1.0 } else { 0.0 };
                            outs[base + 2] = velocity;
                        }
                    }
                    NodeType::Lfo => {
                        let cv = read_input(outs, offsets, nodes, i, 0);
                        let rate = (nodes[i].p1 * (1.0 + cv)).clamp(0.01, 40.0);
                        nodes[i].lfo_phase = (nodes[i].lfo_phase + rate / sr).fract();
                        outs[base] = (nodes[i].lfo_phase * std::f32::consts::TAU).sin() * nodes[i].p2;
                    }
                    NodeType::Oscillator => {
                        let pitch = read_input(outs, offsets, nodes, i, 0);
                        let sync = read_input(outs, offsets, nodes, i, 1);
                        let pwm = read_input(outs, offsets, nodes, i, 2);
                        // Hard-sync the phase on a rising edge of the Sync input.
                        if sync > 0.5 && nodes[i].prev_sync <= 0.5 {
                            nodes[i].phase = 0.0;
                        }
                        nodes[i].prev_sync = sync;
                        let freq = if pitch > 1.0 { pitch } else { 220.0 };
                        nodes[i].phase = (nodes[i].phase + freq / sr).fract();
                        let ph = nodes[i].phase;
                        let pulse_width = (nodes[i].p3 + pwm).clamp(0.05, 0.95);
                        let v = match (nodes[i].p1.round() as i32).rem_euclid(4) {
                            0 => (ph * std::f32::consts::TAU).sin(),
                            1 => 2.0 * ph - 1.0,
                            2 => {
                                if ph < pulse_width {
                                    1.0
                                } else {
                                    -1.0
                                }
                            }
                            _ => 4.0 * (ph - 0.5).abs() - 1.0,
                        };
                        outs[base] = v;
                    }
                    NodeType::Envelope => {
                        let g = read_input(outs, offsets, nodes, i, 0) > 0.5;
                        let a = nodes[i].p1.max(0.001);
                        let d = nodes[i].p2.max(0.001);
                        let s = nodes[i].p3.clamp(0.0, 1.0);
                        if g && nodes[i].env_stage == 0 {
                            nodes[i].env_stage = 1;
                        }
                        match nodes[i].env_stage {
                            1 => {
                                nodes[i].env_level += 1.0 / (a * sr);
                                if nodes[i].env_level >= 1.0 {
                                    nodes[i].env_level = 1.0;
                                    nodes[i].env_stage = 2;
                                }
                            }
                            2 => {
                                if g {
                                    nodes[i].env_level += (s - nodes[i].env_level) * (1.0 / (d * sr)).min(1.0);
                                } else {
                                    nodes[i].env_stage = 3;
                                }
                            }
                            3 => {
                                nodes[i].env_level -= 1.0 / (0.2 * sr);
                                if nodes[i].env_level <= 0.0 {
                                    nodes[i].env_level = 0.0;
                                    nodes[i].env_stage = 0;
                                }
                            }
                            _ => {}
                        }
                        outs[base] = nodes[i].env_level;
                    }
                    NodeType::Filter => {
                        let x = read_input(outs, offsets, nodes, i, 0);
                        let cv = read_input(outs, offsets, nodes, i, 1);
                        let rcv = read_input(outs, offsets, nodes, i, 2);
                        let cutoff = (nodes[i].p1 * (1.0 + cv)).clamp(20.0, sr * 0.45);
                        let q = (nodes[i].p2 + rcv * 4.0).clamp(0.5, 20.0);
                        let f = (2.0 * (std::f32::consts::PI * cutoff / sr).sin()).clamp(0.001, 0.99);
                        let high = x - nodes[i].svf_low - (1.0 / q) * nodes[i].svf_band;
                        let band = nodes[i].svf_band + f * high;
                        let low = nodes[i].svf_low + f * band;
                        nodes[i].svf_low = low;
                        nodes[i].svf_band = band;
                        outs[base] = low;
                        outs[base + 1] = high;
                        outs[base + 2] = band;
                    }
                    NodeType::Distortion => {
                        let x = read_input(outs, offsets, nodes, i, 0);
                        let cv = read_input(outs, offsets, nodes, i, 1);
                        let drive = 1.0 + nodes[i].p1 + cv * 4.0;
                        outs[base] = (x * drive).tanh() * 0.8;
                    }
                    NodeType::Delay => {
                        let x = read_input(outs, offsets, nodes, i, 0);
                        let cv = read_input(outs, offsets, nodes, i, 1);
                        let len = nodes[i].delay.len();
                        if len > 1 {
                            let time_ms = (nodes[i].p1 * (1.0 + cv)).clamp(1.0, 1000.0);
                            let d = ((time_ms / 1000.0 * sr) as usize).clamp(1, len - 1);
                            let r = (nodes[i].delay_w + len - d) % len;
                            let delayed = nodes[i].delay[r];
                            let out = x + delayed * nodes[i].p2.clamp(0.0, 0.95);
                            let w = nodes[i].delay_w;
                            nodes[i].delay[w] = out;
                            nodes[i].delay_w = (w + 1) % len;
                            outs[base] = out;
                        } else {
                            outs[base] = x;
                        }
                    }
                    NodeType::Reverb => {
                        let x = read_input(outs, offsets, nodes, i, 0);
                        let mcv = read_input(outs, offsets, nodes, i, 1);
                        let len = nodes[i].rev.len();
                        if len > 1 {
                            let mix = (nodes[i].p2 + mcv).clamp(0.0, 1.0);
                            let d1 = ((sr * 0.089) as usize).clamp(1, len - 1);
                            let d2 = ((sr * 0.113) as usize).clamp(1, len - 1);
                            let w = nodes[i].rev_w;
                            let r1 = (w + len - d1) % len;
                            let r2 = (w + len - d2) % len;
                            let dl = nodes[i].rev[r1];
                            let dr = nodes[i].rev[r2];
                            nodes[i].rev[w] = x + (dl + dr) * 0.5 * nodes[i].p1.clamp(0.0, 0.95);
                            nodes[i].rev_w = (w + 1) % len;
                            outs[base] = x * (1.0 - mix) + dl * mix;
                            outs[base + 1] = x * (1.0 - mix) + dr * mix;
                        } else {
                            outs[base] = x;
                            outs[base + 1] = x;
                        }
                    }
                    NodeType::AudioOut => {
                        let l = read_input(outs, offsets, nodes, i, 0);
                        let r = read_input(outs, offsets, nodes, i, 1);
                        let pan = nodes[i].p2.clamp(-1.0, 1.0);
                        let ang = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
                        acc_l += l * nodes[i].p1 * ang.cos();
                        acc_r += r * nodes[i].p1 * ang.sin();
                    }
                }
            }
        }

        self.last_l = acc_l;
        self.last_r = acc_r;
        (acc_l, acc_r)
    }
}

#[cfg(test)]
mod dsp_tests {
    use super::*;

    fn spec_osc_to_out() -> PatchSpec {
        PatchSpec {
            nodes: vec![
                PatchNodeSpec { id: 1, node_type: NodeType::MidiIn, param1: 60.0, param2: 1.0, param3: 0.0 },
                PatchNodeSpec { id: 2, node_type: NodeType::Oscillator, param1: 1.0, param2: 0.0, param3: 0.0 },
                PatchNodeSpec { id: 3, node_type: NodeType::Envelope, param1: 0.01, param2: 0.2, param3: 0.8 },
                PatchNodeSpec { id: 4, node_type: NodeType::Filter, param1: 4000.0, param2: 2.0, param3: 0.0 },
                PatchNodeSpec { id: 5, node_type: NodeType::AudioOut, param1: 0.8, param2: 0.0, param3: 0.0 },
            ],
            cables: vec![
                (1, 0, 2, 0), // pitch -> osc
                (1, 1, 3, 0), // gate -> env
                (2, 0, 4, 0), // osc -> filter
                (4, 0, 5, 0), // filter lp -> out L
                (4, 0, 5, 1), // filter lp -> out R
            ],
        }
    }

    #[test]
    fn processor_renders_audible_signal() {
        let mut p = PatchProcessor::new(&spec_osc_to_out(), 44100.0);
        p.note_on(220.0, 1.0);
        let mut peak = 0.0f32;
        for _ in 0..44100 {
            let (l, r) = p.process();
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs());
        }
        assert!(peak > 0.05, "patcher produced near-silence (peak {})", peak);
    }

    #[test]
    fn envelope_opens_and_releases() {
        let spec = PatchSpec {
            nodes: vec![
                PatchNodeSpec { id: 1, node_type: NodeType::MidiIn, param1: 60.0, param2: 1.0, param3: 0.0 },
                PatchNodeSpec { id: 2, node_type: NodeType::Envelope, param1: 0.01, param2: 0.1, param3: 0.5 },
                PatchNodeSpec { id: 3, node_type: NodeType::AudioOut, param1: 1.0, param2: 0.0, param3: 0.0 },
            ],
            cables: vec![(1, 1, 2, 0), (2, 0, 3, 0), (2, 0, 3, 1)],
        };
        let mut p = PatchProcessor::new(&spec, 44100.0);
        p.note_on(220.0, 1.0);
        for _ in 0..2000 {
            p.process();
        }
        assert!(p.is_active());
        p.note_off();
        for _ in 0..44100 {
            p.process();
        }
        assert!(!p.is_active());
    }

    fn run_spec(spec: &PatchSpec, samples: usize) -> Vec<(f32, f32)> {
        let mut p = PatchProcessor::new(spec, 44100.0);
        p.note_on(220.0, 1.0);
        (0..samples).map(|_| p.process()).collect()
    }

    #[test]
    fn topo_order_places_sources_before_targets() {
        let ids = vec![5usize, 4, 3, 2, 1];
        let cables = vec![(1, 0, 2, 0), (1, 1, 3, 0), (2, 0, 4, 0), (4, 0, 5, 0), (4, 0, 5, 1)];
        let (order, has_cycle) = topo_order(&ids, &cables);
        assert!(!has_cycle);
        let pos = |id: usize| order.iter().position(|i| ids[*i] == id).unwrap();
        for (from, _fp, to, _tp) in &cables {
            assert!(pos(*from) < pos(*to), "source {from} must be evaluated before target {to}");
        }
    }

    #[test]
    fn topo_order_detects_cycle_and_keeps_all_nodes() {
        let ids = vec![1usize, 2];
        let cables = vec![(1, 0, 2, 0), (2, 0, 1, 0)];
        let (order, has_cycle) = topo_order(&ids, &cables);
        assert!(has_cycle);
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn evaluation_order_is_independent_of_node_listing_order() {
        let canonical = spec_osc_to_out();
        let mut reversed = canonical.clone();
        reversed.nodes.reverse();

        let out_a = run_spec(&canonical, 4410);
        let out_b = run_spec(&reversed, 4410);

        assert_eq!(out_a, out_b, "reversed node listing must not change the audio");
        assert!(out_a.iter().any(|(l, _)| l.abs() > 0.05), "patcher produced near-silence");
    }

    #[test]
    fn graph_reports_feedback_cycle() {
        assert!(!ModularGraph::default().has_cycle());
        let mut cyclic = ModularGraph::default();
        cyclic.cables.push(PatchCable {
            from_node: 3,
            from_pin: 0,
            to_node: 3,
            to_pin: 1,
            color: Color32::BLACK,
        });
        assert!(cyclic.has_cycle());
    }
}
