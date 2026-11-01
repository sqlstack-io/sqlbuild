//! Pin Docker publications to physical image evidence held by the privileged server.

use axum::http::StatusCode;

#[cfg(unix)]
use crate::api::constants::DOCKER_IMAGE_HEX_LENGTH;
use crate::api::errors::ApiError;
use crate::api::models::DockerImageSource;
use crate::snapshots::models::SnapshotInput;

pub(crate) async fn pin(
    sources: &[DockerImageSource],
    mut input: SnapshotInput,
) -> Result<SnapshotInput, ApiError> {
    let Some(source) = sources
        .iter()
        .find(|source| source.location_id == input.location_id && source.target == input.target)
    else {
        return Ok(input);
    };
    let image = inspect(source).await?;
    if input
        .image_id
        .as_ref()
        .is_some_and(|claimed| *claimed != image)
    {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "published image differs from the configured Docker location image".into(),
        });
    }
    input.image_id = Some(image);
    Ok(input)
}

#[cfg(unix)]
async fn inspect(source: &DockerImageSource) -> Result<String, ApiError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(source.socket.as_path())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| unavailable())?;
    let response: serde_json::Value = client
        .get(format!(
            "http://localhost/containers/{}/json",
            source.container
        ))
        .send()
        .await
        .map_err(|_| unavailable())?
        .error_for_status()
        .map_err(|_| unavailable())?
        .json()
        .await
        .map_err(|_| unavailable())?;
    let image = response["Image"].as_str().ok_or_else(unavailable)?;
    let digest = image.strip_prefix("sha256:").ok_or_else(unavailable)?;
    (digest.len() == DOCKER_IMAGE_HEX_LENGTH && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| image.to_owned())
        .ok_or_else(unavailable)
}

#[cfg(not(unix))]
async fn inspect(_source: &DockerImageSource) -> Result<String, ApiError> {
    Err(unavailable())
}

fn unavailable() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "cannot inspect the configured Docker location image".into(),
    }
}
