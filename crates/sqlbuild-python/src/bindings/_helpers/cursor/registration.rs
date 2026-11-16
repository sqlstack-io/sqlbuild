//! Register the native cursor algebra functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::cursor::main::durations::register_durations::register_durations;
use sqlbuild_python_front::cursor::main::intervals::register_intervals::register_intervals;
use sqlbuild_python_front::cursor::main::scalars::register_scalars::register_scalars;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_scalars(module)?;
    register_intervals(module)?;
    register_durations(module)?;
    Ok(())
}
