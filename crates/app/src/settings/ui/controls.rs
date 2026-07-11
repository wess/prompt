//! The per-type controls: switch, slider, choice dropdown, and text field,
//! plus the shared row/panel chrome.

use super::super::schema::{Choice, Setting, Slider};
use super::super::{EditTarget, SettingsView};
use super::*;
use gpui::{
    canvas, div, px, relative, AnyElement, Context, DragMoveEvent, Empty, MouseButton,
    MouseDownEvent, SharedString,
};

/// Drag payload identifying which slider a scrub belongs to, so the shared
/// `on_drag_move` listener only acts on the track the drag started on.
struct SliderDrag(&'static str);

impl SettingsView {
    pub(crate) fn icon(&self, glyph: &str, color: theme::Rgb, size: gpui::Pixels) -> impl IntoElement {
        div()
            .w(size)
            .h(size)
            .rounded(px(5.0))
            .bg(hsla(color))
            .text_color(hsla(TEXT))
            .flex()
            .items_center()
            .justify_center()
            .child(SharedString::from(glyph.to_string()))
    }

    /// A plain label/control row, for the hand-built groups (macros, relay
    /// status, agent tools).
    pub(crate) fn row(
        &self,
        icon: impl IntoElement,
        label: &str,
        control: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .w_full()
            .h(px(52.0))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_none()
                    .child(icon)
                    .child(SharedString::from(label.to_string())),
            )
            .child(control)
    }

    pub(crate) fn list(&self, rows: Vec<AnyElement>) -> impl IntoElement {
        let mut list = div().w_full().flex().flex_col().rounded(px(10.0)).bg(hsla(PANEL));
        for (i, row) in rows.into_iter().enumerate() {
            if i > 0 {
                list = list.child(div().mx_3().h(px(1.0)).bg(hsla(LINE)));
            }
            list = list.child(row);
        }
        list
    }

    pub(crate) fn heading(&self, text: &str) -> impl IntoElement {
        div()
            .pt_4()
            .pb_1()
            .px_1()
            .text_color(hsla(MUTED))
            .child(SharedString::from(text.to_string()))
    }

    /// A bordered text field bound to `target`. When that target is the one
    /// being edited it shows a live caret; otherwise it shows the value (or a
    /// muted placeholder) and starts editing on click.
    pub(crate) fn text_input(
        &self,
        target: EditTarget,
        value: String,
        placeholder: &str,
        width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.editing.as_ref().map(|(t, _)| t) == Some(&target);
        let mut border = hsla(if active { BLUE } else { FIELD_BORDER });
        border.a = if active { 1.0 } else { 0.75 };
        // Flexible width: grow to `width` but shrink below it when the row is
        // narrow, so long values never overflow the content column.
        let mut field = div()
            .flex_1()
            .min_w(px(0.0))
            .max_w(px(width))
            .h(px(26.0))
            .px_2()
            .rounded(px(6.0))
            .border_1()
            .border_color(border)
            .bg(hsla(FIELD_BG))
            .flex()
            .items_center()
            .overflow_hidden();
        if active && self.capturing {
            field = field
                .text_color(hsla(BLUE_TEXT))
                .child(SharedString::from("Press keys\u{2026}"));
        } else if let Some((_, edit)) = self.editing.as_ref().filter(|_| active) {
            field = field.text_color(hsla(TEXT));
            if let Some((before, selected, after)) = edit.split_selection() {
                let mut sel_bg = hsla(BLUE);
                sel_bg.a = 0.35;
                field = field
                    .child(SharedString::from(before))
                    .child(
                        div()
                            .bg(sel_bg)
                            .rounded(px(2.0))
                            .child(SharedString::from(selected)),
                    )
                    .child(SharedString::from(after));
            } else {
                let (before, after) = edit.split();
                field = field
                    .child(SharedString::from(before))
                    .child(div().w(px(1.0)).h(px(16.0)).bg(hsla(TEXT)))
                    .child(SharedString::from(after));
            }
        } else {
            let empty = value.is_empty();
            field = field
                .text_color(hsla(if empty { MUTED } else { TEXT }))
                .child(SharedString::from(if empty {
                    placeholder.to_string()
                } else {
                    value
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev, window, cx| {
                        this.begin_edit(target.clone(), window, cx);
                        cx.stop_propagation();
                    }),
                );
        }
        field
    }

    pub(crate) fn switch(
        &self,
        s: &'static Setting,
        get: fn(&config::Options) -> bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = get(&self.opts);
        let knob_x = if on { px(19.0) } else { px(2.0) };
        div()
            .w(px(45.0))
            .h(px(26.0))
            .rounded(px(13.0))
            .bg(hsla(if on { BLUE } else { FIELD_BG }))
            .relative()
            .child(
                div()
                    .absolute()
                    .left(knob_x)
                    .top(px(2.0))
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(11.0))
                    .bg(hsla(theme::Rgb::new(255, 255, 255))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    this.toggle(s, cx);
                    cx.stop_propagation();
                }),
            )
    }

    /// A draggable value track for a numeric option. Press anywhere on the
    /// track to jump to that value; press and drag to scrub — the drag follows
    /// the pointer anywhere in the window (like a real slider), not just while
    /// it stays over the track. The section's accent colors the fill and knob.
    pub(crate) fn slider(
        &self,
        s: &'static Setting,
        n: Slider,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let accent = s.section.accent();
        let frac = n.fraction(&self.opts);

        let bar = div()
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(7.0))
            .h(px(6.0))
            .rounded(px(3.0))
            .bg(hsla(FIELD_BG));

        let fill = div()
            .absolute()
            .left(px(0.0))
            .top(px(7.0))
            .w(relative(frac))
            .h(px(6.0))
            .rounded(px(3.0))
            .bg(hsla(accent));

        let knob = div()
            .absolute()
            .left(relative(frac))
            .ml(px(-7.0))
            .top(px(3.0))
            .w(px(14.0))
            .h(px(14.0))
            .rounded(px(7.0))
            .bg(hsla(TEXT))
            .border_2()
            .border_color(hsla(accent));

        // Invisible probe that records the track's window-space bounds each
        // frame, so a mouse-down (position only, no bounds) maps to a value.
        let key = s.key;
        let entity = cx.entity();
        let probe = canvas(
            move |bounds, _window, cx| {
                entity.update(cx, |view, _| {
                    view.slider_bounds.insert(key, bounds);
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        let track = div()
            .id(s.key)
            .relative()
            .w(px(150.0))
            .h(px(20.0))
            .cursor_pointer()
            .child(probe)
            .child(bar)
            .child(fill)
            .child(knob)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                    if let Some(b) = this.slider_bounds.get(s.key).copied() {
                        let width = f32::from(b.size.width);
                        if width > 0.0 {
                            let frac = (f32::from(event.position.x - b.left()) / width).clamp(0.0, 1.0);
                            this.slide_to(s, frac, cx);
                        }
                    }
                }),
            )
            .on_drag(SliderDrag(s.key), |_drag, _offset, _window, cx| cx.new(|_| Empty))
            .on_drag_move::<SliderDrag>(cx.listener(
                move |this, event: &DragMoveEvent<SliderDrag>, _window, cx| {
                    // Every track's listener fires for any slider drag; act only
                    // on the one the drag started on, using this track's bounds.
                    if event.drag(cx).0 != s.key {
                        return;
                    }
                    let b = event.bounds;
                    let width = f32::from(b.size.width);
                    if width > 0.0 {
                        let frac = (f32::from(event.event.position.x - b.left()) / width).clamp(0.0, 1.0);
                        this.slide_to(s, frac, cx);
                    }
                },
            ));

        div()
            .flex()
            .items_center()
            .gap_3()
            .child(track)
            .child(
                div()
                    .w(px(56.0))
                    .flex()
                    .justify_end()
                    .text_color(hsla(TEXT))
                    .child(SharedString::from(n.display(&self.opts))),
            )
    }

    /// The closed dropdown: current value plus a chevron; click to expand.
    pub(crate) fn choice_button(
        &self,
        s: &'static Setting,
        c: Choice,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = self.open_choice == Some(s.key);
        let glyph = if open { "\u{2303}" } else { "\u{2304}" };
        button_box(SharedString::from(format!("{}  {glyph}", (c.get)(&self.opts))))
            .min_w(px(120.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    this.toggle_choice(s.key, cx);
                    cx.stop_propagation();
                }),
            )
    }

    /// The expanded variant list under a Choice row.
    pub(crate) fn choice_panel(
        &self,
        s: &'static Setting,
        c: Choice,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current = (c.get)(&self.opts);
        let mut items: Vec<(String, bool)> = Vec::new();
        if let Some(unset) = c.unset {
            items.push((unset.to_string(), true));
        }
        items.extend((c.variants)().into_iter().map(|v| (v, false)));

        let mut panel = div()
            .id(s.key)
            .mx_3()
            .mb_2()
            .max_h(px(220.0))
            .overflow_y_scroll()
            .rounded(px(8.0))
            .border_1()
            .border_color(hsla(FIELD_BORDER))
            .bg(hsla(FIELD_BG))
            .flex()
            .flex_col();
        for (value, unset) in items {
            let selected = value == current;
            let mut item = div()
                .px_3()
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_between()
                .text_color(hsla(if selected { BLUE_TEXT } else { TEXT }))
                .child(SharedString::from(value.clone()));
            if selected {
                item = item.child(SharedString::from("\u{2713}"));
            }
            panel = panel.child(item.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    this.choose(s, value.clone(), unset, cx);
                    cx.stop_propagation();
                }),
            ));
        }
        panel
    }
}
