//! Single-instance IPC over a per-user unix socket.
//!
//! Two clients use it: `prompt --toggle-quick` summons the quick terminal
//! (the Wayland global-summon path, since a Wayland client cannot grab a
//! global hotkey itself), and `prompt mcp` bridges Model Context Protocol
//! tool calls into the running instance.
//!
//! The wire protocol is one newline-terminated JSON request per connection,
//! answered with one newline-terminated JSON response, then the connection
//! closes:
//!
//! ```text
//! request:  {"op":"run_command","args":{"text":"ls"}}
//! response: {"ok":true,"result":{ ... }}   |   {"ok":false,"error":"..."}
//! ```

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::App;
use serde_json::{json, Value};

const POLL: Duration = Duration::from_millis(60);
/// How long the server waits for a client to send its request line.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Per-user socket path: `$XDG_RUNTIME_DIR` on Linux, the temp dir otherwise.
fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("prompt-quick.sock")
}

/// Client: ask a running instance to toggle the quick terminal. Returns
/// whether one answered.
pub fn send_toggle() -> bool {
    match request("toggle_quick", &json!({})) {
        Ok(_) => true,
        Err(_) => {
            eprintln!("prompt: no running instance to toggle the quick terminal");
            false
        }
    }
}

/// Client: send one op to the running instance and return its result body, or
/// an error string (no instance, transport failure, or a server-side error).
pub fn request(op: &str, args: &Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket_path())
        .map_err(|_| "no running prompt instance".to_string())?;
    let line = json!({ "op": op, "args": args }).to_string();
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush())
        .map_err(|e| format!("write request: {e}"))?;
    let mut response = String::new();
    BufReader::new(&stream)
        .read_line(&mut response)
        .map_err(|e| format!("read response: {e}"))?;
    let value: Value =
        serde_json::from_str(response.trim()).map_err(|e| format!("bad response: {e}"))?;
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    } else {
        Err(value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error")
            .to_string())
    }
}

/// Server: own the socket (unless another instance already does) and service
/// one request per connection.
pub fn listen(cx: &mut App) {
    let Some(listener) = bind(&socket_path()) else {
        return;
    };
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    let executor = cx.background_executor().clone();
    cx.spawn(async move |cx| loop {
        executor.timer(POLL).await;
        while let Ok((stream, _)) = listener.accept() {
            serve(stream, cx).await;
        }
    })
    .detach();
}

/// Read one request from `stream`, dispatch it against app state, and write
/// the response back.
async fn serve(stream: UnixStream, cx: &gpui::AsyncApp) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let response = match serde_json::from_str::<Value>(line.trim()) {
        Ok(req) => {
            let op = req.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
            let args = req.get("args").cloned().unwrap_or(Value::Null);
            match cx.update(|cx| crate::mcpbridge::handle(&op, &args, cx)) {
                Ok(result) => json!({ "ok": true, "result": result }),
                Err(error) => json!({ "ok": false, "error": error }),
            }
        }
        Err(e) => json!({ "ok": false, "error": format!("bad request: {e}") }),
    };
    let mut stream = stream;
    let _ = stream
        .write_all(response.to_string().as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush());
}

/// Bind the socket, clearing a stale file left by a crashed instance.
fn bind(path: &Path) -> Option<UnixListener> {
    match UnixListener::bind(path) {
        Ok(listener) => Some(listener),
        // Exists: if nobody is listening it is stale — remove and retry, else
        // another live instance owns it and will service requests.
        Err(_) if UnixStream::connect(path).is_err() => {
            let _ = std::fs::remove_file(path);
            UnixListener::bind(path).ok()
        }
        Err(_) => None,
    }
}
