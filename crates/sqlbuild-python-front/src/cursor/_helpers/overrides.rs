//! Executor cursor bounds from `--start-cursor`/`--end-cursor` overrides.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) const DECIMAL_MODULE: &str = "decimal";
pub(crate) const INTEGER_AS_TIMESTAMP: &str = "an integer cursor cannot be used as a timestamp";
pub(crate) const TIMESTAMP_AS_INTEGER: &str = "a timestamp cursor cannot be used as an integer";

pub(crate) fn instance_of(value: &Bound<'_, PyAny>, class: RecordId) -> PyResult<bool> {
    value.is_instance(&record_class(value.py(), class)?)
}
