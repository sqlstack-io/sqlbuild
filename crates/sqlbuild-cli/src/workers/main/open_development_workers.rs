//! Development uses the ordinary local launcher without writing derived launcher configuration.

use std::path::PathBuf;

use sqlbuild_orchestration::store::errors::ControlError;
use uuid::Uuid;

use crate::workers::models::{Launchers, LocalLauncher, Workers};

pub(crate) fn open_development_workers(
    (location_id, target): (Uuid, String),
    project_dir: PathBuf,
    executable: PathBuf,
    server_url: String,
) -> Result<Workers, ControlError> {
    Ok(Workers {
        configuration: Launchers {
            local: vec![LocalLauncher {
                location_id,
                target,
                server_url,
                max_workers: std::thread::available_parallelism()?,
                project_dir,
                executable,
            }],
            docker: Vec::new(),
            concurrency_limits: Default::default(),
        },
        active: Vec::new(),
        control_variable: "SQLBUILD_CONTROL_DATABASE_URL".into(),
        recovered_epoch: None,
    })
}
