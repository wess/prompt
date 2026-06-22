//! Ghostty-style `key = value` configuration loading for the Prompt
//! terminal emulator.

pub mod action;
mod apply;
pub mod edit;
pub mod keybind;
pub mod options;
pub mod parse;
pub mod value;
pub mod watch;

pub use action::{Action, ResizeDir, SplitDirection, SplitFocus};
pub use edit::{set_list, upsert};
pub use keybind::{
    default_keybinds, diff_from_defaults, format_trigger, parse_keybind, resolve, Keybind, Mods,
};
pub use options::{ClipboardAccess, CursorStyle, FontStyle, OptionAsAlt, Options};
pub use parse::{parse_str, Diagnostic};
pub use watch::{watch, WatchHandle};

use std::path::PathBuf;

/// Default config file path:
/// `$XDG_CONFIG_HOME/prompt/config`, else `~/.config/prompt/config`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("prompt").join("config"));
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
            .join("config"),
    )
}

/// Load configuration from an explicit path. A missing or unreadable file
/// yields defaults with no diagnostics.
pub fn load_path(path: &std::path::Path) -> (Options, Vec<Diagnostic>) {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_str(&text),
        Err(_) => (Options::default(), Vec::new()),
    }
}

/// Load configuration from the default path. A missing file yields defaults.
pub fn load() -> (Options, Vec<Diagnostic>) {
    match default_path() {
        Some(path) => load_path(&path),
        None => (Options::default(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let (opts, diags) = load_path(std::path::Path::new("/nonexistent/prompt/config"));
        assert_eq!(opts, Options::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn load_path_reads_file() {
        let dir = std::env::temp_dir().join(format!("promptconfigtest{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config");
        std::fs::write(&file, "font-size = 17\nbogus = 1\n").unwrap();
        let (opts, diags) = load_path(&file);
        assert_eq!(opts.font_size, 17.0);
        assert_eq!(diags.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_path_shape() {
        // Whatever the environment, if a path comes back it must end with
        // prompt/config.
        if let Some(p) = default_path() {
            assert!(p.ends_with("prompt/config"), "{p:?}");
        }
    }
}
