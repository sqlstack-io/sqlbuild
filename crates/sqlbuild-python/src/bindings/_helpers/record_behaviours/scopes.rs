//! Computed attributes of the natively hosted scope records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The completeness sections of a `ScopeCompleteness`.
const COMPLETENESS_SECTIONS: [&str; 6] = [
    "discovery",
    "static_visibility",
    "runtime_usage",
    "relationships",
    "placement",
    "promotion_impact",
];

/// `ScopeCompleteness.complete`: whether every index section is complete.
pub(crate) fn completeness_complete(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let mut complete: bool = true;
    for section in COMPLETENESS_SECTIONS {
        complete = complete && native_field(instance, section)?.is_truthy()?;
    }
    complete.into_py_any(instance.py())
}

/// `ScopeTargetQuery.unknown`: whether the target is absent rather than merely inaccessible.
pub(crate) fn target_query_unknown(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let unknown: bool = !native_field(instance, "matches")?.is_truthy()?
        && !native_field(instance, "declaration_matches")?.is_truthy()?;
    unknown.into_py_any(instance.py())
}
