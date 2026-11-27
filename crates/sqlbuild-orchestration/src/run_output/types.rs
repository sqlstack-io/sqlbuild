//! Shapes run output reads share.

/// Who reads a run's output: their audit context and person, the run as location, target and ID, and the server's default role.
pub(crate) type PersonRunRead<'a> = (
    (&'a crate::credentials::models::AuditContext, uuid::Uuid),
    (uuid::Uuid, &'a str, uuid::Uuid),
    crate::credentials::models::DefaultRole,
);

/// Reports one run change under its event key: `Ok` once the server acknowledged it, else why it didn't.
pub trait NodeLogReport: std::fmt::Debug + Send + Sync {
    /// Send `change` under `key` and wait for the server's acknowledgement, or say why it didn't come.
    fn report(&self, key: &str, change: &crate::runs::models::RunChange) -> Result<(), String>;
}
