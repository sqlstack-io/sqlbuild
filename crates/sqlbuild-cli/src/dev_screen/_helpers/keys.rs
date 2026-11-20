//! Keys from the terminal in raw mode: Ctrl+C and `q` stop `sqb dev` directly, typing at a sign-in prompt goes to the client.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

#[cfg(unix)]
use rustix::termios::{
    LocalModes, OptionalActions, SpecialCodeIndex, Termios, tcgetattr, tcsetattr,
};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(unix)]
use crate::dev_screen::constants::{INTERRUPT_BYTE, KEY_WAIT_TENTHS};
use crate::dev_screen::models::{KeyInput, ScreenMessage};
use crate::dev_stop::main::stop_switch::stop_switch;
#[cfg(unix)]
use crate::dev_stop::models::StopReason;

/// The key thread; dropping it stops the thread and restores the terminal.
pub(crate) struct KeyReader {
    input: Arc<KeyInput>,
    thread: Option<JoinHandle<()>>,
}

impl KeyReader {
    /// Put stdin in raw mode and read keys into `keys`, or `None` when stdin is not a terminal.
    #[cfg(unix)]
    pub(crate) fn start(keys: UnboundedSender<ScreenMessage>) -> Option<Self> {
        let cooked: Termios = match tcgetattr(std::io::stdin()) {
            Ok(cooked) => cooked,
            Err(_not_a_terminal) => return None,
        };
        stop_switch().save_cooked(cooked.clone());
        if let Err(_unchangeable) =
            tcsetattr(std::io::stdin(), OptionalActions::Now, &raw_mode(&cooked))
        {
            return None;
        }
        let input: Arc<KeyInput> = Arc::new(KeyInput::default());
        let shared: Arc<KeyInput> = Arc::clone(&input);
        let thread = std::thread::spawn(move || read_keys(&shared, &keys));
        Some(Self {
            input,
            thread: Some(thread),
        })
    }

    /// Keys need a Unix terminal; the screen is plain elsewhere.
    #[cfg(not(unix))]
    pub(crate) fn start(_keys: UnboundedSender<ScreenMessage>) -> Option<Self> {
        None
    }

    pub(crate) fn input(&self) -> Arc<KeyInput> {
        Arc::clone(&self.input)
    }
}

impl Drop for KeyReader {
    fn drop(&mut self) {
        self.input.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
        stop_switch().restore_terminal();
    }
}

/// No echo, no line buffering and no signal keys, so Ctrl+C arrives as a byte; reads give up after a tenth of a second.
#[cfg(unix)]
fn raw_mode(cooked: &Termios) -> Termios {
    let mut raw: Termios = cooked.clone();
    raw.local_modes -= LocalModes::ICANON | LocalModes::ECHO | LocalModes::ISIG;
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = KEY_WAIT_TENTHS;
    raw
}

#[cfg(unix)]
fn read_keys(input: &KeyInput, keys: &UnboundedSender<ScreenMessage>) {
    let mut byte: [u8; 1] = [0];
    while !input.stop.load(Ordering::SeqCst) {
        match rustix::io::read(std::io::stdin(), &mut byte) {
            Ok(1) => key(input, keys, byte[0]),
            Ok(_) => {}
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(_) => {
                stop_switch().request(StopReason::Hangup);
                return;
            }
        }
    }
}

#[cfg(unix)]
fn key(input: &KeyInput, keys: &UnboundedSender<ScreenMessage>, byte: u8) {
    if byte == INTERRUPT_BYTE {
        stop_switch().request(StopReason::Interrupt);
        return;
    }
    if let Ok(mut forward) = input.forward.lock()
        && let Some(client) = forward.as_mut()
    {
        drop(client.write_all(&[byte]));
        return;
    }
    match char::from(byte).to_ascii_lowercase() {
        'q' => stop_switch().request(StopReason::Quit),
        key => drop(keys.send(ScreenMessage::Key(key))),
    }
}
