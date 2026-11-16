//! Computed attributes of the natively hosted plan entry records and cursor overrides.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_core::migration_values::main::migration_decision_traits::migration_decision_traits;
use sqlbuild_core::migration_values::main::model_migration_blocks_build::model_migration_blocks_build as blocks_build;
use sqlbuild_core::migration_values::main::storage_transition::storage_transition;
use sqlbuild_core::migration_values::models::{DecisionTraits, ModelMigrationFacts};

use crate::bindings::_helpers::cursor::scalars::parse_value;
use sqlbuild_python_base::boundary::main::call_argument::call_argument;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const DECREASING_DIRECTIONS: [&str; 2] = ["decrease", "mixed"];
const STEP_ACTIONS: [&str; 2] = ["archive_and_view", "view_only"];
const TABLE_BACKED: [&str; 3] = ["table", "incremental", "snapshot"];
const ORIGIN_MISSING: &str = "origin_missing";
const SOURCE_MISSING: &str = "source_missing";
const DENY_POLICY: &str = "deny";
const INCOMPATIBLE: &str = "incompatible";
const DEFAULT_TARGET_LABEL: &str = "default";
const INTEGER_CURSOR: &str = "integer";
/// `CursorOverrides` field positions: `start_ts`, `end_ts`, `start_int`, `end_int`, `start`, `end`.
const OVERRIDE_FIELDS: [&str; 6] = ["start_ts", "end_ts", "start_int", "end_int", "start", "end"];
const TIMESTAMP_FLAGS: [&str; 2] = ["--start-cursor-ts", "--end-cursor-ts"];
const INTEGER_FLAGS: [&str; 2] = ["--start-cursor-int", "--end-cursor-int"];
const UNTYPED_FLAGS: [&str; 2] = ["--start-cursor", "--end-cursor"];

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

fn flag(py: Python<'_>, value: bool) -> PyResult<Py<PyAny>> {
    value.into_py_any(py)
}

/// `RetentionPlanEntry.decreases`: whether the entry lowers any live retention value.
pub(crate) fn retention_decreases(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let direction: String = text(&native_field(instance, "direction")?)?;
    flag(
        instance.py(),
        DECREASING_DIRECTIONS.contains(&direction.as_str()),
    )
}

/// `OldNameViewPlanEntry.runs_steps`: whether the build archives the old relation or creates the view.
pub(crate) fn old_name_runs_steps(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let action: String = text(&native_field(instance, "action")?)?;
    flag(instance.py(), STEP_ACTIONS.contains(&action.as_str()))
}

/// `PlanOutput.python_source_entries`: the source entries Python reads resolve.
pub(crate) fn plan_python_source_entries(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    for name in ["python_source_read_map", "source_read_map"] {
        let entries: Bound<'_, PyAny> = native_field(instance, name)?;
        if entries.is_truthy()? {
            return Ok(entries.unbind());
        }
    }
    Ok(native_field(instance, "source_map")?.unbind())
}

/// `MaterializationType.is_table_backed(materialized=...)`.
pub(crate) fn materialization_is_table_backed(
    _class: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let materialized: Bound<'_, PyAny> = call_argument((args, kwargs), "materialized", 0)?;
    let backed: bool = match materialized.downcast::<PyString>() {
        Ok(value) => TABLE_BACKED.contains(&value.extract::<String>()?.as_str()),
        Err(_) => false,
    };
    flag(args.py(), backed)
}

fn decision_traits(entry: &Bound<'_, PyAny>) -> PyResult<(String, DecisionTraits)> {
    let decision: String = text(&native_field(entry, "decision")?)?;
    let traits: DecisionTraits = migration_decision_traits(&decision);
    Ok((decision, traits))
}

fn model_facts(entry: &Bound<'_, PyAny>) -> PyResult<ModelMigrationFacts> {
    let (decision, traits) = decision_traits(entry)?;
    Ok(ModelMigrationFacts {
        origin_missing: decision == ORIGIN_MISSING,
        origin_tracked: native_field(entry, "origin_tracked")?.is_truthy()?,
        deny_missing_origin: text(&native_field(entry, "missing_origin_policy")?)? == DENY_POLICY,
        decision: traits,
        incompatible: text(&native_field(entry, "compatibility")?)? == INCOMPATIBLE,
    })
}

/// `ModelMigrationPlanEntry.origin_missing`.
pub(crate) fn model_migration_origin_missing(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    flag(entry.py(), model_facts(entry)?.origin_missing)
}

/// `ModelMigrationPlanEntry.origin_hidden`: a missing origin with build history in this target.
pub(crate) fn model_migration_origin_hidden(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let facts: ModelMigrationFacts = model_facts(entry)?;
    flag(entry.py(), facts.origin_missing && facts.origin_tracked)
}

/// `ModelMigrationPlanEntry.storage_transition`: the table-type transition label, when known.
pub(crate) fn model_migration_storage_transition(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let origin: bool = native_field(entry, "origin_is_transient")?.is_truthy()?;
    let stage: Bound<'_, PyAny> = native_field(entry, "stage_is_transient")?;
    let stage: Option<bool> = if stage.is_none() {
        None
    } else {
        Some(stage.is_truthy()?)
    };
    storage_transition(origin, stage).into_py_any(entry.py())
}

/// `ModelMigrationPlanEntry.target_label`: the target name, naming the unnamed target `default`.
pub(crate) fn model_migration_target_label(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let target: Bound<'_, PyAny> = native_field(entry, "target_name")?;
    if target.is_truthy()? {
        return Ok(target.unbind());
    }
    DEFAULT_TARGET_LABEL.into_py_any(entry.py())
}

/// `ModelMigrationPlanEntry.blocks_build`: whether the migration stops a build before execution.
pub(crate) fn model_migration_blocks_build(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    flag(entry.py(), blocks_build(&model_facts(entry)?))
}

/// `ColumnMigrationPlanEntry.origin_missing`.
pub(crate) fn column_migration_origin_missing(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    flag(
        entry.py(),
        text(&native_field(entry, "decision")?)? == SOURCE_MISSING,
    )
}

/// `ColumnMigrationPlanEntry.blocks_build`.
pub(crate) fn column_migration_blocks_build(entry: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let decision: Bound<'_, PyAny> = native_field(entry, "decision")?;
    let blocks: bool = if text(&decision)? == SOURCE_MISSING {
        text(&native_field(entry, "missing_origin_policy")?)? == DENY_POLICY
    } else {
        native_field(&decision, "blocks_build")?.is_truthy()?
    };
    flag(entry.py(), blocks)
}

/// The `PlannerInputError` carrying `message`.
fn planner_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::PlannerInput, &message)
}

/// A `--*-cursor-ts` value: text checked as an ISO timestamp and parsed, others unchanged.
fn override_timestamp<'py>(
    flag_name: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_none() || !value.is_instance_of::<PyString>() {
        return Ok(value.clone());
    }
    let datetime: Bound<'py, PyAny> = imported(py, "datetime", "datetime")?;
    if let Err(error) = datetime.call_method1("fromisoformat", (value,)) {
        let shown: String = error.value(py).str()?.extract()?;
        return Err(planner_error(
            py,
            format!(
                "{flag_name} value '{}' is not a valid ISO timestamp: {shown}",
                text(value)?
            ),
        ));
    }
    parse_value(value, &PyString::new(py, "timestamp").into_any())
}

/// A `--*-cursor-int` value: text checked as a whole number and parsed, others unchanged.
fn override_integer<'py>(
    flag_name: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_none() || !value.is_instance_of::<PyString>() {
        return Ok(value.clone());
    }
    let shown: String = text(value)?;
    let invalid = || {
        planner_error(
            py,
            format!("{flag_name} value '{shown}' is not a valid integer"),
        )
    };
    let Ok(decimal) = imported(py, "decimal", "Decimal")?.call1((value,)) else {
        return Err(invalid());
    };
    let whole: Bound<'py, PyAny> = py.get_type::<pyo3::types::PyInt>().call1((&decimal,))?;
    if !decimal.eq(&whole)? {
        return Err(planner_error(
            py,
            format!("{flag_name} value '{shown}' is not a whole number"),
        ));
    }
    let parsed: Bound<'py, PyAny> =
        parse_value(value, &PyString::new(py, INTEGER_CURSOR).into_any())?;
    if parsed.is_instance(&record_class(py, RecordId::IntegerValue)?)? {
        return Ok(parsed);
    }
    Err(invalid())
}

/// `CursorOverrides`: typed flag values parsed, untyped `start`/`end` kept as text.
pub(crate) fn finish_cursor_overrides(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    for (index, flag_name) in TIMESTAMP_FLAGS.iter().enumerate() {
        let parsed: Bound<'_, PyAny> = override_timestamp(flag_name, values[index].bind(py))?;
        values[index] = parsed.unbind();
    }
    for (offset, flag_name) in INTEGER_FLAGS.iter().enumerate() {
        let parsed: Bound<'_, PyAny> = override_integer(flag_name, values[2 + offset].bind(py))?;
        values[2 + offset] = parsed.unbind();
    }
    Ok(values)
}

/// `CursorOverrides.has_untyped`: whether `--start-cursor` or `--end-cursor` awaits a type.
pub(crate) fn overrides_have_untyped(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let untyped: bool =
        !native_field(instance, "start")?.is_none() || !native_field(instance, "end")?.is_none();
    flag(instance.py(), untyped)
}

/// `parsed or fallback`.
fn either<'py>(
    parsed: Bound<'py, PyAny>,
    fallback: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if parsed.is_truthy()? {
        return Ok(parsed);
    }
    Ok(fallback)
}

/// `CursorOverrides.typed_as(cursor_type=...)`: `--start-cursor`/`--end-cursor` read as that type.
pub(crate) fn overrides_typed_as(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    if !overrides_have_untyped(instance)?.bind(py).is_truthy()? {
        return Ok(instance.clone().unbind());
    }
    let cursor_type: String = text(&call_argument((args, kwargs), "cursor_type", 0)?)?;
    let field = |name: &str| native_field(instance, name);
    let (start, end) = (field(OVERRIDE_FIELDS[4])?, field(OVERRIDE_FIELDS[5])?);
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    if cursor_type == INTEGER_CURSOR {
        keywords.set_item("start_ts", field("start_ts")?)?;
        keywords.set_item("end_ts", field("end_ts")?)?;
        keywords.set_item(
            "start_int",
            either(
                override_integer(UNTYPED_FLAGS[0], &start)?,
                field("start_int")?,
            )?,
        )?;
        keywords.set_item(
            "end_int",
            either(override_integer(UNTYPED_FLAGS[1], &end)?, field("end_int")?)?,
        )?;
    } else {
        keywords.set_item(
            "start_ts",
            either(
                override_timestamp(UNTYPED_FLAGS[0], &start)?,
                field("start_ts")?,
            )?,
        )?;
        keywords.set_item(
            "end_ts",
            either(
                override_timestamp(UNTYPED_FLAGS[1], &end)?,
                field("end_ts")?,
            )?,
        )?;
        keywords.set_item("start_int", field("start_int")?)?;
        keywords.set_item("end_int", field("end_int")?)?;
    }
    Ok(instance.get_type().call((), Some(&keywords))?.unbind())
}

/// `ScenarioArtifactIdentity.kind` coerced to `ScenarioArtifactKind`.
pub(crate) fn finish_scenario_artifact_kind(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    if let Some(kind) = values.first_mut() {
        *kind = enum_from_value(py, RecordId::ScenarioArtifactKind, kind.bind(py))?.unbind();
    }
    Ok(values)
}

/// `ColumnRenameHint.message`: the one-line hint shown beside the added column.
pub(crate) fn rename_hint_message(hint: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let relation: &str = if native_field(hint, "identical")?.is_truthy()? {
        "same expression as"
    } else {
        "similar to"
    };
    let candidates: Vec<String> = native_field(hint, "candidate_columns")?
        .try_iter()?
        .map(|candidate| Ok(candidate?.str()?.to_string()))
        .collect::<PyResult<_>>()?;
    let origin: &str = match candidates.as_slice() {
        [only] => only,
        _ => "<column>",
    };
    format!(
        "{relation} {}; if this is a rename, add {} (migrate_from {origin})",
        candidates.join(", "),
        native_field(hint, "added_column")?.str()?
    )
    .into_py_any(hint.py())
}

/// `QueryProjection.key`: the case-insensitive output name.
pub(crate) fn projection_key(projection: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(projection, "name")?
        .call_method0("lower")?
        .unbind())
}
