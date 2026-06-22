//! Tiny modal window for "Change Tab Title" / "Change Terminal Title". One
//! text field; Enter applies the new title back to the workspace, Escape
//! cancels. Mirrors the Settings window chrome.

use gpui::prelude::*;
use gpui::{
    bounds, div, point, px, size, App, Context, FocusHandle, KeyDownEvent, SharedString,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions,
};
use workspace::PaneId;

use crate::colors;
use crate::root::WorkspaceView;
use crate::textedit::TextEdit;

const WIDTH: f32 = 380.0;
const HEIGHT: f32 = 150.0;

/// What a rename targets: a tab label, a single pane's title, or naming a
/// freshly recorded macro (carrying its captured commands).
#[derive(Clone)]
pub enum Target {
    Tab(usize),
    Pane(PaneId),
    Macro(Vec<String>),
}

/// Open the modal to name and save a just-recorded macro.
pub fn open_macro(
    parent: &Window,
    root: WeakEntity<WorkspaceView>,
    commands: Vec<String>,
    cx: &mut App,
) {
    open(parent, root, Target::Macro(commands), String::new(), cx);
}

/// Open the rename window centered over `parent`, editing `initial`.
pub fn open(
    parent: &Window,
    root: WeakEntity<WorkspaceView>,
    target: Target,
    initial: String,
    cx: &mut App,
) {
    let title = match target {
        Target::Tab(_) => "Change Tab Title",
        Target::Pane(_) => "Change Terminal Title",
        Target::Macro(_) => "Name Macro",
    };
    let center = parent.bounds().center();
    let window_bounds = bounds(
        center - point(px(WIDTH / 2.0), px(HEIGHT / 2.0)),
        size(px(WIDTH), px(HEIGHT)),
    );
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(12.0))),
            }),
            ..Default::default()
        },
        move |window, cx| {
            window.set_window_title(title);
            cx.new(|cx| RenameView::new(root, target, &initial, title, cx))
        },
    );
}

pub struct RenameView {
    root: WeakEntity<WorkspaceView>,
    target: Target,
    title: &'static str,
    edit: TextEdit,
    focus: FocusHandle,
}

impl RenameView {
    fn new(
        root: WeakEntity<WorkspaceView>,
        target: Target,
        initial: &str,
        title: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            root,
            target,
            title,
            edit: TextEdit::new(initial),
            focus: cx.focus_handle(),
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.edit.text();
        let target = self.target.clone();
        self.root
            .update(cx, |workspace, cx| match target {
                Target::Tab(index) => workspace.rename_tab(index, &text, cx),
                Target::Pane(id) => workspace.rename_pane(id, &text, cx),
                Target::Macro(commands) => workspace.save_macro(&text, commands, cx),
            })
            .ok();
        window.remove_window();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        if ks.modifiers.platform || ks.modifiers.control {
            return;
        }
        match ks.key.as_str() {
            "enter" => self.commit(window, cx),
            "escape" => window.remove_window(),
            "backspace" => {
                self.edit.backspace();
            }
            "delete" => {
                self.edit.delete();
            }
            "left" => self.edit.left(),
            "right" => self.edit.right(),
            "home" => self.edit.home(),
            "end" => self.edit.end(),
            _ => {
                if let Some(text) = ks
                    .key_char
                    .as_deref()
                    .filter(|t| !t.is_empty() && !ks.modifiers.alt)
                {
                    self.edit.insert(text);
                }
            }
        }
        cx.notify();
        cx.stop_propagation();
    }
}

impl Render for RenameView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (before, after) = self.edit.split();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .px_4()
            .pt(px(40.0))
            .pb_4()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .bg(hsla(CONTENT_BG))
            .text_color(hsla(TEXT))
            .child(
                div()
                    .text_color(hsla(MUTED))
                    .child(SharedString::from(self.title)),
            )
            .child(
                div()
                    .h(px(30.0))
                    .px_2()
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(hsla(BLUE))
                    .bg(hsla(FIELD_BG))
                    .flex()
                    .items_center()
                    .child(SharedString::from(before))
                    .child(div().w(px(1.0)).h(px(16.0)).bg(hsla(TEXT)))
                    .child(SharedString::from(after)),
            )
            .child(
                div()
                    .text_color(hsla(MUTED))
                    .child(SharedString::from("Return to apply \u{2022} Esc to cancel")),
            )
    }
}

fn hsla(rgb: theme::Rgb) -> gpui::Hsla {
    colors::hsla(rgb)
}

const CONTENT_BG: theme::Rgb = theme::Rgb::new(35, 42, 44);
const FIELD_BG: theme::Rgb = theme::Rgb::new(49, 56, 58);
const TEXT: theme::Rgb = theme::Rgb::new(242, 244, 246);
const MUTED: theme::Rgb = theme::Rgb::new(170, 177, 181);
const BLUE: theme::Rgb = theme::Rgb::new(10, 102, 220);
