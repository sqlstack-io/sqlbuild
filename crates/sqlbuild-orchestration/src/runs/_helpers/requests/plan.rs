//! A plan is frozen like an ad-hoc build but queued as location work, so it never becomes a run.

use chrono::Duration;
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::platform::constants::{LOCATION_SILENCE_SECONDS, PLAN_EXPIRY_SECONDS, PLAN_QUEUE_LIMIT};
use crate::platform::main::enqueue_location_work::enqueue_location_work;
use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkRequest, PlanRequest};
use crate::runs::_helpers::requests::adhoc::frozen_adhoc_request;
use crate::runs::constants::PLAN_REASON;
use crate::runs::models::{AdhocCheckRequest, RunCommand};
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Needs view on every current node; a refused command, a silent location or a full queue queues nothing.
pub(in crate::runs) async fn request_plan_in(
    transaction: ControlTransaction,
    (context, principal_id, planner): (&AuditContext, Uuid, &dyn CommandPlanner),
    (location_id, target, request): (Uuid, &str, &PlanRequest),
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, context) = authorize_listing(
        transaction,
        (context, principal_id),
        (location_id, target),
        DefaultRole::None,
    )
    .await?;
    let (transaction, previous) = read_location_work(transaction, request.request_id).await?;
    if let Some(previous) = previous {
        let same = previous.record.requested_by == context.actor
            && previous.record.request_json.location_id == location_id
            && previous.record.request_json.target == target
            && matches!(&previous.record.request_json.work,
                    LocationWork::Plan { command, .. } if *command == request.command);
        if !same {
            return Err(ControlError::Invalid(
                "location work ID already has different content or actor".into(),
            ));
        }
        return Ok((transaction, false));
    }
    let check = AdhocCheckRequest {
        location_id,
        target: target.to_owned(),
        command: request.command.clone(),
        priority: 0,
    };
    let (transaction, frozen) = frozen_adhoc_request(
        transaction,
        (&context, planner),
        (&check, request.request_id),
    )
    .await?;
    let (verb, frozen) = frozen.map_err(ControlError::Invalid)?;
    if frozen.command != RunCommand::Build {
        return Err(ControlError::Invalid(format!(
            "sqb plan previews builds; sqb {verb} cannot be planned"
        )));
    }
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
            "SELECT \
             (SELECT COUNT(*) FROM sqb_location_instances i WHERE i.location_id=$1 AND i.target=$2 AND i.last_seen_at>=$3) AS live, \
             (SELECT COUNT(*) FROM sqb_location_work w WHERE w.location_id=$1 AND w.target=$2 \
              AND w.completed_at IS NULL AND w.expires_at>$4 AND w.request_json->'work'->>'kind'='plan') AS queued, \
             (SELECT COUNT(*) FROM sqb_location_work w WHERE w.location_id=$1 AND w.target=$2 \
              AND w.completed_at IS NULL AND w.expires_at>$4 AND w.request_json->'work'->>'kind'='plan' \
              AND w.requested_by=$5) AS mine",
            vec![Parameter::Id(location_id), Parameter::Text(target.to_owned()),
                Parameter::Time(now - Duration::seconds(LOCATION_SILENCE_SECONDS)), Parameter::Time(now),
                Parameter::Text(context.actor.clone())]).await?;
    let count = |name: &str| rows.first().and_then(|row| row[name].as_i64()).unwrap_or(0);
    if count("live") == 0 {
        return Err(ControlError::Invalid(format!(
            "no sqb location serve has reported for {target} in the last {LOCATION_SILENCE_SECONDS} seconds; start it and plan again"
        )));
    }
    if count("mine") > 0 {
        return Err(ControlError::Invalid(
            "your previous plan for this target is still running".into(),
        ));
    }
    if count("queued") >= PLAN_QUEUE_LIMIT {
        return Err(ControlError::Invalid(format!(
            "{PLAN_QUEUE_LIMIT} plans are already waiting for this target; try again shortly"
        )));
    }
    let work = LocationWorkRequest {
        request_id: request.request_id,
        location_id,
        target: target.to_owned(),
        reason: PLAN_REASON.into(),
        work: LocationWork::Plan {
            command: request.command.clone(),
            selection: frozen.selection,
            settings: frozen.settings,
        },
    };
    let transaction = enqueue_location_work(
        transaction,
        &context,
        &work,
        Duration::seconds(PLAN_EXPIRY_SECONDS),
    )
    .await?;
    Ok((transaction, true))
}
