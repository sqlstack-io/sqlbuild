//! Session creation rechecks the verified password version under the control-store write lock.

use crate::credentials::_helpers::browser_sessions::issue_live_session;
use crate::credentials::models::{AuditContext, IssuedSession, VerifiedPassword};
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

pub(crate) async fn issue_session(
    store: &ControlStore,
    context: &AuditContext,
    proof: VerifiedPassword,
) -> Result<Option<IssuedSession>, ControlError> {
    write(store, |transaction| async move {
        let (transaction, issued) = issue_live_session(
            transaction,
            context,
            (proof.principal_id, Some(proof.credential_id)),
            "password-session",
        )
        .await?;
        Ok((transaction, issued.map(|(_, session)| session)))
    })
    .await
}
