//! The privileged role talks to a local Docker daemon; workers never receive its socket.

use std::path::Path;

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use sqlbuild_orchestration::declarations::main::job_resources::job_resources;
use sqlbuild_orchestration::declarations::models::RunResources;
use sqlbuild_orchestration::runs::models::{RunClaim, RunKind};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::workers::_helpers::invocation::{arguments, environment, excluded};
use crate::workers::constants::IMAGE_ID_HEX_LENGTH;
use crate::workers::models::{DockerLauncher, DockerWorker, Launcher, RunDefaults, WorkerExit};

impl DockerWorker {
    pub(in crate::workers) fn prepare(
        location: &DockerLauncher,
        claim: &RunClaim,
    ) -> Result<Self, ControlError> {
        let image_id = claim.request.image_id.as_deref().unwrap_or_default();
        match image_id.strip_prefix("sha256:") {
            Some(digest)
                if digest.len() == IMAGE_ID_HEX_LENGTH
                    && digest.as_bytes().iter().all(u8::is_ascii_hexdigit) => {}
            _ => {
                return Err(ControlError::Invalid(
                    "Docker workers require a snapshot pinned to a full sha256 image ID".into(),
                ));
            }
        }
        Ok(Self {
            client: docker_client(&location.socket)?,
            name: format!(
                "sqlbuild-{}-{}",
                claim.request.location_id, claim.request.run_id
            ),
            claim_id: claim.claim_id,
            image_id: image_id.into(),
        })
    }

    pub(in crate::workers) async fn launch(
        &self,
        (store, location): (&ControlStore, &DockerLauncher),
        claim: &RunClaim,
        bearer: &str,
        control_variable: &str,
    ) -> Result<(), ControlError> {
        let resources = match (&claim.request.kind, &claim.request.job_key) {
            (RunKind::Job, Some(job_key)) => {
                job_resources(store, (claim.request.snapshot_id, job_key)).await?
            }
            _ => None,
        };
        let parent: Value = self
            .request(
                Method::GET,
                &format!("containers/{}/json", location.location_container),
                None,
            )
            .await?
            .error_for_status()
            .map_err(docker_error)?
            .json()
            .await
            .map_err(docker_error)?;
        let project = parent["Config"]["WorkingDir"]
            .as_str()
            .filter(|path| path.starts_with('/'))
            .ok_or_else(|| {
                ControlError::Invalid(
                    "location container requires an absolute working directory".into(),
                )
            })?;
        let mut command = arguments(&Launcher::Docker(location).scope(), claim);
        command.push(project.into());
        command.extend(["--image-id".into(), self.image_id.clone()]);
        let mut variables: Vec<String> = parent["Config"]["Env"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|entry| !excluded(entry.split('=').next().unwrap_or(entry), control_variable))
            .map(str::to_owned)
            .collect();
        variables.extend(
            environment(claim, bearer)
                .into_iter()
                .map(|(key, value)| format!("{key}={value}")),
        );
        let body = json!({
            "Image": self.image_id, "Entrypoint": [location.executable], "Cmd": command,
            "WorkingDir": project, "User": parent["Config"]["User"], "Env": variables,
            "Labels": {"io.sqlbuild.claim-id": self.claim_id.to_string()},
            "HostConfig": host_config(&parent, location, resources.unwrap_or_default()),
        });
        self.request(
            Method::POST,
            &format!("containers/create?name={}", self.name),
            Some(&body),
        )
        .await?
        .error_for_status()
        .map_err(docker_error)?;
        self.request(
            Method::POST,
            &format!("containers/{}/start", self.name),
            None,
        )
        .await?
        .error_for_status()
        .map_err(docker_error)?;
        Ok(())
    }

    pub(in crate::workers) async fn try_wait(&self) -> Result<Option<WorkerExit>, ControlError> {
        let response = self
            .request(Method::GET, &format!("containers/{}/json", self.name), None)
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Some(WorkerExit::Absent));
        }
        let container: Value = response
            .error_for_status()
            .map_err(docker_error)?
            .json()
            .await
            .map_err(docker_error)?;
        if container["Config"]["Labels"]["io.sqlbuild.claim-id"] != self.claim_id.to_string()
            || container["Image"] != self.image_id
        {
            return Err(ControlError::Invalid(
                "Docker container does not belong to the retained claim and image".into(),
            ));
        }
        if container["State"]["Running"].as_bool() == Some(true) {
            return Ok(None);
        }
        if container["State"]["Status"].as_str() == Some("created") {
            return Ok(Some(WorkerExit::Absent));
        }
        let code = container["State"]["ExitCode"]
            .as_i64()
            .ok_or_else(|| ControlError::Invalid("Docker container has no exit evidence".into()))?;
        Ok(Some(WorkerExit::Code(code)))
    }

    pub(in crate::workers) async fn signal(&self, signal: &str) -> Result<(), ControlError> {
        if self.try_wait().await?.is_some() {
            return Ok(());
        }
        let response = self
            .request(
                Method::POST,
                &format!("containers/{}/kill?signal={signal}", self.name),
                None,
            )
            .await?;
        if !matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::CONFLICT
        ) {
            response.error_for_status().map_err(docker_error)?;
        }
        Ok(())
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<reqwest::Response, ControlError> {
        let request = self
            .client
            .request(method, format!("http://localhost/{path}"));
        match body {
            Some(body) => request.json(body),
            None => request,
        }
        .send()
        .await
        .map_err(docker_error)
    }
}

/// Location limits and hardening, then launcher run defaults, then the job's resources; only positive limits are sent, because Podman applies a zero.
fn host_config(container: &Value, location: &DockerLauncher, job: RunResources) -> Value {
    let parent = &container["HostConfig"];
    let RunDefaults {
        memory,
        cpus,
        pids,
        cgroup_parent,
        max_memory,
    } = &location.run;
    let inherited = |key: &str| parent[key].as_u64().filter(|limit| *limit > 0);
    let own_memory = inherited("Memory");
    let memory = match (
        job.memory.or(*memory).map(|size| size.0).or(own_memory),
        max_memory,
    ) {
        (Some(bytes), Some(ceiling)) => Some(bytes.min(ceiling.0)),
        (None, Some(ceiling)) => Some(ceiling.0),
        (bytes, None) => bytes,
    };
    let swap = if memory == own_memory {
        inherited("MemorySwap")
    } else {
        memory
    };
    let nano_cpus = job
        .cpus
        .or(*cpus)
        .map(|cpus| (cpus * 1e9).round() as u64)
        .or_else(|| inherited("NanoCpus"));
    json!({
        "NetworkMode": network_mode(container),
        "VolumesFrom": [location.location_container],
        "Memory": memory, "MemorySwap": swap, "NanoCpus": nano_cpus,
        "PidsLimit": job.pids.or(*pids).or_else(|| inherited("PidsLimit")),
        "CgroupParent": cgroup_parent.as_deref().or_else(|| parent["CgroupParent"].as_str()).filter(|path| !path.is_empty()),
        "ReadonlyRootfs": parent["ReadonlyRootfs"],
        "CapDrop": parent["CapDrop"], "SecurityOpt": parent["SecurityOpt"],
    })
}

/// The location's network; Podman reports a container on a named network as `bridge`, so take the network's name instead.
fn network_mode(container: &Value) -> Value {
    let mode = &container["HostConfig"]["NetworkMode"];
    if !matches!(mode.as_str(), Some("bridge" | "default")) {
        return mode.clone();
    }
    let named: Vec<&String> = container["NetworkSettings"]["Networks"]
        .as_object()
        .into_iter()
        .flat_map(serde_json::Map::keys)
        .filter(|name| !matches!(name.as_str(), "bridge" | "podman"))
        .collect();
    match named.as_slice() {
        [name] => json!(name),
        _ => mode.clone(),
    }
}

fn docker_error(error: reqwest::Error) -> ControlError {
    ControlError::Invalid(format!("Docker daemon request failed: {error}"))
}

#[cfg(unix)]
fn docker_client(socket: &Path) -> Result<reqwest::Client, ControlError> {
    reqwest::Client::builder()
        .no_proxy()
        .unix_socket(socket)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(docker_error)
}

#[cfg(not(unix))]
fn docker_client(_socket: &Path) -> Result<reqwest::Client, ControlError> {
    Err(ControlError::Invalid(
        "Docker launchers require a Unix socket".into(),
    ))
}
