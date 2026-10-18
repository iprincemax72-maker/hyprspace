// `claude --resume <id>` only finds a conversation from the folder it started in, so a resume
// runs there whatever folder the caller passed. Claude keeps each conversation as
// `<config>/projects/<encoded folder>/<id>.jsonl`, and its lines carry the real `cwd`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use hyprspace_proto::Rewind;
use serde_json::Value;

/// Claude's config folder: `CLAUDE_CONFIG_DIR` when set, else `~/.claude`.
pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(dir.into());
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".claude"))
}

pub(crate) fn transcripts(config: &Path, thread: &str) -> Vec<PathBuf> {
    // an id is a uuid; anything else could walk out of the projects folder
    if thread.is_empty()
        || !thread
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Vec::new();
    }
    let name = format!("{thread}.jsonl");
    let Ok(dirs) = std::fs::read_dir(config.join("projects")) else {
        return Vec::new();
    };
    dirs.flatten()
        .map(|d| d.path().join(&name))
        .filter(|p| p.is_file())
        .collect()
}

/// The folder `thread` started in, or None when no transcript for it exists.
pub(crate) fn origin(config: &Path, thread: &str) -> Option<PathBuf> {
    for path in transcripts(config, thread) {
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        // the first lines are summaries and snapshots; the cwd shows up within a few
        for line in BufReader::new(file).lines().take(50).map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && let Some(cwd) = v["cwd"].as_str().filter(|c| !c.is_empty())
            {
                return Some(PathBuf::from(cwd));
            }
        }
    }
    None
}

/// Some(None) for the first prompt, which keeps nothing; None when the prompt isn't found.
pub(crate) fn rewind_point(config: &Path, thread: &str, rewind: &Rewind) -> Option<Option<String>> {
    let want = rewind.text.trim();
    if want.is_empty() {
        return None;
    }
    let file = std::fs::File::open(transcripts(config, thread).first()?).ok()?;
    let mut seen = 0;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if v["type"] != "user" || v["isMeta"].as_bool() == Some(true) {
            continue;
        }
        if !said(&v["message"]["content"])
            .trim_start()
            .starts_with(want)
        {
            continue;
        }
        if seen == rewind.nth {
            return Some(v["parentUuid"].as_str().map(str::to_string));
        }
        seen += 1;
    }
    None
}

fn said(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_folder_a_conversation_started_in() {
        let config = tempfile::tempdir().unwrap();
        let dir = config.path().join("projects").join("C--work-app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("abc-123.jsonl"),
            "{\"type\":\"summary\"}\nnot json\n{\"type\":\"user\",\"cwd\":\"C:\\\\work\\\\app\"}\n",
        )
        .unwrap();
        assert_eq!(
            origin(config.path(), "abc-123"),
            Some(PathBuf::from(r"C:\work\app"))
        );
        assert_eq!(origin(config.path(), "gone"), None);
        assert_eq!(origin(config.path(), "../abc-123"), None);
    }

    #[test]
    fn a_rewind_keeps_the_entry_before_its_prompt() {
        let config = tempfile::tempdir().unwrap();
        let dir = config.path().join("projects").join("C--work-app");
        std::fs::create_dir_all(&dir).unwrap();
        let lines = [
            r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"content":"fix it"}}"#,
            r#"{"type":"assistant","uuid":"a1","parentUuid":"u1"}"#,
            r#"{"type":"user","uuid":"m1","parentUuid":"a1","isMeta":true,"message":{"content":"fix it"}}"#,
            r#"{"type":"user","uuid":"u2","parentUuid":"a1","message":{"content":[{"type":"text","text":"fix it\n\nAttached file: a.pdf"}]}}"#,
            r#"{"type":"assistant","uuid":"a2","parentUuid":"u2"}"#,
        ];
        std::fs::write(dir.join("t-1.jsonl"), lines.join("\n")).unwrap();
        let at = |nth| {
            rewind_point(
                config.path(),
                "t-1",
                &Rewind {
                    text: "fix it".into(),
                    nth,
                },
            )
        };
        assert_eq!(at(0), Some(None));
        assert_eq!(at(1), Some(Some("a1".into())));
        assert_eq!(at(2), None);
    }
}
