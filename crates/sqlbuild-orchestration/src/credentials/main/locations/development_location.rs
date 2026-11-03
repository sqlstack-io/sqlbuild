//! A dev project/target retains its opaque location identity across process restarts.

use uuid::Uuid;

use crate::credentials::_helpers::locations::create_location_in;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn development_location(
    store: &ControlStore,
    project_path: &str,
    target: &str,
    context: &AuditContext,
) -> Result<Uuid, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Write).await?,
        "SELECT location_id FROM sqb_development_locations WHERE project_path=$1 AND target=$2",
        vec![
            Parameter::Text(project_path.into()),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    if let Some(row) = rows.first() {
        let id = row["location_id"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .map_err(|_| ControlError::Schema("invalid development location identity".into()))?;
        commit(transaction).await?;
        return Ok(id);
    }
    let (transaction, id) = create_location_in(
        transaction,
        &format!("development-{}", Uuid::now_v7()),
        context,
    )
    .await?;
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_development_locations (project_path,target,location_id) VALUES ($1,$2,$3)",
        vec![
            Parameter::Text(project_path.into()),
            Parameter::Text(target.into()),
            Parameter::Id(id),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(id)
}
