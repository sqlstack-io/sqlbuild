//! Find a program installed beside this one and run it with the same arguments and streams; the `sqlbuild` forwarder compiles this file into its own binary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The file `name` beside this program or beside the file it links to.
pub(crate) fn beside(program: &Path, name: &str) -> Option<PathBuf> {
    let resolved = std::fs::canonicalize(program).unwrap_or_else(|_| program.to_path_buf());
    [program, resolved.as_path()]
        .into_iter()
        .filter_map(Path::parent)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// Replace this process with `target`; returns only the error that kept it from starting.
#[cfg(unix)]
pub(crate) fn hand_off(target: &Path, arguments: &[OsString]) -> std::io::Result<Option<i32>> {
    use std::os::unix::process::CommandExt;

    Err(Command::new(target).args(arguments).exec())
}

/// Run `target` as a child that alone handles Ctrl+C and Ctrl+Break, and return its exit code.
#[cfg(not(unix))]
pub(crate) fn hand_off(target: &Path, arguments: &[OsString]) -> std::io::Result<Option<i32>> {
    let _ = ctrlc::set_handler(|| {});
    Command::new(target)
        .args(arguments)
        .status()
        .map(|status| status.code())
}

/// End the process with `code`; a code beyond one byte ends it directly.
pub(crate) fn process_exit(code: i32) -> ExitCode {
    match u8::try_from(code) {
        Ok(code) => ExitCode::from(code),
        Err(_) => std::process::exit(code),
    }
}
