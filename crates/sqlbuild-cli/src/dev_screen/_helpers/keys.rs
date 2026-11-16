//! Hotkeys from the terminal: stdin in raw mode, handed back to the warehouse client while it prompts.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use rustix::termios::{
    LocalModes, OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr,
};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(unix)]
use crate::dev_screen::constants::KEY_POLL;
use crate::dev_screen::models::{KeyGate, ScreenMessage};

/// The hotkey thread; dropping it restores the terminal.
pub(crate) struct KeyReader {
    gate: Arc<KeyGate>,
    thread: Option<JoinHandle<()>>,
}

impl KeyReader {
    /// Start reading keys into `keys`, or `None` when stdin is not a terminal.
    #[cfg(unix)]
    pub(crate) fn start(keys: UnboundedSender<ScreenMessage>) -> Option<Self> {
        let cooked: Termios = match tcgetattr(std::io::stdin()) {
            Ok(cooked) => cooked,
            Err(_) => return None,
        };
        let gate: Arc<KeyGate> = Arc::new(KeyGate::default());
        let shared: Arc<KeyGate> = Arc::clone(&gate);
        let thread = std::thread::spawn(move || read_keys(&shared, &cooked, &keys));
        Some(Self {
            gate,
            thread: Some(thread),
        })
    }

    /// Hotkeys need a Unix terminal; the screen is plain elsewhere.
    #[cfg(not(unix))]
    pub(crate) fn start(_keys: UnboundedSender<ScreenMessage>) -> Option<Self> {
        None
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
#[cfg(unix)]
fn raw_mode(cooked: &Termios) -> Termios {
    let mut raw: Termios = cooked.clone();
    raw.local_modes -= LocalModes::ICANON | LocalModes::ECHO;
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = 1;
    raw
}

#[cfg(unix)]
fn read_keys(gate: &KeyGate, cooked: &Termios, keys: &UnboundedSender<ScreenMessage>) {
    let raw: Termios = raw_mode(cooked);
    let mut byte: [u8; 1] = [0];
    while !gate.stop.load(Ordering::SeqCst) && !keys.is_closed() {
        let reading: bool = gate.reading.load(Ordering::SeqCst);
        if gate.closed.load(Ordering::SeqCst) {
            if reading {
                let _ = tcsetattr(std::io::stdin(), OptionalActions::Now, cooked);
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
            Ok(1) => drop(keys.send(ScreenMessage::Key(char::from(byte[0]).to_ascii_lowercase()))),
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let _ = tcsetattr(std::io::stdin(), OptionalActions::Now, cooked);
    gate.reading.store(false, Ordering::SeqCst);
}
