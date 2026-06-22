//! Translate a resolved config keybind trigger into a gpui keystroke
//! string. gpui joins a keystroke's components with `-` (e.g.
//! `cmd-shift-d`) and names a few keys differently than the config crate.

/// gpui keystroke string for a config trigger, or `None` when the key has
/// no spelling we can emit. The caller still validates the result with
/// [`gpui::Keystroke::parse`] before binding, so an odd key is skipped
/// rather than panicking.
pub fn keystroke(mods: config::Mods, key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let key = gpui_key(key);
    let mut s = String::new();
    if mods.ctrl {
        s.push_str("ctrl-");
    }
    if mods.alt {
        s.push_str("alt-");
    }
    if mods.shift {
        s.push_str("shift-");
    }
    if mods.cmd {
        // `secondary` resolves to Command on macOS and Control elsewhere,
        // so a single `cmd+...` config binding is correct on every platform.
        s.push_str("secondary-");
    }
    s.push_str(&key);
    Some(s)
}

/// Map config key names onto gpui's spellings. Most match; only the paged
/// navigation keys differ (`page_up` vs `pageup`).
fn gpui_key(key: &str) -> String {
    match key {
        "page_up" => "pageup".to_string(),
        "page_down" => "pagedown".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, shift: bool, alt: bool, cmd: bool) -> config::Mods {
        config::Mods {
            ctrl,
            shift,
            alt,
            cmd,
        }
    }

    #[test]
    fn plain_and_modified_keys() {
        assert_eq!(
            keystroke(mods(false, false, false, true), "t").unwrap(),
            "secondary-t"
        );
        assert_eq!(
            keystroke(mods(false, true, false, true), "d").unwrap(),
            "shift-secondary-d"
        );
        assert_eq!(
            keystroke(mods(true, false, true, false), "x").unwrap(),
            "ctrl-alt-x"
        );
        assert_eq!(keystroke(config::Mods::default(), "a").unwrap(), "a");
    }

    #[test]
    fn modifier_order_is_canonical() {
        // ctrl, alt, shift, cmd(secondary) regardless of how many are set.
        assert_eq!(
            keystroke(mods(true, true, true, true), "k").unwrap(),
            "ctrl-alt-shift-secondary-k"
        );
    }

    #[test]
    fn paged_keys_are_renamed() {
        assert_eq!(
            keystroke(mods(false, true, false, false), "page_up").unwrap(),
            "shift-pageup"
        );
        assert_eq!(
            keystroke(mods(false, true, false, false), "page_down").unwrap(),
            "shift-pagedown"
        );
    }

    #[test]
    fn punctuation_keys_pass_through() {
        // The minus key with cmd renders as `secondary--`, which gpui parses
        // as the platform modifier + the `-` key.
        assert_eq!(
            keystroke(mods(false, false, false, true), "-").unwrap(),
            "secondary--"
        );
        assert_eq!(
            keystroke(mods(false, false, false, true), "+").unwrap(),
            "secondary-+"
        );
        assert_eq!(
            keystroke(mods(false, false, false, true), "=").unwrap(),
            "secondary-="
        );
        assert_eq!(
            keystroke(mods(false, false, false, true), ",").unwrap(),
            "secondary-,"
        );
    }

    #[test]
    fn named_keys_pass_through() {
        for k in ["enter", "tab", "escape", "space", "up", "home", "f5"] {
            assert_eq!(keystroke(config::Mods::default(), k).unwrap(), k);
        }
    }

    #[test]
    fn every_emitted_default_binding_parses_in_gpui() {
        // The whole default set must produce gpui-parseable keystrokes,
        // so binding them at startup never panics.
        for kb in config::default_keybinds() {
            let ks = keystroke(kb.mods, &kb.key)
                .unwrap_or_else(|| panic!("no keystroke for {:?}+{}", kb.mods, kb.key));
            gpui::Keystroke::parse(&ks).unwrap_or_else(|e| panic!("gpui rejected {ks:?}: {e:?}"));
        }
    }
}
