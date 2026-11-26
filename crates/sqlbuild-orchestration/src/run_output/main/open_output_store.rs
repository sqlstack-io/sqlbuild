//! Open the output store `server.toml` names, or the default folder for the control store.

use std::path::PathBuf;
use std::sync::Arc;

use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path;

use crate::run_output::_helpers::folders::{default_folder, user_folders};
use crate::run_output::constants::{FILE_SCHEME, S3_SCHEME};
use crate::run_output::models::{OutputSettings, OutputStore};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::tls::main::install_crypto_provider::install_crypto_provider;

/// A local folder is created when missing; an S3 store takes its credentials from the standard AWS environment or role.
pub fn open_output_store(
    settings: &OutputSettings,
    control: &ControlStore,
) -> Result<OutputStore, ControlError> {
    let mut location: String = match &settings.store {
        Some(location) => location.clone(),
        None => format!(
            "{FILE_SCHEME}{}",
            default_folder(control, user_folders())?.display()
        ),
    };
    let (objects, prefix): (Arc<dyn ObjectStore>, Path) =
        if let Some(folder) = location.strip_prefix(FILE_SCHEME) {
            std::fs::create_dir_all(folder)?;
            let resolved: PathBuf = std::path::absolute(folder)?;
            location = format!("{FILE_SCHEME}{}", resolved.display());
            (
                Arc::new(LocalFileSystem::new_with_prefix(resolved)?),
                Path::default(),
            )
        } else if let Some(rest) = location.strip_prefix(S3_SCHEME) {
            install_crypto_provider();
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
