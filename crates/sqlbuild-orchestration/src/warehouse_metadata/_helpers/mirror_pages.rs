//! Mirror history pages share database scoping and the (observed_at, event_id) keyset cursor.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::store::models::Parameter;

/// Adds scope, cursor and page size to the parameters, with the SQL after the caller's conditions.
pub(crate) fn page_tail(
    mut parameters: Vec<Parameter>,
    database: Option<&String>,
    after: (Option<DateTime<Utc>>, Option<Uuid>),
    limit: usize,
) -> (Vec<Parameter>, String) {
    let database = database.map_or("state_database IS NULL".to_owned(), |database| {
        parameters.push(Parameter::Text(database.clone()));
        format!("state_database=${}", parameters.len())
    });
    let after = after
        .0
        .zip(after.1)
        .map_or(String::new(), |(observed_at, event_id)| {
            parameters.extend([Parameter::Time(observed_at), Parameter::Id(event_id)]);
            let position = parameters.len();
            format!(
                " AND (observed_at,event_id)>(${},${position})",
                position - 1
            )
        });
    parameters.push(Parameter::Integer(limit as i64));
    let tail = format!(
        " AND {database}{after} ORDER BY observed_at,event_id LIMIT ${}",
        parameters.len()
    );
    (parameters, tail)
}
