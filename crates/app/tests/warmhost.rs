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

/// A server that answers exactly one request and then exits.
fn one_shot_server() -> (&'static str, Vec<String>) {
    (
        "sh",
        vec![
            "-c".to_string(),
            "IFS= read -r line; printf '%s\\n' \"$line\"".to_string(),
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
fn dead_process_respawns_on_next_request() {
    let mut warm = WarmPlugins::new();
    let (program, args) = one_shot_server();
    let cwd = Path::new(".");
    assert_eq!(warm.request("x", program, &args, cwd, "a").unwrap(), "a");
    // The child exited after its single reply; the next request must respawn
    // a fresh one rather than wedge on the dead pipe.
    assert_eq!(warm.request("x", program, &args, cwd, "b").unwrap(), "b");
}

#[test]
fn closed_output_reports_an_error_and_recovers() {
    let mut warm = WarmPlugins::new();
    let cwd = Path::new(".");
    // Exits without answering: the request errors instead of blocking...
    let quiet = ("sh", vec!["-c".to_string(), "exit 0".to_string()]);
    assert!(warm.request("q", quiet.0, &quiet.1, cwd, "a").is_err());
    // ...and the slot recovers with a working server afterward.
    let (program, args) = echo_server();
    assert_eq!(warm.request("q", program, &args, cwd, "b").unwrap(), "b");
}
