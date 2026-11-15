//! Per-model plan actions, schema actions and plan warnings.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::planner_changes::results::native_enum;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;

use sqlbuild_analysis::planner_strategy::main::materialization_type::materialization_type;
use sqlbuild_analysis::planner_strategy::main::model_plan_action::model_plan_action;
use sqlbuild_analysis::planner_strategy::main::model_warnings::model_warnings;
use sqlbuild_analysis::planner_strategy::main::schema_actions::schema_actions;
use sqlbuild_analysis::planner_strategy::models::{
    ModelStrategy, PlanAction, PlanReason, SchemaActionKind, WarningInputs,
};

use crate::bindings::_helpers::planner_changes::results::planner_enum;
use crate::bindings::_helpers::planner_strategy::values::{
    change_facts, config_value, finding_rows, input_error, materialization_value,
    model_materialization, optional_string, plan_warning, text,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

fn action_value(action: PlanAction) -> &'static str {
    match action {
        PlanAction::CreateView => "create_view",
        PlanAction::CreateTable => "create_table",
        PlanAction::IncrementalAppend => "incremental_append",
        PlanAction::IncrementalDeleteInsert => "incremental_delete_insert",
        PlanAction::IncrementalMerge => "incremental_merge",
        PlanAction::Snapshot => "snapshot",
        PlanAction::Skip => "skip",
        PlanAction::Custom => "custom",
    }
}

fn reason_value(reason: PlanReason) -> &'static str {
    match reason {
        PlanReason::FirstRun => "first_run",
        PlanReason::Renamed => "renamed",
        PlanReason::FullRefresh => "full_refresh",
        PlanReason::QueryChanged => "query_changed",
        PlanReason::FunctionChanged => "function_changed",
        PlanReason::ConfigChanged => "config_changed",
        PlanReason::SchemaChanged => "schema_changed",
        PlanReason::NormalIncremental => "normal_incremental",
        PlanReason::NoChange => "no_change",
        PlanReason::Disabled => "disabled",
    }
}

/// The optional `on_schema_change` policy as text.
fn schema_policy(on_schema_change: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if on_schema_change.is_none() {
        return Ok(None);
    }
    text(on_schema_change).map(Some)
}

/// The `MaterializationType` of a model's `materialized` config: table when unset, custom when unknown.
#[pyfunction(name = "get_materialization_type")]
#[pyo3(signature = (model))]
pub(crate) fn materialization_of<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    planner_enum(
        model.py(),
        RecordId::MaterializationType,
        materialization_value(model_materialization(model)?),
    )
}

/// The plan action and reason for one model.
#[pyfunction]
#[pyo3(signature = (*, model, change_result, full_refresh))]
pub(crate) fn resolve_model_plan_action<'py>(
    model: &Bound<'py, PyAny>,
    change_result: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let enabled: Bound<'py, PyAny> = config_value(model, "enabled")?;
    let strategy = ModelStrategy {
        name: model_reads::model_name(model)?,
        disabled: enabled.is_instance_of::<PyBool>() && !enabled.is_truthy()?,
        materialization: model_materialization(model)?,
        incremental_strategy: optional_string(&config_value(model, "incremental_strategy")?)?,
    };
    let (action, reason) =
        model_plan_action(&strategy, &change_facts(change_result)?, full_refresh).map_err(
            |error| {
                input_error(
                    py,
                    ErrorId::PlannerInput,
                    &error.message(),
                    Some(error.code()),
                )
            },
        )?;
    PyTuple::new(
        py,
        [
            planner_enum(py, RecordId::PlanAction, action_value(action))?,
            planner_enum(py, RecordId::PlanReason, reason_value(reason))?,
        ],
    )
}

/// Concrete schema change actions from findings and `on_schema_change`.
#[pyfunction]
#[pyo3(signature = (*, schema_findings, on_schema_change))]
pub(crate) fn resolve_schema_actions<'py>(
    schema_findings: &Bound<'py, PyAny>,
    on_schema_change: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = schema_findings.py();
    let policy: Option<String> = schema_policy(on_schema_change)?;
    let mut actions: Vec<Bound<'py, PyAny>> = Vec::new();
    for row in schema_actions(&finding_rows(schema_findings)?, policy.as_deref()) {
        let kind: &str = match row.kind {
            SchemaActionKind::AddColumn => "add_column",
            SchemaActionKind::DropColumn => "drop_column",
            SchemaActionKind::AlterColumnType => "alter_column_type",
        };
        actions.push(new_record_with(
            py,
            RecordId::SchemaAction,
            &[
                ("kind", native_enum(py, RecordId::SchemaActionKind, kind)?),
                (
                    "column_name",
                    PyString::new(py, &row.column_name).into_any(),
                ),
                ("column_type", row.column_type.into_pyobject(py)?.into_any()),
            ],
        )?);
    }
    PyTuple::new(py, actions)
}

/// The warning inputs of one model's plan decisions.
pub(crate) fn warning_inputs(
    model_name: &Bound<'_, PyAny>,
    materialization: &Bound<'_, PyAny>,
    (schema_actions, on_schema_change, type_enforcement): (
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
    ),
) -> PyResult<WarningInputs> {
    Ok(WarningInputs {
        model_name: text(model_name)?,
        materialization: materialization_type(Some(&text(materialization)?)),
        has_schema_actions: schema_actions.is_truthy()?,
        on_schema_change: schema_policy(on_schema_change)?,
        type_enforcement: type_enforcement.is_truthy()?,
    })
}

/// The `PlanWarning`s of one model's warning inputs and change result.
pub(crate) fn model_warning_records<'py>(
    model_name: &Bound<'py, PyAny>,
    inputs: &WarningInputs,
    change_result: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    model_warnings(inputs, &change_facts(change_result)?)
        .into_iter()
        .map(|row| plan_warning(model_name, row))
        .collect()
}

/// Warnings for one model from its change detection and plan decisions.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_model_warnings<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_model_warnings",
        options,
        (
            &[
                "model_name",
                "materialization_type",
                "change_result",
                "schema_actions",
                "on_schema_change",
                "type_enforcement",
            ],
            &[],
        ),
    )?;
    let model_name: Bound<'py, PyAny> = arguments.value("model_name")?;
    let inputs: WarningInputs = warning_inputs(
        &model_name,
        &arguments.value("materialization_type")?,
        (
            &arguments.value("schema_actions")?,
            &arguments.value("on_schema_change")?,
            &arguments.value("type_enforcement")?,
        ),
    )?;
    PyTuple::new(
        py,
        model_warning_records(&model_name, &inputs, &arguments.value("change_result")?)?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(materialization_of, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_plan_action, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_schema_actions, module)?)?;
    module.add_function(wrap_pyfunction!(build_model_warnings, module)?)
}
