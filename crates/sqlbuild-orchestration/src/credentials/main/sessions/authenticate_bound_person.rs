//! Explicit OIDC bindings issue sessions and replace the person's groups at each sign-in.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::_helpers::browser_sessions::issue_live_session;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, IssuedSession};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::write::write;
use crate::store::models::{ControlStore, Parameter};

pub(crate) async fn authenticate_bound_person(
    store: &ControlStore,
    context: &AuditContext,
    (principal_id, groups): (Uuid, &[String]),
) -> Result<Option<IssuedSession>, ControlError> {
    write(store, |transaction| async move {
        let (transaction, issued) =
            issue_live_session(transaction, context, (principal_id, None), "oidc").await?;
        let Some((context, session)) = issued else {
            return Ok((transaction, None));
        };
        let (mut transaction, _) = query(
            transaction,
            "DELETE FROM sqb_person_groups WHERE principal_id=$1",
            vec![Parameter::Id(principal_id)],
        )
        .await?;
        for group in groups.iter().collect::<BTreeSet<_>>() {
            (transaction, _) = query(
                transaction,
                "INSERT INTO sqb_person_groups (principal_id,group_name) VALUES ($1,$2)",
                vec![Parameter::Id(principal_id), Parameter::Text(group.clone())],
            )
            .await?;
        }
        let identity = json!({"principal_id": principal_id, "groups": groups});
        let transaction = append_audit(
            transaction,
            &context,
            (context.request_id, "oidc.authenticated"),
            (identity.clone(), json!(null), identity),
        )
        .await?;
        Ok((transaction, Some(session)))
    })
    .await
}
