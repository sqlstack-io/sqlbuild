//! Assemble selected audits in compiled-project order without querying lineage for an empty selection.

use crate::planner_strategy::_helpers::audit_entries::plan_audit;
use crate::planner_strategy::main::audit_entries::audit_is_selected::audit_is_selected;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyList, PyListMethods};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::project::audits::audits;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_strategy::models::AuditPlanning;

/// Assemble selected audits in compiled-project order without querying lineage for an empty selection.
pub fn build_selected_audit_entries<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    let request = Request::new("build_selected_audit_entries", request)?;
    let py = request.py();
    let project = request.get("project")?;
    let scope = scope_data(&request.get("scope")?)?;
    let mut selected: Vec<Bound<'py, PyAny>> = Vec::new();
    for audit in audits(&project)?.try_iter()? {
        let audit = audit?;
        if audit_is_selected(&audit, &scope.selected)? {
            selected.push(audit);
        }
    }
    let entries = PyList::empty(py);
    if selected.is_empty() {
        return Ok(entries);
    }
    let relations = request.get("relations")?;
    let planning = AuditPlanning {
        model_locations: native_field(&relations, "model_locations")?,
        seed_locations: native_field(&relations, "seed_locations")?,
        source_map: native_field(&relations, "source_read_map")?,
        dialect: Dialect::passed(request.get("dialect")?)?,
        model_materializations: request.get("model_materializations")?,
        lineage: native_field(&project, "lineage_graph")?.extract()?,
    };
    for audit in selected {
        entries.append(plan_audit(&audit, &planning)?)?;
    }
    Ok(entries)
}
