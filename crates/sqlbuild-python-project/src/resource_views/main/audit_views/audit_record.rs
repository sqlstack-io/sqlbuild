//! The audit record of an attached audit, before assembly resolves its scope dependencies.

use crate::resource_views::models::AuditProjections;
use crate::resource_views::types::AttachedAudit;
use crate::resource_views::types::AuditRecord;
use pyo3::sync::PyOnceLock;

/// The audit record of an attached audit, before assembly resolves its scope dependencies.
pub fn audit_record(attached: AttachedAudit) -> AuditRecord {
    AuditRecord {
        attached,
        scope_deps: None,
        host: AuditProjections {
            references: PyOnceLock::new(),
            key: PyOnceLock::new(),
        },
    }
}
