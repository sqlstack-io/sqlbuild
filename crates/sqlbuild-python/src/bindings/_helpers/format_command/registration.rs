//! Register the native `sqb format` command and its atomic file writer.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use crate::bindings::_helpers::format_command::cache::FORMAT_RESULT_STORE_FILE_NAME;
use crate::bindings::_helpers::format_command::command;
use crate::bindings::_helpers::format_command::files;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    command::register(module)?;
    files::register(module)?;
    module.add(
        "FORMAT_RESULT_STORE_FILE_NAME",
        FORMAT_RESULT_STORE_FILE_NAME,
    )
}
