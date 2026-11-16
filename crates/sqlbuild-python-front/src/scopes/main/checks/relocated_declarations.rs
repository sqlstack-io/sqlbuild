//! Relocated records after every move in `moves`, and per record whether it lacks a needed group.

use crate::scopes::_helpers::checks::scope_validation::RUNTIME_USAGE;
use crate::scopes::_helpers::checks::scope_validation::replace;
use crate::scopes::_helpers::index::scope_objects::{
    read_completeness, read_placement_index, root_object, tuple_attribute,
};
use crate::scopes::models::ScopeClasses;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyTuple, PyTupleMethods};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::placement::main::relocate_for_resource_moves::relocate_for_resource_moves;
use sqlbuild_scopes::placement::models::{PlacementIndex, Relocation};
use sqlbuild_scopes::scope_index::models::ResourceIdentity;

/// Relocated records after every move in `moves`, and per record whether it lacks a needed group.
pub fn relocated_declarations<'py>(
    index: &Bound<'py, PyAny>,
    moves: &[(ResourceIdentity, String)],
) -> PyResult<Option<(Bound<'py, PyTuple>, Vec<bool>)>> {
    let py: Python<'py> = index.py();
    if !read_completeness(index)?[RUNTIME_USAGE] {
        return Ok(None);
    }
    let placement: PlacementIndex = read_placement_index(index)?;
    let relocations: Option<Vec<Relocation>> = py
        .compiler_detach(|| Ok(relocate_for_resource_moves(&placement, moves)))
        .map_err(compiler_error)?;
    let Some(relocations) = relocations else {
        return Ok(None);
    };
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let records: Bound<'_, PyTuple> = tuple_attribute(index, "declarations")?;
    let mut moved: Vec<Bound<'py, PyAny>> = Vec::with_capacity(relocations.len());
    let mut grouping: Vec<bool> = Vec::with_capacity(relocations.len());
    for item in relocations {
        let owning_path: Bound<'_, PyAny> = item.owning_path.into_bound_py_any(py)?;
        let record: Bound<'py, PyAny> = replace(
            &records.get_item(item.declaration)?,
            &[
                ("path", item.path.into_bound_py_any(py)?),
                (
                    "scope",
                    classes.member(RecordId::ScopeKind, item.scope.as_str())?,
                ),
                (
                    "ownership_root",
                    root_object(&classes, &item.ownership_root)?,
                ),
                ("owning_path", owning_path),
            ],
        )?;
        moved.push(record);
        grouping.push(item.requires_group);
    }
    Ok(Some((PyTuple::new(py, moved)?, grouping)))
}
