//! The jobs and groups this server's pool caps lower, as one health observation of the current snapshot.

use uuid::Uuid;

use crate::declarations::models::JobRecord;
use crate::platform::models::{
    LocationHealth, LocationHealthObservation, LocationHealthReport, PoolCapWarning,
};
use crate::runs::models::RunKind;
use crate::snapshots::models::SnapshotRecord;

/// Each clamped pool of `jobs`, in job then pool order.
pub(crate) fn pool_cap_warnings(jobs: &[JobRecord]) -> Vec<PoolCapWarning> {
    let mut warnings: Vec<PoolCapWarning> = Vec::new();
    for job in jobs {
        for pool in &job.pools {
            let Some(cap) = pool.cap else {
                continue;
            };
            if !pool.clamped {
                continue;
            }
            let noun = match job.kind {
                RunKind::Group => "job group",
                RunKind::Job | RunKind::Adhoc => "job",
            };
            warnings.push(PoolCapWarning {
                job_key: job.job_key.clone(),
                kind: job.kind,
                pool: pool.pool.clone(),
                amount: pool.amount,
                cap,
                message: format!(
                    "{} {} takes {} of pool {}; this server caps it at {cap}, so it runs alone \
                     in that pool",
                    noun, job.job_key, pool.amount, pool.pool
                ),
            });
        }
    }
    warnings
}

/// The warnings as an observation of `snapshot`, received when it was published; `None` without any.
pub(crate) fn pool_cap_observation(
    snapshot: &SnapshotRecord,
    warnings: Vec<PoolCapWarning>,
) -> Option<LocationHealthObservation> {
    if warnings.is_empty() {
        return None;
    }
    Some(LocationHealthObservation {
        received_at: snapshot.published_at,
        report: LocationHealthReport {
            event_id: snapshot.snapshot_id,
            session_id: Uuid::nil(),
            target: snapshot.target.clone(),
            health: LocationHealth::PoolCapped {
                generation: snapshot.generation,
                jobs: warnings,
            },
        },
    })
}
