//! Minimal MCP client: JSON-RPC 2.0 over the Streamable HTTP transport.
//!
//! Only what a Suno library browser needs — `initialize`, `tools/list`, `tools/call`. The
//! transport allows a plain JSON body or an SSE stream as the answer, so both are parsed here;
//! a server that streams its reply and one that does not must look identical to the caller.
//!
//! **No retry.** A 401 means the token is expired, and sending the same token again cannot fix
//! that — the caller refreshes the token and builds a new client.

use serde_json::{Value, json};
use std::time::Duration;

/// Suno's MCP endpoint (measured: an unauthenticated initialize gets 401 + resource_metadata).
pub const MCP_ENDPOINT: &str = "https://api.suno.com/mcp";

/// The protocol revision we speak. Sent on every request after `initialize`, as the spec asks.
const PROTOCOL_VERSION: &str = "2025-06-18";
const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpError {
    /// HTTP 401: the token is expired or revoked. Refresh it; do not retry with the same one.
    Expired,
    /// Anything that stopped the call short of a JSON-RPC error (network, status, body shape).
    Transport(String),
    /// The server answered with a JSON-RPC `error` object.
    Rpc(String),
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::Expired => write!(f, "Suno access token has expired — sign in again"),
            McpError::Transport(m) => write!(f, "MCP transport: {m}"),
            McpError::Rpc(m) => write!(f, "MCP error: {m}"),
        }
    }
}

impl std::error::Error for McpError {}

#[derive(Debug, Clone)]
pub struct McpClient {
    pub endpoint: String,
    pub token: String,
    /// `Mcp-Session-Id` from `initialize`; carried on every later request.
    pub session: Option<String>,
    next_id: u64,
}

impl McpClient {
    pub fn new(endpoint: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            token: token.into(),
            session: None,
            next_id: 0,
        }
    }

    /// Handshake. Records the session id and sends `notifications/initialized`, as the spec
    /// requires before any other request.
    pub fn initialize(&mut self) -> Result<Value, McpError> {
        let result = self.rpc(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "sonix", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        self.notify("notifications/initialized")?;
        Ok(result)
    }

    pub fn tools_list(&mut self) -> Result<Vec<McpTool>, McpError> {
        let result = self.rpc("tools/list", json!({}))?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| McpError::Transport("tools/list answer has no tools array".into()))?;
        Ok(tools
            .iter()
            .map(|t| McpTool {
                name: t["name"].as_str().unwrap_or_default().to_string(),
                description: t["description"].as_str().unwrap_or_default().to_string(),
                input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
            })
            .collect())
    }

    /// Returns the JSON-RPC `result` of the tool call (`content` etc.), untouched.
    pub fn call_tool(&mut self, name: &str, args: Value) -> Result<Value, McpError> {
        self.rpc("tools/call", json!({ "name": name, "arguments": args }))
    }

    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        self.next_id += 1;
        let id = self.next_id;
        let body = self.post(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        let msg = pick_response(&body, id).ok_or_else(|| {
            McpError::Transport(format!(
                "{method}: no JSON-RPC response in body: {}",
                truncate(&body, 200)
            ))
        })?;
        if let Some(err) = msg.get("error") {
            return Err(McpError::Rpc(format!(
                "{} (code {})",
                err["message"].as_str().unwrap_or("unknown"),
                err["code"]
            )));
        }
        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
    }

    fn notify(&mut self, method: &str) -> Result<(), McpError> {
        self.post(&json!({ "jsonrpc": "2.0", "method": method }))
            .map(|_| ())
    }

    fn post(&mut self, payload: &Value) -> Result<String, McpError> {
        let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).build();
        let mut req = agent
            .post(&self.endpoint)
            .set("Content-Type", "application/json")
            // Both, in this order: the server picks either framing.
            .set("Accept", "application/json, text/event-stream")
            .set("MCP-Protocol-Version", PROTOCOL_VERSION)
            .set("Authorization", &format!("Bearer {}", self.token));
        if let Some(session) = &self.session {
            req = req.set("Mcp-Session-Id", session);
        }
        match req.send_string(&payload.to_string()) {
            Ok(resp) => {
                if let Some(session) = resp.header("Mcp-Session-Id") {
                    self.session = Some(session.to_string());
                }
                resp.into_string()
                    .map_err(|e| McpError::Transport(format!("reading the reply: {e}")))
            }
            Err(ureq::Error::Status(401, _)) => Err(McpError::Expired),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(McpError::Transport(format!(
                    "HTTP {code}: {}",
                    truncate(&text, 200)
                )))
            }
            Err(e) => Err(McpError::Transport(e.to_string())),
        }
    }
}

/// Every JSON-RPC message in the body, whichever framing the answer used.
fn responses(body: &str) -> Vec<Value> {
    let body = body.trim_start();
    if body.starts_with('{') {
        return serde_json::from_str(body).ok().into_iter().collect();
    }
    let mut out = Vec::new();
    let mut data: Vec<&str> = Vec::new();
    // The trailing "" flushes an event that the server closed without a blank line.
    for line in body.lines().chain(std::iter::once("")) {
        if let Some(d) = line.strip_prefix("data:") {
            data.push(d.trim_start());
        } else if line.trim().is_empty() && !data.is_empty() {
            if let Ok(v) = serde_json::from_str::<Value>(&data.join("\n")) {
                out.push(v);
            }
            data.clear();
        }
    }
    out
}

/// The message answering our request id (or, for a server that omits ids, the last one).
fn pick_response(body: &str, id: u64) -> Option<Value> {
    let all = responses(body);
    all.iter()
        .find(|v| v.get("id").and_then(Value::as_u64) == Some(id))
        .or_else(|| all.last())
        .cloned()
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

/// A hand-written HTTP server on `127.0.0.1:0`: the tests never touch the network.
///
/// It serves exactly one connection per scripted reply and then stops accepting, so a client
/// that retries runs out of replies instead of quietly passing.
#[cfg(test)]
pub(crate) mod mock {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;

    pub struct Reply {
        pub status: u16,
        pub headers: Vec<(&'static str, String)>,
        pub body: String,
    }

    impl Reply {
        pub fn json(body: impl Into<String>) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type", "application/json".into())],
                body: body.into(),
            }
        }

        pub fn sse(body: impl Into<String>) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type", "text/event-stream".into())],
                body: body.into(),
            }
        }

        /// A bare status with an empty body (202 for a notification, 401 for a dead token).
        pub fn status(code: u16) -> Self {
            Self {
                status: code,
                headers: Vec::new(),
                body: String::new(),
            }
        }

        pub fn header(mut self, key: &'static str, value: impl Into<String>) -> Self {
            self.headers.push((key, value.into()));
            self
        }
    }

    pub struct Mock {
        pub url: String,
        seen: Arc<Mutex<Vec<String>>>,
        _thread: JoinHandle<()>,
    }

    impl Mock {
        pub fn new(replies: Vec<Reply>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("mock: bind a free port");
            let url = format!(
                "http://{}",
                listener.local_addr().expect("mock: local addr")
            );
            let seen = Arc::new(Mutex::new(Vec::new()));
            let log = Arc::clone(&seen);
            let thread = std::thread::spawn(move || {
                for reply in replies {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    log.lock().unwrap().push(read_request(&mut stream));
                    let mut head = format!(
                        "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (key, value) in &reply.headers {
                        head.push_str(&format!("{key}: {value}\r\n"));
                    }
                    head.push_str("\r\n");
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(reply.body.as_bytes());
                    let _ = stream.flush();
                }
            });
            Self {
                url,
                seen,
                _thread: thread,
            }
        }

        pub fn count(&self) -> usize {
            self.seen.lock().unwrap().len()
        }

        pub fn requests(&self) -> Vec<String> {
            self.seen.lock().unwrap().clone()
        }
    }

    /// Request line + headers + body, read until Content-Length is satisfied.
    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let Ok(n) = stream.read(&mut chunk) else {
                break;
            };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).into_owned();
            if let Some(end) = text.find("\r\n\r\n") {
                let head = &text[..end];
                let len = head
                    .lines()
                    .filter_map(|l| l.split_once(':'))
                    .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{Mock, Reply};
    use super::*;

    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"suno","version":"0.1"}}}"#;
    const TOOLS: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[
        {"name":"library_search","description":"Search the user's Suno library","inputSchema":{"type":"object","properties":{"query":{"type":"string"}}}},
        {"name":"credits_balance","description":"Current credit balance","inputSchema":{"type":"object"}}]}}"#;
    const CALL: &str = r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"3 tracks found"}],"isError":false}}"#;

    /// (a) initialize + tools/list + tools/call against the mock, printing what came back.
    #[test]
    fn initialize_tools_list_and_call_round_trip() {
        let mock = Mock::new(vec![
            Reply::json(INIT).header("Mcp-Session-Id", "sess-mock-1"),
            Reply::status(202),
            Reply::json(TOOLS),
            Reply::json(CALL),
        ]);
        let mut client = McpClient::new(&mock.url, "tok-test-123456");

        let init = client.initialize().expect("initialize");
        println!("initialize: {init}");

        let tools = client.tools_list().expect("tools/list");
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        println!("tool names: {names:?}");
        for tool in &tools {
            println!("  {} — {}", tool.name, tool.description);
            assert!(!tool.name.is_empty());
            assert!(tool.input_schema.is_object(), "inputSchema lost");
        }
        assert_eq!(names, ["library_search", "credits_balance"]);

        let result = client
            .call_tool("library_search", json!({"query":"ambient"}))
            .expect("tools/call");
        println!("tools/call result: {result}");
        assert_eq!(result["content"][0]["text"], "3 tracks found");

        // The session id from initialize rode along, and the notification was sent.
        assert_eq!(client.session.as_deref(), Some("sess-mock-1"));
        let reqs = mock.requests();
        assert_eq!(reqs.len(), 4, "expected 4 requests, saw {}", reqs.len());
        assert!(reqs[0].contains("\"method\":\"initialize\""));
        assert!(reqs[1].contains("\"method\":\"notifications/initialized\""));
        assert!(
            reqs[1].contains("Mcp-Session-Id: sess-mock-1"),
            "session header missing on the second request: {}",
            reqs[1]
        );
        assert!(reqs[0].contains("Accept: application/json, text/event-stream"));
        assert_eq!(mock.count(), 4);
    }

    /// (b) The same payload framed as SSE parses to exactly the JSON-framed result.
    #[test]
    fn sse_framed_reply_parses_like_json() {
        let sse_body = format!("event: message\ndata: {CALL}\n\n");
        let mock = Mock::new(vec![Reply::json(CALL), Reply::sse(sse_body)]);
        let mut client = McpClient::new(&mock.url, "tok-test-123456");

        let json_result = client
            .call_tool("library_search", json!({"query":"ambient"}))
            .expect("json-framed call");
        print!("json framed result: {json_result}");
        println!();
        let sse_result = client
            .call_tool("library_search", json!({"query":"ambient"}))
            .expect("sse-framed call");
        print!("sse framed result:  {sse_result}");
        println!();
        println!("equal: {}", json_result == sse_result);
        assert_eq!(json_result, sse_result);
        assert_eq!(mock.count(), 2);
    }

    /// (c) A 401 is a typed error, and the client does not try again.
    #[test]
    fn a_401_is_a_typed_expired_error_without_retry() {
        let mock = Mock::new(vec![Reply::status(401).header(
            "WWW-Authenticate",
            "Bearer resource_metadata=\"https://api.suno.com/.well-known/oauth-protected-resource/mcp\"",
        )]);
        let mut client = McpClient::new(&mock.url, "tok-stale-999999");

        let err = client.initialize().expect_err("401 must not succeed");
        println!("typed error: {err}");
        println!("error kind: {:?}", err);
        println!("requests the mock saw: {}", mock.count());
        assert_eq!(err, McpError::Expired);
        assert_eq!(mock.count(), 1, "the client retried a 401");
    }
}
