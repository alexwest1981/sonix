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
/// How many listing pages [`SunoSession::library`] follows at most; a server that keeps handing
/// out a cursor must still terminate.
const LIB_PAGES: usize = 10;

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

/// One song as a library listing describes it. The server's payload shape is unknown, so every
/// field but `raw` may come out empty; `raw` keeps the item untouched for the UI to inspect.
/// Felet som betyder "fråga användaren om krediten först", inte "det gick inte".
pub const NEEDS_CREDIT: &str =
    "suno: the download costs one credit — confirm with the user first";

/// Varje clip-id i svaret: `clip_ids` när det finns, annars det enskilda `id`:t.
fn clip_ids(payload: &Value) -> Vec<String> {
    let listed: Vec<String> = payload
        .get("clip_ids")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !listed.is_empty() {
        return listed;
    }
    find_string(payload, &["song_id", "job_id", "task_id", "id"])
        .map(|id| vec![id])
        .into_iter()
        .flatten()
        .collect()
}

/// Vänteverktygets argument: en lista när schemat har `ids`, annars ett enskilt id.
fn wait_args(tool: &McpTool, ids: &[String]) -> Value {
    if props(&tool.input_schema).iter().any(|p| p.1 == "ids") {
        return serde_json::json!({ "ids": ids });
    }
    let id = ids.first().cloned().unwrap_or_default();
    id_args(tool, &id)
}

/// Sant när servern säger att nedladdningen kräver en kredit och en bekräftelse.
fn confirmation_needed(reply: &Value) -> bool {
    let text = expand(reply).to_string().to_lowercase();
    text.contains("requires_credit") || text.contains("confirm")
}

#[derive(Debug, Clone)]
pub struct SunoSong {
    pub id: String,
    pub title: String,
    pub audio_urls: Vec<String>,
    pub stem_urls: Vec<String>,
    pub duration_secs: Option<f64>,
    pub created: Option<String>,
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
    /// Connects **without** ever opening a browser: the saved tokens are used, refreshed once if
    /// they have expired, and anything else is an error. The CLI needs this — a probe should not
    /// launch a sign-in window.
    pub fn connect_cached() -> Result<SunoSession, String> {
        let oauth = SunoOAuth::new();
        connect_at(&oauth.mcp, &oauth.token_file, None, false)
    }

    pub fn connect_with_slot(
        url_slot: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
    ) -> Result<SunoSession, String> {
        let oauth = SunoOAuth::new();
        connect_at(&oauth.mcp, &oauth.token_file, url_slot, true)
    }


    /// **Ett 401 mitt i en session ska inte döda den.** Token lever en timme; en skapning kan ta
    /// längre. Vid `Expired` förnyas token ur den sparade refresh-token och anropet görs om en gång.
    fn call_tool_refreshing(&mut self, name: &str, args: Value) -> Result<Value, McpError> {
        match self.client.call_tool(name, args.clone()) {
            Err(McpError::Expired) => {
                let oauth = SunoOAuth::new();
                let tokens = Tokens::load(&oauth.token_file).ok_or(McpError::Expired)?;
                let fresh = oauth.refresh(&tokens).map_err(|_| McpError::Expired)?;
                let _ = fresh.save(&oauth.token_file);
                self.client.token = fresh.access_token;
                self.client.call_tool(name, args)
            }
            other => other,
        }
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn endpoint(&self) -> &str {
        &self.client.endpoint
    }

    /// The tool [`Self::generate`] would call, so the UI can read its options.
    pub fn generation_tool(&self) -> Option<McpTool> {
        pick_generation(&self.tools).ok().cloned()
    }

    /// That tool's own `input_schema` — the form the panel draws. Never `Null`: a server that
    /// declares no schema answers with an empty `properties` object, which is the same form.
    /// Schemat för ett verktyg vid namn — används för att mäta vad cover/mashup kräver.
    pub fn tool_schema(&self, name: &str) -> Option<Value> {
        self.tools
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.input_schema.clone())
    }

    pub fn generation_schema(&self) -> Option<Value> {
        let schema = self.generation_tool()?.input_schema;
        Some(if schema.is_object() {
            schema
        } else {
            json!({ "type": "object", "properties": {} })
        })
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
        self.generate_with_into(args, base)
    }

    /// Sends exactly `args` to the generation tool and saves what comes back, the same way
    /// [`Self::generate`] does. Which keys exist is the tool's schema's business, not ours.
    pub fn generate_with(&mut self, args: Value) -> Result<GeneratedSong, String> {
        self.generate_with_into(args, &crate::paths::paths().suno_dir())
    }

    /// Som [`Self::generate_with`], men med verktyget namngivet: cover och mashup är egna verktyg
    /// (`audio-covers-create`, `audio-mashups-create`), inte lägen i `audio-create`.
    pub fn generate_named(&mut self, tool_name: &str, args: Value) -> Result<GeneratedSong, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == tool_name)
            .cloned()
            .ok_or_else(|| format!("suno: the server has no tool `{tool_name}`"))?;
        // **Bara verktygets egna fält.** Alla verktyg har `additionalProperties: false`, och cover
        // och mashup saknar t.ex. `description` — att skicka panelens hela formulär gav 400.
        let declared: Vec<String> = props(&tool.input_schema)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        let args = match args {
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .filter(|(key, _)| declared.iter().any(|d| d == key))
                    .collect(),
            ),
            other => other,
        };
        self.run_generation(tool, args, &crate::paths::paths().suno_dir())
    }

    fn generate_with_into(&mut self, args: Value, base: &Path) -> Result<GeneratedSong, String> {
        let tool = pick_generation(&self.tools)?.clone();
        self.run_generation(tool, args, base)
    }

    fn run_generation(
        &mut self,
        tool: McpTool,
        args: Value,
        base: &Path,
    ) -> Result<GeneratedSong, String> {
        println!("suno: tools/call {} {args}", tool.name);
        // Felmeddelandet bär kroppen vi skickade: serverns svar är ofta bara "invalid request
        // body", och då är det enda sättet att ringa in fältet att se vad vi faktiskt skickade.
        let sent = args.to_string();
        // The prompt (or whatever the caller put in its place) is the title's last resort.
        let fallback = find_string(&args, &["prompt", "description", "style"]);
        let mut result = self
            .call_tool_refreshing(&tool.name, args)
            .map_err(|e| e.to_string())?;

        // An asynchronous generator answers with an id and a status; ask until audio appears.
        let status = self
            .tools
            .iter()
            .find(|t| {
                let n = t.name.to_lowercase();
                // Den riktiga beta-servern kallar vänteverktyget `audio-wait`, inte "status".
                n.contains("status") || n.contains("poll") || n.contains("wait")
            })
            .cloned();
        // **En beställning kan bli flera låtar.** Mätt: `audio-create` svarar
        // `{"id":…, "clip_ids":[…], "status":"submitted"}` och säger själv "call audio-wait once
        // with all of them in ids".
        let first = expand(&result);
        let ids = clip_ids(&first);
        if let (Some(status), true) = (status, !ids.is_empty()) {
            for attempt in 1..=POLL_TRIES {
                if !audio_urls(&expand(&result)).is_empty() {
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
                let args = wait_args(&status, &ids);
                println!(
                    "suno: poll {attempt}/{POLL_TRIES} via {} {args}",
                    status.name
                );
                result = self
                    .call_tool_refreshing(&status.name, args)
                    .map_err(|e| e.to_string())?;
            }
        }

        let payload = expand(&result);
        let urls = audio_urls(&payload);
        if urls.is_empty() {
            // **Klart är inte samma sak som hämtat.** `audio-wait` svarar med färdiga clips men
            // utan ljud-URL: själva filen kostar en nedladdningskredit via `audio-download`.
            let done = payload.to_string().to_lowercase().contains("\"complete\"");
            if done {
                return Err(
                    "suno: the song is ready in your library — fetching the audio costs one download credit (Library → Bekräfta)".to_string(),
                );
            }
            let shown: String = payload.to_string().chars().take(300).collect();
            return Err(format!(
                "suno: `{}` ({shown}) — skickade: {sent}",
                tool.name
            ));
        }

        let title = find_string(&payload, &["title", "song_title"])
            .or(fallback)
            .map(|s| s.chars().take(60).collect::<String>())
            .unwrap_or_default();
        save_song(&urls, &title, has_stems(&payload), result, base)
    }

    /// Every song the beta server will list, `limit` at most (0 = whatever it returns), optionally
    /// filtered by `query`. The listing tool is discovered like the generator is; pagination is
    /// followed while the reply carries a cursor/next token the tool's schema has a slot for.
    pub fn library(&mut self, query: Option<&str>, limit: usize) -> Result<Vec<SunoSong>, String> {
        // **Rätt verktyg för rätt fråga.** Mätt mot den riktiga servern: `library-search` kräver
        // en query på 1–200 tecken och svarar `invalid_request` utan, medan `library-list` listar
        // allt. Utan den här uppdelningen blev ett tomt sökfält ett fel i stället för biblioteket.
        let tool = match query {
            Some(_) => pick_library_search(&self.tools),
            None => pick_library_all(&self.tools),
        }?
        .clone();
        let mut args = library_args(&tool, query, limit);
        let mut songs: Vec<SunoSong> = Vec::new();
        for page in 1..=LIB_PAGES {
            println!("suno: library page {page} via {} {args}", tool.name);
            let result = self
                .call_tool_refreshing(&tool.name, args.clone())
                .map_err(|e| e.to_string())?;
            let payload = expand(&result);
            let batch = songs_from(&payload);
            println!("suno: page {page} returned {} song(s)", batch.len());
            if batch.is_empty() {
                let shown: String = payload.to_string().chars().take(300).collect();
                println!("suno: unparsed page: {shown}");
                break;
            }
            songs.extend(batch);
            if limit > 0 && songs.len() >= limit {
                break;
            }
            let Some(token) = next_token(&payload) else {
                break;
            };
            let Some(key) = cursor_prop(&tool) else {
                println!(
                    "suno: `{}` declares no pagination argument; stopping after page {page}",
                    tool.name
                );
                break;
            };
            args[key] = token;
        }
        if limit > 0 {
            songs.truncate(limit);
        }
        println!(
            "suno: {} song(s) listed for query {:?}",
            songs.len(),
            query.unwrap_or("<none>")
        );
        Ok(songs)
    }

    /// Downloads a listed song: its stems when it has any, else its audio. Same per-song folder
    /// and the same `stems` flag as [`Self::generate`].
    /// Samma hämtning, men med krediten bekräftad av användaren. Anropas bara från ett klick som
    /// uttryckligen säger att det kostar en nedladdningskredit.
    pub fn download_song_confirmed(&mut self, id: &str) -> Result<GeneratedSong, String> {
        let reply = self.call(
            "audio-download",
            serde_json::json!({ "id": id, "confirm": true }),
        )?;
        let payload = expand(&reply);
        let urls = audio_urls(&payload);
        if urls.is_empty() {
            let shown: String = payload.to_string().chars().take(300).collect();
            return Err(format!("suno: the download reply carried no audio url: {shown}"));
        }
        let song = SunoSong {
            id: id.to_string(),
            title: payload
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or(id)
                .to_string(),
            audio_urls: urls,
            stem_urls: Vec::new(),
            duration_secs: None,
            created: None,
            raw: payload,
        };
        self.download_song(&song)
    }

    pub fn download_song(&mut self, song: &SunoSong) -> Result<GeneratedSong, String> {
        self.download_song_into(song, &crate::paths::paths().suno_dir())
    }

    fn download_song_into(
        &mut self,
        song: &SunoSong,
        base: &Path,
    ) -> Result<GeneratedSong, String> {
        // A row from a listing carries no urls on the live server: the full record comes from the
        // retrieve tool, per id.
        let listed_title = song.title.clone();
        let mut song = if song.audio_urls.is_empty() && song.stem_urls.is_empty() {
            self.retrieve_song(&song.id)?
        } else {
            song.clone()
        };
        if song.title.trim().is_empty() {
            song.title = listed_title;
        }
        let song = &song;
        // **En nedladdning kostar en kredit.** Mätt mot den riktiga servern: `audio-download`
        // svarar `{"status":"requires_credit", …}` tills man skickar `confirm: true`. Vi svarar med
        // ett igenkännbart fel så UI:t kan fråga Alex först — aldrig spendera i tysthet.
        if let Ok(reply) = self.call("audio-download", serde_json::json!({ "id": song.id }))
            && confirmation_needed(&reply)
        {
            return Err(NEEDS_CREDIT.to_string());
        }
        let urls = if song.stem_urls.is_empty() {
            &song.audio_urls
        } else {
            &song.stem_urls
        };
        if urls.is_empty() {
            return Err(format!(
                "suno: song `{}` ({}) lists no audio or stem url",
                song.title, song.id
            ));
        }
        let title = if song.title.trim().is_empty() {
            song.id.clone()
        } else {
            song.title.clone()
        };
        save_song(
            urls,
            &title,
            !song.stem_urls.is_empty(),
            song.raw.clone(),
            base,
        )
    }

    /// The voices/personas the account owns, for the panel's picker. A server without such a
    /// tool is not an error — it is an empty list and [`Self::voices_hint`] saying why.
    pub fn voices(&mut self) -> Result<Vec<SunoVoice>, String> {
        let Some(tool) = voice_tool(&self.tools).cloned() else {
            println!("{}", self.voices_hint().unwrap_or_default());
            return Ok(Vec::new());
        };
        println!("suno: voices via {} {}", tool.name, json!({}));
        let result = self
            .call_tool_refreshing(&tool.name, json!({}))
            .map_err(|e| e.to_string())?;
        let payload = expand(&result);
        let voices = voices_from(&payload);
        if voices.is_empty() {
            let shown: String = payload.to_string().chars().take(300).collect();
            println!("suno: unparsed voice reply: {shown}");
        }
        println!("suno: {} voice(s) via {}", voices.len(), tool.name);
        Ok(voices)
    }

    /// Why [`Self::voices`] came back empty, when the server exposes no voice tool at all.
    pub fn voices_hint(&self) -> Option<String> {
        if voice_tool(&self.tools).is_some() {
            return None;
        }
        let names: Vec<&str> = self.tools.iter().map(|t| t.name.as_str()).collect();
        Some(format!(
            "suno: this server exposes no voice/persona tool (it has: {}); \
             the picker needs one whose name or description mentions voice, persona, singer or \
             artist, e.g. a `list_voices` tool",
            names.join(", ")
        ))
    }

    /// The generation schema's argument that takes a voice/persona id, so the UI knows where the
    /// chosen voice goes.
    pub fn voice_param(&self) -> Option<String> {
        let schema = self.generation_schema()?;
        voice_prop(&schema)
    }

    /// Every tool the server exposed, sorted, for a UI that only wants names.
    pub fn tool_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.iter().map(|t| t.name.clone()).collect();
        names.sort();
        names
    }

    /// Sends exactly `name` and `args` to the server and returns the `tools/call` result as it
    /// arrived. The escape hatch for tools that have no wrapper here: no expansion, no parsing,
    /// no argument building — including the mutating ones.
    pub fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        println!("suno: tools/call {tool} {args}");
        self.call_tool_refreshing(tool, args)
            .map_err(|e| e.to_string())
    }

    /// One song by id, with the audio and stem urls a listing does not carry.
    pub fn retrieve_song(&mut self, id: &str) -> Result<SunoSong, String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name.to_lowercase().contains("retrieve"))
            .cloned()
            .ok_or_else(|| format!("suno: no tool to retrieve one song — `{id}` needs one whose name contains `retrieve`"))?;
        let args = id_args(&tool, id);
        println!("suno: retrieve {id} via {} {args}", tool.name);
        let result = self
            .call_tool_refreshing(&tool.name, args)
            .map_err(|e| e.to_string())?;
        let payload = expand(&result);
        let mut song = song_from(&payload).ok_or_else(|| {
            let shown: String = payload.to_string().chars().take(300).collect();
            format!("suno: `{}` for `{id}` carried no song: {shown}", tool.name)
        })?;
        if song.id.trim().is_empty() {
            song.id = id.to_string();
        }
        println!(
            "suno: retrieved \"{}\" — {} audio url(s), {} stem url(s)",
            song.title,
            song.audio_urls.len(),
            song.stem_urls.len()
        );
        Ok(song)
    }

    /// The account's playlists. `playlists-list` takes no arguments here.
    pub fn playlists(&mut self) -> Result<Vec<SunoPlaylist>, String> {
        let tool = self.tool_with(&["playlist", "list"])?.to_string();
        let payload = self.payload(&tool)?;
        let playlists: Vec<SunoPlaylist> = rows(&payload, &["playlists", "items", "results"])
            .iter()
            .filter_map(parse_playlist)
            .collect();
        println!("suno: {} playlist(s) via {tool}", playlists.len());
        Ok(playlists)
    }

    /// The account's workspaces.
    pub fn workspaces(&mut self) -> Result<Vec<SunoWorkspace>, String> {
        let tool = self.tool_with(&["workspace", "list"])?.to_string();
        let payload = self.payload(&tool)?;
        let workspaces: Vec<SunoWorkspace> = rows(&payload, &["workspaces", "items", "results"])
            .iter()
            .filter_map(parse_workspace)
            .collect();
        println!("suno: {} workspace(s) via {tool}", workspaces.len());
        Ok(workspaces)
    }

    /// Credits and plan, merged from `account-usage` and/or `account-me`.
    ///
    /// Only read-only account tools are asked: `account-upgrade` also starts with `account` and
    /// must never be called by a status read. `raw` maps each tool's name to its payload.
    pub fn account(&mut self) -> Result<SunoAccount, String> {
        let tools: Vec<String> = self
            .tools
            .iter()
            .filter(|t| {
                let n = t.name.to_lowercase();
                (n.contains("account") && ["usage", "me", "info", "status"].iter().any(|w| n.contains(w)))
                    || (n.contains("credit") && n.contains("balance"))
            })
            .map(|t| t.name.clone())
            .collect();
        if tools.is_empty() {
            return Err("suno: this server exposes no account usage, account info or credit balance tool".to_string());
        }
        let mut raw = Map::new();
        let mut credits = None;
        let mut plan = None;
        for name in &tools {
            let payload = self.payload(name)?;
            credits = credits.or_else(|| {
                number_at(
                    &payload,
                    &["credits_left", "credits", "credits_remaining", "remaining_credits", "credits_balance", "balance"],
                )
            });
            plan = plan.or_else(|| find_string(&payload, &["plan", "plan_name", "subscription", "tier"]));
            raw.insert(name.clone(), payload);
        }
        println!("suno: account — credits {credits:?}, plan {plan:?}");
        Ok(SunoAccount {
            credits,
            plan,
            raw: Value::Object(raw),
        })
    }

    /// The one exposed tool whose lowercase name contains every `word`.
    fn tool_with(&self, words: &[&str]) -> Result<&str, String> {
        self.tools
            .iter()
            .find(|t| {
                let name = t.name.to_lowercase();
                words.iter().all(|w| name.contains(w))
            })
            .map(|t| t.name.as_str())
            .ok_or_else(|| {
                format!(
                    "suno: this server exposes no `{}` tool; it has: {}",
                    words.join("-"),
                    self.tool_names().join(", ")
                )
            })
    }

    /// Calls `tool` with no arguments and returns its reply, expanded.
    fn payload(&mut self, tool: &str) -> Result<Value, String> {
        println!("suno: {tool} {}", json!({}));
        let result = self
            .call_tool_refreshing(tool, json!({}))
            .map_err(|e| e.to_string())?;
        Ok(expand(&result))
    }
}

/// One playlist. Same tolerance as [`SunoSong`]: `raw` is the truth, the fields are what a list
/// row can draw.
#[derive(Debug, Clone)]
pub struct SunoPlaylist {
    pub id: String,
    pub name: String,
    pub clip_count: Option<u32>,
    pub raw: Value,
}

/// One workspace.
#[derive(Debug, Clone)]
pub struct SunoWorkspace {
    pub id: String,
    pub name: String,
    pub raw: Value,
}

/// Credits and plan, whichever account reply carried them.
#[derive(Debug, Clone)]
pub struct SunoAccount {
    pub credits: Option<f64>,
    pub plan: Option<String>,
    /// Tool name → that tool's reply.
    pub raw: Value,
}

/// One voice or persona. Same shape rule as [`SunoSong`]: the beta's field names are guesses,
/// `raw` is the truth.
#[derive(Debug, Clone)]
pub struct SunoVoice {
    pub id: String,
    pub name: String,
    pub raw: Value,
}

/// The body of [`SunoSession::connect`], with the endpoint and token file supplied so the mock
/// can drive it.
fn connect_at(
    endpoint: &str,
    token_file: &Path,
    url_slot: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>,
    allow_login: bool,
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
                    if allow_login {
                        sign_in(&mut oauth, url_slot.clone())?
                    } else {
                        return Err(format!("the saved tokens could not be refreshed: {e}"));
                    }
                }
                None => {
                    if allow_login {
                        sign_in(&mut oauth, url_slot.clone())?
                    } else {
                        return Err("no saved client id — sign in from the panel once".to_string());
                    }
                }
            }
        }
        None => {
            if allow_login {
                sign_in(&mut oauth, url_slot.clone())?
            } else {
                return Err("no saved Suno tokens — sign in from the panel once".to_string());
            }
        }
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
const NOT_GEN: [&str; 20] = [
    "search", "list", "library", "status", "delete", "credits", "history",
    // Mätt mot den riktiga servern: 32 verktyg, och de här orden sitter på allt som *inte* är
    // en ny låt — covers, mashups, extensions, uppladdningar, redigeringar, hämtningar, likes.
    "cover", "mashup", "extension", "upload", "edit", "like", "download", "retrieve",
    "wait", "analysis", "alignment",
    // `speech-create` är textuppläsning, inte musik — den vann annars på ordet create.
    "speech", "tts",
];
/// Argument names a generator is expected to take; their presence points at generation.
const GEN_PROPS: [&str; 4] = ["prompt", "style", "lyrics", "title"];
/// Rena skaparnamn, mätta på den riktiga servern (`audio-create`, `audio-full-song`). Utan de här
/// vann `audio-mashups-create` med 8 poäng — ett verktyg som gör en mashup, inte en ny låt.
const GEN_EXACT: [(&str, i32); 3] = [("audio-create", 12), ("audio-full-song", 10), ("song-create", 10)];

/// Argument names a library listing is expected to take; their presence points at a listing.
const LIB_PROPS: [&str; 5] = ["query", "limit", "cursor", "search", "q"];
/// Name words that mean "list what already exists", with the weight each carries.
const LIB_WORDS: [(&str, i32); 5] = [
    ("library", 5),
    ("search", 4),
    ("list", 4),
    ("songs", 3),
    ("song", 3),
];
/// Verbs that make a song or manage one job. A tool carrying one is not a lister.
const NOT_LIB: [&str; 5] = ["generate", "create", "status", "delete", "credits"];

fn score(tool: &McpTool) -> (i32, String) {
    let (mut total, mut why) = scoring(tool, &GEN_WORDS, &GEN_PROPS, &NOT_GEN);
    let name = tool.name.to_lowercase();
    for (exact, weight) in GEN_EXACT {
        if name == exact {
            total += weight;
            why += &format!(" name:{exact}+{weight}");
        }
    }
    (total, why)
}

/// Name, description and declared arguments, each word counted with its weight. A tool named
/// after something it is not (`generate`, `search`, …) is pushed well below zero.
fn scoring(
    tool: &McpTool,
    words: &[(&str, i32)],
    wanted: &[&str],
    unwanted: &[&str],
) -> (i32, String) {
    let name = tool.name.to_lowercase();
    let desc = tool.description.to_lowercase();
    let mut total = 0;
    let mut why = String::new();
    for (word, weight) in words {
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
        if wanted.contains(&prop.1.as_str()) {
            total += 1;
            why += &format!(" schema:{}+1", prop.1);
        }
    }
    for word in unwanted {
        if name.contains(word) {
            total -= 6;
            why += &format!(" name:{word}-6");
        }
    }
    (total, why.trim().to_string())
}

/// The best-scoring tool, or an error naming everything the server did expose.
fn pick<'a>(
    tools: &'a [McpTool],
    words: &[(&str, i32)],
    wanted: &[&str],
    unwanted: &[&str],
    what: &str,
) -> Result<&'a McpTool, String> {
    let best = tools
        .iter()
        .map(|t| (t, scoring(t, words, wanted, unwanted)))
        .filter(|(_, (s, _))| *s > 0)
        .max_by_key(|(_, (s, _))| *s);
    match best {
        Some((tool, (s, why))) => {
            println!("suno: {what} tool `{}` (score {s}): {why}", tool.name);
            Ok(tool)
        }
        None => {
            let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
            Err(format!(
                "suno: no {what} tool among the {} this server exposes: {}",
                tools.len(),
                names.join(", ")
            ))
        }
    }
}

fn pick_generation(tools: &[McpTool]) -> Result<&McpTool, String> {
    pick(tools, &GEN_WORDS, &GEN_PROPS, &NOT_GEN, "song-generation")
}


/// `library-list` (eller `library-search` när servern bara har den) för en lista utan fråga.
fn pick_library_all(tools: &[McpTool]) -> Result<&McpTool, String> {
    let listing = tools
        .iter()
        .find(|t| {
            let name = t.name.to_lowercase();
            name.contains("library") && name.contains("list") && !name.contains("liked")
        })
        .or_else(|| {
            tools.iter().find(|t| {
                let name = t.name.to_lowercase();
                name == "library-search" || (name.contains("library") && name.contains("search"))
            })
        });
    match listing {
        Some(tool) => Ok(tool),
        None => pick_library(tools),
    }
}

/// Sökverktyget, när användaren faktiskt skrev något.
fn pick_library_search(tools: &[McpTool]) -> Result<&McpTool, String> {
    tools
        .iter()
        .find(|t| t.name.to_lowercase().contains("library") && t.name.to_lowercase().contains("search"))
        .map(Ok)
        .unwrap_or_else(|| pick_library(tools))
}

fn pick_library(tools: &[McpTool]) -> Result<&McpTool, String> {
    pick(tools, &LIB_WORDS, &LIB_PROPS, &NOT_LIB, "song-library")
}

/// Name words that mean "the account's voices/personas", with the weight each carries.
const VOICE_WORDS: [(&str, i32); 5] =
    [("persona", 5), ("voice", 5), ("singer", 3), ("artist", 3), ("list", 6)];
/// Verbs that make or remove a song. Such a tool is not the voices list, even when its
/// description mentions an artist or a vocal.
const NOT_VOICE: [&str; 4] = ["generate", "create", "delete", "start"];

/// The voice/persona lister, if the server has one. Its absence is not an error here: the caller
/// reports it as an empty list plus [`SunoSession::voices_hint`].
fn voice_tool(tools: &[McpTool]) -> Option<&McpTool> {
    pick(tools, &VOICE_WORDS, &[], &NOT_VOICE, "voice/persona").ok()
}

/// Query and limit go into whatever argument names the listing tool declares.
fn library_args(tool: &McpTool, query: Option<&str>, limit: usize) -> Value {
    let schema = &tool.input_schema;
    let mut args = Map::new();
    if let Some(query) = query
        && let Some(key) = ["query", "search", "q", "text", "prompt"]
            .iter()
            .find_map(|k| prop_named(schema, k))
    {
        args.insert(key, json!(query));
    }
    if let Some(key) = ["limit", "count", "page_size", "max_results", "per_page"]
        .iter()
        .find_map(|k| prop_named(schema, k))
    {
        args.insert(key, json!(limit));
    }
    Value::Object(args)
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

/// The status and retrieve tools take the id: in their `id` argument when they declare one.
fn id_args(tool: &McpTool, id: &str) -> Value {
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

/// The first number under one of `keys`, looking through nested objects and arrays. A numeric
/// string counts as a number.
fn number_at(v: &Value, keys: &[&str]) -> Option<f64> {
    fn num(v: &Value) -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                if keys.iter().any(|want| k.eq_ignore_ascii_case(want))
                    && let Some(n) = num(val)
                {
                    return Some(n);
                }
            }
            o.values().find_map(|val| number_at(val, keys))
        }
        Value::Array(a) => a.iter().find_map(|val| number_at(val, keys)),
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

// ---------------------------------------------------------------- reading a library reply

/// Keys a listing is expected to put its songs under, when it is not a bare array.
const LIST_KEYS: [&str; 5] = ["tracks", "songs", "items", "results", "data"];
/// Keys that carry a playable audio url in a listing item.
const URL_KEYS: [&str; 4] = ["audio_url", "url", "stream_url", "download_url"];

/// The items of a listing: a bare array (handled by [`songs_from`]), or the array under one of
/// [`LIST_KEYS`] — also when that key sits deeper, as in an MCP `content` wrapper.
fn list_items(v: &Value) -> Option<&Vec<Value>> {
    array_under(v, &LIST_KEYS)
}

/// The array under one of `keys`, at this level or deeper.
fn array_under<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                if keys.contains(&k.to_lowercase().as_str())
                    && let Some(a) = val.as_array()
                {
                    return Some(a);
                }
            }
            o.values().find_map(|val| array_under(val, keys))
        }
        Value::Array(a) => a.iter().find_map(|v| array_under(v, keys)),
        _ => None,
    }
}

fn songs_from(v: &Value) -> Vec<SunoSong> {
    if let Value::Array(items) = v {
        return items.iter().filter_map(parse_song).collect();
    }
    list_items(v)
        .or_else(|| first_object_array(v))
        .map(|items| items.iter().filter_map(parse_song).collect())
        .unwrap_or_default()
}

/// The one song in a retrieve reply: the outermost object that is a song (an id or an audio
/// url), else the first song nested anywhere inside it.
fn song_from(v: &Value) -> Option<SunoSong> {
    let direct = if v.is_object() { parse_song(v) } else { None };
    direct
        .filter(|s| !s.id.is_empty() || !s.audio_urls.is_empty())
        .or_else(|| match v {
            Value::Object(o) => o.values().find_map(song_from),
            Value::Array(a) => a.iter().find_map(song_from),
            _ => None,
        })
}

/// The first array whose objects look like songs, for a server that did not put its listing under
/// a [`LIST_KEYS`] name. `url`/`stem` keys make a stems table qualify too, but this is only the
/// last resort after the named keys came up empty.
fn first_object_array(v: &Value) -> Option<&Vec<Value>> {
    fn looks_like_songs(a: &[Value]) -> bool {
        a.iter().filter_map(Value::as_object).any(|o| {
            o.keys().any(|k| {
                let k = k.to_lowercase();
                [
                    "id",
                    "title",
                    "name",
                    "audio_url",
                    "url",
                    "stream_url",
                    "download_url",
                ]
                .contains(&k.as_str())
                    || k.contains("stem")
            })
        })
    }
    match v {
        Value::Array(a) if looks_like_songs(a) => Some(a),
        Value::Array(a) => a.iter().find_map(first_object_array),
        Value::Object(o) => o.values().find_map(first_object_array),
        _ => None,
    }
}

/// One listing item, read by the keys the beta plausibly uses. Nothing is required: an item the
/// walker cannot place still comes back with its `raw` so the UI can show what the server said.
fn parse_song(v: &Value) -> Option<SunoSong> {
    let o = v.as_object()?;
    let mut audio_urls = Vec::new();
    urls_at(v, &mut audio_urls);
    let mut stem_urls = Vec::new();
    stems_at(v, &mut stem_urls);
    Some(SunoSong {
        id: text_at(o, &["id"]).unwrap_or_default(),
        title: text_at(o, &["title", "name", "prompt"]).unwrap_or_default(),
        audio_urls,
        stem_urls,
        duration_secs: duration_at(o),
        created: text_at(o, &["created", "created_at"]),
        raw: v.clone(),
    })
}

/// The first non-empty string (or number) under one of `keys` at this level.
fn text_at(o: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        o.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| match v {
                Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
    })
}

/// Audio urls at this level, and inside an `audio`/`media` object or array of them.
fn urls_at(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                let k = k.to_lowercase();
                if URL_KEYS.contains(&k.as_str()) {
                    push_strings(val, out);
                } else if k == "audio" || k == "media" {
                    urls_at(val, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| urls_at(v, out)),
        _ => {}
    }
}

/// Everything under a key containing `stem`, at any depth: a url, an array of urls, or an object
/// of stem name → url all arrive here.
fn stems_at(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                if k.to_lowercase().contains("stem") {
                    push_strings(val, out);
                } else {
                    stems_at(val, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| stems_at(v, out)),
        _ => {}
    }
}

fn push_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) if !s.trim().is_empty() => {
            let s = s.trim().to_string();
            if !out.contains(&s) {
                out.push(s);
            }
        }
        Value::Array(a) => a.iter().for_each(|v| push_strings(v, out)),
        _ => {}
    }
}

/// The beta may report a length in seconds or milliseconds and does not say which. Values above
/// 1000 are read as milliseconds — a song is minutes, so only that unit can exceed 1000.
fn duration_at(o: &Map<String, Value>) -> Option<f64> {
    let raw = ["duration", "length"].iter().find_map(|key| {
        o.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse::<f64>().ok(),
                _ => None,
            })
    })?;
    Some(if raw > 1000.0 { raw / 1000.0 } else { raw })
}

/// The first cursor-like value anywhere in the reply, top level first, also through an MCP
/// `content` array.
fn next_token(v: &Value) -> Option<Value> {
    fn usable(v: &Value) -> bool {
        match v {
            Value::String(s) => !s.trim().is_empty(),
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::Bool(b) => *b,
            _ => false,
        }
    }
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                let k = k.to_lowercase();
                let cursorish = k.contains("cursor")
                    || k == "next"
                    || k == "next_token"
                    || k == "next_offset"
                    || k == "offset";
                if cursorish && usable(val) {
                    return Some(val.clone());
                }
            }
            o.values().find_map(next_token)
        }
        Value::Array(a) => a.iter().find_map(next_token),
        _ => None,
    }
}

/// The listing tool's own pagination argument, if it declares one. Without it a cursor cannot be
/// sent back, so there is nothing to follow.
fn cursor_prop(tool: &McpTool) -> Option<String> {
    [
        "cursor",
        "next_cursor",
        "page_token",
        "next",
        "offset",
        "page",
    ]
    .iter()
    .find_map(|k| prop_named(&tool.input_schema, k))
}

// ---------------------------------------------------------------- reading a voice reply

/// Keys a voice listing is expected to put its voices under, when it is not a bare array.
const VOICE_KEYS: [&str; 4] = ["voices", "personas", "items", "results"];

fn voices_from(v: &Value) -> Vec<SunoVoice> {
    let items = match v {
        Value::Array(a) => Some(a),
        _ => array_under(v, &VOICE_KEYS),
    };
    items
        .map(|items| items.iter().filter_map(parse_voice).collect())
        .unwrap_or_default()
}

/// One voice. An id is what the picker needs, so an item without one is not a voice; the name
/// falls back to the id so a row is never blank.
fn parse_voice(v: &Value) -> Option<SunoVoice> {
    if let Some(s) = v.as_str().filter(|s| !s.trim().is_empty()) {
        let id = s.trim().to_string();
        return Some(SunoVoice {
            name: id.clone(),
            id,
            raw: v.clone(),
        });
    }
    let o = v.as_object()?;
    let id = text_at(o, &["id", "voice_id", "persona_id"])?;
    let name = text_at(o, &["name", "title", "display_name"]).unwrap_or_else(|| id.clone());
    Some(SunoVoice {
        id,
        name,
        raw: v.clone(),
    })
}

/// The generation schema argument that carries a voice/persona id, by its name or description.
fn voice_prop(schema: &Value) -> Option<String> {
    schema
        .get("properties")?
        .as_object()?
        .iter()
        .find_map(|(key, prop)| {
            let described = prop
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            let named = key.to_lowercase();
            [named.as_str(), described.as_str()]
                .iter()
                .any(|s| s.contains("voice") || s.contains("persona"))
                .then(|| key.clone())
        })
}

// ---------------------------------------------------------------- reading playlist/workspace replies

/// The items of a list reply: a bare array, the array under one of `keys` (lowercase), or the
/// first array of id/name objects — which is what an MCP `content[].text` wrapper holds.
fn rows(v: &Value, keys: &[&str]) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        _ => array_under(v, keys)
            .or_else(|| first_object_array(v))
            .cloned()
            .unwrap_or_default(),
    }
}

/// One playlist row. A row without an id is not a playlist.
fn parse_playlist(v: &Value) -> Option<SunoPlaylist> {
    let o = v.as_object()?;
    let id = text_at(o, &["id", "playlist_id"])?;
    Some(SunoPlaylist {
        name: text_at(o, &["name", "title"]).unwrap_or_else(|| id.clone()),
        clip_count: number_at(
            v,
            &[
                "clip_count",
                "num_clips",
                "song_count",
                "track_count",
                "total_clips",
                "count",
            ],
        )
        .map(|n| n as u32),
        id,
        raw: v.clone(),
    })
}

/// One workspace row.
fn parse_workspace(v: &Value) -> Option<SunoWorkspace> {
    let o = v.as_object()?;
    let id = text_at(o, &["id", "workspace_id"])?;
    Some(SunoWorkspace {
        name: text_at(o, &["name", "title"]).unwrap_or_else(|| id.clone()),
        id,
        raw: v.clone(),
    })
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

/// Saves every url into one fresh per-song folder and reports the song. `stems_hint` lets a
/// caller that saw a `stems` key keep the flag even when only one file arrived.
fn save_song(
    urls: &[String],
    title: &str,
    stems_hint: bool,
    raw: Value,
    base: &Path,
) -> Result<GeneratedSong, String> {
    let dir = unique_dir(base, title)?;
    let mut files = Vec::new();
    for (index, url) in urls.iter().enumerate() {
        let path = download(url, &dir, index + 1)?;
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!("suno: saved {} ({bytes} bytes)", path.display());
        files.push(path);
    }
    let stems = files.len() > 1 || urls.iter().any(|u| url_ext(u) == ".zip") || stems_hint;
    println!(
        "suno: \"{title}\" — {} file(s), stems: {stems}",
        files.len()
    );
    Ok(GeneratedSong {
        files,
        stems,
        title: title.to_string(),
        raw,
    })
}

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
        let session = connect_at(&mock.url, &fresh_tokens("pick"), None, true).expect("connect");

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
        let mut session = connect_at(&mock.url, &fresh_tokens("stems"), None, true).expect("connect");
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
        let mut session = connect_at(&mock.url, &fresh_tokens("unknown"), None, true).expect("connect");
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
        let mut session = connect_at(&mock.url, &fresh_tokens("poll"), None, true).expect("connect");
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

    // ------------------------------------------------------------ the library

    /// A `tools/call` answer carrying `payload` as the `content[].text` the real server uses.
    fn call_reply(id: u64, payload: &Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [
            { "type": "text", "text": payload.to_string() } ] } })
        .to_string()
    }

    fn tool_calls(mock: &Mock) -> usize {
        mock.requests()
            .iter()
            .filter(|r| r.contains("tools/call"))
            .count()
    }

    /// (a) Two songs under `tracks`: titles, ids, audio url count and stem detection.
    #[test]
    fn a_tracks_object_lists_two_songs_with_stems() {
        let tools = vec![
            tool(
                "library_search",
                "Search your Suno library",
                json!({
                    "query": { "type": "string" },
                    "limit": { "type": "integer" },
                }),
            ),
            tool(
                "generate_song",
                "Create a new song from a prompt",
                json!({ "prompt": { "type": "string" } }),
            ),
            tool("credits_balance", "Current credit balance", json!({})),
        ];
        let tracks = json!({ "tracks": [
            {
                "id": "song-1",
                "title": "Neon Drive",
                "audio_url": "https://cdn.example/neon-drive.mp3",
                "stems": ["https://cdn.example/neon-drive-drums.wav",
                          "https://cdn.example/neon-drive-bass.wav"],
                "duration": 215.4,
                "created": "2026-10-01T12:00:00Z"
            },
            {
                "id": "song-2",
                "title": "Midnight Run",
                "audio_url": "https://cdn.example/midnight-run.mp3",
                "duration": 180000
            }
        ] });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &tracks)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("lib-a"), None, true).expect("connect");
        let songs = session.library(Some("neon"), 10).expect("library");

        assert_eq!(songs.len(), 2);
        for song in &songs {
            println!(
                "title: {:<14} id: {:<8} audio urls: {} stems: {} ({} stem urls)",
                song.title,
                song.id,
                song.audio_urls.len(),
                !song.stem_urls.is_empty(),
                song.stem_urls.len()
            );
        }
        println!(
            "durations: {:?}",
            songs.iter().map(|s| s.duration_secs).collect::<Vec<_>>()
        );
        println!("created: {:?}", songs[0].created);
        assert_eq!(songs[0].title, "Neon Drive");
        assert_eq!(songs[0].id, "song-1");
        assert_eq!(songs[0].audio_urls.len(), 1);
        assert_eq!(songs[0].stem_urls.len(), 2);
        assert_eq!(songs[0].duration_secs, Some(215.4));
        assert_eq!(songs[1].id, "song-2");
        assert_eq!(songs[1].duration_secs, Some(180.0));
        assert!(songs[1].stem_urls.is_empty());
        assert_eq!(mock.count(), 4);
    }

    /// (b) A bare array with `name`/`stream_url`/`audio.stems`: the tolerant keys are the ones used.
    #[test]
    fn a_bare_array_uses_the_fallback_keys() {
        let tools = vec![tool(
            "my_songs",
            "List your songs",
            json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
        )];
        let items = json!([{
            "id": "song-7",
            "name": "Glass Harbor",
            "stream_url": "https://cdn.example/glass-harbor.mp3",
            "audio": { "stems": ["https://cdn.example/glass-harbor-vox.wav",
                                 "https://cdn.example/glass-harbor-keys.wav"] },
            "length": "247"
        }]);
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &items)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("lib-b"), None, true).expect("connect");
        let songs = session.library(None, 5).expect("library");

        assert_eq!(songs.len(), 1);
        let song = &songs[0];
        println!("song: {song:?}");
        println!("title from `name`: {}", song.title);
        println!("audio from `stream_url`: {:?}", song.audio_urls);
        println!("stems from `audio.stems`: {:?}", song.stem_urls);
        println!("duration from `length` string: {:?}", song.duration_secs);
        assert_eq!(song.title, "Glass Harbor");
        assert_eq!(
            song.audio_urls,
            vec!["https://cdn.example/glass-harbor.mp3"]
        );
        assert_eq!(song.stem_urls.len(), 2);
        assert_eq!(song.duration_secs, Some(247.0));
        assert_eq!(song.created, None);
    }

    /// (c) Page 1 carries a `next` cursor, page 2 does not: exactly two listing calls, two songs.
    #[test]
    fn a_next_cursor_pulls_the_second_page() {
        let tools = vec![tool(
            "library_search",
            "Search your Suno library",
            json!({
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "cursor": { "type": "string" },
            }),
        )];
        let page1 = json!({
            "songs": [{ "id": "a", "title": "First Light",
                        "audio_url": "https://cdn.example/a.mp3" }],
            "next": "page-2"
        });
        let page2 = json!({
            "songs": [{ "id": "b", "title": "Second Wind",
                        "audio_url": "https://cdn.example/b.mp3" }]
        });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &page1)),
            Reply::json(call_reply(4, &page2)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("lib-c"), None, true).expect("connect");
        let songs = session.library(None, 10).expect("library");

        let calls = tool_calls(&mock);
        println!(
            "listing calls: {calls}, total http requests: {}",
            mock.count()
        );
        println!("songs: {}", songs.len());
        println!(
            "titles: {:?}",
            songs.iter().map(|s| s.title.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(calls, 2);
        assert_eq!(mock.count(), 5);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[1].title, "Second Wind");
        let requests = mock.requests();
        let second = requests
            .iter()
            .find(|r| r.contains("tools/call") && r.contains("page-2"));
        println!(
            "page 2 carried the cursor: {}",
            second.is_some_and(|r| r.contains("\"cursor\":\"page-2\""))
        );
        assert!(
            second
                .expect("second page call")
                .contains("\"cursor\":\"page-2\"")
        );
    }

    /// (d) A server with no library-like tool: the error lists every tool it does expose.
    #[test]
    fn a_server_without_a_listing_tool_says_what_it_has() {
        let tools = vec![
            tool(
                "generate_song",
                "Create a new song from a prompt",
                json!({ "prompt": { "type": "string" } }),
            ),
            tool(
                "create_music",
                "Create a song",
                json!({ "prompt": { "type": "string" } }),
            ),
            tool(
                "song_status",
                "Check generation status for a song",
                json!({ "id": { "type": "string" } }),
            ),
        ];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("lib-d"), None, true).expect("connect");
        let err = session.library(None, 10).expect_err("no lister exists");
        println!("error: {err}");
        for name in ["generate_song", "create_music", "song_status"] {
            assert!(err.contains(name), "error does not list {name}: {err}");
        }
        // Nothing was called: the failure is at tool selection.
        println!("requests: {}", mock.count());
        assert_eq!(mock.count(), 3);
    }

    /// (e) `download_song` puts the listed song's stems in the same per-song folder as `generate`.
    #[test]
    fn download_song_saves_the_stems_of_a_listed_song() {
        let audio = Mock::new(vec![Reply::json("RIFFdrums"), Reply::json("RIFFbass-")]);
        let tools = vec![tool(
            "library_search",
            "Search your Suno library",
            json!({ "query": { "type": "string" }, "limit": { "type": "integer" } }),
        )];
        let tracks = json!({ "tracks": [
            {
                "id": "song-1",
                "title": "Neon Drive",
                "stems": [format!("{}/drum-stem.wav", audio.url),
                          format!("{}/bass-stem.wav", audio.url)]
            },
            { "id": "song-2", "title": "Midnight Run",
              "audio_url": "https://cdn.example/midnight-run.mp3" }
        ] });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &tracks)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("lib-e"), None, true).expect("connect");
        let songs = session.library(None, 10).expect("library");
        let dir = tmp_dir("lib-e");
        let saved = session
            .download_song_into(&songs[0], &dir)
            .expect("download");

        println!("title: {}", saved.title);
        println!("stems: {}", saved.stems);
        for path in &saved.files {
            let bytes = std::fs::metadata(path).expect("file").len();
            println!("file: {} ({bytes} bytes)", path.display());
            assert!(bytes > 0);
        }
        println!("download requests: {}", audio.count());
        assert_eq!(saved.title, "Neon Drive");
        assert!(saved.stems);
        assert_eq!(saved.files.len(), 2);
        assert!(saved.files[0].parent().unwrap().ends_with("neon-drive"));
        assert_eq!(audio.count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------ the options a UI draws

    /// A generator tool carrying the options Suno's own "More options" panel offers.
    fn option_tool() -> Value {
        tool(
            "generate_song",
            "Create a new song from a prompt",
            json!({
                "prompt": { "type": "string" },
                "vocal_gender": { "type": "string", "enum": ["female", "male"] },
                "duration": { "type": "string", "enum": ["auto", "custom"] },
                "weirdness": { "type": "number" },
                "max_mode": { "type": "boolean" },
                "title": { "type": "string" }
            }),
        )
    }

    /// (a) Every option the server declares is reachable from `generation_schema()`.
    #[test]
    fn the_generation_schema_exposes_every_option_the_server_declares() {
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, vec![option_tool()])),
        ]);
        let session = connect_at(&mock.url, &fresh_tokens("schema"), None, true).expect("connect");

        let chosen = session.generation_tool().expect("a generator");
        println!("generation tool: {}", chosen.name);
        let schema = session.generation_schema().expect("a schema");
        let props = schema["properties"].as_object().expect("properties");
        println!("{} properties:", props.len());
        for (name, prop) in props {
            let ty = prop["type"].as_str().unwrap_or("?");
            let enums: Vec<&str> = prop["enum"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            println!("  {name:<14} type {ty:<7} enum {enums:?}");
        }
        for key in ["vocal_gender", "duration", "weirdness", "max_mode", "title"] {
            assert!(props.contains_key(key), "schema has no {key}: {schema}");
        }
        assert_eq!(props["vocal_gender"]["enum"], json!(["female", "male"]));
        assert_eq!(props["duration"]["enum"], json!(["auto", "custom"]));
        assert_eq!(props["weirdness"]["type"], "number");
        assert_eq!(props["max_mode"]["type"], "boolean");
        println!("schema: {schema}");
    }

    /// A tool that declares no schema still answers with a form (empty `properties`).
    #[test]
    fn a_tool_without_a_schema_still_yields_an_empty_form() {
        let bare =
            json!({ "name": "generate_song", "description": "Create a new song from a prompt" });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, vec![bare])),
        ]);
        let session = connect_at(&mock.url, &fresh_tokens("no-schema"), None, true).expect("connect");
        let schema = session.generation_schema().expect("a schema");
        println!("schema: {schema}");
        assert_eq!(schema["properties"], json!({}));
    }

    /// (b) `generate_with` sends the JSON it was handed, untouched, and saves what comes back.
    #[test]
    fn generate_with_sends_exactly_the_given_args() {
        let audio = Mock::new(vec![Reply::json("RIFF-female")]);
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, vec![option_tool()])),
            Reply::json(call_reply(
                3,
                &json!({
                    "title": "Hand Built",
                    "audio_url": format!("{}/hand-built.wav", audio.url)
                }),
            )),
        ]);
        let dir = tmp_dir("given");
        let mut session = connect_at(&mock.url, &fresh_tokens("given"), None, true).expect("connect");
        let args = json!({ "prompt": "x", "vocal_gender": "female", "weirdness": 0.7 });
        let song = session
            .generate_with_into(args.clone(), &dir)
            .expect("generate");

        let request = mock
            .requests()
            .into_iter()
            .find(|r| r.contains("\"method\":\"tools/call\""))
            .expect("a tools/call request");
        let body = request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or(&request);
        let call: Value = serde_json::from_str(body.trim()).expect("a JSON-RPC body");
        println!("tools/call body: {body}");
        println!("arguments: {}", call["params"]["arguments"]);
        assert_eq!(call["params"]["name"], "generate_song");
        assert_eq!(call["params"]["arguments"], args);

        println!("title: {}", song.title);
        for path in &song.files {
            let bytes = std::fs::metadata(path).expect("file").len();
            println!("file: {} ({bytes} bytes)", path.display());
            assert!(bytes > 0, "{} is empty", path.display());
        }
        assert_eq!(song.files.len(), 1);
        assert_eq!(song.title, "Hand Built");
        assert_eq!(audio.count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) A `voices` reply: both entries come out with id and name, called with no arguments.
    #[test]
    fn voices_are_read_from_a_voices_reply() {
        let tools = vec![
            option_tool(),
            tool(
                "list_voices",
                "List the user's voices and personas",
                json!({}),
            ),
        ];
        let reply = json!({ "voices": [
            { "voice_id": "v1", "name": "Smoky" },
            { "persona_id": "p2", "title": "Bright" }
        ] });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("voices"), None, true).expect("connect");
        let voices = session.voices().expect("voices");

        for voice in &voices {
            println!(
                "voice: id {} name {} raw {}",
                voice.id, voice.name, voice.raw
            );
        }
        assert_eq!(voices.len(), 2);
        assert_eq!(
            (voices[0].id.as_str(), voices[0].name.as_str()),
            ("v1", "Smoky")
        );
        assert_eq!(
            (voices[1].id.as_str(), voices[1].name.as_str()),
            ("p2", "Bright")
        );
        assert!(session.voices_hint().is_none());
        let request = mock
            .requests()
            .into_iter()
            .find(|r| r.contains("list_voices"))
            .expect("a voice call");
        println!(
            "voice call sent empty arguments: {}",
            request.contains("\"arguments\":{}")
        );
        assert!(request.contains("\"arguments\":{}"));
    }

    /// (d) No voice tool: an empty list and a hint naming what the server would need.
    #[test]
    fn a_server_without_a_voice_tool_says_what_is_needed() {
        let tools = vec![
            option_tool(),
            tool(
                "library_search",
                "Search your Suno library",
                json!({ "query": { "type": "string" } }),
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
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("no-voice"), None, true).expect("connect");

        let voices = session
            .voices()
            .expect("an absent voice tool is not an error");
        let hint = session.voices_hint().expect("a hint");
        println!("voices: {voices:?}");
        println!("hint: {hint}");
        assert!(voices.is_empty());
        assert!(hint.contains("list_voices"), "hint names no tool: {hint}");
        assert!(hint.contains("generate_song"), "hint lists nothing: {hint}");
        assert_eq!(mock.count(), 3, "a tool was called without a voice tool");
    }

    /// (e) The schema argument that takes a voice/persona id is found, by name or description.
    #[test]
    fn the_voice_parameter_is_read_from_the_schema() {
        let tools = vec![tool(
            "generate_song",
            "Create a new song",
            json!({
                "prompt": { "type": "string" },
                "persona_id": { "type": "string", "description": "the persona to sing" },
                "duration": { "type": "string", "enum": ["auto", "custom"] }
            }),
        )];
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
        ]);
        let session = connect_at(&mock.url, &fresh_tokens("voice-param"), None, true).expect("connect");

        let param = session.voice_param().expect("a voice parameter");
        println!("voice param: {param}");
        assert_eq!(param, "persona_id");
        // A schema with neither a voice-ish key nor description has nowhere to put the id.
        let plain = json!({ "properties": { "duration": { "type": "string" } } });
        println!("voice param for a plain schema: {:?}", voice_prop(&plain));
        assert_eq!(voice_prop(&plain), None);
    }

    // ------------------------------------------------------------ the rest of the server's tools

    /// The tool name of every `tools/call` the mock saw, in request order.
    fn called_tools(mock: &Mock) -> Vec<String> {
        mock.requests()
            .iter()
            .filter_map(|r| {
                let body = r.split_once("\r\n\r\n").map(|(_, body)| body)?;
                let call: Value = serde_json::from_str(body.trim()).ok()?;
                (call["method"] == "tools/call")
                    .then(|| call["params"]["name"].as_str().unwrap_or_default().to_string())
            })
            .collect()
    }

    /// (a) `audio-retrieve` is found by name, the id goes in the argument its schema names, and
    /// the reply yields the audio and stem urls a listing does not carry.
    #[test]
    fn retrieve_song_reads_the_audio_and_stem_urls_of_one_song() {
        let tools = vec![
            tool(
                "library-list",
                "List your songs",
                json!({ "limit": { "type": "integer" } }),
            ),
            tool(
                "audio-retrieve",
                "Retrieve one song by id",
                json!({ "id": { "type": "string" } }),
            ),
            tool(
                "audio-create",
                "Create a new song",
                json!({ "description": { "type": "string" } }),
            ),
        ];
        let reply = json!({
            "id": "song-77",
            "title": "Deep Water",
            "audio_url": "https://cdn.example/deep-water.mp3",
            "stems": ["https://cdn.example/deep-water-vox.wav",
                      "https://cdn.example/deep-water-drums.wav"]
        });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("retrieve"), None, true).expect("connect");
        let song = session.retrieve_song("song-77").expect("retrieve");

        println!("id: {}", song.id);
        println!("title: {}", song.title);
        println!(
            "audio urls: {} stem urls: {}",
            song.audio_urls.len(),
            song.stem_urls.len()
        );
        println!("audio: {:?}", song.audio_urls);
        println!("stems: {:?}", song.stem_urls);
        assert_eq!(song.id, "song-77");
        assert_eq!(song.title, "Deep Water");
        assert_eq!(song.audio_urls.len(), 1);
        assert_eq!(song.stem_urls.len(), 2);

        let request = mock
            .requests()
            .into_iter()
            .find(|r| r.contains("audio-retrieve"))
            .expect("a retrieve call");
        println!("tool calls: {:?}", called_tools(&mock));
        assert!(
            request.contains("\"id\":\"song-77\""),
            "the id did not reach the declared argument: {request}"
        );
    }

    /// (b) A row from `library-list` has no urls: `download_song` asks the retrieve tool first,
    /// and the retrieved urls are what land on disk.
    #[test]
    fn download_song_retrieves_a_listed_song_before_saving_it() {
        let audio = Mock::new(vec![Reply::json("RIFFvox-"), Reply::json("RIFFdrum")]);
        let tools = vec![
            tool("library-list", "List your songs", json!({})),
            tool(
                "audio-retrieve",
                "Retrieve one song by id",
                json!({ "id": { "type": "string" } }),
            ),
        ];
        let retrieved = json!({
            "id": "song-9",
            "title": "Row Song",
            "audio_url": format!("{}/row-song.mp3", audio.url),
            "stems": [format!("{}/row-vox.wav", audio.url),
                      format!("{}/row-drum.wav", audio.url)]
        });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &retrieved)),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("dl-row"), None, true).expect("connect");
        // Exactly what a listing gives on the live server: an id and a title, no urls.
        let row = SunoSong {
            id: "song-9".to_string(),
            title: "Row Song".to_string(),
            audio_urls: Vec::new(),
            stem_urls: Vec::new(),
            duration_secs: Some(201.0),
            created: Some("2026-10-30T09:00:00Z".to_string()),
            raw: json!({ "id": "song-9", "title": "Row Song" }),
        };
        let dir = tmp_dir("dl-row");
        let saved = session
            .download_song_into(&row, &dir)
            .expect("download");

        println!("request order: {:?}", called_tools(&mock));
        println!("title: {}, stems: {}", saved.title, saved.stems);
        for path in &saved.files {
            let bytes = std::fs::metadata(path).expect("file").len();
            println!("file: {} ({bytes} bytes)", path.display());
            assert!(bytes > 0, "{} is empty", path.display());
        }
        assert_eq!(
            called_tools(&mock),
            ["audio-retrieve"],
            "the retrieve tool was not the (only) call"
        );
        assert_eq!(saved.files.len(), 2);
        assert!(saved.stems);
        assert_eq!(saved.title, "Row Song");
        assert!(saved.files[0].parent().unwrap().ends_with("row-song"));
        assert_eq!(audio.count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) `playlists()` reads a `playlists` object, with the clip count.
    #[test]
    fn playlists_are_read_from_a_playlists_reply() {
        let tools = vec![
            tool("playlists-list", "List the user's playlists", json!({})),
            tool(
                "playlists-create",
                "Create a playlist",
                json!({ "name": { "type": "string" } }),
            ),
        ];
        let reply = json!({ "playlists": [
            { "id": "p1", "name": "Roadtrip", "clip_count": 12 }
        ] });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("playlists"), None, true).expect("connect");
        let playlists = session.playlists().expect("playlists");

        println!("tool calls: {:?}", called_tools(&mock));
        for p in &playlists {
            println!("playlist: {} / {} / {:?} clips", p.id, p.name, p.clip_count);
        }
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].id, "p1");
        assert_eq!(playlists[0].name, "Roadtrip");
        assert_eq!(playlists[0].clip_count, Some(12));
        assert_eq!(called_tools(&mock), ["playlists-list"]);
    }

    /// (d) `workspaces()` reads a bare array.
    #[test]
    fn workspaces_are_read_from_a_bare_array() {
        let tools = vec![tool("workspaces-list", "List workspaces", json!({}))];
        let reply = json!([
            { "id": "w1", "name": "Night Studio" },
            { "id": "w2" }
        ]);
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("workspaces"), None, true).expect("connect");
        let workspaces = session.workspaces().expect("workspaces");

        for w in &workspaces {
            println!("workspace: {} / {}", w.id, w.name);
        }
        assert_eq!(workspaces.len(), 2);
        assert_eq!(
            (workspaces[0].id.as_str(), workspaces[0].name.as_str()),
            ("w1", "Night Studio")
        );
        // A nameless workspace falls back to its id so the row is never blank.
        assert_eq!(workspaces[1].name, "w2");
    }

    /// (e) `account()` reads credits and plan, and never calls the mutating `account-upgrade`.
    #[test]
    fn account_reads_credits_and_plan_without_upgrading() {
        let tools = vec![
            tool("account-me", "Your account", json!({})),
            tool("account-upgrade", "Upgrade your plan", json!({ "plan": { "type": "string" } })),
            tool("account-usage", "Your credit usage", json!({})),
        ];
        let reply = json!({ "credits_left": 120.5, "plan": "Pro" });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
            // The second account tool answers too; the first value found wins.
            Reply::json(call_reply(4, &json!({ "credits": 118.0 }))),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("account"), None, true).expect("connect");
        let account = session.account().expect("account");

        println!("tool calls: {:?}", called_tools(&mock));
        println!("credits: {:?}", account.credits);
        println!("plan: {:?}", account.plan);
        println!("raw: {}", account.raw);
        assert_eq!(account.credits, Some(120.5));
        assert_eq!(account.plan.as_deref(), Some("Pro"));
        assert!(
            !called_tools(&mock).contains(&"account-upgrade".to_string()),
            "a status read called a mutating tool"
        );
    }

    /// (f) `call` sends the arguments it was given, byte for byte, and returns the reply raw.
    #[test]
    fn call_passes_the_arguments_through_untouched() {
        let tools = vec![
            tool(
                "audio-wait",
                "Wait for a clip to finish",
                json!({ "clip_id": { "type": "string" } }),
            ),
            tool("account-me", "Your account", json!({})),
        ];
        let reply = json!({ "status": "complete", "clip_id": "c-1" });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session = connect_at(&mock.url, &fresh_tokens("call"), None, true).expect("connect");
        let args = json!({ "clip_id": "c-1", "wait": true, "timeout_ms": 30000 });
        let result = session.call("audio-wait", args.clone()).expect("call");

        let request = mock
            .requests()
            .into_iter()
            .find(|r| r.contains("audio-wait"))
            .expect("an audio-wait call");
        let body = request.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or(&request);
        let call: Value = serde_json::from_str(body.trim()).expect("a JSON-RPC body");
        println!("tool call name: {}", call["params"]["name"]);
        println!("arguments the mock received: {}", call["params"]["arguments"]);
        println!("reply returned: {result}");
        assert_eq!(call["params"]["arguments"], args);
        // Untouched: the MCP result, not its `content[].text` expanded.
        assert_eq!(result["content"][0]["text"], reply.to_string());
    }

    /// En nedladdning som kostar en kredit får **inte** ske tyst: svaret ska bli `NEEDS_CREDIT`,
    /// och inga filer ska skrivas.
    #[test]
    fn a_download_that_costs_a_credit_asks_first() {
        let tools = vec![
            tool(
                "library-list",
                "List your songs",
                json!({ "limit": { "type": "integer" } }),
            ),
            tool(
                "audio-download",
                "Download one song",
                json!({ "id": { "type": "string" } }),
            ),
            tool(
                "audio-create",
                "Create a new song",
                json!({ "description": { "type": "string" } }),
            ),
        ];
        let reply = json!({
            "credits_remaining": 22,
            "format": "mp3",
            "message": "Downloading this song uses one of the user's suno.com download credits. Ask them first, then call again with confirm: true.",
            "status": "requires_credit"
        });
        let mock = Mock::new(vec![
            Reply::json(INIT),
            Reply::status(202),
            Reply::json(tools_reply(2, tools)),
            Reply::json(call_reply(3, &reply)),
        ]);
        let mut session =
            connect_at(&mock.url, &fresh_tokens("credit"), None, true).expect("connect");
        let song = SunoSong {
            id: "credit-1".to_string(),
            title: "Costs A Credit".to_string(),
            audio_urls: vec!["https://cdn.example/c.mp3".to_string()],
            stem_urls: Vec::new(),
            duration_secs: None,
            created: None,
            raw: json!({}),
        };
        let error = session.download_song(&song).expect_err("must ask first");
        println!("fel: {error}");
        assert_eq!(error, NEEDS_CREDIT);
    }
}