//! `redact_text`: obvious named secrets and URI passwords removed from diagnostic text.

use crate::redaction::_helpers::patterns::{
    AUTHORIZATION, BEARER, DDL_PASSWORD, SECRET_VALUE, URI_CREDENTIAL, substituted,
};
use crate::redaction::constants::{
    BEARER_REPLACEMENT, DDL_REPLACEMENT, PAIR_REPLACEMENT, URI_REPLACEMENT,
};

/// Authorization headers, bearer tokens, `name=value` secrets, SQL passwords, then URI passwords redacted.
#[must_use]
pub fn redact_text(value: &str) -> String {
    let authorization: String = substituted(&AUTHORIZATION, value, PAIR_REPLACEMENT);
    let bearer: String = substituted(&BEARER, &authorization, BEARER_REPLACEMENT);
    let named: String = substituted(&SECRET_VALUE, &bearer, PAIR_REPLACEMENT);
    let statement: String = substituted(&DDL_PASSWORD, &named, DDL_REPLACEMENT);
    substituted(&URI_CREDENTIAL, &statement, URI_REPLACEMENT)
}
