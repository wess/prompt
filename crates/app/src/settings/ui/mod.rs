//! Rendering for the settings window: the search bar, category sidebar,
//! reusable controls, and the schema-driven rows.

use gpui::prelude::*;
use gpui::{div, px, Div, MouseButton, SharedString, Window, WindowControlArea};
use gpui::{Context, Hsla};

use super::schema::Section;
use super::SettingsView;
use crate::colors;

mod ai;
mod controls;
mod lists;
mod rows;

const SIDEBAR: f32 = 226.0;

impl SettingsView {
    fn sidebar_item(&self, section: Section, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.section == section && self.search().is_empty();
        let mut bg = hsla(if selected { BLUE } else { SIDEBAR_BG });
        bg.a = if selected { 1.0 } else { 0.0 };
        div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(32.0))
            .px_2()
            .rounded(px(7.0))
            .bg(bg)
            .text_color(hsla(TEXT))
            .child(self.icon(section.icon(), section.accent(), px(20.0)))
            .child(SharedString::from(section.title()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    this.set_section(section, cx);
                    cx.stop_propagation();
                }),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR))
            .flex_none()
            .h_full()
            .px_3()
            .pt(px(58.0))
            .pb_3()
            .bg(hsla(SIDEBAR_BG))
            .child(self.identity());
        for section in Section::ALL {
            bar = bar.child(self.sidebar_item(section, cx));
        }
        bar.child(div().flex_1()).child(self.file_link(cx))
    }

    /// The escape hatch: open the backing settings.json in an editor.
    fn file_link(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(32.0))
            .px_2()
            .rounded(px(7.0))
            .text_color(hsla(MUTED))
            .child(self.icon("{}", theme::Rgb::new(99, 99, 102), px(20.0)))
            .child(SharedString::from("Edit in settings.json"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _ev, _window, cx| {
                    super::open_settings_file();
                    cx.stop_propagation();
                }),
            )
    }

    fn identity(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .mb_4()
            .child(
                div()
                    .w(px(38.0))
                    .h(px(38.0))
                    .rounded(px(19.0))
                    .bg(hsla(theme::Rgb::new(232, 235, 241)))
                    .text_color(hsla(theme::Rgb::new(97, 103, 112)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(SharedString::from("S")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(SharedString::from("Sinclair"))
                    .child(
                        div()
                            .text_color(hsla(MUTED))
                            .child(SharedString::from("Settings")),
                    ),
            )
    }

    /// The always-live search box. It has no click-to-focus state: whenever
    /// no field editor is active, typing lands here.
    fn search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let idle = self.editing.is_some();
        let text = self.query.text();
        let mut border = hsla(if idle { FIELD_BORDER } else { BLUE });
        border.a = if idle { 0.75 } else { 1.0 };
        let mut field = div()
            .flex_1()
            .h(px(30.0))
            .px_2()
            .rounded(px(7.0))
            .border_1()
            .border_color(border)
            .bg(hsla(FIELD_BG))
            .flex()
            .items_center()
            .gap_1()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _ev, window, cx| {
                    // Commit any field edit; keys then flow to the search box.
                    this.commit_edit(cx);
                    window.focus(&this.focus, cx);
                    cx.stop_propagation();
                }),
            );
        if text.is_empty() && idle {
            field = field
                .text_color(hsla(MUTED))
                .child(SharedString::from("Search settings"));
        } else if idle {
            field = field.text_color(hsla(TEXT)).child(SharedString::from(text));
        } else {
            // Live: draw the caret (and selection) like the field editors do.
            field = field.text_color(hsla(TEXT));
            if let Some((before, selected, after)) = self.query.split_selection() {
                let mut sel_bg = hsla(BLUE);
                sel_bg.a = 0.35;
                field = field
                    .child(SharedString::from(before))
                    .child(div().bg(sel_bg).rounded(px(2.0)).child(SharedString::from(selected)))
                    .child(SharedString::from(after));
            } else {
                let (before, after) = self.query.split();
                if before.is_empty() && after.is_empty() {
                    field = field
                        .child(div().w(px(1.0)).h(px(16.0)).bg(hsla(TEXT)))
                        .child(
                            div()
                                .text_color(hsla(MUTED))
                                .child(SharedString::from("Search settings")),
                        );
                } else {
                    field = field
                        .child(SharedString::from(before))
                        .child(div().w(px(1.0)).h(px(16.0)).bg(hsla(TEXT)))
                        .child(SharedString::from(after));
                }
            }
        }
        let mut bar = div().flex().items_center().gap_2().pb_3().child(field);
        if !self.query.is_empty() {
            bar = bar.child(
                button_box("\u{2715}")
                    .text_color(hsla(MUTED))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _ev, _window, cx| {
                            this.query = guise::TextEdit::new("");
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        bar
    }

    /// The section header shown when browsing (search empty).
    fn section_header(&self) -> impl IntoElement {
        div()
            .pb_2()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(20.0))
                    .text_color(hsla(TEXT))
                    .child(SharedString::from(self.section.title())),
            )
            .child(
                div()
                    .text_color(hsla(MUTED))
                    .child(SharedString::from(self.section.subtitle())),
            )
    }

    fn content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let searching = !self.search().is_empty();
        let mut body = div()
            .id("settings-content")
            .flex_1()
            .min_h(px(0.0))
            .pb(px(40.0))
            .overflow_y_scroll();
        if searching {
            for group in self.search_results(cx) {
                body = body.child(group);
            }
        } else {
            body = body.child(self.section_header());
            for group in self.section_content(cx) {
                body = body.child(group);
            }
        }
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .px_5()
            .pt(px(50.0))
            .bg(hsla(CONTENT_BG))
            .child(self.search_bar(cx))
            .child(body)
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .text_color(hsla(TEXT))
            .bg(hsla(CONTENT_BG))
            .child(self.sidebar(cx))
            .child(self.content(cx))
            .child(drag_strip())
    }
}

/// A drag handle across the transparent titlebar so the window can be moved.
/// Left-inset on macOS to clear the traffic lights.
fn drag_strip() -> impl IntoElement {
    let lead = if cfg!(target_os = "macos") { 78.0 } else { 0.0 };
    div()
        .absolute()
        .top_0()
        .left(px(lead))
        .right_0()
        .h(px(30.0))
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
}

/// The shared chrome for a small bordered button (no behavior attached yet).
fn button_box(label: impl Into<SharedString>) -> Div {
    div()
        .h(px(26.0))
        .min_w(px(28.0))
        .px_2()
        .rounded(px(6.0))
        .border_1()
        .border_color(hsla(FIELD_BORDER))
        .bg(hsla(FIELD_BG))
        .flex()
        .items_center()
        .justify_center()
        .text_color(hsla(TEXT))
        .child(label.into())
}

fn hsla(rgb: theme::Rgb) -> Hsla {
    colors::hsla(rgb)
}

/// Truncate to `n` chars with an ellipsis.
fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}\u{2026}", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

const SIDEBAR_BG: theme::Rgb = theme::Rgb::new(30, 35, 38);
const CONTENT_BG: theme::Rgb = theme::Rgb::new(35, 42, 44);
const PANEL: theme::Rgb = theme::Rgb::new(43, 52, 54);
const FIELD_BG: theme::Rgb = theme::Rgb::new(49, 56, 58);
const FIELD_BORDER: theme::Rgb = theme::Rgb::new(76, 84, 88);
const LINE: theme::Rgb = theme::Rgb::new(61, 70, 73);
const TEXT: theme::Rgb = theme::Rgb::new(242, 244, 246);
const MUTED: theme::Rgb = theme::Rgb::new(170, 177, 181);
const BLUE: theme::Rgb = theme::Rgb::new(10, 102, 220);
const BLUE_TEXT: theme::Rgb = theme::Rgb::new(90, 170, 255);
