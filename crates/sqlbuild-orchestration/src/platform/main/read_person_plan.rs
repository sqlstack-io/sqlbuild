//! A plan is read only by the person who asked for it, with view still held on every current node.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::plans::plan_access::read_person_plan_with_sql;
use crate::platform::models::LocationWorkRecord;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// SQL, config, Python source and dependency diffs stay out unless the reader may read SQL on every current node.
pub async fn read_person_plan(
    store: &ControlStore,
    reader: (&AuditContext, Uuid),
    plan: (Uuid, &str, Uuid),
) -> Result<Option<LocationWorkRecord>, ControlError> {
    Ok(read_person_plan_with_sql(store, reader, plan)
        .await?
        .map(|(record, _)| record))
}
