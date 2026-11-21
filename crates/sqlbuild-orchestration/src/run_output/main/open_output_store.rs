//! Open the output store `server.toml` names, or a folder beside a SQLite control store.
//! A PostgreSQL control store serves several replicas, so it needs a named store.

use std::path::PathBuf;
use std::sync::Arc;

use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path;

use crate::run_output::constants::{DEFAULT_OUTPUT_FOLDER, FILE_SCHEME, S3_SCHEME};
use crate::run_output::models::{OutputSettings, OutputStore};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// A local folder is created when missing; an S3 store takes its credentials from the standard AWS environment or role.
pub fn open_output_store(
    settings: &OutputSettings,
    control: &ControlStore,
) -> Result<OutputStore, ControlError> {
    let location: String = match &settings.store {
        Some(location) => location.clone(),
        None => format!("{FILE_SCHEME}{}", default_folder(control)?.display()),
    };
    let (objects, prefix): (Arc<dyn ObjectStore>, Path) =
        if let Some(folder) = location.strip_prefix(FILE_SCHEME) {
            std::fs::create_dir_all(folder)?;
            (
                Arc::new(LocalFileSystem::new_with_prefix(folder)?),
                Path::default(),
            )
        } else if let Some(rest) = location.strip_prefix(S3_SCHEME) {
            let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
            if bucket.is_empty() {
                return Err(ControlError::Invalid(
                    "[output] store needs a bucket, as in s3://bucket/prefix".into(),
                ));
            }
            (
                Arc::new(
                    AmazonS3Builder::from_env()
                        .with_bucket_name(bucket)
                        .build()?,
                ),
                Path::from(prefix.trim_matches('/')),
            )
        } else {
            return Err(ControlError::Invalid(format!(
                "[output] store must be a file:// folder or an s3:// bucket, not {location}"
            )));
        };
    Ok(OutputStore {
        objects,
        prefix,
        location,
        max_bytes_per_run: settings.max_bytes_per_run,
    })
}

/// `output` beside a SQLite control store; a PostgreSQL one has no folder of its own.
fn default_folder(control: &ControlStore) -> Result<PathBuf, ControlError> {
    match control {
        ControlStore::Sqlite(sqlite) => Ok(sqlite.path.parent().map_or_else(
            || PathBuf::from(DEFAULT_OUTPUT_FOLDER),
            |folder| folder.join(DEFAULT_OUTPUT_FOLDER),
        )),
        ControlStore::Postgres(_) => Err(ControlError::Invalid(
            "a PostgreSQL control store needs [output] store in server.toml: an s3:// bucket, or a file:// folder every server replica shares".into(),
        )),
    }
}
