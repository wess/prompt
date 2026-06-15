//! Small value parsing helpers for config values.

/// Parse a boolean value. Accepts true/false, 1/0, yes/no (case-insensitive).
pub fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// Parse an f32.
pub fn parse_f32(s: &str) -> Option<f32> {
    s.parse().ok()
}

/// Parse a u32.
pub fn parse_u32(s: &str) -> Option<u32> {
    s.parse().ok()
}

/// Parse a usize.
pub fn parse_usize(s: &str) -> Option<usize> {
    s.parse().ok()
}

/// Parse a finite f32 and clamp it into `lo..=hi`.
pub fn parse_f32_range(s: &str, lo: f32, hi: f32) -> Option<f32> {
    let v: f32 = s.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(v.clamp(lo, hi))
}

/// Parse a cell-size adjustment: an integer pixel count with an optional
/// `px` suffix, e.g. `2`, `-1`, `+3px`.
pub fn parse_adjust(s: &str) -> Option<i32> {
    let t = s.strip_suffix("px").unwrap_or(s).trim();
    t.parse().ok()
}

/// Parse and normalize a hex color: optional `#`, then 6 hex digits.
/// Returns the normalized `#rrggbb` (lowercase) form.
pub fn parse_color(s: &str) -> Option<String> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", hex.to_ascii_lowercase()))
    } else {
        None
    }
}

/// Validate a font feature: optional `+`/`-` sign, then an alphanumeric
/// tag like `liga` or `ss01`. Returned verbatim.
pub fn parse_fontfeature(s: &str) -> Option<String> {
    let tag = s.strip_prefix(['+', '-']).unwrap_or(s);
    if !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(s.to_string())
    } else {
        None
    }
}

/// Strip a single pair of surrounding double quotes, if present.
pub fn unquote(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse the `N=#rrggbb` palette form into (index, color).
/// The color part is kept as a string; it must start with `#` and have
/// 6 hex digits after it.
pub fn parse_palette(s: &str) -> Option<(u8, String)> {
    let (idx, color) = s.split_once('=')?;
    let idx: u8 = idx.trim().parse().ok()?;
    let color = unquote(color.trim());
    let hex = color.strip_prefix('#')?;
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some((idx, color.to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bools() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("TRUE"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("yes"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("no"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
        assert_eq!(parse_bool(""), None);
    }

    #[test]
    fn numbers() {
        assert_eq!(parse_f32("13.5"), Some(13.5));
        assert_eq!(parse_f32("13"), Some(13.0));
        assert_eq!(parse_f32("abc"), None);
        assert_eq!(parse_u32("42"), Some(42));
        assert_eq!(parse_u32("-1"), None);
        assert_eq!(parse_usize("10000"), Some(10000));
        assert_eq!(parse_usize("x"), None);
    }

    #[test]
    fn f32_ranges() {
        assert_eq!(parse_f32_range("1.5", 1.0, 21.0), Some(1.5));
        assert_eq!(parse_f32_range("0.5", 1.0, 21.0), Some(1.0));
        assert_eq!(parse_f32_range("100", 1.0, 21.0), Some(21.0));
        assert_eq!(parse_f32_range("abc", 1.0, 21.0), None);
        assert_eq!(parse_f32_range("NaN", 1.0, 21.0), None);
        assert_eq!(parse_f32_range("inf", 1.0, 21.0), None);
    }

    #[test]
    fn adjusts() {
        assert_eq!(parse_adjust("2"), Some(2));
        assert_eq!(parse_adjust("-1"), Some(-1));
        assert_eq!(parse_adjust("+3"), Some(3));
        assert_eq!(parse_adjust("4px"), Some(4));
        assert_eq!(parse_adjust("-2px"), Some(-2));
        assert_eq!(parse_adjust("2 px"), Some(2));
        assert_eq!(parse_adjust("10%"), None);
        assert_eq!(parse_adjust("abc"), None);
        assert_eq!(parse_adjust(""), None);
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#1d1f21"), Some("#1d1f21".to_string()));
        assert_eq!(parse_color("1d1f21"), Some("#1d1f21".to_string()));
        assert_eq!(parse_color("#FFAA00"), Some("#ffaa00".to_string()));
        assert_eq!(parse_color("#fff"), None);
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#12345g"), None);
        assert_eq!(parse_color(""), None);
    }

    #[test]
    fn fontfeatures() {
        assert_eq!(parse_fontfeature("liga"), Some("liga".to_string()));
        assert_eq!(parse_fontfeature("-liga"), Some("-liga".to_string()));
        assert_eq!(parse_fontfeature("+ss01"), Some("+ss01".to_string()));
        assert_eq!(parse_fontfeature("-"), None);
        assert_eq!(parse_fontfeature("no spaces"), None);
        assert_eq!(parse_fontfeature(""), None);
    }

    #[test]
    fn unquoting() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("hello"), "hello");
        assert_eq!(unquote("  \"spaced value\"  "), "spaced value");
        assert_eq!(unquote("\""), "\"");
        assert_eq!(unquote(""), "");
        assert_eq!(unquote("\"\""), "");
    }

    #[test]
    fn palette() {
        assert_eq!(parse_palette("0=#1d1f21"), Some((0, "#1d1f21".to_string())));
        assert_eq!(
            parse_palette(" 15 = #FFFFFF "),
            Some((15, "#FFFFFF".to_string()))
        );
        assert_eq!(parse_palette("256=#000000"), None); // index overflows u8
        assert_eq!(parse_palette("0=1d1f21"), None); // missing #
        assert_eq!(parse_palette("0=#zzz"), None); // bad hex
        assert_eq!(parse_palette("0=#12345"), None); // short hex
        assert_eq!(parse_palette("#1d1f21"), None); // missing index
    }
}
