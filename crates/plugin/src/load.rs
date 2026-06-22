//! Plugin discovery and loading.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::manifest::{self, Diagnostic, Plugin};
use crate::MANIFEST;

pub fn defaultdir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("prompt").join("plugins"));
        }
    }
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("prompt")
            .join("plugins"),
    )
}

/// Load explicitly configured plugin directories plus installed plugins from
/// the default plugin directory. Duplicate manifest paths are ignored.
pub fn load(paths: &[String]) -> (Vec<Plugin>, Vec<Diagnostic>) {
    let mut manifests = Vec::new();
    for path in paths {
        manifests.push(manifestpath(&expand(path)));
    }
    if let Some(dir) = defaultdir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest = manifestpath(&path);
                    if manifest.is_file() {
                        manifests.push(manifest);
                    }
                }
            }
        }
    }
    loadmanifests(manifests)
}

fn loadmanifests(paths: Vec<PathBuf>) -> (Vec<Plugin>, Vec<Diagnostic>) {
    let mut seen = HashSet::new();
    let mut plugins = Vec::new();
    let mut diags = Vec::new();
    for source in paths {
        let path = manifestpath(&source);
        if !seen.insert(path.clone()) {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (plugin, mut local) = manifest::parse(path, &text);
                diags.append(&mut local);
                if let Some(plugin) = plugin {
                    plugins.push(plugin);
                }
            }
            Err(error) => diags.push(Diagnostic {
                path,
                line: 0,
                message: format!("failed to read plugin manifest: {error}"),
            }),
        }
    }
    (plugins, diags)
}

fn manifestpath(path: &Path) -> PathBuf {
    if path.file_name().is_some_and(|name| name == MANIFEST) {
        path.to_path_buf()
    } else {
        path.join(MANIFEST)
    }
}

fn expand(path: &str) -> PathBuf {
    if path == "~" {
        return home().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("promptplugintest{}{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn explicit_path_can_be_directory_or_manifest() {
        let dir = tempdir("explicit");
        let manifest = dir.join(MANIFEST);
        std::fs::write(&manifest, "id = tools\n[[command]]\nid = top\nrun = top\n").unwrap();
        let (plugins, diags) = loadmanifests(vec![dir.clone(), manifest]);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "tools");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unreadable_manifest_reports_diagnostic() {
        let (plugins, diags) =
            loadmanifests(vec![PathBuf::from("/definitely/missing/plugin.toml")]);
        assert!(plugins.is_empty());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("failed to read"));
    }
}
