//! Projektfilen — formen på disk, sparande, inläsning, autospar och återställning.
//!
//! Här bor `SonixProjectData` och dess `Saved*`-syskon: det som blir JSON i
//! `~/Music/Sonix/Projects/`. Regeln för nya fält är repots: `#[serde(default)]` så en
//! gammal fil läses exakt som förut, och ett prov som läser en **riktig gammal fil** —
//! inte bara en rundtur genom den nya formen.
//!
//! Status: byggs — projektformatet; migreringar flyttar men raderar aldrig.
//! Rör inte: fältnamnen på disk är ett kontrakt mot filer som redan finns hos användaren.
//! Döp aldrig om ett fält — lägg till det nya vid sidan av (`AutomationPointOnDisk` är mönstret).

use super::*;

/// En autosave som erbjuds vid start (Fas 6.1).
#[derive(Clone)]
pub struct RecoveryCandidate {
    pub entry: crate::autosave::AutosaveEntry,
    /// Projektets läsbara namn, läst ur autosavens JSON (annars sluggen).
    pub name: String,
}

/// Autosaves som är nyare än motsvarande manuella projektfil.
///
/// Jämförelsen görs mot den *manuella* filens ändringstid, inte mot en sparad
/// flagga: har användaren sparat efter autosaven finns det inget att rädda, och
/// då ska ingen dialog stå i vägen. Saknas den manuella filen helt (krasch före
/// första sparningen) är autosaven allt som finns och erbjuds alltid.
pub(crate) fn collect_recovery_candidates() -> Vec<RecoveryCandidate> {
    collect_recovery_candidates_in(&crate::paths::paths())
}

/// Samma sak mot en explicit sökvägsuppsättning (testbar utan miljöberoende).
pub(crate) fn collect_recovery_candidates_in(paths: &crate::paths::Paths) -> Vec<RecoveryCandidate> {
    let mut out: Vec<RecoveryCandidate> = Vec::new();
    for entry in crate::autosave::latest_per_project(&paths.autosave_dir()) {
        let Ok(bytes) = std::fs::read(&entry.path) else {
            continue;
        };
        let name = serde_json::from_slice::<SonixProjectData>(&bytes)
            .map(|d| d.name)
            .unwrap_or_else(|_| entry.project.replace('-', " "));
        let manual = std::fs::metadata(paths.project_file(&name))
            .and_then(|m| m.modified())
            .ok();
        if entry.worth_offering(manual) {
            out.push(RecoveryCandidate { entry, name });
        }
    }
    out.sort_by(|a, b| b.entry.stamp.cmp(&a.entry.stamp));
    out.truncate(5);
    out
}

/// Ett projekt i "Senaste projekt"-listan (`recent.json`).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    /// Unix-sekunder då projektet senast öppnades eller sparades.
    #[serde(default)]
    pub opened: u64,
}

/// Hur många projekt läslistan minns.
pub(crate) const RECENT_MAX: usize = 8;

/// Läser `recent.json`. Trasig eller saknad fil ger en tom lista — läslistan är
/// en bekvämlighet, aldrig en förutsättning för att kunna öppna ett projekt.
pub(crate) fn load_recent_projects() -> Vec<RecentProject> {
    load_recent_projects_in(&crate::paths::paths())
}

/// Samma läsning mot en explicit sökvägsuppsättning (testbar).
pub(crate) fn load_recent_projects_in(paths: &crate::paths::Paths) -> Vec<RecentProject> {
    let path = paths.recent_file();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut entries: Vec<RecentProject> = serde_json::from_str(&text).unwrap_or_default();
    // Filen kan ha flyttats eller städats bort utanför appen.
    entries.retain(|e| std::path::Path::new(&e.path).exists());
    entries.truncate(RECENT_MAX);
    entries
}

/// Skriver läslistan atomiskt (samma temp+rename-väg som autosaven).
fn store_recent_projects(entries: &[RecentProject]) {
    store_recent_projects_in(&crate::paths::paths(), entries)
}

/// Samma skrivning mot en explicit sökvägsuppsättning (testbar).
pub(crate) fn store_recent_projects_in(paths: &crate::paths::Paths, entries: &[RecentProject]) {
    let path = paths.recent_file();
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = crate::autosave::write_atomic(&path, json.as_bytes());
    }
}

/// Lägger ett projekt först i läslistan (flyttar upp det om det redan finns).
pub(crate) fn push_recent_project(entries: &mut Vec<RecentProject>, name: &str, path: &str) {
    entries.retain(|e| e.path != path);
    entries.insert(
        0,
        RecentProject {
            name: name.to_string(),
            path: path.to_string(),
            opened: crate::autosave::now_stamp(),
        },
    );
    entries.truncate(RECENT_MAX);
}

/// Ett pattern som det sparas i projektfilen (Fas 6.7).
///
/// `Pattern` själv kan inte serialiseras (Color32 saknar serde), och färgen
/// sparas därför som RGBA.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedPattern {
    pub name: String,
    #[serde(default = "default_ui_color")]
    pub color: [u8; 4],
    pub channel_steps: Vec<[bool; 16]>,
    pub channel_notes: Vec<[u8; 16]>,
    pub piano_roll_grid: [[bool; 16]; 24],
    /// Tagningen med sin tajming (Fas 6.4). Tom i äldre projekt.
    #[serde(default)]
    pub take: crate::midi_take::Take,
}

/// En kanal i Channel Racket som den sparas (Fas 6.7).
///
/// `pcm_audio` och `waveform_preview` sparas **inte**: ljudet ligger redan på
/// disk och läses tillbaka från `sample_path`, och vågformen räknas om ur
/// ljudet. Att spara dem skulle blåsa upp projektfilen med hundratals kilobyte
/// per kanal utan att tillföra något.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedChannel {
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_ui_color")]
    pub color: [u8; 4],
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub steps: [bool; 16],
    pub notes: [u8; 16],
    #[serde(default)]
    pub pitch_semitones: i8,
    #[serde(default)]
    pub pitch_fine_cents: f32,
    #[serde(default)]
    pub sample_start: f32,
    #[serde(default = "default_sample_end")]
    pub sample_end: f32,
    /// Slicekartan ur kanalens ljud (Fas 8.7).
    ///
    /// Den **sparas**, till skillnad från PCM och vågformen: den är ett resultat
    /// av en analys men också något användaren kan ha valt ut, och ett projekt
    /// ska se ut och låta som det gjorde när det stängdes. En gammal fil utan
    /// fältet läses som en kanal utan slicar.
    #[serde(default)]
    pub slices: Vec<(f32, f32)>,
    /// Den gamla attack/decay-ratten. Ingen DSP läste den, men den **sparas** fortfarande:
    /// migreringar flyttar men raderar aldrig. Ett gammalt projekts värde får sätta
    /// samplerns attack när filen öppnas i en version med samplern (Fas 8.4).
    #[serde(default)]
    pub attack_decay: f32,
    /// **Samplern** (Fas 8.4). Saknas fälten i en äldre fil läses kanalen som en
    /// en-skottsprovspelare utan envelop — alltså precis som den lät då.
    #[serde(default)]
    pub loop_mode: crate::audio::LoopMode,
    #[serde(default)]
    pub sample_loop_start: f32,
    #[serde(default = "default_sample_end")]
    pub sample_loop_end: f32,
    #[serde(default)]
    pub ping_pong: bool,
    #[serde(default = "default_sampler_env")]
    pub amp_env: crate::audio::envelope::AdsrParams,
    /// **Velocitetskänsligheten** (Fas 8.4/7). Saknas fältet i en äldre fil är värdet `1,0` —
    /// exakt den linjära faktor velocityn alltid har haft, alltså låter filen som förut.
    #[serde(default = "default_velocity_sensitivity")]
    pub velocity_sensitivity: f32,
    /// **Anslagets kurva** (Fas 8.4/7). Saknas fältet i en äldre fil är kurvan **rak** — den
    /// kanalen alltid har haft, alltså låter filen som förut.
    #[serde(default)]
    pub velocity_curve: crate::audio::envelope::VelocityCurve,
    /// **Filterenvelopen** (Fas 8.4/7). Saknas fältet i en äldre fil är filtret **avstängt** —
    /// alltså exakt det ljud filen hade. (`SamplerFilter::default()` bär både avstängningen och
    /// standardvärdena för cutoff/resonans, så det finns bara en tabell för dem.)
    #[serde(default)]
    pub filter: crate::audio::filter::SamplerFilter,
    /// **Multi-samples** (Fas 8.4/7): kanalens keymap. Saknas fältet i en äldre fil är listan
    /// **tom** — och då spelar kanalen sitt eget sampel, precis som den gjorde.
    #[serde(default)]
    pub zones: Vec<SavedZone>,
    #[serde(default)]
    pub is_reverse: bool,
    #[serde(default)]
    pub sample_path: Option<String>,
    #[serde(default)]
    pub sample_base_note: u8,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SonixProjectData {
    pub name: String,
    pub bpm: f32,
    /// Tempobyten (Fas 8.2). Saknas i äldre projekt — då gäller `bpm` som förut,
    /// och filen läses exakt som den skrevs.
    #[serde(default)]
    pub tempo_points: Vec<crate::audio::tempo::TempoPoint>,
    /// Tonarten (Fas 8.11). Saknas i äldre projekt — då gäller Eb/Dur, samma
    /// standard som konstruktorn sätter (`#[serde(default)]` för skalan, och
    /// `default_song_key_root` för grundtonen).
    /// "Följ tempot" (Fas 8.10 steg 2). **På som standard** — en smurf som uppstår av
    /// misstag är värre än en effekt man får leta efter (Abletons ordning: pitch-bevarande
    /// är default, Re-Pitch är undantaget). En gammal projektfil läses som PÅ, eftersom det
    /// är vad 8.10 steg 1 redan gjorde med klipp som hade ett känt tempo.
    #[serde(default = "default_follow_tempo")]
    pub follow_tempo: bool,
    #[serde(default = "default_song_key_root")]
    pub song_key_root: u8,
    #[serde(default)]
    pub song_key_scale: usize,
    pub swing: f32,
    pub master_volume: f32,
    pub master_pan: f32,
    pub tracks: Vec<SavedTrackData>,
    /// Per-engine-stem-track plugin inserts. Index-aligned with the engine's
    /// stem tracks; `None` means "no plugin on this track".
    #[serde(default)]
    pub plugin_slots: Vec<Option<SavedPluginData>>,
    /// Sub-mix bus group gains (Fas 5.2). Defaults to unity for old projects.
    #[serde(default = "default_bus_volume")]
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    #[serde(default)]
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    /// **Bussarnas egna kurvor** (Fas 8.8). `#[serde(default)]` så en projektfil från före
    /// 8.8 läses som en tom lista — bussen har då bara sin fader, precis som förut.
    #[serde(default)]
    pub bus_automation: Vec<BusAutomationLane>,
    /// **Plugin-insertarnas kurvor** (Fas 8.8). `#[serde(default)]` av samma skäl: en fil
    /// från före 8.8 läses som en tom lista, och inga parametrar styrs av någon kurva.
    #[serde(default)]
    pub plugin_automation: Vec<PluginAutomationLane>,
    #[serde(default)]
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    /// VCA group gains (Fas 5.2). Defaults to unity for old projects.
    #[serde(default = "default_vca_volume")]
    pub vca_volume: [f32; crate::audio::synth::NUM_VCAS],
    #[serde(default)]
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    #[serde(default)]
    pub vca_solo: [bool; crate::audio::synth::NUM_VCAS],
    /// Mönstren med noterna (Fas 6.7). Saknas i äldre projekt — då skapas
    /// standardpattern som förut, men nya projekt får med sig arbetet.
    #[serde(default)]
    pub patterns: Vec<SavedPattern>,
    #[serde(default)]
    pub selected_pattern: usize,
    /// Stegvolymerna i trumsekvenserna (Fas 6.7). `None` = filen skrevs före
    /// den här ändringen, och då lämnas appens nuvarande värden orörda.
    #[serde(default)]
    pub step_velocities: Option<[f32; 16]>,
    /// Channel Racket (Fas 6.7).
    #[serde(default)]
    pub channels: Vec<SavedChannel>,
}

/// Standardgrundton för ett projekt som sparades innan tonarten fanns (Fas 8.11):
/// Eb, samma värde som `SonixApp::new` sätter.
fn default_song_key_root() -> u8 {
    3
}

pub(crate) fn default_bus_volume() -> [f32; crate::audio::synth::NUM_BUSES] {
    [1.0; crate::audio::synth::NUM_BUSES]
}

fn default_ui_color() -> [u8; 4] {
    [120, 120, 130, 255]
}

fn default_sample_end() -> f32 {
    1.0
}

/// **En zon som den står i projektfilen** (Fas 8.4/7): filen och intervallen. Ljudet sparas
/// **inte** — det läses ur `sample_path` igen, samma regel som för kanalens eget sampel, och av
/// samma skäl: ljudet ligger redan på disk och en projektfil ska inte bära kopior av det.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedZone {
    #[serde(default)]
    pub sample_path: Option<String>,
    /// Sampelns grundton.
    #[serde(default)]
    pub root: u8,
    #[serde(default)]
    pub key_low: u8,
    /// Hela registret som standard: en zon utan intervall ska höras, inte tystna.
    #[serde(default = "default_key_high")]
    pub key_high: u8,
    #[serde(default)]
    pub vel_low: f32,
    #[serde(default = "default_vel_high")]
    pub vel_high: f32,
}

fn default_key_high() -> u8 {
    127
}

fn default_vel_high() -> f32 {
    1.0
}

/// Samplerns envelop som standard: **identiteten** (Fas 8.4). Ett projekt som sparades
/// innan samplern fanns har inget `amp_env`-fält, och då ska kanalen låta exakt som den
/// gjorde — inte få den inbyggda syntens standard-ADSR på köpet.
fn default_sampler_env() -> crate::audio::envelope::AdsrParams {
    crate::audio::envelope::AdsrParams::identity()
}

/// Velocitetskänslighetens standard: **1,0**, den faktor velocityn alltid har haft (Fas 8.4/7).
/// Ett projekt som sparades innan ratten fanns har inget fält — och ska då låta exakt som det
/// gjorde, inte tappa sitt anslag.
fn default_velocity_sensitivity() -> f32 {
    1.0
}

/// Det som ska tillbaka in i appen när en projektfil öppnas (Fas 6.7).
pub struct SavedMusic<'a> {
    pub patterns: &'a [SavedPattern],
    pub selected_pattern: usize,
    pub step_velocities: Option<[f32; 16]>,
    pub channels: &'a [SavedChannel],
}

/// Lägger tillbaka mönster, kanalrack och stegvolymer.
///
/// Ren funktion (Fas 6.7), så att *inläsningsvägen* kan testas utan GUI — det
/// var just den vägen som saknades: filen hade inga noter att läsa, och ingen
/// kod som ens försökte. Har filen inga mönster (skriven före den här
/// ändringen) lämnas appens standardpatterns och standardrack orörda.
pub fn restore_saved_music(
    saved: &SavedMusic,
    patterns: &mut Vec<Pattern>,
    channels: &mut [ChannelStrip],
    selected_pattern: &mut usize,
    step_velocities: &mut [f32; 16],
) {
    if !saved.patterns.is_empty() {
        for (i, s) in saved.patterns.iter().enumerate() {
            let restored = saved_to_pattern(s);
            if i < patterns.len() {
                patterns[i] = restored;
            } else {
                patterns.push(restored);
            }
        }
        *selected_pattern = saved.selected_pattern.min(patterns.len().saturating_sub(1));
    }
    if let Some(v) = saved.step_velocities {
        *step_velocities = v;
    }
    for (i, s) in saved.channels.iter().enumerate() {
        if i < channels.len() {
            channels[i] = saved_to_channel(s);
        }
    }
}

pub(crate) fn pattern_to_saved(p: &Pattern) -> SavedPattern {
    SavedPattern {
        name: p.name.clone(),
        color: p.color.to_array(),
        channel_steps: p.channel_steps.clone(),
        channel_notes: p.channel_notes.clone(),
        piano_roll_grid: p.piano_roll_grid,
        take: p.take.clone(),
    }
}

pub(crate) fn saved_to_pattern(s: &SavedPattern) -> Pattern {
    Pattern {
        name: s.name.clone(),
        color: Color32::from_rgba_premultiplied(s.color[0], s.color[1], s.color[2], s.color[3]),
        channel_steps: s.channel_steps.clone(),
        channel_notes: s.channel_notes.clone(),
        piano_roll_grid: s.piano_roll_grid,
        take: s.take.clone(),
    }
}

pub(crate) fn channel_to_saved(c: &ChannelStrip) -> SavedChannel {
    SavedChannel {
        name: c.name.clone(),
        icon: c.icon.clone(),
        color: c.color.to_array(),
        volume: c.volume,
        pan: c.pan,
        muted: c.muted,
        solo: c.solo,
        steps: c.steps,
        notes: c.notes,
        pitch_semitones: c.pitch_semitones,
        pitch_fine_cents: c.pitch_fine_cents,
        sample_start: c.sample_start,
        sample_end: c.sample_end,
        slices: c.slices.clone(),
        attack_decay: c.attack_decay,
        loop_mode: c.loop_mode,
        sample_loop_start: c.sample_loop_start,
        sample_loop_end: c.sample_loop_end,
        ping_pong: c.ping_pong,
        amp_env: c.amp_env,
        velocity_sensitivity: c.velocity_sensitivity,
        velocity_curve: c.velocity_curve,
        filter: c.filter,
        // **Zonerna** (Fas 8.4/7): filen och intervallen sparas, ljudet läses (se `SavedZone`).
        zones: c
            .zones
            .iter()
            .map(|z| SavedZone {
                sample_path: z.sample_path.clone(),
                root: z.root,
                key_low: z.key_low,
                key_high: z.key_high,
                vel_low: z.vel_low,
                vel_high: z.vel_high,
            })
            .collect(),
        is_reverse: c.is_reverse,
        sample_path: c.sample_path.clone(),
        sample_base_note: c.sample_base_note,
    }
}

/// Bygger tillbaka en kanal. Ljudet läses från `sample_path` igen och vågformen
/// räknas om ur ljudet — det är därför de två inte sparas.
pub(crate) fn saved_to_channel(s: &SavedChannel) -> ChannelStrip {
    let pcm = s.sample_path.as_deref().and_then(load_sample_pcm_arcs);
    let preview = match &pcm {
        Some((left, _, _)) => waveform_preview_from_pcm(left, 128),
        None => Vec::new(),
    };
    ChannelStrip {
        name: s.name.clone(),
        icon: s.icon.clone(),
        color: Color32::from_rgba_premultiplied(s.color[0], s.color[1], s.color[2], s.color[3]),
        volume: s.volume,
        pan: s.pan,
        muted: s.muted,
        solo: s.solo,
        steps: s.steps,
        notes: s.notes,
        pitch_semitones: s.pitch_semitones,
        pitch_fine_cents: s.pitch_fine_cents,
        sample_start: s.sample_start,
        sample_end: s.sample_end,
        // Slicekartan följer med i projektfilen (se `SavedChannel::slices`): den är
        // ett resultat av en analys, men också något användaren kan ha valt ut.
        slices: s.slices.clone(),
        attack_decay: s.attack_decay,
        // **Migreringen** (Fas 8.4): ett projekt som sparades innan samplern fanns har
        // ingen envelop alls (`identity`), och då får den gamla attack/decay-ratten sätta
        // attacken — värdet flyttas i stället för att tappas. Har filen ett `amp_env`
        // används det rakt av.
        amp_env: if s.amp_env.is_identity() && s.attack_decay > 0.0 {
            crate::audio::envelope::AdsrParams {
                attack: (s.attack_decay * 0.5).max(0.0),
                ..crate::audio::envelope::AdsrParams::identity()
            }
        } else {
            s.amp_env
        },
        // **Anslagets känslighet** (Fas 8.4/7) har ingen migrering: fältet saknas i en äldre fil
        // och blir då 1,0 — exakt den linjära faktor velocityn alltid har haft.
        velocity_sensitivity: s.velocity_sensitivity,
        velocity_curve: s.velocity_curve,
        filter: s.filter,
        // **Zonerna** (Fas 8.4/7): ljudet läses ur filen igen — kanalen gör precis likadant med
        // sitt eget sampel strax ovanför. En zon vars fil inte går att läsa får `pcm: None`, och
        // då spelar kanalens eget sampel i stället (och gränssnittet säger att filen inte lästes).
        zones: s
            .zones
            .iter()
            .map(|z| crate::audio::keymap::SampleZone {
                pcm: z.sample_path.as_deref().and_then(load_sample_pcm_arcs),
                sample_path: z.sample_path.clone(),
                root: z.root,
                key_low: z.key_low,
                key_high: z.key_high,
                vel_low: z.vel_low,
                vel_high: z.vel_high,
            })
            .collect(),
        loop_mode: s.loop_mode,
        sample_loop_start: s.sample_loop_start,
        sample_loop_end: s.sample_loop_end,
        ping_pong: s.ping_pong,
        is_reverse: s.is_reverse,
        waveform_preview: preview,
        sample_path: s.sample_path.clone(),
        pcm_audio: pcm,
        sample_base_note: s.sample_base_note,
    }
}

pub(crate) fn default_vca_volume() -> [f32; crate::audio::synth::NUM_VCAS] {
    [1.0; crate::audio::synth::NUM_VCAS]
}

/// A plugin insert persisted with the project: its shared-object path plus the
/// opaque state blob produced by `clap.state`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedPluginData {
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: Vec<u8>,
    /// Whether the plugin was loaded in an out-of-process sandbox (Fas 4.5b).
    #[serde(default)]
    pub sandboxed: bool,
    /// **Manuellt latens-offset i ramar** (Fas 8.6). `#[serde(default)]` så en projektfil
    /// från före 8.6 läses som **noll** — alltså exakt den kompensation som gällde då.
    #[serde(default)]
    pub latency_offset_frames: i32,
    /// **Smart disable** (Fas 8.6). `#[serde(default)]`: en fil från före 8.6 läses som
    /// **av** — och av är exakt det beteende filen spelades in med.
    #[serde(default)]
    pub smart_disable: bool,
    /// **Pluginens egna utbussar → spår** (Fas 8.6): `extra_out_targets[port]` är målet för
    /// pluginens utbuss `port` — den första *egna* bussen; port 0 är huvudutgången och går
    /// alltid till spårets egen kedja. `#[serde(default)]`: en fil från före den här punkten
    /// läses med **inga kopplingar**, alltså exakt det beteende den spelades in med (bussarna
    /// lästes inte alls).
    #[serde(default)]
    pub extra_out_targets: Vec<Option<usize>>,
}

/// In-memory mirror of [`SavedPluginData`] kept on [`SonixApp`].
#[derive(Clone)]
pub struct PluginSlot {
    pub path: String,
    pub name: String,
    pub state: Vec<u8>,
    pub sandboxed: bool,
    /// Manuellt latens-offset i ramar (Fas 8.6). Se `compensated_latency`.
    pub latency_offset_frames: i32,
    /// Smart disable (Fas 8.6): låt pluginen vila när den varken får eller ger ljud.
    pub smart_disable: bool,
    /// Pluginens egna utbussar → spår (Fas 8.6). Tom = inga kopplingar.
    pub extra_out_targets: Vec<Option<usize>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedTrackData {
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub clips: [Option<usize>; 32],
    pub regions: Vec<AudioRegion>,
    #[serde(default)]
    pub eq: TrackEq,
    #[serde(default = "default_comp_thresh")]
    pub comp_threshold_db: f32,
    #[serde(default = "default_comp_ratio")]
    pub comp_ratio: f32,
    #[serde(default)]
    pub reverb_send: f32,
    #[serde(default)]
    pub delay_send: f32,
    #[serde(default)]
    pub automation: Vec<AutomationLaneOnDisk>,
    /// Sub-mix bus assignment (Fas 5.2). Defaults to bus 0 for old projects.
    #[serde(default)]
    pub bus: usize,
    /// Optional VCA group assignment (Fas 5.2).
    #[serde(default)]
    pub vca: Option<usize>,
    /// Sidokedja (Fas 8.3): spåret duckas av det här spårets ljud. Saknas i
    /// äldre projekt — då finns ingen sidokedja, precis som förut.
    #[serde(default)]
    pub sidechain_from: Option<usize>,
    #[serde(default)]
    pub sidechain_amount_db: f32,
    #[serde(default = "default_sidechain_threshold_db")]
    pub sidechain_threshold_db: f32,
    /// Sends (Fas 8.13): parallella vägar till andra bussar. Saknas i äldre
    /// projekt — då finns inga sends, precis som förut.
    #[serde(default)]
    pub sends: Vec<crate::audio::StemSend>,
    /// Fruset spår (Tier 2): var ljudet ligger och fingeravtrycket av källan.
    /// Äldre projektfil utan fältet läses som ofrusade.
    #[serde(default)]
    pub frozen: Option<SavedFrozenTrack>,
}

/// Standardtröskel för en sidokedja (Fas 8.3): −30 dB är där en duckare brukar
/// börja, och en äldre projektfil ska få samma värde som ett nytt spår.
/// Standard för "Följ tempot": **på** (se `SonixProjectData::follow_tempo`).
fn default_follow_tempo() -> bool {
    true
}

fn default_sidechain_threshold_db() -> f32 {
    -30.0
}

/// Ett fruset spår i projektfilen. Ljudet ligger i en fil i projektets egen
/// materialmapp — filen bär sökvägen, inte ljudet, precis som för samplingar.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SavedFrozenTrack {
    pub path: String,
    pub digest: u64,
    #[serde(default)]
    pub stamp: u64,
}

/// Ett fruset spår (Tier 2): spåret ligger färdigrenderat som ljud, och
/// pattern-uppspelningen hoppas över till förmån för det. Motorn strömmar
/// ljudet som ett stem-spår — samma väg ljudspåren redan använder — så ingen ny
/// uppspelningsväg behöver underhållas.
#[derive(Clone, Debug)]
pub struct FrozenTrack {
    /// WAV-filen i projektets egen materialmapp (`Frozen/`).
    pub path: String,
    /// Fingeravtryck av det som påverkade renderingen: tempo, spårets klipp och
    /// fader, mönstren klippen pekar på, kanalracket och röstinställningarna.
    /// Ändras något av det visas frysningen som inaktuell i stället för att
    /// spela fel ljud tyst. Det är en varning, inte ett bevis: fingeravtrycket
    /// ser att något skiljer sig, det avgör inte vad som är rätt.
    pub digest: u64,
    /// Unix-tid för frysningen, så att åldern går att visa.
    pub stamp: u64,
}

/// Klippen som ska trigga i en rendering: ett fruset spår har sitt ljud i
/// stället, så dess patterns ska vara tysta — annars räknas spåret två gånger i
/// både uppspelning och export. Ren funktion, så att regeln går att pröva.
pub fn render_clips_for(track: &PlaylistTrack) -> [Option<usize>; 32] {
    if track.is_frozen() {
        [None; 32]
    } else {
        track.clips
    }
}

/// Ska ett spårs ljud med i en rendering?
///
/// Ett fruset spår hörs som ljud i **sång-läget**. Pattern-läget renderar
/// kanalracket, och där hör spårets frysta ljud inte hemma — utan den regeln
/// hamnar det i en pattern-export som inte bad om det. Regeln ligger som en egen
/// funktion för att den gick att tappa bort i en omskrivning (den gjorde det,
/// under arbetet med tempokartan) och för att den nu prövas av sviten.
pub fn frozen_audio_in_render(track: &PlaylistTrack, pattern_mode: bool) -> bool {
    !(pattern_mode && track.frozen_pcm.is_some())
}

#[derive(Clone)]
pub struct PreloadedTrackData {
    pub frozen: Option<SavedFrozenTrack>,
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub regions: Vec<AudioRegion>,
    pub clips: [Option<usize>; 32],
    pub eq: TrackEq,
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    pub automation: Vec<AutomationLaneOnDisk>,
    pub bus: usize,
    pub vca: Option<usize>,
    /// Sidokedja (Fas 8.3), med i förinläsningen så att ett laddat projekt
    /// duckar precis som det gjorde när det sparades.
    pub sidechain_from: Option<usize>,
    pub sidechain_amount_db: f32,
    pub sidechain_threshold_db: f32,
    /// Sends (Fas 8.13), med i förinläsningen så att ett laddat projekt skickar
    /// sin signal till samma bussar som när det sparades.
    pub sends: Vec<crate::audio::StemSend>,
    pub stem_pcms: Vec<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
    /// Det **frusna** spårets ljud, om filen gick att läsa vid inläsningen.
    /// Hålls åtskild från `stem_pcms`: `track_pcm` blir det *sista* som lades i
    /// stem_pcms, och ett fruset spår vars fil saknas får inte ärva spårets egna
    /// klippljud och kalla det för sin frysning.
    pub frozen_pcm: Option<(std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32)>,
}

#[derive(Clone)]
pub struct LoadedProjectPayload {
    pub name: String,
    pub bpm: f32,
    /// Tempobyten (Fas 8.2). Saknas i äldre projekt — då gäller `bpm` som förut
    /// (fältet är `#[serde(default)]` i `SonixProjectData`, som är den som läses).
    pub tempo_points: Vec<crate::audio::tempo::TempoPoint>,
    /// Tonarten (Fas 8.11) — läses ur projektfilen och sätts på appen, så att
    /// piano rollen och AI-kontexten visar samma tonart som när filen sparades.
    pub song_key_root: u8,
    pub song_key_scale: usize,
    /// "Följ tempot" (Fas 8.10 steg 2) — läses ur projektfilen så att switchen står
    /// där den stod när filen sparades. Äldre filer saknar fältet och får
    /// `default_follow_tempo()` = på, samma värde som en ny app har.
    pub follow_tempo: bool,
    pub swing: f32,
    pub master_volume: f32,
    pub master_pan: f32,
    pub tracks: Vec<PreloadedTrackData>,
    pub plugin_slots: Vec<Option<SavedPluginData>>,
    pub bus_volume: [f32; crate::audio::synth::NUM_BUSES],
    pub bus_muted: [bool; crate::audio::synth::NUM_BUSES],
    /// **Bussarnas egna kurvor** (Fas 8.8).
    pub bus_automation: Vec<BusAutomationLane>,
    /// **Plugin-insertarnas kurvor** (Fas 8.8).
    pub plugin_automation: Vec<PluginAutomationLane>,
    pub bus_solo: [bool; crate::audio::synth::NUM_BUSES],
    pub vca_volume: [f32; crate::audio::synth::NUM_VCAS],
    pub vca_muted: [bool; crate::audio::synth::NUM_VCAS],
    pub vca_solo: [bool; crate::audio::synth::NUM_VCAS],
    /// Mönster, kanalrack och stegvolymer (Fas 6.7).
    pub patterns: Vec<SavedPattern>,
    pub selected_pattern: usize,
    pub step_velocities: Option<[f32; 16]>,
    pub channels: Vec<SavedChannel>,
    pub file_path: String,
}

/// Sammanfattning av mixerns ljudbild: varje värde som `sync_track_audio_state`
/// och `sync_group_state` skickar till motorn, hashat till ett tal.
///
/// Används av mixer-undon (Fas 6.2) för att upptäcka *att* mixern ändrats utan
/// att jämföra hela projektet, och av testet som bevisar att varje mixat fält
/// faktiskt ingår. Ändrar du vad motorn tar emot ska du ändra här också —
/// testet `mixer_digest_covers_every_mixed_field` räknar upp fälten.
pub(crate) fn mixer_digest(
    tracks: &[PlaylistTrack],
    bus_volume: &[f32],
    bus_muted: &[bool],
    bus_solo: &[bool],
    vca_faders: &[f32],
    vca_muted: &[bool],
    vca_solos: &[bool],
    non_track: &state::NonTrackSound,
) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f32| {
        for b in v.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for t in tracks {
        mix(t.volume);
        mix(t.pan);
        mix(if t.muted { 1.0 } else { 0.0 });
        mix(if t.solo { 1.0 } else { 0.0 });
        mix(t.comp_threshold_db);
        mix(t.comp_ratio);
        mix(t.reverb_send);
        mix(t.delay_send);
        mix(t.pitch_semitones);
        mix(t.bus as f32);
        mix(t.vca.map(|v| v as f32).unwrap_or(-1.0));
        // Sends (Fas 8.13) hör till mixen: målet OCH nivån, och antalet — en send
        // som tas bort ändrar ljudet även om de kvarvarande är likadana.
        mix(t.sends.len() as f32);
        for send in &t.sends {
            // Målet hör till mixen oavsett sort — en buss och ett spår med samma nummer
            // är inte samma väg. Spår-mål läggs i ett eget intervall så att de inte kan
            // förväxlas med en buss i digesten.
            match send.target {
                crate::audio::SendTarget::Bus { target_bus } => mix(target_bus as f32),
                crate::audio::SendTarget::Track { target_track } => {
                    mix(1000.0 + target_track as f32)
                }
            }
            mix(send.level);
        }
        mix(t.eq.low_gain_db);
        mix(t.eq.low_freq);
        mix(t.eq.mid_gain_db);
        mix(t.eq.mid_freq);
        mix(t.eq.high_gain_db);
        mix(t.eq.high_freq);
    }
    for v in bus_volume.iter().chain(vca_faders.iter()).copied() {
        mix(v);
    }
    for b in bus_muted.iter().chain(bus_solo).chain(vca_muted).chain(vca_solos).copied() {
        mix(if b { 1.0 } else { 0.0 });
    }
    // **Mastern och plugin-kurvorna** (Fas 6.2). De hörs, men ligger utanför `playlist_tracks`,
    // och av samma skäl som bussarna ovan måste de in i digesten: annars skapas ingen
    // ångringspunkt när mastern ändras — och då finns det ingenting att ångra.
    mix(non_track.drive);
    mix(non_track.master_volume);
    mix(non_track.delay.time_ms);
    mix(non_track.delay.feedback);
    mix(non_track.delay.mix);
    mix(non_track.reverb.room_size);
    mix(non_track.reverb.damping);
    mix(non_track.reverb.mix);
    // Antalet först: en pedal som tas bort ändrar ljudet även om de kvarvarande står likadant
    // (samma regel som för sends).
    mix(non_track.pedals.len() as f32);
    for p in &non_track.pedals {
        mix(if p.enabled { 1.0 } else { 0.0 });
        mix(p.p1_val);
        mix(p.p2_val);
        mix(p.p3_val);
    }
    mix(non_track.eq_nodes.len() as f32);
    for n in &non_track.eq_nodes {
        mix(n.freq_hz);
        mix(n.gain_db);
        mix(n.q);
        mix(if n.is_active { 1.0 } else { 0.0 });
    }
    mix(non_track.compressor_release_ms);
    // Plugin-kurvorna: spår, parameter-id, på/av och **punkterna**. En kurva som flyttas är en
    // ändring av ljudet, inte av gränssnittet.
    mix(non_track.plugin_automation.len() as f32);
    for lane in &non_track.plugin_automation {
        mix(lane.track as f32);
        mix(lane.param_id as f32);
        mix(if lane.enabled { 1.0 } else { 0.0 });
        mix(lane.points.len() as f32);
        for p in &lane.points {
            mix(p.time_bars);
            mix(p.value);
        }
    }
    h
}

#[derive(Clone, Debug)]
pub enum ScreenshotTarget {
    View(ViewMode),
    VocalTakeLanes,
    VocalCustomSounds,
    AudioSettingsModal,
    AiSettingsModal,
    ProjectManagerModal,
    RenderQueueModal,
    SunoImportModal,
    ControllerModal,
    StemFocusModal,
    MicSettingsModal,
    ChordGeneratorModal,
    TunerModal,
    DiceGeneratorModal,
    FxRackModal,
    SongStructureModal,
    AddTrackModal,
    HelpGuideModal,
    AboutModal,
    /// Frågan om mp3 eller wav (Fas 8.5).
    WavQuestionModal,
}

#[derive(Clone, Debug)]
pub enum ScreenshotState {
    Idle,
    Preparing { target: ScreenshotTarget, dest: std::path::PathBuf, frames_left: usize },
    /// Väntar på `Event::Screenshot`. `frames_left` är tålamodet: en begäran som
    /// aldrig besvaras ska säga det i stället för att vänta för evigt.
    AwaitingCapture { dest: std::path::PathBuf, frames_left: u32 },
}

/// Hur många bildrutor en skärmdump får vänta på sitt svar innan körningen ger upp.
///
/// `ViewportCommand::Screenshot` är en **begäran**: svaret kommer tillbaka som ett
/// `Event::Screenshot`. I vissa sessioner kommer det aldrig — 2026-09-12 stod
/// loopen stilla på första målet för evigt, med fönstret uppe och synligt, utan ett
/// ord om varför. Ett verktyg som väntar i det tysta ser ut att arbeta; 180
/// bildrutor (≈3 s) räcker för den som svarar, och den som inte svarar får ett fel.
pub(crate) const SCREENSHOT_WAIT_FRAMES: u32 = 180;

impl SonixApp {
pub fn enable_screenshot_mode(&mut self, out_dir: &std::path::Path) {
    self.screenshot_mode_active = true;
    self.screenshot_state = ScreenshotState::Idle;
    self.populate_demo_data();
    let _ = std::fs::create_dir_all(out_dir);


    let targets = vec![
        (ScreenshotTarget::View(ViewMode::PlaylistArranger), out_dir.join("01_tidslinje_arranger.png")),
        (ScreenshotTarget::View(ViewMode::ChannelRack), out_dir.join("02_channel_rack.png")),
        (ScreenshotTarget::View(ViewMode::PianoRoll), out_dir.join("03_piano_roll.png")),
        (ScreenshotTarget::VocalTakeLanes, out_dir.join("04_vocal_studio_leads.png")),
        (ScreenshotTarget::VocalCustomSounds, out_dir.join("05_vocal_studio_sampler.png")),
        (ScreenshotTarget::View(ViewMode::EffectsMixer), out_dir.join("06_effects_mixer.png")),
        (ScreenshotTarget::View(ViewMode::AiMusicAssistant), out_dir.join("07_ai_music_assistant.png")),
        (ScreenshotTarget::View(ViewMode::AlchemySynth), out_dir.join("08_alchemy_synth.png")),
        (ScreenshotTarget::View(ViewMode::SessionDrummer), out_dir.join("09_session_drummer.png")),
        (ScreenshotTarget::View(ViewMode::ModularPatcher), out_dir.join("10_modular_patcher.png")),
        (ScreenshotTarget::View(ViewMode::StemSeparator), out_dir.join("11_stem_separator.png")),
        (ScreenshotTarget::View(ViewMode::PluginManager), out_dir.join("12_plugin_manager.png")),
        (ScreenshotTarget::View(ViewMode::RemixFx), out_dir.join("13_remix_fx.png")),
        (ScreenshotTarget::AudioSettingsModal, out_dir.join("14_dialog_ljudinstallningar.png")),
        (ScreenshotTarget::AiSettingsModal, out_dir.join("15_dialog_ai_installningar.png")),
        (ScreenshotTarget::ProjectManagerModal, out_dir.join("16_dialog_projekthanterare.png")),
        (ScreenshotTarget::RenderQueueModal, out_dir.join("17_dialog_render_queue.png")),
        (ScreenshotTarget::SunoImportModal, out_dir.join("18_dialog_suno_import.png")),
        (ScreenshotTarget::ControllerModal, out_dir.join("19_dialog_midi_controller.png")),
        (ScreenshotTarget::StemFocusModal, out_dir.join("20_dialog_stem_focus_editor.png")),
        (ScreenshotTarget::MicSettingsModal, out_dir.join("21_dialog_mikrofon_installningar.png")),
        (ScreenshotTarget::ChordGeneratorModal, out_dir.join("22_dialog_ackord_matris.png")),
        (ScreenshotTarget::TunerModal, out_dir.join("23_dialog_strobe_tuner.png")),
        (ScreenshotTarget::DiceGeneratorModal, out_dir.join("24_dialog_tarning_generator.png")),
        (ScreenshotTarget::FxRackModal, out_dir.join("25_dialog_modular_fx_rack.png")),
        (ScreenshotTarget::SongStructureModal, out_dir.join("26_dialog_latstruktur_arrangemang.png")),
        (ScreenshotTarget::AddTrackModal, out_dir.join("27_dialog_lagg_till_spar.png")),
        (ScreenshotTarget::HelpGuideModal, out_dir.join("28_dialog_hjalpguide_manual.png")),
        (ScreenshotTarget::AboutModal, out_dir.join("29_dialog_om_sonix.png")),
        (ScreenshotTarget::WavQuestionModal, out_dir.join("30_dialog_mp3_eller_wav.png")),
    ];

    self.screenshot_queue = targets;
}
}

impl SonixApp {
pub fn apply_screenshot_target(&mut self, target: ScreenshotTarget) {
    self.show_help_guide = false;
    self.show_project_manager_modal = false;
    self.show_suno_import_modal = false;
    self.show_render_queue_modal = false;
    self.show_stem_focus_modal = false;
    self.show_controller_modal = false;
    self.show_about_modal = false;
    self.show_ai_settings_modal = false;
    self.show_audio_settings_modal = false;
    self.show_import_modal = false;
    self.show_mic_settings_modal = false;
    self.show_chord_generator_modal = false;
    self.show_tuner_modal = false;
    self.show_dice_generator_modal = false;
    self.show_fx_rack_modal = false;
    self.show_song_structure_modal = false;
    self.show_add_track_modal = false;

    match target {
        ScreenshotTarget::View(vm) => {
            self.view_mode = vm;
        }
        ScreenshotTarget::VocalTakeLanes => {
            self.view_mode = ViewMode::VocalStudio;
            self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::LeadVocals;
        }
        ScreenshotTarget::VocalCustomSounds => {
            self.view_mode = ViewMode::VocalStudio;
            self.vocal_studio.recording_mode = crate::audio::recorder::RecordingMode::CustomSounds;
        }
        ScreenshotTarget::AudioSettingsModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_audio_settings_modal = true;
        }
        ScreenshotTarget::AiSettingsModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_ai_settings_modal = true;
        }
        ScreenshotTarget::ProjectManagerModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_project_manager_modal = true;
        }
        ScreenshotTarget::RenderQueueModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.open_export_modal();
        }
        ScreenshotTarget::SunoImportModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_suno_import_modal = true;
        }
        ScreenshotTarget::WavQuestionModal => {
            // Ett påhittat men trovärdigt underlag: frågan ser ut som den gör
            // när en import har tre mp3:er som redan har sina wav-syskon.
            self.view_mode = ViewMode::PlaylistArranger;
            self.pending_wav_question = Some(WavQuestion::StemImport {
                source: StemImportSource::Folder(
                    crate::paths::paths().samples_dir().to_string_lossy().into_owned(),
                ),
                title: "Cyberpunk Odyssey".to_string(),
                bpm: 126.0,
                duplicates: 3,
            });
        }
        ScreenshotTarget::ControllerModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_controller_modal = true;
        }
        ScreenshotTarget::StemFocusModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.focused_stem_track = Some(0);
            self.show_stem_focus_modal = true;
        }
        ScreenshotTarget::MicSettingsModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_mic_settings_modal = true;
        }
        ScreenshotTarget::ChordGeneratorModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_chord_generator_modal = true;
        }
        ScreenshotTarget::TunerModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_tuner_modal = true;
        }
        ScreenshotTarget::DiceGeneratorModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_dice_generator_modal = true;
        }
        ScreenshotTarget::FxRackModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_fx_rack_modal = true;
        }
        ScreenshotTarget::SongStructureModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_song_structure_modal = true;
        }
        ScreenshotTarget::AddTrackModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_add_track_modal = true;
        }
        ScreenshotTarget::HelpGuideModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_help_guide = true;
        }
        ScreenshotTarget::AboutModal => {
            self.view_mode = ViewMode::PlaylistArranger;
            self.show_about_modal = true;
        }
    }
}
}

impl SonixApp {
pub fn populate_demo_data(&mut self) {
    self.project_name = "Cyberpunk Odyssey (Suno AI + Sonix Lead)".to_string();
    self.bpm = 124.0;
    self.swing = 0.15;
    self.song_bar = 4;
    self.song_step_in_bar = 2;

    // Populate Channel rack steps
    if self.channels.len() >= 6 {
        self.channels[0].steps = [true, false, false, false, true, false, false, false, true, false, false, false, true, false, false, false];
        self.channels[1].steps = [false, false, false, false, true, false, false, false, false, false, false, false, true, false, false, false];
        self.channels[2].steps = [true, false, true, false, true, false, true, false, true, false, true, false, true, false, true, false];
        self.channels[3].steps = [false, false, true, false, false, false, true, false, false, false, true, false, false, false, true, false];
        self.channels[4].steps = [true, false, false, true, false, true, false, false, true, false, true, false, false, true, false, false];
        self.channels[5].steps = [true, true, false, false, true, true, false, false, true, true, false, false, true, true, false, false];
    }

    // Populate Piano Roll Grid (24 notes x 16 steps)
    for r in 0..24 {
        for c in 0..16 {
            self.piano_roll_grid[r][c] = false;
        }
    }
    let notes_to_set = [
        (12, 0), (12, 1), (15, 2), (19, 3), (12, 4), (17, 6), (15, 7),
        (10, 8), (14, 10), (17, 11), (10, 12), (12, 14), (15, 15),
        (5, 0), (5, 4), (5, 8), (5, 12), (0, 0), (0, 4), (0, 8), (0, 12)
    ];
    for (r, c) in notes_to_set {
        if r < 24 && c < 16 {
            self.piano_roll_grid[r][c] = true;
        }
    }

    // Populate Audio Regions in Playlist Tracks
    let sample_waveform: Vec<f32> = (0..120).map(|i| {
        let t = i as f32 / 120.0;
        ((t * 24.0).sin() * 0.5 + (t * 50.0).sin() * 0.3 + 0.1).abs().clamp(0.05, 0.95)
    }).collect();

    if self.playlist_tracks.is_empty() {
        self.playlist_tracks = vec![
            PlaylistTrack::new("🎤 Lead Vocals (Suno AI)".to_string(), "🎤", TrackKind::VocalAudio, Color32::from_rgb(255, 100, 120)),
            PlaylistTrack::new("🎧 Backing Choir".to_string(), "🎧", TrackKind::VocalAudio, Color32::from_rgb(220, 140, 255)),
            PlaylistTrack::new("🥁 808 & Drums".to_string(), "🥁", TrackKind::Drums, Theme::FL_ORANGE),
            PlaylistTrack::new("🎹 Nexus Synth Lead".to_string(), "🎹", TrackKind::SynthLead, Theme::FL_CYAN),
            PlaylistTrack::new("🎸 Moog Sub Bass".to_string(), "🎸", TrackKind::Bassline, Theme::FL_GREEN),
            PlaylistTrack::new("🌌 Space FX & Risers".to_string(), "🌌", TrackKind::Fx, Theme::FL_YELLOW),
        ];
    }

    if let Some(t0) = self.playlist_tracks.get_mut(0) {
        t0.name = "🎤 Lead Vocals (Suno AI)".to_string();
        t0.regions = vec![
            AudioRegion {
                source_bpm: 0.0,
                tape: false,
                id: 101,
                name: "Chorus Take 1 (Main)".to_string(),
                start_bar: 1.0,
                length_bars: 7.0,
                sample_offset_sec: 0.0,
                source_path: Some("stems/lead_vocals.wav".to_string()),
                waveform_peaks: sample_waveform.clone(),
                volume: 0.95,
                fade_in_bars: 0.1,
                fade_out_bars: 0.2,
                muted: false,
                is_reverse: false,
                color: Color32::from_rgb(255, 100, 120),
                loop_length_bars: 0.0,
            },
            AudioRegion {
                source_bpm: 0.0,
                tape: false,
                id: 102,
                name: "Verse Hook (Harmonized)".to_string(),
                start_bar: 9.0,
                length_bars: 6.0,
                sample_offset_sec: 0.0,
                source_path: Some("stems/lead_vocals_hook.wav".to_string()),
                waveform_peaks: sample_waveform.clone(),
                volume: 0.90,
                fade_in_bars: 0.1,
                fade_out_bars: 0.2,
                muted: false,
                is_reverse: false,
                color: Color32::from_rgb(255, 120, 140),
                loop_length_bars: 0.0,
            },
        ];
    }

    if let Some(t1) = self.playlist_tracks.get_mut(1) {
        t1.name = "🎧 Backing Choir".to_string();
        t1.regions = vec![
            AudioRegion {
                source_bpm: 0.0,
                tape: false,
                id: 201,
                name: "Stereo Choir Pad (4-Part)".to_string(),
                start_bar: 3.0,
                length_bars: 5.0,
                sample_offset_sec: 0.0,
                source_path: Some("stems/choir.wav".to_string()),
                waveform_peaks: sample_waveform.clone(),
                volume: 0.80,
                fade_in_bars: 0.5,
                fade_out_bars: 0.5,
                muted: false,
                is_reverse: false,
                color: Color32::from_rgb(200, 130, 255),
                loop_length_bars: 0.0,
            },
        ];
    }

    if let Some(t2) = self.playlist_tracks.get_mut(2) {
        t2.name = "🥁 808 Drum Beat".to_string();
        t2.clips[0] = Some(0);
        t2.clips[1] = Some(0);
        t2.clips[2] = Some(0);
        t2.clips[3] = Some(0);
    }

    if let Some(t3) = self.playlist_tracks.get_mut(3) {
        t3.name = "🎹 Nexus Synth Lead".to_string();
        t3.clips[0] = Some(1);
        t3.clips[1] = Some(1);
    }

    if let Some(t4) = self.playlist_tracks.get_mut(4) {
        t4.name = "🎸 Moog Sub Bass".to_string();
        t4.clips[0] = Some(2);
        t4.clips[1] = Some(2);
    }

    // Modular graph default connections
    self.modular_graph.load_default_preset();

    // AI Assistant prompt
    self.ai_assistant.prompt_input = crate::i18n::t("Cyberpunk 80s synthwave lead med mörk rezonans i A-moll").to_string();
    self.ai_assistant.last_status = crate::i18n::t("✨ AI genererade 3 variationer av Synth Lead & Bassline").to_string();
}
}

impl SonixApp {
pub fn new_empty_project(&mut self) {
    self.stop_playback();
    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    for t in &mut self.playlist_tracks {
        t.regions.clear();
        t.clips = [None; 32];
    }
    for p in &mut self.patterns {
        p.channel_steps.fill([false; 16]);
        p.piano_roll_grid = [[false; 16]; 24];
    }
    for ch in &mut self.channels {
        ch.steps = [false; 16];
    }
    self.piano_roll_grid = [[false; 16]; 24];
    self.selected_audio_region = None;
    self.project_name = crate::i18n::t("Namnlöst Projekt").to_string();
    self.project_file_path = None;
    self.ensure_mic_track_exists();
    self.status_message = crate::i18n::t("✨ Skapade ett nytt tomt projekt!").to_string();
}
}

impl SonixApp {
pub fn load_demo_project(&mut self) {
    let progress = self.project_load_progress.clone();
    {
        if let Ok(mut p) = progress.lock() {
            p.is_loading = true;
            p.project_name = "Sonix Synthwave Demo".to_string();
            p.stage = crate::i18n::t("Läser in mönster och instrumentspår...").to_string();
            p.current_track = "Trumsektion & Kick (Electro)".to_string();
            p.current_idx = 1;
            p.total_tracks = 6;
            p.progress_ratio = 0.15;
            p.completed_payload = None;
            p.error_message = None;
        }
    }

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(140));
        if let Ok(mut p) = progress.lock() {
            p.stage = crate::i18n::t("Konfigurerar mönster & arpeggios...").to_string();
            p.current_track = "Acid Groove & Synth Lead".to_string();
            p.current_idx = 3;
            p.progress_ratio = 0.55;
        }

        std::thread::sleep(std::time::Duration::from_millis(140));
        if let Ok(mut p) = progress.lock() {
            p.stage = crate::i18n::t("Bygger tidslinje och effekter...").to_string();
            p.current_track = "303 Resonant Sub Bass".to_string();
            p.current_idx = 6;
            p.progress_ratio = 0.90;
        }

        let mut tracks = Vec::new();
        let track_defs = [
            ("🥁 Trumsektion (Electro)", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            ("💥 Kick & Punch", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            ("👏 Clap & Percussion", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            ("🧂 Hi-Hats & Shakers", TrackKind::Drums, Color32::from_rgb(255, 140, 25)),
            ("🎹 80s Poly Synth Lead", TrackKind::SynthLead, Color32::from_rgb(65, 155, 255)),
            ("🎸 303 Resonant Sub Bass", TrackKind::Bassline, Color32::from_rgb(45, 225, 105)),
            (crate::i18n::t("🎤 Mic (Voice & Sång)"), TrackKind::VocalAudio, Color32::WHITE),
        ];

        for (name, kind, _col) in track_defs {
            let clips = [None; 32];
            tracks.push(PreloadedTrackData {
                frozen: None,
                name: name.to_string(),
                volume: 0.90,
                pan: 0.0,
                muted: false,
                solo: false,
                regions: Vec::new(),
                clips,
                eq: TrackEq::default(),
                comp_threshold_db: 0.0,
                comp_ratio: 1.0,
                reverb_send: 0.15,
                delay_send: 0.10,
                automation: Vec::new(),
                bus: default_bus_for_kind(kind),
                vca: None,
                sidechain_from: None,
                sidechain_amount_db: 0.0,
                sidechain_threshold_db: -30.0,
                sends: Vec::new(),
                stem_pcms: Vec::new(),
                frozen_pcm: None,
            });
        }

        if tracks.len() >= 6 {
            for b in 0..4 { tracks[1].clips[b] = Some(1); }
            for b in 4..16 { tracks[1].clips[b] = Some(0); }
            for b in 4..8 { tracks[2].clips[b] = Some(0); }
            for b in 8..12 { tracks[2].clips[b] = Some(2); }
            for b in 12..16 { tracks[2].clips[b] = Some(0); }
            for b in 0..16 { tracks[3].clips[b] = Some(3); }
            for b in 0..8 { tracks[4].clips[b] = Some(2); }
            tracks[5].clips[7] = Some(1);
            tracks[5].clips[15] = Some(1);
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(mut p) = progress.lock() {
            p.progress_ratio = 1.0;
            p.stage = crate::i18n::t("Färdig! Laddar Synthwave Demo...").to_string();
            p.completed_payload = Some(LoadedProjectPayload {
                name: "Sonix Synthwave Demo".to_string(),
                bpm: 126.0,
                follow_tempo: true,
                swing: 0.15,
                tempo_points: Vec::new(),
                song_key_root: 3,
                song_key_scale: 0,
                master_volume: 0.90,
                master_pan: 0.0,
                tracks,
                plugin_slots: Vec::new(),
                bus_volume: default_bus_volume(),
                bus_muted: [false; crate::audio::synth::NUM_BUSES],
                bus_automation: Vec::new(),
                plugin_automation: Vec::new(),
                bus_solo: [false; crate::audio::synth::NUM_BUSES],
                vca_volume: default_vca_volume(),
                vca_muted: [false; crate::audio::synth::NUM_VCAS],
                vca_solo: [false; crate::audio::synth::NUM_VCAS],
                patterns: Vec::new(),
                selected_pattern: 0,
                step_velocities: None,
                channels: Vec::new(),
                file_path: "demo".to_string(),
            });
        }
    });
}
}

impl SonixApp {
/// Bygger projektets serialiserbara form.
///
/// Delas av manuell sparning och autosave. Om de två byggde sina egna
/// objekt kunde de glida isär, och då skulle autosaven skydda något annat
/// än det användaren faktiskt sparar.
pub(crate) fn project_data(&self, name: &str) -> SonixProjectData {
    let saved_tracks: Vec<SavedTrackData> = self.playlist_tracks.iter().map(|t| SavedTrackData {
        name: t.name.clone(),
        volume: t.volume,
        pan: t.pan,
        muted: t.muted,
        solo: t.solo,
        clips: t.clips,
        regions: t.regions.clone(),
        frozen: t.frozen.as_ref().map(|f| SavedFrozenTrack {
            path: f.path.clone(),
            digest: f.digest,
            stamp: f.stamp,
        }),
        eq: t.eq.clone(),
        comp_threshold_db: t.comp_threshold_db,
        comp_ratio: t.comp_ratio,
        reverb_send: t.reverb_send,
        delay_send: t.delay_send,
        // Punkterna skrivs i **takter** (Fas 8.2). Gamla projekt läses genom
        // `AutomationLaneOnDisk::to_live`, så en fil som redan är skriven behåller sitt
        // innehåll — men nya filer bär bara takter, och bara en gång.
        automation: t.automation.iter().map(|l| l.to_disk()).collect(),
        bus: t.bus,
        vca: t.vca,
        sidechain_from: t.sidechain_from,
        sidechain_amount_db: t.sidechain_amount_db,
        sidechain_threshold_db: t.sidechain_threshold_db,
        sends: t.sends.clone(),
    }).collect();

    SonixProjectData {
        name: name.to_string(),
        bpm: self.bpm,
        follow_tempo: self.follow_tempo,
        tempo_points: self.tempo_points.clone(),
        song_key_root: self.song_key_root,
        song_key_scale: self.song_key_scale,
        swing: self.swing,
        master_volume: self.master_volume,
        master_pan: self.master_pan,
        tracks: saved_tracks,
        bus_volume: self.bus_volume,
        bus_muted: self.bus_muted,
        bus_automation: self.bus_automation.clone(),
        plugin_automation: self.plugin_automation.clone(),
        bus_solo: self.bus_solo,
        vca_volume: self.vca_faders,
        vca_muted: self.vca_muted,
        vca_solo: self.vca_solos,
        patterns: self.patterns.iter().map(pattern_to_saved).collect(),
        selected_pattern: self.selected_pattern,
        step_velocities: Some(self.step_velocities),
        channels: self.channels.iter().map(channel_to_saved).collect(),
        plugin_slots: self
            .plugin_slots
            .iter()
            .map(|slot| {
                slot.as_ref().map(|s| SavedPluginData {
                    path: s.path.clone(),
                    name: s.name.clone(),
                    state: s.state.clone(),
                    sandboxed: s.sandboxed,
                    latency_offset_frames: s.latency_offset_frames,
                    smart_disable: s.smart_disable,
                    extra_out_targets: s.extra_out_targets.clone(),
                })
            })
            .collect(),
    }
}
}

impl SonixApp {
pub fn save_project(&mut self, name: &str) {
    let clean_name = if name.trim().is_empty() {
        crate::i18n::t("Namnlöst Projekt")
    } else {
        name.trim()
    };
    let dir = crate::paths::paths().projects_dir();
    let _ = std::fs::create_dir_all(&dir);

    let file_path = crate::paths::paths().project_file(clean_name);
    let data = self.project_data(clean_name);

    if let Ok(json) = serde_json::to_string_pretty(&data)
        // Temp + rename: en avbruten skrivning får aldrig ersätta en hel
        // projektfil med en halv.
        && crate::autosave::write_atomic(&file_path, json.as_bytes()).is_ok() {
            self.project_name = clean_name.to_string();
            self.project_file_path = Some(file_path.to_string_lossy().to_string());
            // Kom ihåg läget, så autosaven inte skriver en kopia av exakt
            // det som just sparades.
            self.autosave_last_fp = Some(crate::autosave::fingerprint(json.as_bytes()));
            self.remember_recent_project(clean_name, &file_path);
            self.status_message = crate::tstatus!("💾 Sparade projekt till '{}'!", file_path.display());
            return;
        }
    self.status_message = crate::i18n::t("❌ Misslyckades att spara projektet.").to_string();
}
}

impl SonixApp {
/// Lägger projektet först i läslistan (`~/.local/state/sonix/recent.json`).
fn remember_recent_project(&self, name: &str, path: &std::path::Path) {
    let mut entries = load_recent_projects();
    push_recent_project(&mut entries, name, &path.to_string_lossy());
    store_recent_projects(&entries);
}
}

impl SonixApp {
/// Autosparar om projektet ändrats sedan det senast skyddades.
///
/// Tre grindar innan en fil skrivs: läget får inte vara identiskt med förra
/// autosaven, inte heller med filen på disk (då finns inget osparat), och
/// skrivningen sker atomiskt i state-katalogen. Varje skrivning roterar
/// historiken så att de senaste versionerna finns kvar.
pub fn maybe_autosave(&mut self) {
    // Under en pågående projektinläsning är state halvfärdigt — att frysa
    // det som en autosave vore att skydda fel läge.
    if self
        .project_load_progress
        .try_lock()
        .map(|p| p.is_loading)
        .unwrap_or(false)
    {
        return;
    }

    let name = self.project_name.clone();
    let data = self.project_data(&name);
    let Ok(json) = serde_json::to_string_pretty(&data) else {
        return;
    };
    let bytes = json.as_bytes();
    let fp = crate::autosave::fingerprint(bytes);

    if self.autosave_last_fp == Some(fp) {
        return;
    }
    if let Some(path) = self.project_file_path.as_deref()
        && std::fs::read(path)
            .map(|disk| crate::autosave::fingerprint(&disk) == fp)
            .unwrap_or(false)
    {
        // Identiskt med det användaren redan sparat — inget att skydda.
        self.autosave_last_fp = Some(fp);
        return;
    }

    let dir = crate::paths::paths().autosave_dir();
    let stamp = crate::autosave::now_stamp();
    match crate::autosave::save(&dir, &name, bytes, stamp) {
        Ok(path) => {
            let removed = crate::autosave::prune(&dir, crate::autosave::KEEP_PER_PROJECT)
                .unwrap_or(0);
            self.autosave_last_fp = Some(fp);
            self.autosave_last_write = Some(stamp);
            // Autosaven är en bakgrundshändelse och får inte skriva över en
            // färsk ångringsbekräftelse. Mätt i GUI: ångringen märker
            // ändringen för autosave, autosaven skriver nästa frame och
            // ersatte "↶ Ångrade …" inom millisekunder — så användaren kunde
            // inte läsa vad som just ångrades.
            let fresh_undo = self.status_message.starts_with("↶ ")
                || self.status_message.starts_with("↷ ");
            if !fresh_undo {
                self.status_message = if removed > 0 {
                    crate::tstatus!(
                        "🛟 Autosparade '{}' ({} äldre versioner städade)",
                        name,
                        removed
                    )
                } else {
                    crate::tstatus!("🛟 Autosparade '{}'", name)
                };
            }
            let _ = path;
        }
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte autospara: {}", e);
        }
    }
}
}

impl SonixApp {
/// Autosparar direkt efter en strukturell ändring (Fas 6.1: "vid varje
/// strukturell ändring"), men högst var tionde sekund — annars skulle en
/// jämn ström av undo-punkter skriva en fil per steg.
pub fn autosave_after_structural_change(&mut self) {
    let now = crate::autosave::now_stamp();
    if let Some(last) = self.autosave_last_write
        && now.saturating_sub(last) < 10
    {
        return;
    }
    self.maybe_autosave();
}
}

impl SonixApp {
/// Återställer en autosave från återställningsdialogen.
///
/// Filen *pensioneras* (döps om till `*.restored`) i stället för att
/// raderas: frågan ska inte ställas igen nästa start, men det återställda
/// läget ska gå att gräva fram om användaren ångrar sig.
pub fn restore_autosave(&mut self, index: usize) {
    let Some(candidate) = self.recovery_candidates.get(index).cloned() else {
        self.show_recovery_modal = false;
        return;
    };
    let path = candidate.entry.path.clone();
    self.recovery_candidates.clear();
    self.show_recovery_modal = false;
    match crate::autosave::retire(&path) {
        // Sökvägen som retire LÄMNAR TILLBAKA är den filen nu finns på. Att läsa
        // den gamla — vilket koden gjorde — kan aldrig fungera: retire har just
        // döpt om den. Alex' "Restore gör inget" var precis detta: filen blev
        // `*.restored`, och läste man sedan det gamla namnet fanns inget där.
        Ok(retired) => {
            self.load_project_file(retired.to_string_lossy().as_ref());
            self.autosave_accum = 0.0;
            self.autosave_last_fp = None;
            self.status_message = crate::tstatus!(
                "🛟 Återställde autosave för '{}' — granska och spara projektet",
                candidate.name
            );
        }
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte återställa autosaven: {}", e);
        }
    }
}
}

impl SonixApp {
pub fn load_project_file(&mut self, path_str: &str) {
    let path = path_str.to_string();
    let progress = self.project_load_progress.clone();

    {
        if let Ok(mut p) = progress.lock() {
            p.is_loading = true;
            p.project_name = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| crate::i18n::t("Projekt").to_string());
            p.stage = crate::i18n::t("Läser projektfil och förbereder spår...").to_string();
            p.current_track = String::new();
            p.current_idx = 0;
            p.total_tracks = 0;
            p.progress_ratio = 0.05;
            p.completed_payload = None;
            p.error_message = None;
        }
    }

    std::thread::spawn(move || {
        // Small initial pause for UI modal display
        std::thread::sleep(std::time::Duration::from_millis(80));

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                if let Ok(mut p) = progress.lock() {
                    p.is_loading = false;
                    p.error_message = Some(crate::tstatus!("Kunde inte läsa filen: {}", e));
                }
                return;
            }
        };

        let data = match serde_json::from_str::<SonixProjectData>(&content) {
            Ok(d) => d,
            Err(e) => {
                if let Ok(mut p) = progress.lock() {
                    p.is_loading = false;
                    p.error_message = Some(format!("Ogiltigt projektformat: {}", e));
                }
                return;
            }
        };

        let total_tracks = data.tracks.len();
        // Ett tempobyte gör takt-till-sekunder beroende av hela kartan, och då är
        // mätningen av ett klipps inspelningstempo (nedan) inte ett mått på
        // något. Enkel karta = formeln gäller.
        let simple_tempo_map = data.tempo_points.is_empty();
        let project_bpm = data.bpm;
        let mut preloaded_tracks = Vec::with_capacity(total_tracks);
        // Problem med frusna filer samlas och sägs en gång efter inläsningen:
        // förut skrev varje problem över det förra i samma fält, så bara den
        // sista filen nämndes.
        let mut frozen_problems: Vec<String> = Vec::new();

        for (t_idx, st) in data.tracks.into_iter().enumerate() {
            let track_name = st.name.clone();
            let ratio = 0.10 + ((t_idx + 1) as f32 / total_tracks.max(1) as f32) * 0.85;

            // Ett fruset spår har sitt ljud i en fil i projektets egen mapp.
            // Filen måste finnas **och** gå att läsa: en frysning vars fil
            // försvunnit eller inte kan avkodas är inte samma sak som ett
            // ofruset spår, och det ska sägas högt i stället för att tyst
            // spela något annat. "Filen saknas" och "filen går inte att läsa"
            // är samma besked för den som står vid datorn — förut fick bara
            // den första av dem ett ord.
            let frozen_pcm = match st.frozen.as_ref() {
                Some(f) => match load_audio_or_report(&f.path) {
                    Some((pcm_l, pcm_r, sr)) => Some((
                        std::sync::Arc::new(pcm_l),
                        std::sync::Arc::new(pcm_r),
                        sr,
                    )),
                    None => {
                        let why = if std::path::Path::new(&f.path).exists() {
                            crate::i18n::t("filen går inte att läsa")
                        } else {
                            crate::i18n::t("filen saknas")
                        };
                        frozen_problems.push(crate::tstatus!(
                            "'{}' är fruset men {}: {}",
                            st.name,
                            why,
                            f.path
                        ));
                        None
                    }
                },
                None => None,
            };
            if let Ok(mut p) = progress.lock() {
                p.stage = crate::tstatus!("Läser in och avkodar ljudspår ({}/{})...", t_idx + 1, total_tracks);
                p.current_track = track_name.clone();
                p.current_idx = t_idx + 1;
                p.total_tracks = total_tracks;
                p.progress_ratio = ratio;
            }

            let mut stem_pcms = Vec::new();
            let mut regions = st.regions;
            for r in regions.iter_mut() {
                if let Some(ref p_src) = r.source_path
                    && let Some((pcm_l, pcm_r, sr)) = load_audio_or_report(p_src)
                {
                    // Gamla projekt sparades utan inspelningstempo (Fas 8.10).
                    // Här finns både filens längd och klossens takter — alltså
                    // går måttet att räkna, och det sätts **bara** när det
                    // stämmer med projektets tempo. Annars står 0,0 kvar och
                    // ljudet rörs inte när tempot ändras: att gissa vore att
                    // hitta på data, och måttet är härlett ur den här filen —
                    // inte ur ett annat klipps ljud.
                    if simple_tempo_map {
                        let source_secs = pcm_l.len() as f32 / sr.max(1) as f32;
                        r.source_bpm = source_bpm_from_region(r, source_secs, project_bpm);
                    }
                    stem_pcms.push((std::sync::Arc::new(pcm_l), std::sync::Arc::new(pcm_r), sr));
                }
            }
            // Tyst bortfall är det som gör en sådan här sak osynlig. Rapporten
            // sker nu på ett ställe (take_unreadable_sources) i stället för här,
            // så att alla elva anrop omfattas — inte bara projektinläsningen.
            // Ett fruset spår har sitt ljud i en fil i projektets mapp. Att
            // lägga den i stem_pcms gör att samma väg som ljudspåren används
            // — både vid inläsning och vid en senare omsynk. Bara när filen
            // gick att läsa: annars får spåret inget fruset ljud alls.
            if let Some(pcm) = frozen_pcm.clone() {
                stem_pcms.push(pcm);
            }

            preloaded_tracks.push(PreloadedTrackData {
                frozen: st.frozen,
                name: st.name,
                volume: st.volume,
                pan: st.pan,
                muted: st.muted,
                solo: st.solo,
                regions,
                clips: st.clips,
                eq: st.eq,
                comp_threshold_db: st.comp_threshold_db,
                comp_ratio: st.comp_ratio,
                reverb_send: st.reverb_send,
                delay_send: st.delay_send,
                automation: st.automation,
                bus: st.bus,
                vca: st.vca,
                sidechain_from: st.sidechain_from,
                sidechain_amount_db: st.sidechain_amount_db,
                sidechain_threshold_db: st.sidechain_threshold_db,
                sends: st.sends,
                stem_pcms,
                frozen_pcm,
            });

            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if !frozen_problems.is_empty()
            && let Ok(mut p) = progress.lock()
        {
            let list = frozen_problems.join(" · ");
            p.error_message = Some(if frozen_problems.len() == 1 {
                crate::tstatus!("en frusen fil kunde inte användas: {}", list)
            } else {
                crate::tstatus!(
                    "{} frusna filer kunde inte användas: {}",
                    frozen_problems.len(),
                    list
                )
            });
        }

        if let Ok(mut p) = progress.lock() {
            p.progress_ratio = 1.0;
            p.stage = crate::i18n::t("Färdigställer tidslinje och ljudmotor...").to_string();
            p.completed_payload = Some(LoadedProjectPayload {
                name: data.name,
                bpm: data.bpm,
                follow_tempo: data.follow_tempo,
                tempo_points: data.tempo_points,
                song_key_root: 3,
                song_key_scale: 0,
                swing: data.swing,
                master_volume: data.master_volume,
                master_pan: data.master_pan,
                tracks: preloaded_tracks,
                plugin_slots: data.plugin_slots,
                bus_volume: data.bus_volume,
                bus_muted: data.bus_muted,
                // **Kurvorna följde inte med genom förinläsningen** (rättat 2026-09-14):
                // `bus_automation` fanns både i filen och i payloaden, men byggdes här med en
                // tom lista — så en laddad busskurva försvann tyst, medan sparningen skrev den.
                // Ett fält som finns på båda sidor är inte samma sak som ett fält som kopplats
                // ihop.
                bus_automation: data.bus_automation,
                plugin_automation: data.plugin_automation,
                bus_solo: data.bus_solo,
                vca_volume: data.vca_volume,
                vca_muted: data.vca_muted,
                vca_solo: data.vca_solo,
                patterns: data.patterns,
                selected_pattern: data.selected_pattern,
                step_velocities: data.step_velocities,
                channels: data.channels,
                file_path: path,
            });
        }
    });
}
}

impl SonixApp {
pub fn apply_loaded_project_payload(&mut self, payload: LoadedProjectPayload) {
    self.stop_playback();
    let _ = self.engine.send_command(AudioCommand::ClearAllStemTracks);
    // The old plugins are being replaced: close their editors and hold the
    // shared handles until shutdown so the cores are not freed on the audio
    // thread while it may still be rendering the previous graph.
    self.close_all_plugin_guis();
    self.retire_all_plugin_handles();

    let plugin_slots = payload.plugin_slots;
    let bus_volume = payload.bus_volume;
    let bus_muted = payload.bus_muted;
    let bus_automation = payload.bus_automation;
    let plugin_automation = payload.plugin_automation;
    let bus_solo = payload.bus_solo;
    let vca_volume = payload.vca_volume;
    let vca_muted = payload.vca_muted;
    let vca_solo = payload.vca_solo;
    let patterns = payload.patterns;
    let selected_pattern = payload.selected_pattern;
    let step_velocities = payload.step_velocities;
    let channels = payload.channels;

    self.project_name = payload.name;
    self.bpm = payload.bpm;
    self.follow_tempo = payload.follow_tempo;
    self.tempo_points = payload.tempo_points;
    self.song_key_root = payload.song_key_root;
    self.song_key_scale = payload.song_key_scale;
    self.swing = payload.swing;
    self.master_volume = payload.master_volume;
    self.master_pan = payload.master_pan;
    self.bus_volume = bus_volume;
    self.bus_automation = bus_automation;
    self.bus_muted = bus_muted;
    self.bus_solo = bus_solo;
    self.vca_faders = vca_volume;
    self.vca_muted = vca_muted;
    self.vca_solos = vca_solo;

    // Mönstren, kanalracket och stegvolymerna (Fas 6.7).
    restore_saved_music(
        &SavedMusic {
            patterns: &patterns,
            selected_pattern,
            step_velocities,
            channels: &channels,
        },
        &mut self.patterns,
        &mut self.channels,
        &mut self.selected_pattern,
        &mut self.step_velocities,
    );
    self.sync_group_state();

    self.playlist_tracks.clear();
    for (t_idx, st) in payload.tracks.into_iter().enumerate() {
        // Send preloaded PCM to engine, and keep a reference on the track so
        // a later ClearAllStemTracks + resync (duplicate/delete/reload) can
        // re-send it. Without this the app loses the PCM after project load.
        let mut track_pcm = None;
        for (pcm_l, pcm_r, sr) in st.stem_pcms {
            track_pcm = Some((pcm_l.clone(), pcm_r.clone(), sr));
            let _ = self.engine.send_command(AudioCommand::LoadStemTrack {
                track_index: t_idx,
                left: pcm_l,
                right: pcm_r,
                sample_rate: sr as f32,
                volume: st.volume,
                pan: st.pan,
                start_time_secs: 0.0,
            });
        }

        let (kind, icon, color) = classify_track_style(&st.name);
        let mut loaded_track = PlaylistTrack::new(st.name, icon, kind, color);
        loaded_track.volume = st.volume;
        loaded_track.pan = st.pan;
        loaded_track.muted = st.muted;
        loaded_track.solo = st.solo;
        loaded_track.regions = st.regions;
        loaded_track.clips = st.clips;
        if st.frozen.is_some() {
            // Ljudet kom in via stem_pcms ovan; här knyts läget till spåret.
            loaded_track.frozen = st.frozen.map(|f| FrozenTrack {
                path: f.path,
                digest: f.digest,
                stamp: f.stamp,
            });
            // Bara det **frusna** ljudet får bli `frozen_pcm` (Fas 8.5). Förut
            // ärvde spåret `track_pcm`, som är det sista som lades i stem_pcms
            // och alltså kan vara spårets eget klippljud: ett spår som ser
            // fruset ut spelade då sin ofrusna mix — något annat än det som
            // renderades, och i värsta fall tyst. `None` här betyder att
            // motorn och exporten faller tillbaka på spårets egna klipp.
            loaded_track.frozen_pcm = st.frozen_pcm.clone();
        }
        loaded_track.eq = st.eq;
        loaded_track.comp_threshold_db = st.comp_threshold_db;
        loaded_track.comp_ratio = st.comp_ratio;
        loaded_track.reverb_send = st.reverb_send;
        loaded_track.delay_send = st.delay_send;
        // **Omräkningen sker här** (Fas 8.2), inte i `Deserialize`: en punkt skriven i
        // sekunder kan bara bli rätt takt med tempokartan, och kartan finns inte i filen.
        // Projektets eget tempo är laddat vid det här laget, så en gammal kurva hamnar på
        // samma ställe i musiken som den lät på.
        let tempo = self.tempo_map();
        loaded_track.automation = st
            .automation
            .iter()
            .map(|l| l.to_live(&tempo))
            .collect();
        loaded_track.bus = st.bus.min(crate::audio::synth::NUM_BUSES - 1);
        loaded_track.vca = st.vca.filter(|&v| v < crate::audio::synth::NUM_VCAS);
        // Sidokedjan får bara peka på ett spår som finns — ett sparat projekt
        // kan ha färre spår än när det skrevs.
        loaded_track.sidechain_from = st
            .sidechain_from
            .filter(|&k| k < t_idx && k < self.playlist_tracks.len());
        loaded_track.sends = st
            .sends
            .into_iter()
            .filter(|s| s.level.is_finite() && s.level.abs() > 1e-6)
            .map(|s| crate::audio::StemSend {
                target: match s.target {
                    crate::audio::SendTarget::Bus { target_bus } => {
                        crate::audio::SendTarget::bus(
                            target_bus.min(crate::audio::synth::NUM_BUSES - 1),
                        )
                    }
                    // Ett spårmål behålls orört: spåret kan komma senare i filen.
                    other => other,
                },
                level: s.level.clamp(0.0, 2.0),
            })
            .collect();
        loaded_track.sidechain_amount_db = st.sidechain_amount_db.clamp(0.0, 60.0);
        loaded_track.sidechain_threshold_db = st.sidechain_threshold_db.clamp(-80.0, 0.0);
        loaded_track.pcm_audio = track_pcm;
        self.playlist_tracks.push(loaded_track);
        self.sync_track_regions(t_idx);
    }

    // Ensure Mic track is always at the end & all track colors are properly classified
    self.ensure_mic_track_exists();

    // **Plugin-kurvorna knyts till spåren när spåren finns** (Fas 8.8). Raden ligger *efter*
    // spårbygget med flit: `playlist_tracks.clear()` sker ovanför, så en kontroll här mot en
    // halvfärdig lista hade släppt kurvor som hörde till det nya projektet och behållit sådana
    // som hörde till det förra. En kurva vars spår inte finns kvar släpps — den kan inte styra
    // något, och att behålla den hade sett levande ut utan att vara det.
    let track_count = self.playlist_tracks.len();
    self.plugin_automation = plugin_automation
        .into_iter()
        .filter(|l| l.track < track_count)
        .collect();

    // Re-instantiate per-track plugins and restore their state on the main
    // thread (clap.state is main-thread only, so it must not go through the
    // audio thread). Slot indices map 1:1 onto engine stem tracks.
    self.plugin_slots = plugin_slots
        .iter()
        .map(|slot| {
            slot.as_ref().map(|s| PluginSlot {
                path: s.path.clone(),
                name: s.name.clone(),
                state: s.state.clone(),
                sandboxed: s.sandboxed,
                latency_offset_frames: s.latency_offset_frames,
                smart_disable: s.smart_disable,
                extra_out_targets: s.extra_out_targets.clone(),
            })
        })
        .collect();
    let mut plugin_errors: Vec<String> = Vec::new();
    for (track_index, slot) in plugin_slots.iter().enumerate() {
        if let Some(data) = slot
            && let Some(err) = self.restore_plugin_slot(track_index, data)
        {
            plugin_errors.push(err);
        }
        // **Offsetet följer med till motorn** (Fas 8.6). Att bara skriva det i sloten hade
        // gjort det synligt i gränssnittet och i den sparade filen medan motorn körde utan
        // kompensation — projektfilen hade sett riktig ut och ljudet varit fel. Nollställs
        // även för spår **utan** plugin, så en borttagen plugin inte lämnar ett offset kvar
        // på ett spår som inte längre har något att kompensera.
        let frames = slot.as_ref().map(|s| s.latency_offset_frames).unwrap_or(0);
        let _ = self
            .engine
            .send_command(AudioCommand::SetPluginLatencyOffset { track_index, frames });
        // Smart disable (Fas 8.6) går samma väg, av samma skäl: flaggan bor i insertet på
        // ljudtråden, så den måste skickas dit — att bara spara den i sloten hade gett en
        // projektfil som ser rätt ut medan pluginen processar hela tiden.
        let enabled = slot.as_ref().map(|s| s.smart_disable).unwrap_or(false);
        let _ = self
            .engine
            .send_command(AudioCommand::SetPluginSmartDisable { track_index, enabled });
        // **Utbussarna till motorn** (Fas 8.6), av exakt samma skäl som offsetet ovan: en
        // koppling som bara bor i sloten syns i gränssnittet och i filen medan motorn inte
        // routar något. Tom lista för spår utan plugin, så en borttagen plugin inte lämnar
        // kvar en koppling i motorn — samma regel som offsetet fick.
        let targets = slot
            .as_ref()
            .map(|s| s.extra_out_targets.clone())
            .unwrap_or_default();
        let _ = self
            .engine
            .send_command(AudioCommand::SetPluginExtraOutputs { track_index, targets });
    }

    self.loop_end_bar = self.get_max_project_bars().max(32);
    self.project_file_path = Some(payload.file_path);
    // Öppnade projekt hör till läslistan precis som sparade (Fas 6.1).
    if let Some(path) = self.project_file_path.clone() {
        let name = self.project_name.clone();
        self.remember_recent_project(&name, std::path::Path::new(&path));
    }
    // Nytt läge: nästa autosave-kontroll jämför mot filen på disk i stället
    // för mot det förra projektets fingeravtryck.
    self.autosave_last_fp = None;
    self.autosave_accum = 0.0;
    self.selected_audio_region = None;
    let opened_name = self.project_name.clone();
    let opened = if plugin_errors.is_empty() {
        crate::tstatus!("📂 Öppnade projekt '{}'!", opened_name)
    } else {
        crate::tstatus!(
            "📂 Öppnade projekt '{}' – ⚠ {} plugin(s) kunde inte återställas",
            opened_name,
            plugin_errors.len()
        )
    };
    // **Säg det vid inläsningen, inte först när tempot rörs** (Fas 8.10).
    //
    // Ett projekt vars klipp saknar känt inspelningstempo ser ut som ett projekt där
    // tempokontrollen är död: rutnätet går i projektets tempo medan ljudet står still.
    // Alex mötte precis det — ett återställt tempo på 68 BPM (hans eget drag) och nio
    // klipp utan mått — och fick ingen ledtråd förrän han bytte tempo. En rad vid
    // inläsningen hade sagt det direkt.
    self.status_message = match tempo_change_note(
        self.clips_following_tempo(),
        self.clips_standing_still(),
        self.clips_too_far_from_tempo(),
        self.geometry_tempo_for_unknown_clips(),
    ) {
        Some(note) => crate::tstatus!("{} {}", opened, note),
        None => opened,
    };
}
}

