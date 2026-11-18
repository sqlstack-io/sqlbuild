//! A plan from synced state: the target's mirrored catalog, which inspection checks before use.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyList, PyTuple};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::warehouse_metadata::main::location_reads::read_development_location::read_development_location;
use sqlbuild_orchestration::warehouse_metadata::main::location_reads::read_synced_catalog::read_synced_catalog;
use sqlbuild_orchestration::warehouse_metadata::models::SyncedCatalogRelation;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::native_host::constants::{
    CONTROL_STORE_FILE, CONTROL_STORE_VARIABLE, SYNCED_CATALOG_ATTRIBUTE, SYNCED_STATE,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;

/// Give `adapter` the target's synced catalog when the plan reads from synced state.
pub(crate) fn attach_synced_catalog(
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
                &host::entry(py, AdapterHost::SyncedCatalog)?.getattr("from_rows")?,
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

/// Write what the synced state checks found, once the plan is made.
pub(crate) fn report_synced_catalog(adapter: &Bound<'_, PyAny>) -> PyResult<()> {
    if !adapter.hasattr(SYNCED_CATALOG_ATTRIBUTE)? {
        return Ok(());
    }
    let catalog: Bound<'_, PyAny> = adapter.getattr(SYNCED_CATALOG_ATTRIBUTE)?;
    if catalog.is_none() {
        return Ok(());
    }
    let report: Option<String> = catalog.call_method0("report")?.extract()?;
    match report {
        Some(line) => write_stderr(adapter.py(), &format!("{line}\n")),
        None => Ok(()),
    }
}

/// Whether `--plan-from`, or else the target's `plan_from`, asks for the synced state.
fn plans_from_synced_state(
    discovered: &Bound<'_, PyAny>,
    options: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let requested: Option<String> = native_field(options, "plan_from")?.extract()?;
    if let Some(requested) = requested {
        return Ok(requested == SYNCED_STATE);
    }
    if target.is_none() {
        return Ok(false);
    }
    let config: Bound<'_, PyAny> = resolve_target_config(
        discovered.py(),
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        target,
    )?;
    let configured: Option<String> = native_field(&config, "plan_from")?.extract()?;
    Ok(configured.as_deref() == Some(SYNCED_STATE))
}

/// Write `text` to Python's stderr, where the plan's other progress goes, and flush it.
fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    stderr.call_method1("write", (text,))?;
    stderr.call_method0("flush")?;
    Ok(())
}

fn planning_live(py: Python<'_>, reason: &str) -> PyResult<()> {
    write_stderr(py, &format!("Planning live: {reason}\n"))
}

/// The mirrored catalog the project's control store holds for `target`'s development location.
fn synced_relations(
    project_dir: &Path,
    target: &str,
) -> Result<Vec<SyncedCatalogRelation>, String> {
    let url: String = match std::env::var(CONTROL_STORE_VARIABLE) {
        Ok(url) => url,
        Err(_) => {
            let path: PathBuf = project_dir.join(CONTROL_STORE_FILE);
            if !path.is_file() {
                return Err("the project has no control store".to_owned());
            }
            path.to_string_lossy().into_owned()
        }
    };
    let project: PathBuf =
        std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let read = runtime.block_on(async {
        let store = open_store(&url).await.map_err(|error| error.to_string())?;
        let location = read_development_location(&store, target, |path| {
            std::fs::canonicalize(path).is_ok_and(|path| path == project)
        })
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no location syncs target '{target}' for this project"))?;
        read_synced_catalog(&store, (location, target))
            .await
            .map_err(|error| error.to_string())
    });
    runtime.shutdown_timeout(Duration::from_secs(1));
    read
}

/// `(database, schema, name, columns, observed_at)` of one mirrored relation.
fn synced_row<'py>(
    py: Python<'py>,
    relation: &SyncedCatalogRelation,
) -> PyResult<Bound<'py, PyTuple>> {
    let record = &relation.record;
    let columns: Vec<(String, String)> = record
        .columns
        .iter()
        .map(|column| (column.name.clone(), column.data_type.clone()))
        .collect();
    PyTuple::new(
        py,
        [
            record.state_database.clone().into_pyobject(py)?.into_any(),
            record.state_schema.clone().into_pyobject(py)?.into_any(),
            record.name.clone().into_pyobject(py)?.into_any(),
            columns.into_pyobject(py)?.into_any(),
            relation
                .observed_at
                .to_rfc3339()
                .into_pyobject(py)?
                .into_any(),
        ],
    )
}
