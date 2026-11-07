//! The `sqb` executable's whole process.

use std::ffi::OsString;
use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use crate::launcher::_helpers::dispatch::launch;
use crate::launcher::_helpers::handoff::process_exit;

/// Run this process's invocation; an exit code beyond one byte ends the process directly.
pub fn run_launcher() -> ExitCode {
    let started = (Instant::now(), SystemTime::now());
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    process_exit(launch(arguments, started))
}
