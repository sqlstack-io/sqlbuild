//! Reads and writes a plan from synced state shares: the control store's mirrored catalog and stderr notes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::warehouse_metadata::main::location_reads::read_development_location::read_development_location;
use sqlbuild_orchestration::warehouse_metadata::main::location_reads::read_synced_catalog::read_synced_catalog;
use sqlbuild_orchestration::warehouse_metadata::models::SyncedCatalogRelation;

use crate::native_host::constants::{CONTROL_STORE_FILE, CONTROL_STORE_VARIABLE, SYNCED_STATE};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;

/// Whether `--plan-from`, or else the target's `plan_from`, asks for the synced state.
pub(crate) fn plans_from_synced_state(
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
pub(crate) fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    stderr.call_method1("write", (text,))?;
    stderr.call_method0("flush")?;
    Ok(())
}

pub(crate) fn planning_live(py: Python<'_>, reason: &str) -> PyResult<()> {
    write_stderr(py, &format!("Planning live: {reason}\n"))
}

/// The mirrored catalog the project's control store holds for `target`'s development location.
pub(crate) fn synced_relations(
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
pub(crate) fn synced_row<'py>(
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
