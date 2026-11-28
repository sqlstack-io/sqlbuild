//! People read a target's health only with view on every current node, like its listings; the server decides its verdict and its sync's state.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::main::current_jobs::current_jobs;
use crate::declarations::models::ServerPoolCaps;
use crate::platform::_helpers::health::failing_syncs::sync_state;
use crate::platform::_helpers::health::pool_cap_warnings::{
    pool_cap_observation, pool_cap_warnings,
};
use crate::platform::_helpers::health::problems::location_problems;
use crate::platform::_helpers::health::recent_health::recent_health;
use crate::platform::constants::LOCATION_RETENTION_DAYS;
use crate::platform::models::{
    LocationState, LocationVerdict, PersonLocationHealth, SyncRequest, TestedAt,
};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// The target's state and its newest observations for this target only, newest first, with this server's pool-cap warning on the current snapshot.
pub async fn read_person_location_health(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    (default_role, caps): (DefaultRole, &ServerPoolCaps),
) -> Result<PersonLocationHealth, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let now = transaction.now;
    let (transaction, mut observations) =
        recent_health(transaction, location_id, Some(target)).await?;
    let state = LocationState::of(&observations, (target, now));
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let mut warehouse_sync = sync_state(&observations, (target, now));
    let (transaction, requested) = pending_sync(transaction, (location_id, target)).await?;
    warehouse_sync.requested = requested;
    let (transaction, tested_at) = tested_at(transaction, (location_id, target)).await?;
    // A location checks its connection only when it starts and publishes, so a version older than the health history has had its check pruned, not skipped.
    let tested_at = tested_at.or_else(|| {
        snapshot
            .as_ref()
            .map(|snapshot| snapshot.published_at)
            .filter(|at| now - *at > TimeDelta::days(LOCATION_RETENTION_DAYS))
    });
    let (transaction, problems) =
        location_problems(transaction, Some((location_id, target))).await?;
    let verdict = LocationVerdict::of(
        (
            state,
            snapshot.as_ref().map(|snapshot| snapshot.published_at),
        ),
        (&warehouse_sync, tested_at),
        &problems.facts(),
    );
    let (transaction, jobs) = current_jobs(transaction, (location_id, target), caps).await?;
    commit(transaction).await?;
    if let Some(warning) =
        snapshot.and_then(|snapshot| pool_cap_observation(&snapshot, pool_cap_warnings(&jobs)))
    {
        observations.push(warning);
        observations.sort_by_key(|observation| std::cmp::Reverse(observation.received_at));
    }
    Ok(PersonLocationHealth {
        state,
        verdict,
        warehouse_sync,
        observations,
    })
}

/// The newest sync someone asked for that the location hasn't started, while the request lasts, naming who asked.
async fn pending_sync(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<SyncRequest>), ControlError> {
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT w.requested_at, COALESCE(p.name, w.requested_by) AS requested_by FROM sqb_location_work w \
         LEFT JOIN sqb_principals p ON 'principal:' || p.principal_id = w.requested_by \
         WHERE w.location_id=$1 AND w.target=$2 AND w.completed_at IS NULL AND w.expires_at>$3 \
         AND w.request_json->'work'->>'kind'='sync_warehouse' ORDER BY w.requested_at DESC LIMIT 1",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.to_owned()),
            Parameter::Time(now),
        ],
    )
    .await?;
    let request: Option<SyncRequest> = rows.into_iter().next().map(decode_row).transpose()?;
    Ok((transaction, request))
}

/// When the target last reported a warehouse connection check, within the health retention window.
async fn tested_at(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<DateTime<Utc>>), ControlError> {
    let today = transaction.now.date_naive();
    let days: Vec<Parameter> = (0..=LOCATION_RETENTION_DAYS)
        .map(|day| {
            Parameter::Text(format!(
                "location/{location_id}/{}",
                today - TimeDelta::days(day)
            ))
        })
        .collect();
    let count = days.len();
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT MAX(occurred_at) AS tested_at FROM recall_events WHERE invocation_id IN ({}) \
             AND event_type='location_health' AND payload->'report'->>'target'=${} \
             AND payload->'report'->'health'->>'kind'='connection'",
            Parameter::placeholders(1..=count),
            count + 1
        ),
        days.into_iter()
            .chain([Parameter::Text(target.to_owned())])
            .collect(),
    )
    .await?;
    let tested: Option<TestedAt> = rows.into_iter().next().map(decode_row).transpose()?;
    Ok((transaction, tested.and_then(|tested| tested.tested_at)))
}
