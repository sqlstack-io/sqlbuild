//! The plan safety gates in the order they apply.

use crate::build_command::_helpers::migrations::{
    column_migration_refusal, missing_origin_confirmation, model_migration_refusal,
    old_name_refusal,
};
use crate::build_command::_helpers::storage::{retention_step, snapshot_steps, table_type_step};
use crate::build_command::errors::BuildCommandError;
use crate::build_command::main::model_limit_error::model_limit_error;
use crate::build_command::models::{
    ModelErrorFact, ModelLimit, PlanSafetyFacts, PlanSafetyGates, PolicyStep, SnapshotPolicies,
};

/// A selected model that couldn't be planned refuses the build with its own error; several are joined under the first one's code.
fn model_error_refusal(errors: &[ModelErrorFact]) -> Option<BuildCommandError> {
    let first: &ModelErrorFact = errors.first()?;
    let message: String = errors
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<&str>>()
        .join("; ");
    Some(BuildCommandError::new(
        first.code.as_deref(),
        message,
        first.help.as_deref(),
    ))
}

/// Model-error, migration, old-name, model-limit, snapshot, table-type and retention gates; a refusal ends the list.
pub fn plan_safety_steps(
    facts: &PlanSafetyFacts,
    gates: &PlanSafetyGates,
    (limit, snapshot_policies): (&ModelLimit, &SnapshotPolicies),
) -> Result<Vec<PolicyStep>, BuildCommandError> {
    let mut steps: Vec<PolicyStep> = Vec::new();
    let refusal: Option<BuildCommandError> = model_error_refusal(&facts.model_errors)
        .or_else(|| model_migration_refusal(&facts.model_migrations))
        .or_else(|| column_migration_refusal(&facts.column_migrations));
    if let Some(error) = refusal {
        steps.push(PolicyStep::Refuse(error));
        return Ok(steps);
    }
    if !gates.allow_missing_migration_origin
        && let Some(confirmation) = missing_origin_confirmation(
            (&facts.model_migrations, &facts.column_migrations),
            &gates.missing_origin_help,
        )
    {
        steps.push(PolicyStep::Confirm(confirmation));
    }
    let refusal: Option<BuildCommandError> =
        old_name_refusal(&facts.error_warnings).or_else(|| {
            model_limit_error(
                facts.executable_model_count,
                limit,
                &gates.execution_limit_note,
            )
        });
    if let Some(error) = refusal {
        steps.push(PolicyStep::Refuse(error));
        return Ok(steps);
    }
    let storage: [Option<PolicyStep>; 3] = [
        snapshot_steps(
            &facts.snapshot_refreshes,
            snapshot_policies,
            (
                gates.allow_snapshot_full_refresh,
                &gates.snapshot_full_refresh_help,
            ),
        )?,
        table_type_step(
            &facts.table_type_downgrades,
            (
                gates.allow_table_type_downgrade,
                &gates.table_type_downgrade_help,
            ),
        ),
        retention_step(
            &facts.retention_decreases,
            (
                gates.allow_retention_decrease,
                &gates.retention_decrease_help,
            ),
        ),
    ];
    for step in storage.into_iter().flatten() {
        let refused: bool = matches!(step, PolicyStep::Refuse(_));
        steps.push(step);
        if refused {
            break;
        }
    }
    Ok(steps)
}
