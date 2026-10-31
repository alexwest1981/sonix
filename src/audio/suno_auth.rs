//! OAuth 2.1 for Suno's MCP endpoint: authorization code + PKCE, public client (no secret).
//!
//! Measured against the live service on 2026-10-31:
//! - `https://api.suno.com/mcp` answers an unauthenticated `initialize` with 401 and
//!   `WWW-Authenticate: Bearer resource_metadata="https://api.suno.com/.well-known/oauth-protected-resource/mcp"`.
//! - That metadata names `https://studio-api-prod.suno.com/api/v2/external/oauth` as the
//!   authorization server; its `/.well-known/oauth-authorization-server` advertises the code and
//!   refresh_token grants, S256 PKCE and dynamic client registration under `/register/`.
//!
//! So there is nothing to configure: register once, open a URL, catch the redirect on loopback.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Suno's MCP endpoint — the resource the token is issued for (RFC 8707).
pub const MCP_ENDPOINT: &str = "https://api.suno.com/mcp";

/// The authorization server, from the protected-resource metadata.
pub const AUTH_SERVER: &str = "https://studio-api-prod.suno.com/api/v2/external/oauth";

/// Everything the server advertises, asked for in full: `offline_access` is what makes the
/// grant yield a refresh token.
pub const SCOPES: &str = "api:v0 credits:spend download library:read library:search library:write offline_access profile:read";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds, from `expires_in`.
    pub expires_at: u64,
}

impl Tokens {
    pub fn expired(&self) -> bool {
        now_unix() >= self.expires_at
    }

    /// Never log the token itself — the first six characters identify it, nothing more.
    pub fn masked(&self) -> String {
        self.access_token.chars().take(6).collect::<String>() + "…"
    }

    /// Written 0600: the file is a bearer token for the user's account.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialising tokens: {e}"))?;
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            // mode at open time, so the file is never readable by others even for a moment;
            // set_permissions afterwards fixes a copy left at 0644 by an earlier version.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| format!("opening {}: {e}", path.display()))?;
            file.write_all(json.as_bytes())
                .map_err(|e| format!("writing {}: {e}", path.display()))?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("chmod 0600 {}: {e}", path.display()))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(path, json).map_err(|e| format!("writing {}: {e}", path.display()))?;
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Option<Tokens> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}

/// PKCE(S256) plus the `state` that ties the redirect back to this attempt.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn new() -> Result<Self, String> {
        let verifier = random_hex(32)?;
        let digest = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
        Ok(Self {
            challenge: base64url(digest.as_ref()),
            state: random_hex(16)?,
            verifier,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SunoOAuth {
    /// Resource indicator sent with the grant (RFC 8707) — this token is for the MCP endpoint.
    pub mcp: String,
    /// Authorization server base; the endpoints hang off it (`/authorize/`, `/token/`, …).
    pub auth_server: String,
    /// Where [`Tokens`] land after a login or a refresh.
    pub token_file: PathBuf,
    client_id: Option<String>,
}

impl Default for SunoOAuth {
    fn default() -> Self {
        Self {
            mcp: MCP_ENDPOINT.to_string(),
            auth_server: AUTH_SERVER.to_string(),
            token_file: crate::paths::paths().suno_tokens_file(),
            client_id: None,
        }
    }
}

impl SunoOAuth {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dynamic client registration. Public client (`none`), so no secret is stored. The id is
    /// kept for [`Self::login_blocking`] and for later refreshes.
    pub fn register_client(&mut self, redirect_uri: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "client_name": "Sonix",
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        });
        let text = post(
            &format!("{}/register/", self.auth_server),
            "application/json",
            &body.to_string(),
        )?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| format!("registration response is not JSON: {e}"))?;
        let id = v["client_id"]
            .as_str()
            .ok_or("registration response has no client_id")?
            .to_string();
        self.client_id = Some(id.clone());
        Ok(id)
    }

    pub fn authorize_url(&self, client_id: &str, redirect_uri: &str, pkce: &Pkce) -> String {
        let params = [
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("scope", SCOPES),
            ("state", pkce.state.as_str()),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("resource", self.mcp.as_str()),
        ];
        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .collect();
        format!("{}/authorize/?{}", self.auth_server, query.join("&"))
    }

    /// The redirect URI a login on `port` is registered for.
    pub fn redirect_uri(port: u16) -> String {
        format!("http://127.0.0.1:{port}/callback")
    }

    /// Binds the loopback listener first, then tells the user where to go, then waits.
    pub fn login_blocking(&self, port: u16) -> Result<Tokens, String> {
        self.login_blocking_with(port, &Pkce::new()?)
    }

    /// As [`Self::login_blocking`], with the PKCE values supplied — the tests need to know the
    /// `state` they echo back.
    pub fn login_blocking_with(&self, port: u16, pkce: &Pkce) -> Result<Tokens, String> {
        let client_id = self
            .client_id
            .clone()
            .ok_or("register_client must run before login_blocking")?;
        let redirect_uri = Self::redirect_uri(port);
        // Bound before the URL is printed: a browser that is already open cannot race us.
        let listener = std::net::TcpListener::bind(("127.0.0.1", port))
            .map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))?;
        let url = self.authorize_url(&client_id, &redirect_uri, pkce);
        println!("Open this URL to sign in to Suno: {url}");

        let (mut stream, _) = listener
            .accept()
            .map_err(|e| format!("waiting for the sign-in redirect: {e}"))?;
        let mut buf = [0u8; 4096];
        let read = stream
            .read(&mut buf)
            .map_err(|e| format!("reading the sign-in redirect: {e}"))?;
        let request = String::from_utf8_lossy(&buf[..read]).into_owned();

        let outcome = match query_value(&request, "error") {
            Some(error) => Err(format!("sign-in was refused: {error}")),
            None => match query_value(&request, "state") {
                state if state.as_deref() != Some(pkce.state.as_str()) => {
                    Err("sign-in redirect carried the wrong state".to_string())
                }
                _ => match query_value(&request, "code") {
                    Some(code) => Ok(code),
                    None => Err("sign-in redirect carried no code".to_string()),
                },
            },
        };

        let page = match &outcome {
            Ok(_) => "You are signed in — you can close this window.",
            Err(_) => "Sign-in failed — you can close this window and try again.",
        };
        let body =
            format!("<!doctype html><meta charset=\"utf-8\"><title>Sonix</title><p>{page}</p>");
        let head = format!(
            "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            if outcome.is_ok() { 200 } else { 400 },
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();

        let code = outcome?;
        let tokens = self.token_request(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &client_id),
            ("code_verifier", &pkce.verifier),
            ("resource", &self.mcp),
        ])?;
        tokens.save(&self.token_file)?;
        Ok(tokens)
    }

    pub fn refresh(&self, tokens: &Tokens) -> Result<Tokens, String> {
        let client_id = self
            .client_id
            .clone()
            .ok_or("register_client must run before refresh")?;
        let mut fresh = self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &tokens.refresh_token),
            ("client_id", &client_id),
            ("resource", &self.mcp),
        ])?;
        if fresh.refresh_token.is_empty() {
            fresh.refresh_token = tokens.refresh_token.clone();
        }
        fresh.save(&self.token_file)?;
        Ok(fresh)
    }

    /// The tokens to use now: refreshed once if they have expired. No loop — a refresh that
    /// fails means the user signs in again, and retrying the same refresh token cannot help.
    pub fn ensure_fresh(&self, tokens: &Tokens) -> Result<Tokens, String> {
        if tokens.expired() {
            self.refresh(tokens)
        } else {
            Ok(tokens.clone())
        }
    }

    fn token_request(&self, params: &[(&str, &str)]) -> Result<Tokens, String> {
        let form: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .collect();
        let text = post(
            &format!("{}/token/", self.auth_server),
            "application/x-www-form-urlencoded",
            &form.join("&"),
        )?;
        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("token response is not JSON: {e}"))?;
        let access_token = v["access_token"]
            .as_str()
            .ok_or("token response has no access_token")?
            .to_string();
        Ok(Tokens {
            access_token,
            refresh_token: v["refresh_token"].as_str().unwrap_or_default().to_string(),
            expires_at: now_unix() + v["expires_in"].as_u64().unwrap_or(3600),
        })
    }
}

fn post(url: &str, content_type: &str, body: &str) -> Result<String, String> {
    match ureq::post(url)
        .set("Content-Type", content_type)
        .set("Accept", "application/json")
        .send_string(body)
    {
        Ok(response) => response
            .into_string()
            .map_err(|e| format!("reading the answer from {url}: {e}")),
        Err(ureq::Error::Status(code, response)) => {
            let text = response.into_string().unwrap_or_default();
            Err(format!(
                "{url} returned {code}: {}",
                text.chars().take(300).collect::<String>()
            ))
        }
        Err(e) => Err(format!("{url}: {e}")),
    }
}

/// Percent-encodes everything outside the unreserved set (RFC 3986 §2.3).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// base64url without padding — the encoding PKCE specifies for the challenge.
fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        for i in 0..=chunk.len() {
            out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

fn random_hex(bytes: usize) -> Result<String, String> {
    use ring::rand::SecureRandom;
    let mut buf = vec![0u8; bytes];
    ring::rand::SystemRandom::new()
        .fill(&mut buf)
        .map_err(|_| "the system random generator failed".to_string())?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `name=value` from the query string of a request line (`GET /callback?code=… HTTP/1.1`).
fn query_value(request: &str, name: &str) -> Option<String> {
    let line = request.lines().next()?;
    let query = line.split_once('?')?.1;
    query
        .split_whitespace()
        .next()?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mcp_client::mock::{Mock, Reply};

    fn tmp_file(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sonix-suno-auth-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        dir.join("suno_tokens.json")
    }

    fn free_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind 0");
        listener.local_addr().expect("addr").port()
    }

    /// Sends the browser's half of the redirect, retrying until the listener is up.
    fn visit_callback(port: u16, query: &str) -> String {
        for _ in 0..200 {
            if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.write_all(
                    format!(
                        "GET /callback?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                );
                let mut answer = String::new();
                let _ = stream.read_to_string(&mut answer);
                return answer;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the loopback listener never came up on port {port}");
    }

    /// (d) Registration, the authorization URL, the loopback redirect, the code exchange and
    /// the 0600 token file — end to end against the mock.
    #[test]
    fn oauth_flow_end_to_end_against_the_mock() {
        let mock = Mock::new(vec![
            Reply::json(
                r#"{"client_id":"dcr-mock-42","client_name":"Sonix","scope":"api:v0 library:read offline_access"}"#,
            ),
            Reply::json(
                r#"{"access_token":"mock-access-token-abcdef","refresh_token":"mock-refresh-token","expires_in":3600,"token_type":"Bearer"}"#,
            ),
        ]);
        let token_file = tmp_file("e2e");
        let mut oauth = SunoOAuth {
            mcp: MCP_ENDPOINT.to_string(),
            auth_server: mock.url.clone(),
            token_file: token_file.clone(),
            client_id: None,
        };

        let port = free_port();
        let redirect = SunoOAuth::redirect_uri(port);
        let client_id = oauth.register_client(&redirect).expect("registration");
        println!("registered client_id: {client_id}");
        assert_eq!(client_id, "dcr-mock-42");

        let pkce = Pkce::new().expect("pkce");
        let url = oauth.authorize_url(&client_id, &redirect, &pkce);
        println!("authorize url: {url}");
        assert!(
            url.contains("code_challenge="),
            "no code_challenge in {url}"
        );
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("state={}", pkce.state)));
        assert!(url.contains("response_type=code"));

        let login = {
            let oauth = oauth.clone();
            let pkce = Pkce {
                verifier: pkce.verifier.clone(),
                challenge: pkce.challenge.clone(),
                state: pkce.state.clone(),
            };
            std::thread::spawn(move || oauth.login_blocking_with(port, &pkce))
        };
        let answer = visit_callback(port, &format!("code=mock-code-777&state={}", pkce.state));
        let tokens = login.join().expect("login thread").expect("login");
        let page = answer.lines().next().unwrap_or_default();
        println!("browser answer: {page}");
        assert!(
            answer.contains("200"),
            "browser did not get a 200: {answer}"
        );
        assert!(
            answer.to_lowercase().contains("close this window"),
            "no close-the-window page: {answer}"
        );

        println!("access token (masked): {}", tokens.masked());
        println!(
            "refresh token (masked): {}",
            tokens.refresh_token.chars().take(6).collect::<String>() + "…"
        );
        assert_eq!(tokens.access_token, "mock-access-token-abcdef");
        assert_eq!(tokens.refresh_token, "mock-refresh-token");
        assert!(tokens.expires_at > now_unix());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&token_file)
                .expect("token file")
                .permissions()
                .mode()
                & 0o777;
            println!("token file mode: {mode:o}");
            assert_eq!(mode, 0o600, "token file is not 0600");
        }
        assert_eq!(Tokens::load(&token_file).expect("stored tokens"), tokens);

        let reqs = mock.requests();
        assert_eq!(reqs.len(), 2);
        assert!(
            reqs[0].starts_with("POST /register/ "),
            "registration went elsewhere: {}",
            reqs[0].lines().next().unwrap_or_default()
        );
        assert!(reqs[0].contains("\"token_endpoint_auth_method\":\"none\""));
        assert!(
            reqs[1].starts_with("POST /token/ "),
            "exchange went elsewhere: {}",
            reqs[1].lines().next().unwrap_or_default()
        );
        assert!(reqs[1].contains("grant_type=authorization_code"));
        assert!(reqs[1].contains("code=mock-code-777"));
        assert!(reqs[1].contains(&format!("code_verifier={}", pkce.verifier)));
        println!("requests the mock saw: {}", mock.count());
        let _ = std::fs::remove_dir_all(token_file.parent().unwrap());
    }

    /// (e) Expired tokens are refreshed exactly once, then reused without a call.
    #[test]
    fn an_expired_token_refreshes_exactly_once() {
        let mock = Mock::new(vec![Reply::json(
            r#"{"access_token":"refreshed-access-xyz","expires_in":3600}"#,
        )]);
        let token_file = tmp_file("refresh");
        let mut oauth = SunoOAuth {
            mcp: MCP_ENDPOINT.to_string(),
            auth_server: mock.url.clone(),
            token_file: token_file.clone(),
            client_id: None,
        };
        oauth.client_id = Some("dcr-mock-42".to_string());

        let stale = Tokens {
            access_token: "stale-access-111111".to_string(),
            refresh_token: "stale-refresh".to_string(),
            expires_at: now_unix().saturating_sub(60),
        };
        assert!(stale.expired());

        let fresh = oauth.ensure_fresh(&stale).expect("refresh");
        println!("refreshed token (masked): {}", fresh.masked());
        assert_eq!(fresh.access_token, "refreshed-access-xyz");
        assert_eq!(
            fresh.refresh_token, "stale-refresh",
            "refresh token lost when the server did not rotate it"
        );
        assert!(!fresh.expired());
        let after_first = mock.count();
        println!("token endpoint calls the mock saw: {after_first}");
        assert_eq!(after_first, 1);

        // A second use of the now-fresh tokens must not hit the token endpoint at all.
        let reused = oauth.ensure_fresh(&fresh).expect("reuse");
        println!("token endpoint calls after reuse: {}", mock.count());
        assert_eq!(mock.count(), 1, "a valid token was refreshed again");
        assert_eq!(reused, fresh);
        let _ = std::fs::remove_dir_all(token_file.parent().unwrap());
    }

    #[test]
    fn pkce_challenge_is_s256_of_the_verifier() {
        let pkce = Pkce::new().expect("pkce");
        let expected = base64url(
            ring::digest::digest(&ring::digest::SHA256, pkce.verifier.as_bytes()).as_ref(),
        );
        println!(
            "challenge: {} (len {})",
            pkce.challenge,
            pkce.challenge.len()
        );
        assert_eq!(pkce.challenge, expected);
        assert_eq!(
            pkce.challenge.len(),
            43,
            "base64url of 32 bytes without padding"
        );
        assert!(pkce.verifier.len() >= 43 && pkce.verifier.len() <= 128);
        assert_eq!(pkce.state.len(), 32);
        assert_ne!(Pkce::new().expect("pkce").state, pkce.state);
    }
}
