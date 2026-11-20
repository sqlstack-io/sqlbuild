//! A dev project/target retains its location identity across restarts, named after the project.

use uuid::Uuid;

use crate::credentials::_helpers::locations::create_location_in;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn development_location(
    store: &ControlStore,
    (project_path, project_name): (&str, &str),
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
    let mut transaction = transaction;
    let mut name = project_name.to_owned();
    for suffix in 2.. {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT 1 AS taken FROM sqb_locations WHERE name=$1",
            vec![Parameter::Text(name.clone())],
        )
        .await?;
        if rows.is_empty() {
            break;
        }
        name = format!("{project_name} ({suffix})");
    }
    let (transaction, id) = create_location_in(transaction, &name, context).await?;
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

/// The location `sqb dev` keeps for a project and target, without creating one; `None` before its first start.
pub async fn existing_development_location(
    store: &ControlStore,
    project_path: &str,
    target: &str,
) -> Result<Option<Uuid>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT location_id FROM sqb_development_locations WHERE project_path=$1 AND target=$2",
        vec![
            Parameter::Text(project_path.into()),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    commit(transaction).await?;
    rows.first()
        .map(|row| {
            row["location_id"]
                .as_str()
                .unwrap_or_default()
                .parse()
                .map_err(|_| ControlError::Schema("invalid development location identity".into()))
        })
        .transpose()
}
