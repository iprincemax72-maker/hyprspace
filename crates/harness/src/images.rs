// Pictures a tool hands back inside its result, like an MCP screenshot, saved as files so the
// transcript can show them. Claude wraps one as `{"type":"image","source":{"data",
// "media_type"}}`, MCP and Codex as `{"type":"image","data","mimeType"}`.

use std::path::PathBuf;

use base64::Engine;
use serde_json::Value;

const DEEPEST: usize = 32;
const LARGEST: usize = 32 << 20;

/// Every picture in `content`, written once to `hyprspace-images` in the temp folder, where pasted
/// ones go too. A name from the bytes means a history read twice writes nothing new.
pub(crate) fn saved(content: &Value) -> Vec<PathBuf> {
    let mut out = Vec::new();
    find(content, 0, &mut out);
    out
}

fn find(v: &Value, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > DEEPEST {
        return;
    }
    match v {
        Value::Array(items) => items.iter().for_each(|i| find(i, depth + 1, out)),
        Value::Object(o) => {
            if o.get("type").and_then(Value::as_str) == Some("image")
                && let Some(path) = picture(v)
            {
                out.push(path);
                return;
            }
            o.values().for_each(|i| find(i, depth + 1, out));
        }
        _ => {}
    }
}

fn picture(v: &Value) -> Option<PathBuf> {
    let data = v["source"]["data"].as_str().or(v["data"].as_str())?;
    let mime = v["source"]["media_type"]
        .as_str()
        .or(v["mimeType"].as_str())
        .or(v["mime_type"].as_str())
        .unwrap_or("image/png");
    let ext = match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    };
    if data.len() > LARGEST {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(data))
        .ok()?;
    let dir = std::env::temp_dir().join("hyprspace-images");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("tool-{:016x}.{ext}", fnv(&bytes)));
    if !path.exists() {
        std::fs::write(&path, &bytes).ok()?;
    }
    Some(path)
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn both_shapes_save_once_and_text_is_skipped() {
        let png = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG not really");
        let claude = json!([
            { "type": "text", "text": "Captured." },
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": png } },
        ]);
        let codex =
            json!({ "content": [{ "type": "image", "data": png, "mimeType": "image/png" }] });
        let a = saved(&claude);
        let b = saved(&codex);
        assert_eq!(a.len(), 1);
        assert_eq!(a, b, "the same picture lands in the same file");
        assert_eq!(std::fs::read(&a[0]).unwrap(), b"\x89PNG not really");
        assert!(saved(&json!("plain text")).is_empty());
        assert!(saved(&json!([{ "type": "image", "data": "not base64!" }])).is_empty());
    }
}
