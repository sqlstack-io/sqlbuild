//! Shapes shared inside the warehouse-history domain.

/// Who read, what kind of read, and which run: the key of an audited repeated read.
pub(crate) type AuditKey = (String, String, uuid::Uuid);
