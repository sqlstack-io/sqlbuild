//! Chunks in the output store, keyed by run, claim and batch.

use object_store::ObjectStoreExt;
use object_store::path::Path;
use uuid::Uuid;

use crate::run_output::_helpers::store::chunks::decompress;
use crate::run_output::models::OutputStore;
use crate::store::errors::ControlError;

/// The key of one batch's chunk; a retried batch writes the same object again.
pub(crate) fn chunk_key(store: &OutputStore, (run_id, claim_id): (Uuid, Uuid), batch: u64) -> Path {
    run_prefix(store, run_id)
        .join(claim_id.to_string())
        .join(format!("{batch:012}.zst"))
}

/// Every object of one run sits under this prefix.
pub(crate) fn run_prefix(store: &OutputStore, run_id: Uuid) -> Path {
    store.prefix.clone().join("runs").join(run_id.to_string())
}

pub(crate) async fn put_chunk(
    store: &OutputStore,
    key: &Path,
    stored: Vec<u8>,
) -> Result<(), ControlError> {
    store.objects.put(key, stored.into()).await?;
    Ok(())
}

/// A chunk's text, at most `capacity` bytes.
pub(crate) async fn read_chunk(
    store: &OutputStore,
    key: &str,
    capacity: usize,
) -> Result<String, ControlError> {
    let stored = store.objects.get(&Path::from(key)).await?.bytes().await?;
    let text: String = tokio::task::spawn_blocking(move || decompress(&stored, capacity)).await??;
    Ok(text)
}

/// Delete chunks; one already gone is not an error.
pub(crate) async fn delete_chunks(
    store: &OutputStore,
    keys: &[String],
) -> Result<(), ControlError> {
    for key in keys {
        match store.objects.delete(&Path::from(key.as_str())).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
