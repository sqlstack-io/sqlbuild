//! The secret patterns, compiled once per process.

use std::sync::LazyLock;

use regex::{Captures, Regex, RegexBuilder};

use crate::redaction::constants::{
    AUTHORIZATION_PATTERN, BARE_USERINFO_PATTERN, BEARER_PATTERN, DDL_PASSWORD_PATTERN,
    PATTERN_SIZE_LIMIT, SECRET_VALUE_PATTERN, URI_CREDENTIAL_PATTERN,
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
