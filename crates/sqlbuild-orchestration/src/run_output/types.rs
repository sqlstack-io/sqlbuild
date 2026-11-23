//! Shapes run output reads share.

/// Who reads a run's output: their audit context and person, the run as location, target and ID, and the server's default role.
pub(crate) type PersonRunRead<'a> = (
    (&'a crate::credentials::models::AuditContext, uuid::Uuid),
    (uuid::Uuid, &'a str, uuid::Uuid),
    crate::credentials::models::DefaultRole,
);
