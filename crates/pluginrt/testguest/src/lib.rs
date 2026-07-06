//! The Stage-1 example guest: a WASM plugin exercising the tool path and a gated
//! host call. Built for `wasm32-wasip2`; the resulting component is checked in
//! as a test fixture for `pluginrt`. Not part of the workspace.

wit_bindgen::generate!({
    world: "example",
    path: "../wit",
});

use crate::exports::prompt::plugin::guest::Guest;
use crate::prompt::plugin::host_commands::run_command;
use crate::prompt::plugin::types::CommandTarget;

struct Example;

impl Guest for Example {
    fn init() {}

    fn call_tool(name: String, params_json: String) -> Result<String, String> {
        match name.as_str() {
            // Pure: echoes its params (no host call).
            "echo" => Ok(params_json),
            // Uses the gated `host-commands` interface — only reachable when the
            // plugin was granted the `commands` capability.
            "run" => run_command("echo hi", CommandTarget::Pane).map(|()| "{\"ran\":true}".to_string()),
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

export!(Example);
