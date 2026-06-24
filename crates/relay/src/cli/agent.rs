use anyhow::{anyhow, Result};

/// A resolved command to run an agent.
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
}

/// Inputs needed to construct an agent launch.
pub struct Spec<'a> {
    pub agent: &'a str,
    pub custom: Option<&'a str>,
    pub name: &'a str,
    pub role: &'a str,
    pub prompt: &'a str,
    pub mcp_file: &'a str,
    pub url: &'a str,
    pub headless: bool,
    pub model: Option<&'a str>,
    pub channels: &'a [String],
    pub skip_perms: bool,
}

/// The wait-loop harness every agent receives as its opening instruction.
/// `brief` is the optional role description; `task` is the per-launch focus.
pub fn harness_prompt(
    name: &str,
    role: &str,
    brief: &str,
    channels: &[String],
    task: Option<&str>,
) -> String {
    let join = if channels.is_empty() {
        String::new()
    } else {
        format!("- After registering, `join` these channels: {}.\n", channels.join(", "))
    };
    let brief = if brief.trim().is_empty() {
        String::new()
    } else {
        format!("\nYour role:\n{}\n", brief.trim())
    };
    let task = task
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!("\nYour standing focus: {}\n", t.trim()))
        .unwrap_or_default();
    format!(
        "You are \"{name}\", a {role} connected to the Relay mesh via the `relay` MCP tools.\n\
         Protocol — follow exactly:\n\
         - Call `register` with name=\"{name}\" and role=\"{role}\" first.\n\
         {join}\
         - Call `wait` to receive work; it blocks until a message arrives.\n\
         - Do the requested work in this session, then report back with `send` to the \
         message's sender (or `post` to the relevant channel).\n\
         - ALWAYS end your turn by calling `wait` again so you stay reachable. \
         Never stop the wait-loop.\n\
         {brief}{task}"
    )
}

/// Build the command to launch `agent`, wiring in the relay MCP server.
pub fn build(spec: &Spec) -> Result<Launch> {
    if let Some(tmpl) = spec.custom {
        return Ok(from_template(tmpl, spec));
    }
    match spec.agent {
        "claude" => Ok(claude(spec)),
        "codex" => Ok(codex(spec)),
        "ollama" => ollama(spec),
        "gemini" => Ok(from_template(gemini_template(), spec)),
        other => Err(anyhow!(
            "unknown agent '{other}'. Use --agent claude|codex|ollama|gemini, or pass --cmd with a template."
        )),
    }
}

fn claude(spec: &Spec) -> Launch {
    let mut args: Vec<String> = Vec::new();
    if spec.headless {
        args.extend([
            "-p".into(),
            spec.prompt.into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ]);
        if spec.skip_perms {
            args.push("--dangerously-skip-permissions".into());
        }
    } else {
        args.push(spec.prompt.into());
    }
    args.extend([
        "--mcp-config".into(),
        spec.mcp_file.into(),
        "--strict-mcp-config".into(),
    ]);
    if let Some(m) = spec.model {
        args.extend(["--model".into(), m.into()]);
    }
    Launch {
        program: "claude".into(),
        args,
    }
}

/// Codex speaks streamable-HTTP MCP, wired via `-c mcp_servers.relay.url`.
fn codex(spec: &Spec) -> Launch {
    let mcp = format!("mcp_servers.relay.url=\"{}\"", spec.url);
    let mut args: Vec<String> = Vec::new();
    if spec.headless {
        args.push("exec".into());
        args.push(spec.prompt.into());
        args.push("-c".into());
        args.push("approval_policy=\"never\"".into());
    } else {
        args.push(spec.prompt.into());
    }
    args.push("-c".into());
    args.push(mcp);
    if let Some(m) = spec.model {
        args.push("-c".into());
        args.push(format!("model=\"{m}\""));
    }
    Launch {
        program: "codex".into(),
        args,
    }
}

/// Ollama is not an agent CLI; relay runs its own bridge loop as `relay agent
/// ollama ...`, which drives the model and speaks to the bus over the control
/// plane.
fn ollama(spec: &Spec) -> Result<Launch> {
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();
    let mut args = vec![
        "agent".into(),
        "ollama".into(),
        "--name".into(),
        spec.name.into(),
        "--role".into(),
        spec.role.into(),
        "--url".into(),
        spec.url.into(),
        "--system".into(),
        spec.prompt.into(),
    ];
    if let Some(m) = spec.model {
        args.push("--model".into());
        args.push(m.into());
    }
    for ch in spec.channels {
        args.push("--channel".into());
        args.push(ch.clone());
    }
    Ok(Launch { program: exe, args })
}

// NOTE: gemini MCP wiring is best-effort — adjust here or pass --cmd.
fn gemini_template() -> &'static str {
    "gemini --mcp-config {mcp} --prompt {prompt}"
}

/// Split a template into argv, substituting placeholders per token so that
/// {prompt} stays a single argument even though it contains spaces.
fn from_template(tmpl: &str, spec: &Spec) -> Launch {
    let mut tokens = tmpl.split_whitespace().map(|t| subst(t, spec));
    let program = tokens.next().unwrap_or_default();
    Launch {
        program,
        args: tokens.collect(),
    }
}

fn subst(token: &str, spec: &Spec) -> String {
    token
        .replace("{prompt}", spec.prompt)
        .replace("{mcp}", spec.mcp_file)
        .replace("{url}", spec.url)
        .replace("{name}", spec.name)
}
