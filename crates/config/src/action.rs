//! Keybind actions, Ghostty-style: a name plus an optional `:param`.

/// Direction for `new_split`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Right,
    Down,
    Left,
    Up,
}

impl SplitDirection {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "right" => Some(Self::Right),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "up" => Some(Self::Up),
            _ => None,
        }
    }
}

/// Target for `goto_split`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitFocus {
    Previous,
    Next,
    Up,
    Down,
    Left,
    Right,
}

impl SplitFocus {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "previous" => Some(Self::Previous),
            "next" => Some(Self::Next),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// A keybind action. Names follow Ghostty: `new_tab`, `goto_tab:3`,
/// `increase_font_size:1`, `unbind`, ...
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    NewTab,
    CloseSurface,
    NewSplit(SplitDirection),
    GotoSplit(SplitFocus),
    /// 1-based tab index; negative counts from the end (`-1` = last).
    GotoTab(i32),
    PreviousTab,
    NextTab,
    /// Move the current tab by a signed delta.
    MoveTab(i32),
    Copy,
    Paste,
    IncreaseFontSize(f32),
    DecreaseFontSize(f32),
    ResetFontSize,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    /// Jump the viewport by a signed number of shell prompts (negative =
    /// toward the top/older).
    JumpToPrompt(i32),
    ClearScreen,
    /// Toggle the scrollback search overlay.
    ToggleSearch,
    /// Toggle the settings panel.
    ToggleSettings,
    ReloadConfig,
    ToggleFullscreen,
    Quit,
    /// The special `unbind` action: removes the trigger's binding.
    Unbound,
}

impl Action {
    /// Parse `name` or `name:param`. Unknown names or bad params are errors.
    pub fn parse(s: &str) -> Result<Self, String> {
        let (name, param) = match s.split_once(':') {
            Some((n, p)) => (n.trim().to_ascii_lowercase(), Some(p.trim())),
            None => (s.trim().to_ascii_lowercase(), None),
        };
        match name.as_str() {
            "new_tab" => only(Self::NewTab, &name, param),
            "close_surface" => only(Self::CloseSurface, &name, param),
            "new_split" => {
                let p = req(&name, param)?;
                let dir = SplitDirection::parse(p)
                    .ok_or_else(|| format!("invalid new_split direction `{p}`"))?;
                Ok(Self::NewSplit(dir))
            }
            "goto_split" => {
                let p = req(&name, param)?;
                let focus = SplitFocus::parse(p)
                    .ok_or_else(|| format!("invalid goto_split target `{p}`"))?;
                Ok(Self::GotoSplit(focus))
            }
            "goto_tab" => {
                let n = int(&name, param)?;
                if n == 0 {
                    return Err("goto_tab requires a non-zero index".to_string());
                }
                Ok(Self::GotoTab(n))
            }
            "previous_tab" => only(Self::PreviousTab, &name, param),
            "next_tab" => only(Self::NextTab, &name, param),
            "move_tab" => Ok(Self::MoveTab(int(&name, param)?)),
            "copy_to_clipboard" | "copy" => only(Self::Copy, &name, param),
            "paste_from_clipboard" | "paste" => only(Self::Paste, &name, param),
            "increase_font_size" => Ok(Self::IncreaseFontSize(amount(&name, param)?)),
            "decrease_font_size" => Ok(Self::DecreaseFontSize(amount(&name, param)?)),
            "reset_font_size" => only(Self::ResetFontSize, &name, param),
            "scroll_page_up" => only(Self::ScrollPageUp, &name, param),
            "scroll_page_down" => only(Self::ScrollPageDown, &name, param),
            "scroll_to_top" => only(Self::ScrollToTop, &name, param),
            "scroll_to_bottom" => only(Self::ScrollToBottom, &name, param),
            "jump_to_prompt" => Ok(Self::JumpToPrompt(int(&name, param)?)),
            "clear_screen" => only(Self::ClearScreen, &name, param),
            "toggle_search" => only(Self::ToggleSearch, &name, param),
            "open_settings" | "toggle_settings" => only(Self::ToggleSettings, &name, param),
            "reload_config" => only(Self::ReloadConfig, &name, param),
            "toggle_fullscreen" => only(Self::ToggleFullscreen, &name, param),
            "quit" => only(Self::Quit, &name, param),
            "unbind" => only(Self::Unbound, &name, param),
            _ => Err(format!("unknown action `{name}`")),
        }
    }
}

/// The action takes no parameter.
fn only(action: Action, name: &str, param: Option<&str>) -> Result<Action, String> {
    match param {
        None => Ok(action),
        Some(_) => Err(format!("action `{name}` takes no parameter")),
    }
}

/// The action requires a non-empty parameter.
fn req<'a>(name: &str, param: Option<&'a str>) -> Result<&'a str, String> {
    match param {
        Some(p) if !p.is_empty() => Ok(p),
        _ => Err(format!("action `{name}` requires a parameter")),
    }
}

/// The action requires an integer parameter.
fn int(name: &str, param: Option<&str>) -> Result<i32, String> {
    req(name, param)?
        .parse()
        .map_err(|_| format!("action `{name}` requires an integer parameter"))
}

/// Optional positive number parameter, defaulting to 1.
fn amount(name: &str, param: Option<&str>) -> Result<f32, String> {
    let Some(p) = param else {
        return Ok(1.0);
    };
    let v: f32 = p
        .parse()
        .map_err(|_| format!("action `{name}` requires a number parameter"))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(format!("action `{name}` requires a positive number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_actions() {
        let cases = [
            ("new_tab", Action::NewTab),
            ("close_surface", Action::CloseSurface),
            ("previous_tab", Action::PreviousTab),
            ("next_tab", Action::NextTab),
            ("copy_to_clipboard", Action::Copy),
            ("copy", Action::Copy),
            ("paste_from_clipboard", Action::Paste),
            ("paste", Action::Paste),
            ("reset_font_size", Action::ResetFontSize),
            ("scroll_page_up", Action::ScrollPageUp),
            ("scroll_page_down", Action::ScrollPageDown),
            ("scroll_to_top", Action::ScrollToTop),
            ("scroll_to_bottom", Action::ScrollToBottom),
            ("clear_screen", Action::ClearScreen),
            ("reload_config", Action::ReloadConfig),
            ("toggle_fullscreen", Action::ToggleFullscreen),
            ("quit", Action::Quit),
            ("unbind", Action::Unbound),
        ];
        for (src, want) in cases {
            assert_eq!(Action::parse(src), Ok(want), "{src}");
        }
    }

    #[test]
    fn name_is_case_insensitive() {
        assert_eq!(Action::parse("NEW_TAB"), Ok(Action::NewTab));
        assert_eq!(
            Action::parse("New_Split:Right"),
            Ok(Action::NewSplit(SplitDirection::Right))
        );
    }

    #[test]
    fn new_split_params() {
        let cases = [
            ("new_split:right", SplitDirection::Right),
            ("new_split:down", SplitDirection::Down),
            ("new_split:left", SplitDirection::Left),
            ("new_split:up", SplitDirection::Up),
        ];
        for (src, dir) in cases {
            assert_eq!(Action::parse(src), Ok(Action::NewSplit(dir)), "{src}");
        }
        assert!(Action::parse("new_split:sideways").is_err());
        assert!(Action::parse("new_split").is_err());
        assert!(Action::parse("new_split:").is_err());
    }

    #[test]
    fn goto_split_params() {
        let cases = [
            ("goto_split:previous", SplitFocus::Previous),
            ("goto_split:next", SplitFocus::Next),
            ("goto_split:up", SplitFocus::Up),
            ("goto_split:down", SplitFocus::Down),
            ("goto_split:left", SplitFocus::Left),
            ("goto_split:right", SplitFocus::Right),
        ];
        for (src, focus) in cases {
            assert_eq!(Action::parse(src), Ok(Action::GotoSplit(focus)), "{src}");
        }
        assert!(Action::parse("goto_split:over").is_err());
        assert!(Action::parse("goto_split").is_err());
    }

    #[test]
    fn goto_tab_params() {
        assert_eq!(Action::parse("goto_tab:3"), Ok(Action::GotoTab(3)));
        assert_eq!(Action::parse("goto_tab:-1"), Ok(Action::GotoTab(-1)));
        assert!(Action::parse("goto_tab:0").is_err());
        assert!(Action::parse("goto_tab:first").is_err());
        assert!(Action::parse("goto_tab").is_err());
    }

    #[test]
    fn move_tab_params() {
        assert_eq!(Action::parse("move_tab:1"), Ok(Action::MoveTab(1)));
        assert_eq!(Action::parse("move_tab:-2"), Ok(Action::MoveTab(-2)));
        assert!(Action::parse("move_tab").is_err());
        assert!(Action::parse("move_tab:x").is_err());
    }

    #[test]
    fn font_size_params() {
        assert_eq!(
            Action::parse("increase_font_size"),
            Ok(Action::IncreaseFontSize(1.0))
        );
        assert_eq!(
            Action::parse("increase_font_size:2.5"),
            Ok(Action::IncreaseFontSize(2.5))
        );
        assert_eq!(
            Action::parse("decrease_font_size"),
            Ok(Action::DecreaseFontSize(1.0))
        );
        assert_eq!(
            Action::parse("decrease_font_size:0.5"),
            Ok(Action::DecreaseFontSize(0.5))
        );
        assert!(Action::parse("increase_font_size:0").is_err());
        assert!(Action::parse("increase_font_size:-1").is_err());
        assert!(Action::parse("increase_font_size:big").is_err());
    }

    #[test]
    fn param_on_paramless_action_is_error() {
        assert!(Action::parse("quit:now").is_err());
        assert!(Action::parse("new_tab:2").is_err());
        assert!(Action::parse("unbind:all").is_err());
    }

    #[test]
    fn unknown_action_is_error() {
        assert!(Action::parse("select_all").is_err());
        assert!(Action::parse("").is_err());
    }
}
