//! Lightweight URL detection in terminal text, so a URL can be opened with
//! a click even when the program did not emit an OSC 8 hyperlink.

/// Recognized URL schemes (checked case-insensitively).
const SCHEMES: &[&str] = &["https://", "http://", "ftp://", "file://", "mailto:"];

/// Find URLs in `chars` as char-index ranges `[start, end)`.
pub fn find(chars: &[char]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(len) = match_at(&chars[i..]) {
            spans.push((i, i + len));
            i += len;
        } else {
            i += 1;
        }
    }
    spans
}

/// If a URL starts at the front of `s`, return its length in chars.
fn match_at(s: &[char]) -> Option<usize> {
    let scheme_len = SCHEMES.iter().find_map(|scheme| {
        let sl: Vec<char> = scheme.chars().collect();
        (s.len() > sl.len()
            && s[..sl.len()]
                .iter()
                .zip(&sl)
                .all(|(a, b)| a.to_ascii_lowercase() == *b))
        .then_some(sl.len())
    })?;

    let mut len = scheme_len;
    while len < s.len() && is_url_char(s[len]) {
        len += 1;
    }
    // Need at least one char of authority/path after the scheme.
    if len == scheme_len {
        return None;
    }
    // Trim trailing sentence punctuation and an unbalanced closing paren.
    while len > scheme_len && is_trailing(s[len - 1]) {
        if s[len - 1] == ')' && balanced_paren(&s[scheme_len..len]) {
            break;
        }
        len -= 1;
    }
    (len > scheme_len).then_some(len)
}

/// Characters allowed inside a URL body (RFC 3986-ish, minus delimiters
/// that commonly bound URLs in prose).
fn is_url_char(c: char) -> bool {
    !c.is_whitespace()
        && !c.is_control()
        && !matches!(c, '"' | '<' | '>' | '`' | '{' | '}' | '|' | '\\' | '^')
}

/// Punctuation often trailing a URL in prose, trimmed from the match.
fn is_trailing(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '\'' | '"')
}

/// Whether parentheses in `s` are balanced (so a trailing `)` belongs).
fn balanced_paren(s: &[char]) -> bool {
    let opens = s.iter().filter(|&&c| c == '(').count();
    let closes = s.iter().filter(|&&c| c == ')').count();
    opens >= closes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        find(&chars)
            .into_iter()
            .map(|(a, b)| chars[a..b].iter().collect())
            .collect()
    }

    #[test]
    fn finds_basic_urls() {
        assert_eq!(urls("see https://example.com now"), ["https://example.com"]);
        assert_eq!(urls("http://a.b/c?d=1&e=2"), ["http://a.b/c?d=1&e=2"]);
        assert_eq!(urls("mailto:me@x.io"), ["mailto:me@x.io"]);
    }

    #[test]
    fn trims_trailing_punctuation() {
        assert_eq!(urls("go to https://x.io."), ["https://x.io"]);
        assert_eq!(urls("(see https://x.io)"), ["https://x.io"]);
        assert_eq!(urls("\"https://x.io\","), ["https://x.io"]);
    }

    #[test]
    fn keeps_balanced_parens() {
        assert_eq!(
            urls("https://en.wikipedia.org/wiki/Foo_(bar)"),
            ["https://en.wikipedia.org/wiki/Foo_(bar)"]
        );
    }

    #[test]
    fn multiple_and_none() {
        assert_eq!(
            urls("a https://1.com b http://2.com"),
            ["https://1.com", "http://2.com"]
        );
        assert!(urls("no links here").is_empty());
        assert!(urls("https://").is_empty()); // scheme only
        assert!(urls("nothttp://x").len() == 1); // still matches the embedded one
    }

    #[test]
    fn case_insensitive_scheme() {
        assert_eq!(urls("HTTPS://X.IO/p"), ["HTTPS://X.IO/p"]);
    }
}
