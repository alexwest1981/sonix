//! Out-of-process plugin sandbox (Fas 4.5a + 4.5b).
//!
//! 4.5a establishes the **process boundary** and crash supervision: a worker
//! process loads a CLAP plugin and answers control-plane requests over a
//! length-prefixed JSON protocol on stdin/stdout, while a [`SandboxHost`] in the
//! main process supervises it and restarts it after a crash.
//!
//! 4.5b moves the **audio processing itself** into the worker: the host creates
//! a shared-memory region (see [`crate::audio::sandbox_audio`]) and passes its
//! descriptor to the worker, which maps the same region and streams stereo
//! blocks through it. [`SandboxProcessor`] is the host-side
//! [`crate::audio::plugin_host_live::PluginProcessor`] that drives that ring.
//!
//! The worker is the same binary re-executed with [`WORKER_FLAG`], so the
//! sandbox needs no extra artifact.

use std::io::{self, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Command-line flag that puts the Sonix binary into sandbox-worker mode.
pub const WORKER_FLAG: &str = "--plugin-sandbox-worker";

/// Upper bound on a single frame, as a sanity check against corrupt lengths.
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// A control-plane request from the supervisor to the worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum SandboxRequest {
    Ping,
    Info,
    Parameters,
    SetParameter { id: u32, value: f64 },
    SaveState,
    LoadState { data: Vec<u8> },
    Reset,
    /// Reports the plugin's own latency in frames (not counting the transport).
    Latency,
    Shutdown,
}

/// A worker response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SandboxResponse {
    Pong,
    Info { info: SandboxInfo },
    Parameters { parameters: Vec<SandboxParam> },
    Ok,
    State { data: Vec<u8> },
    Latency { frames: u32 },
    Error { message: String },
}

/// Plain, serialisable copy of [`crate::audio::plugin_host_live::PluginInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SandboxInfo {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub description: String,
}

/// Plain, serialisable copy of
/// [`crate::audio::plugin_host_live::PluginParameter`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxParam {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub flags: u32,
}

/// A control-plane snapshot taken from a sandbox worker.
#[derive(Debug, Clone)]
pub struct SandboxInspection {
    pub path: String,
    pub info: Option<SandboxInfo>,
    pub parameters: Vec<SandboxParam>,
    pub restarts: u32,
    pub error: Option<String>,
}

/// How the supervisor is doing after a health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    /// The worker is running normally.
    Running,
    /// The worker had died and was successfully restarted.
    Restarted,
    /// The worker died and the restart budget was exhausted.
    Crashed,
    /// The supervisor was shut down intentionally.
    Stopped,
}

// ============================================================================
// Framing
// ============================================================================

/// Writes `payload` as a 4-byte little-endian length followed by the bytes.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()
}

/// Reads one frame. Returns `Ok(None)` on a clean EOF at a frame boundary.
pub fn read_frame<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame length exceeds the limit",
        ));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

// ============================================================================
// Worker
// ============================================================================

fn info_to_dto(info: &crate::audio::plugin_host_live::PluginInfo) -> SandboxInfo {
    SandboxInfo {
        id: info.id.clone(),
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        version: info.version.clone(),
        description: info.description.clone(),
    }
}

fn param_to_dto(param: &crate::audio::plugin_host_live::PluginParameter) -> SandboxParam {
    SandboxParam {
        id: param.id,
        name: param.name.clone(),
        module: param.module.clone(),
        min_value: param.min_value,
        max_value: param.max_value,
        default_value: param.default_value,
        flags: param.flags,
    }
}

pub fn info_from_dto(dto: &SandboxInfo) -> crate::audio::plugin_host_live::PluginInfo {
    crate::audio::plugin_host_live::PluginInfo {
        id: dto.id.clone(),
        name: dto.name.clone(),
        vendor: dto.vendor.clone(),
        version: dto.version.clone(),
        description: dto.description.clone(),
        features: Vec::new(),
    }
}

pub fn param_from_dto(dto: &SandboxParam) -> crate::audio::plugin_host_live::PluginParameter {
    crate::audio::plugin_host_live::PluginParameter {
        id: dto.id,
        name: dto.name.clone(),
        module: dto.module.clone(),
        min_value: dto.min_value,
        max_value: dto.max_value,
        default_value: dto.default_value,
        flags: dto.flags,
    }
}

fn respond<W: Write>(writer: &mut W, response: &SandboxResponse) -> io::Result<()> {
    let payload = serde_json::to_vec(response).map_err(io::Error::other)?;
    write_frame(writer, &payload)
}

fn handle_request(
    processor: &mut dyn crate::audio::plugin_host_live::PluginProcessor,
    request: SandboxRequest,
) -> SandboxResponse {
    match request {
        SandboxRequest::Ping => SandboxResponse::Pong,
        SandboxRequest::Info => SandboxResponse::Info {
            info: info_to_dto(processor.info()),
        },
        SandboxRequest::Parameters => SandboxResponse::Parameters {
            parameters: processor.parameters().iter().map(param_to_dto).collect(),
        },
        SandboxRequest::SetParameter { id, value } => {
            if processor.set_parameter(id, value) {
                SandboxResponse::Ok
            } else {
                SandboxResponse::Error {
                    message: format!("okänd parameter {id}"),
                }
            }
        }
        SandboxRequest::SaveState => SandboxResponse::State {
            data: processor.save_state(),
        },
        SandboxRequest::LoadState { data } => {
            if processor.load_state(&data) {
                SandboxResponse::Ok
            } else {
                SandboxResponse::Error {
                    message: "pluginen avvisade state-bloben".to_string(),
                }
            }
        }
        SandboxRequest::Reset => {
            processor.reset();
            SandboxResponse::Ok
        }
        SandboxRequest::Latency => SandboxResponse::Latency {
            frames: processor.latency_frames(),
        },
        SandboxRequest::Shutdown => SandboxResponse::Ok,
    }
}

/// Runs the worker loop against `reader`/`writer` (usually stdin/stdout).
///
/// Loads `plugin_path` once and then answers framed [`SandboxRequest`]s until a
/// `Shutdown` request or EOF. A load failure is reported as an error response
/// but the loop keeps serving so the supervisor can read the reason and shut
/// down cleanly.
pub fn serve<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    plugin_path: &str,
    sample_rate: f32,
    block_frames: u32,
) -> io::Result<()> {
    let mut processor =
        match crate::audio::plugin_host_live::load_processor(plugin_path, sample_rate, block_frames)
        {
            Ok(processor) => Some(processor),
            Err(err) => {
                respond(writer, &SandboxResponse::Error { message: err })?;
                None
            }
        };

    loop {
        let Some(frame) = read_frame(reader)? else {
            break;
        };
        let request: SandboxRequest = match serde_json::from_slice(&frame) {
            Ok(request) => request,
            Err(err) => {
                respond(
                    writer,
                    &SandboxResponse::Error {
                        message: format!("ogiltig begäran: {err}"),
                    },
                )?;
                continue;
            }
        };
        if request == SandboxRequest::Shutdown {
            respond(writer, &SandboxResponse::Ok)?;
            break;
        }
        let response = match processor.as_mut() {
            Some(processor) => handle_request(&mut **processor, request),
            None => SandboxResponse::Error {
                message: "pluginen kunde inte laddas i sandboxen".to_string(),
            },
        };
        respond(writer, &response)?;
    }
    Ok(())
}

/// Parses `["<plugin>", "<sample_rate>", "<block_frames>"]` and runs the worker.
/// Returns an error string suitable for `main`.
pub fn serve_from_args(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(format!(
            "{WORKER_FLAG}: förväntade <plugin> <sample_rate> <block_frames> [<memfd>]"
        ));
    }
    if args.len() >= 4 {
        return serve_audio_from_args(args);
    }
    let sample_rate: f32 = args[1].parse().unwrap_or(48_000.0);
    let block_frames: u32 = args[2].parse().unwrap_or(128);
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(
        &mut stdin.lock(),
        &mut stdout.lock(),
        &args[0],
        sample_rate,
        block_frames,
    )
    .map_err(|e| e.to_string())
}

// ============================================================================
// Sandbox audio processor (host side)
// ============================================================================

/// Host-side [`crate::audio::plugin_host_live::PluginProcessor`] that streams
/// stereo audio to the worker over shared memory (Fas 4.5b).
///
/// It reports `block_frames + plugin_latency` frames of latency so the engine's
/// PDC compensates the transport. If the worker is behind or has crashed the
/// block is emitted as silence; the supervisor restarts the worker and the ring
/// re-synchronises.
pub struct SandboxProcessor {
    bridge: crate::audio::sandbox_audio::AudioBridge,
    info: crate::audio::plugin_host_live::PluginInfo,
    parameters: Vec<crate::audio::plugin_host_live::PluginParameter>,
    plugin_latency: u32,
    block_frames: usize,
}

impl SandboxProcessor {
    pub fn new(
        bridge: crate::audio::sandbox_audio::AudioBridge,
        info: crate::audio::plugin_host_live::PluginInfo,
        parameters: Vec<crate::audio::plugin_host_live::PluginParameter>,
        plugin_latency: u32,
    ) -> Self {
        let block_frames = bridge.block_frames();
        Self {
            bridge,
            info,
            parameters,
            plugin_latency,
            block_frames,
        }
    }
}

impl crate::audio::plugin_host_live::PluginProcessor for SandboxProcessor {
    fn backend(&self) -> &'static str {
        "clap (sandbox)"
    }

    fn info(&self) -> &crate::audio::plugin_host_live::PluginInfo {
        &self.info
    }

    fn parameters(&self) -> &[crate::audio::plugin_host_live::PluginParameter] {
        &self.parameters
    }

    fn latency_frames(&self) -> u32 {
        // One block for the transport round-trip plus the plugin's own latency.
        self.block_frames as u32 + self.plugin_latency
    }

    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let status = self.bridge.process_block(left, right);
        if !status.output_available {
            for sample in left.iter_mut() {
                *sample = 0.0;
            }
            for sample in right.iter_mut() {
                *sample = 0.0;
            }
        }
    }

    fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
        // Live parameter changes for sandboxed plugins are not routed across the
        // process boundary in 4.5b; the control plane is main-thread only.
        false
    }

    fn reset(&mut self) {}
}

// ============================================================================
// Sandbox worker with audio (worker side)
// ============================================================================

/// Runs the audio worker: a control thread owns stdin/stdout while the audio
/// loop owns the plugin and pumps the shared-memory ring.
pub fn serve_audio_from_args(args: &[String]) -> Result<(), String> {
    let sample_rate: f32 = args[1].parse().unwrap_or(48_000.0);
    let block_frames: u32 = args[2].parse().unwrap_or(128);
    let fd: std::os::fd::RawFd = args[3].parse().map_err(|_| {
        format!("{WORKER_FLAG}: ogiltig memfd '{}'", args[3])
    })?;
    let bridge = crate::audio::sandbox_audio::AudioBridge::attach(fd)
        .map_err(|e| e.to_string())?;
    let processor = match crate::audio::plugin_host_live::load_processor(
        &args[0],
        sample_rate,
        block_frames,
    ) {
        Ok(processor) => Some(processor),
        Err(_) => None,
    };

    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let control = std::thread::spawn(move || {
        control_loop(io::stdin(), io::stdout(), request_tx)
    });
    run_audio_worker(processor, &bridge, request_rx);
    let _ = control.join();
    Ok(())
}

/// The worker's audio loop. Handles control requests between blocks and pumps
/// the shared-memory ring. Returns on shutdown or when the control thread goes
/// away.
pub fn run_audio_worker(
    mut processor: Option<Box<dyn crate::audio::plugin_host_live::PluginProcessor>>,
    bridge: &crate::audio::sandbox_audio::AudioBridge,
    control_rx: std::sync::mpsc::Receiver<(SandboxRequest, std::sync::mpsc::Sender<SandboxResponse>)>,
) {
    use std::sync::mpsc::TryRecvError;
    bridge.worker_begin();
    loop {
        loop {
            match control_rx.try_recv() {
                Ok((request, reply)) => {
                    let is_shutdown = request == SandboxRequest::Shutdown;
                    let response = match processor.as_mut() {
                        Some(processor) => handle_request(&mut **processor, request),
                        None => SandboxResponse::Error {
                            message: "pluginen kunde inte laddas i sandboxen".to_string(),
                        },
                    };
                    let _ = reply.send(response);
                    if is_shutdown {
                        bridge.request_shutdown();
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if bridge.shutdown_requested() {
            return;
        }
        let processed = match processor.as_mut() {
            Some(processor) => bridge.process_one(&mut **processor),
            None => false,
        };
        if processed {
            bridge.worker_heartbeat();
        } else {
            std::thread::sleep(Duration::from_micros(100));
        }
    }
}

/// Reads framed control requests and forwards them to the audio loop, writing
/// the response back. Runs on its own thread in the worker.
fn control_loop<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    request_tx: std::sync::mpsc::Sender<(
        SandboxRequest,
        std::sync::mpsc::Sender<SandboxResponse>,
    )>,
) -> io::Result<()> {
    loop {
        let Some(frame) = read_frame(&mut reader)? else {
            break;
        };
        let request: SandboxRequest = match serde_json::from_slice(&frame) {
            Ok(request) => request,
            Err(err) => {
                respond(
                    &mut writer,
                    &SandboxResponse::Error {
                        message: format!("ogiltig begäran: {err}"),
                    },
                )?;
                continue;
            }
        };
        let is_shutdown = request == SandboxRequest::Shutdown;
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        if request_tx.send((request, reply_tx)).is_err() {
            respond(
                &mut writer,
                &SandboxResponse::Error {
                    message: "sandbox-arbetaren har stängt".to_string(),
                },
            )?;
            break;
        }
        let response = reply_rx.recv().unwrap_or(SandboxResponse::Error {
            message: "sandbox-arbetaren svarade inte".to_string(),
        });
        respond(&mut writer, &response)?;
        if is_shutdown {
            break;
        }
    }
    Ok(())
}

// ============================================================================
// Supervisor
// ============================================================================

/// A crash-supervising handle to an out-of-process plugin worker.
pub struct SandboxHost {
    launcher: Box<dyn Fn() -> Command + Send>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<io::BufReader<ChildStdout>>,
    restarts: u32,
    max_restarts: u32,
    stopped: bool,
}

impl SandboxHost {
    /// Builds a supervisor that re-executes the current binary in worker mode.
    pub fn new(plugin_path: &str, sample_rate: f32, block_frames: u32) -> io::Result<Self> {
        Self::with_audio_fd(plugin_path, sample_rate, block_frames, None)
    }

    /// Builds a supervisor whose worker also attaches the shared-memory audio
    /// region identified by `audio_fd` (Fas 4.5b).
    pub fn new_audio(
        plugin_path: &str,
        sample_rate: f32,
        block_frames: u32,
        audio_fd: std::os::fd::RawFd,
    ) -> io::Result<Self> {
        Self::with_audio_fd(plugin_path, sample_rate, block_frames, Some(audio_fd))
    }

    fn with_audio_fd(
        plugin_path: &str,
        sample_rate: f32,
        block_frames: u32,
        audio_fd: Option<std::os::fd::RawFd>,
    ) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let plugin = plugin_path.to_string();
        let launcher = Box::new(move || {
            let mut cmd = Command::new(&exe);
            cmd.arg(WORKER_FLAG)
                .arg(&plugin)
                .arg(format!("{sample_rate}"))
                .arg(block_frames.to_string())
                .stderr(Stdio::null());
            if let Some(fd) = audio_fd {
                cmd.arg(fd.to_string());
            }
            cmd
        });
        Ok(Self::with_launcher(launcher))
    }

    /// Builds a supervisor from an arbitrary command factory (used by tests).
    pub fn with_launcher(launcher: Box<dyn Fn() -> Command + Send>) -> Self {
        Self {
            launcher,
            child: None,
            stdin: None,
            stdout: None,
            restarts: 0,
            max_restarts: 3,
            stopped: false,
        }
    }

    /// Spawns the worker and connects its pipes.
    pub fn spawn(&mut self) -> io::Result<()> {
        let mut cmd = (self.launcher)();
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        let mut child = cmd.spawn()?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(io::BufReader::new);
        self.child = Some(child);
        self.stopped = false;
        Ok(())
    }

    /// Sends one request and reads one response.
    pub fn request(&mut self, request: &SandboxRequest) -> io::Result<SandboxResponse> {
        let payload = serde_json::to_vec(request).map_err(io::Error::other)?;
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "sandbox-arbetaren saknas")
        })?;
        write_frame(stdin, &payload)?;
        let stdout = self.stdout.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "sandbox-arbetaren saknas")
        })?;
        let frame = read_frame(stdout)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "sandbox-arbetaren stängde anslutningen",
            )
        })?;
        serde_json::from_slice(&frame).map_err(io::Error::other)
    }

    /// Whether the worker process is currently alive.
    pub fn is_alive(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    /// Health-checks the worker, restarting it once if it has died.
    pub fn poll(&mut self) -> SandboxState {
        if self.stopped {
            return SandboxState::Stopped;
        }
        if self.is_alive() {
            return SandboxState::Running;
        }
        if self.restarts < self.max_restarts {
            self.restarts += 1;
            if self.spawn().is_ok() {
                return SandboxState::Restarted;
            }
        }
        SandboxState::Crashed
    }

    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// Sets the restart budget before giving up. Used by tests and by callers
    /// that want a stricter policy than the default.
    #[allow(dead_code)]
    pub fn set_max_restarts(&mut self, max: u32) {
        self.max_restarts = max;
    }

    /// Requests a graceful shutdown, then kills the worker if it does not exit
    /// promptly. Idempotent.
    pub fn shutdown(&mut self) {
        self.stopped = true;
        if let Some(mut stdin) = self.stdin.take()
            && let Ok(payload) = serde_json::to_vec(&SandboxRequest::Shutdown)
        {
            let _ = write_frame(&mut stdin, &payload);
        }
        if let Some(child) = self.child.as_mut() {
            let mut exited = false;
            for _ in 0..20 {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        exited = true;
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
            if !exited {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.child = None;
        self.stdout = None;
    }
}

impl Drop for SandboxHost {
    fn drop(&mut self) {
        if !self.stopped {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use crate::audio::plugin_host_live::PluginProcessor;

    #[test]
    fn frames_round_trip() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, b"hello").unwrap();
        write_frame(&mut buffer, b"").unwrap();
        let mut reader = Cursor::new(buffer);
        assert_eq!(read_frame(&mut reader).unwrap().as_deref(), Some(&b"hello"[..]));
        assert_eq!(read_frame(&mut reader).unwrap().as_deref(), Some(&b""[..]));
        assert_eq!(read_frame(&mut reader).unwrap(), None);
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&(u32::MAX).to_le_bytes());
        let err = read_frame(&mut Cursor::new(buffer)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn worker_answers_the_control_protocol() {
        let Some(mock) = option_env!("SONIX_MOCK_CLAP") else {
            return;
        };
        let requests = [
            SandboxRequest::Ping,
            SandboxRequest::Info,
            SandboxRequest::Parameters,
            SandboxRequest::SetParameter { id: 0, value: 0.5 },
            SandboxRequest::SaveState,
            SandboxRequest::Shutdown,
        ];
        let mut input = Vec::new();
        for request in &requests {
            write_frame(&mut input, &serde_json::to_vec(request).unwrap()).unwrap();
        }
        let mut output = Vec::new();
        serve(&mut Cursor::new(input), &mut output, mock, 48_000.0, 128).unwrap();

        let mut reader = Cursor::new(output);
        let mut responses = Vec::new();
        while let Some(frame) = read_frame(&mut reader).unwrap() {
            responses.push(serde_json::from_slice::<SandboxResponse>(&frame).unwrap());
        }
        assert_eq!(responses.len(), requests.len());
        assert_eq!(responses[0], SandboxResponse::Pong);
        match &responses[1] {
            SandboxResponse::Info { info } => assert_eq!(info.name, "Sonix Mock Gain"),
            other => panic!("expected Info, got {other:?}"),
        }
        match &responses[2] {
            SandboxResponse::Parameters { parameters } => assert_eq!(parameters.len(), 2),
            other => panic!("expected Parameters, got {other:?}"),
        }
        assert_eq!(responses[3], SandboxResponse::Ok);
        match &responses[4] {
            SandboxResponse::State { data } => assert_eq!(data.len(), 16),
            other => panic!("expected State, got {other:?}"),
        }
        assert_eq!(responses[5], SandboxResponse::Ok);
    }

    fn sleep_command(seconds: u32) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &format!("sleep {seconds}")]);
        cmd
    }

    fn exit_command(code: i32) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &format!("exit {code}")]);
        cmd
    }

    fn wait_for<F: FnMut() -> SandboxState>(mut f: F) -> SandboxState {
        for _ in 0..200 {
            let state = f();
            if state != SandboxState::Running {
                return state;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        SandboxState::Running
    }

    fn wait_for_state<F: FnMut() -> SandboxState>(
        mut f: F,
        target: SandboxState,
    ) -> SandboxState {
        let mut last = SandboxState::Running;
        for _ in 0..500 {
            last = f();
            if last == target {
                return last;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        last
    }

    #[test]
    fn supervisor_restarts_a_crashed_worker() {
        let spawns = Arc::new(AtomicU32::new(0));
        let counter = spawns.clone();
        let mut host = SandboxHost::with_launcher(Box::new(move || {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                exit_command(3)
            } else {
                sleep_command(30)
            }
        }));
        host.spawn().unwrap();
        let state = wait_for(|| host.poll());
        assert_eq!(state, SandboxState::Restarted);
        assert_eq!(host.restarts(), 1);
        assert!(host.is_alive());
        host.shutdown();
    }

    #[test]
    fn supervisor_gives_up_after_the_restart_budget() {
        let mut host = SandboxHost::with_launcher(Box::new(|| exit_command(1)));
        host.set_max_restarts(2);
        host.spawn().unwrap();
        let state = wait_for_state(|| host.poll(), SandboxState::Crashed);
        assert_eq!(state, SandboxState::Crashed);
        assert_eq!(host.restarts(), 2);
        host.shutdown();
    }

    #[test]
    fn shutdown_stops_supervision() {
        let mut host = SandboxHost::with_launcher(Box::new(|| sleep_command(30)));
        host.spawn().unwrap();
        host.shutdown();
        assert_eq!(host.poll(), SandboxState::Stopped);
    }

    struct GainProcessor {
        gain: f32,
        info: crate::audio::plugin_host_live::PluginInfo,
    }

    impl GainProcessor {
        fn new(gain: f32) -> Self {
            Self {
                gain,
                info: crate::audio::plugin_host_live::PluginInfo {
                    id: "se.sonix.test.gain".to_string(),
                    name: "Test Gain".to_string(),
                    vendor: "Sonix".to_string(),
                    version: "1.0".to_string(),
                    description: String::new(),
                    features: Vec::new(),
                },
            }
        }
    }

    impl crate::audio::plugin_host_live::PluginProcessor for GainProcessor {
        fn backend(&self) -> &'static str {
            "test"
        }
        fn info(&self) -> &crate::audio::plugin_host_live::PluginInfo {
            &self.info
        }
        fn parameters(&self) -> &[crate::audio::plugin_host_live::PluginParameter] {
            &[]
        }
        fn latency_frames(&self) -> u32 {
            0
        }
        fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
            for sample in left.iter_mut() {
                *sample *= self.gain;
            }
            for sample in right.iter_mut() {
                *sample *= self.gain;
            }
        }
        fn set_parameter(&mut self, _id: u32, _value: f64) -> bool {
            false
        }
        fn reset(&mut self) {}
    }

    fn attach_second(bridge: &crate::audio::sandbox_audio::AudioBridge) -> crate::audio::sandbox_audio::AudioBridge {
        let fd = unsafe { libc::dup(bridge.raw_fd()) };
        assert!(fd >= 0, "dup failed");
        crate::audio::sandbox_audio::AudioBridge::attach(fd).unwrap()
    }

    #[test]
    fn sandbox_processor_reports_transport_latency() {
        let block = 16usize;
        let bridge =
            crate::audio::sandbox_audio::AudioBridge::create(block, 4).unwrap();
        let processor = SandboxProcessor::new(bridge, GainProcessor::new(1.0).info, Vec::new(), 7);
        assert_eq!(processor.latency_frames(), block as u32 + 7);
    }

    #[test]
    fn audio_worker_streams_blocks_over_shared_memory() {
        let block = 8usize;
        let host = crate::audio::sandbox_audio::AudioBridge::create(block, 4).unwrap();
        let worker = attach_second(&host);
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let processor: Box<dyn crate::audio::plugin_host_live::PluginProcessor> =
                Box::new(GainProcessor::new(2.0));
            run_audio_worker(Some(processor), &worker, request_rx);
        });

        // Wait for the worker to attach (and reset the ring) before streaming.
        let (ping_tx, ping_rx) = std::sync::mpsc::channel();
        request_tx
            .send((SandboxRequest::Ping, ping_tx))
            .unwrap();
        assert_eq!(ping_rx.recv().unwrap(), SandboxResponse::Pong);

        let left = vec![0.25f32; block];
        let right = vec![0.25f32; block];
        assert!(host.publish_input(&left, &right));

        let mut out_l = vec![0.0f32; block];
        let mut out_r = vec![0.0f32; block];
        let mut produced = false;
        for _ in 0..400 {
            if host.take_output(&mut out_l, &mut out_r) {
                produced = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(produced, "arbetaren producerade ingen utdata");
        assert_eq!(out_l, vec![0.5f32; block]);
        assert_eq!(out_r, vec![0.5f32; block]);

        let (reply_tx, _reply_rx) = std::sync::mpsc::channel();
        request_tx
            .send((SandboxRequest::Shutdown, reply_tx))
            .unwrap();
        handle.join().unwrap();
    }
}
