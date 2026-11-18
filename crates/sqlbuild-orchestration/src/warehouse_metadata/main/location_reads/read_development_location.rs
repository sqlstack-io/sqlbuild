//! The location `sqb dev` keeps for a project's target, read without creating one.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::DEVELOPMENT_LOCATIONS_SQL;

/// The development location of `target` whose project path `same_project` accepts, if any.
pub async fn read_development_location(
    store: &ControlStore,
    target: &str,
    same_project: impl Fn(&str) -> bool,
) -> Result<Option<Uuid>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        DEVELOPMENT_LOCATIONS_SQL,
        vec![Parameter::Text(target.into())],
    )
    .await?;
    commit(transaction).await?;
    rows.iter()
        .find(|row| row["project_path"].as_str().is_some_and(&same_project))
        .map(|row| {
            row["location_id"]
                .as_str()
                .unwrap_or_default()
                .parse()
                .map_err(|_| ControlError::Schema("invalid development location identity".into()))
        })
        .transpose()
}
