use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use warp::Filter;
use warp::http::Response;
use warp::sse::Event;

const OLLAMA_DEFAULT_URL: &str = "http://localhost:11434";

// ---------------------------------------------------------------------------
// Application state (read-only after startup, so we share it with plain Arc)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    config: HashMap<String, String>,
    grammar_topics: Vec<GrammarTopic>,
    tutor_template: String,
    ollama_url: String,
    model: String,
}

#[derive(Clone, serde::Serialize)]
struct GrammarTopic {
    title: String,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let grammar_topics = load_grammar_topics().unwrap_or_default();
    let tutor_template = std::fs::read_to_string("tutor.md").unwrap_or_default();

    let model = config
        .get("model")
        .cloned()
        .unwrap_or_else(|| "gemma-4:31".to_string());
    let ollama_url = config
        .get("OLLAMA_URL")
        .cloned()
        .unwrap_or_else(|| OLLAMA_DEFAULT_URL.to_string());

    // Make sure the dialogues directory exists.
    let _ = std::fs::create_dir_all("dialogues");

    let state = Arc::new(AppState {
        config,
        grammar_topics,
        tutor_template,
        ollama_url,
        model,
    });

    let routes = api_routes(state.clone()).or(static_routes());

    println!("Tutor is listening on http://127.0.0.1:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// Loading config / grammar / template
// ---------------------------------------------------------------------------

fn load_config() -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string("config.yml")?;
    let parsed: HashMap<String, String> = serde_yaml::from_str(&contents)?;
    Ok(parsed)
}

/// Parse `grammar/french.md`: every line starting with `#` is a topic title,
/// the following non-heading lines form its description.
fn load_grammar_topics() -> Result<Vec<GrammarTopic>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string("grammar/french.md")?;
    let mut topics: Vec<GrammarTopic> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let title = trimmed.trim_start_matches('#').trim().to_string();
            if !title.is_empty() {
                topics.push(GrammarTopic {
                    title,
                    description: String::new(),
                });
            }
        } else if !trimmed.is_empty() {
            if let Some(last) = topics.last_mut() {
                if !last.description.is_empty() {
                    last.description.push(' ');
                }
                last.description.push_str(trimmed);
            }
        }
    }

    Ok(topics)
}

/// Replace `{{KEY}}` placeholders in a template with values from `vars`.
fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{}}}}}", key), value);
    }
    out
}

// ---------------------------------------------------------------------------
// Request payloads
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    topic: String,
    #[serde(default)]
    grammar: String,
    #[serde(default, rename = "skillLevel")]
    skill_level: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct DialogueRequest {
    #[serde(default)]
    topic: String,
    #[serde(default)]
    grammar: String,
    #[serde(default, rename = "skillLevel")]
    skill_level: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct TranslateRequest {
    text: String,
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

fn api_routes(
    state: Arc<AppState>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let get_grammar = warp::get()
        .and(warp::path!("api" / "grammar"))
        .and(with_state(state.clone()))
        .map(|state: Arc<AppState>| {
            let titles: Vec<&String> = state.grammar_topics.iter().map(|t| &t.title).collect();
            warp::reply::json(&json!({ "topics": titles }))
        });

    let get_config = warp::get()
        .and(warp::path!("api" / "config"))
        .and(with_state(state.clone()))
        .map(|state: Arc<AppState>| warp::reply::json(&json!({ "config": state.config })));

    // POST /api/chat -> streaming (SSE) reply from Ollama.
    let post_chat = warp::post()
        .and(warp::path!("api" / "chat"))
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .map(|state: Arc<AppState>, req: ChatRequest| {
            let stream = chat_stream(state, req);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    // POST /api/translate -> streaming (SSE) translation from Ollama.
    let post_translate = warp::post()
        .and(warp::path!("api" / "translate"))
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .map(|state: Arc<AppState>, req: TranslateRequest| {
            let stream = translate_stream(state, req);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    // POST /api/dialogue -> save transcript to dialogues/.
    let post_dialogue = warp::post()
        .and(warp::path!("api" / "dialogue"))
        .and(warp::body::json())
        .map(|req: DialogueRequest| match save_dialogue(&req) {
            Ok(path) => warp::reply::json(&json!({ "status": "success", "path": path })),
            Err(e) => warp::reply::json(&json!({ "status": "error", "message": e.to_string() })),
        });

    get_grammar
        .or(get_config)
        .or(post_chat)
        .or(post_translate)
        .or(post_dialogue)
}

fn static_routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let index_html = warp::get().and(warp::path::end()).map(|| {
        Response::builder()
            .header("content-type", "text/html; charset=utf-8")
            .body(include_str!("../index.html"))
    });

    let css_file = warp::get().and(warp::path!("style.css")).map(|| {
        Response::builder()
            .header("content-type", "text/css; charset=utf-8")
            .body(include_str!("../style.css"))
    });

    let app_js = warp::get().and(warp::path!("app.js")).map(|| {
        Response::builder()
            .header("content-type", "application/javascript; charset=utf-8")
            .body(include_str!("../app.js"))
    });

    index_html.or(css_file).or(app_js)
}

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

// ---------------------------------------------------------------------------
// Ollama chat streaming
// ---------------------------------------------------------------------------

/// Build the system primer from tutor.md + config + the request parameters.
fn build_system_prompt(state: &AppState, req: &ChatRequest) -> String {
    let mut vars = state.config.clone();
    vars.insert("TOPIC".to_string(), req.topic.clone());
    vars.insert("SKILL_LEVEL".to_string(), req.skill_level.clone());

    // Expand the grammar focus with its description (if we have one on file).
    let grammar_detail = state
        .grammar_topics
        .iter()
        .find(|t| t.title == req.grammar)
        .map(|t| {
            if t.description.is_empty() {
                t.title.clone()
            } else {
                format!("{} — {}", t.title, t.description)
            }
        })
        .unwrap_or_else(|| req.grammar.clone());
    vars.insert("GRAMMAR_FOCUS".to_string(), grammar_detail);

    render_template(&state.tutor_template, &vars)
}

/// Returns an SSE stream that proxies Ollama's streamed chat response.
fn chat_stream(
    state: Arc<AppState>,
    req: ChatRequest,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    let system = build_system_prompt(&state, &req);

    // Assemble the message list Ollama expects: system primer first, then history.
    let mut messages: Vec<Value> = vec![json!({ "role": "system", "content": system })];
    for m in &req.messages {
        messages.push(json!({ "role": m.role, "content": m.content }));
    }

    let body = json!({
        "model": state.model,
        "messages": messages,
        "stream": true,
    });
    let url = format!("{}/api/chat", state.ollama_url);

    ollama_sse(url, body)
}

/// Returns an SSE stream that translates `req.text` from the configured
/// explanation language into the learning language, reusing the chat model.
fn translate_stream(
    state: Arc<AppState>,
    req: TranslateRequest,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    let from = state
        .config
        .get("EXPLANATION_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "German".to_string());
    let to = state
        .config
        .get("LEARNING_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "French".to_string());

    let system = format!(
        "You are a translation engine. Translate the user's text from {from} to {to}. \
         Output only the translation in {to}, with no explanations, notes, or quotation marks."
    );

    let messages: Vec<Value> = vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": req.text }),
    ];

    let body = json!({
        "model": state.model,
        "messages": messages,
        "stream": true,
    });
    let url = format!("{}/api/chat", state.ollama_url);

    ollama_sse(url, body)
}
fn ollama_sse(
    url: String,
    body: Value,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    // Channel bridges the spawned request task to the SSE response stream.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, Infallible>>();

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let resp = match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Ok(Event::default()
                    .event("error")
                    .data(format!("Ollama nicht erreichbar: {}", e))));
                return;
            }
        };

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(Ok(Event::default()
                        .event("error")
                        .data(format!("Stream-Fehler: {}", e))));
                    break;
                }
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            // Ollama emits newline-delimited JSON objects.
            while let Some(pos) = buf.find('\n') {
                let line: String = buf.drain(..=pos).collect();
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    if let Some(content) = v
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !content.is_empty() {
                            // Encode as JSON so leading/trailing spaces survive SSE
                            // (the SSE spec strips one leading space after `data:`).
                            let payload = json!({ "t": content }).to_string();
                            let _ = tx.send(Ok(Event::default().data(payload)));
                        }
                    }
                    if v.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                        let _ = tx.send(Ok(Event::default().event("done").data("")));
                        return;
                    }
                }
            }
        }

        let _ = tx.send(Ok(Event::default().event("done").data("")));
    });

    tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}

// ---------------------------------------------------------------------------
// Dialogue persistence
// ---------------------------------------------------------------------------

fn slugify(input: &str) -> String {
    let mut s: String = input
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

fn save_dialogue(req: &DialogueRequest) -> Result<String, Box<dyn std::error::Error>> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = format!(
        "dialogues/{}-{}-{}-{}.md",
        slugify(&req.topic),
        slugify(&req.grammar),
        slugify(&req.skill_level),
        ts
    );

    let mut body = String::new();
    body.push_str(&format!("# {}\n\n", req.topic));
    body.push_str(&format!(
        "- Grammaire: {}\n- Niveau: {}\n\n---\n\n",
        req.grammar, req.skill_level
    ));
    for m in &req.messages {
        let who = if m.role == "user" {
            "**Thorsten**"
        } else {
            "**Amelie**"
        };
        body.push_str(&format!("{}: {}\n\n", who, m.content));
    }

    std::fs::write(&filename, body)?;
    Ok(filename)
}
