//! A requested plan runs in the location's runtime interpreter on frozen node keys, from the synced copy.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::PreviewCopyRead;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::native_commands::_helpers::location::execution::{plan_failure, without_diffs};
use crate::native_commands::_helpers::location::held_copy::{HELD_COPY, held_copy, hold_copy};
use crate::native_commands::_helpers::location::plan_request::{
    plan_arguments, run_plan, selected_names,
};
use crate::native_commands::_helpers::location::plan_reuse::{PLAN_REUSE, reused_plan};
use crate::native_commands::models::{
    HeldCopy, LocationRuntimeRequest, LocationServe, PlanKey, PlanToKeep,
};
use crate::native_commands::types::LocationPlan;

/// The plan with what to keep once its checks pass, an `{"error": ...}` report, or `None` when stopped first.
pub(super) async fn plan_location(
    (options, runtime): (&LocationServe, &mpsc::Sender<LocationRuntimeRequest>),
    (selection, settings, fresh): (&BTreeSet<String>, &BTreeMap<String, Value>, bool),
    (client, session_id, generation): (&LocationWorkClient, Uuid, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationPlan, Option<PlanToKeep>)>, ControlError> {
    let (argv, select) = match plan_arguments(options, settings)
        .and_then(|argv| Ok((argv, selected_names(selection)?)))
    {
        Ok(planned) => planned,
        Err(message) => return Ok(Some((without_diffs(plan_failure(&message)), None))),
    };
    let consumption: bool = settings
        .get("changed")
        .is_some_and(|changed| !matches!(changed, Value::Null | Value::Bool(false)));
    let held: Option<HeldCopy> = held_copy(&HELD_COPY, (&options.target, generation), consumption);
    let read_consumption: bool = consumption || held.as_ref().is_some_and(|held| held.consumption);
    let read = PreviewCopyRead {
        session_id,
        generation,
        consumption: read_consumption,
        known_version: held.as_ref().map(|held| held.version.clone()),
    };
    let copy = match client.preview_copy(&options.target, &read).await {
        Ok(copy) => copy,
        Err(error) => {
            return Ok(Some((
                without_diffs(plan_failure(&format!(
                    "the synced warehouse copy is unavailable: {error}"
                ))),
                None,
            )));
        }
    };
    let planned: HeldCopy = match held {
        Some(held) if copy.unchanged => held,
        _ => hold_copy(&HELD_COPY, options, generation, (copy, read_consumption))?,
    };
    let key: PlanKey = PlanKey {
        argv: argv.clone(),
        select: select.clone(),
        consumption: planned.consumption,
    };
    let planned_at: DateTime<Utc> = Utc::now();
    if !fresh
        && let Some(reused) = reused_plan(
            &PLAN_REUSE,
            (generation, &planned.version),
            &key,
            planned_at,
        )
    {
        return Ok(Some((reused, None)));
    }
    let request = json!({"plan": {
        "argv": argv,
        "select": select,
        "copy": planned.path.to_string_lossy(),
        "version": planned.version,
        "consumption": planned.consumption,
    }});
    let keep = PlanToKeep {
        generation,
        version: planned.version,
        key,
        planned_at,
    };
    Ok(run_plan(runtime, request, stopped)
        .await?
        .map(|plan| (plan, Some(keep))))
}
