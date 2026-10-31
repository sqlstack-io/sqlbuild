//! Reject ambiguous or nonexistent installation-owned launcher scopes before claiming any run.

use std::collections::BTreeSet;

use sqlbuild_orchestration::store::errors::ControlError;
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
}

pub(in crate::workers) fn read_configuration(
    path: Option<&str>,
    scheduler_enabled: bool,
) -> Result<Launchers, ControlError> {
    let configuration: Launchers = match path {
        Some(path) => serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| ControlError::Invalid(format!("launcher configuration: {error}")))?,
        None => Launchers {
            local: Vec::new(),
            docker: Vec::new(),
            concurrency_limits: Default::default(),
        },
    };
    if !scheduler_enabled && configuration.locations().next().is_some() {
        return Err(ControlError::Invalid(
            "launchers require the embedded scheduler".into(),
        ));
    }
    let mut scopes: BTreeSet<(Uuid, &str)> = BTreeSet::new();
    for location in &configuration.local {
        if !location.project_dir.is_absolute()
            || !location.project_dir.is_dir()
            || !location.executable.is_absolute()
            || !location.executable.is_file()
        {
            return Err(ControlError::Invalid("local launchers require unique location/target scopes and existing absolute project/executable paths".into()));
        }
    }
    for location in configuration.locations() {
        let scope = location.scope();
        if scope.target.trim().is_empty() || !scopes.insert((scope.location_id, scope.target)) {
            return Err(ControlError::Invalid(
                "launchers require unique location/target scopes".into(),
            ));
        }
        if let Launcher::Docker(location) = location
            && (!location.socket.is_absolute()
                || location.location_container.trim().is_empty()
                || location.executable.trim().is_empty())
        {
            return Err(ControlError::Invalid("Docker launchers require an absolute socket path, location container and executable".into()));
        }
    }
    Ok(configuration)
}
