//! The community plugin catalog: list and install plugins from this project's
//! own `plugins/` directory on GitHub. Browsing reads the GitHub contents API;
//! installing downloads a plugin folder's files into the user's plugin dir.
//!
//! The source repo is fixed (`wess/prompt`) — we never fetch from an arbitrary
//! location — and plugin names are validated before they touch the filesystem.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

const REPO: &str = "wess/prompt";

/// Names of plugins available in the catalog (the directories under `plugins/`),
/// sorted. Hits the GitHub contents API via `curl`.
pub fn list() -> Result<Vec<String>, String> {
    let body = curl(&format!("https://api.github.com/repos/{REPO}/contents/plugins"))?;
    let value: Value = serde_json::from_slice(&body).map_err(|e| format!("parse catalog: {e}"))?;
    let entries = value.as_array().ok_or("unexpected catalog response")?;
    let mut names: Vec<String> = entries
        .iter()
        .filter(|e| e.get("type").and_then(Value::as_str) == Some("dir"))
        .filter_map(|e| e.get("name").and_then(Value::as_str).map(str::to_string))
        .filter(|n| valid_name(n))
        .collect();
    names.sort();
    Ok(names)
}

/// Download every file of catalog plugin `name` into
/// `~/.config/prompt/plugins/<name>`. Only flat plugin folders are supported.
pub fn install(name: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(format!("invalid plugin name {name:?}"));
    }
    let dir = plugin::defaultdir().ok_or("no plugin directory (set HOME/XDG_CONFIG_HOME)")?;
    let dest = dir.join(name);

    let body = curl(&format!(
        "https://api.github.com/repos/{REPO}/contents/plugins/{name}"
    ))?;
    let value: Value = serde_json::from_slice(&body).map_err(|e| format!("parse listing: {e}"))?;
    let entries = value.as_array().ok_or("plugin is not a directory")?;

    std::fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut wrote = 0usize;
    for entry in entries {
        if entry.get("type").and_then(Value::as_str) != Some("file") {
            continue; // flat folders only for now
        }
        let (Some(file), Some(url)) = (
            entry.get("name").and_then(Value::as_str),
            entry.get("download_url").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !valid_file(file) {
            continue;
        }
        let bytes = curl(url)?;
        std::fs::write(dest.join(file), bytes)
            .map_err(|e| format!("write {file}: {e}"))?;
        wrote += 1;
    }
    if wrote == 0 {
        return Err("nothing downloaded".to_string());
    }
    Ok(dest)
}

fn curl(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args([
            "-sL",
            "--fail",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: prompt-terminal",
            url,
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("fetch failed ({})", out.status));
    }
    Ok(out.stdout)
}

/// A plugin folder name: the manifest id charset, no path separators or dots.
fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A downloadable file name: a plain file, never a path or hidden traversal.
fn valid_file(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.contains('/')
        && !s.contains('\\')
        && s != "."
        && s != ".."
}

#[cfg(test)]
#[path = "../tests/catalog.rs"]
mod tests;
