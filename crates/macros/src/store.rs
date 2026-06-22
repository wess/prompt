//! On-disk macro storage: one `<name>.macro` file per macro, one command per
//! line. Plain text so a macro is easy to read, edit, or version by hand.

use std::path::{Path, PathBuf};

use crate::{valid_name, Macro};

/// File extension for a stored macro.
const EXT: &str = "macro";

/// Default macro directory: `$XDG_CONFIG_HOME/prompt/macros`, falling back to
/// `$HOME/.config/prompt/macros`. Mirrors the plugin directory layout.
pub fn defaultdir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("prompt").join("macros"));
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
            .join("macros"),
    )
}

/// Load every macro in `dir`, sorted by name. A missing directory yields an
/// empty list; files with invalid names or no commands are skipped.
pub fn load(dir: &Path) -> Vec<Macro> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !valid_name(name) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let commands = parse(&text);
        if !commands.is_empty() {
            out.push(Macro::new(name, commands));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Write `mac` to `dir/<name>.macro`, creating the directory if needed. Errors
/// surface as a message for the caller to log.
pub fn save(dir: &Path, mac: &Macro) -> Result<(), String> {
    if !valid_name(&mac.name) {
        return Err(format!("invalid macro name `{}`", mac.name));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let body = mac
        .commands
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let path = file(dir, &mac.name);
    std::fs::write(&path, format!("{body}\n")).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Rename `old` to `new`, overwriting any existing `new`.
pub fn rename(dir: &Path, old: &str, new: &str) -> Result<(), String> {
    if !valid_name(new) {
        return Err(format!("invalid macro name `{new}`"));
    }
    std::fs::rename(file(dir, old), file(dir, new))
        .map_err(|e| format!("rename {old} -> {new}: {e}"))
}

/// Delete the macro named `name`. A missing file is not an error.
pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    match std::fs::remove_file(file(dir, name)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete {name}: {e}")),
    }
}

fn file(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{EXT}"))
}

/// Split stored text into command lines, dropping blank lines and `#` comments
/// so a hand-edited file can carry notes.
fn parse(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("promptmacrotest{}{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempdir("roundtrip");
        let mac = Macro::new("deploy", vec!["git pull".into(), "bun run build".into()]);
        save(&dir, &mac).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded, vec![mac]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn load_sorts_and_skips_comments_and_blanks() {
        let dir = tempdir("parse");
        std::fs::write(dir.join("b.macro"), "# note\n\nls\n").unwrap();
        std::fs::write(dir.join("a.macro"), "echo a\n").unwrap();
        std::fs::write(dir.join("ignore.txt"), "echo nope\n").unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "a");
        assert_eq!(loaded[1], Macro::new("b", vec!["ls".into()]));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rename_and_delete() {
        let dir = tempdir("rename");
        save(&dir, &Macro::new("old", vec!["echo hi".into()])).unwrap();
        rename(&dir, "old", "new").unwrap();
        assert_eq!(load(&dir)[0].name, "new");
        delete(&dir, "new").unwrap();
        assert!(load(&dir).is_empty());
        // Deleting a missing macro is a no-op.
        delete(&dir, "gone").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_rejects_bad_name() {
        let dir = tempdir("badname");
        assert!(save(&dir, &Macro::new("Bad Name", vec!["x".into()])).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
