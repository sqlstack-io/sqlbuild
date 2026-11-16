//! Direct source freshness state and observation exceptions.

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

create_exception!(
    sqlbuild._native,
    SourceFreshnessInputError,
    PyValueError,
    "Raised when persisted source freshness state cannot be read safely."
);
create_exception!(
    sqlbuild._native,
    SourceFreshnessObservationError,
    PyValueError,
    "Raised when source freshness cannot be observed safely."
);

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module.add(
        "SourceFreshnessInputError",
        py.get_type::<SourceFreshnessInputError>(),
    )?;
    module.add(
        "SourceFreshnessObservationError",
        py.get_type::<SourceFreshnessObservationError>(),
    )
}
