//! Write a debug report to a file of its own, for a caller that reads stdout for something else.

use std::io::Write;
use std::path::Path;

/// Write `text` to `path`, a new file only its owner can read, since the report names the connection; an error when `path` already exists or cannot be written.
pub fn write_report_file(path: &Path, text: &str) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(text.as_bytes())
}
