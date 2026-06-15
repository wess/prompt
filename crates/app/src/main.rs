//! Prompt: a terminal emulator. Tabs of split panes, one shell per pane.

mod boxdraw;
mod bridge;
mod colors;
mod element;
mod font;
mod keys;
mod metrics;
mod mouse;
mod pointer;
mod reload;
mod root;
mod session;
mod splits;
mod tabbar;
mod textedit;
mod view;

use std::rc::Rc;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui::AppContext as _;

const DEFAULT_COLS: usize = 80;
const DEFAULT_ROWS: usize = 24;

fn main() {
    let (opts, diagnostics) = config::load();
    for d in &diagnostics {
        eprintln!("prompt: config line {}: {} ({})", d.line, d.message, d.key);
    }

    gpui_platform::application().run(move |cx: &mut App| {
        let colors = Rc::new(colors::from_config(&opts));
        let font = font::build(&opts);
        let font_size = px(opts.font_size.max(1.0));
        let cell = metrics::measure(cx.text_system(), &font, font_size);
        let pad = metrics::Padding {
            x: opts.window_padding_x as f32,
            y: opts.window_padding_y as f32,
        };
        let cols = if opts.window_width > 0 {
            opts.window_width as usize
        } else {
            DEFAULT_COLS
        };
        let rows = if opts.window_height > 0 {
            opts.window_height as usize
        } else {
            DEFAULT_ROWS
        };
        let (width, height) = metrics::pixel_size(cols, rows, pad, cell);

        // Keybindings come from config (defaults + user overrides) and are
        // bound by the workspace view, which owns the resolved table.

        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("prompt".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(move |cx| {
                    root::WorkspaceView::new(
                        opts, colors, font, font_size, cell, pad, cols, rows, window, cx,
                    )
                })
            },
        )
        .expect("open window");
        cx.activate(true);
    });
}
