//! Plugin-värdarna i UI:t — ladda, GUI-fönster, slots och sandlådan.
//!
//! En plugin får aldrig kunna tysta eller krascha motorn: sandlådevägen (4.5) kör i egen
//! process, och en slot som inte kan återställas rapporterar i stället för att tiga.
//!
//! Status: byggs — plugin-värdarna.
//! Rör inte: feature-grinden `plugin-host`; sandlådans väg får inte bli standardvägen.

use super::*;

impl SonixApp {
/// Instantiates a CLAP plugin and sets it as the insert on a stem track
/// (Fas 4.2). Runs on the UI thread; the live processor is moved to the
/// audio thread through the command ring.
pub fn load_plugin_into_track(&mut self, path: &str, track_index: usize) {
    let sample_rate = self.engine.sample_rate as f32;
    let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
    match crate::audio::plugin_host_live::load_processor(path, sample_rate, block as u32) {
        Ok(processor) => {
            let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
            let name = insert.info().name.clone();
            // Capture the fresh state now, on the main thread (clap.state is
            // main-thread only), so a project save can restore it.
            let state = insert.save_state();
            let handle = insert.core_handle();
            self.retire_plugin_handle(track_index);
            self.record_plugin_slot(
                track_index,
                PluginSlot {
                    path: path.to_string(),
                    name: name.clone(),
                    state,
                    sandboxed: false,
                    latency_offset_frames: 0,
                    smart_disable: false,
                    extra_out_targets: Vec::new(),
                },
            );
            self.ensure_plugin_vecs(track_index);
            self.plugin_handles[track_index] = handle;
            let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                track_index,
                insert: Some(insert),
            });
            self.plugin_manager.instantiated_plugin = Some(name.clone());
            self.status_message = crate::tstatus!(
                "✔ {} laddad som insert på stämspår {} (PDC-kompenserad)",
                name,
                track_index + 1
            );
        }
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte ladda plugin: {}", e);
        }
    }
}
}

impl SonixApp {
/// Instantiates `path` into `track_index` with a native preset applied via
/// `clap.preset-load/2`, then captures the resulting state for the project.
pub fn load_plugin_preset_into_track(&mut self, path: &str, track_index: usize, location: &str) {
    let sample_rate = self.engine.sample_rate as f32;
    let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
    match crate::audio::plugin_host_live::load_processor(path, sample_rate, block as u32) {
        Ok(processor) => {
            let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
            let name = insert.info().name.clone();
            if !insert.preset_load(location) {
                self.status_message = crate::tstatus!(
                    "⚠ Pluginen '{}' stödjer inte clap.preset-load/2 (eller avvisade preseten)",
                    name
                );
                return;
            }
            let state = insert.save_state();
            let handle = insert.core_handle();
            self.retire_plugin_handle(track_index);
            self.record_plugin_slot(
                track_index,
                PluginSlot {
                    path: path.to_string(),
                    name: name.clone(),
                    state,
                    sandboxed: false,
                    latency_offset_frames: 0,
                    smart_disable: false,
                    extra_out_targets: Vec::new(),
                },
            );
            self.ensure_plugin_vecs(track_index);
            self.plugin_handles[track_index] = handle;
            let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                track_index,
                insert: Some(insert),
            });
            self.plugin_manager.instantiated_plugin = Some(name.clone());
            self.status_message = crate::tstatus!(
                "✔ {} laddad med preset på stämspår {} (PDC-kompenserad)",
                name,
                track_index + 1
            );
        }
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte ladda plugin: {}", e);
        }
    }
}
}

impl SonixApp {
/// Removes the plugin insert on `track_index` and forgets its saved slot.
pub fn remove_plugin_from_track(&mut self, track_index: usize) {
    self.retire_plugin_handle(track_index);
    #[cfg(feature = "plugin-host")]
    self.remove_plugin_sandbox(track_index);
    let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
        track_index,
        insert: None,
    });
    if let Some(slot) = self.plugin_slots.get_mut(track_index) {
        *slot = None;
    }
    // **Offsetet nollas i motorn när pluginen tas bort** (Fas 8.6). Det bor på spåret, så
    // utan den här raden hade ett offset utan plugin fortsatt skjuta spåret mot de andra —
    // och eftersom spåret då inte har något som fördröjer det vore förskjutningen ren.
    let _ = self.engine.send_command(AudioCommand::SetPluginLatencyOffset {
        track_index,
        frames: 0,
    });
    self.plugin_manager.instantiated_plugin = None;
    self.status_message = crate::tstatus!(
        "🗑 Tog bort plugin från stämspår {}",
        track_index + 1
    );
}
}

impl SonixApp {
/// Instantiates `path` in an out-of-process sandbox and streams its audio
/// over shared memory (Fas 4.5b). The plugin's GUI is not available for
/// sandboxed instances.
#[cfg(feature = "plugin-host")]
pub fn load_plugin_into_sandbox_track(&mut self, path: &str, track_index: usize) {
    use crate::audio::plugin_sandbox::{
        SandboxHost, SandboxProcessor, SandboxRequest, SandboxResponse, info_from_dto,
        param_from_dto,
    };
    use crate::audio::sandbox_audio::{AudioBridge, DEFAULT_SLOTS};

    let sample_rate = self.engine.sample_rate as f32;
    let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;

    // Replace any previous plugin (in-process or sandboxed) on this track.
    self.retire_plugin_handle(track_index);
    self.remove_plugin_sandbox(track_index);

    let bridge = match AudioBridge::create(block, DEFAULT_SLOTS) {
        Ok(bridge) => bridge,
        Err(err) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte skapa ljudbryggan: {}", err);
            return;
        }
    };
    let mut host = match SandboxHost::new_audio(path, sample_rate, block as u32, bridge.raw_fd())
    {
        Ok(host) => host,
        Err(err) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte skapa sandbox: {}", err);
            return;
        }
    };
    let outcome = (|| -> Result<(crate::audio::plugin_host_live::PluginInfo, Vec<crate::audio::plugin_host_live::PluginParameter>, u32, Vec<u8>), String> {
        host.spawn().map_err(|e| e.to_string())?;
        let info = match host.request(&SandboxRequest::Info).map_err(|e| e.to_string())? {
            SandboxResponse::Info { info } => info_from_dto(&info),
            SandboxResponse::Error { message } => return Err(message),
            other => return Err(format!("oväntat svar: {other:?}")),
        };
        let parameters = match host
            .request(&SandboxRequest::Parameters)
            .map_err(|e| e.to_string())?
        {
            SandboxResponse::Parameters { parameters } => {
                parameters.iter().map(param_from_dto).collect()
            }
            _ => Vec::new(),
        };
        let latency = match host
            .request(&SandboxRequest::Latency)
            .map_err(|e| e.to_string())?
        {
            SandboxResponse::Latency { frames } => frames,
            _ => 0,
        };
        let state = match host
            .request(&SandboxRequest::SaveState)
            .map_err(|e| e.to_string())?
        {
            SandboxResponse::State { data } => data,
            _ => Vec::new(),
        };
        Ok((info, parameters, latency, state))
    })();

    match outcome {
        Ok((info, parameters, latency, state)) => {
            let name = info.name.clone();
            let processor = SandboxProcessor::new(bridge, info, parameters, latency);
            let insert =
                crate::audio::plugin_host_live::PluginInsert::new(Box::new(processor), block);
            self.record_plugin_slot(
                track_index,
                PluginSlot {
                    path: path.to_string(),
                    name: name.clone(),
                    state,
                    sandboxed: true,
                    latency_offset_frames: 0,
                    smart_disable: false,
                    extra_out_targets: Vec::new(),
                },
            );
            self.ensure_plugin_sandboxes(track_index);
            self.plugin_sandboxes[track_index] = Some(host);
            let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                track_index,
                insert: Some(insert),
            });
            self.plugin_manager.instantiated_plugin = Some(name.clone());
            self.status_message = crate::tstatus!(
                "🧪 {} laddad i sandbox på stämspår {} (separat process, PDC-kompenserad)",
                name,
                track_index + 1
            );
        }
        Err(err) => {
            self.status_message = crate::tstatus!("⚠ Sandbox misslyckades: {}", err);
            host.shutdown();
        }
    }
}
}

impl SonixApp {
#[cfg(feature = "plugin-host")]
fn ensure_plugin_sandboxes(&mut self, track_index: usize) {
    if self.plugin_sandboxes.len() <= track_index {
        self.plugin_sandboxes.resize_with(track_index + 1, || None);
    }
}
}

impl SonixApp {
#[cfg(feature = "plugin-host")]
fn remove_plugin_sandbox(&mut self, track_index: usize) {
    if let Some(mut host) = self
        .plugin_sandboxes
        .get_mut(track_index)
        .and_then(|slot| slot.take())
    {
        host.shutdown();
    }
}
}

impl SonixApp {
fn ensure_plugin_vecs(&mut self, track_index: usize) {
    if self.plugin_handles.len() <= track_index {
        self.plugin_handles.resize_with(track_index + 1, || None);
    }
    if self.plugin_gui_sessions.len() <= track_index {
        self.plugin_gui_sessions.resize_with(track_index + 1, || None);
    }
}
}

impl SonixApp {
/// Closes the GUI and moves the track's shared handle to the retirement
/// list so the instance is not destroyed on the audio thread.
fn retire_plugin_handle(&mut self, track_index: usize) {
    if let Some(slot) = self.plugin_gui_sessions.get_mut(track_index) {
        *slot = None;
    }
    if let Some(handle) = self.plugin_handles.get_mut(track_index).and_then(|h| h.take()) {
        self.retired_plugin_handles.push(handle);
    }
}
}

impl SonixApp {
pub(crate) fn close_all_plugin_guis(&mut self) {
    for slot in self.plugin_gui_sessions.iter_mut() {
        *slot = None;
    }
}
}

impl SonixApp {
pub(crate) fn retire_all_plugin_handles(&mut self) {
    for slot in self.plugin_handles.iter_mut() {
        if let Some(handle) = slot.take() {
            self.retired_plugin_handles.push(handle);
        }
    }
}
}

impl SonixApp {
/// Whether a plugin GUI is currently open for `track_index`.
pub fn is_plugin_gui_open(&self, track_index: usize) -> bool {
    self.plugin_gui_sessions
        .get(track_index)
        .and_then(|s| s.as_ref())
        .map(|s| s.is_alive())
        .unwrap_or(false)
}
}

impl SonixApp {
/// Opens the plugin editor for `track_index` in a real X11 window, sharing
/// the instance that is processing audio. Main-thread only.
pub fn open_plugin_gui(&mut self, track_index: usize) {
    self.ensure_plugin_vecs(track_index);
    if self.plugin_gui_sessions[track_index].is_some() {
        return;
    }
    let Some(handle) = self.plugin_handles.get(track_index).and_then(|h| h.clone()) else {
        self.status_message =
            crate::tstatus!("⚠ Ingen aktiv plugin på stämspår {}", track_index + 1);
        return;
    };
    let name = handle.info().name.clone();
    let title = format!("{} – Sonix", name);
    match crate::audio::plugin_gui::GuiSession::open(handle, &title) {
        Ok(session) => {
            let size = session.size();
            self.plugin_gui_sessions[track_index] = Some(session);
            self.status_message = match size {
                Some((width, height)) => crate::tstatus!(
                    "🪟 Öppnade plugin-GUI för '{}' ({}×{})",
                    name,
                    width,
                    height
                ),
                None => crate::tstatus!("🪟 Öppnade plugin-GUI för '{}'", name),
            };
        }
        Err(e) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte öppna plugin-GUI: {}", e);
        }
    }
}
}

impl SonixApp {
/// Closes the plugin editor for `track_index`, if open.
pub fn close_plugin_gui(&mut self, track_index: usize) {
    if let Some(slot) = self.plugin_gui_sessions.get_mut(track_index)
        && slot.is_some()
    {
        *slot = None;
        self.status_message = crate::i18n::t("🪟 Stängde plugin-GUI").to_string();
    }
}
}

impl SonixApp {
/// Pumps X11 events for every open plugin window and drops sessions whose
/// window was closed by the user. Call once per UI frame.
pub fn poll_plugin_guis(&mut self) {
    let mut closed = Vec::new();
    for (index, slot) in self.plugin_gui_sessions.iter_mut().enumerate() {
        if let Some(session) = slot
            && !session.poll()
        {
            closed.push(index);
        }
    }
    for index in closed {
        let title = self.plugin_gui_sessions[index]
            .as_ref()
            .map(|s| s.title().to_string());
        self.plugin_gui_sessions[index] = None;
        self.status_message = match title {
            Some(title) => crate::tstatus!("🪟 Plugin-GUI stängt ({})", title),
            None => crate::i18n::t("🪟 Plugin-GUI stängt").to_string(),
        };
    }
}
}

impl SonixApp {
/// Spawns a sandbox worker for `path` and reads its info + parameters over
/// the process boundary (Fas 4.5a). Replaces any previous worker.
#[cfg(feature = "plugin-host")]
pub fn sandbox_inspect(&mut self, path: &str) {
    use crate::audio::plugin_sandbox::{
        SandboxHost, SandboxInspection, SandboxRequest, SandboxResponse,
    };
    if let Some(mut previous) = self.plugin_sandbox.take() {
        previous.shutdown();
    }
    let mut host = match SandboxHost::new(
        path,
        self.engine.sample_rate as f32,
        crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES as u32,
    ) {
        Ok(host) => host,
        Err(err) => {
            self.status_message = crate::tstatus!("⚠ Kunde inte skapa sandbox: {}", err);
            return;
        }
    };
    let outcome = (|| -> Result<SandboxInspection, String> {
        host.spawn().map_err(|e| e.to_string())?;
        let info = match host.request(&SandboxRequest::Info).map_err(|e| e.to_string())? {
            SandboxResponse::Info { info } => Some(info),
            SandboxResponse::Error { message } => return Err(message),
            other => return Err(format!("oväntat svar: {other:?}")),
        };
        let parameters = match host
            .request(&SandboxRequest::Parameters)
            .map_err(|e| e.to_string())?
        {
            SandboxResponse::Parameters { parameters } => parameters,
            _ => Vec::new(),
        };
        Ok(SandboxInspection {
            path: path.to_string(),
            info,
            parameters,
            restarts: 0,
            error: None,
        })
    })();
    match outcome {
        Ok(inspection) => {
            let name = inspection
                .info
                .as_ref()
                .map(|i| i.name.clone())
                .unwrap_or_else(|| path.to_string());
            self.status_message = crate::tstatus!(
                "🧪 Sandbox: läste {} parametrar ur '{}' i en separat process",
                inspection.parameters.len(),
                name
            );
            self.sandbox_inspection = Some(inspection);
            self.plugin_sandbox = Some(host);
        }
        Err(err) => {
            self.status_message = crate::tstatus!("⚠ Sandbox misslyckades: {}", err);
            self.sandbox_inspection = Some(SandboxInspection {
                path: path.to_string(),
                info: None,
                parameters: Vec::new(),
                restarts: 0,
                error: Some(err),
            });
            host.shutdown();
        }
    }
}
}

impl SonixApp {
/// Health-checks the sandbox worker each frame and reports crashes (Fas 4.5a).
#[cfg(feature = "plugin-host")]
pub fn poll_plugin_sandbox(&mut self) {
    use crate::audio::plugin_sandbox::SandboxState;
    let state = match self.plugin_sandbox.as_mut() {
        Some(host) => host.poll(),
        None => return,
    };
    match state {
        SandboxState::Restarted => {
            let restarts = self
                .plugin_sandbox
                .as_ref()
                .map(|h| h.restarts())
                .unwrap_or(0);
            if let Some(inspection) = self.sandbox_inspection.as_mut() {
                inspection.restarts = restarts;
            }
            self.status_message = crate::tstatus!(
                "🧪 Sandbox: plugin-processen kraschade och startades om (omstart {})",
                restarts
            );
        }
        SandboxState::Crashed => {
            let restarts = self
                .plugin_sandbox
                .as_ref()
                .map(|h| h.restarts())
                .unwrap_or(0);
            if let Some(mut host) = self.plugin_sandbox.take() {
                host.shutdown();
            }
            if let Some(inspection) = self.sandbox_inspection.as_mut() {
                inspection.restarts = restarts;
                inspection.error =
                    Some(crate::i18n::t("sandbox-processen kraschade upprepade gånger").to_string());
            }
            self.status_message = crate::i18n::t(
                "⚠ Sandbox: plugin-processen kraschade upprepade gånger",
            )
            .to_string();
        }
        _ => {}
    }

    // Supervise the per-track audio sandboxes (Fas 4.5b).
    for index in 0..self.plugin_sandboxes.len() {
        let state = match self.plugin_sandboxes[index].as_mut() {
            Some(host) => host.poll(),
            None => continue,
        };
        match state {
            SandboxState::Restarted => {
                let restarts = self.plugin_sandboxes[index]
                    .as_ref()
                    .map(|h| h.restarts())
                    .unwrap_or(0);
                self.status_message = crate::tstatus!(
                    "🧪 Sandbox på stämspår {}: plugin-processen startades om (omstart {})",
                    index + 1,
                    restarts
                );
            }
            SandboxState::Crashed => {
                if let Some(mut host) = self.plugin_sandboxes[index].take() {
                    host.shutdown();
                }
                let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                    track_index: index,
                    insert: None,
                });
                if let Some(slot) = self.plugin_slots.get_mut(index) {
                    *slot = None;
                }
                self.status_message = crate::tstatus!(
                    "⚠ Sandbox på stämspår {} kraschade upprepade gånger och togs bort",
                    index + 1
                );
            }
            _ => {}
        }
    }
}
}

impl SonixApp {
/// Formats the sandbox inspection for the plugin-manager view.
#[cfg(feature = "plugin-host")]
pub fn sandbox_status_text(&self) -> Option<String> {
    let inspection = self.sandbox_inspection.as_ref()?;
    Some(match &inspection.info {
        Some(info) => crate::tstatus!(
            "🧪 Sandbox (separat process): {} v{} – {} parametrar, {} omstarter",
            info.name,
            info.version,
            inspection.parameters.len(),
            inspection.restarts
        ),
        None => crate::tstatus!(
            "🧪 Sandbox misslyckades för '{}': {}",
            inspection.path,
            inspection.error.as_deref().unwrap_or("okänt fel")
        ),
    })
}
}

impl SonixApp {
/// **Sätter det manuella latens-offsetet för ett spårs plugin** (Fas 8.6).
///
/// En dörr, två mottagare: **sloten** (som sparas med projektet och ritas i vyn) och
/// **motorn** (som kompenserar med det). Skriver man bara den ena ser projektfilen riktig ut
/// medan ljudet är fel, eller tvärtom — så de två skrivs här, med samma tal.
pub fn set_plugin_latency_offset(&mut self, track_index: usize, frames: i32) {
    let Some(slot) = self.plugin_slots.get_mut(track_index).and_then(|s| s.as_mut()) else {
        return;
    };
    slot.latency_offset_frames = frames;
    let _ = self
        .engine
        .send_command(AudioCommand::SetPluginLatencyOffset { track_index, frames });
    let ms = crate::audio::plugin_host_live::frames_to_ms(frames, self.engine.sample_rate as f32);
    let ms_text = format!("{ms:+.2}");
    self.status_message = crate::tstatus!(
        "🎯 Latens-offset på stämspår {}: {} ms ({} ramar)",
        track_index + 1,
        ms_text,
        frames
    );
}
}

impl SonixApp {
/// **Smart disable av/på för ett spårs plugin** (Fas 8.6). Samma två mottagare som offsetet:
/// sloten (som sparas med projektet) och insertet på ljudtråden (som vilar).
pub fn set_plugin_smart_disable(&mut self, track_index: usize, enabled: bool) {
    let Some(slot) = self.plugin_slots.get_mut(track_index).and_then(|s| s.as_mut()) else {
        return;
    };
    slot.smart_disable = enabled;
    let _ = self
        .engine
        .send_command(AudioCommand::SetPluginSmartDisable { track_index, enabled });
    self.status_message = if enabled {
        crate::tstatus!("😴 Smart disable på för stämspår {}", track_index + 1)
    } else {
        crate::tstatus!("⚡ Smart disable av för stämspår {}", track_index + 1)
    };
}
}

impl SonixApp {
/// **Kopplar en av pluginens egna utbussar till ett spår** (Fas 8.6).
///
/// Båda mottagarna skrivs, som för offsetet och vilan: sloten (som sparas med projektet) och
/// **motorn** (som routar). Skrev man bara sloten såg projektfilen riktig ut medan motorn inte
/// routade något — samma fälla som de två andra 8.6-reglagen, och den står i `ROADMAP.md`.
pub fn set_plugin_extra_out_routing(
    &mut self,
    track_index: usize,
    port: usize,
    target: Option<usize>,
) {
    let Some(slot) = self.plugin_slots.get_mut(track_index).and_then(|s| s.as_mut()) else {
        return;
    };
    if slot.extra_out_targets.len() <= port {
        slot.extra_out_targets.resize(port + 1, None);
    }
    slot.extra_out_targets[port] = target;
    let targets = slot.extra_out_targets.clone();
    let _ = self
        .engine
        .send_command(AudioCommand::SetPluginExtraOutputs { track_index, targets });
    self.status_message = match target {
        Some(to) => crate::tstatus!("↪ Buss {} → spår {}", port + 1, to + 1),
        None => crate::tstatus!("↪ Buss {} frånkopplad", port + 1),
    };
}
}

impl SonixApp {
fn record_plugin_slot(&mut self, track_index: usize, mut slot: PluginSlot) {
    if self.plugin_slots.len() <= track_index {
        self.plugin_slots.resize_with(track_index + 1, || None);
    }
    // **Ett offset som redan är satt hör till spåret, inte till plugin-instansen**
    // (Fas 8.6). Byter man plugin på samma spår behålls det: offsetet beskriver hur spåret
    // ligger i förhållande till de andra, och att nolla det i smyg vore samma sorts tysta
    // förlust som `LoadStemTrack`-fällan. Att nollställa är ett medvetet drag i
    // gränssnittet, inte en bieffekt av att ladda en plugin.
    if let Some(previous) = self.plugin_slots.get(track_index).and_then(|s| s.as_ref()) {
        slot.latency_offset_frames = previous.latency_offset_frames;
        // Kryssrutan ärvs av samma skäl som offsetet: den beskriver hur **spåret** ska
        // behandlas, och att tappa den vid ett pluginbyte hade tyst satt igång processningen
        // igen för en plugin användaren medvetet hade låtit vila.
        slot.smart_disable = previous.smart_disable;
    }
    self.plugin_slots[track_index] = Some(slot);
}
}

impl SonixApp {
/// Re-instantiates a saved plugin on `track_index` and restores its state.
/// Must run on the main thread. Returns an error message on failure.
pub(crate) fn restore_plugin_slot(&mut self, track_index: usize, data: &SavedPluginData) -> Option<String> {
    if data.sandboxed {
        #[cfg(feature = "plugin-host")]
        {
            use crate::audio::plugin_sandbox::{SandboxRequest, SandboxResponse};
            self.load_plugin_into_sandbox_track(&data.path, track_index);
            let host = self
                .plugin_sandboxes
                .get_mut(track_index)
                .and_then(|slot| slot.as_mut());
            let Some(host) = host else {
                return Some(crate::tstatus!(
                    "kunde inte återställa sandboxad plugin '{}'",
                    data.name
                ));
            };
            if !data.state.is_empty()
                && !matches!(
                    host.request(&SandboxRequest::LoadState {
                        data: data.state.clone(),
                    }),
                    Ok(SandboxResponse::Ok)
                )
            {
                return Some(crate::tstatus!(
                    "kunde inte återställa state för sandboxad plugin '{}'",
                    data.name
                ));
            }
            if let Some(slot) = self.plugin_slots.get_mut(track_index).and_then(|s| s.as_mut()) {
                slot.state = data.state.clone();
            }
            return None;
        }
        #[cfg(not(feature = "plugin-host"))]
        return Some(crate::i18n::t(
            "sandboxade plugins kräver att Sonix byggs med --features plugin-host",
        )
        .to_string());
    }
    let sample_rate = self.engine.sample_rate as f32;
    let block = crate::audio::plugin_host_live::DEFAULT_BLOCK_FRAMES;
    match crate::audio::plugin_host_live::load_processor(&data.path, sample_rate, block as u32) {
        Ok(processor) => {
            let mut insert = crate::audio::plugin_host_live::PluginInsert::new(processor, block);
            if !data.state.is_empty() && !insert.load_state(&data.state) {
                return Some(crate::tstatus!(
                    "kunde inte återställa state för '{}'",
                    data.name
                ));
            }
            let handle = insert.core_handle();
            self.ensure_plugin_vecs(track_index);
            self.plugin_handles[track_index] = handle;
            let _ = self.engine.send_command(AudioCommand::SetTrackPlugin {
                track_index,
                insert: Some(insert),
            });
            None
        }
        Err(e) => Some(e),
    }
}
}

