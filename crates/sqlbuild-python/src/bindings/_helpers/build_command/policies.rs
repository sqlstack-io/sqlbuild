//! Plan safety gates and the model-count limit, read from a plan and enforced on the given streams.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::build_command::constants::POLICY_REQUIRE_CONFIRMATION;
use sqlbuild_core::build_command::errors::BuildCommandError;
use sqlbuild_core::build_command::main::model_limit_error::model_limit_error;
use sqlbuild_core::build_command::main::plan_safety_steps::plan_safety_steps;
use sqlbuild_core::build_command::main::render_confirmation_prompt::render_confirmation_prompt;
use sqlbuild_core::build_command::models::{
    ColumnMigrationFact, ModelErrorFact, ModelLimit, ModelMigrationFact, PlanSafetyFacts,
    PlanSafetyGates, PlanWarningFact, PolicyStep, RetentionFact, SnapshotPolicies,
    SnapshotRefreshFact, TableTypeFact,
};
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const SNAPSHOT_VALUE: &str = "snapshot";
const FULL_REFRESH_REASON: &str = "full_refresh";
const SKIP_ACTION: &str = "skip";
const ERROR_SEVERITY: &str = "error";
const POLICY_OPTIONS: (&[&str], &[&str]) = (
    &[
        "plan",
        "gates",
        "snapshots_config",
        "target_name",
        "execution_limits",
        "streams",
    ],
    &[],
);
const LIMIT_OPTIONS: (&[&str], &[&str]) = (
    &["model_count", "target_name", "limits", "refusal_note"],
    &[],
);

/// `CliUserError(message, code=..., help=...)`.
pub(crate) fn build_command_error(py: Python<'_>, error: BuildCommandError) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &error.message,
        (error.code.as_deref(), error.help.as_deref()),
    )
}

fn text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    native_field(value, name)?.str()?.extract()
}

fn optional_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let attribute: Bound<'_, PyAny> = native_field(value, name)?;
    if attribute.is_none() {
        return Ok(None);
    }
    Ok(Some(attribute.str()?.extract()?))
}

fn flag(value: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    native_field(value, name)?.is_truthy()
}

fn model_migration(entry: &Bound<'_, PyAny>) -> PyResult<ModelMigrationFact> {
    let origin: Bound<'_, PyAny> = native_field(entry, "origin")?;
    let qualified: Bound<'_, PyAny> = native_field(&origin, "qualified_name")?;
    let origin_label: String = if qualified.is_truthy()? {
        qualified.str()?.extract()?
    } else {
        text(&origin, "name")?
    };
    Ok(ModelMigrationFact {
        model_name: text(entry, "model_name")?,
        origin_label,
        decision: text(entry, "decision")?,
        blocks_build: flag(entry, "blocks_build")?,
        origin_hidden: flag(entry, "origin_hidden")?,
        origin_missing: flag(entry, "origin_missing")?,
        confirm_missing_origin: text(entry, "missing_origin_policy")?
            == POLICY_REQUIRE_CONFIRMATION,
    })
}

fn column_migration(entry: &Bound<'_, PyAny>) -> PyResult<ColumnMigrationFact> {
    let decision: Bound<'_, PyAny> = native_field(entry, "decision")?;
    Ok(ColumnMigrationFact {
        model_name: text(entry, "model_name")?,
        destination_column: text(entry, "destination_column")?,
        origin_column: text(entry, "origin_column")?,
        decision: decision.str()?.extract()?,
        decision_label: text(&decision, "label")?,
        blocks_build: flag(entry, "blocks_build")?,
        origin_missing: flag(entry, "origin_missing")?,
        confirm_missing_origin: text(entry, "missing_origin_policy")?
            == POLICY_REQUIRE_CONFIRMATION,
    })
}

fn entries<'py>(plan: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(plan, name)?.try_iter()?.collect()
}

/// The models a plan executes: every model entry that is not skipped.
pub(crate) fn executable_model_count(plan: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut count: usize = 0;
    for entry in entries(plan, "model_entries")? {
        if text(&entry, "action")? != SKIP_ACTION {
            count += 1;
        }
    }
    Ok(count)
}

fn plan_facts(plan: &Bound<'_, PyAny>) -> PyResult<PlanSafetyFacts> {
    let mut facts: PlanSafetyFacts = PlanSafetyFacts {
        executable_model_count: executable_model_count(plan)?,
        ..PlanSafetyFacts::default()
    };
    for entry in entries(plan, "migration_entries")? {
        facts.model_migrations.push(model_migration(&entry)?);
    }
    for entry in entries(plan, "column_migration_entries")? {
        facts.column_migrations.push(column_migration(&entry)?);
    }
    for warning in entries(plan, "warnings")? {
        if text(&warning, "severity")? == ERROR_SEVERITY {
            facts.error_warnings.push(PlanWarningFact {
                code: optional_text(&warning, "code")?,
                message: text(&warning, "message")?,
            });
        }
    }
    for entry in entries(plan, "model_entries")? {
        let planning_error: Bound<'_, PyAny> = native_field(&entry, "planning_error")?;
        if !planning_error.is_none() {
            facts.model_errors.push(ModelErrorFact {
                code: optional_text(&planning_error, "code")?,
                message: text(&planning_error, "message")?,
                help: optional_text(&planning_error, "help")?,
            });
        }
        if optional_text(&entry, "materialization_type")?.as_deref() == Some(SNAPSHOT_VALUE)
            && text(&entry, "reason")? == FULL_REFRESH_REASON
        {
            facts.snapshot_refreshes.push(SnapshotRefreshFact {
                name: text(&entry, "name")?,
                historical: !native_field(&entry, "observed_at_column")?.is_none(),
                model_policy: optional_text(&entry, "snapshot_full_refresh")?,
            });
        }
    }
    for entry in entries(plan, "table_type_entries")? {
        if flag(&entry, "downgrade")? {
            facts.table_type_downgrades.push(TableTypeFact {
                model_name: text(&entry, "model_name")?,
                policy: text(&entry, "downgrade_policy")?,
            });
        }
    }
    for entry in entries(plan, "retention_entries")? {
        if flag(&entry, "decreases")? {
            facts.retention_decreases.push(RetentionFact {
                model_names: native_field(&entry, "model_names")?.extract()?,
                policy: text(&entry, "decrease_policy")?,
            });
        }
    }
    Ok(facts)
}

fn read_gates(value: &Bound<'_, PyAny>) -> PyResult<PlanSafetyGates> {
    Ok(PlanSafetyGates {
        allow_missing_migration_origin: flag(value, "allow_missing_migration_origin")?,
        allow_snapshot_full_refresh: flag(value, "allow_snapshot_full_refresh")?,
        allow_table_type_downgrade: flag(value, "allow_table_type_downgrade")?,
        allow_retention_decrease: flag(value, "allow_retention_decrease")?,
        missing_origin_help: text(value, "missing_origin_help")?,
        snapshot_full_refresh_help: text(value, "snapshot_full_refresh_help")?,
        table_type_downgrade_help: text(value, "table_type_downgrade_help")?,
        retention_decrease_help: text(value, "retention_decrease_help")?,
        execution_limit_note: text(value, "execution_limit_note")?,
    })
}

/// The target's model limit from its `ExecutionLimitsConfig`.
pub(crate) fn model_limit(
    target_name: &Bound<'_, PyAny>,
    limits: &Bound<'_, PyAny>,
) -> PyResult<ModelLimit> {
    Ok(ModelLimit {
        target_name: target_name.extract()?,
        max_models: native_field(limits, "max_models")?.extract()?,
        remediation: native_field(limits, "remediation")?.extract()?,
    })
}

/// Ask for each confirmation on the terminal and raise each refusal, in order.
fn apply_steps(
    py: Python<'_>,
    steps: Vec<PolicyStep>,
    (input, output): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    for step in steps {
        let confirmation = match step {
            PolicyStep::Refuse(error) => return Err(build_command_error(py, error)),
            PolicyStep::Confirm(confirmation) => confirmation,
        };
        if !input.call_method0("isatty")?.is_truthy()? {
            return Err(build_command_error(py, confirmation.refusal));
        }
        output.call_method1("write", (render_confirmation_prompt(&confirmation),))?;
        output.call_method0("flush")?;
        let answer: String = input.call_method0("readline")?.extract()?;
        if answer.trim() != confirmation.expected {
            return Err(build_command_error(py, confirmation.cancellation));
        }
    }
    Ok(())
}

/// Apply every plan safety gate to a plan, prompting on the given streams.
pub(crate) fn enforce_plan_policies(
    plan: &Bound<'_, PyAny>,
    (gates, snapshots_config): (&PlanSafetyGates, &Bound<'_, PyAny>),
    limit: &ModelLimit,
    streams: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> Result<(), PyErr> {
    let py: Python<'_> = plan.py();
    let policies: SnapshotPolicies = SnapshotPolicies {
        historical: text(snapshots_config, "historical_full_refresh")?,
        current_state: text(snapshots_config, "current_state_full_refresh")?,
    };
    let steps: Vec<PolicyStep> = plan_safety_steps(&plan_facts(plan)?, gates, (limit, &policies))
        .map_err(|error| build_command_error(py, error))?;
    apply_steps(py, steps, streams)
}

/// Apply every plan safety gate before a build executes anything.
#[pyfunction]
#[pyo3(name = "enforce_plan_safety_policies", signature = (**options))]
pub(crate) fn enforce_plan_safety_policies_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "enforce_plan_safety_policies", options, POLICY_OPTIONS)?;
    let streams: (Bound<'py, PyAny>, Bound<'py, PyAny>) = arguments.value("streams")?.extract()?;
    enforce_plan_policies(
        &arguments.value("plan")?,
        (
            &read_gates(&arguments.value("gates")?)?,
            &arguments.value("snapshots_config")?,
        ),
        &model_limit(
            &arguments.value("target_name")?,
            &arguments.value("execution_limits")?,
        )?,
        (&streams.0, &streams.1),
    )
}

/// Reject a build whose expanded model count exceeds its target policy.
#[pyfunction]
#[pyo3(name = "enforce_model_execution_limit", signature = (**options))]
pub(crate) fn enforce_model_execution_limit_binding<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "enforce_model_execution_limit", options, LIMIT_OPTIONS)?;
    let note: String = arguments.value("refusal_note")?.extract()?;
    let limit: ModelLimit = model_limit(
        &arguments.value("target_name")?,
        &arguments.value("limits")?,
    )?;
    match model_limit_error(arguments.value("model_count")?.extract()?, &limit, &note) {
        Some(error) => Err(build_command_error(py, error)),
        None => Ok(()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        enforce_plan_safety_policies_binding,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        enforce_model_execution_limit_binding,
        module
    )?)?;
    Ok(())
}
