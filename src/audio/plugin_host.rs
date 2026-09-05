use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginFormat {
    Clap,
    Vst3,
    Lv2,
    WineYabridge,
}

impl PluginFormat {
    pub fn name(&self) -> &'static str {
        match self {
            PluginFormat::Clap => "CLAP (Native)",
            PluginFormat::Vst3 => "VST3 (Native Linux)",
            PluginFormat::Lv2 => "LV2 (Linux Audio)",
            PluginFormat::WineYabridge => "Windows VST3 (Yabridge/Wine)",
        }
    }

    pub fn badge_color(&self) -> Color32 {
        match self {
            PluginFormat::Clap => Color32::from_rgb(0, 220, 255), // Cyan
            PluginFormat::Vst3 => Color32::from_rgb(255, 140, 0), // Orange
            PluginFormat::Lv2 => Color32::from_rgb(46, 204, 113),  // Green
            PluginFormat::WineYabridge => Color32::from_rgb(180, 100, 255), // Purple Wine
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginCategory {
    Synth,
    Effect,
    Compressor,
    Equalizer,
    Reverb,
    Sampler,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub name: String,
    pub vendor: String,
    pub format: PluginFormat,
    pub category: PluginCategory,
    pub file_path: String,
    pub is_sandboxed: bool,
    pub is_loaded: bool,
    pub cpu_usage: f32,
    pub latency_samples: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PluginManager {
    pub plugins: Vec<PluginDescriptor>,
    pub yabridge_installed: bool,
    pub wine_version: String,
    pub sandboxing_enabled: bool,
    pub search_query: String,
    pub selected_format_filter: Option<PluginFormat>,
    pub last_scan_time: &'static str,
}

impl Default for PluginManager {
    fn default() -> Self {
        let mut mgr = Self {
            plugins: Vec::new(),
            yabridge_installed: true,
            wine_version: "Wine Staging 9.14 (Low-Latency PREEMPT_RT)".to_string(),
            sandboxing_enabled: true,
            search_query: String::new(),
            selected_format_filter: None,
            last_scan_time: "2026-09-05 17:40 (Auto-Scan)",
        };
        mgr.populate_known_plugins();
        mgr
    }
}

impl PluginManager {
    pub fn populate_known_plugins(&mut self) {
        self.plugins.clear();

        // 1. Native CLAP Plugins
        self.plugins.push(PluginDescriptor {
            name: "Surge XT".to_string(),
            vendor: "Surge Synth Team".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "/usr/lib/clap/Surge-XT.clap".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 1.8,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Vital Wavetable".to_string(),
            vendor: "Matt Tytel".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "~/.clap/Vital.clap".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 2.4,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Cardinal Modular (Eurorack)".to_string(),
            vendor: "DISTRHO".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "/usr/lib/clap/Cardinal.clap".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 3.1,
            latency_samples: 0,
        });

        // 2. Native VST3 Plugins
        self.plugins.push(PluginDescriptor {
            name: "u-he Diva".to_string(),
            vendor: "u-he".to_string(),
            format: PluginFormat::Vst3,
            category: PluginCategory::Synth,
            file_path: "~/.vst3/u-he/Diva.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 4.2,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Helm Polyphonic".to_string(),
            vendor: "Matt Tytel".to_string(),
            format: PluginFormat::Vst3,
            category: PluginCategory::Synth,
            file_path: "/usr/lib/vst3/Helm.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.2,
            latency_samples: 0,
        });

        // 3. Native LV2
        self.plugins.push(PluginDescriptor {
            name: "Calf Multiwave Chorus".to_string(),
            vendor: "Calf Studio Gear".to_string(),
            format: PluginFormat::Lv2,
            category: PluginCategory::Effect,
            file_path: "/usr/lib/lv2/calf.lv2".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 0.4,
            latency_samples: 0,
        });

        // 4. Windows Bridged via Yabridge + Wine (Zero Latency IPC)
        self.plugins.push(PluginDescriptor {
            name: "FabFilter Pro-Q 3".to_string(),
            vendor: "FabFilter (Windows Bridged)".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Equalizer,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/FabFilter Pro-Q 3.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 0.9,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Soundtoys Decapitator".to_string(),
            vendor: "Soundtoys (Windows Bridged)".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Effect,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/Decapitator.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.4,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Valhalla VintageVerb".to_string(),
            vendor: "Valhalla DSP (Windows Bridged)".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Reverb,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/ValhallaVintageVerb.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 1.1,
            latency_samples: 0,
        });

        self.plugins.push(PluginDescriptor {
            name: "Serum Wavetable Synthesizer".to_string(),
            vendor: "Xfer Records (Windows Bridged)".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Synth,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/Serum.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 3.5,
            latency_samples: 0,
        });
    }

    pub fn rescan(&mut self) {
        self.populate_known_plugins();
    }
}
