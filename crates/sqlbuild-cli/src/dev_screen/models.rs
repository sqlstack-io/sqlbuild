//! The state of the `sqb dev` screen and the messages that change it.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use sqlbuild_core::screen_text::models::{DevHeader, PhaseState, ScreenFacts, WarehouseState};
use sqlbuild_orchestration::dev_events::models::DevEvent;

/// One startup step of the checklist, in the order it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DevStep {
    ControlStore,
    Api,
    Scheduler,
    Discover,
    Compile,
    Publish,
    Warehouse,
}

/// What the screen hears about: its own steps, the location child, the warehouse, logs and keys.
#[derive(Clone, Debug)]
pub(crate) enum ScreenMessage {
    /// A step began; `detail` says what it waits on, such as `snowflake \u{b7} waiting for sign-in`.
    Started {
        step: DevStep,
        detail: String,
    },
    Finished {
        step: DevStep,
        detail: String,
    },
    Child(DevEvent),
    /// The development location's ID, known once the control store is open.
    Location(String),
    /// The dev target's triggers, such as `41 paused`, read after the first publication.
    Triggers(String),
    /// The foreground warehouse connection's outcome.
    Warehouse {
        adapter: String,
        state: WarehouseState,
    },
    /// A line of the warehouse client's own output, drawn inside the framed block.
    ClientOutput(String),
    /// A run started, or finished with `ok`; `detail` names its origin or node count.
    Run {
        name: String,
        detail: String,
        finished: Option<(bool, Duration)>,
    },
    /// A detailed log line, kept for `l`.
    Log(String),
    Key(char),
    /// Redraw the running step's spinner and timer.
    Tick,
}

/// What the screen asks its owner to do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScreenAction {
    None,
    OpenUi,
    Recompile,
    RetryWarehouse,
    Quit,
}

/// The text one message writes and the action it asks for.
#[derive(Clone, Debug)]
pub(crate) struct ScreenOutput {
    pub(crate) text: String,
    pub(crate) action: ScreenAction,
}

/// One checklist row: its state, detail and when it started.
#[derive(Clone, Debug)]
pub(crate) struct StepRow {
    pub(crate) step: DevStep,
    pub(crate) state: PhaseState,
    pub(crate) detail: String,
    pub(crate) started: Option<Instant>,
    pub(crate) elapsed: Option<Duration>,
}

/// What the screen knows before the first step starts.
#[derive(Clone, Debug)]
pub(crate) struct ScreenSetup {
    pub(crate) live: bool,
    pub(crate) facts: ScreenFacts,
    pub(crate) header: DevHeader,
    pub(crate) watching: String,
    pub(crate) project_root: String,
}

/// Everything the screen draws, and how many live rows sit below the last printed line.
#[derive(Clone, Debug)]
pub(crate) struct DevScreen {
    /// Redraws in place with a spinner and hotkeys; otherwise plain stable lines.
    pub(crate) live: bool,
    pub(crate) facts: ScreenFacts,
    pub(crate) header: DevHeader,
    pub(crate) ui_url: String,
    pub(crate) watching: String,
    pub(crate) triggers: String,
    pub(crate) started: Instant,
    pub(crate) steps: Vec<StepRow>,
    pub(crate) ready: bool,
    pub(crate) location: String,
    pub(crate) adapter: String,
    pub(crate) warehouse: Option<WarehouseState>,
    /// When the generation being compiled started, for its event line's time.
    pub(crate) generation_started: Option<Instant>,
    /// Changed files since the last generation finished.
    pub(crate) pending_changes: usize,
    /// The newest published generation, which keeps serving after a failure.
    pub(crate) serving: Option<i64>,
    /// The last compile's summary, such as `412 python nodes, 2280 models`.
    pub(crate) compile_detail: String,
    /// The project directory, which error positions link into.
    pub(crate) project_root: String,
    /// The warehouse client is writing inside the framed block; nothing is redrawn until it ends.
    pub(crate) client_output: bool,
    /// The client's last output ended mid-line, such as at a prompt.
    pub(crate) client_partial: bool,
    /// The ready card was printed and no line follows it yet; the first one is set apart by a blank row.
    pub(crate) event_gap: bool,
    pub(crate) log: VecDeque<String>,
    pub(crate) show_log: bool,
    pub(crate) frame: usize,
    /// Rows drawn below the last permanent line, redrawn by the next message.
    pub(crate) drawn: usize,
}

/// Who may read stdin: the hotkey thread, or a foreground child while the gate is closed.
#[derive(Debug, Default)]
pub(crate) struct KeyGate {
    pub(crate) closed: AtomicBool,
    /// Whether stdin is in raw mode and the thread reads it.
    pub(crate) reading: AtomicBool,
    pub(crate) stop: AtomicBool,
}

impl KeyGate {
    /// Restore the terminal and stop reading keys; returns once the thread has let go of stdin.
    pub(crate) async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        while self.reading.load(Ordering::SeqCst) {
            tokio::time::sleep(crate::dev_screen::constants::KEY_POLL).await;
        }
    }

    /// Read keys again.
    pub(crate) fn open(&self) {
        self.closed.store(false, Ordering::SeqCst);
    }
}

/// A running screen: where to send messages, the actions its keys ask for, and how to stop it.
#[derive(Debug)]
pub(crate) struct DevScreenHandle {
    pub(crate) messages: UnboundedSender<ScreenMessage>,
    pub(crate) actions: UnboundedReceiver<ScreenAction>,
    /// Closed while a foreground child owns the terminal's input; `None` without hotkeys.
    pub(crate) keys: Option<Arc<KeyGate>>,
    pub(crate) finish: oneshot::Sender<bool>,
    pub(crate) task: JoinHandle<()>,
}
