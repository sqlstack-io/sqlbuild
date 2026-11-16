//! `MeasurementThresholds` of `parsed`, or `None`.

use crate::model_config::_helpers::headers::header_metadata;
use crate::model_config::models::ContractClasses;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_model_config::header_metadata::models::ParsedThresholds;

/// `MeasurementThresholds` of `parsed`, or `None`.
#[inline]
pub fn thresholds<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: Option<&ParsedThresholds>,
) -> PyResult<Bound<'py, PyAny>> {
    header_metadata::thresholds(py, classes, parsed)
}
