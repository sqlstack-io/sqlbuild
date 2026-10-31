//! The privileged role talks to a local Docker daemon; workers never receive its socket.

use std::path::Path;

use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use sqlbuild_orchestration::runs::models::RunClaim;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::workers::_helpers::invocation::{arguments, environment, excluded};
use crate::workers::constants::IMAGE_ID_HEX_LENGTH;
use crate::workers::models::{DockerLauncher, DockerWorker, Launcher, WorkerExit};

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
        location: &DockerLauncher,
        claim: &RunClaim,
        bearer: &str,
        control_variable: &str,
    ) -> Result<(), ControlError> {
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
            "HostConfig": {
                "NetworkMode": format!("container:{}", location.location_container),
                "VolumesFrom": [location.location_container],
                "Memory": parent["HostConfig"]["Memory"], "MemorySwap": parent["HostConfig"]["MemorySwap"],
                "NanoCpus": parent["HostConfig"]["NanoCpus"], "PidsLimit": parent["HostConfig"]["PidsLimit"],
                "ReadonlyRootfs": parent["HostConfig"]["ReadonlyRootfs"],
            },
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
