//! Microseconds since the epoch: UTC for aware values, the wall clock for naive ones.

use crate::cursor::constants::DATETIME_MODULE;
use crate::state_rows::main::event_rows::offset_aware::offset_aware;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::imported::imported;

/// Microseconds since the epoch: UTC for aware values, the wall clock for naive ones.
pub fn instant_micros(value: &Bound<'_, PyAny>) -> PyResult<i128> {
    let py: Python<'_> = value.py();
    let datetime: Bound<'_, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let epoch: Bound<'_, PyAny> = if offset_aware(value)? {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("tzinfo", imported(py, DATETIME_MODULE, "UTC")?)?;
        datetime.call((1970, 1, 1), Some(&options))?
    } else {
        datetime.call1((1970, 1, 1))?
    };
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("microseconds", 1)?;
    let micro: Bound<'_, PyAny> =
        imported(py, DATETIME_MODULE, "timedelta")?.call((), Some(&options))?;
    value.sub(epoch)?.floor_div(micro)?.extract()
}
