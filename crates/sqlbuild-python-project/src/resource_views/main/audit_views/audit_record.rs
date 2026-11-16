//! The audit record of an attached audit, before assembly resolves its scope dependencies.

use crate::resource_views::_helpers::audit_views;
use crate::resource_views::types::AttachedAudit;
use crate::resource_views::types::AuditRecord;

/// The audit record of an attached audit, before assembly resolves its scope dependencies.
#[inline]
pub fn audit_record(attached: AttachedAudit) -> AuditRecord {
    audit_views::audit_record(attached)
}
