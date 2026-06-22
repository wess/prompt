//! The stdio read/dispatch/write loop and JSON-RPC message shaping.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::{Tool, PROTOCOL_VERSION};

/// Runs one tool call: `(name, arguments) -> result | error message`. A
/// returned string is sent verbatim as text content; any other JSON value is
/// pretty-printed. An `Err` is reported to the client as a failed tool call
/// (`isError: true`) rather than a protocol error.
pub type Handler<'a> = dyn Fn(&str, &Value) -> Result<Value, String> + 'a;

/// Serve MCP over stdin/stdout until stdin closes. Blocks the calling thread;
/// intended to be the whole body of a `prompt mcp` subcommand.
pub fn serve(tools: Vec<Tool>, handler: &Handler<'_>) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let server_info = json!({ "name": "prompt", "version": env!("CARGO_PKG_VERSION") });

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
            // A malformed line cannot carry an id to answer; skip it.
            continue;
        };
        if let Some(reply) = dispatch(&msg, &tools, &server_info, handler) {
            // Best effort: a broken pipe means the client went away.
            if writeln!(stdout, "{reply}").and_then(|()| stdout.flush()).is_err() {
                break;
            }
        }
    }
}

/// Produce the reply for one message, or `None` for notifications (no id).
fn dispatch(msg: &Value, tools: &[Tool], server_info: &Value, handler: &Handler<'_>) -> Option<String> {
    let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
    // Notifications carry no id and never get a reply.
    let id = msg.get("id").cloned()?;

    let outcome = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": server_info,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list(tools) })),
        "tools/call" => return Some(call(&id, msg, handler)),
        other => Err((-32601, format!("method not found: {other}"))),
    };

    Some(match outcome {
        Ok(result) => ok(&id, result),
        Err((code, message)) => err(&id, code, &message),
    })
}

/// Handle `tools/call`: invoke the handler and wrap the result as MCP content.
fn call(id: &Value, msg: &Value, handler: &Handler<'_>) -> String {
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    if name.is_empty() {
        return err(id, -32602, "tools/call requires a tool name");
    }
    let empty = json!({});
    let args = params.get("arguments").unwrap_or(&empty);

    match handler(name, args) {
        Ok(value) => ok(id, content(value, false)),
        Err(message) => ok(id, content(Value::String(message), true)),
    }
}

/// Wrap a value as a single text-content tool result.
fn content(value: Value, is_error: bool) -> Value {
    let text = match value {
        Value::String(s) => s,
        other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string()),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

fn tool_list(tools: &[Tool]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            })
        })
        .collect()
}

fn ok(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn err(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Tool> {
        vec![Tool::new("echo", "echoes input", json!({ "type": "object" }))]
    }

    fn handle(name: &str, args: &Value) -> Result<Value, String> {
        match name {
            "echo" => Ok(args.clone()),
            _ => Err(format!("unknown tool {name}")),
        }
    }

    #[test]
    fn initialize_advertises_protocol_and_tools_capability() {
        let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });
        let reply: Value = serde_json::from_str(
            &dispatch(&msg, &tools(), &json!({ "name": "prompt" }), &handle).unwrap(),
        )
        .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert!(reply["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn notifications_get_no_reply() {
        let msg = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(dispatch(&msg, &tools(), &json!({}), &handle).is_none());
    }

    #[test]
    fn tools_list_returns_registered_tools() {
        let msg = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let reply: Value =
            serde_json::from_str(&dispatch(&msg, &tools(), &json!({}), &handle).unwrap()).unwrap();
        let list = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["name"], "echo");
    }

    #[test]
    fn tools_call_wraps_handler_result_as_text_content() {
        let msg = json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "echo", "arguments": { "hi": 1 } }
        });
        let reply: Value =
            serde_json::from_str(&dispatch(&msg, &tools(), &json!({}), &handle).unwrap()).unwrap();
        assert_eq!(reply["result"]["isError"], false);
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"hi\""));
    }

    #[test]
    fn handler_error_is_a_failed_tool_call_not_a_protocol_error() {
        let msg = json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "missing" }
        });
        let reply: Value =
            serde_json::from_str(&dispatch(&msg, &tools(), &json!({}), &handle).unwrap()).unwrap();
        assert!(reply["result"]["isError"].as_bool().unwrap());
        assert!(reply.get("error").is_none());
    }

    #[test]
    fn unknown_method_is_a_protocol_error() {
        let msg = json!({ "jsonrpc": "2.0", "id": 5, "method": "frobnicate" });
        let reply: Value =
            serde_json::from_str(&dispatch(&msg, &tools(), &json!({}), &handle).unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32601);
    }
}
