use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);

            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        if tx_clone.send(Event::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(event)) = reader.next() => {
                        let send_result = match event {
                            CrosstermEvent::Key(key) => tx_clone.send(Event::Key(key)),
                            CrosstermEvent::Resize(w, h) => tx_clone.send(Event::Resize(w, h)),
                            _ => Ok(()),
                        };
                        if send_result.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx, _tx: tx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
