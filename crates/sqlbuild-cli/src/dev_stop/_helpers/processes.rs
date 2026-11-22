//! Signals for the processes `sqb dev` started, and the terminal mode it changed.

#[cfg(target_os = "linux")]
use rustix::process::kill_process;
#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
#[cfg(unix)]
use rustix::termios::{OptionalActions, Termios, tcsetattr};

/// SIGKILL `group` when `kill`, else SIGTERM; a group that already exited is fine.
#[cfg(unix)]
pub(crate) fn signal_group(group: i32, kill: bool) {
    if let Some(pid) = Pid::from_raw(group) {
        let signal: Signal = if kill { Signal::KILL } else { Signal::TERM };
        let _ = kill_process_group(pid, signal);
    }
}

#[cfg(not(unix))]
pub(crate) fn signal_group(_group: i32, _kill: bool) {}

/// On Linux, also SIGKILL every direct child still running and the process group it leads, as a backstop for children nothing registered; registered groups, such as each local worker's, are killed on every Unix.
#[cfg(target_os = "linux")]
pub(crate) fn kill_children() {
    let own: i32 = rustix::process::getpid().as_raw_nonzero().get();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return;
    };
    for entry in entries.flatten() {
        let Some(Ok(pid)) = entry.file_name().to_str().map(str::parse::<i32>) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some(Ok(parent)) = parent_of(&stat)
            && parent == own
            && let Some(pid) = Pid::from_raw(pid)
        {
            let _ = kill_process_group(pid, Signal::KILL);
            let _ = kill_process(pid, Signal::KILL);
        }
    }
}

/// Elsewhere, children die with their registered groups, which `killpg` reaches on every Unix, or when their handles drop.
#[cfg(not(target_os = "linux"))]
pub(crate) fn kill_children() {}

/// The parent ID in a `/proc/<pid>/stat` line, read after the parenthesised command name.
#[cfg(target_os = "linux")]
fn parent_of(stat: &str) -> Option<Result<i32, std::num::ParseIntError>> {
    let rest: &str = &stat[stat.rfind(')')? + 1..];
    Some(rest.split_whitespace().nth(1)?.parse::<i32>())
}

/// Put stdin's terminal back in `cooked` mode.
#[cfg(unix)]
pub(crate) fn restore_termios(cooked: &Termios) {
    let _ = tcsetattr(std::io::stdin(), OptionalActions::Now, cooked);
}
