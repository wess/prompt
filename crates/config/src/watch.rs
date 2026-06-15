//! Config file reload support: a background thread polls the file's
//! mtime and fires a callback when it changes. No external dependencies.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// Stops the watcher thread when dropped.
pub struct WatchHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Watch `path` for mtime changes, polling every `interval`, and call
/// `on_change` whenever it differs (including the file appearing or
/// disappearing). Dropping the returned handle stops the thread promptly,
/// even with a long interval.
pub fn watch(
    path: PathBuf,
    interval: Duration,
    on_change: impl Fn() + Send + 'static,
) -> WatchHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let slice = Duration::from_millis(20);
        let mut last = mtime(&path);
        loop {
            // Sleep in short slices so drop is responsive.
            let mut waited = Duration::ZERO;
            while waited < interval {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let step = slice.min(interval - waited);
                std::thread::sleep(step);
                waited += step;
            }
            if flag.load(Ordering::Relaxed) {
                return;
            }
            let now = mtime(&path);
            if now != last {
                last = now;
                on_change();
            }
        }
    });
    WatchHandle { stop, thread: Some(thread) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    fn tempfile(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("promptwatchtest{}{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config")
    }

    #[test]
    fn fires_on_mtime_change_and_stops_on_drop() {
        let file = tempfile("fires");
        std::fs::write(&file, "font-size = 13\n").unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let handle = watch(file.clone(), Duration::from_millis(10), move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        // Bump the mtime until the watcher notices (bounded by a deadline).
        let f = std::fs::File::options().write(true).open(&file).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut bump = 10;
        while hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            f.set_modified(SystemTime::now() + Duration::from_secs(bump))
                .unwrap();
            bump += 10;
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1, "watcher never fired");

        // After drop the thread is joined: no further callbacks.
        drop(handle);
        let settled = hits.load(Ordering::SeqCst);
        f.set_modified(SystemTime::now() + Duration::from_secs(bump))
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(hits.load(Ordering::SeqCst), settled);
        std::fs::remove_dir_all(file.parent().unwrap()).ok();
    }

    #[test]
    fn unchanged_file_does_not_fire() {
        let file = tempfile("quiet");
        std::fs::write(&file, "a").unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let handle = watch(file.clone(), Duration::from_millis(10), move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(150));
        drop(handle);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        std::fs::remove_dir_all(file.parent().unwrap()).ok();
    }

    #[test]
    fn drop_is_prompt_with_long_interval() {
        let file = tempfile("longint");
        std::fs::write(&file, "a").unwrap();
        let handle = watch(file.clone(), Duration::from_secs(60), || {});
        let start = Instant::now();
        drop(handle);
        assert!(start.elapsed() < Duration::from_secs(1));
        std::fs::remove_dir_all(file.parent().unwrap()).ok();
    }

    #[test]
    fn file_appearing_fires() {
        let file = tempfile("appear");
        std::fs::remove_file(&file).ok();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let handle = watch(file.clone(), Duration::from_millis(10), move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(&file, "created").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1);
        drop(handle);
        std::fs::remove_dir_all(file.parent().unwrap()).ok();
    }
}
