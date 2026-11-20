//! Materialize column decisions from native proofs and live evidence.

use crate::planner_migrations::_helpers::planning::location_relation::location_relation;
use crate::planner_phases::main::inputs::runtime_reads::runtime_reads;
use crate::planner_warehouse::models::{AdapterRead, Read};

use crate::planner_migrations::_helpers::column::column_candidates::declared_names;
use crate::planner_migrations::_helpers::column::column_objects::lower;
use crate::planner_migrations::_helpers::column::column_objects::model;
use crate::planner_migrations::_helpers::column::column_objects::replace;
use crate::planner_migrations::_helpers::column::columns::identical_renames;
use crate::planner_migrations::_helpers::column::columns::rename_hints;
use crate::planner_migrations::constants::AUTOMATIC;
use crate::planner_migrations::constants::DONE;
use crate::planner_migrations::models::CheckedColumns;
use crate::planner_migrations::models::ColumnCandidate;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyListMethods, PyTuple};
use sqlbuild_analysis::planner_migrations::main::column_decision::column_decision;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::state_rows::main::migrations::newest_column::newest_column;
use sqlbuild_python_project::compiled_views::main::compiled_models::inferred_rows::inferred_rows;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::planner_phases::main::object::object;

pub(crate) fn decide<'py>(
    runtime: &Bound<'py, PyAny>,
    candidate: &ColumnCandidate<'py>,
    events: &Bound<'py, PyAny>,
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = runtime.py();
    let mut pairs: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>, &str)> = Vec::new();
    let declared = PyDict::new(py);
    for (destination, origin) in candidate.declared.iter() {
        declared.set_item(&origin, &destination)?;
        pairs.push((origin, destination, "manual"));
    }
    if !candidate.previous.is_none() && !candidate.current.is_none() {
        let detected = object(
            py,
            identical_renames(
                py,
                Some(keywords(
                    py,
                    &[
                        ("previous", candidate.previous.clone()),
                        ("current", candidate.current.clone()),
                        ("excluded", declared_names(candidate)?.into_any()),
                        ("declared", declared.into_any()),
                        ("input_columns", inputs.clone()),
                    ],
                )?),
            )?,
        )?;
        for pair in detected.try_iter()? {
            let pair = pair?;
            pairs.push((pair.get_item(0)?, pair.get_item(1)?, "automatic"));
        }
    }
    let mut outputs: Vec<String> = Vec::new();
    if !candidate.current.is_none() {
        for projection in native_field(&candidate.current, "projections")?.try_iter()? {
            outputs.push(native_field(&projection?, "key")?.extract::<String>()?);
        }
    } else {
        for (name, _, _) in inferred_rows(&candidate.model)?.unwrap_or_default() {
            outputs.push(name.to_lowercase());
        }
    }
    let relation = location_relation(
        &sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(
            &candidate.model,
        )?,
    )?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for (origin_name, destination, discovery) in pairs {
        let origin_key = lower(&origin_name)?;
        let origin = candidate.warehouse.get_item(&origin_key)?;
        let has_destination = candidate.warehouse.contains(lower(&destination)?)?;
        let newest = newest_column(
            events,
            &relation,
            (
                &origin_name.extract::<String>()?,
                &destination.extract::<String>()?,
            ),
        )?;
        let recorded = !newest.is_none()
            && call_with(
                &native_field(&newest, "renamed")?,
                &[
                    ("origin_column", origin_name.clone()),
                    ("destination_column", destination.clone()),
                ],
            )?
            .is_truthy()?;
        let decision_name = column_decision(
            recorded,
            (origin.is_some(), has_destination),
            outputs.contains(&origin_key),
        );
        let decision = enum_from_value(py, RecordId::ColumnMigrationDecision, decision_name)?;
        if discovery == AUTOMATIC && native_field(&decision, "blocks_build")?.is_truthy()? {
            continue;
        }
        let done = decision_name == DONE && !newest.is_none();
        entries.push(model(
            py,
            RecordId::ColumnMigrationPlanEntry,
            &[
                ("model_name", name_object(&candidate.model)?),
                ("destination", sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(&candidate.model)?),
                (
                    "origin_column",
                    if let Some(origin) = origin {
                        native_field(&origin, "name")?
                    } else {
                        origin_name
                    },
                ),
                ("destination_column", destination),
                (
                    "discovery",
                    enum_from_value(py, RecordId::MigrationDiscovery, discovery)?,
                ),
                ("decision", decision),
                (
                    "target_name",
                    if done {
                        native_field(&newest, "target_name")?
                    } else {
                        native_field(&native_field(runtime, "project")?, "effective_target_name")?
                    },
                ),
                (
                    "completed_at",
                    if done {
                        native_field(&newest, "created_at")?
                    } else {
                        py.None().into_bound(py)
                    },
                ),
            ],
        )?);
    }
    Ok(entries)
}

pub(crate) fn checked<'py>(
    runtime: &Bound<'py, PyAny>,
    candidate: &ColumnCandidate<'py>,
    entries: Vec<Bound<'py, PyAny>>,
    physical: &Bound<'py, PyAny>,
) -> PyResult<CheckedColumns<'py>> {
    let py = runtime.py();
    let mut pending = false;
    for entry in &entries {
        pending |= native_field(&native_field(entry, "decision")?, "renames")?.is_truthy()?;
    }
    if !pending {
        return Ok(CheckedColumns {
            entries,
            warnings: Vec::new(),
        });
    }
    let destination =
        sqlbuild_python_project::compiled_views::main::compiled_models::destination::destination(
            &candidate.model,
        )?;
    let qualified = native_field(&destination, "qualified_name")?;
    let relation = if physical.is_truthy()? {
        physical.clone()
    } else if qualified.is_truthy()? {
        qualified
    } else {
        native_field(&destination, "name")?
    };
    let reason = match runtime_reads(runtime)?
        .get()
        .reads()
        .adapter_read(runtime.py(), AdapterRead::ColumnRenameUnavailable(relation))?
    {
        Read::Known(reason) => reason,
        Read::Unknown(_) => runtime.py().None().into_bound(runtime.py()),
    };
    if reason.is_none() {
        return Ok(CheckedColumns {
            entries,
            warnings: Vec::new(),
        });
    }
    let mut checked: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in entries {
        if !native_field(&native_field(&entry, "decision")?, "renames")?.is_truthy()? {
            checked.push(entry);
        } else if native_field(&entry, "discovery")?.eq("manual")? {
            checked.push(replace(
                &entry,
                &[
                    (
                        "decision",
                        enum_member(py, RecordId::ColumnMigrationDecision, "UNSUPPORTED")?,
                    ),
                    ("message", reason.clone()),
                ],
            )?);
        } else {
            warnings.push(
                crate::planner_migrations::_helpers::column::column_warnings::unavailable(
                    &entry,
                    &reason.extract::<String>()?,
                )?,
            );
        }
    }
    Ok(CheckedColumns {
        entries: checked,
        warnings,
    })
}

pub(crate) fn hints<'py>(
    candidate: &ColumnCandidate<'py>,
    entries: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py = candidate.model.py();
    if candidate.previous.is_none() || candidate.current.is_none() {
        return Ok(PyTuple::empty(py).into_any());
    }
    let excluded = declared_names(candidate)?.call_method0("copy")?;
    let claimed = PyDict::new(py);
    for entry in entries {
        claimed.set_item(lower(&native_field(entry, "origin_column")?)?, py.None())?;
        claimed.set_item(
            lower(&native_field(entry, "destination_column")?)?,
            py.None(),
        )?;
    }
    object(
        py,
        rename_hints(
            py,
            Some(keywords(
                py,
                &[
                    ("model_name", name_object(&candidate.model)?),
                    ("previous", candidate.previous.clone()),
                    ("current", candidate.current.clone()),
                    ("excluded", excluded.call_method1("union", (claimed,))?),
                    (
                        "live_columns",
                        PyFrozenSet::new(py, candidate.warehouse.keys().iter())?.into_any(),
                    ),
                ],
            )?),
        )?,
    )
}
