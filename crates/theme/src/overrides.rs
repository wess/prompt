//! Apply config-sourced string overrides on top of a base scheme.

use std::fmt;

use crate::rgb::{ParseRgbError, Rgb};
use crate::scheme::Scheme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideError {
    /// A hex color string failed to parse; `field` names the entry.
    Hex {
        field: String,
        value: String,
        error: ParseRgbError,
    },
    /// An ANSI override index outside `0..=15`.
    Index(u8),
}

impl fmt::Display for OverrideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverrideError::Hex {
                field,
                value,
                error,
            } => {
                write!(f, "invalid color {value:?} for {field}: {error}")
            }
            OverrideError::Index(index) => {
                write!(f, "ansi override index {index} out of range (0..=15)")
            }
        }
    }
}

impl std::error::Error for OverrideError {}

fn parse(field: &str, value: &str) -> Result<Rgb, OverrideError> {
    value.parse().map_err(|error| OverrideError::Hex {
        field: field.to_string(),
        value: value.to_string(),
        error,
    })
}

/// Copy `scheme` and replace any field whose override is `Some`, plus
/// the listed ANSI slots (`0..=15`). Hex strings accept `#rgb`,
/// `#rrggbb`, and the same forms without `#`. The first invalid value
/// or out-of-range index aborts with an error; the base scheme is
/// never partially mutated.
#[allow(clippy::too_many_arguments)]
pub fn apply_overrides(
    scheme: &Scheme,
    background: Option<&str>,
    foreground: Option<&str>,
    cursor: Option<&str>,
    cursor_text: Option<&str>,
    selection_foreground: Option<&str>,
    selection_background: Option<&str>,
    ansi: &[(u8, String)],
) -> Result<Scheme, OverrideError> {
    let mut out = *scheme;
    if let Some(value) = background {
        out.background = parse("background", value)?;
    }
    if let Some(value) = foreground {
        out.foreground = parse("foreground", value)?;
    }
    if let Some(value) = cursor {
        out.cursor = parse("cursor", value)?;
    }
    if let Some(value) = cursor_text {
        out.cursor_text = parse("cursor_text", value)?;
    }
    if let Some(value) = selection_foreground {
        out.selection_foreground = parse("selection_foreground", value)?;
    }
    if let Some(value) = selection_background {
        out.selection_background = parse("selection_background", value)?;
    }
    for (index, value) in ansi {
        if *index > 15 {
            return Err(OverrideError::Index(*index));
        }
        out.ansi[*index as usize] = parse(&format!("ansi[{index}]"), value)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    fn base() -> &'static Scheme {
        builtin::builtin("dracula").unwrap()
    }

    fn hex(s: &str) -> Rgb {
        s.parse().unwrap()
    }

    #[test]
    fn no_overrides_is_identity() {
        let out = apply_overrides(base(), None, None, None, None, None, None, &[]).unwrap();
        assert_eq!(out, *base());
    }

    #[test]
    fn field_overrides_apply() {
        let out = apply_overrides(
            base(),
            Some("#000000"),
            Some("#ffffff"),
            Some("#ff0000"),
            Some("00ff00"),
            Some("#abc"),
            Some("#123456"),
            &[],
        )
        .unwrap();
        assert_eq!(out.background, hex("#000000"));
        assert_eq!(out.foreground, hex("#ffffff"));
        assert_eq!(out.cursor, hex("#ff0000"));
        assert_eq!(out.cursor_text, hex("#00ff00"));
        assert_eq!(out.selection_foreground, hex("#aabbcc"));
        assert_eq!(out.selection_background, hex("#123456"));
        // Untouched fields keep base values.
        assert_eq!(out.ansi, base().ansi);
        assert_eq!(out.name, base().name);
    }

    #[test]
    fn partial_override_leaves_rest() {
        let out =
            apply_overrides(base(), Some("#101010"), None, None, None, None, None, &[]).unwrap();
        assert_eq!(out.background, hex("#101010"));
        assert_eq!(out.foreground, base().foreground);
        assert_eq!(out.cursor, base().cursor);
    }

    #[test]
    fn ansi_overrides_apply() {
        let ansi = [(1u8, "#000001".to_string()), (15u8, "#0f0f0f".to_string())];
        let out = apply_overrides(base(), None, None, None, None, None, None, &ansi).unwrap();
        assert_eq!(out.ansi[1], hex("#000001"));
        assert_eq!(out.ansi[15], hex("#0f0f0f"));
        assert_eq!(out.ansi[0], base().ansi[0]);
        assert_eq!(out.ansi[14], base().ansi[14]);
    }

    #[test]
    fn bad_hex_is_reported() {
        let err =
            apply_overrides(base(), Some("nope"), None, None, None, None, None, &[]).unwrap_err();
        match err {
            OverrideError::Hex { field, value, .. } => {
                assert_eq!(field, "background");
                assert_eq!(value, "nope");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bad_ansi_hex_is_reported() {
        let ansi = [(3u8, "#zzz".to_string())];
        let err = apply_overrides(base(), None, None, None, None, None, None, &ansi).unwrap_err();
        match err {
            OverrideError::Hex { field, .. } => assert_eq!(field, "ansi[3]"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn out_of_range_index_is_reported() {
        let ansi = [(16u8, "#ffffff".to_string())];
        let err = apply_overrides(base(), None, None, None, None, None, None, &ansi).unwrap_err();
        assert_eq!(err, OverrideError::Index(16));
    }

    #[test]
    fn errors_display() {
        assert!(OverrideError::Index(200).to_string().contains("200"));
        let err =
            apply_overrides(base(), None, Some("xx"), None, None, None, None, &[]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("foreground") && msg.contains("xx"));
    }
}
