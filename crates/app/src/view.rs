//! One terminal pane: owns its session and handles input/events.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, SharedString,
    Subscription, Window, div, px,
};
use terminal::{Event, Session};

use crate::colors::{self, Colors};
use crate::element::TerminalElement;
use crate::metrics::{CellSize, Padding};
use crate::mouse::MouseState;

/// Maximum time a frame is withheld for synchronized output before it is
/// painted anyway, so a stuck ?2026 cannot freeze the view (xterm/contour
/// use a similar bound).
const SYNC_TIMEOUT: Duration = Duration::from_millis(150);

/// Pane events the workspace root reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewEvent {
    /// The vt title changed: refresh tab labels / the window title.
    Title,
    /// The child exited: close this pane.
    Exited,
}

/// Pane title: the vt title when set and non-blank, else the fallback.
pub fn label<'a>(title: Option<&'a str>, fallback: &'a str) -> &'a str {
    match title {
        Some(t) if !t.trim().is_empty() => t,
        _ => fallback,
    }
}

/// Scrollback search overlay state.
struct Search {
    edit: crate::textedit::TextEdit,
    /// Index of the focused match among current results.
    current: usize,
}

/// Config-derived appearance pushed to every pane on a live reload.
pub struct Appearance {
    pub colors: Rc<Colors>,
    pub font: gpui::Font,
    pub font_size: gpui::Pixels,
    pub cell: CellSize,
    pub pad: Padding,
    pub cursor_default: config::CursorStyle,
    pub copy_on_select: bool,
}

pub struct TerminalView {
    session: Arc<Session>,
    colors: Rc<Colors>,
    font: gpui::Font,
    font_size: gpui::Pixels,
    cell: CellSize,
    pad: Padding,
    cursor_default: config::CursorStyle,
    copy_on_select: bool,
    /// Pointer state shared with the element's per-frame event closures.
    mouse: Rc<RefCell<MouseState>>,
    focus: FocusHandle,
    /// Last vt title (OSC 0/2); `None` until the child sets one.
    title: Option<String>,
    /// Title fallback: the shell name.
    fallback: String,
    /// Set when BEL arrives. TODO: visual bell.
    pub bell: bool,
    /// True while a repaint is being withheld for synchronized output
    /// (?2026), with a safety timer armed to release it.
    sync_pending: bool,
    /// Active scrollback search, if the overlay is open.
    search: Option<Search>,
    /// Focus in/out listeners that drive focus reporting (?1004).
    _focus_subs: [Subscription; 2],
}

impl TerminalView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: Arc<Session>,
        colors: Rc<Colors>,
        font: gpui::Font,
        font_size: gpui::Pixels,
        cell: CellSize,
        pad: Padding,
        cursor_default: config::CursorStyle,
        copy_on_select: bool,
        fallback: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Install the colors the child can query via OSC 4/10/11/12.
        session.with_term(|term| term.set_report_colors(colors::report_colors(&colors)));
        let focus = cx.focus_handle();
        // Focus reporting (?1004): tell the child when this pane gains or
        // loses focus.
        let on_in = cx.weak_entity();
        let sub_in = window.on_focus_in(&focus, cx, move |_window, cx| {
            let _ = on_in.update(cx, |this, _| this.report_focus(true));
        });
        let on_out = cx.weak_entity();
        let sub_out = window.on_focus_out(&focus, cx, move |_event, _window, cx| {
            let _ = on_out.update(cx, |this, _| this.report_focus(false));
        });
        Self {
            session,
            colors,
            font,
            font_size,
            cell,
            pad,
            cursor_default,
            copy_on_select,
            mouse: Rc::new(RefCell::new(MouseState::default())),
            focus,
            title: None,
            fallback,
            bell: false,
            sync_pending: false,
            search: None,
            _focus_subs: [sub_in, sub_out],
        }
    }

    /// Emit a focus-in/out report to the child if it enabled ?1004, then
    /// flush the bytes to the pty.
    fn report_focus(&self, focused: bool) {
        let out = self.session.with_term(|term| {
            term.report_focus(focused);
            term.take_output()
        });
        if !out.is_empty() {
            let _ = self.session.write(&out);
        }
    }

    /// Current pane title for tab labels and the window title.
    pub fn title(&self) -> &str {
        label(self.title.as_deref(), &self.fallback)
    }

    /// Apply a reloaded appearance. A font/size change shifts the cell box,
    /// so the next layout pass re-grids and resizes the session; here we
    /// just swap the fields and repaint.
    pub fn set_appearance(&mut self, a: &Appearance, cx: &mut Context<Self>) {
        self.colors = a.colors.clone();
        self.font = a.font.clone();
        self.font_size = a.font_size;
        self.cell = a.cell;
        self.pad = a.pad;
        self.cursor_default = a.cursor_default;
        self.copy_on_select = a.copy_on_select;
        self.session
            .with_term(|term| term.set_report_colors(colors::report_colors(&self.colors)));
        cx.notify();
    }

    /// The pane's OSC 7 working directory report, if any.
    pub fn cwd(&self) -> Option<String> {
        self.session.with_term(|term| term.cwd().map(str::to_string))
    }

    /// Apply one session event; called from the bridge task.
    pub fn apply(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Wakeup => self.wakeup(cx),
            Event::TitleChanged(title) => {
                self.title = Some(title);
                cx.emit(ViewEvent::Title);
            }
            Event::Bell => self.bell = true,
            Event::Clipboard { data, .. } => {
                // OSC 52 write; macOS has no primary selection, so any kind
                // goes to the system clipboard.
                let text = String::from_utf8_lossy(&data).into_owned();
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            Event::Exit(_) => cx.emit(ViewEvent::Exited),
        }
    }

    /// Handle new child output. While the program holds synchronized output
    /// (?2026), withhold the repaint so the frame lands atomically, but arm
    /// a short safety timer so a program that never clears ?2026 can't
    /// freeze the view.
    fn wakeup(&mut self, cx: &mut Context<Self>) {
        if self.session.with_term(|t| t.synchronized_output()) {
            if !self.sync_pending {
                self.sync_pending = true;
                let timer = cx.background_executor().timer(SYNC_TIMEOUT);
                cx.spawn(async move |this, cx| {
                    timer.await;
                    let _ = this.update(cx, |this, cx| {
                        if this.sync_pending {
                            this.sync_pending = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            return;
        }
        self.sync_pending = false;
        cx.notify();
    }

    /// Open/close the scrollback search overlay.
    pub fn toggle_search(&mut self, cx: &mut Context<Self>) {
        self.search = match self.search {
            Some(_) => None,
            None => Some(Search { edit: crate::textedit::TextEdit::new(""), current: 0 }),
        };
        cx.notify();
    }

    /// Current search results against the live buffer.
    fn search_matches(&self) -> Vec<vt::Match> {
        match &self.search {
            Some(s) => {
                let q = s.edit.text();
                if q.is_empty() {
                    Vec::new()
                } else {
                    self.session.with_term(|t| t.search(&q, false))
                }
            }
            None => Vec::new(),
        }
    }

    /// Clamp the focused match and scroll it into view.
    fn search_jump(&mut self, cx: &mut Context<Self>) {
        let matches = self.search_matches();
        let Some(s) = self.search.as_mut() else { return };
        if matches.is_empty() {
            cx.notify();
            return;
        }
        s.current = s.current.min(matches.len() - 1);
        let line = matches[s.current].line;
        self.session.with_term(|t| {
            let sb = t.grid().scrollback().len();
            t.set_display_offset(sb.saturating_sub(line));
        });
        cx.notify();
    }

    /// Move the focused match by `delta`, wrapping.
    fn search_step(&mut self, delta: i64, cx: &mut Context<Self>) {
        let len = self.search_matches().len() as i64;
        if len == 0 {
            cx.notify();
            return;
        }
        if let Some(s) = self.search.as_mut() {
            s.current = (((s.current as i64 + delta) % len + len) % len) as usize;
        }
        self.search_jump(cx);
    }

    /// Handle a keystroke while the search overlay is open.
    fn search_key(&mut self, ks: &gpui::Keystroke, mods: input::Mods, cx: &mut Context<Self>) {
        if mods.cmd {
            return; // leave cmd chords (incl. toggle) to the action system
        }
        match ks.key.as_str() {
            "escape" => {
                self.search = None;
                cx.notify();
            }
            "enter" | "down" => self.search_step(1, cx),
            "up" => self.search_step(-1, cx),
            "left" => {
                if let Some(s) = self.search.as_mut() {
                    s.edit.left();
                }
                cx.notify();
            }
            "right" => {
                if let Some(s) = self.search.as_mut() {
                    s.edit.right();
                }
                cx.notify();
            }
            "home" => {
                if let Some(s) = self.search.as_mut() {
                    s.edit.home();
                }
                cx.notify();
            }
            "end" => {
                if let Some(s) = self.search.as_mut() {
                    s.edit.end();
                }
                cx.notify();
            }
            "backspace" | "delete" => {
                if let Some(s) = self.search.as_mut() {
                    if ks.key == "backspace" {
                        s.edit.backspace();
                    } else {
                        s.edit.delete();
                    }
                    s.current = 0;
                }
                self.search_jump(cx);
            }
            _ => {
                let text = ks
                    .key_char
                    .as_deref()
                    .filter(|t| !t.is_empty() && !mods.ctrl && !mods.alt);
                if let Some(text) = text {
                    if let Some(s) = self.search.as_mut() {
                        s.edit.insert(text);
                        s.current = 0;
                    }
                    self.search_jump(cx);
                }
            }
        }
    }

    /// The floating search overlay (bottom-right), with a caret in the query.
    fn search_bar(&self, before: &str, after: &str, pos: usize, total: usize) -> impl IntoElement {
        let mut caret = colors::hsla(self.colors.cursor);
        caret.a = 0.9;
        div()
            .absolute()
            .bottom(px(8.0))
            .right(px(8.0))
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .bg(colors::rgba(self.colors.selection_bg))
            .text_color(colors::rgba(self.colors.selection_fg))
            .text_size(self.font_size)
            .child(SharedString::from("\u{2315} "))
            .child(SharedString::from(before.to_string()))
            .child(div().w(px(1.0)).h(px(14.0)).bg(caret))
            .child(SharedString::from(after.to_string()))
            .child(SharedString::from(format!("    {pos}/{total}")))
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let mods = input::Mods {
            shift: keystroke.modifiers.shift,
            alt: keystroke.modifiers.alt,
            ctrl: keystroke.modifiers.control,
            cmd: keystroke.modifiers.platform,
        };
        if self.search.is_some() {
            self.search_key(keystroke, mods, cx);
            cx.stop_propagation();
            return;
        }
        let state = self.session.with_term(|term| input::TermState {
            cursor_keys_app: term.cursor_keys_app(),
            keypad_app: term.keypad_app(),
            bracketed_paste: term.bracketed_paste(),
            kitty_flags: term.kitty_keyboard_flags(),
        });
        let text = keystroke.key_char.as_deref();
        if let Some(bytes) = input::encode_key(&keystroke.key, text, mods, state) {
            self.scroll_to_bottom(cx);
            let _ = self.session.write(&bytes);
            cx.stop_propagation();
        }
    }

    /// Any write to the pty snaps the view back to the live bottom.
    fn scroll_to_bottom(&self, cx: &mut Context<Self>) {
        let was_back = self.session.with_term(|term| {
            let back = term.display_offset() != 0;
            term.set_display_offset(0);
            back
        });
        if was_back {
            cx.notify();
        }
    }

    /// Copy the current selection to the clipboard, if any.
    pub fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.session.with_term(|term| term.selection_text()) else {
            return;
        };
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Paste the clipboard into the pty (bracketed when the app requested it).
    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let bracketed = self.session.with_term(|term| term.bracketed_paste());
        self.scroll_to_bottom(cx);
        let _ = self.session.write(&input::encode_paste(&text, bracketed));
    }

    /// Scroll the viewport by `delta` rows into (positive) or out of
    /// (negative) scrollback history.
    pub fn scroll_lines(&mut self, delta: isize, cx: &mut Context<Self>) {
        let moved = self.session.with_term(|term| {
            let before = term.display_offset();
            term.scroll_display(delta);
            term.display_offset() != before
        });
        if moved {
            cx.notify();
        }
    }

    /// Scroll by whole pages (the pane's row count), sign as in
    /// [`Self::scroll_lines`].
    pub fn scroll_pages(&mut self, pages: isize, cx: &mut Context<Self>) {
        let rows = self.session.with_term(|term| term.rows()) as isize;
        self.scroll_lines(pages * rows.max(1), cx);
    }

    /// Jump to the oldest scrollback line.
    pub fn scroll_to_top(&mut self, cx: &mut Context<Self>) {
        let moved = self.session.with_term(|term| {
            let max = term.grid().scrollback().len();
            let before = term.display_offset();
            term.set_display_offset(max);
            term.display_offset() != before
        });
        if moved {
            cx.notify();
        }
    }

    /// Jump to the live bottom (alias of the input scroll-to-bottom path).
    pub fn scroll_to_live(&mut self, cx: &mut Context<Self>) {
        self.scroll_to_bottom(cx);
    }

    /// Move the viewport by `delta` shell prompts (OSC 133;A marks);
    /// negative scrolls toward older prompts.
    pub fn jump_prompt(&mut self, delta: i32, cx: &mut Context<Self>) {
        if delta == 0 {
            return;
        }
        let moved = self.session.with_term(|term| {
            let prompts = term.prompt_lines();
            if prompts.is_empty() {
                return false;
            }
            let sb = term.grid().scrollback().len();
            let mut top = sb - term.display_offset().min(sb);
            let mut changed = false;
            for _ in 0..delta.unsigned_abs() {
                let next = if delta < 0 {
                    prompts.iter().rev().find(|&&p| p < top).copied()
                } else {
                    prompts.iter().find(|&&p| p > top).copied()
                };
                match next {
                    Some(p) => {
                        top = p;
                        changed = true;
                    }
                    None => break,
                }
            }
            if changed {
                term.set_display_offset(sb.saturating_sub(top));
            }
            changed
        });
        if moved {
            cx.notify();
        }
    }

    /// Clear the visible screen the way most terminals' "clear" does: send
    /// a form feed so the shell redraws its prompt at the top.
    pub fn clear_screen(&mut self, cx: &mut Context<Self>) {
        self.scroll_to_bottom(cx);
        let _ = self.session.write(b"\x0c");
    }
}

impl EventEmitter<ViewEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.as_ref().map(|s| crate::element::SearchQuery {
            query: s.edit.text(),
            current: s.current,
        });
        let bar = self.search.as_ref().map(|s| {
            let total = self.search_matches().len();
            let pos = if total == 0 { 0 } else { s.current + 1 };
            let (before, after) = s.edit.split();
            self.search_bar(&before, &after, pos, total)
        });
        div()
            .relative()
            .size_full()
            .bg(colors::rgba(self.colors.bg))
            .key_context("Terminal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .child(TerminalElement::new(
                self.session.clone(),
                self.colors.clone(),
                self.font.clone(),
                self.font_size,
                self.cell,
                self.pad,
                self.cursor_default,
                self.mouse.clone(),
                self.copy_on_select,
                query,
            ))
            .children(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_prefers_nonblank_title() {
        assert_eq!(label(Some("vim"), "zsh"), "vim");
        assert_eq!(label(Some(""), "zsh"), "zsh");
        assert_eq!(label(Some("   "), "zsh"), "zsh");
        assert_eq!(label(None, "zsh"), "zsh");
    }
}
