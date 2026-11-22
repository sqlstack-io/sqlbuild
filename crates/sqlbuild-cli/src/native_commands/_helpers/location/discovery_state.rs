//! What a location's compiler task keeps of its discovered sets between publications.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredSetCheck, DiscoveredSetChecked, DiscoveryPublishRequest,
};
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use tokio::time::Instant;

use crate::native_commands::models::LocationLoad;

/// The sets the location last published, where their functions live, and when each is next checked.
#[derive(Debug, Default)]
pub(crate) struct LocationDiscovery {
    sets: DiscoveredSets,
    files: BTreeMap<String, String>,
    /// Each set's next `check_every` check.
    due: BTreeMap<String, Instant>,
    /// Whether this location has published since it started; its first publication discovers everything.
    published: bool,
    /// The `rediscovers` count the last publication satisfied.
    rediscovers: u64,
    /// The `rediscovers` count the newest request asked for.
    requested: u64,
    /// The functions edited since the last publication.
    edited: BTreeSet<String>,
}

impl LocationDiscovery {
    /// The request for `generation`: everything first or on request, else only edited functions.
    pub(crate) fn publish_request(
        &mut self,
        generation: i64,
        (before, after): (&[PathStamp], &[PathStamp]),
        rediscovers: u64,
    ) -> DiscoveryPublishRequest {
        let everything: bool = !self.published || rediscovers != self.rediscovers;
        self.requested = rediscovers;
        let changed: BTreeSet<&str> = changed_paths(before, after);
        for (name, file) in &self.files {
            if changed.contains(file.as_str()) {
                self.edited.insert(name.clone());
            }
        }
        let rediscover: Option<BTreeSet<String>> = (!everything).then(|| self.edited.clone());
        DiscoveryPublishRequest {
            generation,
            rediscover,
            previous: self.sets.clone(),
            seed: !self.published,
        }
    }

    /// Keep what a publication's compile used, and schedule each set's next check from `now`.
    pub(crate) fn published(&mut self, load: &LocationLoad, now: Instant) {
        if load.published.is_some() {
            self.published = true;
            self.rediscovers = self.requested;
            self.edited.clear();
            self.files.clone_from(&load.discover_files);
        }
        if load.discovered.is_empty() && load.published.is_none() {
            return;
        }
        let mut refetched: BTreeSet<String> = BTreeSet::new();
        for (name, set) in &load.discovered {
            let before = self.sets.get(name).map(|before| before.fetched_at);
            if before != Some(set.fetched_at) {
                refetched.insert(name.clone());
            }
        }
        self.sets.clone_from(&load.discovered);
        let mut due: BTreeMap<String, Instant> = BTreeMap::new();
        for (name, set) in &self.sets {
            let Some(seconds) = set.check_every_seconds else {
                continue;
            };
            let next: Instant = match self.due.get(name) {
                Some(next) if !refetched.contains(name) => *next,
                _ => now + Duration::from_secs(seconds),
            };
            due.insert(name.clone(), next);
        }
        self.due = due;
    }

    /// Whether to report `load`'s sets: on the first publication, with sets, or when the last one went.
    pub(crate) fn reports(&self, load: &LocationLoad) -> bool {
        load.published.is_some()
            && (!self.published || !load.discovered.is_empty() || !self.sets.is_empty())
    }

    /// When the next check is due; `None` when no set is checked.
    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.due.values().min().copied()
    }

    /// The check of the set due soonest, if it is due at `now`.
    pub(crate) fn due_check(&self, now: Instant) -> Option<DiscoveredSetCheck> {
        let (name, _) = self
            .due
            .iter()
            .filter(|(_, at)| **at <= now)
            .min_by_key(|(_, at)| **at)?;
        Some(DiscoveredSetCheck {
            check_discovered: name.clone(),
            previous: self.sets.get(name).cloned(),
        })
    }

    /// Keep a changed result for the next publication; true when the location should republish.
    pub(crate) fn checked(
        &mut self,
        name: &str,
        reply: DiscoveredSetChecked,
        now: Instant,
    ) -> bool {
        let every: Option<u64> = self.sets.get(name).and_then(|set| set.check_every_seconds);
        match every {
            Some(seconds) => {
                self.due
                    .insert(name.to_owned(), now + Duration::from_secs(seconds));
            }
            None => {
                self.due.remove(name);
            }
        }
        match reply.set {
            Some(set) if reply.changed => {
                self.sets.insert(name.to_owned(), set);
                true
            }
            _ => false,
        }
    }
}

/// The relative paths whose stamps differ between `before` and `after`, added and removed ones too.
fn changed_paths<'a>(before: &'a [PathStamp], after: &'a [PathStamp]) -> BTreeSet<&'a str> {
    let index = |stamps: &'a [PathStamp]| -> BTreeMap<&'a str, &'a PathStamp> {
        stamps
            .iter()
            .map(|stamp| (stamp.relative_path.as_str(), stamp))
            .collect()
    };
    let (before, after) = (index(before), index(after));
    before
        .keys()
        .chain(after.keys())
        .copied()
        .filter(|path| before.get(path) != after.get(path))
        .collect()
}
