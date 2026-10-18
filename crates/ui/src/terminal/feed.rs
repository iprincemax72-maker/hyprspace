use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use futures::channel::mpsc::UnboundedSender;
use gpui::Global;
use hyprspace_proto::{Client, Command, Event, SessionId};

use super::emulator::Emulator;

pub(super) type Shared = Arc<Mutex<Emulator>>;

pub(super) fn lock(emu: &Shared) -> MutexGuard<'_, Emulator> {
    emu.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone)]
pub(super) struct Feed {
    pub(super) emu: Shared,
    pub(super) client: Client,
    pub(super) fed: Arc<AtomicU64>,
    pub(super) shown: Arc<AtomicBool>,
    pub(super) pending: Arc<AtomicBool>,
    pub(super) wake: UnboundedSender<()>,
}

impl Feed {
    pub(super) fn take(&self, id: SessionId, bytes: &[u8]) {
        let reply = lock(&self.emu).feed(bytes);
        if !reply.is_empty() {
            self.client
                .send(Command::WriteTerminal { id, bytes: reply });
        }
        self.fed.fetch_add(1, Ordering::Relaxed);
        if self.shown.load(Ordering::Relaxed) {
            self.ring();
        }
    }

    pub(super) fn ring(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            let _ = self.wake.unbounded_send(());
        }
    }
}

#[derive(Clone, Default)]
pub struct Feeds(Arc<Mutex<HashMap<SessionId, Feed>>>);

impl Global for Feeds {}

impl Feeds {
    fn map(&self) -> MutexGuard<'_, HashMap<SessionId, Feed>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn add(&self, id: SessionId, feed: Feed) {
        self.map().insert(id, feed);
    }

    pub(super) fn remove(&self, id: SessionId) {
        self.map().remove(&id);
    }

    /// Parses terminal output here, off the main thread. Anything else comes back.
    pub fn route(&self, event: Event) -> Option<Event> {
        let Event::TerminalOutput { id, bytes } = event else {
            return Some(event);
        };
        let feed = self.map().get(&id).cloned();
        match feed {
            Some(feed) => {
                feed.take(id, &bytes);
                None
            }
            None => Some(Event::TerminalOutput { id, bytes }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(
        shown: bool,
    ) -> (
        Feed,
        futures::channel::mpsc::UnboundedReceiver<Command>,
        futures::channel::mpsc::UnboundedReceiver<()>,
    ) {
        let (tx, commands) = futures::channel::mpsc::unbounded();
        let (wake, rung) = futures::channel::mpsc::unbounded();
        let palette: super::super::emulator::Palette = |_| (0, 0, 0);
        let feed = Feed {
            emu: Arc::new(Mutex::new(Emulator::new(20, 4, true, palette))),
            client: Client::new(tx),
            fed: Arc::default(),
            shown: Arc::new(AtomicBool::new(shown)),
            pending: Arc::default(),
            wake,
        };
        (feed, commands, rung)
    }

    #[test]
    fn output_is_parsed_where_it_is_routed_and_queries_are_answered() {
        let feeds = Feeds::default();
        let (f, mut commands, mut rung) = feed(false);
        feeds.add(SessionId(7), f.clone());
        let out = |bytes: &[u8]| Event::TerminalOutput {
            id: SessionId(7),
            bytes: bytes.to_vec(),
        };
        assert!(feeds.route(out(b"hi\x1b[6n")).is_none());
        assert_eq!(lock(&f.emu).row_text(0), "hi");
        assert_eq!(
            commands.try_recv().ok(),
            Some(Command::WriteTerminal {
                id: SessionId(7),
                bytes: b"\x1b[1;3R".to_vec(),
            })
        );
        assert!(
            rung.try_recv().is_err(),
            "a hidden terminal never wakes the UI"
        );
        let other = Event::TerminalExit {
            id: SessionId(7),
            code: 0,
        };
        assert!(feeds.route(other).is_some());
        assert!(
            feeds
                .route(Event::TerminalOutput {
                    id: SessionId(8),
                    bytes: b"x".to_vec(),
                })
                .is_some()
        );
    }

    #[test]
    fn a_shown_terminal_wakes_the_ui_once_until_it_catches_up() {
        let feeds = Feeds::default();
        let (f, _commands, mut rung) = feed(true);
        feeds.add(SessionId(1), f.clone());
        for _ in 0..5 {
            feeds.route(Event::TerminalOutput {
                id: SessionId(1),
                bytes: b"x".to_vec(),
            });
        }
        assert!(rung.try_recv().is_ok());
        assert!(rung.try_recv().is_err());
        assert_eq!(f.fed.load(Ordering::Relaxed), 5);
        f.pending.store(false, Ordering::Release);
        f.take(SessionId(1), b"y");
        assert!(rung.try_recv().is_ok());
    }
}
