//! People see the scheduler's state, and broken locations and late ticks only for targets they list.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::person_lists_target::person_lists_target;
use crate::credentials::models::DefaultRole;
use crate::platform::_helpers::problems::{late_ticks, scan_problems};
use crate::platform::models::PlatformProblems;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Access is checked per target before that target's late ticks are read, so one target cannot crowd out another.
pub async fn read_person_problems(
    store: &ControlStore,
    principal_id: Uuid,
    default_role: DefaultRole,
) -> Result<PlatformProblems, ControlError> {
    let scan = scan_problems(store).await?;
    let mut listed: BTreeSet<(Uuid, String)> = BTreeSet::new();
    for (location_id, target) in scan
        .broken
        .iter()
        .map(|location| (location.location_id, location.target.clone()))
        .chain(scan.late_targets.iter().cloned())
        .collect::<BTreeSet<_>>()
    {
        if person_lists_target(store, principal_id, (location_id, &target), default_role).await? {
            listed.insert((location_id, target));
        }
    }
    Ok(PlatformProblems {
        scheduler_leader: scan
            .lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at > scan.now),
        scheduler_lease_expires_at: scan.lease.map(|lease| lease.expires_at),
        overdue_ticks: late_ticks(store, scan.late_targets.intersection(&listed), scan.now).await?,
        broken_locations: scan
            .broken
            .into_iter()
            .filter(|location| listed.contains(&(location.location_id, location.target.clone())))
            .collect(),
    })
}
