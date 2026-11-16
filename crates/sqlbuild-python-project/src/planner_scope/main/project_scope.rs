//! Preserve public record identity and dataclass behavior on explicit projection.

use crate::planner_scope::models::RetainedScope;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::models::NativeRecord;
use sqlbuild_python_base::records::types::RecordId;

/// Preserve public record identity and dataclass behavior on explicit projection.
pub fn project_scope<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let Ok(scope) = value.cast::<RetainedScope>() else {
        return Ok(value.clone());
    };
    scope
        .get()
        .projection
        .get_or_try_init(value.py(), || {
            let names = [
                "upstream_deps",
                "downstream_deps",
                "all_keys",
                "models_by_name",
                "selected_keys",
                "execution_order",
                "user_selected_keys",
                "python_read_source_names",
                "sql_test_selection",
                "load_sources",
                "end_audit_source_keys",
            ];
            let fields = names
                .iter()
                .map(|name| Ok((*name, scope.get().data.field(value.py(), name)?)))
                .collect::<PyResult<Vec<_>>>()?;
            Ok::<_, pyo3::PyErr>(
                new_record_with(value.py(), RecordId::PlannerScope, &fields)?
                    .cast_into::<NativeRecord>()
                    .map(Bound::unbind)?,
            )
        })
        .map(|value| value.bind(scope.py()).clone().into_any())
}
