//! Terminal runtime: ties a pty session to the vt emulation core.
//!
//! [`Session::spawn`] starts a child process on a pty, feeds its output into
//! a [`vt::Terminal`] on a dedicated reader thread, and reports [`Event`]s
//! (wakeups, title changes, bell, exit) to the embedder over a channel.

#![cfg(unix)]

pub mod event;
pub mod options;
pub mod session;

pub use event::Event;
pub use options::SessionOptions;
pub use session::Session;
