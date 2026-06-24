//! Core bus operations shared by the MCP tools and the plain-HTTP control plane
//! (used by non-MCP participants like the Ollama bridge).

use crate::db;
use crate::protocol::Message;
use crate::state::App;
use std::time::Duration;

/// Park until messages addressed to `name` arrive (when `block`), then drain
/// them and advance the read cursor. Returns the delivered messages (empty on
/// timeout or when not blocking and the inbox is empty).
pub async fn await_messages(app: &App, name: &str, block: bool, max_wait: Duration) -> Vec<Message> {
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        // Arm the wake signal before querying, so a message inserted during the
        // query still wakes us.
        let notified = app.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        let cursor = db::cursor_of(&app.db, name).await.unwrap_or(0);
        let pending = db::pending_for(&app.db, name, cursor)
            .await
            .unwrap_or_default();
        if !pending.is_empty() {
            if let Some(last) = pending.last() {
                let _ = db::advance_cursor(&app.db, name, last.id).await;
            }
            return pending;
        }
        if !block {
            return Vec::new();
        }
        tokio::select! {
            _ = &mut notified => continue,
            _ = tokio::time::sleep_until(deadline) => return Vec::new(),
        }
    }
}

/// Insert a message and wake parked waiters.
pub async fn deliver(
    app: &App,
    from: &str,
    kind: &str,
    target: Option<&str>,
    body: &str,
) -> anyhow::Result<i64> {
    let id = db::insert_message(&app.db, from, kind, target, body).await?;
    app.wake();
    Ok(id)
}
