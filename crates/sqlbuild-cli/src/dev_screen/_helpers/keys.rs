//! Keys from the terminal in raw mode: Ctrl+C and `q` stop `sqb dev`, Ctrl+\\ forces the stop, typing at a sign-in prompt goes to the client, and a terminal that hangs up stops it.

#[cfg(unix)]
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
use crate::dev_screen::constants::{
    FORCE_BYTE, INTERRUPT_BYTE, KEY_WAIT, SUSPEND_BYTE, SUSPEND_UNSUPPORTED,
};
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

/// No echo, no line buffering and no signal keys, so Ctrl+C, Ctrl+\\ and Ctrl+Z arrive as bytes; a read returns what is there without waiting.
#[cfg(unix)]
fn raw_mode(cooked: &Termios) -> Termios {
    let mut raw: Termios = cooked.clone();
    raw.local_modes -= LocalModes::ICANON | LocalModes::ECHO | LocalModes::ISIG;
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = 0;
    raw
}

/// Wait up to `KEY_WAIT` for a key; a terminal that is readable but has nothing to read has hung up.
#[cfg(unix)]
fn read_keys(input: &KeyInput, keys: &UnboundedSender<ScreenMessage>) {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let wait: Timespec = Timespec {
        tv_sec: 0,
        tv_nsec: i64::from(KEY_WAIT.subsec_nanos()),
    };
    let mut byte: [u8; 1] = [0];
    while !input.stop.load(Ordering::SeqCst) {
        let stdin = std::io::stdin();
        let mut ready: [PollFd<'_>; 1] = [PollFd::new(&stdin, PollFlags::IN)];
        match poll(&mut ready, Some(&wait)) {
            Ok(0) | Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Ok(_) => {}
            Err(_) => return hung_up(),
        }
        match rustix::io::read(&stdin, &mut byte) {
            Ok(1) => key(input, keys, byte[0]),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Ok(_) | Err(_) => return hung_up(),
        }
    }
}

/// The terminal went away: stop as SIGHUP would.
#[cfg(unix)]
fn hung_up() {
    stop_switch().request(StopReason::Hangup);
}

#[cfg(unix)]
fn key(input: &KeyInput, keys: &UnboundedSender<ScreenMessage>, byte: u8) {
    if byte == INTERRUPT_BYTE {
        stop_switch().request(StopReason::Interrupt);
        return;
    }
    if byte == FORCE_BYTE {
        stop_switch().force(StopReason::Interrupt);
    }
    if byte == SUSPEND_BYTE {
        drop(keys.send(ScreenMessage::Log(SUSPEND_UNSUPPORTED.to_owned())));
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
