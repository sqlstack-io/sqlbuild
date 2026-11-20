//! Why `sqb dev` stops, and the switch every stop request goes through.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[cfg(unix)]
use rustix::termios::Termios;
use tokio::sync::watch;

use crate::dev_stop::_helpers::processes::{kill_children, restore_termios, signal_group};
use crate::dev_stop::constants::{
    FORCED_LINE, INTERRUPT_STATUS, QUIT_STATUS, STOP_DEADLINE, STOP_GRACE, TERMINATE_STATUS,
};

/// What asked `sqb dev` to stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopReason {
    /// `q`.
    Quit,
    /// Ctrl+C or SIGINT.
    Interrupt,
    /// SIGTERM.
    Terminate,
}

impl StopReason {
    /// The exit status a shell expects for this reason.
    pub(crate) fn status(self) -> i32 {
        match self {
            Self::Quit => QUIT_STATUS,
            Self::Interrupt => INTERRUPT_STATUS,
            Self::Terminate => TERMINATE_STATUS,
        }
    }

    /// The result line's note, such as `(interrupted)`.
    pub(crate) fn note(self) -> &'static str {
        match self {
            Self::Quit => "",
            Self::Interrupt => " (interrupted)",
            Self::Terminate => " (terminated)",
        }
    }
}

/// Shared by the signal task, the key thread and the roles: the first request stops cleanly, a second Ctrl+C or signal forces it.
#[derive(Debug)]
pub(crate) struct StopSwitch {
    asked: watch::Sender<Option<StopReason>>,
    requests: AtomicUsize,
    /// Process groups of children that must not outlive `sqb dev`.
    groups: Mutex<Vec<i32>>,
    /// The terminal's cooked mode, put back however `sqb dev` exits.
    #[cfg(unix)]
    cooked: Mutex<Option<Termios>>,
    /// The terminal the screen writes to while it captures stderr.
    output: Mutex<Option<File>>,
    ready: AtomicBool,
    asked_at: Mutex<Option<Instant>>,
}

impl StopSwitch {
    pub(crate) fn new() -> Self {
        Self {
            asked: watch::channel(None).0,
            requests: AtomicUsize::new(0),
            groups: Mutex::new(Vec::new()),
            #[cfg(unix)]
            cooked: Mutex::new(None),
            output: Mutex::new(None),
            ready: AtomicBool::new(false),
            asked_at: Mutex::new(None),
        }
    }

    /// Ask to stop: the first request starts a clean stop with a deadline, a later Ctrl+C or signal exits at once.
    pub(crate) fn request(&'static self, reason: StopReason) {
        let earlier: usize = self.requests.fetch_add(1, Ordering::SeqCst);
        if earlier == 0 {
            if let Ok(mut asked_at) = self.asked_at.lock() {
                *asked_at = Some(Instant::now());
            }
            self.asked.send_replace(Some(reason));
            std::thread::spawn(move || {
                std::thread::sleep(STOP_GRACE);
                self.kill_all();
                std::thread::sleep(STOP_DEADLINE);
                self.exit_now(reason.status(), FORCED_LINE);
            });
            return;
        }
        if reason != StopReason::Quit {
            let first: StopReason = self.reason().unwrap_or(reason);
            self.exit_now(first.status(), FORCED_LINE);
        }
    }

    /// The first stop reason, once something asked.
    pub(crate) fn reason(&self) -> Option<StopReason> {
        *self.asked.borrow()
    }

    /// How long ago the stop was asked for.
    pub(crate) fn stopping_for(&self) -> Option<Duration> {
        self.asked_at
            .lock()
            .ok()
            .and_then(|asked_at| asked_at.map(|asked_at| asked_at.elapsed()))
    }

    /// Wait until something asks to stop.
    pub(crate) async fn requested(&self) -> StopReason {
        let mut asked = self.asked.subscribe();
        loop {
            if let Some(reason) = *asked.borrow_and_update() {
                return reason;
            }
            if asked.changed().await.is_err() {
                return StopReason::Terminate;
            }
        }
    }

    /// `pid` leads a process group that is killed with `sqb dev`.
    pub(crate) fn register_group(&self, pid: u32) {
        if let (Ok(pid), Ok(mut groups)) = (i32::try_from(pid), self.groups.lock()) {
            groups.push(pid);
        }
    }

    /// `pid`'s group has exited.
    pub(crate) fn forget_group(&self, pid: u32) {
        if let (Ok(pid), Ok(mut groups)) = (i32::try_from(pid), self.groups.lock()) {
            groups.retain(|group| *group != pid);
        }
    }

    /// Kill every registered group and every remaining child, such as local workers.
    pub(crate) fn kill_all(&self) {
        if let Ok(groups) = self.groups.lock() {
            for group in groups.iter() {
                signal_group(*group, true);
            }
        }
        kill_children();
    }

    /// Ask every registered group to stop.
    pub(crate) fn terminate_groups(&self) {
        if let Ok(groups) = self.groups.lock() {
            for group in groups.iter() {
                signal_group(*group, false);
            }
        }
    }

    /// Remember the terminal's cooked mode, to put back on every exit.
    #[cfg(unix)]
    pub(crate) fn save_cooked(&self, cooked: Termios) {
        if let Ok(mut saved) = self.cooked.lock() {
            saved.get_or_insert(cooked);
        }
    }

    /// Put the terminal's cooked mode back.
    pub(crate) fn restore_terminal(&self) {
        #[cfg(unix)]
        if let Ok(saved) = self.cooked.lock()
            && let Some(cooked) = saved.as_ref()
        {
            restore_termios(cooked);
        }
    }

    /// Where a forced stop writes while stderr is captured.
    pub(crate) fn save_output(&self, terminal: Option<File>) {
        if let Ok(mut output) = self.output.lock() {
            *output = terminal;
        }
    }

    /// The development services are up; a later failure is no longer a start-up failure.
    pub(crate) fn mark_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Kill everything, restore the terminal, say so, and exit with `status`.
    fn exit_now(&self, status: i32, line: &str) -> ! {
        self.kill_all();
        self.restore_terminal();
        let text: String = format!("\n{line}");
        match self.output.lock().ok().and_then(|output| {
            output
                .as_ref()
                .and_then(|terminal| terminal.try_clone().ok())
        }) {
            Some(mut terminal) => drop(terminal.write_all(text.as_bytes())),
            None => drop(std::io::stderr().write_all(text.as_bytes())),
        }
        std::process::exit(status)
    }
}

/// How a `sqb dev` session ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DevEnd {
    /// Something asked it to stop.
    Stopped(StopReason),
    /// It did not start: why, whether the screen already showed the problem, and what to try next.
    NotStarted {
        reason: String,
        explained: bool,
        next: Vec<(String, String)>,
    },
    /// A service failed after the services were ready.
    Failed { reason: String },
}
