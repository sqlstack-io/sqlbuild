//! The terminal's settings, which a forced exit puts back after a hidden-input prompt.

use pyo3::prelude::{Py, PyAny, PyAnyMethods, PyResult, Python};

use crate::boundary::main::imported::imported;

/// The first standard descriptor that is a terminal, with its settings; `None` without one.
pub(crate) fn terminal_settings(py: Python<'_>) -> Option<(i32, Py<PyAny>)> {
    for descriptor in [0, 2, 1] {
        let settings: PyResult<Py<PyAny>> = imported(py, "termios", "tcgetattr")
            .and_then(|read| read.call1((descriptor,)))
            .map(pyo3::Bound::unbind);
        if let Ok(settings) = settings {
            return Some((descriptor, settings));
        }
    }
    None
}

/// Put `settings` back at once.
pub(crate) fn restore_terminal(
    py: Python<'_>,
    (descriptor, settings): &(i32, Py<PyAny>),
) -> PyResult<()> {
    let termios = py.import("termios")?;
    termios.call_method1(
        "tcsetattr",
        (*descriptor, termios.getattr("TCSANOW")?, settings.bind(py)),
    )?;
    Ok(())
}
