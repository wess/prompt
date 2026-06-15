//! Workspace root: ordered tabs of split panes, one shell per pane.
//!
//! Owns the `workspace::Tabs` model and a map from pane id to terminal
//! view entity. All tab/split mutations funnel through here; the panes
//! themselves only know their own session.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use config::{Action, Keybind, SplitDirection, SplitFocus};
use futures::StreamExt;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable as _, KeyBinding, KeyDownEvent, Menu,
    MenuItem, MouseButton, SharedString, Subscription, WeakEntity, Window, div, px,
};
use terminal::Session;
use workspace::{Axis, Direction, PaneId, PaneIds, Rect, SplitId, Tabs};

use crate::bridge;
use crate::colors::{self, Colors};
use crate::keys;
use crate::metrics::{CellSize, Padding};
use crate::session;
use crate::splits::{self, Drag, SplitsElement};
use crate::tabbar;
use crate::textedit;
use crate::view::{TerminalView, ViewEvent};

/// One keybind dispatch: the index into the workspace's resolved keybind
/// table. A single action type keeps every binding flowing through one
/// handler regardless of which config action it carries.
#[derive(Clone, PartialEq, Default, Debug, gpui::Action)]
#[action(namespace = prompt, no_json)]
pub struct RunBind(pub usize);

/// A change the settings panel can make. Each maps to one config key that
/// is written back to the config file and live-reloaded.
#[derive(Clone, Copy)]
enum Setting {
    ThemeCycle(i32),
    FontSize(i32),
    CursorStyleCycle,
    FontStyleCycle,
    PaddingX(i32),
    PaddingY(i32),
    Scrollback(i32),
    ToggleCopyOnSelect,
}

/// A free-text setting edited via the in-panel text field.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsField {
    FontFamily,
    Shell,
    Foreground,
    Background,
}

impl SettingsField {
    fn key(self) -> &'static str {
        match self {
            SettingsField::FontFamily => "font-family",
            SettingsField::Shell => "shell",
            SettingsField::Foreground => "foreground",
            SettingsField::Background => "background",
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingsField::FontFamily => "Font family",
            SettingsField::Shell => "Shell",
            SettingsField::Foreground => "Foreground",
            SettingsField::Background => "Background",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            SettingsField::FontFamily => "(default)",
            SettingsField::Shell => "(login shell)",
            SettingsField::Foreground | SettingsField::Background => "(theme)",
        }
    }
}

/// Grid for a fresh pane until its first layout pass resizes it.
const SPAWN_COLS: usize = 80;
const SPAWN_ROWS: usize = 24;

struct Pane {
    view: Entity<TerminalView>,
    _subscription: Subscription,
}

pub struct WorkspaceView {
    opts: config::Options,
    colors: Rc<Colors>,
    font: gpui::Font,
    font_size: gpui::Pixels,
    cell: CellSize,
    pad: Padding,
    tabs: Tabs,
    ids: PaneIds,
    panes: HashMap<PaneId, Pane>,
    /// Divider drag in progress, shared with the splits element.
    drag: Rc<RefCell<Option<Drag>>>,
    /// Resolved keybindings (defaults + user config); `RunBind` indexes here.
    keybinds: Vec<Keybind>,
    /// Configured font size, restored by `reset_font_size`.
    base_font_size: gpui::Pixels,
    /// Whether the settings panel is open.
    settings_open: bool,
    /// In-progress edit of a free-text setting (field + buffer).
    editing: Option<(SettingsField, textedit::TextEdit)>,
    /// Focus target that captures keys while a settings field is edited.
    settings_focus: FocusHandle,
    /// Config-file watcher; kept alive so live reload keeps working.
    _watch: Option<config::WatchHandle>,
}

impl WorkspaceView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        opts: config::Options,
        colors: Rc<Colors>,
        font: gpui::Font,
        font_size: gpui::Pixels,
        cell: CellSize,
        pad: Padding,
        cols: usize,
        rows: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (keybinds, diags) = config::resolve(&opts.keybind);
        for d in &diags {
            eprintln!("prompt: {}: {}", d.key, d.message);
        }
        let mut this = Self {
            base_font_size: font_size,
            settings_open: false,
            editing: None,
            settings_focus: cx.focus_handle(),
            opts,
            colors,
            font,
            font_size,
            cell,
            pad,
            // Temporary: replaced right below with the first real pane.
            tabs: Tabs::new(PaneIds::new().next()),
            ids: PaneIds::new(),
            panes: HashMap::new(),
            drag: Rc::new(RefCell::new(None)),
            keybinds,
            _watch: None,
        };
        this.applykeybinds(cx);
        this.setmenus(cx);
        let options = session::options(&this.opts, cols, rows, None);
        let Some(id) = this.spawn(options, window, cx) else {
            std::process::exit(1);
        };
        this.tabs = Tabs::new(id);
        this.focusactive(window, cx);
        this.startwatch(window, cx);
        this
    }

    /// (Re)bind every resolved keybind to a [`RunBind`] carrying its table
    /// index. Triggers with no gpui spelling are skipped. Called at startup
    /// and after a live reload.
    fn applykeybinds(&self, cx: &mut Context<Self>) {
        cx.clear_key_bindings();
        let mut bindings = Vec::new();
        for (i, kb) in self.keybinds.iter().enumerate() {
            let Some(ks) = keys::keystroke(kb.mods, &kb.key) else {
                continue;
            };
            if gpui::Keystroke::parse(&ks).is_err() {
                continue;
            }
            bindings.push(KeyBinding::new(&ks, RunBind(i), Some("Workspace")));
        }
        cx.bind_keys(bindings);
    }

    /// A native menu item driving the same action as the keybind for
    /// `action`; gpui shows that binding's shortcut automatically. `None`
    /// when nothing is bound to the action (so it would have no shortcut and
    /// no dispatch path).
    fn menu_item(&self, label: &str, action: Action) -> Option<MenuItem> {
        let index = self.keybinds.iter().position(|k| k.action == action)?;
        Some(MenuItem::action(label.to_string(), RunBind(index)))
    }

    /// Install the native application menu bar (macOS). Items reuse the
    /// config-driven actions, so the menu and keymap never drift. Re-run
    /// after a reload since keybind indices may change.
    fn setmenus(&self, cx: &mut Context<Self>) {
        let menu = |name: &str, items: Vec<Option<MenuItem>>| Menu {
            name: name.to_string().into(),
            items: items.into_iter().flatten().collect(),
            disabled: false,
        };
        let sep = || Some(MenuItem::separator());
        cx.set_menus(vec![
            menu(
                "Prompt",
                vec![
                    self.menu_item("Reload Config", Action::ReloadConfig),
                    sep(),
                    self.menu_item("Quit Prompt", Action::Quit),
                ],
            ),
            menu(
                "Shell",
                vec![
                    self.menu_item("New Tab", Action::NewTab),
                    self.menu_item("Split Right", Action::NewSplit(SplitDirection::Right)),
                    self.menu_item("Split Down", Action::NewSplit(SplitDirection::Down)),
                    sep(),
                    self.menu_item("Close", Action::CloseSurface),
                ],
            ),
            menu(
                "Edit",
                vec![
                    self.menu_item("Copy", Action::Copy),
                    self.menu_item("Paste", Action::Paste),
                    sep(),
                    self.menu_item("Find\u{2026}", Action::ToggleSearch),
                ],
            ),
            menu(
                "View",
                vec![
                    self.menu_item("Increase Font Size", Action::IncreaseFontSize(1.0)),
                    self.menu_item("Decrease Font Size", Action::DecreaseFontSize(1.0)),
                    self.menu_item("Reset Font Size", Action::ResetFontSize),
                    sep(),
                    self.menu_item("Clear Screen", Action::ClearScreen),
                    self.menu_item("Jump to Previous Prompt", Action::JumpToPrompt(-1)),
                    self.menu_item("Jump to Next Prompt", Action::JumpToPrompt(1)),
                ],
            ),
            menu(
                "Window",
                vec![
                    self.menu_item("Previous Tab", Action::PreviousTab),
                    self.menu_item("Next Tab", Action::NextTab),
                ],
            ),
        ]);
    }

    /// Watch the config file and reload appearance on every edit.
    fn startwatch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((handle, mut changes)) = crate::reload::watch() else {
            return;
        };
        self._watch = Some(handle);
        let weak = cx.weak_entity();
        window
            .spawn(cx, async move |cx| {
                while changes.next().await.is_some() {
                    if weak.update(cx, |this, cx| this.reload(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
    }

    /// Re-read the config file and apply everything that can change at
    /// runtime: theme/colors, font family/size, padding, cursor style,
    /// copy-on-select. Shell, scrollback and window size only affect new
    /// sessions or need a restart, matching Ghostty.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let (opts, diagnostics) = config::load();
        for d in &diagnostics {
            eprintln!("prompt: config line {}: {} ({})", d.line, d.message, d.key);
        }
        self.colors = Rc::new(colors::from_config(&opts));
        self.font = crate::font::build(&opts);
        self.font_size = px(opts.font_size.max(1.0));
        self.cell = crate::metrics::measure(cx.text_system(), &self.font, self.font_size);
        self.pad = Padding {
            x: opts.window_padding_x as f32,
            y: opts.window_padding_y as f32,
        };
        self.base_font_size = self.font_size;
        self.opts = opts;
        let (keybinds, diags) = config::resolve(&self.opts.keybind);
        for d in &diags {
            eprintln!("prompt: {}: {}", d.key, d.message);
        }
        self.keybinds = keybinds;
        self.applykeybinds(cx);
        self.setmenus(cx);
        self.pushappearance(cx);
        cx.notify();
    }

    /// Push the current appearance to every pane.
    fn pushappearance(&self, cx: &mut Context<Self>) {
        let appearance = crate::view::Appearance {
            colors: self.colors.clone(),
            font: self.font.clone(),
            font_size: self.font_size,
            cell: self.cell,
            pad: self.pad,
            cursor_default: self.opts.cursor_style,
            copy_on_select: self.opts.copy_on_select,
        };
        for pane in self.panes.values() {
            pane.view.update(cx, |view, cx| view.set_appearance(&appearance, cx));
        }
    }

    /// Re-measure the cell box for the current font size and republish.
    fn setfontsize(&mut self, size: gpui::Pixels, cx: &mut Context<Self>) {
        let size = px(f32::from(size).max(1.0));
        if size == self.font_size {
            return;
        }
        self.font_size = size;
        self.cell = crate::metrics::measure(cx.text_system(), &self.font, self.font_size);
        self.pushappearance(cx);
        cx.notify();
    }

    /// Spawn a session, wrap it in a pane view, wire its event bridge and
    /// subscription, and register it. `None` if the shell failed to spawn.
    fn spawn(
        &mut self,
        options: terminal::SessionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<PaneId> {
        let (session, events) = match Session::spawn(options) {
            Ok(pair) => pair,
            Err(error) => {
                eprintln!("prompt: failed to spawn shell: {error}");
                return None;
            }
        };
        let session = Arc::new(session);
        let fallback = session::shellname(self.opts.shell.as_deref());
        let view = cx.new(|cx| {
            TerminalView::new(
                session,
                self.colors.clone(),
                self.font.clone(),
                self.font_size,
                self.cell,
                self.pad,
                self.opts.cursor_style,
                self.opts.copy_on_select,
                fallback,
                window,
                cx,
            )
        });

        // Pump session events into the pane view on the foreground.
        let weak = view.downgrade();
        let mut events = bridge::forward(events);
        window
            .spawn(cx, async move |cx| {
                while let Some(event) = events.next().await {
                    if weak.update(cx, |view, cx| view.apply(event, cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();

        let id = self.ids.next();
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |this: &mut Self, _view, event: &ViewEvent, window, cx| {
                this.paneevent(id, event, window, cx);
            },
        );
        self.panes.insert(id, Pane { view, _subscription: subscription });
        Some(id)
    }

    /// Spawn a pane inheriting the focused pane's working directory.
    fn spawnpane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<PaneId> {
        let inherit = self
            .panes
            .get(&self.tabs.focused())
            .and_then(|pane| pane.view.read(cx).cwd())
            .and_then(|osc| session::cwdpath(&osc));
        let options = session::options(&self.opts, SPAWN_COLS, SPAWN_ROWS, inherit);
        self.spawn(options, window, cx)
    }

    fn paneevent(
        &mut self,
        pane: PaneId,
        event: &ViewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ViewEvent::Title => {
                if pane == self.tabs.focused() {
                    self.settitle(window, cx);
                }
                cx.notify(); // tab labels
            }
            ViewEvent::Exited => self.closepane(pane, window, cx),
        }
    }

    /// Close one pane: collapse its split, or close its tab when it is the
    /// last pane there, or quit when it is the last pane of the last tab.
    fn closepane(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.tabindex(pane) else {
            return;
        };
        let lastpane = self.tabs.get(index).expect("tab").tree.panes().len() == 1;
        if lastpane && self.tabs.len() == 1 {
            cx.quit();
            return;
        }
        if lastpane {
            self.tabs.close_tab(index);
        } else {
            // Mutations go through the active tab; visit and restore.
            let previous = self.tabs.active_index();
            self.tabs.activate(index);
            let next = (self.tabs.focused() == pane)
                .then(|| workspace::next(&self.tabs.active().tree, pane))
                .flatten();
            self.tabs.active_mut().tree.remove(pane);
            if let Some(next) = next {
                self.tabs.focus(next);
            }
            self.tabs.activate(previous);
        }
        self.panes.remove(&pane);
        self.focusactive(window, cx);
        cx.notify();
    }

    /// Close a whole tab (tab-bar close glyph), dropping all its panes.
    pub fn closetab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let removed = tab.tree.panes();
        if self.tabs.len() == 1 {
            cx.quit();
            return;
        }
        self.tabs.close_tab(index);
        for pane in removed {
            self.panes.remove(&pane);
        }
        self.focusactive(window, cx);
        cx.notify();
    }

    pub fn activatetab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.activate(index) {
            self.focusactive(window, cx);
            cx.notify();
        }
    }

    pub fn focuspane(&mut self, pane: PaneId, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.focus(pane) {
            self.focusactive(window, cx);
            cx.notify();
        }
    }

    /// Set a divider's ratio in the active tab (divider drag).
    pub fn setratio(&mut self, split: SplitId, ratio: f32, cx: &mut Context<Self>) {
        if self.tabs.active_mut().tree.set_ratio(split, ratio) {
            cx.notify();
        }
    }

    /// Split the focused pane. `first` places the new pane before the
    /// existing one (left/up) instead of after it (right/down).
    fn split(&mut self, axis: Axis, first: bool, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.tabs.focused();
        let Some(id) = self.spawnpane(window, cx) else {
            return;
        };
        if self.tabs.active_mut().tree.split(target, axis, id, first).is_none() {
            self.panes.remove(&id);
            return;
        }
        self.tabs.focus(id);
        self.focusactive(window, cx);
        cx.notify();
    }

    fn focusdir(&mut self, direction: Direction, window: &mut Window, cx: &mut Context<Self>) {
        // Directional nav only needs relative geometry, so the viewport
        // rect is close enough without the exact splits bounds.
        let viewport = window.viewport_size();
        let rect = Rect::new(
            0.0,
            0.0,
            f32::from(viewport.width).max(1.0),
            f32::from(viewport.height).max(1.0),
        );
        let layout = workspace::compute_layout(&self.tabs.active().tree, rect, splits::DIVIDER);
        if let Some(next) = workspace::neighbor(&layout, self.tabs.focused(), direction) {
            self.focuspane(next, window, cx);
        }
    }

    /// Move window focus to the active tab's focused pane and retitle.
    fn focusactive(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.panes.get(&self.tabs.focused()) {
            window.focus(&pane.view.focus_handle(cx), cx);
        }
        self.settitle(window, cx);
    }

    fn settitle(&self, window: &mut Window, cx: &App) {
        let title = self
            .panes
            .get(&self.tabs.focused())
            .map(|pane| pane.view.read(cx).title().to_string())
            .unwrap_or_else(|| "prompt".to_string());
        window.set_window_title(&title);
    }

    fn tabindex(&self, pane: PaneId) -> Option<usize> {
        (0..self.tabs.len()).find(|i| self.tabs.get(*i).is_some_and(|t| t.tree.contains(pane)))
    }

    /// One label per tab: its focused pane's title.
    fn titles(&self, cx: &App) -> Vec<String> {
        (0..self.tabs.len())
            .map(|i| {
                let tab = self.tabs.get(i).expect("tab index");
                self.panes
                    .get(&tab.focused)
                    .map(|pane| pane.view.read(cx).title().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn newtab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.spawnpane(window, cx) {
            self.tabs.new_tab(id);
            self.focusactive(window, cx);
            cx.notify();
        }
    }

    /// Cycle focus to the previous/next pane in the active tab's layout.
    fn cyclesplit(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.tabs.focused();
        let tree = &self.tabs.active().tree;
        let next = if forward {
            workspace::next(tree, focused)
        } else {
            workspace::prev(tree, focused)
        };
        if let Some(next) = next {
            self.focuspane(next, window, cx);
        }
    }

    /// Activate a 1-based tab index; negative counts from the end.
    fn gototab(&mut self, n: i32, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.tabs.len() as i32;
        let index = if n < 0 { len + n } else { n - 1 };
        if (0..len).contains(&index) {
            self.activatetab(index as usize, window, cx);
        }
    }

    /// Reorder the active tab by a signed delta, clamped to the ends.
    fn movetab(&mut self, delta: i32, cx: &mut Context<Self>) {
        let from = self.tabs.active_index();
        let len = self.tabs.len() as i32;
        let to = (from as i32 + delta).clamp(0, len - 1) as usize;
        if self.tabs.move_tab(from, to) {
            cx.notify();
        }
    }

    /// Run something on the focused pane's view.
    fn onfocused(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut TerminalView, &mut Context<TerminalView>)) {
        if let Some(pane) = self.panes.get(&self.tabs.focused()) {
            pane.view.update(cx, |view, cx| f(view, cx));
        }
    }

    /// Dispatch handler shared by every keybinding.
    fn runbind(&mut self, action: &RunBind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kb) = self.keybinds.get(action.0) else {
            return;
        };
        self.dispatch(kb.action, window, cx);
    }

    /// Carry out one config action.
    fn dispatch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::NewTab => self.newtab(window, cx),
            Action::CloseSurface => self.closepane(self.tabs.focused(), window, cx),
            Action::NewSplit(dir) => {
                let (axis, first) = match dir {
                    SplitDirection::Right => (Axis::Horizontal, false),
                    SplitDirection::Left => (Axis::Horizontal, true),
                    SplitDirection::Down => (Axis::Vertical, false),
                    SplitDirection::Up => (Axis::Vertical, true),
                };
                self.split(axis, first, window, cx);
            }
            Action::GotoSplit(focus) => match focus {
                SplitFocus::Previous => self.cyclesplit(false, window, cx),
                SplitFocus::Next => self.cyclesplit(true, window, cx),
                SplitFocus::Up => self.focusdir(Direction::Up, window, cx),
                SplitFocus::Down => self.focusdir(Direction::Down, window, cx),
                SplitFocus::Left => self.focusdir(Direction::Left, window, cx),
                SplitFocus::Right => self.focusdir(Direction::Right, window, cx),
            },
            Action::GotoTab(n) => self.gototab(n, window, cx),
            Action::PreviousTab => {
                self.tabs.activate_prev();
                self.focusactive(window, cx);
                cx.notify();
            }
            Action::NextTab => {
                self.tabs.activate_next();
                self.focusactive(window, cx);
                cx.notify();
            }
            Action::MoveTab(delta) => self.movetab(delta, cx),
            Action::Copy => self.onfocused(cx, |v, cx| v.copy_selection(cx)),
            Action::Paste => self.onfocused(cx, |v, cx| v.paste_clipboard(cx)),
            Action::IncreaseFontSize(amount) => {
                self.setfontsize(px(f32::from(self.font_size) + amount), cx)
            }
            Action::DecreaseFontSize(amount) => {
                self.setfontsize(px(f32::from(self.font_size) - amount), cx)
            }
            Action::ResetFontSize => self.setfontsize(self.base_font_size, cx),
            Action::ScrollPageUp => self.onfocused(cx, |v, cx| v.scroll_pages(1, cx)),
            Action::ScrollPageDown => self.onfocused(cx, |v, cx| v.scroll_pages(-1, cx)),
            Action::ScrollToTop => self.onfocused(cx, |v, cx| v.scroll_to_top(cx)),
            Action::ScrollToBottom => self.onfocused(cx, |v, cx| v.scroll_to_live(cx)),
            Action::JumpToPrompt(delta) => self.onfocused(cx, |v, cx| v.jump_prompt(delta, cx)),
            Action::ClearScreen => self.onfocused(cx, |v, cx| v.clear_screen(cx)),
            Action::ToggleSearch => self.onfocused(cx, |v, cx| v.toggle_search(cx)),
            Action::ToggleSettings => self.toggle_settings(window, cx),
            Action::ReloadConfig => self.reload(cx),
            Action::ToggleFullscreen => window.toggle_fullscreen(),
            Action::Quit => cx.quit(),
            Action::Unbound => {}
        }
    }

    /// Open/close the settings panel. Closing returns focus to the pane.
    fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        self.editing = None;
        if !self.settings_open {
            self.focusactive(window, cx);
        }
        cx.notify();
    }

    /// The current config value backing a free-text field.
    fn field_value(&self, field: SettingsField) -> String {
        match field {
            SettingsField::FontFamily => match self.opts.font_family.first() {
                Some(f) => f.clone(),
                None => String::new(),
            },
            SettingsField::Shell => self.opts.shell.clone().unwrap_or_default(),
            SettingsField::Foreground => self.opts.foreground.clone().unwrap_or_default(),
            SettingsField::Background => self.opts.background.clone().unwrap_or_default(),
        }
    }

    /// Begin editing a free-text field; focus moves to capture keys.
    fn start_edit(&mut self, field: SettingsField, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.field_value(field);
        self.editing = Some((field, textedit::TextEdit::new(&current)));
        window.focus(&self.settings_focus, cx);
        cx.notify();
    }

    /// Commit the edited field to the config file and reload.
    fn commit_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((field, edit)) = self.editing.take() {
            self.write_config(field.key(), edit.text().trim());
            self.reload(cx);
        }
        // Return focus to the active pane.
        self.focusactive(window, cx);
        cx.notify();
    }

    /// Keys typed while a settings field is focused.
    fn settings_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_none() {
            return;
        }
        let ks = &event.keystroke;
        // Leave platform/ctrl chords to the action system (e.g. cmd+, closes).
        if ks.modifiers.platform || ks.modifiers.control {
            return;
        }
        match ks.key.as_str() {
            "enter" => {
                self.commit_edit(window, cx);
                cx.stop_propagation();
                return;
            }
            "escape" => {
                self.editing = None;
                self.focusactive(window, cx);
                cx.notify();
                cx.stop_propagation();
                return;
            }
            other => {
                if let Some((_, edit)) = self.editing.as_mut() {
                    match other {
                        "backspace" => {
                            edit.backspace();
                        }
                        "delete" => {
                            edit.delete();
                        }
                        "left" => edit.left(),
                        "right" => edit.right(),
                        "home" => edit.home(),
                        "end" => edit.end(),
                        _ => {
                            let text = ks
                                .key_char
                                .as_deref()
                                .filter(|t| !t.is_empty() && !ks.modifiers.alt);
                            if let Some(text) = text {
                                edit.insert(text);
                            }
                        }
                    }
                }
            }
        }
        cx.notify();
        cx.stop_propagation();
    }

    /// One editable free-text row: clicking it starts editing; while active
    /// it shows the live buffer with a caret.
    fn text_row(&self, field: SettingsField, cx: &mut Context<Self>) -> impl IntoElement {
        let active = matches!(&self.editing, Some((f, _)) if *f == field);
        let mut border = colors::hsla(self.colors.fg);
        border.a = if active { 0.6 } else { 0.25 };

        let mut boxed = div()
            .flex()
            .items_center()
            .min_w(px(220.0))
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .border_1()
            .border_color(border);

        if active {
            let (before, after) = self.editing.as_ref().expect("active").1.split();
            let mut caret = colors::hsla(self.colors.cursor);
            caret.a = 0.9;
            boxed = boxed
                .text_color(colors::rgba(self.colors.fg))
                .child(SharedString::from(before))
                .child(div().w(px(1.0)).h(px(16.0)).bg(caret))
                .child(SharedString::from(after));
        } else {
            let current = self.field_value(field);
            let (text, mut color) = if current.is_empty() {
                (field.placeholder().to_string(), colors::hsla(self.colors.fg))
            } else {
                (current, colors::hsla(self.colors.fg))
            };
            if self.field_value(field).is_empty() {
                color.a = 0.4;
            }
            boxed = boxed.text_color(color).child(SharedString::from(text)).on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, window, cx| {
                    this.start_edit(field, window, cx);
                    cx.stop_propagation();
                }),
            );
        }
        self.srow(field.label(), boxed)
    }

    /// Persist one `key = value` into the config file (preserving the rest),
    /// then reload so the change applies immediately. No-op without a config
    /// path.
    fn write_config(&self, key: &str, value: &str) {
        let Some(path) = config::default_path() else {
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let updated = config::upsert(&text, key, value);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, updated);
    }

    /// Apply one settings-panel change: compute the new value from the
    /// current options, write it to the config file, and reload.
    fn apply_setting(&mut self, setting: Setting, cx: &mut Context<Self>) {
        let o = &self.opts;
        let (key, value): (&str, String) = match setting {
            Setting::ThemeCycle(dir) => {
                let names = theme::names();
                let cur = names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(o.theme.trim()))
                    .unwrap_or(0) as i32;
                let n = names.len() as i32;
                let idx = (((cur + dir) % n + n) % n) as usize;
                ("theme", names[idx].to_string())
            }
            Setting::FontSize(d) => {
                let v = (o.font_size + d as f32).clamp(6.0, 72.0);
                ("font-size", format!("{v}"))
            }
            Setting::CursorStyleCycle => {
                let next = match o.cursor_style {
                    config::CursorStyle::Block => "bar",
                    config::CursorStyle::Bar => "underline",
                    config::CursorStyle::Underline => "block",
                };
                ("cursor-style", next.to_string())
            }
            Setting::FontStyleCycle => {
                let next = match o.font_style {
                    config::FontStyle::Normal => "bold",
                    config::FontStyle::Bold => "italic",
                    config::FontStyle::Italic => "bold-italic",
                    config::FontStyle::BoldItalic => "normal",
                };
                ("font-style", next.to_string())
            }
            Setting::PaddingX(d) => (
                "window-padding-x",
                (o.window_padding_x as i32 + d).max(0).to_string(),
            ),
            Setting::PaddingY(d) => (
                "window-padding-y",
                (o.window_padding_y as i32 + d).max(0).to_string(),
            ),
            Setting::Scrollback(d) => (
                "scrollback-limit",
                (o.scrollback_limit as i64 + d as i64).max(0).to_string(),
            ),
            Setting::ToggleCopyOnSelect => {
                ("copy-on-select", (!o.copy_on_select).to_string())
            }
        };
        self.write_config(key, &value);
        self.reload(cx);
    }

    /// A small clickable chip that applies `setting` when pressed.
    fn chip(
        &self,
        label: impl Into<SharedString>,
        setting: Setting,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors::rgba(self.colors.selection_bg))
            .text_color(colors::rgba(self.colors.selection_fg))
            .child(label.into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    this.apply_setting(setting, cx);
                    cx.stop_propagation();
                }),
            )
    }

    /// One labeled settings row: label on the left, controls on the right.
    fn srow(&self, label: &str, control: impl IntoElement) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .items_center()
            .w_full()
            .py_1()
            .child(
                div()
                    .text_color(colors::rgba(self.colors.fg))
                    .child(SharedString::from(label.to_string())),
            )
            .child(control)
    }

    /// A `‹ value ›` cycle control.
    fn cycle(
        &self,
        value: String,
        prev: Setting,
        next: Setting,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.chip("\u{2039}", prev, cx))
            .child(
                div()
                    .min_w(px(150.0))
                    .text_color(colors::rgba(self.colors.fg))
                    .child(SharedString::from(value)),
            )
            .child(self.chip("\u{203a}", next, cx))
    }

    /// A `− value +` stepper control.
    fn stepper(
        &self,
        value: String,
        dec: Setting,
        inc: Setting,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.chip("\u{2212}", dec, cx))
            .child(
                div()
                    .min_w(px(60.0))
                    .text_color(colors::rgba(self.colors.fg))
                    .child(SharedString::from(value)),
            )
            .child(self.chip("+", inc, cx))
    }

    /// The settings panel overlay.
    fn settings_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let o = &self.opts;
        let theme = if o.theme.trim().is_empty() {
            "default".to_string()
        } else {
            o.theme.clone()
        };
        let cursor = match o.cursor_style {
            config::CursorStyle::Block => "block",
            config::CursorStyle::Bar => "bar",
            config::CursorStyle::Underline => "underline",
        };
        let fstyle = match o.font_style {
            config::FontStyle::Normal => "normal",
            config::FontStyle::Bold => "bold",
            config::FontStyle::Italic => "italic",
            config::FontStyle::BoldItalic => "bold-italic",
        };
        let mut border = colors::hsla(self.colors.fg);
        border.a = 0.2;

        let panel = div()
            .w(px(460.0))
            .bg(colors::rgba(self.colors.bg))
            .border_1()
            .border_color(border)
            .rounded(px(10.0))
            .p_4()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_2()
                    .child(
                        div()
                            .text_color(colors::rgba(self.colors.fg))
                            .child(SharedString::from("Settings")),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(colors::rgba(self.colors.selection_bg))
                            .text_color(colors::rgba(self.colors.selection_fg))
                            .child(SharedString::from("\u{2715}"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _ev, window, cx| {
                                    this.toggle_settings(window, cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    ),
            )
            .child(self.srow(
                "Theme",
                self.cycle(theme, Setting::ThemeCycle(-1), Setting::ThemeCycle(1), cx),
            ))
            .child(self.srow(
                "Font size",
                self.stepper(
                    format!("{}", o.font_size),
                    Setting::FontSize(-1),
                    Setting::FontSize(1),
                    cx,
                ),
            ))
            .child(self.srow(
                "Font style",
                self.cycle(
                    fstyle.to_string(),
                    Setting::FontStyleCycle,
                    Setting::FontStyleCycle,
                    cx,
                ),
            ))
            .child(self.srow(
                "Cursor",
                self.cycle(
                    cursor.to_string(),
                    Setting::CursorStyleCycle,
                    Setting::CursorStyleCycle,
                    cx,
                ),
            ))
            .child(self.srow(
                "Padding X",
                self.stepper(
                    o.window_padding_x.to_string(),
                    Setting::PaddingX(-1),
                    Setting::PaddingX(1),
                    cx,
                ),
            ))
            .child(self.srow(
                "Padding Y",
                self.stepper(
                    o.window_padding_y.to_string(),
                    Setting::PaddingY(-1),
                    Setting::PaddingY(1),
                    cx,
                ),
            ))
            .child(self.srow(
                "Scrollback",
                self.stepper(
                    o.scrollback_limit.to_string(),
                    Setting::Scrollback(-1000),
                    Setting::Scrollback(1000),
                    cx,
                ),
            ))
            .child(self.srow(
                "Copy on select",
                self.chip(
                    if o.copy_on_select { "On" } else { "Off" },
                    Setting::ToggleCopyOnSelect,
                    cx,
                ),
            ))
            .child(self.text_row(SettingsField::FontFamily, cx))
            .child(self.text_row(SettingsField::Shell, cx))
            .child(self.text_row(SettingsField::Foreground, cx))
            .child(self.text_row(SettingsField::Background, cx))
            .child(
                div()
                    .pt_2()
                    .text_color(border)
                    .child(SharedString::from(
                        "click a field, type, Enter to save \u{00b7} \u{2318}, to close",
                    )),
            )
            // Swallow clicks on the panel so they don't reach the scrim
            // (which would close) or the terminal beneath.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _ev, _w, cx| cx.stop_propagation()),
            );

        let mut scrim = colors::hsla(self.colors.bg);
        scrim.a = 0.6;
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(scrim)
            .child(panel)
            // A click on the dimmed backdrop closes the panel.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev, window, cx| {
                    this.toggle_settings(window, cx);
                    cx.stop_propagation();
                }),
            )
            .into_any_element()
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = self.tabs.active().tree.clone();
        let focused = self.tabs.focused();
        let children: Vec<(PaneId, AnyElement)> = tree
            .panes()
            .into_iter()
            .filter_map(|id| {
                self.panes.get(&id).map(|pane| (id, pane.view.clone().into_any_element()))
            })
            .collect();
        let mut dividercolor = colors::hsla(self.colors.fg);
        dividercolor.a = 0.2;
        let mut focuscolor = colors::hsla(self.colors.fg);
        focuscolor.a = 0.35;
        let root: WeakEntity<Self> = cx.weak_entity();
        let splitselement = SplitsElement::new(
            tree,
            focused,
            children,
            dividercolor,
            focuscolor,
            self.drag.clone(),
            root,
        );

        let mut base = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::rgba(self.colors.bg))
            .key_context("Workspace")
            .track_focus(&self.settings_focus)
            .on_key_down(cx.listener(Self::settings_key))
            .on_action(cx.listener(Self::runbind));

        if self.tabs.len() > 1 {
            let titles = self.titles(cx);
            base = base.child(tabbar::bar(
                &titles,
                self.tabs.active_index(),
                &self.colors,
                self.cell,
                &self.font,
                self.font_size,
                cx,
            ));
        }

        base = base.child(div().w_full().flex_1().min_h(px(0.0)).child(splitselement));
        if self.settings_open {
            base = base.child(self.settings_modal(cx));
        }
        base
    }
}
