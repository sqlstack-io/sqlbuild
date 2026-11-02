//! Open a shared-access control database without Python or warehouse connections.

use crate::store::_helpers::open_control_store::open_control_store;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Open PostgreSQL by URL, or SQLite by an explicit filesystem path.
pub async fn open_store(url: &str) -> Result<ControlStore, ControlError> {
    open_control_store(url, false).await
}
