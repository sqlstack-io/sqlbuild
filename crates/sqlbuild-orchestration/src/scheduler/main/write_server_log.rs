//! The server log is stderr: the operator reads there what a health problem leaves out.

/// Writes `line` to the server log.
pub(crate) fn write_server_log(line: &str) {
    eprintln!("{line}");
}
