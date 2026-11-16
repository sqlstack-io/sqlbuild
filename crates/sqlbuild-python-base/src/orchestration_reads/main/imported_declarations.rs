//! Sort imported declarations into calendars and sensors, refusing a calendar name defined twice.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use crate::orchestration_reads::models::ImportedDeclarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Sort imported declarations into calendars and sensors, refusing a calendar name defined twice.
#[inline]
pub fn imported_declarations<'py>(
    py: Python<'py>,
    declarations: Vec<Bound<'py, PyAny>>,
) -> PyResult<ImportedDeclarations<'py>> {
    orchestration_declarations::imported_declarations(py, declarations)
}
