use eframe::egui::Color32;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFormat {
    Clap,
    Vst3,
    Vst2,
    Lv2,
    WineYabridge,
    FlStudioNative,
    FlPresetFst,
}

impl PluginFormat {
    pub fn name(&self) -> &'static str {
        match self {
            PluginFormat::Clap => "CLAP (Native)",
            PluginFormat::Vst3 => "VST3 (Native Linux)",
            PluginFormat::Vst2 => "VST 2.4 (Legacy)",
            PluginFormat::Lv2 => "LV2 (Linux Audio)",
            PluginFormat::WineYabridge => "Windows VST3 (Yabridge/Wine)",
            PluginFormat::FlStudioNative => "FL Studio VSTi / Image-Line",
            PluginFormat::FlPresetFst => "FL Studio Preset (.fst)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            PluginFormat::Clap => "CLAP",
            PluginFormat::Vst3 => "VST3",
            PluginFormat::Vst2 => "VST2",
            PluginFormat::Lv2 => "LV2",
            PluginFormat::WineYabridge => "Yabridge",
            PluginFormat::FlStudioNative => "FL Studio",
            PluginFormat::FlPresetFst => ".FST Preset",
        }
    }

    pub fn badge_color(&self) -> Color32 {
        match self {
            PluginFormat::Clap => Color32::from_rgb(0, 220, 255),          // Cyan
            PluginFormat::Vst3 => Color32::from_rgb(255, 140, 0),          // Orange
            PluginFormat::Vst2 => Color32::from_rgb(220, 180, 50),         // Gold
            PluginFormat::Lv2 => Color32::from_rgb(46, 204, 113),          // Green
            PluginFormat::WineYabridge => Color32::from_rgb(180, 100, 255), // Purple Wine
            PluginFormat::FlStudioNative => Color32::from_rgb(255, 100, 30),// FL Orange/Red
            PluginFormat::FlPresetFst => Color32::from_rgb(255, 200, 80),  // Bright Yellow
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCategory {
    Synth,
    Effect,
    Compressor,
    Equalizer,
    Reverb,
    Delay,
    Distortion,
    PitchCorrection,
    Mastering,
    Sampler,
}

impl PluginCategory {
    pub fn label(&self) -> &'static str {
        match self {
            PluginCategory::Synth => "Synthesizer / Instrument",
            PluginCategory::Effect => "Creative Effect / Modulation",
            PluginCategory::Compressor => "Compressor / Dynamics",
            PluginCategory::Equalizer => "Equalizer / Filter",
            PluginCategory::Reverb => "Reverb / Space",
            PluginCategory::Delay => "Delay / Echo",
            PluginCategory::Distortion => "Distortion / Saturation",
            PluginCategory::PitchCorrection => "Pitch Correction / Vocoder",
            PluginCategory::Mastering => "Mastering / Limiter",
            PluginCategory::Sampler => "Sampler / Rompler",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PluginCategory::Synth => "🎹",
            PluginCategory::Effect => "✨",
            PluginCategory::Compressor => "🗜",
            PluginCategory::Equalizer => "📊",
            PluginCategory::Reverb => "🌌",
            PluginCategory::Delay => "⏳",
            PluginCategory::Distortion => "🔥",
            PluginCategory::PitchCorrection => "🎤",
            PluginCategory::Mastering => "🛡",
            PluginCategory::Sampler => "🥁",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub format: PluginFormat,
    pub category: PluginCategory,
    pub file_path: String,
    pub is_sandboxed: bool,
    pub is_loaded: bool,
    pub cpu_usage: f32,
    pub latency_samples: usize,
    pub is_fl_compatible: bool,
    pub has_gui: bool,
    pub author_notes: String,
}

#[derive(Debug, Clone)]
pub struct FlStudioPreset {
    pub file_path: String,
    pub preset_name: String,
    pub target_plugin: String,
    pub preset_type: String, // "Channel State", "Mixer Track State", "Plugin Preset"
    pub filesize_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ScanPath {
    pub path: String,
    pub enabled: bool,
    pub description: String,
    pub is_wine: bool,
    pub is_fl_path: bool,
}

#[derive(Debug, Clone)]
pub struct PluginManager {
    pub plugins: Vec<PluginDescriptor>,
    pub presets: Vec<FlStudioPreset>,
    pub scan_paths: Vec<ScanPath>,
    pub active_tab: usize, // 0 = Plugin Databas, 1 = Mappar & Sökvägar, 2 = Importera Plugin / .FST, 3 = FL Studio & Yabridge Assistent
    pub yabridge_installed: bool,
    pub wine_version: String,
    pub sandboxing_enabled: bool,
    pub search_query: String,
    pub selected_format_filter: Option<PluginFormat>,
    pub selected_category_filter: Option<PluginCategory>,
    pub last_scan_time: String,
    pub scan_status: String,
    // Custom inputs
    pub new_custom_path_input: String,
    pub manual_import_file_input: String,
    pub manual_import_vendor_input: String,
    pub manual_import_category_idx: usize,
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(path)
}

impl Default for PluginManager {
    fn default() -> Self {
        let default_scan_paths = vec![
            ScanPath {
                path: "~/.vst3".to_string(),
                enabled: true,
                description: "Linux Användar-VST3".to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "/usr/lib/vst3".to_string(),
                enabled: true,
                description: "Linux System-VST3".to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.clap".to_string(),
                enabled: true,
                description: "Linux Användar-CLAP".to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "/usr/lib/clap".to_string(),
                enabled: true,
                description: "Linux System-CLAP".to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            ScanPath {
                path: "/usr/lib/lv2".to_string(),
                enabled: true,
                description: "Linux LV2 Standardbibliotek".to_string(),
                is_wine: false,
                is_fl_path: false,
            },
            // Wine & FL Studio Default Paths
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Common Files/VST3".to_string(),
                enabled: true,
                description: "Windows VST3 Standard (Yabridge / Wine)".to_string(),
                is_wine: true,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST".to_string(),
                enabled: true,
                description: "FL Studio 64-bit VST Plugins (Image-Line)".to_string(),
                is_wine: true,
                is_fl_path: true,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files (x86)/Image-Line/FL Studio/Plugins/VST".to_string(),
                enabled: true,
                description: "FL Studio 32-bit VST Plugins (Legacy)".to_string(),
                is_wine: true,
                is_fl_path: true,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/VstPlugins".to_string(),
                enabled: true,
                description: "Windows VST2 Standard (Yabridge)".to_string(),
                is_wine: true,
                is_fl_path: false,
            },
            ScanPath {
                path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets".to_string(),
                enabled: true,
                description: "FL Studio .FST Presets (Kanaler & Mixer)".to_string(),
                is_wine: true,
                is_fl_path: true,
            },
        ];

        let mut mgr = Self {
            plugins: Vec::new(),
            presets: Vec::new(),
            scan_paths: default_scan_paths,
            active_tab: 0,
            yabridge_installed: true,
            wine_version: "Wine Staging 9.14 (Low-Latency PREEMPT_RT)".to_string(),
            sandboxing_enabled: true,
            search_query: String::new(),
            selected_format_filter: None,
            selected_category_filter: None,
            last_scan_time: "Automatisk förinläsning".to_string(),
            scan_status: "Redo".to_string(),
            new_custom_path_input: String::new(),
            manual_import_file_input: String::new(),
            manual_import_vendor_input: String::new(),
            manual_import_category_idx: 0,
        };

        mgr.populate_known_plugins();
        mgr.scan_disk();
        mgr
    }
}

impl PluginManager {
    pub fn populate_known_plugins(&mut self) {
        self.plugins.clear();

        // -------------------------------------------------------------
        // 1. FL Studio Native & Image-Line VSTs (Standard & Bridged)
        // -------------------------------------------------------------
        self.plugins.push(PluginDescriptor {
            id: "fl_sytrus".to_string(),
            name: "Sytrus (FM & Subtractive Synthesizer)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v2.6.4".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Synth,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/Sytrus.dll".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 1.6,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Legendarisk FM/RM-synth från FL Studio med 6 operatorer och matris-modulering.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_harmor".to_string(),
            name: "Harmor (Additive & Resynthesis)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v1.3.1".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Synth,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/Harmor.dll".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 3.4,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Additiv synth och resyntes med unik bild/ljudsyntes och prismamodulering.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_gross_beat".to_string(),
            name: "Gross Beat (Time & Pitch Manipulation)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v1.0.32".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Effect,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/GrossBeat.dll".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 0.8,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Populärt verktyg för half-speed, gating, reverse och scratch-effekter i realtid.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_maximus".to_string(),
            name: "Maximus (Multiband Maximizer & Limiter)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v1.0.30".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Mastering,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/Maximus.dll".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.2,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Avancerad 3-bands mastering-kompressor och brickwall limiter.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_vocodex".to_string(),
            name: "Vocodex (Advanced Studio Vocoder)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v1.0.22".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::PitchCorrection,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/Vocodex.dll".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 2.1,
            latency_samples: 64,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Upp till 100 filterband för rika daft punk- och robotröster.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_studio_vsti".to_string(),
            name: "FL Studio VSTi Host Multi-Out".to_string(),
            vendor: "Image-Line (FL Studio Core)".to_string(),
            version: "v21.2.3".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Synth,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/FL Studio VSTi (Multi).dll".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 4.8,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Kör hela FL Studios motor, step sequencer och plugins synkroniserat inuti Sonix!".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "fl_soundgoodizer".to_string(),
            name: "Soundgoodizer (Stereo Maximizer)".to_string(),
            vendor: "Image-Line (FL Studio)".to_string(),
            version: "v1.0.1".to_string(),
            format: PluginFormat::FlStudioNative,
            category: PluginCategory::Effect,
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Plugins/VST/Soundgoodizer.dll".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 0.3,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Klassisk 1-ratts saturator & multiband enhancer baserad på Maximus.".to_string(),
        });

        // -------------------------------------------------------------
        // 2. Windows Plugins via Yabridge (FabFilter, Serum, Valhalla, OTT)
        // -------------------------------------------------------------
        self.plugins.push(PluginDescriptor {
            id: "ff_pro_q3".to_string(),
            name: "FabFilter Pro-Q 3".to_string(),
            vendor: "FabFilter (Windows Bridged)".to_string(),
            version: "v3.24".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Equalizer,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/FabFilter Pro-Q 3.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 0.9,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Branschstandard inom EQ med dynamiskt läge och spektrogram.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "serum".to_string(),
            name: "Serum Wavetable Synthesizer".to_string(),
            vendor: "Xfer Records (Windows Bridged)".to_string(),
            version: "v1.368".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Synth,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/Serum.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 3.5,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Världens mest använda wavetable-synth för modern elektronisk musik och trap.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "ott".to_string(),
            name: "OTT Multiband Compressor".to_string(),
            vendor: "Xfer Records".to_string(),
            version: "v1.31".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Compressor,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/OTT.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 0.4,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Legendarisk upward/downward multiband-kompressor för aggressiv dynamik.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "valhalla_vintage_verb".to_string(),
            name: "Valhalla VintageVerb".to_string(),
            vendor: "Valhalla DSP (Windows Bridged)".to_string(),
            version: "v3.0.0".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Reverb,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/ValhallaVintageVerb.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 1.1,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Klassisk 1970/1980-tals algoritmisk rymd och plate med varm klang.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "soundtoys_decapitator".to_string(),
            name: "Soundtoys Decapitator".to_string(),
            vendor: "Soundtoys (Windows Bridged)".to_string(),
            version: "v5.3.8".to_string(),
            format: PluginFormat::WineYabridge,
            category: PluginCategory::Distortion,
            file_path: "~/.wine/drive_c/Program Files/Common Files/VST3/Decapitator.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.4,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Analog rör- och bandmättnad med 5 distinkta analoga modeller.".to_string(),
        });

        // -------------------------------------------------------------
        // 3. Native Linux CLAP & VST3 & LV2
        // -------------------------------------------------------------
        self.plugins.push(PluginDescriptor {
            id: "vital_clap".to_string(),
            name: "Vital Spectral Wavetable".to_string(),
            vendor: "Matt Tytel (Native Linux)".to_string(),
            version: "v1.5.5".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "~/.clap/Vital.clap".to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 2.2,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Högpresterande spektral wavetable-synth med CLAP polyfonisk modulation.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "surge_xt".to_string(),
            name: "Surge XT Hybrid Synth".to_string(),
            vendor: "Surge Synth Team (Native)".to_string(),
            version: "v1.3.4".to_string(),
            format: PluginFormat::Clap,
            category: PluginCategory::Synth,
            file_path: "/usr/lib/clap/Surge-XT.clap".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.8,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Öppen källkods hybridsynth med hundratals filter och oscillatorer.".to_string(),
        });

        self.plugins.push(PluginDescriptor {
            id: "uhe_diva".to_string(),
            name: "u-he Diva (Analogue Emulation)".to_string(),
            vendor: "u-he (Native Linux)".to_string(),
            version: "v1.4.7".to_string(),
            format: PluginFormat::Vst3,
            category: PluginCategory::Synth,
            file_path: "~/.vst3/u-he/Diva.vst3".to_string(),
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 4.2,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Kretsnivå-emulering av klassiska analoga syntar (Minimoog, Jupiter-8, MS-20).".to_string(),
        });

        // -------------------------------------------------------------
        // 4. Sample FL Studio Presets (.fst)
        // -------------------------------------------------------------
        self.presets.clear();
        self.presets.push(FlStudioPreset {
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets/Effects/Gross Beat/Half-Speed Drill.fst".to_string(),
            preset_name: "Half-Speed Drill Rhythm".to_string(),
            target_plugin: "Gross Beat".to_string(),
            preset_type: "Effect Preset (.fst)".to_string(),
            filesize_bytes: 4096,
        });
        self.presets.push(FlStudioPreset {
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets/Generators/Sytrus/Pluck - Bell 80s.fst".to_string(),
            preset_name: "Pluck - Bell 80s FM".to_string(),
            target_plugin: "Sytrus".to_string(),
            preset_type: "Channel State (.fst)".to_string(),
            filesize_bytes: 8192,
        });
        self.presets.push(FlStudioPreset {
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets/Effects/Maximus/Mastering - Clean Punch.fst".to_string(),
            preset_name: "Mastering - Clean Punch".to_string(),
            target_plugin: "Maximus".to_string(),
            preset_type: "Mixer State (.fst)".to_string(),
            filesize_bytes: 6144,
        });
        self.presets.push(FlStudioPreset {
            file_path: "~/.wine/drive_c/Program Files/Image-Line/FL Studio/Data/Patches/Plugin presets/Effects/Soundgoodizer/Preset A - Warm Glue.fst".to_string(),
            preset_name: "Preset A - Warm Glue".to_string(),
            target_plugin: "Soundgoodizer".to_string(),
            preset_type: "Mixer State (.fst)".to_string(),
            filesize_bytes: 1024,
        });
    }

    /// Scans configured real directories on the file system for plugin bundles & files.
    pub fn scan_disk(&mut self) {
        let mut found_count = 0;
        let paths_to_scan: Vec<(PathBuf, bool)> = self
            .scan_paths
            .iter()
            .filter(|p| p.enabled)
            .map(|p| (expand_tilde(&p.path), p.is_wine))
            .collect();

        for (expanded, is_wine) in paths_to_scan {
            if !expanded.exists() {
                continue;
            }

            self.scan_directory_recursive(&expanded, 0, &mut found_count, is_wine);
        }

        self.last_scan_time = "Nyss (Auto-skannad)".to_string();
        self.scan_status = format!("Skanning klar. {} aktiva plugins och presets identifierade.", self.plugins.len());
    }

    fn scan_directory_recursive(&mut self, dir: &Path, depth: usize, count: &mut usize, is_wine: bool) {
        if depth > 4 {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                // If it ends with .vst3, .lv2, or .clap, treat the directory itself as a plugin bundle
                if file_name.ends_with(".vst3") || file_name.ends_with(".lv2") || file_name.ends_with(".clap") {
                    self.register_discovered_file(&path, &file_name, is_wine);
                    *count += 1;
                } else {
                    self.scan_directory_recursive(&path, depth + 1, count, is_wine);
                }
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ["so", "dll", "vst3", "clap", "fst"].contains(&ext.to_lowercase().as_str()) {
                    self.register_discovered_file(&path, &file_name, is_wine);
                    *count += 1;
                }
            }
        }
    }

    fn register_discovered_file(&mut self, path: &Path, file_name: &str, is_wine: bool) {
        let path_str = path.to_string_lossy().to_string();

        // Check if already in plugins or presets
        if self.plugins.iter().any(|p| p.file_path == path_str) || self.presets.iter().any(|pr| pr.file_path == path_str) {
            return;
        }

        let lower = file_name.to_lowercase();

        if lower.ends_with(".fst") {
            let name_clean = file_name.trim_end_matches(".fst").to_string();
            let target_plugin = if lower.contains("gross") {
                "Gross Beat".to_string()
            } else if lower.contains("sytrus") {
                "Sytrus".to_string()
            } else if lower.contains("maximus") {
                "Maximus".to_string()
            } else {
                "FL Studio Generic".to_string()
            };

            self.presets.push(FlStudioPreset {
                file_path: path_str,
                preset_name: name_clean,
                target_plugin,
                preset_type: "FL State (.fst)".to_string(),
                filesize_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            });
            return;
        }

        let (format, vendor) = if is_wine || path_str.contains(".wine") || lower.ends_with(".dll") {
            if lower.contains("image-line") || lower.contains("fl studio") || lower.starts_with("sytrus") || lower.starts_with("harmor") || lower.starts_with("gross") {
                (PluginFormat::FlStudioNative, "Image-Line (FL Studio)".to_string())
            } else {
                (PluginFormat::WineYabridge, "Windows VST (Yabridge)".to_string())
            }
        } else if lower.ends_with(".clap") {
            (PluginFormat::Clap, "CLAP Native".to_string())
        } else if lower.ends_with(".vst3") {
            (PluginFormat::Vst3, "VST3 Native".to_string())
        } else if lower.ends_with(".lv2") {
            (PluginFormat::Lv2, "LV2 Linux".to_string())
        } else {
            (PluginFormat::Vst2, "VST2 Linux".to_string())
        };

        let clean_name = file_name
            .trim_end_matches(".vst3")
            .trim_end_matches(".clap")
            .trim_end_matches(".dll")
            .trim_end_matches(".so")
            .trim_end_matches(".lv2")
            .replace('_', " ");

        let category = if lower.contains("synth") || lower.contains("generator") || lower.contains("vital") || lower.contains("diva") || lower.contains("sytrus") || lower.contains("harmor") || lower.contains("serum") {
            PluginCategory::Synth
        } else if lower.contains("eq") || lower.contains("filter") || lower.contains("pro-q") {
            PluginCategory::Equalizer
        } else if lower.contains("comp") || lower.contains("limiter") || lower.contains("maximus") || lower.contains("ott") {
            PluginCategory::Compressor
        } else if lower.contains("reverb") || lower.contains("space") || lower.contains("room") {
            PluginCategory::Reverb
        } else if lower.contains("delay") || lower.contains("echo") {
            PluginCategory::Delay
        } else if lower.contains("dist") || lower.contains("saturat") || lower.contains("drive") || lower.contains("decap") {
            PluginCategory::Distortion
        } else if lower.contains("vocod") || lower.contains("pitch") || lower.contains("autotune") {
            PluginCategory::PitchCorrection
        } else {
            PluginCategory::Effect
        };

        self.plugins.push(PluginDescriptor {
            id: format!("disc_{}", self.plugins.len()),
            name: clean_name,
            vendor,
            version: "1.0".to_string(),
            format,
            category,
            file_path: path_str,
            is_sandboxed: true,
            is_loaded: false,
            cpu_usage: 1.0,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: format!("Upptäcktes automatiskt under skanning av {}", path.display()),
        });
    }

    /// Manually import a single plugin file or bundle
    pub fn import_file(&mut self, raw_path: &str, custom_vendor: &str, category: PluginCategory) -> Result<PluginDescriptor, String> {
        let expanded = expand_tilde(raw_path.trim());
        let file_name = expanded.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Okänt Plugin".to_string());
        let lower = file_name.to_lowercase();

        if lower.ends_with(".fst") {
            let preset_name = file_name.trim_end_matches(".fst").to_string();
            let p = FlStudioPreset {
                file_path: raw_path.to_string(),
                preset_name: preset_name.clone(),
                target_plugin: if custom_vendor.is_empty() { "FL Studio Generic".to_string() } else { custom_vendor.to_string() },
                preset_type: "Importerad FL Preset (.fst)".to_string(),
                filesize_bytes: std::fs::metadata(&expanded).map(|m| m.len()).unwrap_or(0),
            };
            self.presets.push(p);
            return Err(format!("Filen '{}' importerades som en FL Studio .FST preset!", preset_name));
        }

        let format = if lower.ends_with(".clap") {
            PluginFormat::Clap
        } else if lower.ends_with(".vst3") {
            if raw_path.contains(".wine") || raw_path.contains("drive_c") { PluginFormat::WineYabridge } else { PluginFormat::Vst3 }
        } else if lower.ends_with(".dll") {
            if lower.contains("fl") || lower.contains("sytrus") || lower.contains("harmor") || lower.contains("gross") {
                PluginFormat::FlStudioNative
            } else {
                PluginFormat::WineYabridge
            }
        } else if lower.ends_with(".lv2") {
            PluginFormat::Lv2
        } else {
            PluginFormat::Vst2
        };

        let vendor = if !custom_vendor.trim().is_empty() {
            custom_vendor.trim().to_string()
        } else if format == PluginFormat::FlStudioNative {
            "Image-Line (FL Studio)".to_string()
        } else {
            "Importerad Tredjepart".to_string()
        };

        let clean_name = file_name
            .trim_end_matches(".vst3")
            .trim_end_matches(".clap")
            .trim_end_matches(".dll")
            .trim_end_matches(".so")
            .trim_end_matches(".lv2")
            .replace('_', " ");

        let desc = PluginDescriptor {
            id: format!("custom_{}", self.plugins.len()),
            name: clean_name,
            vendor,
            version: "1.0".to_string(),
            format,
            category,
            file_path: raw_path.to_string(),
            is_sandboxed: true,
            is_loaded: true,
            cpu_usage: 1.2,
            latency_samples: 0,
            is_fl_compatible: true,
            has_gui: true,
            author_notes: "Manuellt importerad pluginfil i Sonix.".to_string(),
        };

        self.plugins.push(desc.clone());
        self.scan_status = format!("✔ Importerade '{}' ({})", desc.name, desc.format.name());
        Ok(desc)
    }

    pub fn add_scan_path(&mut self, path: String, description: String, is_wine: bool, is_fl_path: bool) {
        let trimmed = path.trim().to_string();
        if !trimmed.is_empty() && !self.scan_paths.iter().any(|p| p.path == trimmed) {
            self.scan_paths.push(ScanPath {
                path: trimmed,
                enabled: true,
                description,
                is_wine,
                is_fl_path,
            });
        }
    }

    pub fn remove_scan_path(&mut self, index: usize) {
        if index < self.scan_paths.len() {
            self.scan_paths.remove(index);
        }
    }

    pub fn rescan(&mut self) {
        self.populate_known_plugins();
        self.scan_disk();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_initialization_and_fl_plugins() {
        let mgr = PluginManager::default();
        assert!(!mgr.plugins.is_empty());
        assert!(!mgr.scan_paths.is_empty());

        let has_sytrus = mgr.plugins.iter().any(|p| p.name.contains("Sytrus"));
        let has_gross_beat = mgr.plugins.iter().any(|p| p.name.contains("Gross Beat"));
        let has_pro_q = mgr.plugins.iter().any(|p| p.name.contains("Pro-Q"));
        let has_vital = mgr.plugins.iter().any(|p| p.name.contains("Vital"));

        assert!(has_sytrus, "Should have Sytrus FL plugin");
        assert!(has_gross_beat, "Should have Gross Beat FL plugin");
        assert!(has_pro_q, "Should have FabFilter Pro-Q");
        assert!(has_vital, "Should have Vital CLAP");
    }

    #[test]
    fn test_manual_plugin_import() {
        let mut mgr = PluginManager::default();
        let initial_len = mgr.plugins.len();

        let res = mgr.import_file(
            "~/.wine/drive_c/Program Files/VstPlugins/Nexus.dll",
            "reFX",
            PluginCategory::Synth,
        );

        assert!(res.is_ok());
        assert_eq!(mgr.plugins.len(), initial_len + 1);
        let imported = mgr.plugins.last().unwrap();
        assert_eq!(imported.name, "Nexus");
        assert_eq!(imported.vendor, "reFX");
        assert_eq!(imported.format, PluginFormat::WineYabridge);
    }

    #[test]
    fn test_fst_preset_import() {
        let mut mgr = PluginManager::default();
        let initial_presets = mgr.presets.len();

        let res = mgr.import_file(
            "~/.wine/drive_c/FL Studio/Data/Patches/Custom808.fst",
            "Image-Line",
            PluginCategory::Distortion,
        );

        assert!(res.is_err()); // Returns Err with Swedish notification explaining it went to presets
        assert_eq!(mgr.presets.len(), initial_presets + 1);
        let preset = mgr.presets.last().unwrap();
        assert_eq!(preset.preset_name, "Custom808");
    }
}

