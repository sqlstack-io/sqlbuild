//! The process's terminal: its width, colour support and stream writes.

use std::io::{self, BufRead, IsTerminal, Read, Write};

use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::argv_host::python_int;
use crate::native_commands::constants::{
    BROKEN_PIPE_EXCEPTION, COLUMNS_VARIABLE, EXIT_FLUSH_FAILURE, EXIT_FLUSH_FAILURE_STATUS,
    FALLBACK_COLUMNS, NO_COLOR_VARIABLE, PASSWORD_INPUT_BYTES, PYTHON_STDOUT_BUFFER_SIZE,
    UNCAUGHT_EXCEPTION_STATUS,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::models::LostStdout;

/// `int(os.environ["COLUMNS"])`, `0` when unset or not an integer; beyond 64 bits is Python's.
fn columns_variable() -> Result<i64, PythonNeeded> {
    let Ok(text) = std::env::var(COLUMNS_VARIABLE) else {
        return Ok(0);
    };
    let Some(number) = python_int(&text) else {
        return Ok(0);
    };
    number.parse::<i64>().map_err(|_| PythonNeeded::Arguments)
}

/// The width of the terminal on stdout, or the fallback when stdout is not one.
#[cfg(unix)]
fn stdout_columns() -> Result<i64, PythonNeeded> {
    match rustix::termios::tcgetwinsize(io::stdout()) {
        Ok(size) if size.ws_col > 0 => Ok(i64::from(size.ws_col)),
        _ => Ok(FALLBACK_COLUMNS),
    }
}

/// A console's width needs the console API; only a redirected stdout has a known width.
#[cfg(not(unix))]
fn stdout_columns() -> Result<i64, PythonNeeded> {
    if io::stdout().is_terminal() {
        return Err(PythonNeeded::Arguments);
    }
    Ok(FALLBACK_COLUMNS)
}

/// `shutil.get_terminal_size().columns`, unless only Python can tell.
pub(crate) fn terminal_columns() -> Result<i64, PythonNeeded> {
    let columns: i64 = columns_variable()?;
    if columns > 0 {
        return Ok(columns);
    }
    stdout_columns()
}

/// `stdout_supports_color`: stdout is a terminal and `NO_COLOR` is unset.
pub(crate) fn stdout_supports_color() -> bool {
    std::env::var_os(NO_COLOR_VARIABLE).is_none() && io::stdout().is_terminal()
}

/// Passwords never enter parsed arguments, environment variables or command output.
pub(crate) fn read_password() -> Result<String, ControlError> {
    let password = if io::stdin().is_terminal() {
        let password = rpassword::prompt_password("Password: ")?;
        if password != rpassword::prompt_password("Confirm password: ")? {
            return Err(ControlError::Invalid("passwords do not match".into()));
        }
        password
    } else {
        let mut password = String::new();
        io::stdin()
            .lock()
            .take(PASSWORD_INPUT_BYTES)
            .read_line(&mut password)?;
        if password.len() as u64 == PASSWORD_INPUT_BYTES {
            return Err(ControlError::Invalid("password input is too large".into()));
        }
        if password.ends_with('\n') {
            password.pop();
            if password.ends_with('\r') {
                password.pop();
            }
        }
        password
    };
    if password.is_empty() {
        return Err(ControlError::Invalid("password input was empty".into()));
    }
    Ok(password)
}

/// `\n` as a Python text stream writes it on this platform.
fn console_bytes(text: &str) -> Vec<u8> {
    if cfg!(windows) {
        text.replace('\n', "\r\n").into_bytes()
    } else {
        text.as_bytes().to_vec()
    }
}

fn write_stream(stream: Stream, bytes: &[u8]) -> io::Result<()> {
    match stream {
        Stream::Stdout => {
            let mut output = io::stdout().lock();
            output.write_all(bytes).and_then(|()| output.flush())
        }
        Stream::Stderr => {
            let mut output = io::stderr().lock();
            output.write_all(bytes).and_then(|()| output.flush())
        }
    }
}

/// The system's description of an I/O failure, without Rust's `(os error N)` suffix.
pub(crate) fn error_description(error: &io::Error) -> String {
    let described: String = error.to_string();
    match error.raw_os_error() {
        Some(errno) => described
            .strip_suffix(&format!(" (os error {errno})"))
            .unwrap_or(&described)
            .to_owned(),
        None => described,
    }
}

/// The last line of the traceback Python prints for a failed write.
fn exception_line(error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::BrokenPipe {
        return BROKEN_PIPE_EXCEPTION.to_owned();
    }
    match error.raw_os_error() {
        Some(errno) => format!("OSError: [Errno {errno}] {}", error_description(error)),
        None => format!("OSError: {error}"),
    }
}

/// CPython's 8 KiB stdout buffer keeps up to half until exit, drops more, and fails a full write.
pub(crate) fn lost_stdout(total: usize, error: &io::Error) -> LostStdout {
    if total >= PYTHON_STDOUT_BUFFER_SIZE {
        LostStdout::Raised(exception_line(error))
    } else if total <= PYTHON_STDOUT_BUFFER_SIZE / 2 {
        LostStdout::AtExit(exception_line(error))
    } else {
        LostStdout::Dropped
    }
}

/// Write each text in order and flush it; errors on stderr are ignored, as Python's are.
pub(crate) fn write_output(writes: &[(Stream, &str)]) -> Option<LostStdout> {
    let encoded: Vec<(Stream, Vec<u8>)> = writes
        .iter()
        .filter(|(_, text)| !text.is_empty())
        .map(|(stream, text)| (*stream, console_bytes(text)))
        .collect();
    let total: usize = encoded
        .iter()
        .filter(|(stream, _)| *stream == Stream::Stdout)
        .map(|(_, bytes)| bytes.len())
        .sum();
    let mut lost: Option<LostStdout> = None;
    for (stream, bytes) in &encoded {
        let written: io::Result<()> = match (stream, &lost) {
            (Stream::Stdout, Some(_)) => Ok(()),
            _ => write_stream(*stream, bytes),
        };
        if let (Stream::Stdout, Err(error)) = (stream, written) {
            lost = Some(lost_stdout(total, &error));
        }
    }
    lost
}

/// Print a rejected parse's writes; the exit status, as `finish_lost_stdout` settles it.
pub(crate) fn finish_rejected_parse(writes: &[(Stream, String)], code: i64) -> i32 {
    let writes: Vec<(Stream, &str)> = writes
        .iter()
        .map(|(stream, text)| (*stream, text.as_str()))
        .collect();
    let lost: Option<LostStdout> = write_output(&writes);
    finish_lost_stdout(lost.as_ref(), i32::try_from(code).unwrap_or(1))
}

/// Report lost stdout as Python does and return the exit status that results.
pub(crate) fn finish_lost_stdout(lost: Option<&LostStdout>, exit_code: i32) -> i32 {
    match lost {
        None | Some(LostStdout::Dropped) => exit_code,
        Some(LostStdout::AtExit(exception)) => {
            let text: String = format!("{EXIT_FLUSH_FAILURE}\n{exception}\n");
            drop(write_stream(Stream::Stderr, &console_bytes(&text)));
            EXIT_FLUSH_FAILURE_STATUS
        }
        Some(LostStdout::Raised(exception)) => {
            drop(write_stream(
                Stream::Stderr,
                &console_bytes(&format!("{exception}\n")),
            ));
            UNCAUGHT_EXCEPTION_STATUS
        }
    }
}
