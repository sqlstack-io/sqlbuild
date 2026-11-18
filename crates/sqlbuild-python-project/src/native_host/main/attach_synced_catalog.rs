//! Give the adapter the target's synced catalog when a plan reads from synced state.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyList, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_orchestration::warehouse_metadata::models::SyncedCatalogRelation;

use crate::native_host::_helpers::synced_catalog::{
    planning_live, plans_from_synced_state, synced_relations, synced_row,
};
use crate::native_host::constants::SYNCED_CATALOG_ATTRIBUTE;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Give `adapter` the target's synced catalog when the plan reads from synced state.
pub fn attach_synced_catalog(
    adapter: &Bound<'_, PyAny>,
    (discovered, compiled, options): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    let py: Python<'_> = adapter.py();
    if adapter.hasattr(SYNCED_CATALOG_ATTRIBUTE)? {
        adapter.setattr(SYNCED_CATALOG_ATTRIBUTE, py.None())?;
    }
    let target: Bound<'_, PyAny> =
        native_field(&native_field(compiled, "project")?, "effective_target_name")?;
    if !plans_from_synced_state(discovered, options, &target)? {
        return Ok(());
    }
    let project_dir: Option<PathBuf> = native_field(discovered, "project_dir")?.extract()?;
    let target: Option<String> = target.extract()?;
    let (Some(project_dir), Some(target)) = (project_dir, target) else {
        return planning_live(py, "the project directory or target is unknown");
    };
    let loaded: Result<Vec<SyncedCatalogRelation>, String> =
        py.detach(|| synced_relations(&project_dir, &target));
    match loaded {
        Ok(relations) if !relations.is_empty() => {
            let rows: Vec<Bound<'_, PyTuple>> = relations
                .iter()
                .map(|relation| synced_row(py, relation))
                .collect::<PyResult<_>>()?;
            let catalog: Bound<'_, PyAny> = call_with(
                &host_entry(py, AdapterHost::SyncedCatalog)?.getattr("from_rows")?,
                &[("rows", PyList::new(py, rows)?.into_any())],
            )?;
            adapter.setattr(SYNCED_CATALOG_ATTRIBUTE, catalog)
        }
        Ok(_) => planning_live(
            py,
            "the control store holds no synced catalog for this target",
        ),
        Err(reason) => planning_live(py, &reason),
    }
}
