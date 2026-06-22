//! System Settings-style preferences window. Every option Prompt reads from
//! its config file is shown and edited here; the window never asks the user
//! to go hand-edit the file. Writes go straight back to the config file and
//! the live-reload watcher applies them.

mod model;
mod ui;

use gpui::prelude::*;
use gpui::{
    bounds, point, px, size, App, Context, FocusHandle, KeyDownEvent, TitlebarOptions, Window,
    WindowBounds, WindowOptions,
};

use crate::textedit::TextEdit;
use model::{Bool, Choice, Field, ListKind, Num, Section};

const WIDTH: f32 = 725.0;
const HEIGHT: f32 = 810.0;

/// What the single active text editor is bound to.
#[derive(Clone, PartialEq)]
enum EditTarget {
    /// A scalar free-text option.
    Field(Field),
    /// An existing entry of a repeated option.
    Item(ListKind, usize),
    /// A new, not-yet-saved entry being typed for a repeated option.
    NewItem(ListKind),
}

pub struct SettingsView {
    opts: config::Options,
    section: Section,
    editing: Option<(EditTarget, TextEdit)>,
    focus: FocusHandle,
}

pub fn open(parent: &Window, cx: &mut App) {
    let center = parent.bounds().center();
    let bounds = bounds(
        center - point(px(WIDTH / 2.0), px(HEIGHT / 2.0)),
        size(px(WIDTH), px(HEIGHT)),
    );
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(20.0), px(20.0))),
            }),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title("Settings");
            cx.new(|cx| SettingsView::new(cx))
        },
    );
}

impl SettingsView {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            opts: config::Options::default(),
            section: Section::General,
            editing: None,
            focus: cx.focus_handle(),
        };
        view.reload();
        view
    }

    fn reload(&mut self) {
        let (opts, diagnostics) = config::load();
        for d in &diagnostics {
            eprintln!("prompt: config line {}: {} ({})", d.line, d.message, d.key);
        }
        self.opts = opts;
    }

    fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.editing = None;
        cx.notify();
    }

    // --- Control actions ---------------------------------------------------

    fn toggle(&mut self, b: Bool, cx: &mut Context<Self>) {
        write_config(b.key(), &(!b.get(&self.opts)).to_string());
        self.reload();
        cx.notify();
    }

    fn step(&mut self, n: Num, dir: i32, cx: &mut Context<Self>) {
        write_config(n.key(), &n.write_value(&self.opts, dir));
        self.reload();
        cx.notify();
    }

    fn cycle(&mut self, c: Choice, dir: i32, cx: &mut Context<Self>) {
        write_config(c.key(), &c.write_value(&self.opts, dir));
        self.reload();
        cx.notify();
    }

    // --- Editing -----------------------------------------------------------

    fn begin_edit(&mut self, target: EditTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            EditTarget::Field(f) => self.start_field(f, window, cx),
            EditTarget::Item(k, i) => self.start_item(k, i, window, cx),
            EditTarget::NewItem(k) => self.start_new_item(k, window, cx),
        }
    }

    fn start_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some((
            EditTarget::Field(field),
            TextEdit::new(&field.value(&self.opts)),
        ));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn start_item(&mut self, kind: ListKind, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        let current = kind.values(&self.opts).get(idx).cloned().unwrap_or_default();
        self.editing = Some((EditTarget::Item(kind, idx), TextEdit::new(&current)));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn start_new_item(&mut self, kind: ListKind, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some((EditTarget::NewItem(kind), TextEdit::new("")));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn remove_item(&mut self, kind: ListKind, idx: usize, cx: &mut Context<Self>) {
        let mut entries = kind.values(&self.opts);
        if idx < entries.len() {
            entries.remove(idx);
            self.write_list(kind, &entries);
        }
        self.editing = None;
        cx.notify();
    }

    fn commit_edit(&mut self, cx: &mut Context<Self>) {
        if let Some((target, edit)) = self.editing.take() {
            let text = edit.text();
            match target {
                EditTarget::Field(field) => write_config(field.key(), text.trim()),
                EditTarget::Item(kind, idx) => {
                    let mut entries = kind.values(&self.opts);
                    if idx < entries.len() {
                        if text.trim().is_empty() {
                            entries.remove(idx);
                        } else {
                            entries[idx] = text.trim().to_string();
                        }
                        self.write_list(kind, &entries);
                    }
                }
                EditTarget::NewItem(kind) => {
                    if !text.trim().is_empty() {
                        let mut entries = kind.values(&self.opts);
                        entries.push(text.trim().to_string());
                        self.write_list(kind, &entries);
                    }
                }
            }
            self.reload();
        }
        cx.notify();
    }

    fn write_list(&self, kind: ListKind, entries: &[String]) {
        let (key, values) = kind.to_config(entries);
        write_list(key, &values);
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_none() {
            return;
        }
        let ks = &event.keystroke;
        if ks.modifiers.platform || ks.modifiers.control {
            return;
        }
        match ks.key.as_str() {
            "enter" => {
                self.commit_edit(cx);
                cx.stop_propagation();
                return;
            }
            "escape" => {
                self.editing = None;
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
                            if let Some(text) = ks
                                .key_char
                                .as_deref()
                                .filter(|t| !t.is_empty() && !ks.modifiers.alt)
                            {
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
}

/// Write a single `key = value` line to the config file in place.
fn write_config(key: &str, value: &str) {
    let Some(path) = config::default_path() else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = config::upsert(&text, key, value);
    persist(&path, &updated);
}

/// Replace every line for a repeated `key` with the given values.
fn write_list(key: &str, values: &[String]) {
    let Some(path) = config::default_path() else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = config::set_list(&text, key, values);
    persist(&path, &updated);
}

fn persist(path: &std::path::Path, contents: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, contents);
}
