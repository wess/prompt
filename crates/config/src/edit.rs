//! Editing the config file in place. The settings UI writes single
//! `key = value` lines back; the file stays the source of truth and the
//! live-reload watcher picks the change up.

/// Return `text` with `key` set to `value`: the first non-comment line for
/// `key` is replaced in place (preserving every other line and comment),
/// or a new line is appended when the key is absent.
pub fn upsert(text: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 4);
    let mut replaced = false;
    for line in text.lines() {
        if !replaced && line_key(line) == Some(key) {
            out.push_str(&format!("{key} = {value}"));
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(&format!("{key} = {value}\n"));
    }
    out
}

/// The key of a `key = value` line, or `None` for comments/blanks/malformed.
fn line_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = line.split_once('=')?;
    Some(key.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_when_absent() {
        assert_eq!(upsert("", "theme", "nord"), "theme = nord\n");
        assert_eq!(
            upsert("font-size = 14\n", "theme", "nord"),
            "font-size = 14\ntheme = nord\n"
        );
    }

    #[test]
    fn replaces_in_place_preserving_rest() {
        let src = "# my config\nfont-size = 12\ntheme = dark\ncopy-on-select = true\n";
        let got = upsert(src, "theme", "nord");
        assert_eq!(
            got,
            "# my config\nfont-size = 12\ntheme = nord\ncopy-on-select = true\n"
        );
    }

    #[test]
    fn ignores_commented_keys() {
        let src = "# theme = dark\nfont-size = 12\n";
        let got = upsert(src, "theme", "nord");
        assert_eq!(got, "# theme = dark\nfont-size = 12\ntheme = nord\n");
    }

    #[test]
    fn only_replaces_first_occurrence() {
        let src = "theme = a\ntheme = b\n";
        assert_eq!(upsert(src, "theme", "c"), "theme = c\ntheme = b\n");
    }

    #[test]
    fn round_trips_through_parser() {
        let text = upsert(&upsert("", "theme", "nord"), "font-size", "16");
        let (opts, diags) = crate::parse_str(&text);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(opts.theme, "nord");
        assert_eq!(opts.font_size, 16.0);
    }
}
