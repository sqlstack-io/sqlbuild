//! Credential contracts compiled within the credential owner.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "locations/helpers.rs"]
pub(crate) mod location_helpers;
#[path = "test_administrators.rs"]
mod test_administrators;
#[path = "test_contract.rs"]
mod test_contract;
#[path = "passwords/test_contract.rs"]
mod test_passwords;
#[path = "sessions/test_contract.rs"]
mod test_sessions;
#[path = "test_types.rs"]
mod test_types;
