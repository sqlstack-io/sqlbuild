//! The platform call that hands a child a new session and its controlling terminal.

use std::process::Command;

/// Before `command`'s program runs: `setsid`, then the terminal on stdin as the session's controlling terminal.
#[allow(unsafe_code)]
pub(crate) fn own_session_terminal(command: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: the hook runs in the forked child before exec; it allocates nothing and makes only the async-signal-safe `setsid` and `ioctl(TIOCSCTTY)` system calls on the inherited stdin descriptor.
    unsafe {
        command.pre_exec(|| {
            rustix::process::setsid()?;
            rustix::process::ioctl_tiocsctty(rustix::stdio::stdin())?;
            Ok(())
        })
    }
}
