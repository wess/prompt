//! Unix pseudo-terminal management for the Prompt terminal emulator.
//!
//! Open a pty pair, spawn a shell (or any argv) on the slave, and drive the
//! master: read child output, write input, resize, kill/wait.
//!
//! ```no_run
//! let opts = pty::SpawnOptions::default(); // user's login shell
//! let session = pty::Pty::spawn(&opts).unwrap();
//! session.resize(pty::Winsize::new(120, 40)).unwrap();
//! ```

#![cfg(unix)]

mod session;
mod spawn;
mod unix;
mod winsize;

pub use session::Pty;
pub use spawn::{default_env, default_shell, SpawnOptions};
pub use unix::{open_pair, spawn_child, PtyPair};
pub use winsize::Winsize;
