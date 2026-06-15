//! Applies a single `key = value` pair to [`Options`].

use crate::options::{ClipboardAccess, CursorStyle, FontStyle, OptionAsAlt, Options};
use crate::value;

/// Apply one `key = value` pair to the options. An empty value resets the
/// key to its default. Returns an error message for unknown keys or
/// unparseable values.
pub fn apply(opts: &mut Options, key: &str, val: &str) -> Result<(), String> {
    let d = Options::default();
    let empty = val.is_empty();
    match key {
        "font-family" => {
            // Empty resets the chain; otherwise each entry appends a
            // fallback (the first becomes the primary font).
            if empty {
                opts.font_family = d.font_family;
            } else {
                opts.font_family.push(val.to_string());
            }
        }
        "font-size" => {
            opts.font_size = if empty {
                d.font_size
            } else {
                value::parse_f32(val).ok_or_else(|| bad("number", val))?
            };
        }
        "font-style" => {
            opts.font_style = if empty {
                d.font_style
            } else {
                FontStyle::parse(val)
                    .ok_or_else(|| bad("normal|bold|italic|bold-italic", val))?
            };
        }
        "font-feature" => {
            if empty {
                opts.font_feature = d.font_feature;
            } else {
                let feature = value::parse_fontfeature(val)
                    .ok_or_else(|| bad("feature tag like `-liga` or `+ss01`", val))?;
                opts.font_feature.push(feature);
            }
        }
        "adjust-cell-width" => {
            opts.adjust_cell_width = if empty {
                d.adjust_cell_width
            } else {
                value::parse_adjust(val).ok_or_else(|| bad("integer pixels", val))?
            };
        }
        "adjust-cell-height" => {
            opts.adjust_cell_height = if empty {
                d.adjust_cell_height
            } else {
                value::parse_adjust(val).ok_or_else(|| bad("integer pixels", val))?
            };
        }
        "theme" => {
            opts.theme = if empty { d.theme } else { val.to_string() };
        }
        "background" => {
            opts.background = if empty { d.background } else { Some(val.to_string()) };
        }
        "foreground" => {
            opts.foreground = if empty { d.foreground } else { Some(val.to_string()) };
        }
        "cursor-style" => {
            opts.cursor_style = if empty {
                d.cursor_style
            } else {
                CursorStyle::parse(val).ok_or_else(|| bad("block|bar|underline", val))?
            };
        }
        "cursor-style-blink" => {
            opts.cursor_style_blink = if empty {
                d.cursor_style_blink
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "cursor-color" => {
            opts.cursor_color = if empty { d.cursor_color } else { Some(color(val)?) };
        }
        "cursor-text" => {
            opts.cursor_text = if empty { d.cursor_text } else { Some(color(val)?) };
        }
        "selection-foreground" => {
            opts.selection_foreground = if empty {
                d.selection_foreground
            } else {
                Some(color(val)?)
            };
        }
        "selection-background" => {
            opts.selection_background = if empty {
                d.selection_background
            } else {
                Some(color(val)?)
            };
        }
        "bold-is-bright" => {
            opts.bold_is_bright = if empty {
                d.bold_is_bright
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "minimum-contrast" => {
            opts.minimum_contrast = if empty {
                d.minimum_contrast
            } else {
                value::parse_f32_range(val, 1.0, 21.0)
                    .ok_or_else(|| bad("number in 1..21", val))?
            };
        }
        "unfocused-split-opacity" => {
            opts.unfocused_split_opacity = if empty {
                d.unfocused_split_opacity
            } else {
                value::parse_f32_range(val, 0.15, 1.0)
                    .ok_or_else(|| bad("number in 0.15..1", val))?
            };
        }
        "split-divider-color" => {
            opts.split_divider_color = if empty {
                d.split_divider_color
            } else {
                Some(color(val)?)
            };
        }
        "mouse-scroll-multiplier" => {
            opts.mouse_scroll_multiplier = if empty {
                d.mouse_scroll_multiplier
            } else {
                value::parse_f32_range(val, 0.01, 10_000.0)
                    .ok_or_else(|| bad("number in 0.01..10000", val))?
            };
        }
        "macos-option-as-alt" => {
            opts.macos_option_as_alt = if empty {
                d.macos_option_as_alt
            } else {
                OptionAsAlt::parse(val).ok_or_else(|| bad("false|true|left|right", val))?
            };
        }
        "window-inherit-working-directory" => {
            opts.window_inherit_working_directory = if empty {
                d.window_inherit_working_directory
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "quit-after-last-window-closed" => {
            opts.quit_after_last_window_closed = if empty {
                d.quit_after_last_window_closed
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "title" => {
            opts.title = if empty { d.title } else { Some(val.to_string()) };
        }
        "clipboard-read" => {
            opts.clipboard_read = if empty {
                d.clipboard_read
            } else {
                ClipboardAccess::parse(val).ok_or_else(|| bad("allow|ask|deny", val))?
            };
        }
        "clipboard-write" => {
            opts.clipboard_write = if empty {
                d.clipboard_write
            } else {
                ClipboardAccess::parse(val).ok_or_else(|| bad("allow|ask|deny", val))?
            };
        }
        "scrollback-limit" => {
            opts.scrollback_limit = if empty {
                d.scrollback_limit
            } else {
                value::parse_usize(val).ok_or_else(|| bad("non-negative integer", val))?
            };
        }
        "window-padding-x" => {
            opts.window_padding_x = if empty {
                d.window_padding_x
            } else {
                value::parse_u32(val).ok_or_else(|| bad("non-negative integer", val))?
            };
        }
        "window-padding-y" => {
            opts.window_padding_y = if empty {
                d.window_padding_y
            } else {
                value::parse_u32(val).ok_or_else(|| bad("non-negative integer", val))?
            };
        }
        "window-width" => {
            opts.window_width = if empty {
                d.window_width
            } else {
                value::parse_u32(val).ok_or_else(|| bad("non-negative integer", val))?
            };
        }
        "window-height" => {
            opts.window_height = if empty {
                d.window_height
            } else {
                value::parse_u32(val).ok_or_else(|| bad("non-negative integer", val))?
            };
        }
        "command" => {
            opts.shell = if empty { d.shell } else { Some(val.to_string()) };
        }
        "working-directory" => {
            opts.working_directory = if empty {
                d.working_directory
            } else {
                Some(val.to_string())
            };
        }
        "copy-on-select" => {
            opts.copy_on_select = if empty {
                d.copy_on_select
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "confirm-close-surface" => {
            opts.confirm_close_surface = if empty {
                d.confirm_close_surface
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "mouse-hide-while-typing" => {
            opts.mouse_hide_while_typing = if empty {
                d.mouse_hide_while_typing
            } else {
                value::parse_bool(val).ok_or_else(|| bad("boolean", val))?
            };
        }
        "palette" => {
            if empty {
                opts.palette = d.palette;
            } else {
                let entry =
                    value::parse_palette(val).ok_or_else(|| bad("N=#rrggbb", val))?;
                opts.palette.push(entry);
            }
        }
        "keybind" => {
            if empty {
                opts.keybind = d.keybind;
            } else {
                opts.keybind.push(val.to_string());
            }
        }
        _ => return Err(format!("unknown key `{key}`")),
    }
    Ok(())
}

fn color(val: &str) -> Result<String, String> {
    value::parse_color(val).ok_or_else(|| bad("hex color `#rrggbb`", val))
}

fn bad(expected: &str, got: &str) -> String {
    format!("invalid value `{got}`, expected {expected}")
}

#[cfg(test)]
mod tests {
    use crate::options::{ClipboardAccess, FontStyle, OptionAsAlt, Options};
    use crate::parse::parse_str;

    #[test]
    fn new_options_parse() {
        let src = r#"
font-style = bold-italic
font-feature = -liga
font-feature = +ss01
adjust-cell-width = 2
adjust-cell-height = -1px
cursor-color = #ff0000
cursor-text = 00ff00
selection-foreground = #FFFFFF
selection-background = #000000
bold-is-bright = true
minimum-contrast = 3
unfocused-split-opacity = 0.5
split-divider-color = #444444
mouse-scroll-multiplier = 2.5
macos-option-as-alt = left
window-inherit-working-directory = false
quit-after-last-window-closed = false
title = my terminal
clipboard-read = deny
clipboard-write = ask
"#;
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(o.font_style, FontStyle::BoldItalic);
        assert_eq!(o.font_feature, vec!["-liga".to_string(), "+ss01".to_string()]);
        assert_eq!(o.adjust_cell_width, 2);
        assert_eq!(o.adjust_cell_height, -1);
        assert_eq!(o.cursor_color.as_deref(), Some("#ff0000"));
        assert_eq!(o.cursor_text.as_deref(), Some("#00ff00"));
        assert_eq!(o.selection_foreground.as_deref(), Some("#ffffff"));
        assert_eq!(o.selection_background.as_deref(), Some("#000000"));
        assert!(o.bold_is_bright);
        assert_eq!(o.minimum_contrast, 3.0);
        assert_eq!(o.unfocused_split_opacity, 0.5);
        assert_eq!(o.split_divider_color.as_deref(), Some("#444444"));
        assert_eq!(o.mouse_scroll_multiplier, 2.5);
        assert_eq!(o.macos_option_as_alt, OptionAsAlt::Left);
        assert!(!o.window_inherit_working_directory);
        assert!(!o.quit_after_last_window_closed);
        assert_eq!(o.title.as_deref(), Some("my terminal"));
        assert_eq!(o.clipboard_read, ClipboardAccess::Deny);
        assert_eq!(o.clipboard_write, ClipboardAccess::Ask);
    }

    #[test]
    fn new_options_bad_values_diagnose() {
        let cases = [
            "font-style = fancy",
            "font-feature = no good",
            "adjust-cell-width = wide",
            "adjust-cell-height = 10%",
            "cursor-color = red",
            "cursor-text = #fff",
            "selection-foreground = #12345",
            "selection-background = blue",
            "bold-is-bright = maybe",
            "minimum-contrast = abc",
            "unfocused-split-opacity = dim",
            "split-divider-color = gray",
            "mouse-scroll-multiplier = fast",
            "macos-option-as-alt = middle",
            "window-inherit-working-directory = sometimes",
            "quit-after-last-window-closed = perhaps",
            "clipboard-read = never",
            "clipboard-write = always",
        ];
        for case in cases {
            let (o, diags) = parse_str(case);
            assert_eq!(diags.len(), 1, "no diagnostic for `{case}`");
            assert_eq!(o, Options::default(), "value applied for `{case}`");
        }
    }

    #[test]
    fn new_options_empty_value_resets() {
        let src = "font-style = bold\nfont-style =\n\
                   font-feature = -liga\nfont-feature =\n\
                   adjust-cell-width = 3\nadjust-cell-width =\n\
                   adjust-cell-height = 3\nadjust-cell-height =\n\
                   cursor-color = #ff0000\ncursor-color =\n\
                   cursor-text = #ff0000\ncursor-text =\n\
                   selection-foreground = #ff0000\nselection-foreground =\n\
                   selection-background = #ff0000\nselection-background =\n\
                   bold-is-bright = true\nbold-is-bright =\n\
                   minimum-contrast = 4\nminimum-contrast =\n\
                   unfocused-split-opacity = 0.5\nunfocused-split-opacity =\n\
                   split-divider-color = #ff0000\nsplit-divider-color =\n\
                   mouse-scroll-multiplier = 3\nmouse-scroll-multiplier =\n\
                   macos-option-as-alt = left\nmacos-option-as-alt =\n\
                   window-inherit-working-directory = false\nwindow-inherit-working-directory =\n\
                   quit-after-last-window-closed = false\nquit-after-last-window-closed =\n\
                   title = x\ntitle =\n\
                   clipboard-read = deny\nclipboard-read =\n\
                   clipboard-write = deny\nclipboard-write =\n";
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(o, Options::default());
    }

    #[test]
    fn ranged_values_clamp() {
        let src = "minimum-contrast = 0.5\nunfocused-split-opacity = 0.01\n\
                   mouse-scroll-multiplier = 0.001\n";
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty());
        assert_eq!(o.minimum_contrast, 1.0);
        assert_eq!(o.unfocused_split_opacity, 0.15);
        assert_eq!(o.mouse_scroll_multiplier, 0.01);

        let src = "minimum-contrast = 100\nunfocused-split-opacity = 2\n\
                   mouse-scroll-multiplier = 99999999\n";
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty());
        assert_eq!(o.minimum_contrast, 21.0);
        assert_eq!(o.unfocused_split_opacity, 1.0);
        assert_eq!(o.mouse_scroll_multiplier, 10_000.0);
    }

    #[test]
    fn font_feature_accumulates_and_resets() {
        let src = "font-feature = -liga\nfont-feature = ss01\n\
                   font-feature =\nfont-feature = +calt\n";
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty());
        assert_eq!(o.font_feature, vec!["+calt".to_string()]);
    }

    #[test]
    fn font_family_builds_a_fallback_chain() {
        let src = "font-family = JetBrains Mono\nfont-family = Menlo\n\
                   font-family = Apple Color Emoji\n";
        let (o, diags) = parse_str(src);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(o.primary_font(), "JetBrains Mono");
        assert_eq!(o.font_fallbacks(), ["Menlo", "Apple Color Emoji"]);
        // An empty value resets the chain back to the default.
        let (o, _) = parse_str("font-family = X\nfont-family =\n");
        assert!(o.font_family.is_empty());
        assert_eq!(o.primary_font(), "Menlo");
    }
}
