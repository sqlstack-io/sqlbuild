//! Authorization decisions for one person, scope or row, and the security audit they write.

pub(crate) mod authorize_listing;
pub(crate) mod authorize_person;
pub(crate) mod begin_authorized;
pub(crate) mod decide_person_scopes;
pub(crate) mod deny_authorization;
pub(crate) mod person_holds_target;
pub mod read_security_audit;
pub(crate) mod refuse_unknown;
pub(crate) mod row_in_target;
