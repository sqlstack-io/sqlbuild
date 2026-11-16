//! Direct source freshness state and observation exceptions.

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;

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
