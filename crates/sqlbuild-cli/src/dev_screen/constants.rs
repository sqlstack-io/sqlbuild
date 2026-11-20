//! Labels, bounds and keys of the `sqb dev` screen.

use std::time::Duration;

use crate::dev_screen::models::DevStep;

/// Detailed log lines kept in memory for `l`; older lines drop off the front.
pub(crate) const LOG_RING_LINES: usize = 500;
/// The keys before the services are ready.
pub(crate) const STARTING_KEYS: [(char, &str); 2] = [('l', "full log"), ('q', "quit")];
/// The keys once they are.
pub(crate) const READY_KEYS: [(char, &str); 4] = [
    ('o', "open UI"),
    ('l', "full log"),
    ('r', "recompile"),
    ('q', "quit"),
];
/// The snapshot ID characters a published line shows.
pub(crate) const SNAPSHOT_ID_CHARS: usize = 8;
/// The steps listed as pending until they start, in order; reading the configuration is never pending.
pub(crate) const STEPS: [DevStep; 7] = [
    DevStep::ControlStore,
    DevStep::Api,
    DevStep::Scheduler,
    DevStep::Discover,
    DevStep::Compile,
    DevStep::Publish,
    DevStep::Warehouse,
];
/// The warehouse step's detail while the client waits for the person.
pub(crate) const SIGN_IN_DETAIL: &str = "waiting for the sign-in above";
/// How often the running steps' spinners and timers redraw.
pub(crate) const TICK: Duration = Duration::from_millis(100);
/// How long the key reader waits for a key before it checks whether to stop.
pub(crate) const KEY_WAIT: Duration = Duration::from_millis(100);
/// Log lines left on screen when `sqb dev` stops on an error the screen did not explain.
pub(crate) const FAILURE_LOG_LINES: usize = 40;
/// Ctrl+C, read as a byte while the terminal is in raw mode.
pub(crate) const INTERRUPT_BYTE: u8 = 0x03;
/// Ctrl+\, read as a byte while the terminal is in raw mode: stop at once, without cancelling runs.
pub(crate) const FORCE_BYTE: u8 = 0x1c;
/// Ctrl+Z, read as a byte while the terminal is in raw mode; `sqb dev` does not suspend.
pub(crate) const SUSPEND_BYTE: u8 = 0x1a;
/// What the log says when Ctrl+Z is pressed.
pub(crate) const SUSPEND_UNSUPPORTED: &str =
    "Ctrl+Z does not suspend sqb dev; press q to stop it, or Ctrl+C";
