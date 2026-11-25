//! Select audits by name, attached audits by their logical target and singular audits by dependency overlap.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::graph::main::nodes::native_key::native_key;
use sqlbuild_python_project::planner_strategy::constants::COMPILE_TYPES_MODULE;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_kind::attached_target_kind;
use sqlbuild_python_project::resource_views::main::audit_reads::attached_target_name::attached_target_name;
use sqlbuild_python_project::resource_views::main::audit_reads::name::name;
use sqlbuild_python_project::resource_views::main::audit_reads::scope_deps::scope_deps;
use std::collections::HashSet;

/// Select audits named in `selected_names`, attached audits by their logical target and singular audits by dependency overlap.
pub fn audit_is_selected(
    audit: &Bound<'_, PyAny>,
    (selected_keys, selected_names): (&HashSet<ObjectKey>, &HashSet<String>),
) -> PyResult<bool> {
    let py = audit.py();
    if !selected_names.is_empty() && selected_names.contains(&name(audit)?.extract::<String>()?) {
        return Ok(true);
    }
    let kind = attached_target_kind(audit)?;
    let name = attached_target_name(audit)?;
    if !kind.is_none() && !name.is_none() {
        let target_value = native_field(
            &imported(py, COMPILE_TYPES_MODULE, "AttachedAuditTargetKind")?.call1((kind,))?,
            "value",
        )?;
        let key = (target_value.extract()?, name.extract()?);
        return Ok(selected_keys.contains(&key));
    }
    for dependency in scope_deps(audit)?.try_iter()? {
        if selected_keys.contains(&native_key(&dependency?)?) {
            return Ok(true);
        }
    }
    Ok(false)
}
