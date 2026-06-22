//! Keystroke-driven command capture.
//!
//! Recording works at the shell-command level: as the user types at a prompt
//! the recorder accumulates printable text into the current line, and each
//! submitted line (Enter) becomes one captured command. This deliberately
//! ignores cursor movement and other control input — it captures *what was
//! typed*, segmented on Enter, which is the honest unit a "command macro"
//! replays. The caller feeds plain key descriptions so this stays decoupled
//! from any input encoder.

/// Accumulates typed command lines while capture is active.
#[derive(Debug, Default)]
pub struct Recorder {
    active: bool,
    /// The line currently being typed, not yet submitted.
    line: String,
    /// Submitted command lines, in order.
    lines: Vec<String>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether capture is currently running.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Begin a fresh capture, discarding anything previously buffered.
    pub fn start(&mut self) {
        self.active = true;
        self.line.clear();
        self.lines.clear();
    }

    /// Abort capture and drop everything buffered.
    pub fn cancel(&mut self) {
        self.active = false;
        self.line.clear();
        self.lines.clear();
    }

    /// Stop capture and return the captured command lines. A non-empty
    /// in-progress line (no trailing Enter) is included as a final command.
    pub fn finish(&mut self) -> Vec<String> {
        self.flush_line();
        self.active = false;
        std::mem::take(&mut self.lines)
    }

    /// Feed one key event. `key` is the logical key name (gpui spelling:
    /// `"enter"`, `"backspace"`, ...); `text` is the character(s) the key
    /// produces, if any. No-op while inactive.
    pub fn key(&mut self, key: &str, text: Option<&str>) {
        if !self.active {
            return;
        }
        match key {
            "enter" => self.flush_line(),
            "backspace" => {
                self.line.pop();
            }
            _ => {
                if let Some(text) = text {
                    // Only printable text contributes; control runs (escape
                    // sequences, arrows) carry no command text.
                    if !text.is_empty() && !text.chars().any(char::is_control) {
                        self.line.push_str(text);
                    }
                }
            }
        }
    }

    /// Number of commands captured so far (excluding the in-progress line).
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether no commands have been captured yet.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn flush_line(&mut self) {
        let trimmed = self.line.trim();
        if !trimmed.is_empty() {
            self.lines.push(trimmed.to_string());
        }
        self.line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(rec: &mut Recorder, s: &str) {
        for ch in s.chars() {
            let mut buf = [0u8; 4];
            rec.key("", Some(ch.encode_utf8(&mut buf)));
        }
    }

    #[test]
    fn captures_lines_on_enter() {
        let mut rec = Recorder::new();
        rec.start();
        type_str(&mut rec, "echo hi");
        rec.key("enter", None);
        type_str(&mut rec, "ls -la");
        rec.key("enter", None);
        assert_eq!(rec.len(), 2);
        assert_eq!(rec.finish(), vec!["echo hi".to_string(), "ls -la".to_string()]);
        assert!(!rec.is_active());
    }

    #[test]
    fn backspace_edits_current_line() {
        let mut rec = Recorder::new();
        rec.start();
        type_str(&mut rec, "echoo");
        rec.key("backspace", None);
        type_str(&mut rec, " hi");
        rec.key("enter", None);
        assert_eq!(rec.finish(), vec!["echo hi".to_string()]);
    }

    #[test]
    fn trailing_unsubmitted_line_is_kept() {
        let mut rec = Recorder::new();
        rec.start();
        type_str(&mut rec, "pending");
        assert_eq!(rec.finish(), vec!["pending".to_string()]);
    }

    #[test]
    fn blank_lines_and_control_text_are_ignored() {
        let mut rec = Recorder::new();
        rec.start();
        rec.key("enter", None); // blank submission
        rec.key("", Some("\u{1b}[A")); // an arrow escape: dropped
        type_str(&mut rec, "  real  ");
        rec.key("enter", None);
        assert_eq!(rec.finish(), vec!["real".to_string()]);
    }

    #[test]
    fn inactive_recorder_ignores_input() {
        let mut rec = Recorder::new();
        type_str(&mut rec, "ignored");
        rec.key("enter", None);
        assert!(rec.finish().is_empty());
    }

    #[test]
    fn cancel_drops_everything() {
        let mut rec = Recorder::new();
        rec.start();
        type_str(&mut rec, "echo hi");
        rec.key("enter", None);
        rec.cancel();
        assert!(!rec.is_active());
        assert!(rec.finish().is_empty());
    }
}
