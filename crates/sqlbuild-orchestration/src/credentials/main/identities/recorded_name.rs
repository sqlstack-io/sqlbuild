//! The operator's CLI takes IDs and says what they name.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

/// The name of the location or principal `id` identifies, or `None` when neither does.
pub async fn recorded_name(store: &ControlStore, id: Uuid) -> Result<Option<String>, ControlError> {
    let named: Vec<serde_json::Value> = read_records(
        store,
        "SELECT name FROM sqb_locations WHERE location_id=$1 \
         UNION ALL SELECT name FROM sqb_principals WHERE principal_id=$1",
        vec![Parameter::Id(id)],
    )
    .await?;
    Ok(named
        .into_iter()
        .find_map(|row| row["name"].as_str().map(str::to_owned)))
}
