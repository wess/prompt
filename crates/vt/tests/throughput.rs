//! Parser throughput benchmark. Ignored by default (timing, not a pass/fail
//! assertion); run it with:
//!
//! ```text
//! cargo test -p vt --release --test throughput -- --ignored --nocapture
//! ```

use std::time::Instant;

/// Build a realistic chunk: printable text, SGR color changes, cursor moves,
/// and newlines — the mix a build log or `ls --color` produces.
fn sample() -> Vec<u8> {
    let mut out = Vec::new();
    for i in 0..2000 {
        out.extend_from_slice(b"\x1b[32m");
        out.extend_from_slice(format!("line {i:05} ").as_bytes());
        out.extend_from_slice(b"\x1b[0m\x1b[1mthe quick brown fox \x1b[0m");
        out.extend_from_slice("jumps over the lazy dog 0123456789\r\n".as_bytes());
        if i % 50 == 0 {
            out.extend_from_slice(b"\x1b[2J\x1b[H"); // periodic clear+home
        }
    }
    out
}

#[test]
#[ignore = "benchmark; run with --ignored --nocapture in release"]
fn feed_throughput() {
    let chunk = sample();
    let total_bytes = chunk.len();

    // Warm up.
    let mut term = vt::Terminal::new(120, 40, 10_000);
    term.feed(&chunk);

    let iterations = 64;
    let start = Instant::now();
    for _ in 0..iterations {
        let mut term = vt::Terminal::new(120, 40, 10_000);
        term.feed(&chunk);
        std::hint::black_box(term.cursor_pos());
    }
    let elapsed = start.elapsed();

    let bytes = (total_bytes * iterations) as f64;
    let mb = bytes / (1024.0 * 1024.0);
    let secs = elapsed.as_secs_f64();
    eprintln!(
        "vt feed: {mb:.1} MiB in {secs:.3}s = {:.0} MiB/s ({:.1} ns/byte)",
        mb / secs,
        elapsed.as_nanos() as f64 / bytes,
    );
    assert!(secs > 0.0);
}
