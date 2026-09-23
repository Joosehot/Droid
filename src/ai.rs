//! LLM client: the AI writes the whole game (spec + script) as one JSON document.
//! Backends: Claude Messages API (raw HTTP, streaming) or any OpenAI-compatible
//! local server such as llama.cpp.

use crate::script::ScriptHost;
use crate::spec::GameSpec;
use anyhow::{anyhow, bail, Context};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

const ENGINE_API: &str = include_str!("../docs/ENGINE_API.md");

const SYSTEM: &str = "You are the game designer, artist and programmer inside AIGE, an AI-native game engine. \
You produce complete, playable games as a single JSON document that the engine runs directly.\n\n\
Output rules:\n\
- Respond with ONLY the JSON document. No markdown fences, no commentary before or after.\n\
- The `script` field must be valid Rhai using only the API documented below.\n\
- Make the game fun and finished: clear goal, feedback text, difficulty ramp, restart.\n\
- Build visuals from coloured primitives with good contrast against the background.\n\n\
ENGINE REFERENCE:\n\n";

#[derive(Clone, Debug)]
pub enum Backend {
    Claude { api_key: String, model: String },
    OpenAiCompat { base_url: String, model: String },
}

pub struct Client {
    backend: Backend,
    http: reqwest::blocking::Client,
    pub verbose: bool,
}

#[derive(Clone, Debug)]
pub struct Msg {
    pub role: &'static str,
    pub content: String,
}

impl Client {
    /// Picks a backend from the environment:
    /// - `AIGE_BACKEND=local` (or no ANTHROPIC_API_KEY) -> OpenAI-compatible at `AIGE_LOCAL_URL` (default http://localhost:8080/v1)
    /// - otherwise Claude with `ANTHROPIC_API_KEY`, model `AIGE_MODEL` (default claude-opus-5)
    pub fn from_env() -> anyhow::Result<Client> {
        let backend_env = std::env::var("AIGE_BACKEND").unwrap_or_default().to_ascii_lowercase();
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty());
        let backend = if backend_env == "local" || (backend_env.is_empty() && api_key.is_none()) {
            Backend::OpenAiCompat {
                base_url: std::env::var("AIGE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080/v1".into()),
                model: std::env::var("AIGE_MODEL").unwrap_or_else(|_| "default".into()),
            }
        } else {
            Backend::Claude {
                api_key: api_key.ok_or_else(|| anyhow!("ANTHROPIC_API_KEY not set (or set AIGE_BACKEND=local)"))?,
                model: std::env::var("AIGE_MODEL").unwrap_or_else(|_| "claude-opus-5".into()),
            }
        };
        let http = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(1800)).build()?;
        Ok(Client { backend, http, verbose: true })
    }

    pub fn describe(&self) -> String {
        match &self.backend {
            Backend::Claude { model, .. } => format!("Claude ({model})"),
            Backend::OpenAiCompat { base_url, model } => format!("local {model} @ {base_url}"),
        }
    }

    fn system_prompt() -> String {
        format!("{SYSTEM}{ENGINE_API}")
    }

    /// Generate a brand-new game from a description.
    pub fn new_game(&self, prompt: &str) -> anyhow::Result<GameSpec> {
        let user = format!("Create a complete game from this description:\n\n{prompt}\n\nReturn the full game JSON.");
        self.generate_valid(vec![Msg { role: "user", content: user }])
    }

    /// Modify an existing game according to a request. `runtime_error` is the last
    /// script/engine error, if any, so the AI can fix it.
    pub fn edit_game(&self, current: &GameSpec, request: &str, runtime_error: Option<&str>) -> anyhow::Result<GameSpec> {
        let mut user = format!("Here is the current game JSON:\n\n{}\n\n", current.to_json());
        if let Some(e) = runtime_error {
            user.push_str(&format!("The engine reported this error while running it:\n{e}\n\n"));
        }
        user.push_str(&format!("Change request: {request}\n\nApply the change (and fix any error) and return the FULL updated game JSON."));
        self.generate_valid(vec![Msg { role: "user", content: user }])
    }

    /// Ask, parse, validate; feed errors back to the model up to 3 times.
    fn generate_valid(&self, mut msgs: Vec<Msg>) -> anyhow::Result<GameSpec> {
        let system = Self::system_prompt();
        for attempt in 1..=4 {
            let raw = self.complete(&system, &msgs)?;
            match parse_game(&raw) {
                Ok(spec) => return Ok(spec),
                Err(e) => {
                    if attempt == 4 {
                        bail!("AI could not produce a valid game after {attempt} attempts: {e}");
                    }
                    if self.verbose {
                        eprintln!("[aige] attempt {attempt} invalid ({e}); asking the AI to fix it");
                    }
                    msgs.push(Msg { role: "assistant", content: raw });
                    msgs.push(Msg {
                        role: "user",
                        content: format!("That was not valid. Error:\n{e}\n\nReturn the corrected FULL game JSON only."),
                    });
                }
            }
        }
        unreachable!()
    }

    fn complete(&self, system: &str, msgs: &[Msg]) -> anyhow::Result<String> {
        match &self.backend {
            Backend::Claude { api_key, model } => self.claude(api_key, model, system, msgs),
            Backend::OpenAiCompat { base_url, model } => self.openai_compat(base_url, model, system, msgs),
        }
    }

    fn claude(&self, api_key: &str, model: &str, system: &str, msgs: &[Msg]) -> anyhow::Result<String> {
        let body = json!({
            "model": model,
            "max_tokens": 32000,
            "stream": true,
            "fallbacks": "default",
            "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
            "messages": msgs.iter().map(|m| json!({ "role": m.role, "content": m.content })).collect::<Vec<_>>(),
        });
        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "server-side-fallback-2026-07-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .context("request to Claude API failed")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            bail!("Claude API error {status}: {text}");
        }
        let mut out = String::new();
        let mut stop_reason = String::new();
        let reader = BufReader::new(resp);
        let mut stderr = std::io::stderr();
        for line in reader.lines() {
            let line = line?;
            let Some(data) = line.strip_prefix("data: ") else { continue };
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            match v["type"].as_str().unwrap_or("") {
                "content_block_delta" => {
                    if let Some(t) = v["delta"]["text"].as_str() {
                        out.push_str(t);
                        if self.verbose {
                            let _ = write!(stderr, ".");
                            let _ = stderr.flush();
                        }
                    }
                }
                "message_delta" => {
                    if let Some(s) = v["delta"]["stop_reason"].as_str() {
                        stop_reason = s.to_string();
                    }
                }
                "error" => bail!("Claude stream error: {}", v["error"]),
                _ => {}
            }
        }
        if self.verbose {
            let _ = writeln!(stderr);
        }
        match stop_reason.as_str() {
            "refusal" => bail!("the model declined this request (stop_reason=refusal)"),
            "max_tokens" => bail!("the model ran out of output tokens; try a simpler game"),
            _ => {}
        }
        Ok(out)
    }

    fn openai_compat(&self, base_url: &str, model: &str, system: &str, msgs: &[Msg]) -> anyhow::Result<String> {
        let mut messages = vec![json!({ "role": "system", "content": system })];
        messages.extend(msgs.iter().map(|m| json!({ "role": m.role, "content": m.content })));
        let body = json!({ "model": model, "messages": messages, "stream": false, "max_tokens": 16000 });
        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let resp = self.http.post(&url).json(&body).send().with_context(|| format!("request to {url} failed"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            bail!("local LLM error {status}: {}", resp.text().unwrap_or_default());
        }
        let v: Value = resp.json()?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("unexpected response: {v}"))
    }
}

/// Extract the JSON object from a model reply (tolerates fences / prose) and validate it.
pub fn parse_game(raw: &str) -> anyhow::Result<GameSpec> {
    let start = raw.find('{').ok_or_else(|| anyhow!("no JSON object found in reply"))?;
    let end = raw.rfind('}').ok_or_else(|| anyhow!("no closing brace in reply"))?;
    if end <= start {
        bail!("malformed JSON in reply");
    }
    let json = &raw[start..=end];
    let spec = GameSpec::from_json(json).map_err(|e| anyhow!("JSON does not match the game format: {e}"))?;
    if spec.entities.is_empty() {
        bail!("the game has no entities");
    }
    ScriptHost::check(&spec.script).map_err(|e| anyhow!("Rhai script error: {e}"))?;
    Ok(spec)
}
