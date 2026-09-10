use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ai_generator::AiGeneratedClip;

const SYSTEM_PROMPT: &str = "You are Sonix, a music-pattern generator embedded in a DAW. \
Respond with ONLY a JSON array (no markdown fences, no prose) containing 1 to 3 objects. \
Each object must have exactly these fields:\n\
- \"title\": a short string\n\
- \"genre\": a string\n\
- \"key_signature\": a string such as \"A Minor\"\n\
- \"bpm\": a number\n\
- \"steps\": an array of exactly 16 booleans (true = a note fires on that 16th-note step)\n\
- \"notes\": an array of exactly 16 integer MIDI note numbers (0-127). Put the root note on steps where \"steps\" is false.\n\
Keep the notes inside one octave suited to the requested instrument. Return valid JSON only.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    Offline,
    OpenAi,
    Anthropic,
    OpenRouter,
    Ollama,
}

impl Default for AiProvider {
    fn default() -> Self {
        AiProvider::Offline
    }
}

impl AiProvider {
    pub const ALL: [AiProvider; 5] = [
        AiProvider::Offline,
        AiProvider::OpenAi,
        AiProvider::Anthropic,
        AiProvider::OpenRouter,
        AiProvider::Ollama,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AiProvider::Offline => "Offline (regelbaserad)",
            AiProvider::OpenAi => "OpenAI",
            AiProvider::Anthropic => "Anthropic (Claude)",
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Ollama => "Ollama (lokal)",
        }
    }

    pub fn requires_key(self) -> bool {
        matches!(
            self,
            AiProvider::OpenAi | AiProvider::Anthropic | AiProvider::OpenRouter
        )
    }

    pub fn default_base(self) -> &'static str {
        match self {
            AiProvider::Offline => "",
            AiProvider::OpenAi => "https://api.openai.com/v1",
            AiProvider::Anthropic => "https://api.anthropic.com/v1",
            AiProvider::OpenRouter => "https://openrouter.ai/api/v1",
            AiProvider::Ollama => "http://localhost:11434/v1",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            AiProvider::Offline => "",
            AiProvider::OpenAi => "gpt-4o-mini",
            AiProvider::Anthropic => "claude-3-5-haiku-latest",
            AiProvider::OpenRouter => "openai/gpt-4o-mini",
            AiProvider::Ollama => "llama3.1",
        }
    }

    /// Suggested model names shown in the UI picker. Free-text entry is still
    /// allowed, so this list does not need to be exhaustive.
    pub fn known_models(self) -> &'static [&'static str] {
        match self {
            AiProvider::Offline => &[],
            AiProvider::OpenAi => &["gpt-4o-mini", "gpt-4o", "gpt-4.1-mini", "gpt-4.1", "o4-mini"],
            AiProvider::Anthropic => &[
                "claude-3-5-haiku-latest",
                "claude-3-5-sonnet-latest",
                "claude-3-7-sonnet-latest",
                "claude-sonnet-4-0",
            ],
            AiProvider::OpenRouter => &[
                "openai/gpt-4o-mini",
                "anthropic/claude-3.5-sonnet",
                "google/gemini-2.0-flash-001",
                "meta-llama/llama-3.3-70b-instruct",
            ],
            AiProvider::Ollama => &["llama3.1", "llama3.2", "qwen2.5", "mistral", "phi4"],
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_lowercase().as_str() {
            "offline" | "local" => Some(AiProvider::Offline),
            "openai" | "open-ai" => Some(AiProvider::OpenAi),
            "anthropic" | "claude" => Some(AiProvider::Anthropic),
            "openrouter" | "open-router" => Some(AiProvider::OpenRouter),
            "ollama" => Some(AiProvider::Ollama),
            _ => None,
        }
    }
}

fn default_timeout() -> u64 {
    60
}

fn default_max_tokens() -> u32 {
    1024
}

fn default_temperature() -> f32 {
    0.8
}

/// Backends that return rendered audio bytes (WAV/MP3/…) rather than note data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioProvider {
    Offline,
    #[serde(rename = "openai_tts")]
    OpenAiTts,
    Stability,
}

impl Default for AudioProvider {
    fn default() -> Self {
        AudioProvider::Offline
    }
}

impl AudioProvider {
    pub const ALL: [AudioProvider; 3] = [
        AudioProvider::Offline,
        AudioProvider::OpenAiTts,
        AudioProvider::Stability,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AudioProvider::Offline => "Offline (lokal DSP)",
            AudioProvider::OpenAiTts => "OpenAI TTS (tal/sång)",
            AudioProvider::Stability => "Stability Stable Audio",
        }
    }

    pub fn requires_key(self) -> bool {
        matches!(self, AudioProvider::OpenAiTts | AudioProvider::Stability)
    }

    pub fn default_base(self) -> &'static str {
        match self {
            AudioProvider::Offline => "",
            AudioProvider::OpenAiTts => "https://api.openai.com/v1",
            AudioProvider::Stability => "https://api.stability.ai",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            AudioProvider::Offline => "",
            AudioProvider::OpenAiTts => "gpt-4o-mini-tts",
            AudioProvider::Stability => "stable-audio-2",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_lowercase().as_str() {
            "offline" | "local" => Some(AudioProvider::Offline),
            "openai" | "openai_tts" | "tts" => Some(AudioProvider::OpenAiTts),
            "stability" | "stable-audio" | "stable_audio" => Some(AudioProvider::Stability),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub provider: AiProvider,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default)]
    pub audio_provider: AudioProvider,
    #[serde(default)]
    pub audio_base_url: String,
    #[serde(default)]
    pub audio_model: String,
    #[serde(default)]
    pub audio_api_key: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::Offline,
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            timeout_secs: default_timeout(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            audio_provider: AudioProvider::Offline,
            audio_base_url: String::new(),
            audio_model: String::new(),
            audio_api_key: String::new(),
        }
    }
}

impl AiConfig {
    pub fn config_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("sonix").join("ai.json"))
    }

    pub fn load() -> Self {
        let mut cfg = Self::config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<AiConfig>(&s).ok())
            .unwrap_or_default();

        if let Ok(v) = std::env::var("SONIX_AI_PROVIDER")
            && let Some(p) = AiProvider::from_code(&v)
        {
            cfg.provider = p;
        }
        if let Ok(v) = std::env::var("SONIX_AI_API_KEY") {
            cfg.api_key = v;
        }
        if let Ok(v) = std::env::var("SONIX_AI_BASE_URL") {
            cfg.base_url = v;
        }
        if let Ok(v) = std::env::var("SONIX_AI_MODEL") {
            cfg.model = v;
        }
        if let Ok(v) = std::env::var("SONIX_AI_AUDIO_PROVIDER")
            && let Some(p) = AudioProvider::from_code(&v)
        {
            cfg.audio_provider = p;
        }
        if let Ok(v) = std::env::var("SONIX_AI_AUDIO_KEY") {
            cfg.audio_api_key = v;
        }
        if let Ok(v) = std::env::var("SONIX_AI_AUDIO_BASE_URL") {
            cfg.audio_base_url = v;
        }
        if let Ok(v) = std::env::var("SONIX_AI_AUDIO_MODEL") {
            cfg.audio_model = v;
        }
        cfg
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Self::config_path().ok_or("Kunde inte hitta någon konfigurationsmapp")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Kunde inte skapa {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Kunde inte serialisera AI-konfiguration: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Kunde inte skriva {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(path)
    }

    pub fn resolved_base(&self) -> String {
        if self.base_url.trim().is_empty() {
            self.provider.default_base().to_string()
        } else {
            self.base_url.trim().to_string()
        }
    }

    pub fn resolved_model(&self) -> String {
        if self.model.trim().is_empty() {
            self.provider.default_model().to_string()
        } else {
            self.model.trim().to_string()
        }
    }

    pub fn is_ready(&self) -> bool {
        self.provider != AiProvider::Offline
            && (!self.provider.requires_key() || !self.api_key.trim().is_empty())
            && !self.resolved_base().is_empty()
            && !self.resolved_model().is_empty()
    }

    pub fn resolved_audio_base(&self) -> String {
        if self.audio_base_url.trim().is_empty() {
            self.audio_provider.default_base().to_string()
        } else {
            self.audio_base_url.trim().to_string()
        }
    }

    pub fn resolved_audio_model(&self) -> String {
        if self.audio_model.trim().is_empty() {
            self.audio_provider.default_model().to_string()
        } else {
            self.audio_model.trim().to_string()
        }
    }

    /// Audio requests fall back to the chat API key when no dedicated one is set.
    pub fn resolved_audio_key(&self) -> String {
        if self.audio_api_key.trim().is_empty() {
            self.api_key.trim().to_string()
        } else {
            self.audio_api_key.trim().to_string()
        }
    }

    pub fn audio_is_ready(&self) -> bool {
        self.audio_provider != AudioProvider::Offline
            && (!self.audio_provider.requires_key() || !self.resolved_audio_key().is_empty())
            && !self.resolved_audio_base().is_empty()
            && !self.resolved_audio_model().is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct AiClipJson {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    genre: Option<String>,
    #[serde(default)]
    key_signature: Option<String>,
    #[serde(default)]
    bpm: Option<f32>,
    #[serde(default)]
    steps: Vec<bool>,
    #[serde(default)]
    notes: Vec<i64>,
}

impl AiClipJson {
    fn into_clip(self) -> Result<AiGeneratedClip, String> {
        if self.steps.len() != 16 {
            return Err(format!("'steps' hade {} värden (förväntade 16)", self.steps.len()));
        }
        if self.notes.len() != 16 {
            return Err(format!("'notes' hade {} värden (förväntade 16)", self.notes.len()));
        }
        let mut notes = [60u8; 16];
        for (i, &n) in self.notes.iter().enumerate() {
            notes[i] = n.clamp(0, 127) as u8;
        }
        let mut steps = [false; 16];
        steps.copy_from_slice(&self.steps);
        Ok(AiGeneratedClip {
            title: self.title.unwrap_or_else(|| "AI Clip".to_string()),
            genre: self.genre.unwrap_or_else(|| "AI".to_string()),
            key_signature: self.key_signature.unwrap_or_else(|| "C".to_string()),
            bpm: self.bpm.unwrap_or(120.0).clamp(40.0, 300.0),
            channel_steps: steps,
            notes,
            color: eframe::egui::Color32::from_rgb(46, 204, 113),
        })
    }
}

fn extract_json_slice(s: &str) -> Option<&str> {
    let start = s.find(|c| c == '[' || c == '{')?;
    let end = if s.as_bytes()[start] == b'[' {
        s.rfind(']')?
    } else {
        s.rfind('}')?
    };
    if end <= start {
        return None;
    }
    Some(&s[start..=end])
}

fn extract_content(response_json: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(response_json).map_err(|e| format!("Ogiltigt JSON från AI-API: {e}"))?;
    if let Some(s) = v.pointer("/choices/0/message/content").and_then(|c| c.as_str()) {
        return Ok(s.to_string());
    }
    if let Some(s) = v.pointer("/message/content").and_then(|c| c.as_str()) {
        return Ok(s.to_string());
    }
    if let Some(s) = v.pointer("/content/0/text").and_then(|c| c.as_str()) {
        return Ok(s.to_string());
    }
    Err("AI-svaret saknade textinnehåll".to_string())
}

fn parse_clips(content: &str) -> Result<Vec<AiGeneratedClip>, String> {
    let slice = extract_json_slice(content).ok_or("Hittade ingen JSON i AI-svaret")?;
    let value: serde_json::Value =
        serde_json::from_str(slice).map_err(|e| format!("Kunde inte tolka AI-JSON: {e}"))?;

    let arr = if let Some(a) = value.as_array() {
        a.clone()
    } else if let Some(a) = value.get("clips").and_then(|c| c.as_array()) {
        a.clone()
    } else {
        return Err("AI-JSON var varken en lista eller ett objekt med 'clips'".to_string());
    };

    let mut clips = Vec::new();
    for item in arr {
        let clip: AiClipJson = serde_json::from_value(item)
            .map_err(|e| format!("Kunde inte tolka ett AI-klip: {e}"))?;
        clips.push(clip.into_clip()?);
    }
    if clips.is_empty() {
        return Err("AI-svaret innehöll inga klip".to_string());
    }
    Ok(clips)
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

pub fn generate_clips(config: &AiConfig, user_prompt: &str) -> Result<Vec<AiGeneratedClip>, String> {
    if config.provider == AiProvider::Offline {
        return Err("AI-provider är satt till Offline".to_string());
    }
    if config.provider.requires_key() && config.api_key.trim().is_empty() {
        return Err("Ingen API-nyckel angiven".to_string());
    }

    let is_anthropic = config.provider == AiProvider::Anthropic;
    let url = if is_anthropic {
        format!("{}/messages", config.resolved_base().trim_end_matches('/'))
    } else {
        format!("{}/chat/completions", config.resolved_base().trim_end_matches('/'))
    };
    let body = if is_anthropic {
        serde_json::json!({
            "model": config.resolved_model(),
            "max_tokens": config.max_tokens.max(1),
            "system": SYSTEM_PROMPT,
            "temperature": config.temperature,
            "messages": [{"role": "user", "content": user_prompt}],
        })
    } else {
        serde_json::json!({
            "model": config.resolved_model(),
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": user_prompt},
            ],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": false,
        })
    };

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(config.timeout_secs.clamp(5, 300)))
        .build();

    let mut req = agent.post(&url).set("Content-Type", "application/json");
    if is_anthropic {
        req = req
            .set("x-api-key", config.api_key.trim())
            .set("anthropic-version", "2023-06-01");
    } else if !config.api_key.trim().is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", config.api_key.trim()));
    }
    if config.provider == AiProvider::OpenRouter {
        req = req
            .set("HTTP-Referer", "https://github.com/sonix-daw")
            .set("X-Title", "Sonix");
    }

    let response = match req.send_string(&body.to_string()) {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            return Err(format!("AI-API svarade {}: {}", code, truncate(&text, 240)));
        }
        Err(e) => return Err(format!("Nätverksfel: {e}")),
    };

    let text = response
        .into_string()
        .map_err(|e| format!("Kunde inte läsa AI-svaret: {e}"))?;
    let content = extract_content(&text)?;
    parse_clips(&content)
}

fn build_agent(config: &AiConfig) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(config.timeout_secs.clamp(5, 300)))
        .build()
}

fn send_request(result: Result<ureq::Response, ureq::Error>) -> Result<ureq::Response, String> {
    match result {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            Err(format!("AI-API svarade {}: {}", code, truncate(&text, 240)))
        }
        Err(e) => Err(format!("Nätverksfel: {e}")),
    }
}

fn read_audio_response(response: ureq::Response) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("Kunde inte läsa AI-ljudet: {e}"))?;
    if buf.is_empty() {
        return Err("AI-svaret innehöll inget ljud".to_string());
    }
    Ok(buf)
}

fn push_multipart_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn build_multipart(boundary: &str, fields: &[(&str, String)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        push_multipart_field(&mut body, boundary, name, value);
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Generates rendered audio (WAV/MP3 bytes) from a text prompt using the
/// configured audio backend. Returns raw bytes ready to be decoded/imported.
pub fn generate_audio(config: &AiConfig, prompt: &str, duration_secs: f32) -> Result<Vec<u8>, String> {
    match config.audio_provider {
        AudioProvider::Offline => Err("Ljudgenerering är satt till Offline".to_string()),
        AudioProvider::OpenAiTts => generate_openai_tts(config, prompt),
        AudioProvider::Stability => generate_stability(config, prompt, duration_secs),
    }
}

fn generate_openai_tts(config: &AiConfig, prompt: &str) -> Result<Vec<u8>, String> {
    let key = config.resolved_audio_key();
    if key.is_empty() {
        return Err("Ingen API-nyckel angiven".to_string());
    }
    let url = format!("{}/audio/speech", config.resolved_audio_base().trim_end_matches('/'));
    let body = serde_json::json!({
        "model": config.resolved_audio_model(),
        "input": prompt,
        "voice": "alloy",
        "response_format": "mp3",
    });
    let req = build_agent(config)
        .post(&url)
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {key}"));
    read_audio_response(send_request(req.send_string(&body.to_string()))?)
}

fn generate_stability(config: &AiConfig, prompt: &str, duration_secs: f32) -> Result<Vec<u8>, String> {
    let key = config.resolved_audio_key();
    if key.is_empty() {
        return Err("Ingen API-nyckel angiven".to_string());
    }
    let url = format!(
        "{}/v2beta/audio/{}/text-to-audio",
        config.resolved_audio_base().trim_end_matches('/'),
        config.resolved_audio_model()
    );
    let boundary = "----sonixAudioBoundary7MA4YWxkTrZu0gW";
    let fields = [
        ("prompt", prompt.to_string()),
        ("duration", format!("{:.0}", duration_secs.clamp(1.0, 190.0))),
        ("output_format", "mp3".to_string()),
    ];
    let body = build_multipart(boundary, &fields);
    let req = build_agent(config)
        .post(&url)
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .set("Accept", "audio/*")
        .set("Authorization", &format!("Bearer {key}"));
    read_audio_response(send_request(req.send_bytes(&body))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_style_response() {
        let response = r#"{
            "choices": [{
                "message": {
                    "content": "[{\"title\":\"T\",\"genre\":\"Trap\",\"key_signature\":\"A Minor\",\"bpm\":140,\"steps\":[true,false,false,false,true,false,false,false,true,false,false,false,true,false,false,false],\"notes\":[45,45,45,45,45,45,45,45,45,45,45,45,45,45,45,45]}]"
                }
            }]
        }"#;
        let content = extract_content(response).unwrap();
        let clips = parse_clips(&content).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].notes[0], 45);
        assert_eq!(clips[0].genre, "Trap");
    }

    #[test]
    fn parses_anthropic_style_response() {
        let response = r#"{
            "content": [{"type": "text", "text": "[{\"title\":\"T\",\"genre\":\"House\",\"key_signature\":\"C Minor\",\"bpm\":124,\"steps\":[true,false,false,false,true,false,false,false,true,false,false,false,true,false,false,false],\"notes\":[48,48,48,48,48,48,48,48,48,48,48,48,48,48,48,48]}]"}]
        }"#;
        let content = extract_content(response).unwrap();
        let clips = parse_clips(&content).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].genre, "House");
    }

    #[test]
    fn provider_codes_and_models_are_consistent() {
        assert_eq!(AiProvider::from_code("claude"), Some(AiProvider::Anthropic));
        for p in AiProvider::ALL {
            assert!(!p.label().is_empty());
            if p != AiProvider::Offline {
                assert!(!p.default_base().is_empty());
                assert!(!p.default_model().is_empty());
                assert!(!p.known_models().is_empty());
                assert!(p.known_models().contains(&p.default_model()));
            }
        }
    }

    #[test]
    fn strips_markdown_fences() {
        let content = "```json\n[{\"steps\":[true,false,false,false,true,false,false,false,true,false,false,false,true,false,false,false],\"notes\":[60,60,60,60,60,60,60,60,60,60,60,60,60,60,60,60]}]\n```";
        let clips = parse_clips(content).unwrap();
        assert_eq!(clips.len(), 1);
    }

    #[test]
    fn rejects_wrong_step_count() {
        let content = "[{\"steps\":[true,false],\"notes\":[60,60]}]";
        assert!(parse_clips(content).is_err());
    }

    #[test]
    fn offline_config_is_not_ready() {
        let cfg = AiConfig::default();
        assert!(!cfg.is_ready());
        assert!(generate_clips(&cfg, "test").is_err());
        assert!(!cfg.audio_is_ready());
        assert!(generate_audio(&cfg, "test", 5.0).is_err());
    }

    #[test]
    fn audio_provider_defaults_and_codes() {
        assert_eq!(AudioProvider::from_code("tts"), Some(AudioProvider::OpenAiTts));
        assert_eq!(AudioProvider::from_code("stable-audio"), Some(AudioProvider::Stability));
        for p in AudioProvider::ALL {
            assert!(!p.label().is_empty());
            if p != AudioProvider::Offline {
                assert!(!p.default_base().is_empty());
                assert!(!p.default_model().is_empty());
            }
        }
    }

    #[test]
    fn multipart_body_is_well_formed() {
        let body = build_multipart(
            "BOUND",
            &[("prompt", "hello".to_string()), ("duration", "30".to_string())],
        );
        let text = String::from_utf8(body).unwrap();
        assert!(text.starts_with("--BOUND\r\n"));
        assert!(text.contains("name=\"prompt\""));
        assert!(text.contains("hello"));
        assert!(text.ends_with("--BOUND--\r\n"));
    }
}
