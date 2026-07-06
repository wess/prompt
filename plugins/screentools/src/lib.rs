//! screentools — the reference WASM plugin. A sandboxed capability plugin: it
//! reads the terminal screen through the gated `host-screen` interface and
//! exposes a `grep` tool to agents. No process spawning, no ambient filesystem —
//! exactly what the WASM tier is for. Built for `wasm32-wasip2`; `plugin.wasm`
//! is the committed artifact.

wit_bindgen::generate!({
    world: "screentools",
    path: "../../crates/pluginrt/wit",
});

use crate::exports::prompt::plugin::guest::Guest;
use crate::prompt::plugin::host_screen::read_screen;

struct Screentools;

impl Guest for Screentools {
    fn init() {}

    fn call_tool(name: String, params_json: String) -> Result<String, String> {
        if name != "grep" {
            return Err(format!("unknown tool: {name}"));
        }
        let params: serde_json::Value =
            serde_json::from_str(&params_json).map_err(|e| e.to_string())?;
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let lines = params.get("lines").and_then(serde_json::Value::as_u64).unwrap_or(200) as u32;

        let screen = read_screen(lines)?;
        let matches: Vec<&str> = screen
            .lines()
            .filter(|line| query.is_empty() || line.contains(query))
            .collect();
        let out = serde_json::json!({ "count": matches.len(), "matches": matches });
        Ok(out.to_string())
    }
}

export!(Screentools);
