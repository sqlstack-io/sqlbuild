//! The newest mirrored build of each queried destination.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::warehouse_metadata::_helpers::fingerprint_reads;
use crate::warehouse_metadata::models::{FingerprintObservation, FingerprintQuery};

/// The newest mirrored build of each queried destination, with its definition texts.
#[inline]
pub(crate) async fn latest_fingerprints(
    transaction: ControlTransaction,
    scope: (Uuid, &str, &[FingerprintQuery]),
) -> Result<
    (
        ControlTransaction,
        Vec<Option<(FingerprintObservation, String, String)>>,
    ),
    ControlError,
> {
    fingerprint_reads::latest_fingerprints(transaction, scope).await
}
