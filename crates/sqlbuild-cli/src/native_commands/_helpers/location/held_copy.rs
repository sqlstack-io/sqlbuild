//! The plan copy a location holds: one file per target, replaced by each newer copy it reads.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use sqlbuild_orchestration::platform::models::LocationWorkClient;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::{PreviewCopy, PreviewCopyRead};
use uuid::Uuid;

use crate::native_commands::models::{HeldCopy, LocationServe};

/// The location's one held copy; the runtime holds the state built from it under the same version.
pub(crate) static HELD_COPY: Mutex<Option<HeldCopy>> = Mutex::new(None);

/// The held copy of this target and generation, when it also has consumption a plan needs.
pub(crate) fn held_copy(
    held: &Mutex<Option<HeldCopy>>,
    (target, generation): (&str, i64),
    consumption: bool,
) -> Option<HeldCopy> {
    held.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .filter(|held| {
            held.target == target
                && held.generation == generation
                && (held.consumption || !consumption)
        })
}

/// A versioned read; a server without mirror versions rejects the fields, so it is read unversioned.
pub(crate) async fn read_copy(
    client: &LocationWorkClient,
    target: &str,
    read: PreviewCopyRead,
) -> Result<PreviewCopy, String> {
    match client.preview_copy(target, &read).await {
        Ok(copy) => Ok(copy),
        Err(_) => client
            .preview_copy(
                target,
                &PreviewCopyRead {
                    versioned: false,
                    known_version: None,
                    ..read
                },
            )
            .await
            .map_err(|error| error.to_string()),
    }
}

/// The location's one held copy file for its target, which each newer copy overwrites.
fn held_copy_path(options: &LocationServe) -> PathBuf {
    let target: String = options
        .target
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!(
        "sqlbuild-plan-copy-{}-{target}.json",
        options.location_id
    ))
}

/// Atomically replace the held copy file; a copy without a version gets a fresh one.
pub(crate) fn hold_copy(
    held_copies: &Mutex<Option<HeldCopy>>,
    options: &LocationServe,
    generation: i64,
    (copy, consumption): (PreviewCopy, bool),
) -> Result<HeldCopy, ControlError> {
    let path: PathBuf = held_copy_path(options);
    let written: PathBuf = path.with_extension(format!("{}.tmp", std::process::id()));
    let version: String = copy
        .version
        .clone()
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    std::fs::write(
        &written,
        serde_json::to_vec(&copy).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&written, &path)?;
    let held = HeldCopy {
        target: options.target.clone(),
        generation,
        version,
        consumption,
        path,
    };
    let previous: Option<HeldCopy> = held_copies
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .replace(held.clone());
    if let Some(previous) = previous.filter(|previous| previous.path != held.path) {
        drop(std::fs::remove_file(previous.path));
    }
    Ok(held)
}
