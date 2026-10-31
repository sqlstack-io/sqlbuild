//! Completion-order safety for persistent parent claims and atomic, temporary child claims.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::runs::models::{ConcurrencyResource, GroupCapacity, QueueCandidate, RunRequest};
use crate::scheduler::models::Overlap;

pub(crate) fn parent_resources(request: &RunRequest) -> BTreeSet<ConcurrencyResource> {
    let mut resources: BTreeSet<_> = request
        .concurrency_groups
        .iter()
        .cloned()
        .map(ConcurrencyResource::Limit)
        .collect();
    if let Some(key) = &request.job_key {
        resources.insert(ConcurrencyResource::Overlap(
            request.location_id,
            request.target.clone(),
            key.clone(),
        ));
    }
    resources
}

pub(crate) fn claim_resources(
    request: &RunRequest,
    inherited: Option<&BTreeSet<ConcurrencyResource>>,
) -> BTreeSet<ConcurrencyResource> {
    parent_resources(request)
        .into_iter()
        .filter(|resource| match resource {
            ConcurrencyResource::Limit(_) => inherited.is_none_or(|held| !held.contains(resource)),
            ConcurrencyResource::Overlap(..) => request.overlap != Overlap::Allow,
        })
        .collect()
}

impl GroupCapacity {
    /// Find a completion order without reserving children's independent limits.
    pub(crate) fn safe(&self, candidate: &RunRequest) -> bool {
        let mut work = self.available.clone();
        let mut held = self.held.clone();
        let resources = parent_resources(candidate);
        for resource in &resources {
            *work.entry(resource.clone()).or_insert(1) -= 1;
        }
        held.insert(candidate.run_id, resources);
        while !held.is_empty() {
            let before = held.len();
            held.retain(|id, resources| {
                if self.needed[id]
                    .iter()
                    .any(|resource| work.get(resource).copied().unwrap_or(1) <= 0)
                {
                    return true;
                }
                for resource in resources.iter() {
                    *work.entry(resource.clone()).or_insert(0) += 1;
                }
                false
            });
            if held.len() == before {
                return false;
            }
        }
        true
    }

    pub(crate) fn blocks_claim(
        &self,
        candidate: &QueueCandidate,
        waiting: &BTreeSet<ConcurrencyResource>,
    ) -> bool {
        let inherited = candidate
            .request_json
            .parent_run_id
            .and_then(|id| self.held.get(&id));
        let mut consumed = claim_resources(&candidate.request_json, inherited);
        consumed.extend(
            parent_resources(&candidate.request_json)
                .into_iter()
                .filter(|resource| matches!(resource, ConcurrencyResource::Overlap(..))),
        );
        !consumed.is_disjoint(waiting)
    }

    /// Only temporary blockers drain ahead of younger work; persistent blockers must let owners finish.
    pub(crate) fn record_waiter(
        &self,
        candidate: &RunRequest,
        mut waiting: BTreeSet<ConcurrencyResource>,
    ) -> BTreeSet<ConcurrencyResource> {
        let inherited = candidate.parent_run_id.and_then(|id| self.held.get(&id));
        let resources = claim_resources(candidate, inherited);
        if resources.iter().all(|resource| {
            self.available
                .get(resource)
                .is_none_or(|available| *available > 0)
        }) {
            waiting.extend(resources);
        }
        waiting
    }

    pub(crate) fn earlier_waiters<'a>(
        &'a self,
        candidate: &'a QueueCandidate,
    ) -> impl Iterator<Item = &'a QueueCandidate> {
        let rank = |value: &QueueCandidate| {
            (
                Reverse(value.request_json.priority),
                value.queued_at,
                value.request_json.run_id,
            )
        };
        self.waiting
            .iter()
            .filter(move |older| rank(older) < rank(candidate))
    }
}
