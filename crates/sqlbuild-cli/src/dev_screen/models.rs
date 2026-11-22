//! The state of the `sqb dev` screen and the messages that change it.

use std::collections::VecDeque;
use std::fs::File;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use sqlbuild_core::screen_text::models::{
    DevHeader, PhaseState, Problem, ResultLine, ScreenFacts, WarehouseState,
};
use sqlbuild_orchestration::dev_events::models::{DevEvent, UnsetProviderSecrets};

/// One start-up step, in the order pending steps are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DevStep {
    Config,
    ControlStore,
    Api,
    Scheduler,
    Discover,
    Compile,
    Publish,
    Warehouse,
}

/// What the screen hears about: its own steps, the location child, the warehouse, logs, keys and the stop.
#[derive(Clone, Debug)]
pub(crate) enum ScreenMessage {
    /// A step began; `detail` says what it waits on.
    Started {
        step: DevStep,
        detail: String,
    },
    Finished {
        step: DevStep,
        detail: String,
    },
    /// A step of `sqb dev` itself failed, with the problem to show under it.
    Failed {
        step: DevStep,
        detail: String,
        problem: Problem,
    },
    Child(DevEvent),
    /// The development location's ID, known once the control store is open; plain output prints it with `ready`.
    Location(String),
    /// The dev target's triggers, such as `41 paused`, read after the first publication.
    Triggers(String),
    /// The foreground warehouse connection's outcome.
    Warehouse {
        adapter: String,
        state: WarehouseState,
    },
    /// Text the warehouse client wrote while it connects, such as a sign-in prompt.
    ClientOutput(String),
    /// A run started, or finished with `ok`; `detail` names its origin or outcome.
    Run {
        name: String,
        detail: String,
        finished: Option<(bool, Duration)>,
    },
    /// A detailed log line, kept for `l`.
    Log(String),
    Key(char),
    /// Redraw the running steps' spinners and timers.
    Tick,
    /// The session is stopping; the live rows give way to one line per stopped service.
    Stopping,
    /// A stop is cancelling a run first, such as `Cancelling run orders_job… 2 queries (Ctrl+C again to force)`.
    Cancelling {
        label: String,
    },
    /// A service stopped, such as `Stopped UI server`, after `elapsed`.
    Stopped {
        label: String,
        elapsed: Duration,
    },
}

/// What the screen asks its owner to do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScreenAction {
    None,
    OpenUi,
    Recompile,
}

/// The text one message writes and the action it asks for.
#[derive(Clone, Debug)]
pub(crate) struct ScreenOutput {
    pub(crate) text: String,
    pub(crate) action: ScreenAction,
}

/// One step: its state, detail and when it started.
#[derive(Clone, Debug)]
pub(crate) struct StepRow {
    pub(crate) step: DevStep,
    pub(crate) state: PhaseState,
    pub(crate) detail: String,
    pub(crate) started: Option<Instant>,
    pub(crate) elapsed: Option<Duration>,
}

/// The warehouse client's text while it waits for the person, and since when.
#[derive(Clone, Debug)]
pub(crate) struct Prompt {
    pub(crate) text: String,
    pub(crate) since: Instant,
}

/// What the screen knows before the first step starts.
#[derive(Clone, Debug)]
pub(crate) struct ScreenSetup {
    pub(crate) live: bool,
    pub(crate) facts: ScreenFacts,
    pub(crate) header: DevHeader,
    /// The configured adapter, such as `snowflake`, for the sign-in prompt; empty when unknown.
    pub(crate) adapter: String,
    pub(crate) project_root: String,
    /// The env file row of the ready box, such as `.env · 3 variables · edit it, then press r to reload`.
    pub(crate) env_file: Option<String>,
}

/// Everything the screen draws, and the live rows it last drew below the printed lines.
#[derive(Clone, Debug)]
pub(crate) struct DevScreen {
    /// Redraws in place with spinners and keys; otherwise appended lines only.
    pub(crate) live: bool,
    pub(crate) facts: ScreenFacts,
    pub(crate) header: DevHeader,
    pub(crate) header_printed: bool,
    pub(crate) ui_url: String,
    pub(crate) location: String,
    /// The env file row of the ready box.
    pub(crate) env_file: Option<String>,
    /// Providers the latest health read shows missing secrets; drawn as a warning row until a read shows none.
    pub(crate) missing_secrets: Vec<UnsetProviderSecrets>,
    /// The env file that health read looked in.
    pub(crate) secrets_env_file: Option<String>,
    pub(crate) triggers: String,
    pub(crate) steps: Vec<StepRow>,
    pub(crate) ready: bool,
    pub(crate) adapter: String,
    pub(crate) warehouse: Option<WarehouseState>,
    /// When the generation being compiled started, for its event line's time.
    pub(crate) generation_started: Option<Instant>,
    /// Changed files since the last generation finished.
    pub(crate) pending_changes: usize,
    /// The snapshot that serves, which keeps serving after a failure.
    pub(crate) serving: Option<String>,
    /// The last compile's summary, such as `412 python nodes, 2280 models`.
    pub(crate) compile_detail: String,
    /// The project directory, which error positions link into.
    pub(crate) project_root: String,
    pub(crate) prompt: Option<Prompt>,
    /// Plain mode: the client's last text ended mid-line, such as at a prompt.
    pub(crate) client_partial: bool,
    pub(crate) stopping: Option<Instant>,
    pub(crate) log: VecDeque<String>,
    pub(crate) show_log: bool,
    pub(crate) frame: usize,
    /// The live rows drawn below the last printed line, redrawn by the next message.
    pub(crate) drawn: Vec<String>,
}

/// Keys from the terminal: where they go while a foreground child reads them, and when to stop.
#[derive(Debug, Default)]
pub(crate) struct KeyInput {
    /// A sign-in prompt's pseudo-terminal; typed keys go there instead of to the screen.
    pub(crate) forward: Mutex<Option<File>>,
    pub(crate) stop: AtomicBool,
}

/// How the session ended: the result line, and whether to leave the newest log lines above it.
#[derive(Clone, Debug)]
pub(crate) struct ScreenEnd {
    pub(crate) result: ResultLine,
    pub(crate) show_log: bool,
}

/// A running screen: where to send messages, the actions its keys ask for, and how to stop it.
#[derive(Debug)]
pub(crate) struct DevScreenHandle {
    pub(crate) messages: UnboundedSender<ScreenMessage>,
    pub(crate) actions: UnboundedReceiver<ScreenAction>,
    /// The key reader's input; `None` without a terminal to read keys from.
    pub(crate) keys: Option<std::sync::Arc<KeyInput>>,
    pub(crate) finish: oneshot::Sender<ScreenEnd>,
    pub(crate) task: JoinHandle<()>,
}
