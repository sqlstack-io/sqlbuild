//! Derive publication image sources from the same privileged Docker launcher configuration.

use sqlbuild_orchestration::api::models::DockerImageSource;

use crate::workers::models::Workers;

pub(crate) fn docker_image_sources(workers: &Workers) -> Vec<DockerImageSource> {
    workers
        .configuration
        .docker
        .iter()
        .map(|location| DockerImageSource {
            location_id: location.location_id,
            target: location.target.clone(),
            socket: location.socket.clone(),
            container: location.location_container.clone(),
        })
        .collect()
}
