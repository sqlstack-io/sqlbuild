//! Project column renames onto the existing snapshot and redirect cursor reads.

use crate::planner_migrations::_helpers::column::column_objects::config;
use crate::planner_migrations::_helpers::column::column_objects::definition_hash;
use crate::planner_migrations::_helpers::column::column_objects::lower;
use crate::planner_migrations::_helpers::column::column_objects::native;
use crate::planner_migrations::_helpers::column::column_objects::replace;
use crate::planner_migrations::_helpers::projections::query_shapes::renames_explain_change;
use crate::planner_migrations::models::ColumnCandidate;
use crate::planner_migrations::models::ColumnMigrationInputs;
use crate::planner_migrations::models::MigrationPhase;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::compiled_models::query_sql::query_sql;
use sqlbuild_python_project::native_host::models::ModelLookup;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;

pub(crate) fn overlay<'py>(
    snapshot: &Bound<'py, PyAny>,
    candidates: &[ColumnCandidate<'py>],
    entries: &[Bound<'py, PyAny>],
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if entries.is_empty() {
        return Ok(snapshot.clone());
    }
    let py = snapshot.py();
    let columns =
        imported(py, "builtins", "dict")?.call1((native_field(snapshot, "existing_columns")?,))?;
    let stored = native_field(snapshot, "fingerprints")?;
    let fingerprints =
        imported(py, "builtins", "dict")?.call1((native_field(&stored, "models")?,))?;
    for candidate in candidates {
        let name = name_object(&candidate.model)?;
        let renames = PyDict::new(py);
        let pending = PyDict::new(py);
        let mut blocked = false;
        for entry in entries {
            if !native_field(entry, "model_name")?.eq(&name)? {
                continue;
            }
            blocked |= native_field(entry, "blocks_build")?.is_truthy()?
                || native_field(entry, "origin_missing")?.is_truthy()?;
            renames.set_item(
                native_field(entry, "origin_column")?,
                native_field(entry, "destination_column")?,
            )?;
            if native_field(&native_field(entry, "decision")?, "renames")?.is_truthy()? {
                pending.set_item(
                    lower(&native_field(entry, "origin_column")?)?,
                    native_field(entry, "destination_column")?,
                )?;
            }
        }
        if renames.is_empty() || blocked {
            continue;
        }
        let mut renamed: Vec<Bound<'py, PyAny>> = Vec::new();
        for column in native_field(snapshot, "existing_columns")?
            .call_method1("get", (&name, PyTuple::empty(py)))?
            .try_iter()?
        {
            let column = column?;
            let old = native_field(&column, "name")?;
            let new = pending.get_item(lower(&old)?)?.unwrap_or(old);
            renamed.push(replace(&column, &[("name", new)])?);
        }
        columns.set_item(&name, PyTuple::new(py, renamed)?)?;
        if !candidate.fingerprint.is_none()
            && !candidate.previous.is_none()
            && !candidate.current.is_none()
            && object(
                py,
                renames_explain_change(
                    py,
                    Some(keywords(
                        py,
                        &[
                            ("previous", candidate.previous.clone()),
                            ("current", candidate.current.clone()),
                            ("renames", renames.into_any()),
                            ("input_columns", inputs.clone()),
                        ],
                    )?),
                )?,
            )?
            .is_truthy()?
        {
            fingerprints.set_item(
                &name,
                replace(
                    &candidate.fingerprint,
                    &[
                        ("definition", query_sql(&candidate.model)?),
                        (
                            "definition_hash",
                            definition_hash(&candidate.model, snapshot)?,
                        ),
                    ],
                )?,
            )?;
        }
    }
    replace(
        snapshot,
        &[
            ("existing_columns", columns),
            (
                "fingerprints",
                native(
                    py,
                    RecordId::WarehouseFingerprints,
                    &[
                        ("models", fingerprints),
                        ("functions", native_field(&stored, "functions")?),
                        ("seeds", native_field(&stored, "seeds")?),
                        ("python_nodes", native_field(&stored, "python_nodes")?),
                    ],
                )?,
            ),
        ],
    )
}

pub(crate) fn cursors<'py>(
    (phase, columns): (&MigrationPhase<'py>, &ColumnMigrationInputs<'py>),
    snapshot: Bound<'py, PyAny>,
    entries: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py = snapshot.py();
    let models = ModelLookup::Native(scope_data(&phase.scope)?);
    let origins = PyDict::new(py);
    for entry in entries {
        let name = native_field(entry, "model_name")?;
        let model = models.get(py, &name.extract::<String>()?)?;
        let cursor = if model.is_truthy()? {
            config(&model, "cursor")?
        } else {
            py.None().into_bound(py)
        };
        if native_field(&native_field(entry, "decision")?, "renames")?.is_truthy()?
            && !cursor.is_none()
            && lower(&cursor)? == lower(&native_field(entry, "destination_column")?)?
        {
            origins.set_item(name, native_field(entry, "origin_column")?)?;
        }
    }
    if origins.is_empty() {
        return Ok(snapshot);
    }
    let targets = PyDict::new(py);
    for (name, _) in origins.iter() {
        let physical = mapping_get(&columns.physical_relations, &name)?;
        let target = if physical.is_truthy()? {
            physical
        } else {
            let qualified = native_field(
                &destination(&models.get(py, &name.extract::<String>()?)?)?,
                "qualified_name",
            )?;
            if qualified.is_truthy()? {
                qualified
            } else {
                name.clone()
            }
        };
        targets.set_item(name, target)?;
    }
    let redirected = crate::planner_migrations::_helpers::planning::planning_boundary::redirected_cursor_snapshots(
        phase,
        (
            native_field(&snapshot, "existing_relations")?,
            targets.into_any(),
        ),
        Some(origins.into_any()),
    )?;
    let cursors =
        imported(py, "builtins", "dict")?.call1((native_field(&snapshot, "cursor_snapshots")?,))?;
    cursors.call_method1("update", (redirected,))?;
    replace(&snapshot, &[("cursor_snapshots", cursors)])
}
