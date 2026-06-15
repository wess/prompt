//! Bridge the session's blocking std event channel into an async stream
//! that a gpui foreground task can poll.

use std::sync::mpsc::Receiver;

use futures::channel::mpsc::UnboundedReceiver;
use terminal::Event;

/// Spawn a thread that blocks on the std receiver and forwards every
/// event into an async channel. The thread ends when either side closes:
/// the session drops its sender (reader thread exits) or the consumer
/// drops the returned receiver.
pub fn forward(events: Receiver<Event>) -> UnboundedReceiver<Event> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    std::thread::Builder::new()
        .name("eventbridge".to_string())
        .spawn(move || {
            while let Ok(event) = events.recv() {
                if tx.unbounded_send(event).is_err() {
                    break;
                }
            }
        })
        .expect("spawn event bridge thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn forwards_in_order_and_closes() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Event::Wakeup).unwrap();
        tx.send(Event::TitleChanged("t".to_string())).unwrap();
        tx.send(Event::Exit(Some(0))).unwrap();
        drop(tx);
        let mut stream = forward(rx);
        let collected = futures::executor::block_on(async {
            let mut seen = Vec::new();
            while let Some(event) = stream.next().await {
                seen.push(event);
            }
            seen
        });
        assert_eq!(
            collected,
            vec![
                Event::Wakeup,
                Event::TitleChanged("t".to_string()),
                Event::Exit(Some(0)),
            ]
        );
    }
}
