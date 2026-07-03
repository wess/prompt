//! Manages the bundled `relay` sidecar, the agent mesh. Prompt never runs the
//! mesh in-process; it starts/stops the bundled binary as a detached daemon and
//! launches agents into splits. Every parameter comes from settings, passed
//! explicitly on the command line (no environment variables).

use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::path::PathBuf;

/// A saved agent definition, shown under AI → Agents for relaunch.
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
}

fn defs_path() -> PathBuf {
    home().join("agents.json")
}

pub fn list_agent_defs() -> Vec<AgentDef> {
    std::fs::read(defs_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Save (upsert by name) an agent definition.
pub fn save_agent_def(def: AgentDef) {
    let mut defs = list_agent_defs();
    defs.retain(|d| d.name != def.name);
    defs.push(def);
    let _ = std::fs::create_dir_all(home());
    let _ = std::fs::write(defs_path(), serde_json::to_vec_pretty(&defs).unwrap_or_default());
}

/// Build the launch command for a previously-saved agent.
pub fn launch_saved_command(opts: &config::Options, name: &str) -> Option<String> {
    let def = list_agent_defs().into_iter().find(|d| d.name == name)?;
    Some(launch_agent_command(
        opts,
        &def.provider,
        &def.name,
        def.role.as_deref(),
        def.task.as_deref(),
    ))
}

/// The bundled `relay` binary: prefer a sibling of the running executable
/// (inside the app bundle / target dir), else fall back to PATH.
pub(crate) fn binary() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("relay");
            if cand.exists() {
                return cand.to_string_lossy().into_owned();
            }
        }
    }
    "relay".to_string()
}

/// Fixed state directory for the mesh, beside the config file, so every relay
/// call shares one mesh regardless of the calling pane's working directory.
fn home() -> PathBuf {
    config::default_path()
        .and_then(|p| p.parent().map(|d| d.join("relay")))
        .unwrap_or_else(|| PathBuf::from(".relay"))
}

fn home_str() -> String {
    home().to_string_lossy().into_owned()
}

fn db_str() -> String {
    home().join("relay.db").to_string_lossy().into_owned()
}

/// Run a relay subcommand without blocking the UI thread.
fn run_bg(args: Vec<String>) {
    let bin = binary();
    std::thread::spawn(move || {
        let _ = std::process::Command::new(bin).args(&args).output();
    });
}

fn start_args(opts: &config::Options) -> Vec<String> {
    vec![
        "--home".into(),
        home_str(),
        "start".into(),
        "--addr".into(),
        opts.relay_address.clone(),
        "--db".into(),
        db_str(),
    ]
}

/// Whether agent features (quick-launch, teams, the AI menu, the Relay sidebar)
/// are available. Enabling AI is enough — the server is started on demand by
/// [`ensure_running`] when the user actually launches something.
pub fn available(opts: &config::Options) -> bool {
    opts.ai_enabled
}

/// Whether the Relay server should run *persistently* — started at launch and
/// kept alive/reconciled across config reloads. This is the explicit "run the
/// mesh" opt-in; agent launching does not require it (see [`available`]).
pub fn enabled(opts: &config::Options) -> bool {
    opts.ai_enabled && opts.relay_enabled
}

/// Start the daemon at app launch, only if configured to auto-start.
pub fn on_launch(opts: &config::Options) {
    if enabled(opts) && opts.relay_start_on_launch {
        let _ = std::fs::create_dir_all(home());
        run_bg(start_args(opts));
    }
}

/// Ensure the daemon is up before launching an agent. Starts it synchronously
/// (the `start` command polls for health) if enabled but not running. Returns
/// whether it's running afterward.
pub fn ensure_running(opts: &config::Options) -> bool {
    if !available(opts) {
        return false;
    }
    if running() {
        return true;
    }
    let _ = std::fs::create_dir_all(home());
    let _ = std::process::Command::new(binary())
        .args(start_args(opts))
        .output();
    running()
}

/// Start the daemon now (AI → Relay → Start Server).
pub fn start(opts: &config::Options) {
    let _ = std::fs::create_dir_all(home());
    run_bg(start_args(opts));
}

/// Stop the daemon now (AI → Relay → Stop Server).
pub fn stop() {
    run_bg(vec!["--home".into(), home_str(), "stop".into()]);
}

/// Restart the daemon: stop, then start, as one background sequence so the
/// new instance never races the old one for the address.
pub fn restart(opts: &config::Options) {
    let _ = std::fs::create_dir_all(home());
    let bin = binary();
    let stop_args = vec!["--home".to_string(), home_str(), "stop".to_string()];
    let start = start_args(opts);
    std::thread::spawn(move || {
        let _ = std::process::Command::new(&bin).args(&stop_args).output();
        let _ = std::process::Command::new(&bin).args(&start).output();
    });
}

/// The address the running daemon is bound to, from its record.
fn bound_addr() -> Option<String> {
    let bytes = std::fs::read(home().join("server.json")).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v["addr"].as_str().map(str::to_string)
}

/// Reconcile the daemon with current settings after a config reload. A bare
/// `start` early-returns when a daemon is already up, so when the configured
/// address has changed under a live daemon we restart to rebind.
pub fn on_reload(opts: &config::Options) {
    let _ = std::fs::create_dir_all(home());
    if !available(opts) {
        // AI turned off entirely — tear the server down.
        run_bg(vec!["--home".into(), home_str(), "stop".into()]);
    } else if enabled(opts) {
        // Persistent mesh — keep it up and rebind if the address changed.
        if running() && bound_addr().as_deref() != Some(opts.relay_address.as_str()) {
            restart(opts);
        } else {
            run_bg(start_args(opts));
        }
    }
    // Otherwise AI is on but the mesh is on-demand: leave any server a launch
    // already started alone (don't force-start, don't stop it).
}

/// Shell command to stream the bus in a split.
pub fn feed_command() -> String {
    format!("\"{}\" --home \"{}\" feed --follow", binary(), home_str())
}

/// Enabled agent providers, in display order: built-ins that are toggled on,
/// then user-defined custom tools (by label).
pub fn enabled_agents(opts: &config::Options) -> Vec<String> {
    let mut v = Vec::new();
    if opts.agent_claude {
        v.push("claude".to_string());
    }
    if opts.agent_codex {
        v.push("codex".to_string());
    }
    if opts.agent_ollama {
        v.push("ollama".to_string());
    }
    if opts.agent_gemini {
        v.push("gemini".to_string());
    }
    for (label, _) in custom_tools(opts) {
        v.push(label);
    }
    v
}

/// Parse the `agent-custom` entries into `(label, command template)` pairs.
/// Each entry is `label|template`; malformed entries (no `|`, blank label or
/// template) are skipped.
pub fn custom_tools(opts: &config::Options) -> Vec<(String, String)> {
    opts.agent_custom
        .iter()
        .filter_map(|e| {
            let (label, tmpl) = e.split_once('|')?;
            let (label, tmpl) = (label.trim(), tmpl.trim());
            (!label.is_empty() && !tmpl.is_empty())
                .then(|| (label.to_string(), tmpl.to_string()))
        })
        .collect()
}

/// Whether a provider actually resolves on this machine. Built-ins are probed
/// with [`test_tool`] (honoring any configured explicit path); custom tools are
/// trusted, since their template is the user's own command. Blocking (spawns a
/// `--version` probe / TCP connect) — call it off the UI thread.
pub(crate) fn agent_verifies(opts: &config::Options, provider: &str) -> bool {
    let probe = |tool: &str, path: &Option<String>| test_tool(tool, path.as_deref()).is_ok();
    match provider {
        "claude" => probe("claude", &opts.agent_claude_path),
        "codex" => probe("codex", &opts.agent_codex_path),
        "gemini" => probe("gemini", &opts.agent_gemini_path),
        "ollama" => test_tool("ollama", None).is_ok(),
        _ => true,
    }
}

/// How to launch a provider: a built-in `--agent` (with an optional explicit
/// `--bin` path), or a custom `--cmd` template.
struct Resolved {
    agent: Option<String>,
    bin: Option<String>,
    custom: Option<String>,
}

/// Resolve a provider label to its launch shape using the configured paths and
/// custom tools. Unknown labels fall back to `--agent <label>`.
fn resolve_provider(opts: &config::Options, provider: &str) -> Resolved {
    let bin = |p: &Option<String>| p.clone().filter(|s| !s.trim().is_empty());
    match provider {
        "claude" => Resolved {
            agent: Some("claude".into()),
            bin: bin(&opts.agent_claude_path),
            custom: None,
        },
        "codex" => Resolved {
            agent: Some("codex".into()),
            bin: bin(&opts.agent_codex_path),
            custom: None,
        },
        "gemini" => Resolved {
            agent: Some("gemini".into()),
            bin: bin(&opts.agent_gemini_path),
            custom: None,
        },
        "ollama" => Resolved {
            agent: Some("ollama".into()),
            bin: None,
            custom: None,
        },
        other => {
            if let Some((_, tmpl)) = custom_tools(opts).into_iter().find(|(l, _)| l == other) {
                Resolved {
                    agent: None,
                    bin: None,
                    custom: Some(tmpl),
                }
            } else {
                Resolved {
                    agent: Some(other.to_string()),
                    bin: None,
                    custom: None,
                }
            }
        }
    }
}

/// Available role names (built-in + user + project), via the relay CLI.
pub fn role_list() -> Vec<String> {
    let Ok(out) = std::process::Command::new(binary())
        .args(["role", "list", "--json"])
        .output()
    else {
        return Vec::new();
    };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| r["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Build a `relay launch` command for a specific provider/name/role-or-task.
/// `opts` supplies any configured explicit binary path or custom command
/// template for the provider.
pub fn launch_agent_command(
    opts: &config::Options,
    provider: &str,
    name: &str,
    role: Option<&str>,
    task: Option<&str>,
) -> String {
    let r = resolve_provider(opts, provider);
    let mut s = format!(
        "{} --home {} launch {}",
        sh_quote(&binary()),
        sh_quote(&home_str()),
        sh_quote(name)
    );
    if let Some(agent) = &r.agent {
        s.push_str(&format!(" --agent {}", sh_quote(agent)));
    }
    if let Some(bin) = &r.bin {
        s.push_str(&format!(" --bin {}", sh_quote(bin)));
    }
    if let Some(tmpl) = &r.custom {
        s.push_str(&format!(" --cmd {}", sh_quote(tmpl)));
    }
    if let Some(r) = role.filter(|r| !r.is_empty()) {
        s.push_str(&format!(" --role {}", sh_quote(r)));
    }
    if let Some(t) = task.filter(|t| !t.is_empty()) {
        let t = if opts.ai_optimize_tokens { minimize_prompt(t) } else { t.to_string() };
        if !t.is_empty() {
            s.push_str(&format!(" --task {}", sh_quote(&t)));
        }
    }
    if opts.ai_optimize_tokens {
        s.push_str(" --optimize");
    }
    keep_open(s)
}

/// Immediately launch a configured provider (Claude Code, Codex, …) as a one-off
/// agent — the quick-launch menu entries. Reuses [`launch_agent_command`] (so the
/// token-optimization threading applies) with a generated unique name, the
/// default `worker` role, and no standing task.
pub fn quick_launch_command(opts: &config::Options, provider: &str) -> String {
    let name = unique_agent_name(provider);
    launch_agent_command(opts, provider, &name, None, None)
}

/// A friendly display name for a provider, for menus. Built-ins get their brand
/// name; custom tools already carry a user-chosen label, so pass it through.
pub fn provider_label(provider: &str) -> String {
    match provider {
        "claude" => "Claude Code".to_string(),
        "codex" => "Codex".to_string(),
        "gemini" => "Gemini".to_string(),
        "ollama" => "Ollama".to_string(),
        other => other.to_string(),
    }
}

/// A mesh name unlikely to collide across quick launches: the provider plus a
/// short suffix derived from the wall clock (seconds since the epoch, base-36).
fn unique_agent_name(provider: &str) -> String {
    let base: String = provider
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{base}-{}", radix36(secs))
}

/// Lower-case base-36 encoding of `n` (0-9a-z), for compact, readable suffixes.
fn radix36(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// Compact a prompt to spend fewer tokens without dropping content: strip
/// trailing whitespace from every line, collapse runs of spaces/tabs that
/// follow the leading indent into a single space (indentation is preserved so
/// pasted code keeps its shape), and squeeze runs of blank lines down to one.
/// Outer blank lines are trimmed off entirely.
pub(crate) fn minimize_prompt(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blank_run = false;
    for line in text.lines() {
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let body = &line[indent.len()..];
        let mut compact = String::with_capacity(body.len());
        let mut prev_space = false;
        for c in body.chars() {
            let is_space = c == ' ' || c == '\t';
            if is_space {
                if !prev_space {
                    compact.push(' ');
                }
            } else {
                compact.push(c);
            }
            prev_space = is_space;
        }
        let joined = format!("{indent}{}", compact.trim_end());
        if joined.trim().is_empty() {
            if !out.is_empty() {
                blank_run = true;
            }
        } else {
            if blank_run {
                out.push(String::new());
            }
            blank_run = false;
            out.push(joined);
        }
    }
    out.join("\n")
}

/// Single-quote a value for safe interpolation into a `/bin/sh -c` string:
/// wrap it in single quotes and escape any embedded single quote, making the
/// content inert to the shell (no word-splitting, globbing, or `$()`/`;`).
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Wrap a launch command so a failure leaves the pane open with the reason
/// (instead of the shell exiting and the pane vanishing). On success the agent
/// `exec`s and replaces the shell, so the fallback never runs.
fn keep_open(cmd: String) -> String {
    format!(
        "{cmd} || {{ echo; echo '[relay] launch failed — check Settings → AI (is the server running?)'; exec \"${{SHELL:-/bin/sh}}\"; }}"
    )
}

/// Names of available teams (built-in + user + project), via the relay CLI.
pub fn team_list() -> Vec<String> {
    let Ok(out) = std::process::Command::new(binary())
        .args(["team", "list", "--json"])
        .output()
    else {
        return Vec::new();
    };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null);
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// A team's layout shape and ordered `(member, role)` roster.
pub fn team_info(name: &str) -> Option<(String, Vec<(String, String)>)> {
    let out = std::process::Command::new(binary())
        .args(["team", "info", name, "--json"])
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let layout = v["layout"].as_str()?.to_string();
    let members = v["members"]
        .as_array()?
        .iter()
        .map(|m| {
            (
                m["name"].as_str().unwrap_or("agent").to_string(),
                m["role"].as_str().unwrap_or("worker").to_string(),
            )
        })
        .collect();
    Some((layout, members))
}

/// Shell command that launches one team member in a pane. The team's first
/// member is the human-driven `lead`, it stays interactive instead of parking
/// on the `wait`-loop, so the human can steer it.
pub fn launch_member(member: &str, role: &str, lead: bool, optimize: bool) -> String {
    let flag = if lead { " --lead" } else { "" };
    let opt = if optimize { " --optimize" } else { "" };
    keep_open(format!(
        "\"{}\" --home \"{}\" launch {member} --role {role}{flag}{opt}",
        binary(),
        home_str()
    ))
}

/// Path to the relay server's log file.
pub fn log_path() -> PathBuf {
    home().join("server.log")
}

/// Shell command to tail the relay server log in a split.
pub fn log_command() -> String {
    format!("tail -n 200 -F \"{}\"", log_path().display())
}

/// Probe whether a tool is reachable. CLIs are checked with `--version`; Ollama
/// is probed on its API port. `path`, when set, is the configured explicit
/// binary path to probe instead of looking the bare name up on PATH. Returns a
/// short detail on success or failure.
pub fn test_tool(tool: &str, path: Option<&str>) -> Result<String, String> {
    if tool == "ollama" {
        let addr: std::net::SocketAddr = "127.0.0.1:11434".parse().unwrap();
        return std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500))
            .map(|_| "Ollama reachable".to_string())
            .map_err(|_| "not running — start `ollama serve`".to_string());
    }
    let bin = path.map(str::trim).filter(|p| !p.is_empty()).unwrap_or(tool);
    match std::process::Command::new(bin).arg("--version").output() {
        Ok(out) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout);
            let line = v.lines().next().unwrap_or("ok").trim();
            Ok(if line.is_empty() { "ok".into() } else { line.to_string() })
        }
        Ok(_) => Err(format!("`{bin} --version` failed")),
        Err(_) => {
            if path.is_some() {
                Err(format!("`{bin}` not found — check the path"))
            } else {
                Err(format!("`{bin}` not found on PATH — set its path below"))
            }
        }
    }
}

/// Whether the relay server is actually listening (reads its record, probes it).
pub fn running() -> bool {
    let Ok(bytes) = std::fs::read(home().join("server.json")) else {
        return false;
    };
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let Some(addr) = v["addr"].as_str() else {
        return false;
    };
    addr.to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .map(|sa| {
            std::net::TcpStream::connect_timeout(&sa, std::time::Duration::from_millis(200)).is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "../tests/relay.rs"]
mod tests;
