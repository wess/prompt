//! A live terminal session: pty child + vt emulation + reader thread.
//!
//! Both backends expose the same [`Session`] surface. On Unix the reader
//! thread blocks in `poll()` on the master plus a wake pipe (nonblocking
//! writes, deterministic teardown via group SIGHUP/SIGKILL). On Windows the
//! reader blocks in ConPTY pipe reads and teardown closes the console,
//! which unblocks the reader at EOF.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::Session;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Session;
