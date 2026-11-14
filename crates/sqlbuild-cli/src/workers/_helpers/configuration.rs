//! Reject ambiguous or nonexistent installation-owned launcher scopes before claiming any run.

use std::collections::BTreeSet;

use sqlbuild_orchestration::credentials::main::locations::location_named::location_named;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::workers::models::{Launcher, LauncherScope, Launchers};

impl<'a> Launcher<'a> {
    pub(in crate::workers) fn scope(self) -> LauncherScope<'a> {
        match self {
            Self::Local(location) => LauncherScope {
                location_id: location.location_id,
                target: &location.target,
                server_url: &location.server_url,
                max_workers: location.max_workers,
            },
            Self::Docker(location) => LauncherScope {
                location_id: location.location_id,
                target: &location.target,
                server_url: &location.server_url,
                max_workers: location.max_workers,
            },
        }
    }
}

impl Launchers {
    pub(in crate::workers) fn locations(&self) -> impl Iterator<Item = Launcher<'_>> {
        self.local
            .iter()
            .map(Launcher::Local)
            .chain(self.docker.iter().map(Launcher::Docker))
    }

    pub(in crate::workers) fn dispatch_scopes(&self) -> BTreeSet<(Uuid, String)> {
        self.locations()
            .map(|location| {
                let scope = location.scope();
                (scope.location_id, scope.target.to_owned())
            })
            .collect()
    }
}

/// Each launcher names one existing location; the server reads them once, so a new location needs a restart.
pub(in crate::workers) async fn resolve_locations(
    store: &ControlStore,
    configuration: &mut Launchers,
) -> Result<(), ControlError> {
    let names = configuration
        .local
        .iter_mut()
        .map(|launcher| (&launcher.location, &mut launcher.location_id))
        .chain(
            configuration
                .docker
                .iter_mut()
                .map(|launcher| (&launcher.location, &mut launcher.location_id)),
        );
    for (name, location_id) in names {
        *location_id = location_named(store, name).await?.ok_or_else(|| {
            ControlError::Invalid(format!(
                "launcher location '{name}' does not exist; create it with \
                 `sqb admin create-location {name}`, then restart the server"
            ))
        })?;
    }
    Ok(())
}

pub(in crate::workers) fn validate_configuration(
    configuration: &Launchers,
    scheduler_enabled: bool,
) -> Result<(), ControlError> {
    if !scheduler_enabled && configuration.locations().next().is_some() {
        return Err(ControlError::Invalid(
            "launchers require the embedded scheduler".into(),
        ));
    }
    for location in &configuration.local {
        if !location.project_dir.is_absolute()
            || !location.project_dir.is_dir()
            || !location.executable.is_absolute()
            || !location.executable.is_file()
        {
            return Err(ControlError::Invalid("local launchers require unique location/target scopes and existing absolute project/executable paths".into()));
        }
    }
    let mut scopes: BTreeSet<(&str, &str)> = BTreeSet::new();
    let named = configuration
        .local
        .iter()
        .map(|launcher| (launcher.location.as_str(), launcher.target.as_str()))
        .chain(
            configuration
                .docker
                .iter()
                .map(|launcher| (launcher.location.as_str(), launcher.target.as_str())),
        );
    for (name, target) in named {
        if name.trim().is_empty() || target.trim().is_empty() || !scopes.insert((name, target)) {
            return Err(ControlError::Invalid(
                "launchers require a location name and unique location/target scopes".into(),
            ));
        }
    }
    for location in &configuration.docker {
        if !location.socket.is_absolute()
            || location.location_container.trim().is_empty()
            || location.executable.trim().is_empty()
        {
            return Err(ControlError::Invalid("Docker launchers require an absolute socket path, location container and executable".into()));
        }
        let run = &location.run;
        if run.cpus.is_some_and(|cpus| !cpus.is_finite() || cpus <= 0.0)
            || run.pids.is_some_and(|pids| pids <= 0)
            || run.cgroup_parent.as_deref().is_some_and(|path| path.trim().is_empty())
        {
            return Err(ControlError::Invalid(format!(
                "launcher location '{}' run settings require positive cpus and pids and a non-blank cgroup_parent",
                location.location
            )));
        }
        if let (Some(memory), Some(ceiling)) = (run.memory, run.max_memory)
            && memory > ceiling
        {
            return Err(ControlError::Invalid(format!(
                "launcher location '{}' run memory exceeds its max_memory",
                location.location
            )));
        }
    }
    Ok(())
}
