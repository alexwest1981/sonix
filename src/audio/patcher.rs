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

        // 3. Dual Oscillator
        self.nodes.push(PatcherNode {
            id: 3,
            title: "🔊 Dual Wavetable OSC".to_string(),
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
            title: "🌊 SVF 24dB Filter".to_string(),
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
            title: "🔥 Analog Tube Drive".to_string(),
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
            title: "✨ Shimmer Space Reverb".to_string(),
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
            NodeType::Oscillator => ("🔊 Wavetable OSC", Color32::from_rgb(0, 200, 240), vec!["Pitch", "Sync", "PWM"], vec!["Audio Out"], 1.0, 0.0, 0.5),
            NodeType::Filter => ("🌊 SVF 24dB Filter", Color32::from_rgb(46, 204, 113), vec!["Audio In", "Cutoff CV", "Res CV"], vec!["Lowpass", "Highpass", "Bandpass"], 3000.0, 2.5, 1.0),
            NodeType::Envelope => ("📈 ADSR Envelope", Color32::from_rgb(255, 200, 40), vec!["Gate"], vec!["Env Out"], 0.01, 0.25, 0.60),
            NodeType::Lfo => ("🌀 LFO Modulator", Color32::from_rgb(180, 100, 255), vec!["Rate CV"], vec!["LFO Out"], 2.0, 0.8, 0.0),
            NodeType::Delay => ("🌊 Stereo Delay", Color32::from_rgb(0, 230, 220), vec!["Audio In", "Time CV"], vec!["Wet Out"], 320.0, 0.45, 0.3),
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
