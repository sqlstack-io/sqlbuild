//! The signature of a migration's data step.

use std::future::Future;
use std::pin::Pin;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// A one-off data change written in Rust, which a migration runs once after its SQL, in the same transaction.
pub type StoreMigrationStep =
    fn(
        ControlTransaction,
    ) -> Pin<Box<dyn Future<Output = Result<ControlTransaction, ControlError>> + Send>>;
