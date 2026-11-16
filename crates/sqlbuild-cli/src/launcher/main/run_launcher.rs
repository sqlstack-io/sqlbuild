//! The `sqb` executable's whole process.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use crate::launcher::_helpers::dispatch::launch;
use crate::launcher::_helpers::handoff::process_exit;
use crate::launcher::main::install_native_build_identity::install_native_build_identity;

/// Run this process's invocation; an exit code beyond one byte ends the process directly.
pub fn run_launcher() -> ExitCode {
    if let Err(error) = install_native_build_identity() {
        let _ = writeln!(std::io::stderr(), "sqb: {error}");
        return ExitCode::FAILURE;
    }
    let started = (Instant::now(), SystemTime::now());
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    process_exit(launch(arguments, started))
}
