//! Substring search over the grid + scrollback. Matching is per visual
//! row (matches do not span line breaks), which covers the common case.

/// One search hit: a global row index (same space as
/// [`crate::Terminal::prompt_lines`]) and an inclusive column range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// Find every occurrence of `needle` in one row, given the row's visible
/// chars and their columns (wide spacers already removed). `fold` lowercases
/// for case-insensitive search. Non-overlapping, left to right.
pub fn in_row(
    needle: &[char],
    chars: &[char],
    col_of: &[usize],
    line: usize,
    fold: bool,
    wide_tail: impl Fn(usize) -> bool,
) -> Vec<Match> {
    let mut hits = Vec::new();
    if needle.is_empty() || needle.len() > chars.len() {
        return hits;
    }
    let eq = |a: char, b: char| {
        if fold {
            a.to_ascii_lowercase() == b.to_ascii_lowercase()
        } else {
            a == b
        }
    };
    let mut i = 0;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()]
            .iter()
            .zip(needle)
            .all(|(&a, &b)| eq(a, b))
        {
            let last = col_of[i + needle.len() - 1];
            hits.push(Match {
                line,
                start_col: col_of[i],
                end_col: last + usize::from(wide_tail(last)),
            });
            i += needle.len();
        } else {
            i += 1;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(needle: &str, row: &str, fold: bool) -> Vec<(usize, usize)> {
        let chars: Vec<char> = row.chars().collect();
        let cols: Vec<usize> = (0..chars.len()).collect();
        let needle: Vec<char> = needle.chars().collect();
        in_row(&needle, &chars, &cols, 0, fold, |_| false)
            .into_iter()
            .map(|m| (m.start_col, m.end_col))
            .collect()
    }

    #[test]
    fn finds_all_occurrences() {
        assert_eq!(run("ab", "abXabYab", false), [(0, 1), (3, 4), (6, 7)]);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(run("hi", "HI there hi", true), [(0, 1), (9, 10)]);
        assert_eq!(run("hi", "HI there hi", false), [(9, 10)]);
    }

    #[test]
    fn no_match_and_empty() {
        assert!(run("zz", "abc", false).is_empty());
        assert!(run("", "abc", false).is_empty());
        assert!(run("abcd", "abc", false).is_empty());
    }

    #[test]
    fn non_overlapping() {
        // "aa" in "aaaa" yields two matches, not three.
        assert_eq!(run("aa", "aaaa", false), [(0, 1), (2, 3)]);
    }
}
