//! Hotkeys from the terminal: stdin in raw mode, handed back to the warehouse client while it prompts.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use rustix::termios::{
    LocalModes, OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::dev_screen::constants::KEY_POLL;

/// Who may read stdin: the hotkey thread, or a foreground child while the gate is closed.
#[derive(Debug, Default)]
pub(crate) struct KeyGate {
    closed: AtomicBool,
    /// Whether stdin is in raw mode and the thread reads it.
    reading: AtomicBool,
    stop: AtomicBool,
}

impl KeyGate {
    /// Restore the terminal and stop reading keys; returns once the thread has let go of stdin.
    pub(crate) async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        while self.reading.load(Ordering::SeqCst) {
            tokio::time::sleep(KEY_POLL).await;
        }
    }

    /// Read keys again.
    pub(crate) fn open(&self) {
        self.closed.store(false, Ordering::SeqCst);
    }
}

/// The hotkey thread; dropping it restores the terminal.
pub(crate) struct KeyReader {
    gate: Arc<KeyGate>,
    thread: Option<JoinHandle<()>>,
}

impl KeyReader {
    /// Start reading keys into `keys`, or `None` when stdin is not a terminal.
    pub(crate) fn start(keys: UnboundedSender<char>) -> Option<Self> {
        let cooked: Termios = tcgetattr(std::io::stdin()).ok()?;
        let gate: Arc<KeyGate> = Arc::new(KeyGate::default());
        let shared: Arc<KeyGate> = Arc::clone(&gate);
        let thread = std::thread::spawn(move || read_keys(&shared, &cooked, &keys));
        Some(Self {
            gate,
            thread: Some(thread),
        })
    }

    pub(crate) fn gate(&self) -> Arc<KeyGate> {
        Arc::clone(&self.gate)
    }
}

impl Drop for KeyReader {
    fn drop(&mut self) {
        self.gate.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

/// Keys without echo or line buffering; Ctrl-C still interrupts, and reads give up after a tenth of a second.
fn raw_mode(cooked: &Termios) -> Termios {
    let mut raw: Termios = cooked.clone();
    raw.local_modes -= LocalModes::ICANON | LocalModes::ECHO;
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = 1;
    raw
}

fn read_keys(gate: &KeyGate, cooked: &Termios, keys: &UnboundedSender<char>) {
    let raw: Termios = raw_mode(cooked);
    let mut byte: [u8; 1] = [0];
    while !gate.stop.load(Ordering::SeqCst) && !keys.is_closed() {
        let reading: bool = gate.reading.load(Ordering::SeqCst);
        if gate.closed.load(Ordering::SeqCst) {
            if reading {
                drop(tcsetattr(std::io::stdin(), OptionalActions::Now, cooked));
                gate.reading.store(false, Ordering::SeqCst);
            }
            std::thread::sleep(KEY_POLL);
            continue;
        }
        if !reading {
            if tcsetattr(std::io::stdin(), OptionalActions::Now, &raw).is_err() {
                return;
            }
            gate.reading.store(true, Ordering::SeqCst);
        }
        match rustix::io::read(std::io::stdin(), &mut byte) {
            Ok(1) => drop(keys.send(char::from(byte[0]).to_ascii_lowercase())),
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    drop(tcsetattr(std::io::stdin(), OptionalActions::Now, cooked));
    gate.reading.store(false, Ordering::SeqCst);
}
