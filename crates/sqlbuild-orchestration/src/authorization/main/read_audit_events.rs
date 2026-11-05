//! Authorized server consumers page security facts independently of clocks and transport.

use crate::credentials::models::SecurityAuditEvent;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_audit_events(
    transaction: ControlTransaction,
    (after, limit): (i64, u32),
) -> Result<(ControlTransaction, Vec<SecurityAuditEvent>), ControlError> {
    if after < 0 || !(1..=100).contains(&limit) {
        return Err(ControlError::Invalid(
            "audit pages require a nonnegative storage cursor and a limit between 1 and 100".into(),
        ));
    }
    let (transaction, rows) = query(transaction,
        "SELECT storage_id,event_id,occurred_at,actor,action,granting_role,request_id,ip_address,scope_json,before_json,after_json \
         FROM sqb_security_audit WHERE storage_id>$1 ORDER BY storage_id LIMIT $2",
        vec![Parameter::Integer(after), Parameter::Integer(i64::from(limit))]).await?;
    Ok((
        transaction,
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?,
    ))
}
