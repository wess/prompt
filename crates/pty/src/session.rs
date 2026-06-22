//! A live pty session: master fd plus the attached child process.

use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::process::{Child, ExitStatus};

use crate::spawn::SpawnOptions;
use crate::unix;
use crate::winsize::Winsize;

/// A spawned child attached to a pty. Owns the master fd (closed on drop)
/// and the child handle.
pub struct Pty {
    master: OwnedFd,
    child: Child,
}

impl Pty {
    /// Open a pty pair, apply the initial winsize, spawn the child on the
    /// slave, then close the slave in the parent.
    pub fn spawn(opts: &SpawnOptions) -> io::Result<Self> {
        let pair = unix::open_pair()?;
        rustix::termios::tcsetwinsize(&pair.slave, opts.winsize.to_termios())?;
        let child = unix::spawn_child(opts, &pair)?;
        drop(pair.slave);
        Ok(Self {
            master: pair.master,
            child,
        })
    }

    /// Write bytes to the child's input. Blocking.
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        rustix::io::write(&self.master, buf).map_err(io::Error::from)
    }

    /// Read bytes of child output. Blocking; returns `Ok(0)` at EOF and
    /// `EIO` on Linux once the child side is fully closed.
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        rustix::io::read(&self.master, buf).map_err(io::Error::from)
    }

    /// A `File` over a duplicate of the master fd, for a reader thread.
    pub fn try_clone_reader(&self) -> io::Result<File> {
        Ok(File::from(self.master.try_clone()?))
    }

    /// A `File` over a duplicate of the master fd, for a writer.
    pub fn try_clone_writer(&self) -> io::Result<File> {
        Ok(File::from(self.master.try_clone()?))
    }

    /// Resize the terminal (TIOCSWINSZ on the master). The kernel delivers
    /// SIGWINCH to the child's process group.
    pub fn resize(&self, size: Winsize) -> io::Result<()> {
        rustix::termios::tcsetwinsize(&self.master, size.to_termios()).map_err(io::Error::from)
    }

    /// OS pid of the child.
    pub fn child_pid(&self) -> u32 {
        self.child.id()
    }

    /// Send SIGKILL to the child.
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    /// Wait for the child to exit, reaping it.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    /// Non-blocking check for child exit.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}

// The master OwnedFd closes itself when Pty drops; no explicit Drop needed.

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Read from the master until EOF or EIO (Linux reports EIO once the
    /// child side of the pty is gone).
    fn read_to_end(pty: &Pty) -> String {
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if e.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) => break,
                Err(e) => panic!("read failed: {e}"),
            }
        }
        String::from_utf8_lossy(&out).replace('\r', "")
    }

    #[test]
    fn echo_hello_round_trips() {
        let opts = SpawnOptions {
            winsize: Winsize::new(20, 5),
            ..SpawnOptions::command(vec!["/bin/echo".to_string(), "hello".to_string()])
        };
        let mut pty = Pty::spawn(&opts).expect("spawn echo");
        assert!(pty.child_pid() > 0);
        let output = read_to_end(&pty);
        let status = pty.wait().expect("wait echo");
        assert!(status.success());
        assert!(output.contains("hello"), "output was: {output:?}");
    }

    #[test]
    fn stty_reports_initial_winsize() {
        let opts = SpawnOptions {
            winsize: Winsize::new(80, 24),
            ..SpawnOptions::command(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "stty size".to_string(),
            ])
        };
        let mut pty = Pty::spawn(&opts).expect("spawn stty");
        let output = read_to_end(&pty);
        let status = pty.wait().expect("wait stty");
        assert!(status.success());
        assert!(output.contains("24 80"), "output was: {output:?}");
    }

    #[test]
    fn write_reaches_child_stdin() {
        let opts = SpawnOptions {
            winsize: Winsize::new(80, 24),
            ..SpawnOptions::command(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "read line; echo got:$line".to_string(),
            ])
        };
        let mut pty = Pty::spawn(&opts).expect("spawn reader");
        pty.write(b"ping\n").expect("write to pty");
        let output = read_to_end(&pty);
        pty.wait().expect("wait reader");
        assert!(output.contains("got:ping"), "output was: {output:?}");
    }

    #[test]
    fn resize_is_visible_to_child() {
        let opts = SpawnOptions {
            winsize: Winsize::new(80, 24),
            ..SpawnOptions::command(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 1; stty size".to_string(),
            ])
        };
        let mut pty = Pty::spawn(&opts).expect("spawn sleeper");
        pty.resize(Winsize::new(132, 43)).expect("resize");
        let output = read_to_end(&pty);
        pty.wait().expect("wait sleeper");
        assert!(output.contains("43 132"), "output was: {output:?}");
    }

    #[test]
    fn kill_terminates_child() {
        let opts = SpawnOptions::command(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 30".to_string(),
        ]);
        let mut pty = Pty::spawn(&opts).expect("spawn sleeper");
        pty.kill().expect("kill child");
        let status = pty.wait().expect("wait killed child");
        assert!(!status.success());
    }

    #[test]
    fn cloned_reader_reads_output() {
        use std::io::Read;
        let opts = SpawnOptions::command(vec!["/bin/echo".to_string(), "clone".to_string()]);
        let pty = Pty::spawn(&opts).expect("spawn echo");
        let mut reader = pty.try_clone_reader().expect("clone reader");
        let mut out = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
        let text = String::from_utf8_lossy(&out).replace('\r', "");
        assert!(text.contains("clone"), "output was: {text:?}");
    }
}
