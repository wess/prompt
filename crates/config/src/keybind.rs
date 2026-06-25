//! Keybind parsing: `keybind = trigger=action` where the trigger is
//! modifiers and a key joined by `+`.

use crate::action::{Action, SplitDirection, SplitFocus};
use crate::parse::Diagnostic;

/// Modifier keys in a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub cmd: bool,
}

/// One resolved keybinding.
#[derive(Debug, Clone, PartialEq)]
pub struct Keybind {
    pub mods: Mods,
    /// Normalized key: a single character (lowercase) or a named key
    /// such as `enter` or `page_up`.
    pub key: String,
    pub action: Action,
}

impl Keybind {
    /// The trigger as a config string, e.g. `cmd+shift+t`.
    pub fn trigger(&self) -> String {
        format_trigger(self.mods, &self.key)
    }

    /// The full `trigger=action` config value for this binding.
    pub fn config_line(&self) -> String {
        format!("{}={}", self.trigger(), self.action.to_config())
    }
}

/// Format a trigger from modifiers and a normalized key, producing a string
/// that [`parse_trigger`] reads back. Punctuation keys use their named form
/// so the result never collides with the `+`/`=` trigger/action separators.
pub fn format_trigger(mods: Mods, key: &str) -> String {
    let mut s = String::new();
    if mods.cmd {
        push_part(&mut s, "cmd");
    }
    if mods.ctrl {
        push_part(&mut s, "ctrl");
    }
    if mods.alt {
        push_part(&mut s, "alt");
    }
    if mods.shift {
        push_part(&mut s, "shift");
    }
    push_part(&mut s, key_to_name(key));
    s
}

fn push_part(s: &mut String, part: &str) {
    if !s.is_empty() {
        s.push('+');
    }
    s.push_str(part);
}

/// The config name for a normalized key. Named keys and alphanumerics pass
/// through; punctuation maps back to the spelled-out name.
fn key_to_name(key: &str) -> &str {
    match key {
        "+" => "plus",
        "-" => "minus",
        "=" => "equal",
        "," => "comma",
        "." => "period",
        "/" => "slash",
        "\\" => "backslash",
        ";" => "semicolon",
        "'" => "apostrophe",
        "`" => "grave_accent",
        "[" => "bracket_left",
        "]" => "bracket_right",
        other => other,
    }
}

/// Given the desired full keybind set, produce the minimal `keybind` config
/// values that transform [`default_keybinds`] into it: an override line for
/// each binding that differs from (or is absent among) the defaults, and an
/// `=unbind` line for each default the set drops.
pub fn diff_from_defaults(desired: &[Keybind]) -> Vec<String> {
    let defaults = default_keybinds();
    let mut out = Vec::new();
    for kb in desired {
        let default_action = defaults
            .iter()
            .find(|d| d.mods == kb.mods && d.key == kb.key)
            .map(|d| &d.action);
        if default_action != Some(&kb.action) {
            out.push(kb.config_line());
        }
    }
    for d in &defaults {
        let kept = desired.iter().any(|kb| kb.mods == d.mods && kb.key == d.key);
        if !kept {
            out.push(format!("{}=unbind", format_trigger(d.mods, &d.key)));
        }
    }
    out
}

/// Parse one keybind value, e.g. `ctrl+shift+c=copy_to_clipboard`.
pub fn parse_keybind(s: &str) -> Result<Keybind, String> {
    let (trigger, action) = s
        .split_once('=')
        .ok_or_else(|| "expected `trigger=action`".to_string())?;
    let (mods, key) = parse_trigger(trigger.trim())?;
    let action = Action::parse(action.trim())?;
    Ok(Keybind { mods, key, action })
}

/// Parse a trigger like `cmd+shift+page_up` into modifiers plus a key.
pub fn parse_trigger(s: &str) -> Result<(Mods, String), String> {
    if s.is_empty() {
        return Err("empty trigger".to_string());
    }
    // The key is whatever follows the last `+`; a trailing `++` (or a bare
    // `+`) means the key itself is `+`.
    let (mods_part, key_part) = if s == "+" {
        ("", "+")
    } else if s.ends_with("++") {
        (&s[..s.len() - 2], "+")
    } else {
        match s.rfind('+') {
            Some(i) if i + 1 < s.len() => (&s[..i], &s[i + 1..]),
            Some(_) => return Err(format!("missing key in trigger `{s}`")),
            None => ("", s),
        }
    };
    let mut mods = Mods::default();
    if !mods_part.is_empty() {
        for part in mods_part.split('+') {
            match part.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods.ctrl = true,
                "shift" => mods.shift = true,
                "alt" | "opt" | "option" => mods.alt = true,
                "super" | "cmd" | "command" => mods.cmd = true,
                other => return Err(format!("unknown modifier `{other}`")),
            }
        }
    }
    let key = normalize_key(key_part.trim()).ok_or_else(|| format!("unknown key `{key_part}`"))?;
    Ok((mods, key))
}

/// Normalize a key: named keys pass through lowercase, punctuation names
/// map to their character, and any single non-whitespace char is itself.
fn normalize_key(s: &str) -> Option<String> {
    let k = s.to_ascii_lowercase();
    if NAMED_KEYS.contains(&k.as_str()) {
        return Some(k);
    }
    let mapped = match k.as_str() {
        "plus" => "+",
        "minus" => "-",
        "equal" => "=",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "backslash" => "\\",
        "semicolon" => ";",
        "apostrophe" => "'",
        "grave_accent" => "`",
        "bracket_left" => "[",
        "bracket_right" => "]",
        _ => "",
    };
    if !mapped.is_empty() {
        return Some(mapped.to_string());
    }
    let mut chars = k.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() => Some(c.to_string()),
        _ => None,
    }
}

/// Named keys accepted verbatim.
const NAMED_KEYS: &[&str] = &[
    "enter",
    "tab",
    "escape",
    "space",
    "backspace",
    "delete",
    "insert",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "page_up",
    "page_down",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// The built-in bindings, mirroring the app's hardcoded set.
pub fn default_keybinds() -> Vec<Keybind> {
    let cmd = Mods {
        cmd: true,
        ..Mods::default()
    };
    let cmd_shift = Mods {
        cmd: true,
        shift: true,
        ..Mods::default()
    };
    let cmd_alt = Mods {
        cmd: true,
        alt: true,
        ..Mods::default()
    };
    let cmd_alt_shift = Mods {
        cmd: true,
        alt: true,
        shift: true,
        ..Mods::default()
    };
    let kb = |mods: Mods, key: &str, action: Action| Keybind {
        mods,
        key: key.to_string(),
        action,
    };
    let mut binds = vec![
        kb(cmd, "n", Action::NewWindow),
        kb(cmd, "t", Action::NewTab),
        kb(cmd, "w", Action::CloseSurface),
        kb(cmd_alt, "w", Action::CloseTab),
        kb(cmd_shift, "w", Action::CloseWindow),
        kb(cmd_alt_shift, "w", Action::CloseAllWindows),
        kb(cmd, "d", Action::NewSplit(SplitDirection::Right)),
        kb(cmd_shift, "d", Action::NewSplit(SplitDirection::Down)),
        kb(cmd_shift, "[", Action::PreviousTab),
        kb(cmd_shift, "]", Action::NextTab),
        kb(cmd_alt, "up", Action::GotoSplit(SplitFocus::Up)),
        kb(cmd_alt, "down", Action::GotoSplit(SplitFocus::Down)),
        kb(cmd_alt, "left", Action::GotoSplit(SplitFocus::Left)),
        kb(cmd_alt, "right", Action::GotoSplit(SplitFocus::Right)),
        kb(cmd, "c", Action::Copy),
        kb(cmd, "v", Action::Paste),
        kb(cmd, "+", Action::IncreaseFontSize(1.0)),
        kb(cmd, "=", Action::IncreaseFontSize(1.0)),
        kb(cmd, "-", Action::DecreaseFontSize(1.0)),
        kb(cmd, "0", Action::ResetFontSize),
        kb(cmd, "k", Action::ClearScreen),
        kb(cmd, "f", Action::ToggleSearch),
        kb(cmd_shift, "f", Action::ToggleSemanticSearch),
        kb(cmd_shift, "e", Action::ExplainOutput),
        kb(cmd_shift, "g", Action::ComposeCommand),
        kb(cmd_shift, "b", Action::ToggleBroadcast),
        kb(cmd_shift, "p", Action::CommandPalette),
        kb(cmd_shift, "r", Action::ToggleRecording),
        kb(cmd, "up", Action::JumpToPrompt(-1)),
        kb(cmd, "down", Action::JumpToPrompt(1)),
        kb(cmd, ",", Action::ToggleSettings),
        kb(cmd_alt, "t", Action::ToggleQuickTerminal),
        // AI / Relay (only act when AI is enabled, but always bound so the
        // shortcut shows in the menu and stays editable).
        kb(cmd_shift, "a", Action::RelayLaunch),
        kb(cmd_shift, "i", Action::RelayFeed),
        kb(cmd_shift, "l", Action::RelayLog),
        kb(cmd, "q", Action::Quit),
    ];
    for n in 1..=9 {
        binds.push(kb(cmd, &n.to_string(), Action::GotoTab(n as i32)));
    }
    // macOS readline navigation. Option moves by word, Command by line, the
    // standard macOS shell editing sequences (`text:`/`esc:`).
    // Cmd chords never reach the pty on their own (see
    // `input::encode_key`), so they are bound here; Option chords are bound
    // too so word motion works regardless of `macos-option-as-alt`. Scoped to
    // macOS: elsewhere `cmd` resolves to Control, where these would shadow the
    // shell's own Ctrl-A/Ctrl-W bindings.
    #[cfg(target_os = "macos")]
    {
        let alt = Mods {
            alt: true,
            ..Mods::default()
        };
        // Cmd+Left/Right -> start/end of line (Ctrl-A / Ctrl-E).
        binds.push(kb(cmd, "left", Action::SendText(vec![0x01])));
        binds.push(kb(cmd, "right", Action::SendText(vec![0x05])));
        // Cmd+Backspace -> delete to start of line (Ctrl-U).
        binds.push(kb(cmd, "backspace", Action::SendText(vec![0x15])));
        // Option+Left/Right -> word back/forward (ESC b / ESC f).
        binds.push(kb(alt, "left", Action::SendText(vec![0x1b, b'b'])));
        binds.push(kb(alt, "right", Action::SendText(vec![0x1b, b'f'])));
        // Option+Backspace -> delete previous word (ESC DEL).
        binds.push(kb(alt, "backspace", Action::SendText(vec![0x1b, 0x7f])));
        // Cmd+A -> select the whole buffer.
        binds.push(kb(cmd, "a", Action::SelectAll));
    }
    binds
}

/// Resolve raw `keybind` config values against the defaults: a user
/// binding replaces any default with the same trigger, and `unbind`
/// removes it. Invalid entries become diagnostics (line 0) and are
/// skipped.
pub fn resolve(raw: &[String]) -> (Vec<Keybind>, Vec<Diagnostic>) {
    let mut binds = default_keybinds();
    let mut diags = Vec::new();
    for entry in raw {
        match parse_keybind(entry) {
            Ok(kb) => {
                binds.retain(|b| !(b.mods == kb.mods && b.key == kb.key));
                if kb.action != Action::Unbound {
                    binds.push(kb);
                }
            }
            Err(message) => diags.push(Diagnostic {
                line: 0,
                key: "keybind".to_string(),
                message: format!("`{entry}`: {message}"),
            }),
        }
    }
    (binds, diags)
}

#[cfg(test)]
#[path = "../tests/keybind.rs"]
mod tests;
