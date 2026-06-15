//! Minimal Ghostty-style tab bar: rendered only with more than one tab.

use gpui::prelude::*;
use gpui::{ClickEvent, Context, SharedString, div, px};
use theme::Rgb;

use crate::colors::{self, Colors};
use crate::metrics::CellSize;
use crate::root::WorkspaceView;

/// Vertical padding around one cell-height of label text.
pub const PAD: f32 = 8.0;

/// Bar height: one cell plus padding.
pub fn height(cell: CellSize) -> f32 {
    cell.height + PAD
}

/// Linear mix of two colors: `t` 0 is `a`, 1 is `b`. Clamped.
pub fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgb::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
}

/// The tab bar: evenly divided tabs, click activates, x closes. The
/// active tab keeps the terminal background; inactive tabs are dimmed
/// toward the foreground color.
pub fn bar(
    titles: &[String],
    active: usize,
    colors: &Colors,
    cell: CellSize,
    font: &gpui::Font,
    font_size: gpui::Pixels,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    let barbg = colors::rgba(blend(colors.bg, colors.fg, 0.12));
    let activebg = colors::rgba(colors.bg);
    let fg = colors::hsla(colors.fg);
    let mut dim = fg;
    dim.a = 0.55;

    div()
        .w_full()
        .h(px(height(cell)))
        .flex()
        .flex_row()
        .bg(barbg)
        .font_family(font.family.clone())
        .text_size(font_size * 0.9)
        .children(titles.iter().enumerate().map(|(index, title)| {
            let isactive = index == active;
            div()
                .id(("tab", index))
                .flex_1()
                .h_full()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .px(px(8.0))
                .bg(if isactive { activebg } else { barbg })
                .text_color(if isactive { fg } else { dim })
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.activatetab(index, window, cx);
                }))
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(title.clone())),
                )
                .child(
                    div()
                        .id(("tabclose", index))
                        .px(px(4.0))
                        .text_color(dim)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.closetab(index, window, cx);
                        }))
                        .child("×"),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_one_cell_plus_padding() {
        let cell = CellSize { width: 8.0, height: 17.0 };
        assert_eq!(height(cell), 17.0 + PAD);
    }

    #[test]
    fn blend_endpoints_and_midpoint() {
        let a = Rgb::new(0, 0, 0);
        let b = Rgb::new(255, 255, 255);
        assert_eq!(blend(a, b, 0.0), a);
        assert_eq!(blend(a, b, 1.0), b);
        assert_eq!(blend(a, b, 0.5), Rgb::new(128, 128, 128));
        // Out-of-range t clamps.
        assert_eq!(blend(a, b, -1.0), a);
        assert_eq!(blend(a, b, 2.0), b);
    }

    #[test]
    fn blend_mixes_channels_independently() {
        let a = Rgb::new(10, 200, 0);
        let b = Rgb::new(20, 100, 255);
        let m = blend(a, b, 0.1);
        assert_eq!(m, Rgb::new(11, 190, 26));
    }
}
