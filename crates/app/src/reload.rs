//! Bridge the config-file watcher's background callback into an async
//! stream the gpui foreground can poll, so edits trigger a live reload.

use std::time::Duration;

use config::WatchHandle;
use futures::channel::mpsc::UnboundedReceiver;

/// Poll interval for the config file. Coarse enough to coalesce an
/// editor's multi-write save, fine enough to feel immediate.
const INTERVAL: Duration = Duration::from_millis(250);

/// Start watching the default config file. Returns the watch handle (keep
/// it alive to keep watching) and a stream that yields once per change,
/// including the file first appearing. `None` when there is no config path
/// (e.g. no `HOME`), in which case live reload is simply unavailable.
pub fn watch() -> Option<(WatchHandle, UnboundedReceiver<()>)> {
    let path = config::default_path()?;
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let handle = config::watch(path, INTERVAL, move || {
        let _ = tx.unbounded_send(());
    });
    Some((handle, rx))
}
