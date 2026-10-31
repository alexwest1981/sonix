//! A Suno session: sign in, find the song-generation tool in whatever the beta server exposes,
//! ask for a song, download what came back.
//!
//! **No tool name is assumed.** The MCP surface is in beta and its names are guesses here; the
//! generation tool is picked by scoring the server's own `tools/list` (name, description and
//! argument names), and its arguments are built from its own input schema. When nothing scores,
//! the error lists what the server actually exposed — that message is the useful output.
//!
//! Files land in [`crate::paths::Paths::suno_dir`], one folder per song, numbered in download
//! order so a stem import gets a stable channel order.

use crate::audio::mcp_client::{McpClient, McpError, McpTool};
use crate::audio::suno_auth::{SunoOAuth, Tokens};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Loopback port for the browser sign-in. Fixed: the redirect URI is registered with it.
const SIGN_IN_PORT: u16 = 8763;
/// A song is generated asynchronously; the status tool is asked this many times, this far apart.
const POLL_TRIES: usize = 60;
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

/// The four extensions an audio result may arrive as. `.zip` is a stem bundle.
const AUDIO_EXT: [&str; 4] = [".wav", ".mp3", ".flac", ".zip"];

#[derive(Debug, Clone)]
pub struct GeneratedSong {
    /// Every audio file that arrived, in download order (stems: one file each).
    pub files: Vec<PathBuf>,
    /// More than one file, a `.zip`, or a `stems` key in the answer.
    pub stems: bool,
    pub title: String,
    /// The last `tools/call` result, untouched.
    pub raw: Value,
}

pub struct SunoSession {
    client: McpClient,
    tools: Vec<McpTool>,
}

impl SunoSession {
    /// Saved tokens if they are still valid, a refresh if they are not, a browser sign-in if
    /// there are none. Then MCP `initialize` + `tools/list`.
    pub fn connect() -> Result<SunoSession, String> {
        Self::connect_with_slot(None)
    }

    /// As [`Self::connect`], but the sign-in URL is handed to `url_slot` so the panel can show
    /// it: the browser is opened automatically, and when that fails the address is the way in.
    pub fn connect_with_slot(
        url_slot: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
    ) -> Result<SunoSession, String> {
        let oauth = SunoOAuth::new();
        connect_at(&oauth.mcp, &oauth.token_file, url_slot)
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn endpoint(&self) -> &str {
        &self.client.endpoint
    }

    pub fn generate(
        &mut self,
        prompt: &str,
        style: Option<&str>,
        lyrics: Option<&str>,
    ) -> Result<GeneratedSong, String> {
        self.generate_into(prompt, style, lyrics, &crate::paths::paths().suno_dir())
    }

    /// As [`Self::generate`], with the download folder supplied (the tests use a temp dir).
    fn generate_into(
        &mut self,
        prompt: &str,
        style: Option<&str>,
        lyrics: Option<&str>,
        base: &Path,
    ) -> Result<GeneratedSong, String> {
        let tool = pick_generation(&self.tools)?.clone();
        let args = build_args(&tool, prompt, style, lyrics);
        println!("suno: tools/call {} {args}", tool.name);
        let mut result = self
            .client
            .call_tool(&tool.name, args)
            .map_err(|e| e.to_string())?;

        // An asynchronous generator answers with an id and a status; ask until audio appears.
        let status = self
            .tools
            .iter()
            .find(|t| {
                let n = t.name.to_lowercase();
                n.contains("status") || n.contains("poll")
            })
            .cloned();
        // A nested `id` is the song id until the beta proves otherwise.
        let id = find_string(&expand(&result), &["song_id", "job_id", "task_id", "id"]);
        if let (Some(status), Some(id)) = (status, id) {
            for attempt in 1..=POLL_TRIES {
                if !audio_urls(&expand(&result)).is_empty() {
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
                let args = status_args(&status, &id);
                println!(
                    "suno: poll {attempt}/{POLL_TRIES} via {} {args}",
                    status.name
                );
                result = self
                    .client
                    .call_tool(&status.name, args)
                    .map_err(|e| e.to_string())?;
            }
        }

        let payload = expand(&result);
        let urls = audio_urls(&payload);
        if urls.is_empty() {
            let shown: String = payload.to_string().chars().take(400).collect();
            return Err(format!(
                "suno: `{}` returned no audio (.wav/.mp3/.flac/.zip): {shown}",
                tool.name
            ));
        }

        let title = find_string(&payload, &["title", "song_title"])
            .unwrap_or_else(|| prompt.chars().take(60).collect());
        let dir = unique_dir(base, &title)?;
        let mut files = Vec::new();
        for (index, url) in urls.iter().enumerate() {
            let path = download(url, &dir, index + 1)?;
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("suno: saved {} ({bytes} bytes)", path.display());
            files.push(path);
        }
        let stems =
            files.len() > 1 || urls.iter().any(|u| url_ext(u) == ".zip") || has_stems(&payload);
        println!(
            "suno: \"{title}\" — {} file(s), stems: {stems}",
            files.len()
        );
        Ok(GeneratedSong {
            files,
            stems,
            title,
            raw: result,
        })
    }
}

/// The body of [`SunoSession::connect`], with the endpoint and token file supplied so the mock
/// can drive it.
fn connect_at(
    endpoint: &str,
    token_file: &Path,
    url_slot: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
) -> Result<SunoSession, String> {
    let mut oauth = SunoOAuth::new();
    oauth.mcp = endpoint.to_string();
    oauth.token_file = token_file.to_path_buf();

    let tokens = match Tokens::load(token_file) {
        Some(saved) if !saved.expired() => saved,
        Some(stale) => {
            let client_id = std::fs::read_to_string(client_id_file(token_file)).ok();
            match client_id.map(|id| oauth.clone().with_client_id(id.trim()).refresh(&stale)) {
                Some(Ok(fresh)) => fresh,
                Some(Err(e)) => {
                    println!("suno: refresh failed ({e}); signing in again");
                    sign_in(&mut oauth, url_slot.clone())?
                }
                None => sign_in(&mut oauth, url_slot.clone())?,
            }
        }
        None => sign_in(&mut oauth, url_slot.clone())?,
    };
    open(endpoint, &tokens.access_token).map_err(|e| e.to_string())
}

/// Dynamic registration + the browser, then the tokens are saved by `login_blocking`. The client
/// id is kept beside them: a refresh must come from the client the token was issued to.
fn sign_in(
    oauth: &mut SunoOAuth,
    url_slot: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
) -> Result<Tokens, String> {
    let redirect = SunoOAuth::redirect_uri(SIGN_IN_PORT);
    let id = oauth.register_client(&redirect)?;
    let id_file = client_id_file(&oauth.token_file);
    if let Some(dir) = id_file.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    std::fs::write(&id_file, &id).map_err(|e| format!("writing {}: {e}", id_file.display()))?;
    oauth.login_blocking_slot(SIGN_IN_PORT, url_slot.as_ref())
}

fn client_id_file(token_file: &Path) -> PathBuf {
    token_file.with_file_name("suno_client_id")
}

/// `initialize` + `tools/list`. A 401 stays typed, so a caller can tell "sign in again" apart.
fn open(endpoint: &str, token: &str) -> Result<SunoSession, McpError> {
    let mut client = McpClient::new(endpoint, token);
    client.initialize()?;
    let tools = client.tools_list()?;
    Ok(SunoSession { client, tools })
}

// ---------------------------------------------------------------- picking the generation tool

/// Name words that mean "make a song", with the weight each carries.
const GEN_WORDS: [(&str, i32); 4] = [("song", 4), ("generate", 3), ("create", 3), ("music", 2)];
/// Verbs that fetch or manage existing material. A tool carrying one is not a generator.
const NOT_GEN: [&str; 7] = [
    "search", "list", "library", "status", "delete", "credits", "history",
];
/// Argument names a generator is expected to take; their presence points at generation.
const GEN_PROPS: [&str; 4] = ["prompt", "style", "lyrics", "title"];

fn score(tool: &McpTool) -> (i32, String) {
    let name = tool.name.to_lowercase();
    let desc = tool.description.to_lowercase();
    let mut total = 0;
    let mut why = String::new();
    for (word, weight) in GEN_WORDS {
        if name.contains(word) {
            total += weight;
            why += &format!(" name:{word}+{weight}");
        }
        if desc.contains(word) {
            total += 1;
            why += &format!(" description:{word}+1");
        }
    }
    for prop in props(&tool.input_schema) {
        if GEN_PROPS.contains(&prop.1.as_str()) {
            total += 1;
            why += &format!(" schema:{}+1", prop.1);
        }
    }
    for word in NOT_GEN {
        if name.contains(word) {
            total -= 6;
            why += &format!(" name:{word}-6");
        }
    }
    (total, why.trim().to_string())
}

/// The best-scoring tool, or an error naming everything the server did expose.
fn pick_generation(tools: &[McpTool]) -> Result<&McpTool, String> {
    let best = tools
        .iter()
        .map(|t| (t, score(t)))
        .filter(|(_, (s, _))| *s > 0)
        .max_by_key(|(_, (s, _))| *s);
    match best {
        Some((tool, (s, why))) => {
            println!("suno: generation tool `{}` (score {s}): {why}", tool.name);
            Ok(tool)
        }
        None => {
            let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
            Err(format!(
                "suno: no song-generation tool among the {} this server exposes: {}",
                tools.len(),
                names.join(", ")
            ))
        }
    }
}

/// The schema's argument names as `(as written, lowercased)`.
fn props(schema: &Value) -> Vec<(String, String)> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| m.keys().map(|k| (k.clone(), k.to_lowercase())).collect())
        .unwrap_or_default()
}

/// The declared name of an argument, matching case-insensitively (`Prompt` is still `Prompt`).
fn prop_named(schema: &Value, want: &str) -> Option<String> {
    props(schema)
        .into_iter()
        .find(|(_, lower)| lower == want)
        .map(|(written, _)| written)
}

/// The first string argument the schema requires, else the first string one it declares.
fn first_string_prop(schema: &Value) -> Option<String> {
    let declared = schema.get("properties").and_then(Value::as_object);
    let is_string = |name: &str| {
        declared
            .and_then(|d| d.get(name))
            .and_then(|v| v.get("type"))
            .and_then(Value::as_str)
            == Some("string")
    };
    schema
        .get("required")
        .and_then(Value::as_array)
        .and_then(|req| {
            req.iter()
                .filter_map(Value::as_str)
                .find(|n| is_string(n))
                .map(str::to_string)
        })
        .or_else(|| {
            declared?
                .iter()
                .find(|(_, v)| v.get("type").and_then(Value::as_str) == Some("string"))
                .map(|(k, _)| k.clone())
        })
}

/// Fills the tool's own schema: the prompt goes in `prompt`/`description` when declared, style
/// and lyrics only when the schema has somewhere to put them.
fn build_args(tool: &McpTool, prompt: &str, style: Option<&str>, lyrics: Option<&str>) -> Value {
    let schema = &tool.input_schema;
    let text_key = prop_named(schema, "prompt")
        .or_else(|| prop_named(schema, "description"))
        .or_else(|| first_string_prop(schema))
        .unwrap_or_else(|| "prompt".to_string());
    let mut args = Map::new();
    args.insert(text_key, json!(prompt));
    for (key, value) in [("style", style), ("lyrics", lyrics)] {
        if let Some(value) = value
            && let Some(name) = prop_named(schema, key)
        {
            args.insert(name, json!(value));
        }
    }
    Value::Object(args)
}

/// The status tool takes the id: in its `id` argument when it declares one.
fn status_args(tool: &McpTool, id: &str) -> Value {
    let key = prop_named(&tool.input_schema, "id")
        .or_else(|| first_string_prop(&tool.input_schema))
        .unwrap_or_else(|| "id".to_string());
    let mut args = Map::new();
    args.insert(key, json!(id));
    Value::Object(args)
}

// ---------------------------------------------------------------- reading the answer

/// MCP tools commonly answer with JSON inside `content[].text`; expanding those strings lets one
/// walk find urls, ids and titles whichever framing the server used.
fn expand(v: &Value) -> Value {
    match v {
        Value::String(s) => match serde_json::from_str::<Value>(s.trim()) {
            Ok(inner) if inner.is_object() || inner.is_array() => expand(&inner),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(expand).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), expand(v))).collect()),
        other => other.clone(),
    }
}

fn audio_urls(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(v, &mut |s| {
        if AUDIO_EXT.contains(&url_ext(s).as_str()) && !out.contains(&s.to_string()) {
            out.push(s.to_string());
        }
    });
    out
}

/// The lowercased extension of a url, ignoring query and fragment.
fn url_ext(url: &str) -> String {
    let base = url.split(['?', '#']).next().unwrap_or(url);
    Path::new(base)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default()
}

/// The first string under one of `keys`, looking through nested objects and arrays.
fn find_string(v: &Value, keys: &[&str]) -> Option<String> {
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                if keys.iter().any(|want| k.eq_ignore_ascii_case(want))
                    && let Some(s) = val.as_str()
                    && !s.trim().is_empty()
                {
                    return Some(s.trim().to_string());
                }
            }
            o.values().find_map(|val| find_string(val, keys))
        }
        Value::Array(a) => a.iter().find_map(|val| find_string(val, keys)),
        _ => None,
    }
}

fn has_stems(v: &Value) -> bool {
    match v {
        Value::Object(o) => o
            .iter()
            .any(|(k, val)| k.to_lowercase().contains("stem") || has_stems(val)),
        Value::Array(a) => a.iter().any(has_stems),
        _ => false,
    }
}

fn walk(v: &Value, f: &mut impl FnMut(&str)) {
    match v {
        Value::Object(o) => o.values().for_each(|val| walk(val, f)),
        Value::Array(a) => a.iter().for_each(|val| walk(val, f)),
        Value::String(s) => f(s),
        _ => {}
    }
}

// ---------------------------------------------------------------- writing the files

/// `~/.../sono/neon-drive`, or `…-2` when that song is already there — never over a take.
fn unique_dir(base: &Path, title: &str) -> Result<PathBuf, String> {
    let slug = slugify(title);
    let mut dir = base.join(&slug);
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = base.join(format!("{slug}-{n}"));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    Ok(dir)
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "suno-song".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Numbered so a stem import gets a stable channel order whatever the server calls its files.
/// shortcut: a `.zip` bundle is saved whole (no unzip crate), the importer must unpack it.
fn download(url: &str, dir: &Path, index: usize) -> Result<PathBuf, String> {
    let base = url.split(['?', '#']).next().unwrap_or(url);
    let name = base.rsplit('/').next().unwrap_or("");
    let ext = url_ext(url).trim_start_matches('.').to_string();
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");
    let path = dir.join(format!("{index:02}-{}.{ext}", slugify(stem)));

    let agent = ureq::AgentBuilder::new().timeout(DOWNLOAD_TIMEOUT).build();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    let mut reader = response.into_reader();
    let mut file =
        std::fs::File::create(&path).map_err(|e| format!("creating {}: {e}", path.display()))?;
    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mcp_client::mock::{Mock, Reply};
    use serde_json::json;

    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"suno","version":"0.1"}}}"#;

    fn tool(name: &str, description: &str, props: Value) -> Value {
        json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": props,
                "required": props.as_object().map(|p| p.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() }
        })
    }

    fn tools_reply(id: u64, tools: Vec<Value>) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tools } }).to_string()
    }

    /// A token file that is not expired, so `connect_at` goes straight to the MCP handshake.
    fn fresh_tokens(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sonix-suno-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let file = dir.join("suno_tokens.json");
        Tokens {
            access_token: "tok-mock-1234".to_string(),
            refresh_token: "refresh-mock".to_string(),
            expires_at: u64::MAX - 1,
        }
        .save(&file)
        .expect("tokens");
        file
    }

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sonix-suno-dl-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        dir
    }

    /// (a) Three plausible tools plus noise: the chosen one and its score/why are printed.
    #[test]
    fn the_generation_tool_is_picked_from_the_servers_own_list() {
        let tools = vec![
            tool(
                "library_search",
                "Search your Suno library",
                json!({ "query": { "type": "string" } }),
            ),
            tool(
                "generate_song",
                "Create a new song from a prompt",
                json!({
                    "prompt": { "type": "string" },
                    "style": { "type": "string" },
                    "lyrics": { "type": "string" },
                    "title": { "type": "string" },
                }),
            ),
            tool(
                "create_music",
                "Create a song",
                json!({ "prompt": { "type": "string" }, "style": { "type": "string" } }),
            ),
            tool(
                "song_status",
                "Check generation status for a song",
                json!({ "id": { "type": "string" } }),
            ),
            tool("credits_balance", "Current credit balance", json!({})),
        ];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools.clone())),
        ]);
        let session = connect_at(&mock.url, &fresh_tokens("pick"), None).expect("connect");

        println!("endpoint: {}", session.endpoint());
        println!("{} tools exposed", session.tools().len());
        for t in session.tools() {
            let (s, why) = score(t);
            println!("  {:<18} score {s:>3}  {why}", t.name);
        }
        let chosen = pick_generation(session.tools()).expect("a generator");
        println!("chosen: {} (score {})", chosen.name, score(chosen).0);
        assert_eq!(chosen.name, "generate_song");
        assert!(score(chosen).0 > score(&session.tools()[2]).0);
    }

    /// (b) Two audio URLs in a `content[].text` payload: both land on disk, stems is true.
    #[test]
    fn both_audio_urls_are_downloaded_and_reported_as_stems() {
        // The download server comes first, so its address can go into the MCP answer.
        let audio = Mock::new(vec![
            Reply::json("RIFF0000WAVEfmt "),
            Reply::json("RIFF0000"),
        ]);
        let text = json!({
            "title": "Neon Drive",
            "stems": [
                format!("{}/neon-drums.wav", audio.url),
                format!("{}/neon-bass.wav", audio.url),
            ],
        })
        .to_string();
        let tools = vec![tool(
            "generate_song",
            "Create a new song from a prompt",
            json!({ "prompt": { "type": "string" }, "style": { "type": "string" } }),
        )];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            // JSON in `text`, as the real tools answer: the url walk must look inside it.
            Reply::json(
                json!({ "jsonrpc": "2.0", "id": 3, "result": { "content": [
                    { "type": "text", "text": text } ] } })
                .to_string(),
            ),
        ]);
        let dir = tmp_dir("stems");
        let mut session = connect_at(&mock.url, &fresh_tokens("stems"), None).expect("connect");
        let song = session
            .generate_into("dark synthwave", Some("80s"), None, &dir)
            .expect("generate");

        println!("title: {}", song.title);
        println!("stems: {}", song.stems);
        for path in &song.files {
            let bytes = std::fs::metadata(path).expect("file").len();
            println!("file: {} ({bytes} bytes)", path.display());
            assert!(bytes > 0, "{} is empty", path.display());
        }
        assert_eq!(song.files.len(), 2);
        assert!(song.stems);
        assert_eq!(song.title, "Neon Drive");
        assert!(
            song.files[0].parent().unwrap().ends_with("neon-drive"),
            "folder is not the slugified title: {:?}",
            song.files[0]
        );
        // initialize, notification, tools/list, tools/call on one server; two downloads on the other.
        println!(
            "mcp requests: {}, download requests: {}",
            mock.count(),
            audio.count()
        );
        assert_eq!(mock.count(), 4);
        assert_eq!(audio.count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) Nothing recognisable: the error names every tool the server exposes.
    #[test]
    fn an_unrecognisable_tool_list_is_reported_with_its_names() {
        let tools = vec![
            tool("v1_x9", "beta", json!({ "q": { "type": "string" } })),
            tool("zap", "beta", json!({})),
            tool("u_17", "beta", json!({})),
        ];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("unknown"), None).expect("connect");
        let err = session
            .generate_into("anything", None, None, &tmp_dir("unknown"))
            .expect_err("no generator exists");
        println!("error: {err}");
        for name in ["v1_x9", "zap", "u_17"] {
            assert!(err.contains(name), "error does not list {name}: {err}");
        }
        // The failure happened before any tool was called.
        println!("requests: {}", mock.count());
        assert_eq!(mock.count(), 3);
    }

    /// (d) A 401 is the typed error, and the client does not call again.
    #[test]
    fn a_401_is_expired_and_is_not_retried() {
        let mock = Mock::new(vec![Reply::status(401)]);
        // `open` keeps the typed error, which is what tells "sign in again" from "network".
        let err = match open(&mock.url, "tok-stale-000000") {
            Ok(_) => panic!("401 must not succeed"),
            Err(e) => e,
        };
        println!("typed error: {err:?}");
        println!("displayed: {err}");
        println!("requests the mock saw: {}", mock.count());
        assert_eq!(err, McpError::Expired);
        assert_eq!(mock.count(), 1, "a 401 was retried");
    }

    /// A queued generation is polled through the status tool until audio appears.
    #[test]
    fn a_queued_song_is_polled_until_the_audio_appears() {
        let audio = Mock::new(vec![Reply::json("WAVE-one"), Reply::json("WAVE-two")]);
        let queued = json!({ "song": { "id": "song-42", "status": "queued" } }).to_string();
        let done = json!({ "title": "Slow Burn",
            "stems": [format!("{}/a.wav", audio.url), format!("{}/b.wav", audio.url)] })
        .to_string();
        let tools = vec![
            tool(
                "generate_song",
                "Create a new song from a prompt",
                json!({ "prompt": { "type": "string" } }),
            ),
            tool(
                "song_status",
                "Check generation status",
                json!({ "id": { "type": "string" } }),
            ),
        ];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(
                json!({ "jsonrpc": "2.0", "id": 3,
                        "result": { "content": [ { "type": "text", "text": queued } ] } })
                .to_string(),
            ),
            Reply::json(
                json!({ "jsonrpc": "2.0", "id": 4,
                        "result": { "content": [ { "type": "text", "text": done } ] } })
                .to_string(),
            ),
        ]);
        let dir = tmp_dir("poll");
        let mut session = connect_at(&mock.url, &fresh_tokens("poll"), None).expect("connect");
        let song = session
            .generate_into("slow burn", None, None, &dir)
            .expect("generate");

        println!("title: {}, files: {:?}", song.title, song.files);
        assert_eq!(song.title, "Slow Burn");
        assert_eq!(song.files.len(), 2);
        assert!(song.stems);
        let reqs = mock.requests();
        println!("requests: {}", reqs.len());
        assert_eq!(
            reqs.len(),
            5,
            "expected one poll between the call and the download"
        );
        assert!(
            reqs[4].contains("song_status") && reqs[4].contains("\"id\":\"song-42\""),
            "the poll did not carry the song id: {}",
            reqs[4]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (e) The arguments come from the schema, not from a guess about the tool.
    #[test]
    fn arguments_are_built_from_the_tools_schema() {
        let t = McpTool {
            name: "generate_song".to_string(),
            description: String::new(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "style": { "type": "string" }
                },
                "required": ["prompt"]
            }),
        };
        let args = build_args(&t, "dark synthwave", Some("80s"), None);
        println!("args: {args}");
        assert_eq!(args, json!({ "prompt": "dark synthwave", "style": "80s" }));

        // A schema that only names `description` still gets the prompt.
        let d = McpTool {
            name: "compose".to_string(),
            description: String::new(),
            input_schema: json!({ "type": "object", "properties": { "description": { "type": "string" } } }),
        };
        let args = build_args(&d, "dark synthwave", Some("80s"), None);
        println!("description-schema args: {args}");
        assert_eq!(args, json!({ "description": "dark synthwave" }));
    }
}
