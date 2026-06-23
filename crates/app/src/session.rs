//! Session spawning policy: config to options, cwd inheritance, titles.

use std::path::PathBuf;

use terminal::SessionOptions;

/// Map configuration onto session options. `inherit` is the previous
/// pane's working directory (from OSC 7); it wins over the configured
/// `working_directory`, which wins over the default (home).
pub fn options(
    opts: &config::Options,
    cols: usize,
    rows: usize,
    inherit: Option<PathBuf>,
) -> SessionOptions {
    let mut session = SessionOptions {
        cols,
        rows,
        scrollback_limit: opts.scrollback_limit,
        ..SessionOptions::default()
    };
    if let Some(command) = &opts.shell {
        let argv: Vec<String> = command.split_whitespace().map(str::to_string).collect();
        if !argv.is_empty() {
            session.spawn = pty::SpawnOptions::command(argv);
        }
    }
    session.spawn.cwd = inherit
        .or_else(|| opts.working_directory.as_ref().map(PathBuf::from))
        .or_else(home);
    session
}

/// The user's home directory, the default working directory when no pane cwd
/// is inherited and the config sets none. Without it the child would inherit
/// the launcher's cwd — e.g. `/` when Prompt is opened from Finder.
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Shell program basename, used as a pane-title fallback.
pub fn shellname(shell: Option<&str>) -> String {
    let argv0 = shell
        .and_then(|s| s.split_whitespace().next())
        .map(str::to_string)
        .unwrap_or_else(pty::default_shell);
    std::path::Path::new(&argv0)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or(argv0)
}

/// Parse an OSC 7 working-directory report into a path. Accepts a
/// `file://host/path` URL (host ignored, percent-encoding decoded) or a
/// plain absolute path. Anything else is `None`.
pub fn cwdpath(osc: &str) -> Option<PathBuf> {
    if let Some(rest) = osc.strip_prefix("file://") {
        let path = &rest[rest.find('/')?..];
        return Some(PathBuf::from(percentdecode(path)));
    }
    osc.starts_with('/').then(|| PathBuf::from(osc))
}

/// Decode `%XX` escapes; malformed escapes pass through verbatim.
fn percentdecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let decoded = (bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit())
        .then(|| u8::from_str_radix(&s[i + 1..i + 3], 16).ok())
        .flatten();
        match decoded {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_is_login_shell() {
        let opts = config::Options::default();
        let session = options(&opts, 100, 30, None);
        assert_eq!((session.cols, session.rows), (100, 30));
        assert_eq!(session.scrollback_limit, 10_000);
        assert!(session.spawn.login);
        // With no inherit and no configured directory, defaults to home.
        assert_eq!(session.spawn.cwd, home());
    }

    #[test]
    fn options_honors_command_and_cwd() {
        let mut opts = config::Options::default();
        opts.shell = Some("/bin/bash -i".to_string());
        opts.working_directory = Some("/tmp".to_string());
        opts.scrollback_limit = 42;
        let session = options(&opts, 80, 24, None);
        assert_eq!(session.spawn.argv, vec!["/bin/bash", "-i"]);
        assert!(!session.spawn.login);
        assert_eq!(session.spawn.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(session.scrollback_limit, 42);
    }

    #[test]
    fn options_empty_command_falls_back_to_shell() {
        let mut opts = config::Options::default();
        opts.shell = Some("   ".to_string());
        let session = options(&opts, 80, 24, None);
        assert!(session.spawn.login);
        assert!(!session.spawn.argv.is_empty());
    }

    #[test]
    fn options_inherited_cwd_beats_config() {
        let mut opts = config::Options::default();
        opts.working_directory = Some("/tmp".to_string());
        let session = options(&opts, 80, 24, Some(PathBuf::from("/work")));
        assert_eq!(session.spawn.cwd, Some(PathBuf::from("/work")));
    }

    #[test]
    fn shellname_takes_basename_of_first_word() {
        assert_eq!(shellname(Some("/bin/bash -i")), "bash");
        assert_eq!(shellname(Some("zsh")), "zsh");
        assert_eq!(shellname(Some("/usr/local/bin/fish --login")), "fish");
    }

    #[test]
    fn shellname_defaults_to_user_shell() {
        assert!(!shellname(None).is_empty());
        assert!(!shellname(Some("   ")).is_empty());
    }

    #[test]
    fn cwdpath_parses_file_urls() {
        assert_eq!(
            cwdpath("file://host/Users/me"),
            Some(PathBuf::from("/Users/me"))
        );
        assert_eq!(cwdpath("file:///tmp"), Some(PathBuf::from("/tmp")));
        assert_eq!(
            cwdpath("file://host/a%20dir/b"),
            Some(PathBuf::from("/a dir/b"))
        );
    }

    #[test]
    fn cwdpath_accepts_plain_paths_rejects_junk() {
        assert_eq!(cwdpath("/var/log"), Some(PathBuf::from("/var/log")));
        assert_eq!(cwdpath("relative/path"), None);
        assert_eq!(cwdpath(""), None);
        assert_eq!(cwdpath("file://hostonly"), None);
    }

    #[test]
    fn percentdecode_handles_malformed_escapes() {
        assert_eq!(percentdecode("/a%2fb"), "/a/b");
        assert_eq!(percentdecode("/x%zz"), "/x%zz");
        assert_eq!(percentdecode("/trail%2"), "/trail%2");
        assert_eq!(percentdecode("/plain"), "/plain");
        // Multi-byte characters after % must not split a codepoint.
        assert_eq!(percentdecode("/x%éy"), "/x%éy");
    }
}
