//! Bounded catalog traversal excludes only relations with a committed sample this UTC hour.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::_helpers::statistics::eligible_names;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{StatisticsDueRead, StatisticsRead};

pub async fn read_statistics_due(
    store: &ControlStore,
    scope: (Uuid, &str),
    request: &StatisticsDueRead,
) -> Result<StatisticsRead, ControlError> {
    if request.location.schema.trim().is_empty()
        || !(1..=MAX_HISTORY_RECORDS).contains(&request.limit)
    {
        return Err(ControlError::Invalid(
            "statistics eligibility needs a namespace and a bounded page".into(),
        ));
    }
    let transaction =
        begin_ingestion(store, scope, (request.session_id, request.generation)).await?;
    let mut parameters = vec![
        Parameter::Id(scope.0),
        Parameter::Text(scope.1.into()),
        Parameter::Text(request.location.schema.clone()),
        Parameter::Id(request.after.unwrap_or(Uuid::nil())),
    ];
    let database = if let Some(database) = &request.location.database {
        parameters.push(Parameter::Text(database.clone()));
        "state_database=$5"
    } else {
        "state_database IS NULL"
    };
    parameters.push(Parameter::Integer(request.limit as i64));
    let (transaction, rows) = query(transaction,
        &format!("SELECT event_id,record_json FROM sqb_warehouse_catalog WHERE location_id=$1 AND target=$2 \
        AND state_schema=$3 AND event_id>$4 AND {database} ORDER BY event_id LIMIT ${}", parameters.len()), parameters).await?;
    let next_after = if rows.len() == request.limit {
        rows.last()
            .map(|row| serde_json::from_value(row["event_id"].clone()))
            .transpose()
            .map_err(|error| ControlError::Invalid(error.to_string()))?
    } else {
        None
    };
    let (transaction, names) = eligible_names(transaction, scope, rows).await?;
    commit(transaction).await?;
    Ok(StatisticsRead {
        location: request.location.clone(),
        names,
        next_after,
    })
}
