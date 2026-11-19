//! Models on this machine. Code is sent only to a loopback address: a remote URL is refused,
//! sending a repository to a service is a decision for the person running it, and the host's own
//! model (sampling) is the way to use a hosted one. Plain HTTP only: there is no TLS to loopback.

use std::time::Duration;

use serde_json::{Value, json};

use super::types::{CompletionRequest, SemanticProvider};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalRuntime {
    Ollama,
    LmStudio,
}

impl LocalRuntime {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lmstudio",
        }
    }

    const fn display(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::LmStudio => "LM Studio",
        }
    }

    const fn default_url(self) -> &'static str {
        match self {
            Self::Ollama => "http://localhost:11434",
            Self::LmStudio => "http://localhost:1234",
        }
    }
}

/// The origin (`http://host:port`) of a URL that names this machine.
///
/// # Errors
/// The text is not an `http` URL, or its host is not `localhost`, `127.0.0.1` or `[::1]`.
pub fn assert_loopback(url: &str) -> Result<String, String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        if url.starts_with("https://") {
            "Only http is supported to a model on this machine.".to_owned()
        } else {
            format!(
                "\"{}\" is not an http URL.",
                url.chars().take(60).collect::<String>()
            )
        }
    })?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        format!("[{}]", bracketed.split_once(']').map_or("", |(h, _)| h))
    } else {
        authority.split(':').next().unwrap_or("").to_owned()
    };
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
        Ok(format!("http://{authority}"))
    } else {
        Err(format!(
            "Refusing {host}: the semantic layer sends code only to a model on this machine (localhost)."
        ))
    }
}

pub struct LocalProvider {
    runtime: LocalRuntime,
    base: String,
    model: String,
    agent: ureq::Agent,
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // A loopback server that answers with a redirect must not send the code elsewhere, and a
        // proxy in the environment must not carry loopback traffic out of the machine.
        .max_redirects(0)
        .proxy(None)
        .http_status_as_error(false)
        .build()
        .into()
}

fn get_json(agent: &ureq::Agent, url: &str) -> Result<Value, String> {
    let mut response = agent.get(url).call().map_err(|_| "no answer".to_owned())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|_| "unreadable answer".to_owned())?;
    serde_json::from_str(&text).map_err(|_| "answer is not JSON".to_owned())
}

/// The first model the runtime has.
fn discover_model(
    agent: &ureq::Agent,
    runtime: LocalRuntime,
    base: &str,
) -> Result<String, String> {
    let (path, pointer) = match runtime {
        LocalRuntime::Ollama => ("/api/tags", "/models/0/name"),
        LocalRuntime::LmStudio => ("/v1/models", "/data/0/id"),
    };
    let listing = get_json(agent, &format!("{base}{path}"))
        .map_err(|why| format!("{} is not answering at {base} ({why}).", runtime.display()))?;
    listing
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| match runtime {
            LocalRuntime::Ollama => {
                "Ollama has no models; pull one (`ollama pull qwen2.5-coder:7b`).".to_owned()
            }
            LocalRuntime::LmStudio => "LM Studio has no model loaded.".to_owned(),
        })
}

/// Connects to a runtime on this machine and picks its model: the one asked for, or the first it has.
///
/// # Errors
/// The URL is not loopback, the runtime does not answer, or it has no model.
pub fn connect_local(
    runtime: LocalRuntime,
    base_url: Option<&str>,
    model: Option<&str>,
    timeout: Option<Duration>,
) -> Result<LocalProvider, String> {
    let base = assert_loopback(base_url.unwrap_or(runtime.default_url()))?;
    let agent = agent(timeout.unwrap_or(DEFAULT_TIMEOUT));
    let model = match model {
        Some(model) => model.to_owned(),
        None => discover_model(&agent, runtime, &base)?,
    };
    Ok(LocalProvider {
        runtime,
        base,
        model,
        agent,
    })
}

impl LocalProvider {
    fn request_body(&self, request: &CompletionRequest) -> (String, Value) {
        match self.runtime {
            LocalRuntime::Ollama => (
                format!("{}/api/generate", self.base),
                json!({
                    "model": self.model,
                    "prompt": request.user,
                    "system": request.system,
                    "stream": false,
                    "format": "json",
                    "options": { "temperature": 0, "num_predict": request.max_tokens },
                }),
            ),
            LocalRuntime::LmStudio => (
                format!("{}/v1/chat/completions", self.base),
                json!({
                    "model": self.model,
                    "messages": [
                        { "role": "system", "content": request.system },
                        { "role": "user", "content": request.user },
                    ],
                    "stream": false,
                    "temperature": 0,
                    "max_tokens": request.max_tokens,
                }),
            ),
        }
    }
}

impl SemanticProvider for LocalProvider {
    fn name(&self) -> &str {
        self.runtime.id()
    }

    fn model(&self) -> Option<String> {
        Some(self.model.clone())
    }

    fn complete(&self, request: &CompletionRequest) -> Result<String, String> {
        let name = self.runtime.display();
        let (url, body) = self.request_body(request);
        let mut response = self
            .agent
            .post(&url)
            .header("content-type", "application/json")
            .send(body.to_string())
            .map_err(|_| format!("{name} took too long or did not answer."))?;
        if !response.status().is_success() {
            return Err(format!(
                "{name} request failed with HTTP {}",
                response.status().as_u16()
            ));
        }
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|_| format!("{name} took too long or did not answer."))?;
        let answer: Value = serde_json::from_str(&text)
            .map_err(|_| format!("{name} answered with something that is not JSON."))?;
        if let Some(error) = answer["error"]
            .as_str()
            .or_else(|| answer["error"]["message"].as_str())
        {
            return Err(error.chars().take(200).collect());
        }
        let content = match self.runtime {
            LocalRuntime::Ollama => answer["response"].as_str(),
            LocalRuntime::LmStudio => answer["choices"][0]["message"]["content"].as_str(),
        };
        content
            .filter(|c| !c.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} returned nothing."))
    }
}
