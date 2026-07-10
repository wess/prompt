use crate::mcp::{self, Outcome};
use crate::protocol::{err, ok, RpcRequest};
use crate::state::App;
use crate::tools;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Duration;

const SESSION_HEADER: &str = "mcp-session-id";

/// Single Streamable-HTTP endpoint. Immediate methods reply application/json;
/// tool calls stream over SSE so blocking `wait` calls can be held open with
/// keepalives.
pub async fn handle(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let incoming_session = headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let parsed: Result<Value, _> = serde_json::from_slice(&body);
    let Ok(value) = parsed else {
        return Json(err(Value::Null, -32700, "parse error")).into_response();
    };

    // JSON-RPC batching was removed in MCP 2025-06-18; reject arrays outright
    // rather than stalling a batched `wait` for its full timeout with no
    // keepalives on the connection.
    if value.is_array() {
        return Json(err(Value::Null, -32600, "batch requests are not supported")).into_response();
    }

    let Ok(req) = serde_json::from_value::<RpcRequest>(value) else {
        return Json(err(Value::Null, -32600, "invalid request")).into_response();
    };
    let is_initialize = req.method == "initialize";

    match mcp::route(req) {
        Outcome::Accepted => StatusCode::ACCEPTED.into_response(),
        Outcome::Now(v) => {
            let mut resp = Json(v).into_response();
            if is_initialize {
                let session = incoming_session
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if let Ok(hv) = HeaderValue::from_str(&session) {
                    resp.headers_mut().insert(SESSION_HEADER, hv);
                }
            }
            resp
        }
        Outcome::Tool { id, name, args } => {
            let session = incoming_session.unwrap_or_default();
            stream_tool(app, session, id, name, args)
        }
    }
}

/// Streamable-HTTP session teardown: a client DELETEs `/mcp` with its session
/// header when it shuts down; drop the binding so the sessions map does not
/// hold it forever (abandoned sessions also age out — see `App::bind`).
pub async fn end(State(app): State<App>, headers: HeaderMap) -> StatusCode {
    if let Some(session) = headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok()) {
        app.unbind(session).await;
    }
    StatusCode::NO_CONTENT
}

/// Run a tool call as a one-shot SSE stream: keepalive pings while it works,
/// then a single `message` event carrying the JSON-RPC response.
fn stream_tool(app: App, session: String, id: Value, name: String, args: Value) -> Response {
    let stream = async_stream::stream! {
        let reply = tools::call(&app, &session, &name, &args).await;
        let response = ok(id, reply.body);
        let data = serde_json::to_string(&response).unwrap_or_default();
        yield Ok::<Event, Infallible>(Event::default().event("message").data(data));
        // Reached only when the stream is polled past the yield, i.e. after the
        // response event was handed to the connection. A client that died first
        // drops the stream and never acks, so its drain is redelivered by the
        // next wait — the at-least-once half of the bus delivery contract.
        if let Some((agent, last)) = reply.ack {
            if let Err(e) = crate::bus::ack(&app, &agent, last).await {
                tracing::warn!("relay: delivery ack for '{agent}' failed: {e}");
            }
        }
    };

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn app() -> (App, std::path::PathBuf) {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("relay-transport-{}-{n}.db", std::process::id()));
        let pool = crate::db::open(path.to_str().unwrap()).await.unwrap();
        (App::new(pool, "http://127.0.0.1:0".into(), "t".into()), path)
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    async fn body_json(resp: Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Protocol 2025-06-18 removed JSON-RPC batching; an array body
    /// must be rejected with -32600, not processed (or stalled).
    #[tokio::test]
    async fn batch_requests_are_rejected() {
        let (app, path) = app().await;
        let body = Bytes::from(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
        let resp = handle(State(app), HeaderMap::new(), body).await;
        let v = body_json(resp).await;
        assert_eq!(v["error"]["code"], serde_json::json!(-32600));
        cleanup(&path);
    }

    /// A session DELETE drops the binding; the freed name needs a re-register.
    #[tokio::test]
    async fn delete_unbinds_the_session() {
        let (app, path) = app().await;
        app.bind("s1", "backend").await;
        let mut headers = HeaderMap::new();
        headers.insert(SESSION_HEADER, HeaderValue::from_static("s1"));
        let status = end(State(app.clone()), headers).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(app.name_of("s1").await, None);
        cleanup(&path);
    }
}
