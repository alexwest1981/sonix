//! **`sonix-mcp` — MCP-servern för Sonix.** En tunn översättare, ingen ny verktygsyta.
//!
//! Appen har redan allt som behövs: `ai_tools::tool_schema()` (15 verktyg, validerade, ett
//! ångringssteg per anrop) och en HTTP-yta på loopback med bearer-token
//! (`GET /v1/tools`, `POST /v1/execute`, `GET /v1/last`). Den här binären talar MCP över
//! stdio och skickar vidare dit — den äger inga verktyg, ingen validering och ingen state.
//!
//! Varför en egen process och inte en rutt i appen: **MCP över stdio måste äga stdout**, och
//! appen skriver logg där (`println!` i `main.rs`). En separat binär kostar ingenting och
//! fungerar med varje klient som kan starta ett kommando (Hermes, Claude Desktop, Codex, Zed).
//!
//! Körning: klienten startar `sonix-mcp`; appen måste redan vara igång.
//!
//! `shortcut: /v1/last` tar kvittona destruktivt, så två samtidiga MCP-klienter kan stjäla
//! varandras svar, och en app som är upptagen ger "köat" i stället för ett kvitto. Räcker för
//! en användare vid tangentbordet; uppgradera till en synkron `/v1/execute` om fler klienter
//! ska dela på appen.

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Appens AI-yta: samma port och samma tokenfil som `ai_service.rs`.
const DEFAULT_PORT: u16 = 4827;

/// Så länge väntar `tools/call` på att appen ska köra anropet på sin UI-tråd. Ett verktyg som
/// `master_to_target` renderar, så tio sekunder är inte generöst — det är nödvändigt.
const RECEIPT_TIMEOUT: Duration = Duration::from_secs(20);

fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SONIX_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("sonix")
}

fn token() -> Result<String, String> {
    let path = config_dir().join("ai_service_token");
    std::fs::read_to_string(&path)
        .map(|t| t.trim().to_string())
        .map_err(|e| format!("kan inte läsa {}: {e} — har Sonix startat någon gång?", path.display()))
}

fn port() -> u16 {
    std::env::var("SONIX_AI_API_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Vart appens AI-yta pekar. En struct i stället för att läsa miljön på djupet, så proven kan
/// peka på en attrapp utan att röra processens miljö (som är `unsafe` i edition 2024).
struct Endpoint {
    port: u16,
    token: String,
}

impl Endpoint {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            port: port(),
            token: token()?,
        })
    }
}

/// Minimal HTTP/1.1 över loopback. `Ok((status, body))` även för 4xx/5xx, så anroparen kan
/// säga vad appen svarade i stället för att bara "misslyckades".
fn http(
    ep: &Endpoint,
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
) -> Result<(u16, String), String> {
    let token = ep.token.clone();
    let mut stream = TcpStream::connect(("127.0.0.1", ep.port))
        .map_err(|e| format!("Sonix svarar inte på 127.0.0.1:{} ({e}) — är appen igång?", ep.port))?;
    // Tidsgränsen ägs av anroparen: en socket som väntar 30 s kan aldrig hålla en
    // kvittodeadline på 20 s.
    let _ = stream.set_read_timeout(Some(timeout));
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("kunde inte skriva till appen: {e}"))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("kunde inte läsa appens svar: {e}"))?;
    let text = String::from_utf8_lossy(&raw).to_string();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    Ok((status, body))
}

/// OpenAI-verktygsschema → MCP-verktyg. Ren funktion, så formen kan prövas utan en server.
fn to_mcp_tool(function: &serde_json::Value) -> serde_json::Value {
    let f = function.get("function").unwrap_or(function);
    serde_json::json!({
        "name": f.get("name").cloned().unwrap_or(serde_json::Value::Null),
        "description": f.get("description").cloned().unwrap_or(serde_json::Value::Null),
        "inputSchema": f
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}})),
    })
}

fn tools(ep: &Endpoint) -> Result<Vec<serde_json::Value>, String> {
    let (status, body) = http(ep, "GET", "/v1/tools", None, Duration::from_secs(10))?;
    if status != 200 {
        return Err(format!("GET /v1/tools svarade {status}: {body}"));
    }
    let schema: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("tools-schemat var inte JSON: {e}"))?;
    // **Appen svarar med en naken lista** (mätt mot den körande appen 2026-11-03), men en
    // framtida version kan lägga den under `tools` — båda läses, så formen inte är en fälla.
    let list = match &schema {
        serde_json::Value::Array(items) => items,
        other => other
            .get("tools")
            .and_then(|t| t.as_array())
            .ok_or_else(|| "schemat är varken en lista eller {\"tools\": […]".to_string())?,
    };
    Ok(list.iter().map(to_mcp_tool).collect())
}

/// Kör ett verktyg och väntar på kvittot.
fn call(ep: &Endpoint, name: &str, arguments: serde_json::Value) -> Result<String, String> {
    let payload = serde_json::json!({"calls": [{"tool": name, "args": arguments}]}).to_string();
    // `/v1/last` är destruktiv och har ingen korrelations-id: töm den **före** anropet, så ett
    // kvitto från ett tidigare anrop (t.ex. ett som timade ut) inte kan bli svaret på det här.
    let _ = http(ep, "GET", "/v1/last", None, Duration::from_secs(5));
    let (status, body) = http(ep, "POST", "/v1/execute", Some(&payload), Duration::from_secs(5))?;
    if status == 401 {
        return Err("token avvisades (401) — tokenfilen och appen är inte samma".to_string());
    }
    if status != 202 && status != 200 {
        return Err(format!("POST /v1/execute svarade {status}: {body}"));
    }
    let started = Instant::now();
    while started.elapsed() < RECEIPT_TIMEOUT {
        std::thread::sleep(Duration::from_millis(120));
        let left = RECEIPT_TIMEOUT
            .saturating_sub(started.elapsed())
            .max(Duration::from_millis(500));
        let (s, b) = http(ep, "GET", "/v1/last", None, left)?;
        if s == 401 {
            return Err("token avvisades (401) — tokenfilen och appen är inte samma".to_string());
        }
        if s != 200 {
            continue;
        }
        if let Ok(lines) = serde_json::from_str::<Vec<serde_json::Value>>(&b) {
            if let Some(first) = lines.into_iter().next() {
                // **Appens dom, inte adapterns gissning.** En äldre app svarar med bara texten;
                // då finns ingen flagga och anropet räknas som lyckat, som förut.
                return match first {
                    serde_json::Value::Object(o) => {
                        let text = o
                            .get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        if o.get("ok").and_then(|v| v.as_bool()).unwrap_or(true) {
                            Ok(text)
                        } else {
                            Err(text)
                        }
                    }
                    serde_json::Value::String(text) => Ok(text),
                    other => Ok(other.to_string()),
                };
            }
        }
    }
    // **Ärligt fel, inte en tyst framgång**: anropet kan ha körts, eller inte. En MCP-klient
    // avgör på `isError`, så ett "kanske" får aldrig se ut som ett kvitto.
    Err(format!(
        "{name}: inget kvitto inom {} s — appen är upptagen (export, rendering eller en modal); anropet kan ändå ha körts",
        RECEIPT_TIMEOUT.as_secs()
    ))
}

/// Ett JSON-RPC-svar, eller `None` för en notifiering (den ska inte besvaras).
fn handle(msg: &serde_json::Value, ep: &Endpoint) -> Option<serde_json::Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let reply = |result: serde_json::Value| {
        Some(serde_json::json!({"jsonrpc": "2.0", "id": id.clone().unwrap_or(serde_json::Value::Null), "result": result}))
    };
    let fail = |code: i64, text: String| {
        Some(serde_json::json!({"jsonrpc": "2.0", "id": id.clone().unwrap_or(serde_json::Value::Null), "error": {"code": code, "message": text}}))
    };

    match method {
        "initialize" => reply(serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "sonix", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Sonix Studio, en DAW. Verktygen styr det projekt som är öppet i appen. \
Läs med analyze_mix/groove_report innan du ändrar; plan_sections föreslår utan att skriva."
        })),
        // Notifieringar har inget id och ska inte besvaras.
        m if id.is_none() => {
            let _ = m;
            None
        }
        "ping" => reply(serde_json::json!({})),
        "tools/list" => match tools(ep) {
            Ok(list) => reply(serde_json::json!({"tools": list})),
            Err(e) => fail(-32603, e),
        },
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| serde_json::json!({}));
            if name.is_empty() {
                return fail(-32602, "tools/call saknar \"name\"".to_string());
            }
            match call(ep, name, args) {
                Ok(text) => reply(serde_json::json!({
                    "content": [{"type": "text", "text": text}],
                    "isError": false
                })),
                Err(e) => reply(serde_json::json!({
                    "content": [{"type": "text", "text": e}],
                    "isError": true
                })),
            }
        }
        other => fail(-32601, format!("metoden '{other}' finns inte här")),
    }
}

fn main() {
    let ep = match Endpoint::from_env() {
        Ok(ep) => ep,
        Err(e) => {
            // Hellre ett tydligt fel på stderr än en server som svarar på allt utom anrop.
            eprintln!("sonix-mcp: {e}");
            std::process::exit(1);
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                // En trasig rad ska inte döda servern: svara med ett fel och läs vidare.
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("inte JSON: {e}")}})
                );
                let _ = stdout.flush();
                continue;
            }
        };
        if let Some(answer) = handle(&msg, &ep) {
            let _ = writeln!(stdout, "{answer}");
            let _ = stdout.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// En attrapp för appens AI-yta: samma tre rutter, inget UI.
    fn mock_app(stale_receipt: Option<&'static str>, failure: bool) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            let mut last_calls = 0u32;
            for _ in 0..8 {
                let Ok((mut s, _)) = listener.accept() else { return };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /v1/last") && stale_receipt.is_some() && last_calls == 0 {
                    last_calls += 1;
                    format!("[{}]", serde_json::json!(stale_receipt.unwrap()))
                } else if req.starts_with("GET /v1/last") && stale_receipt.is_some() && last_calls == 1 {
                    last_calls += 1;
                    "[]".to_string()
                } else if req.starts_with("GET /v1/tools") {
                    r#"[{"type":"function","function":{"name":"set_tempo","description":"Sätter tempo.","parameters":{"type":"object","properties":{"bpm":{"type":"number"}},"required":["bpm"]}}}]"#.to_string()
                } else if req.starts_with("POST /v1/execute") {
                    "{\"queued\":1}".to_string()
                } else if failure {
                    "[{\"ok\": false, \"text\": \"tempo 400 ligger utanför 40–240\"}]".to_string()
                } else {
                    "[{\"ok\": true, \"text\": \"🎚 tempo 128 BPM\"}]".to_string()
                };
                let status = if req.starts_with("POST /v1/execute") {
                    "202 Accepted"
                } else {
                    "200 OK"
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (port, handle)
    }

    /// Verktygsschemat ska komma ut i MCP-form: `inputSchema`, inte `parameters`.
    #[test]
    fn the_schema_is_translated_to_mcp_shape() {
        let openai = serde_json::json!({"type": "function", "function": {
            "name": "set_tempo", "description": "Sätter tempo.",
            "parameters": {"type": "object", "properties": {"bpm": {"type": "number"}}}
        }});
        let tool = to_mcp_tool(&openai);
        assert_eq!(tool["name"], "set_tempo");
        assert_eq!(tool["inputSchema"]["properties"]["bpm"]["type"], "number");
        assert!(tool.get("parameters").is_none(), "MCP vill ha inputSchema");
    }

    fn endpoint(port: u16) -> Endpoint {
        Endpoint { port, token: "test-token".to_string() }
    }

    /// `initialize` svarar med protokollversion och serverinfo, och en notifiering besvaras inte.
    #[test]
    fn initialize_answers_and_notifications_are_silent() {
        let ep = endpoint(1);
        let init = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
        let answer = handle(&init, &ep).expect("svar");
        assert_eq!(answer["result"]["serverInfo"]["name"], "sonix");
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");

        let note = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(handle(&note, &ep).is_none(), "en notifiering ska inte besvaras");
    }

    /// Hela vägen mot en attrapp: lista verktyg, kör ett, och få appens kvitto tillbaka.
    #[test]
    fn tools_list_and_call_go_through_the_apps_http_surface() {
        let (port, _server) = mock_app(None, false);
        let ep = endpoint(port);

        let list = handle(&serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}), &ep)
            .expect("svar");
        assert_eq!(list["result"]["tools"][0]["name"], "set_tempo");

        let call = handle(&serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "set_tempo", "arguments": {"bpm": 128}}
        }), &ep)
        .expect("svar");
        assert_eq!(call["result"]["isError"], false);
        assert!(
            call["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .contains("tempo"),
            "kvittot ska komma från appen: {call}"
        );
    }

    /// Ett kvitto som ligger kvar från ett tidigare anrop får inte bli svaret på nästa:
    /// `/v1/last` har ingen korrelations-id, så adaptern tömmer den före anropet.
    #[test]
    fn a_stale_receipt_is_not_returned_as_the_answer() {
        let (port, _server) = mock_app(Some("gammalt kvitto från förra anropet"), false);
        let ep = endpoint(port);
        let answer = handle(
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {"name": "set_tempo", "arguments": {"bpm": 128}}
            }),
            &ep,
        )
        .expect("svar");
        let text = answer["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(!text.contains("gammalt"), "gammalt kvitto läckte: {text}");
        assert!(text.contains("tempo"), "fel kvitto: {text}");
    }

    /// **Ett avvisat anrop ska bli `isError: true`.** Appen sätter `ok:false` på kvittot, och
    /// adaptern får inte göra det till en framgång — annars rapporterar modellen ett tempo som
    /// aldrig sattes.
    #[test]
    fn a_rejected_call_becomes_is_error() {
        let (port, _server) = mock_app(None, true);
        let ep = endpoint(port);
        let answer = handle(
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                "params": {"name": "set_tempo", "arguments": {"bpm": 400}}
            }),
            &ep,
        )
        .expect("svar");
        assert_eq!(answer["result"]["isError"], true, "{answer}");
        let text = answer["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(text.contains("utanför"), "appen ska förklara: {text}");
    }
}
