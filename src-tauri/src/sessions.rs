//! Read-only view of omp agent session transcripts (`*.jsonl` in each agent
//! terminal's `--session-dir`). Powers Session History search and usage Stats.
//! Everything is bounded: file size, message count and snippet length.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 2000;
const PREVIEW_CHARS: usize = 160;
const MAX_MESSAGES: usize = 2000;
const MAX_MESSAGE_CHARS: usize = 20_000;
const MAX_HITS: usize = 200;

/// One session-directory owner: the worktree and terminal that run omp there.
pub struct SessionSource {
    pub worker_id: String,
    pub terminal_id: String,
    pub terminal_name: String,
    pub dir: PathBuf,
}

#[derive(Serialize, Default, Clone)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct SessionSummary {
    pub path: String,
    pub worker_id: String,
    pub terminal_id: String,
    pub terminal_name: String,
    pub title: String,
    pub started_at: String,
    pub updated_at: String,
    pub cwd: String,
    pub messages: u32,
    pub user_messages: u32,
    pub preview: String,
    pub models: Vec<String>,
    pub usage: Usage,
}

#[derive(Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub text: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct SessionHit {
    pub path: String,
    pub role: String,
    pub snippet: String,
}

fn files(sources: &[SessionSource]) -> Vec<(&SessionSource, PathBuf)> {
    let mut found = Vec::new();
    for source in sources {
        let Ok(entries) = fs::read_dir(&source.dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl")
                && entry.metadata().is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_FILE_BYTES)
            {
                found.push((source, path));
                if found.len() >= MAX_FILES {
                    return found;
                }
            }
        }
    }
    found
}

fn lines(path: &Path) -> impl Iterator<Item = Value> {
    let reader = fs::File::open(path).ok().map(BufReader::new);
    reader
        .into_iter()
        .flat_map(|reader| reader.lines())
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
}

/// Visible text of a message: text parts only, never thinking or tool payloads.
fn message_text(message: &Value) -> String {
    let mut text = String::new();
    match message.get("content") {
        Some(Value::String(value)) => text.push_str(value),
        Some(Value::Array(parts)) => {
            for part in parts {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(value) = part.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(value);
                    }
                }
            }
        }
        _ => {}
    }
    text
}

fn clip(text: &str, limit: usize) -> (String, bool) {
    let mut chars = text.chars();
    let clipped: String = chars.by_ref().take(limit).collect();
    let truncated = chars.next().is_some();
    (if truncated { format!("{clipped}…") } else { clipped }, truncated)
}

fn number(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or(0)
}

pub fn summarize(source: &SessionSource, path: &Path) -> SessionSummary {
    let mut summary = SessionSummary {
        path: path.to_string_lossy().into_owned(),
        worker_id: source.worker_id.clone(),
        terminal_id: source.terminal_id.clone(),
        terminal_name: source.terminal_name.clone(),
        title: String::new(),
        started_at: String::new(),
        updated_at: String::new(),
        cwd: String::new(),
        messages: 0,
        user_messages: 0,
        preview: String::new(),
        models: Vec::new(),
        usage: Usage::default(),
    };
    for line in lines(path) {
        let kind = line.get("type").and_then(Value::as_str).unwrap_or_default();
        if let Some(timestamp) = line.get("timestamp").and_then(Value::as_str) {
            if summary.started_at.is_empty() {
                summary.started_at = timestamp.to_owned();
            }
            summary.updated_at = timestamp.to_owned();
        }
        match kind {
            "title" => {
                if let Some(title) = line.get("title").and_then(Value::as_str) {
                    summary.title = title.to_owned();
                }
            }
            "session" => {
                if let Some(cwd) = line.get("cwd").and_then(Value::as_str) {
                    summary.cwd = cwd.to_owned();
                }
            }
            "message" => {
                let Some(message) = line.get("message") else { continue };
                summary.messages += 1;
                let role = message.get("role").and_then(Value::as_str).unwrap_or_default();
                if role == "user" {
                    summary.user_messages += 1;
                    if summary.preview.is_empty() {
                        summary.preview = clip(message_text(message).trim(), PREVIEW_CHARS).0;
                    }
                }
                if let Some(model) = message.get("model").and_then(Value::as_str) {
                    if !summary.models.iter().any(|known| known == model) {
                        summary.models.push(model.to_owned());
                    }
                }
                if let Some(usage) = message.get("usage") {
                    summary.usage.input += number(usage.get("input"));
                    summary.usage.output += number(usage.get("output"));
                    summary.usage.cache_read += number(usage.get("cacheRead"));
                    summary.usage.cache_write += number(usage.get("cacheWrite"));
                    summary.usage.cost += match usage.get("cost") {
                        Some(Value::Object(cost)) => cost.get("total").and_then(Value::as_f64).unwrap_or(0.0),
                        Some(value) => value.as_f64().unwrap_or(0.0),
                        None => 0.0,
                    };
                }
            }
            _ => {}
        }
    }
    if summary.title.is_empty() {
        summary.title = if summary.preview.is_empty() { "Untitled session".into() } else { clip(&summary.preview, 60).0 };
    }
    summary
}

pub fn list(sources: &[SessionSource]) -> Vec<SessionSummary> {
    let mut all: Vec<SessionSummary> = files(sources).into_iter().map(|(source, path)| summarize(source, &path)).collect();
    all.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    all
}

/// The transcript of one session, which must belong to a known session directory.
pub fn read(sources: &[SessionSource], path: &str) -> Result<Vec<SessionMessage>, String> {
    let requested = PathBuf::from(path);
    let known = files(sources).into_iter().any(|(_, candidate)| candidate == requested);
    if !known {
        return Err("That session does not belong to a Minnow agent terminal".into());
    }
    let mut messages = Vec::new();
    for line in lines(&requested) {
        if line.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(message) = line.get("message") else { continue };
        let text = message_text(message);
        if text.trim().is_empty() {
            continue;
        }
        let (text, truncated) = clip(&text, MAX_MESSAGE_CHARS);
        messages.push(SessionMessage {
            role: message.get("role").and_then(Value::as_str).unwrap_or("unknown").to_owned(),
            text,
            timestamp: line.get("timestamp").and_then(Value::as_str).unwrap_or_default().to_owned(),
            model: message.get("model").and_then(Value::as_str).map(str::to_owned),
            truncated,
        });
        if messages.len() >= MAX_MESSAGES {
            break;
        }
    }
    Ok(messages)
}

/// Case-insensitive literal search through visible message text.
pub fn search(sources: &[SessionSource], query: &str) -> Vec<SessionHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    'files: for (_, path) in files(sources) {
        for line in lines(&path) {
            let Some(message) = line.get("message") else { continue };
            let text = message_text(message);
            let lower = text.to_lowercase();
            let Some(at) = lower.find(&needle) else { continue };
            let start = lower[..at].char_indices().rev().nth(40).map(|(index, _)| index).unwrap_or(0);
            let snippet = clip(text.get(start..).unwrap_or(&text), PREVIEW_CHARS).0;
            hits.push(SessionHit {
                path: path.to_string_lossy().into_owned(),
                role: message.get("role").and_then(Value::as_str).unwrap_or_default().to_owned(),
                snippet: if start > 0 { format!("…{snippet}") } else { snippet },
            });
            if hits.len() >= MAX_HITS {
                break 'files;
            }
            continue 'files;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_reads_and_searches_a_real_shaped_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("2026-01-01_x.jsonl");
        let body = [
            r#"{"type":"title","v":1,"title":"Fix login","source":"auto","updatedAt":"2026-01-01T00:05:00Z"}"#,
            r#"{"type":"session","version":3,"id":"s","timestamp":"2026-01-01T00:00:00Z","cwd":"C:\\repo"}"#,
            r#"{"type":"message","id":"1","parentId":null,"timestamp":"2026-01-01T00:01:00Z","message":{"role":"user","content":[{"type":"text","text":"Please fix the login redirect"}]}}"#,
            r#"{"type":"message","id":"2","parentId":"1","timestamp":"2026-01-01T00:02:00Z","message":{"role":"assistant","model":"m1","content":[{"type":"thinking","thinking":"secret"},{"type":"text","text":"Done: redirect fixed."}],"usage":{"input":10,"output":5,"cacheRead":2,"cacheWrite":0,"cost":{"total":0.25}}}}"#,
            "not json",
        ]
        .join("\n");
        fs::write(&file, body).unwrap();
        let sources = vec![SessionSource { worker_id: "w".into(), terminal_id: "t".into(), terminal_name: "Agent".into(), dir: dir.path().to_path_buf() }];

        let list = list(&sources);
        assert_eq!(list.len(), 1);
        let summary = &list[0];
        assert_eq!(summary.title, "Fix login");
        assert_eq!(summary.cwd, r"C:\repo");
        assert_eq!((summary.messages, summary.user_messages), (2, 1));
        assert_eq!(summary.preview, "Please fix the login redirect");
        assert_eq!(summary.models, ["m1"]);
        assert_eq!((summary.usage.input, summary.usage.output, summary.usage.cache_read), (10, 5, 2));
        assert!((summary.usage.cost - 0.25).abs() < 1e-9);

        let messages = read(&sources, &summary.path).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].text, "Done: redirect fixed.");
        assert!(!messages.iter().any(|message| message.text.contains("secret")));
        assert!(read(&sources, "C:\\elsewhere.jsonl").is_err());

        let hits = search(&sources, "REDIRECT");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("redirect"));
        assert!(search(&sources, "absent").is_empty());
    }
}
