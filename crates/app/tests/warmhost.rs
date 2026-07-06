use super::*;
use std::path::Path;

/// A tiny stdio echo server: reads a line, writes it back, forever.
fn echo_server() -> (&'static str, Vec<String>) {
    (
        "sh",
        vec![
            "-c".to_string(),
            "while IFS= read -r line; do printf '%s\\n' \"$line\"; done".to_string(),
        ],
    )
}

#[test]
fn warm_process_persists_across_requests() {
    let mut warm = WarmPlugins::new();
    let (program, args) = echo_server();
    let cwd = Path::new(".");
    // Two requests, one persistent process — the warm-tier win.
    assert_eq!(warm.request("echo", program, &args, cwd, "{\"n\":1}").unwrap(), "{\"n\":1}");
    assert_eq!(warm.request("echo", program, &args, cwd, "{\"n\":2}").unwrap(), "{\"n\":2}");
}

#[test]
fn evicted_process_respawns_on_next_request() {
    let mut warm = WarmPlugins::new();
    let (program, args) = echo_server();
    let cwd = Path::new(".");
    assert_eq!(warm.request("x", program, &args, cwd, "a").unwrap(), "a");
    warm.evict("x");
    assert_eq!(warm.request("x", program, &args, cwd, "b").unwrap(), "b");
}
