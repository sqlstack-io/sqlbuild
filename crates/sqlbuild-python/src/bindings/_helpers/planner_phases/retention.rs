//! Warehouse-aware time-travel retention and Snowflake table-type planning.

use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::model_config::typed_config::{config_value, typed_values};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_phases::constants::{
    BIGQUERY_ADAPTER, DECREASE_WARNING, DOWNGRADE_HISTORY_DAYS, PERMANENT_TABLE_TYPE,
    RETENTION_DECREASE, RETENTION_INCREASE, RETENTION_MATCH, SNOWFLAKE_ADAPTER,
    TRANSIENT_TABLE_TYPE,
};
use crate::bindings::_helpers::planner_phases::objects::{
    construct, member, native, native_member, optional_text, planner_error, planner_model, replace,
    sorted_names, text, tuple,
};
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::planner_warehouse::main::retention_change::retention_change;
use sqlbuild_analysis::planner_warehouse::main::retention_direction::retention_direction;
use sqlbuild_analysis::planner_warehouse::main::table_type_copy_name::table_type_copy_name as copy_name;
use std::collections::{BTreeMap, BTreeSet};

const MATERIALIZED_KEY: &str = "materialized";

/// The resolved config of the planned target, or defaults without one.
pub(crate) fn effective_target_config<'py>(
    runtime: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let project_config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let local_config = runtime_field(runtime, |runtime| &runtime.local_config)?;
    let target_name = native_field(&native_field(runtime, "project")?, "effective_target_name")?;
    if project_config.is_none() || local_config.is_none() || target_name.is_none() {
        return native(py, RecordId::TargetConfig, &[]);
    }
    resolve_target_config(py, &project_config, &local_config, &target_name)
}

/// A deterministic identifier-fitted conversion copy name.
#[pyfunction]
#[pyo3(signature = (*, target_name, identifier_limit))]
pub(crate) fn table_type_copy_name(target_name: &str, identifier_limit: i64) -> String {
    copy_name(target_name, identifier_limit)
}

fn selected_models<'py>(
    runtime: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let scope = scope_data(scope)?;
    let mut models: Vec<_> = Vec::new();
    for model in
        project_reads::models(&runtime_field(runtime, |runtime| &runtime.project)?)?.try_iter()?
    {
        let model = model?;
        if scope
            .selected
            .contains(&("model".to_owned(), model_reads::model_name(&model)?))
        {
            models.push(model);
        }
    }
    Ok(models)
}

fn adapter_is(runtime: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    native_field(&native_field(runtime, "adapter")?, "adapter_name")?.eq(name)
}

/// Entries for selected Snowflake table-type drift.
#[pyfunction]
#[pyo3(signature = (*, runtime, warehouse, scope))]
pub(crate) fn plan_table_types<'py>(
    runtime: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let models = selected_models(runtime, scope)?;
    let snowflake = adapter_is(runtime, SNOWFLAKE_ADAPTER)?;
    let mut declared: Vec<_> = Vec::new();
    for model in &models {
        if native_field(
            &native_field(&model_reads::config(model)?, "table_type")?,
            "declared",
        )?
        .is_truthy()?
        {
            declared.push(model_reads::model_name(model)?);
        }
    }
    if !declared.is_empty() && !snowflake {
        return Err(planner_error(
            py,
            &format!(
                "table_type is not supported on this adapter; declared by model(s): {}",
                declared.join(", ")
            ),
            (None, None),
        ));
    }
    if !snowflake {
        return tuple(py, Vec::new());
    }
    let target = effective_target_config(runtime)?;
    let relations = native_field(&native_field(warehouse, "snapshot")?, "existing_relations")?;
    let adapter = runtime_field(runtime, |runtime| &runtime.adapter)?;
    let materialization = record_class(py, RecordId::MaterializationType)?;
    let mut entries: Vec<_> = Vec::new();
    for model in models {
        let config = native_field(&model, "config")?;
        let backed = call_with(
            &native_field(&materialization, "is_table_backed")?,
            &[(
                "materialized",
                config_value(py, &typed_values(&config)?, MATERIALIZED_KEY)?,
            )],
        )?;
        if !backed.is_truthy()? {
            continue;
        }
        let relation = mapping_get(&relations, model_reads::name_object(&model)?)?;
        if relation.is_none() {
            continue;
        }
        let table_type = native_field(&config, "table_type")?;
        let desired_transient = native_field(&table_type, "value")?.eq(TRANSIENT_TABLE_TYPE)?;
        let actual = native_field(&relation, "is_transient")?;
        let actual: Option<bool> = if actual.is_none() {
            None
        } else {
            Some(actual.is_truthy()?)
        };
        if actual == Some(desired_transient) {
            continue;
        }
        let downgrade = actual == Some(false) && desired_transient;
        let destination = model_reads::destination(&model)?;
        let limit: i64 = adapter
            .call_method0("maximum_identifier_length")?
            .extract()?;
        let target_name: String = native_field(&destination, "name")?.extract()?;
        entries.push(planner_model(
            py,
            RecordId::TableTypePlanEntry,
            &[
                ("model_name", model_reads::name_object(&model)?),
                ("destination", destination),
                ("copy_name", text(py, &copy_name(&target_name, limit))),
                ("desired_type", native_field(&native_field(&table_type, "value")?, "value")?),
                (
                    "actual_type",
                    optional_text(
                        py,
                        actual.map(|transient| {
                            if transient {
                                TRANSIENT_TABLE_TYPE
                            } else {
                                PERMANENT_TABLE_TYPE
                            }
                        }),
                    ),
                ),
                ("source", native_field(&native_field(&table_type, "source")?, "value")?),
                ("downgrade", downgrade.into_pyobject(py)?.to_owned().into_any()),
                (
                    "downgrade_policy",
                    native_field(&native_field(&target, "table_type_downgrade")?, "value")?,
                ),
                (
                    "irreversible_warning",
                    optional_text(
                        py,
                        downgrade
                            .then(|| {
                                format!(
                                    "Downgrading may discard up to {DOWNGRADE_HISTORY_DAYS} days of time-travel history."
                                )
                            })
                            .as_deref(),
                    ),
                ),
            ],
        )?);
    }
    tuple(py, entries)
}

struct RetentionEntry<'py> {
    request: Bound<'py, PyAny>,
    model_names: Bound<'py, PyAny>,
    state: Option<Bound<'py, PyAny>>,
    source: Bound<'py, PyAny>,
}

fn entry<'py>(
    base: &RetentionEntry<'py>,
    (direction, phase): (&str, &str),
    statements: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = base.request.py();
    let days = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        base.state.as_ref().map_or_else(
            || Ok(py.None().into_bound(py)),
            |state| native_field(state, name),
        )
    };
    planner_model(
        py,
        RecordId::RetentionPlanEntry,
        &[
            ("request", base.request.clone()),
            ("model_names", base.model_names.clone()),
            ("actual_days", days("configured_days")?),
            ("effective_days", days("effective_days")?),
            ("source", base.source.clone()),
            (
                "direction",
                native_member(py, RecordId::RetentionDirection, direction)?,
            ),
            (
                "phase",
                native_member(py, RecordId::RetentionPlanPhase, phase)?,
            ),
            ("statements", statements),
            (
                "irreversible_warning",
                optional_text(
                    py,
                    (direction == RETENTION_DECREASE).then_some(DECREASE_WARNING),
                ),
            ),
        ],
    )
}

fn flatten<'py>(changes: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let mut statements: Vec<_> = Vec::new();
    for change in changes.try_iter()? {
        for statement in native_field(&change?, "statements")?.try_iter()? {
            statements.push(statement?);
        }
    }
    tuple(changes.py(), statements)
}

fn render_changes<'py>(
    runtime: &Bound<'py, PyAny>,
    request: &Bound<'py, PyAny>,
    state: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut fields = vec![("request", request.clone())];
    if let Some(state) = state {
        fields.push(("state", state.clone()));
    }
    call_with(
        &native_field(
            &native_field(runtime, "adapter")?,
            "render_retention_changes",
        )?,
        &fields,
    )
}

fn inspect<'py>(
    runtime: &Bound<'py, PyAny>,
    request: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &native_field(
            &runtime_field(runtime, |runtime| &runtime.adapter)?,
            "inspect_retention",
        )?,
        &[
            (
                "connection",
                runtime_field(runtime, |runtime| &runtime.connection)?,
            ),
            ("request", request.clone()),
        ],
    )
}

fn state_direction(state: &Bound<'_, PyAny>, desired_days: i64) -> PyResult<&'static str> {
    let optional = |name: &str| -> PyResult<Option<i64>> { native_field(state, name)?.extract() };
    Ok(retention_direction(
        [
            optional("delta_log_retention_days")?,
            optional("delta_deleted_file_retention_days")?,
        ],
        native_field(state, "effective_days")?.extract()?,
        desired_days,
    ))
}

fn transient_error(py: Python<'_>, name: &str, desired_days: i64, convert: &str) -> pyo3::PyErr {
    planner_error(
        py,
        &format!(
            "model '{name}': Snowflake transient tables cannot retain time travel for {desired_days} days; retention above one day requires a permanent table; {convert}table_type permanent on the model, its materialization default, or the target default_table_type"
        ),
        (None, None),
    )
}

fn relation_retention<'py>(
    runtime: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = runtime.py();
    let name: String = model_reads::model_name(model)?;
    let config = native_field(model, "config")?;
    let retention = native_field(&config, "time_travel_retention")?;
    if native_field(&retention, "desired_days")?.is_none()
        || native_field(&retention, "source")?.is_none()
    {
        return Err(planner_error(
            py,
            &format!("model '{name}': managed retention is incomplete"),
            (None, None),
        ));
    }
    let desired_days: i64 = native_field(&retention, "desired_days")?.extract()?;
    let destination = model_reads::destination(model)?;
    let schema = native_field(&destination, "schema")?;
    if schema.is_none() {
        return Err(planner_error(
            py,
            &format!("model '{name}': retention requires a destination schema"),
            (None, None),
        ));
    }
    let request = construct(
        py,
        AdapterHost::RetentionRequest,
        &[
            ("request_id", model_reads::name_object(model)?),
            (
                "scope",
                member(py, AdapterHost::RetentionScope, "relation")?,
            ),
            ("database", native_field(&destination, "database")?),
            ("schema", schema.clone()),
            ("name", native_field(&destination, "name")?),
            ("desired_days", native_field(&retention, "desired_days")?),
        ],
    )?;
    let mut base = RetentionEntry {
        request: request.clone(),
        model_names: tuple(py, vec![model_reads::name_object(model)?])?,
        state: None,
        source: native_field(&native_field(&retention, "source")?, "value")?,
    };
    let snowflake = adapter_is(runtime, SNOWFLAKE_ADAPTER)?;
    let table_type = native_field(&native_field(&config, "table_type")?, "value")?;
    let relations = native_field(&native_field(warehouse, "snapshot")?, "existing_relations")?;
    if !relations.contains(model_reads::name_object(model)?)? {
        if snowflake && desired_days > 1 && table_type.eq(TRANSIENT_TABLE_TYPE)? {
            return Err(transient_error(py, &name, desired_days, "set "));
        }
        let changes = render_changes(runtime, &request, None)?;
        return Ok(vec![entry(
            &base,
            ("apply_after_create", "after_create"),
            flatten(&changes)?,
        )?]);
    }
    let relation = relations.get_item(model_reads::name_object(model)?)?;
    let relation_transient = native_field(&relation, "is_transient")?;
    let lookup = call_with(
        &host::entry(py, AdapterHost::RelationLookup)?.getattr("key")?,
        &[
            ("database", native_field(&destination, "database")?),
            ("schema", schema),
            ("name", native_field(&destination, "name")?),
        ],
    )?;
    let mut listed = py.None().into_bound(py);
    if native_field(&relation, "identity")?.eq(&lookup)? {
        listed = call_with(
            &native_field(
                &native_field(runtime, "adapter")?,
                "retention_state_from_relation",
            )?,
            &[("request", request.clone()), ("relation", relation.clone())],
        )?;
    }
    let state = if listed.is_truthy()? {
        listed
    } else {
        inspect(runtime, &request)?
    };
    if native_field(&state, "is_transient")?.is_truthy()?
        && desired_days > 1
        && table_type.eq(TRANSIENT_TABLE_TYPE)?
    {
        return Err(transient_error(
            py,
            &name,
            desired_days,
            "convert it by setting ",
        ));
    }
    base.state = Some(state.clone());
    if snowflake
        && desired_days > 1
        && relation_transient.is(pyo3::types::PyBool::new(py, true))
        && table_type.eq(PERMANENT_TABLE_TYPE)?
    {
        let changes = render_changes(runtime, &request, None)?;
        return Ok(vec![entry(
            &base,
            ("apply_after_create", "after_create"),
            flatten(&changes)?,
        )?]);
    }
    let direction = state_direction(&state, desired_days)?;
    if direction == RETENTION_MATCH {
        return Ok(vec![entry(
            &base,
            (direction, "none"),
            tuple(py, Vec::new())?,
        )?]);
    }
    let changes = render_changes(runtime, &request, Some(&state))?;
    let mut entries: Vec<_> = Vec::new();
    for change in changes.try_iter()? {
        let change = change?;
        let phase: String = native_field(&change, "phase")?.str()?.extract()?;
        entries.push(entry(
            &base,
            retention_change(&phase, direction),
            native_field(&change, "statements")?,
        )?);
    }
    Ok(entries)
}

fn bigquery_retention<'py>(
    runtime: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = runtime.py();
    let target = effective_target_config(runtime)?;
    if !native_field(&target, "owns_time_travel_retention_namespace")?.is_truthy()? {
        return Err(planner_error(
            py,
            "BigQuery managed time_travel_retention requires target owns_time_travel_retention_namespace = true",
            (None, None),
        ));
    }
    type Scope = (Option<String>, String);
    let mut managed: BTreeMap<Scope, Vec<Bound<'py, PyAny>>> = BTreeMap::new();
    for model in native_field(
        &runtime_field(runtime, |runtime| &runtime.project)?,
        "models",
    )?
    .try_iter()?
    {
        let model = model?;
        let retention = native_field(&native_field(&model, "config")?, "time_travel_retention")?;
        let destination = model_reads::destination(&model)?;
        if native_field(&retention, "unmanaged")?.is_truthy()?
            || native_field(&retention, "desired_days")?.is_none()
            || native_field(&destination, "schema")?.is_none()
        {
            continue;
        }
        managed
            .entry((
                native_field(&destination, "database")?.extract()?,
                native_field(&destination, "schema")?.extract()?,
            ))
            .or_default()
            .push(model);
    }
    let mut scopes: BTreeSet<(String, String, Option<String>)> = BTreeSet::new();
    for model in models {
        let destination = model_reads::destination(model)?;
        let database: Option<String> = native_field(&destination, "database")?.extract()?;
        let schema: String = native_field(&destination, "schema")?.str()?.extract()?;
        scopes.insert((database.clone().unwrap_or_default(), schema, database));
    }
    let mut entries: Vec<_> = Vec::new();
    for (_, schema, database) in scopes {
        let scope_models = managed
            .get(&(database.clone(), schema.clone()))
            .cloned()
            .unwrap_or_default();
        let mut desired_values: BTreeSet<i64> = BTreeSet::new();
        let mut details: Vec<_> = Vec::new();
        let mut sources: BTreeSet<String> = BTreeSet::new();
        let mut names: Vec<_> = Vec::new();
        for item in &scope_models {
            let retention = native_field(&native_field(item, "config")?, "time_travel_retention")?;
            let desired = native_field(&retention, "desired_days")?;
            details.push(format!(
                "{}={}d",
                native_field(item, "name")?.str()?,
                desired.str()?
            ));
            if !desired.is_none() {
                desired_values.insert(desired.extract()?);
            }
            let source = native_field(&retention, "source")?;
            if !source.is_none() {
                sources.insert(native_field(&source, "value")?.extract()?);
            }
            names.push(native_field(item, "name")?.extract::<String>()?);
        }
        let prefix = database
            .as_ref()
            .map_or_else(String::new, |database| format!("{database}."));
        if desired_values.len() != 1 {
            return Err(planner_error(
                py,
                &format!(
                    "BigQuery dataset {prefix}{schema} has conflicting managed retention values: {}",
                    details.join(", ")
                ),
                (None, None),
            ));
        }
        let desired_days = desired_values.into_iter().next().unwrap_or_default();
        let request = construct(
            py,
            AdapterHost::RetentionRequest,
            &[
                ("request_id", text(py, &format!("{prefix}{schema}"))),
                (
                    "scope",
                    member(py, AdapterHost::RetentionScope, "namespace")?,
                ),
                ("database", optional_text(py, database.as_deref())),
                ("schema", text(py, &schema)),
                ("desired_days", desired_days.into_pyobject(py)?.into_any()),
            ],
        )?;
        let state = inspect(runtime, &request)?;
        let base = RetentionEntry {
            request: request.clone(),
            model_names: sorted_names(py, names)?,
            state: Some(state.clone()),
            source: text(py, &sources.into_iter().collect::<Vec<_>>().join(",")),
        };
        if !native_field(&state, "exists")?.is_truthy()? {
            let changes = render_changes(runtime, &request, None)?;
            entries.push(entry(
                &base,
                ("apply_after_create", "after_create"),
                flatten(&changes)?,
            )?);
            continue;
        }
        let direction = state_direction(&state, desired_days)?;
        let statements = if direction == RETENTION_MATCH {
            tuple(py, Vec::new())?
        } else {
            flatten(&render_changes(runtime, &request, Some(&state))?)?
        };
        let phase = if direction == RETENTION_MATCH {
            "none"
        } else if direction == RETENTION_INCREASE {
            "pre"
        } else {
            "post"
        };
        entries.push(entry(&base, (direction, phase), statements)?);
    }
    Ok(entries)
}

/// Inspect managed selected retention scopes and produce independent plan entries.
#[pyfunction]
#[pyo3(signature = (*, runtime, warehouse, scope))]
pub(crate) fn plan_retention<'py>(
    runtime: &Bound<'py, PyAny>,
    warehouse: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let mut models: Vec<_> = Vec::new();
    for model in selected_models(runtime, scope)? {
        let retention = native_field(&model_reads::config(&model)?, "time_travel_retention")?;
        if !native_field(&retention, "unmanaged")?.is_truthy()?
            && !native_field(&retention, "desired_days")?.is_none()
        {
            models.push(model);
        }
    }
    if models.is_empty() {
        return tuple(py, Vec::new());
    }
    let entries = if adapter_is(runtime, BIGQUERY_ADAPTER)? {
        bigquery_retention(runtime, &models)?
    } else {
        let mut entries: Vec<_> = Vec::new();
        for model in &models {
            entries.extend(relation_retention(runtime, warehouse, model)?);
        }
        entries
    };
    let policy = native_field(
        &native_field(
            &effective_target_config(runtime)?,
            "time_travel_retention_decrease",
        )?,
        "value",
    )?;
    let mut decided: Vec<_> = Vec::new();
    for entry in entries {
        decided.push(replace(&entry, &[("decrease_policy", policy.clone())])?);
    }
    tuple(py, decided)
}
