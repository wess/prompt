use crate::protocol::{err, ok, RpcRequest};
use crate::tools;
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// What the transport should do with a parsed request.
pub enum Outcome {
    /// Immediate JSON-RPC response (application/json).
    Now(Value),
    /// A tool call to run, possibly long-lived, streamed over SSE.
    Tool { id: Value, name: String, args: Value },
    /// A notification, no response body, just 202.
    Accepted,
}

/// Route a single JSON-RPC message. Tool calls are handed back to the transport
/// so it can stream the (possibly blocking) result with keepalives.
pub fn route(req: RpcRequest) -> Outcome {
    let id = req.id.clone().unwrap_or(Value::Null);
    if req.jsonrpc.as_deref() != Some("2.0") {
        return Outcome::Now(err(id, -32600, "invalid request: expected jsonrpc \"2.0\""));
    }
    match req.method.as_str() {
        "initialize" => Outcome::Now(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "relay", "version": env!("CARGO_PKG_VERSION") }
            }),
        )),
        "ping" => Outcome::Now(ok(id, json!({}))),
        "tools/list" => Outcome::Now(ok(id, tools::list())),
        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if req.id.is_none() {
                return Outcome::Accepted;
            }
            Outcome::Tool { id, name, args }
        }
        _ if req.id.is_none() => Outcome::Accepted,
        other => Outcome::Now(err(id, -32601, &format!("method not found: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(jsonrpc: Option<&str>, method: &str) -> RpcRequest {
        RpcRequest {
            jsonrpc: jsonrpc.map(str::to_string),
            id: Some(json!(1)),
            method: method.into(),
            params: Value::Null,
        }
    }

    /// `jsonrpc` was carried but never validated; a request without
    /// the `"2.0"` marker must be rejected with -32600 per spec.
    #[test]
    fn missing_or_wrong_jsonrpc_is_invalid() {
        for bad in [None, Some("1.0"), Some("")] {
            let Outcome::Now(v) = route(req(bad, "ping")) else {
                panic!("expected an immediate error for jsonrpc {bad:?}");
            };
            assert_eq!(v["error"]["code"], json!(-32600));
        }
    }

    #[test]
    fn valid_jsonrpc_still_routes() {
        let Outcome::Now(v) = route(req(Some("2.0"), "ping")) else {
            panic!("ping should answer immediately");
        };
        assert!(v.get("error").is_none());
    }
}
