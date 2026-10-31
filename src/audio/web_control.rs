//! `--web [port]`: a local HTTP control surface, standard library only.
//!
//! Three things live here and nothing else: the request-line parser, the transport
//! snapshot the HTTP thread may read, and the listener thread. The thread never
//! touches `SonixApp` — it reads [`WebState`] (written by the UI once per frame) and
//! queues [`AudioCommand`]s on a plain channel that the UI drains.
//!
//! The listener binds **loopback only**: a control surface that changes the transport
//! has no business on the LAN unless the user puts a tunnel in front of it.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::command::AudioCommand;

/// What the HTTP thread is allowed to see of the app. The UI overwrites it each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct WebState {
    pub bpm: f32,
    pub playing: bool,
    pub position: f32,
}

/// A parsed request line: `GET /status?token=abc HTTP/1.1`.
#[derive(Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    /// `None` when the query has no `token=` — treated as forbidden, not as a parse error.
    pub token: Option<&'a str>,
}

/// Splits a request line into method, path and query token. Headers are not read.
pub fn parse_request(line: &str) -> Option<Request<'_>> {
    let mut parts = line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let token = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
        .filter(|t| !t.is_empty());
    Some(Request {
        method,
        path,
        token,
    })
}

pub struct WebServer {
    /// Secret printed at startup; every request must carry it.
    pub token: String,
    pub addr: SocketAddr,
    /// Snapshot the server reads and the UI writes.
    pub state: Arc<Mutex<WebState>>,
    /// Commands queued by the server, drained by the UI each frame.
    pub commands: Receiver<AudioCommand>,
    running: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WebServer {
    /// Binds `port` on 127.0.0.1 (0 = any free port) and starts the one listener thread.
    pub fn start(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let token = new_token();
        let state = Arc::new(Mutex::new(WebState::default()));
        let (tx, commands) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));

        let running_t = running.clone();
        let state_t = state.clone();
        let token_t = token.clone();
        let join = thread::spawn(move || {
            while running_t.load(Ordering::Relaxed) {
                match listener.accept() {
                    // Single thread, so one slow client delays the next request.
                    // shortcut: no worker pool, add one if a phone ever stalls it.
                    Ok((stream, _)) => {
                        let _ = handle(stream, &token_t, &tx, &state_t);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            token,
            addr,
            state,
            commands,
            running,
            join: Some(join),
        })
    }

    /// The URL to open on this machine, token included so the page's buttons work.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?token={}", self.addr.port(), self.token)
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn handle(
    mut stream: TcpStream,
    token: &str,
    commands: &Sender<AudioCommand>,
    state: &Mutex<WebState>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    // Only the request line matters: no route here has a body.
    let mut buf = [0u8; 1024];
    let mut raw = Vec::new();
    while !raw.contains(&b'\n') && raw.len() < 1024 {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let line = String::from_utf8_lossy(&raw);
    let Some(req) = parse_request(line.lines().next().unwrap_or("")) else {
        return respond(&mut stream, "400 Bad Request", "text/plain", "bad request\n");
    };
    // The check comes before any route: a request without the token changes nothing.
    if req.token != Some(token) {
        return respond(&mut stream, "403 Forbidden", "text/plain", "forbidden\n");
    }

    match (req.method, req.path) {
        ("GET", "/") => respond(&mut stream, "200 OK", "text/html; charset=utf-8", &page(token)),
        ("GET", "/status") => {
            let snapshot = state.lock().map(|s| *s).unwrap_or_default();
            respond(&mut stream, "200 OK", "application/json", &status_json(&snapshot))
        }
        ("POST", "/play") => {
            let _ = commands.send(AudioCommand::SetSongPlayback(true));
            respond(&mut stream, "200 OK", "text/plain", "playing\n")
        }
        ("POST", "/stop") => {
            let _ = commands.send(AudioCommand::SetSongPlayback(false));
            respond(&mut stream, "200 OK", "text/plain", "stopped\n")
        }
        ("GET", "/play") | ("GET", "/stop") => {
            respond(&mut stream, "405 Method Not Allowed", "text/plain", "use POST\n")
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", "not found\n"),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

fn status_json(s: &WebState) -> String {
    format!(
        "{{\"bpm\":{:.2},\"playing\":{},\"position\":{:.3}}}",
        s.bpm, s.playing, s.position
    )
}

/// The page is one file with the token baked in, so the buttons work when the phone
/// opens the printed URL and nothing else.
fn page(token: &str) -> String {
    format!(
        r#"<!doctype html><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Sonix</title>
<style>
body{{font:16px system-ui;margin:0;padding:1rem;background:#111;color:#eee;text-align:center}}
button{{font-size:1.2rem;padding:1.2rem 2rem;margin:.4rem;border:0;border-radius:.6rem;background:#3a7;color:#fff;min-width:8rem}}
button.s{{background:#a44}}
#st{{font-size:1.4rem;margin:1rem 0}}
</style>
<h1>Sonix</h1>
<p id="st">–</p>
<button onclick="act('play')">Play</button>
<button class="s" onclick="act('stop')">Stop</button>
<script>
const q = '?token={token}';
async function act(a) {{
  await fetch('/' + a + q, {{ method: 'POST' }});
  poll();
}}
async function poll() {{
  try {{
    const s = await (await fetch('/status' + q)).json();
    document.getElementById('st').textContent =
      (s.playing ? '▶ playing' : '⏹ stopped') + ' · ' + s.bpm + ' BPM · ' + s.position.toFixed(1) + ' s';
  }} catch (e) {{
    document.getElementById('st').textContent = 'no connection';
  }}
}}
poll();
setInterval(poll, 1000);
</script>
"#
    )
}

/// Random enough for a LAN token: nanos + pid, mixed so the low bits are not just time.
fn new_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() ^ u64::from(d.subsec_nanos()))
        .unwrap_or(0);
    let mixed = (nanos ^ (u64::from(std::process::id()) << 32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{mixed:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(addr: SocketAddr, target: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(stream, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("read response");
        let status = raw
            .split(' ')
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (status, body)
    }

    fn post(addr: SocketAddr, target: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(stream, "POST {target} HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).expect("read response");
        let status = raw
            .split(' ')
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let body = raw.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (status, body)
    }

    /// (a) The parser splits method, path and token; a missing token is `None`.
    #[test]
    fn web_control_parses_request_line_and_token() {
        let cases = [
            "GET /status?token=deadbeef HTTP/1.1",
            "POST /play?token=deadbeef&x=1 HTTP/1.1",
            "GET / HTTP/1.1",
        ];
        for line in cases {
            let req = parse_request(line).expect("parseable");
            println!("parser: {line:?} -> {req:?}");
        }
        let status = parse_request(cases[0]).unwrap();
        assert_eq!(
            (status.method, status.path, status.token),
            ("GET", "/status", Some("deadbeef"))
        );
        let extra = parse_request(cases[1]).unwrap();
        assert_eq!((extra.method, extra.path, extra.token), ("POST", "/play", Some("deadbeef")));
        let bare = parse_request(cases[2]).unwrap();
        assert_eq!(bare.token, None);
        assert!(parse_request("garbage").is_none());
    }

    /// (b) A wrong or missing token is 403 and leaves the command channel empty.
    #[test]
    fn web_control_rejects_wrong_token() {
        let server = WebServer::start(0).expect("bind loopback");
        let (status, body) = post(server.addr, "/play?token=not-the-token");
        println!("wrong token: HTTP {status} body={body:?}");
        assert_eq!(status, 403);
        let (status, body) = post(server.addr, "/play");
        println!("missing token: HTTP {status} body={body:?}");
        assert_eq!(status, 403);
        assert_eq!(server.commands.try_recv().err().is_some(), true, "no command must be queued");
    }

    /// (c) Real round trip on an ephemeral port: loopback address, 200 and JSON.
    #[test]
    fn web_control_status_over_the_wire() {
        let server = WebServer::start(0).expect("bind loopback");
        println!("listener: {} (ip_is_loopback={})", server.addr, server.addr.ip().is_loopback());
        assert!(server.addr.ip().is_loopback(), "must not bind 0.0.0.0");
        *server.state.lock().unwrap() = WebState {
            bpm: 128.0,
            playing: true,
            position: 3.5,
        };
        let (status, body) = get(server.addr, &format!("/status?token={}", server.token));
        println!("GET /status: HTTP {status} body={body}");
        assert_eq!(status, 200);
        let json: serde_json::Value = serde_json::from_str(&body).expect("parseable JSON");
        assert_eq!(json["bpm"].as_f64(), Some(128.0));
        assert_eq!(json["playing"].as_bool(), Some(true));
        assert_eq!(json["position"].as_f64(), Some(3.5));

        let (status, body) = get(server.addr, &format!("/?token={}", server.token));
        println!("GET /: HTTP {status} bytes={}", body.len());
        assert_eq!(status, 200);
        assert!(body.contains("token="), "page must carry the token for its buttons");
    }

    /// (d) POST /play with the token queues exactly one play command.
    #[test]
    fn web_control_play_queues_one_command() {
        let server = WebServer::start(0).expect("bind loopback");
        let (status, body) = post(server.addr, &format!("/play?token={}", server.token));
        println!("POST /play: HTTP {status} body={body:?}");
        assert_eq!(status, 200);

        let mut arrived = 0;
        while server.commands.try_recv().is_ok() {
            arrived += 1;
        }
        println!("commands on channel: {arrived}");
        assert_eq!(arrived, 1, "exactly one Play command");

        // And it is the play command, not just any command.
        let server = WebServer::start(0).expect("bind loopback");
        let _ = post(server.addr, &format!("/play?token={}", server.token));
        let cmd = server.commands.try_recv().expect("one command");
        assert!(matches!(cmd, AudioCommand::SetSongPlayback(true)));
    }
}
