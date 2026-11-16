//! Describe full rebuild cause.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_analysis::planner_warehouse::main::full_rebuild_cause::full_rebuild_cause;
use sqlbuild_analysis::planner_warehouse::models::FullRebuildFacts;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::strings::strings;

#[pyfunction]
#[pyo3(signature = (*, change_result, full_refresh))]
pub fn describe_full_rebuild_cause(
    change_result: &Bound<'_, PyAny>,
    full_refresh: bool,
) -> PyResult<String> {
    Ok(full_rebuild_cause(&FullRebuildFacts {
        full_refresh,
        change_kind: native_field(change_result, "change_kind")?
            .str()?
            .extract::<String>()?,
        query_changed: native_field(change_result, "query_changed")?.is_truthy()?,
        changed_functions: strings(&native_field(change_result, "changed_functions")?)?,
        has_schema_findings: native_field(change_result, "schema_findings")?.is_truthy()?,
    }))
}
