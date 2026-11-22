//! The secret patterns, compiled once per process.

use std::sync::LazyLock;

use regex::{Captures, Regex, RegexBuilder};

use crate::redaction::constants::{
    AUTHORIZATION_PATTERN, BARE_USERINFO_PATTERN, BEARER_PATTERN, BEARER_REPLACEMENT,
    DDL_PASSWORD_PATTERN, DDL_REPLACEMENT, PAIR_REPLACEMENT, PATTERN_SIZE_LIMIT,
    SECRET_VALUE_PATTERN, URI_CREDENTIAL_PATTERN, URI_REPLACEMENT,
};

pub(crate) static AUTHORIZATION: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(AUTHORIZATION_PATTERN));
pub(crate) static BEARER: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(BEARER_PATTERN));
pub(crate) static SECRET_VALUE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(SECRET_VALUE_PATTERN));
pub(crate) static DDL_PASSWORD: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(DDL_PASSWORD_PATTERN));
pub(crate) static URI_CREDENTIAL: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(URI_CREDENTIAL_PATTERN));
static BARE_USERINFO: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| compiled(BARE_USERINFO_PATTERN));

fn compiled(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(PATTERN_SIZE_LIMIT)
        .build()
}

/// `pattern.sub(replacement, text)`; a pattern that failed to compile matches nothing.
pub(crate) fn substituted(
    pattern: &LazyLock<Result<Regex, regex::Error>>,
    text: &str,
    replacement: &str,
) -> String {
    match pattern.as_ref() {
        Ok(regex) => regex.replace_all(text, replacement).into_owned(),
        Err(_) => text.to_owned(),
    }
}

/// One reading's named secrets and URI passwords redacted.
pub(crate) fn named_redacted(text: &str) -> String {
    let authorization: String = substituted(&AUTHORIZATION, text, PAIR_REPLACEMENT);
    let bearer: String = substituted(&BEARER, &authorization, BEARER_REPLACEMENT);
    let named: String = substituted(&SECRET_VALUE, &bearer, PAIR_REPLACEMENT);
    let statement: String = substituted(&DDL_PASSWORD, &named, DDL_REPLACEMENT);
    substituted(&URI_CREDENTIAL, &statement, URI_REPLACEMENT)
}

/// One reading of error text, which may also hold a URI cut before its host.
pub(crate) fn error_redacted(text: &str) -> String {
    bare_userinfo_redacted(&named_redacted(text))
}

/// A port is all digits and the authority ends after it (`/`, `?`, `#`); anything else may be a password.
pub(crate) fn bare_userinfo_redacted(text: &str) -> String {
    match BARE_USERINFO.as_ref() {
        Ok(regex) => regex
            .replace_all(text, |found: &Captures<'_>| {
                if !found[3].is_empty() && found[2].bytes().all(|byte| byte.is_ascii_digit()) {
                    found[0].to_owned()
                } else {
                    format!("{}[REDACTED]{}", &found[1], &found[3])
                }
            })
            .into_owned(),
        Err(_) => text.to_owned(),
    }
}
