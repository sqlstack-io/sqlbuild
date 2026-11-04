//! Credential contracts compiled within the credential owner.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "locations/helpers.rs"]
pub(crate) mod location_helpers;
#[path = "test_administrators.rs"]
mod test_administrators;
#[path = "test_audit.rs"]
mod test_audit;
#[path = "test_audit_retention.rs"]
mod test_audit_retention;
#[path = "test_contract.rs"]
mod test_contract;
#[path = "passwords/test_contract.rs"]
mod test_passwords;
#[path = "test_recovery.rs"]
mod test_recovery;
#[path = "sessions/test_contract.rs"]
mod test_sessions;
#[path = "test_types.rs"]
mod test_types;
