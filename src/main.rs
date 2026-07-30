use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
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
    /// Directory that all runtime files/folders (config, grammar, tutor
    /// template, dialogues, vocab) are resolved against.
    base_dir: PathBuf,
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
    /// CEFR skill level this topic belongs to (e.g. "A1", "C1").
    skill_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = parse_base_dir(std::env::args().skip(1))?;

    let config = load_config(&base_dir)?;
    let grammar_topics = load_grammar_topics(&base_dir).unwrap_or_default();
    let tutor_template = std::fs::read_to_string(base_dir.join("tutor.md")).unwrap_or_default();

    let model = config
        .get("model")
        .cloned()
        .unwrap_or_else(|| "gemma-4:31".to_string());
    let ollama_url = config
        .get("OLLAMA_URL")
        .cloned()
        .unwrap_or_else(|| OLLAMA_DEFAULT_URL.to_string());

    // Make sure the dialogues directory exists.
    let _ = std::fs::create_dir_all(base_dir.join("dialogues"));

    let state = Arc::new(AppState {
        base_dir,
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

/// Determine the base directory that all runtime files/folders are resolved
/// against. Accepts either a positional path or `--dir <path>` / `-d <path>`;
/// defaults to `.` (the current working directory) when no argument is given.
fn parse_base_dir(
    args: impl Iterator<Item = String>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut base: Option<String> = None;
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" | "-d" => {
                let value = args
                    .next()
                    .ok_or("--dir requires a path argument")?;
                base = Some(value);
            }
            other if other.starts_with("--dir=") => {
                base = Some(other["--dir=".len()..].to_string());
            }
            // Bare positional path.
            other if !other.starts_with('-') => {
                base = Some(other.to_string());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let dir = PathBuf::from(base.unwrap_or_else(|| ".".to_string()));
    if !dir.is_dir() {
        return Err(format!("base directory does not exist: {}", dir.display()).into());
    }
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Loading config / grammar / template
// ---------------------------------------------------------------------------

fn load_config(base_dir: &Path) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(base_dir.join("config.yml"))?;
    let parsed: HashMap<String, String> = serde_yaml::from_str(&contents)?;
    Ok(parsed)
}

/// Parse `grammar/french.md` in the two-level format:
///   `#`  headings set the current CEFR skill level (e.g. `# A1`),
///   `##` headings define a grammar topic under that level,
///   the following non-heading lines form the topic's description.
///
/// A `##` topic seen before any `#` level is filed under an empty level.
fn load_grammar_topics(
    base_dir: &Path,
) -> Result<Vec<GrammarTopic>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(base_dir.join("grammar/french.md"))?;
    let mut topics: Vec<GrammarTopic> = Vec::new();
    let mut current_level = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("##") {
            // Grammar topic under the current skill level.
            let title = rest.trim_start_matches('#').trim().to_string();
            if !title.is_empty() {
                topics.push(GrammarTopic {
                    title,
                    description: String::new(),
                    skill_level: current_level.clone(),
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix('#') {
            // Skill-level heading.
            let level = rest.trim().to_string();
            if !level.is_empty() {
                current_level = level;
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
    /// Direction of translation. When true, translate from the learning
    /// language back into the explanation language (the reverse of default).
    #[serde(default)]
    reverse: bool,
}

#[derive(Deserialize)]
struct VocabRequest {
    /// Word/phrase in the explanation language.
    explanation: String,
    /// Word/phrase in the learning language.
    learning: String,
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
            // Group topic titles by skill level, in fixed CEFR order.
            const CEFR_ORDER: [&str; 6] = ["A1", "A2", "B1", "B2", "C1", "C2"];
            let mut by_level: HashMap<String, Vec<&String>> = HashMap::new();
            for t in &state.grammar_topics {
                by_level.entry(t.skill_level.clone()).or_default().push(&t.title);
            }

            // Emit levels in CEFR order first, then any unrecognised levels
            // in the order they were encountered.
            let mut levels: Vec<Value> = Vec::new();
            let mut seen: Vec<String> = Vec::new();
            for lvl in CEFR_ORDER {
                if let Some(titles) = by_level.get(lvl) {
                    levels.push(json!({ "level": lvl, "topics": titles }));
                    seen.push(lvl.to_string());
                }
            }
            for t in &state.grammar_topics {
                if !seen.contains(&t.skill_level) && !t.skill_level.is_empty() {
                    let titles = &by_level[&t.skill_level];
                    levels.push(json!({ "level": t.skill_level, "topics": titles }));
                    seen.push(t.skill_level.clone());
                }
            }

            // Flat list kept for backward compatibility with older clients.
            let titles: Vec<&String> = state.grammar_topics.iter().map(|t| &t.title).collect();
            warp::reply::json(&json!({ "topics": titles, "levels": levels }))
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
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .map(
            |state: Arc<AppState>, req: DialogueRequest| match save_dialogue(&state, &req) {
                Ok(path) => warp::reply::json(&json!({ "status": "success", "path": path })),
                Err(e) => warp::reply::json(&json!({ "status": "error", "message": e.to_string() })),
            },
        );

    // POST /api/vocab -> append a word pair to vocab/<EXPL>_<LEARN>.csv.
    let post_vocab = warp::post()
        .and(warp::path!("api" / "vocab"))
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .map(
            |state: Arc<AppState>, req: VocabRequest| match save_vocab(&state, &req) {
                Ok(path) => warp::reply::json(&json!({ "status": "success", "path": path })),
                Err(e) => {
                    warp::reply::json(&json!({ "status": "error", "message": e.to_string() }))
                }
            },
        );

    get_grammar
        .or(get_config)
        .or(post_chat)
        .or(post_translate)
        .or(post_dialogue)
        .or(post_vocab)
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

    // Topic is optional: when the user leaves it empty, instruct the tutor to
    // choose a suitable topic itself instead of adhering to a fixed one.
    let topic = if req.topic.trim().is_empty() {
        "choisis toi-même un sujet adapté au niveau et au point de grammaire, \
         puis annonce-le au début de la conversation"
            .to_string()
    } else {
        req.topic.clone()
    };
    vars.insert("TOPIC".to_string(), topic);
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
    let explanation = state
        .config
        .get("EXPLANATION_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "German".to_string());
    let learning = state
        .config
        .get("LEARNING_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "French".to_string());

    // Default direction is explanation -> learning; `reverse` flips it.
    let (from, to) = if req.reverse {
        (learning, explanation)
    } else {
        (explanation, learning)
    };

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

fn save_dialogue(
    state: &AppState,
    req: &DialogueRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = state.base_dir.join(format!(
        "dialogues/{}-{}-{}-{}.md",
        slugify(&req.topic),
        slugify(&req.grammar),
        slugify(&req.skill_level),
        ts
    ));

    let mut body = String::new();
    let heading = if req.topic.trim().is_empty() {
        "Sujet libre"
    } else {
        &req.topic
    };
    body.push_str(&format!("# {}\n\n", heading));
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
    Ok(filename.display().to_string())
}

// ---------------------------------------------------------------------------
// Vocabulary persistence
// ---------------------------------------------------------------------------

/// Append a word pair to `vocab/<EXPLANATION_LANGUAGE>_<LEARNING_LANGUAGE>.csv`.
///
/// The file uses `:` as the field separator, with a header line
/// `<EXPLANATION_LANGUAGE>:<LEARNING_LANGUAGE>`. The header is written once,
/// when the file is first created.
fn save_vocab(state: &AppState, req: &VocabRequest) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::Write;

    let explanation = state
        .config
        .get("EXPLANATION_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "German".to_string());
    let learning = state
        .config
        .get("LEARNING_LANGUAGE")
        .cloned()
        .unwrap_or_else(|| "French".to_string());

    let expl = req.explanation.trim();
    let learn = req.learning.trim();
    if expl.is_empty() || learn.is_empty() {
        return Err("both words are required".into());
    }

    std::fs::create_dir_all(state.base_dir.join("vocab"))?;
    let filename = state
        .base_dir
        .join(format!("vocab/{}_{}.csv", explanation, learning));

    // A `:` in a field would corrupt the column split; guard against it by
    // rejecting the pair rather than silently writing a broken row.
    if expl.contains(':') || learn.contains(':') {
        return Err("words must not contain ':'".into());
    }

    let need_header = !filename.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;

    if need_header {
        writeln!(file, "{}:{}", explanation, learning)?;
    }
    writeln!(file, "{}:{}", expl, learn)?;

    Ok(filename.display().to_string())
}
