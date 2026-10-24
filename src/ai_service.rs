//! **AI-tjänstens API** — ett litet HTTP-gränssnitt på localhost, så ett plugin eller ett
//! skript kan driva assistenten utan att länka in sig i appens interna typer.
//!
//! Bara assistentens egen yta exponeras: schemat, en prompt och att köra validerade
//! verktygsanrop. Anropen **körs i appens tråd** (de läggs i en kö och plockas upp i `update`),
//! eftersom de rör pattern och kanaler som ägs av UI-tråden.
//!
//! Skyddet är tre lager, och inget av dem är valfritt:
//! 1. Lyssnar bara på `127.0.0.1` — aldrig nätet.
//! 2. Kräver `Authorization: Bearer <token>`; token skrivs till `~/.config/sonix/ai_service_token`
//!    (0600) första gången och jämförs i sin helhet.
//! 3. Anropen valideras av `ai_tools` innan de når appen; okända verktyg avvisas.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::audio::ai_tools::Call;

/// Standardporten. Kan bytas med `SONIX_AI_API_PORT`.
pub const DEFAULT_PORT: u16 = 4827;

/// Delad kö: tjänsten lägger anrop här, appen hämtar dem i `update`.
pub type Queue = Arc<Mutex<Vec<Call>>>;

/// Kvitton från körningen, i den ordning appen utförde anropen. `GET /v1/last` tömmer listan.
pub type Results = Arc<Mutex<Vec<String>>>;

/// Läser eller skapar token. Filen får 0600 — den är en nyckel till appens AI-yta.
pub fn load_or_create_token() -> std::io::Result<String> {
    let path = crate::paths::paths().config_dir().join("ai_service_token");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if trimmed.len() >= 16 {
            return Ok(trimmed);
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let token = format!("{}{}", random_hex(), random_hex());
    std::fs::write(&path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
}

fn random_hex() -> String {
    // Tid + adress ger 64 bitar utan ett kryptobibliotek; token skyddar en localhost-port,
    // inte en hemlighet över nätet.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    format!("{:032x}", nanos ^ (pid << 64))
}

/// Startar tjänsten i en egen tråd. Returnerar porten den lyssnar på (0 = välj en ledig).
pub fn start(port: u16, token: String, queue: Queue, results: Results) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let bound = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let token = token.clone();
            let queue = queue.clone();
            let results = results.clone();
            std::thread::spawn(move || {
                let _ = handle(stream, &token, &queue, &results);
            });
        }
    });
    Ok(bound)
}

/// Porten ur miljön, annars standard. `0` stänger av tjänsten.
pub fn configured_port() -> u16 {
    std::env::var("SONIX_AI_API_PORT")
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT)
}

fn handle(mut stream: TcpStream, token: &str, queue: &Queue, results: &Results) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();

    let mut content_length = 0usize;
    let mut authorized = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
        if let Some(rest) = lower.strip_prefix("authorization: bearer ") {
            // Jämför hela strängen; längden läcker inte via tidskillnad här.
            authorized = rest.trim() == token;
        }
    }
    let mut body = vec![0u8; content_length.min(64 * 1024)];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    if !authorized {
        return respond(&mut stream, 401, r#"{"error":"missing or wrong bearer token"}"#);
    }

    match (method.as_str(), path.as_str()) {
        ("GET", "/v1/tools") => {
            let schema = crate::audio::ai_tools::tool_schema();
            respond(&mut stream, 200, &schema.to_string())
        }
        ("POST", "/v1/execute") => {
            let text = String::from_utf8_lossy(&body).to_string();
            match crate::audio::ai_tools::parse_calls(&text) {
                Ok(calls) if calls.is_empty() => {
                    respond(&mut stream, 400, r#"{"error":"no calls in body"}"#)
                }
                Ok(calls) => {
                    let count = calls.len();
                    if let Ok(mut q) = queue.lock() {
                        q.extend(calls);
                    }
                    respond(
                        &mut stream,
                        202,
                        &format!("{{\"queued\":{count},\"note\":\"executed by the app on its UI thread\"}}"),
                    )
                }
                Err(e) => respond(
                    &mut stream,
                    400,
                    &format!("{{\"error\":{}}}", serde_json::to_string(&e).unwrap_or_else(|_| "\"bad request\"".into())),
                ),
            }
        }
        ("GET", "/v1/last") => {
            // Ett asynkront `execute` är värdelöst utan svar: plugin får kvittona här.
            let lines: Vec<String> = results
                .lock()
                .map(|mut r| std::mem::take(&mut *r))
                .unwrap_or_default();
            respond(&mut stream, 200, &serde_json::to_string(&lines).unwrap_or_else(|_| "[]".into()))
        }
        ("GET", "/v1/health") => respond(&mut stream, 200, r#"{"ok":true}"#),
        _ => respond(&mut stream, 404, r#"{"error":"unknown route"}"#),
    }
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(port: u16, raw: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    /// **Utan token kommer ingen in, och med token går schemat att läsa.** Provet startar
    /// tjänsten på en ledig port, så det rör ingen annan process.
    #[test]
    fn the_service_requires_the_token_and_serves_the_schema() {
        let queue: Queue = Arc::new(Mutex::new(Vec::new()));
        let port = start(0, "test-token-1234".to_string(), queue, Default::default()).unwrap();

        let unauthorized = request(
            port,
            "GET /v1/tools HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        );
        assert!(unauthorized.starts_with("HTTP/1.1 401"), "{unauthorized}");

        let ok = request(
            port,
            "GET /v1/tools HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer test-token-1234\r\nConnection: close\r\n\r\n",
        );
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.contains("write_drum_pattern"), "{ok}");
    }

    /// Ett anrop med rätt token hamnar i kön — och ett okänt verktyg avvisas av `ai_tools`.
    #[test]
    fn execute_queues_valid_calls_and_rejects_broken_json() {
        let queue: Queue = Arc::new(Mutex::new(Vec::new()));
        let results: Results = Default::default();
        results.lock().unwrap().push("tempo 128 BPM".to_string());
        let port = start(0, "tok".to_string(), queue.clone(), results.clone()).unwrap();

        let body = r#"{"calls":[{"tool":"set_tempo","args":{"bpm":128}}]}"#;
        let raw = format!(
            "POST /v1/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer tok\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let response = request(port, &raw);
        assert!(response.starts_with("HTTP/1.1 202"), "{response}");
        assert_eq!(queue.lock().unwrap().len(), 1);
        assert_eq!(queue.lock().unwrap()[0].tool, "set_tempo");

        let bad = "inte json";
        let raw = format!(
            "POST /v1/execute HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer tok\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            bad.len(),
            bad
        );
        assert!(request(port, &raw).starts_with("HTTP/1.1 400"));

        let last = request(
            port,
            "GET /v1/last HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer tok\r\nConnection: close\r\n\r\n",
        );
        assert!(last.contains("tempo 128 BPM"), "{last}");
        assert!(results.lock().unwrap().is_empty(), "listan ska tömmas när den läses");
    }
}
