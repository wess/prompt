//! Bridge the session's blocking std event channel into an async stream
//! that a gpui foreground task can poll.

use std::sync::mpsc::Receiver;

use futures::channel::mpsc::UnboundedReceiver;
use terminal::Event;

/// Spawn a thread that blocks on the std receiver and forwards every
/// event into an async channel. The thread ends when either side closes:
/// the session drops its sender (reader thread exits) or the consumer
/// drops the returned receiver.
///
/// If the thread cannot be spawned (thread exhaustion), the failure is
/// delivered as an [`Event::Exit`] on the returned channel, so the pane
/// tears itself down the same way a dead child would instead of the whole
/// app panicking.
pub fn forward(events: Receiver<Event>) -> UnboundedReceiver<Event> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let sender = tx.clone();
    let spawned = std::thread::Builder::new()
        .name("eventbridge".to_string())
        .spawn(move || {
            while let Ok(event) = events.recv() {
                if sender.unbounded_send(event).is_err() {
                    break;
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("sinclair: event bridge: {error}");
        let _ = tx.unbounded_send(Event::Exit(None));
    }
    rx
}

#[cfg(test)]
#[path = "../tests/bridge.rs"]
mod tests;
