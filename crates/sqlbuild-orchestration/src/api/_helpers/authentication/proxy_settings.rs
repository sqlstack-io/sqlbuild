//! Validated `[trusted_proxy]` settings: identity headers, canonical usernames and the sign-out URL.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::sync::Mutex;

use axum::http::{HeaderMap, HeaderName};
use uuid::Uuid;

use crate::api::_helpers::authentication::proxy_warnings::{
    warn_taken_username, warn_untrusted_overflow, warn_untrusted_source,
};
use crate::api::constants::{
    HTTP_SCHEME, HTTPS_SCHEME, PROXY_NOTED_SOURCES, PROXY_PROFILE_CHARS, PROXY_RESERVED_HEADERS,
    PROXY_USER_HEADER, PROXY_USERNAME_CHARS,
};
use crate::api::models::{ProxyProfile, TrustedProxyAuthentication, UnknownProxyUsers};
use crate::store::errors::ControlError;

pub(crate) struct ProxySettings {
    pub(crate) trusted_peers: BTreeSet<IpAddr>,
    /// Canonical usernames bound to existing principals.
    pub(crate) identities: BTreeMap<String, Uuid>,
    pub(crate) username_header: HeaderName,
    display_name_header: Option<HeaderName>,
    email_header: Option<HeaderName>,
    pub(crate) provision: bool,
    pub(crate) logout_url: Option<String>,
    /// Untrusted peers already logged for sending the username header.
    noted_sources: Mutex<BTreeSet<IpAddr>>,
    /// Usernames already logged for naming another principal.
    noted_usernames: Mutex<BTreeSet<String>>,
}

/// Whether an item is logged now: the first time, or the one time the log fills up.
enum Note {
    First,
    Overflow,
    Quiet,
}

/// The username header when `[trusted_proxy] username_header` is not set.
pub(crate) fn default_username_header() -> String {
    PROXY_USER_HEADER.to_owned()
}

fn note<T: Ord>(noted: &Mutex<BTreeSet<T>>, item: T) -> Note {
    let Ok(mut noted) = noted.lock() else {
        return Note::Quiet;
    };
    if noted.len() > PROXY_NOTED_SOURCES || !noted.insert(item) {
        return Note::Quiet;
    }
    if noted.len() > PROXY_NOTED_SOURCES {
        Note::Overflow
    } else {
        Note::First
    }
}

impl ProxySettings {
    pub(crate) fn new(settings: TrustedProxyAuthentication) -> Result<Self, ControlError> {
        if settings.trusted_peers.is_empty() {
            return Err(ControlError::Invalid(
                "trusted-proxy authentication requires explicit trusted peer IP addresses".into(),
            ));
        }
        let username_header = header_name(&settings.username_header, "username_header")?;
        let display_name_header = settings
            .display_name_header
            .as_deref()
            .map(|name| header_name(name, "display_name_header"))
            .transpose()?;
        let email_header = settings
            .email_header
            .as_deref()
            .map(|name| header_name(name, "email_header"))
            .transpose()?;
        let mut names = vec![&username_header];
        names.extend(display_name_header.iter());
        names.extend(email_header.iter());
        if names
            .iter()
            .map(|name| name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != names.len()
        {
            return Err(ControlError::Invalid(
                "[trusted_proxy] username_header, display_name_header and email_header must differ"
                    .into(),
            ));
        }
        let mut identities: BTreeMap<String, Uuid> = BTreeMap::new();
        for (subject, principal) in settings.identities {
            let username = canonical_username(&subject).ok_or_else(|| {
                ControlError::Invalid(format!(
                    "trusted-proxy usernames must be 1-{PROXY_USERNAME_CHARS} characters of a-z, 0-9, \
                     '.', '_', '@', '+' or '-', starting with a letter or digit; \
                     '{}' in [trusted_proxy.identities] is not",
                    subject.escape_default()
                ))
            })?;
            if identities.insert(username.clone(), principal).is_some() {
                return Err(ControlError::Invalid(format!(
                    "[trusted_proxy.identities] binds '{username}' more than once; \
                     usernames are compared in lowercase"
                )));
            }
        }
        Ok(Self {
            trusted_peers: settings
                .trusted_peers
                .into_iter()
                .map(|peer| peer.to_canonical())
                .collect(),
            identities,
            username_header,
            display_name_header,
            email_header,
            provision: match settings.unknown_users {
                UnknownProxyUsers::ProvisionViewer => true,
                UnknownProxyUsers::Reject => false,
            },
            logout_url: settings
                .proxy_logout_url
                .as_deref()
                .map(logout_url)
                .transpose()?,
            noted_sources: Mutex::new(BTreeSet::new()),
            noted_usernames: Mutex::new(BTreeSet::new()),
        })
    }

    /// Logs each untrusted peer that sends the username header once, so its address can be configured.
    pub(crate) fn note_untrusted_source(&self, peer: IpAddr) {
        match note(&self.noted_sources, peer) {
            Note::First => warn_untrusted_source(self.username_header.as_str(), peer),
            Note::Overflow => warn_untrusted_overflow(),
            Note::Quiet => {}
        }
    }

    /// Logs each refused username that already names another principal once.
    pub(crate) fn note_taken_username(&self, username: &str) {
        if let Note::First = note(&self.noted_usernames, username.to_owned()) {
            warn_taken_username(username);
        }
    }

    /// The display name and email address the proxy reported, read per request and never stored.
    pub(crate) fn profile(&self, headers: &HeaderMap) -> ProxyProfile {
        let value =
            |name: &Option<HeaderName>| name.as_ref().and_then(|name| profile_value(headers, name));
        ProxyProfile {
            display_name: value(&self.display_name_header),
            email: value(&self.email_header),
        }
    }
}

/// The username a proxy reports, trimmed and lowercased, when it is a valid username.
pub(crate) fn canonical_username(raw: &str) -> Option<String> {
    let username = raw.trim().to_ascii_lowercase();
    let mut characters = username.chars();
    let valid = characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '@' | '+' | '-')
        })
        && username.len() <= PROXY_USERNAME_CHARS;
    valid.then_some(username)
}

/// One header's trimmed UTF-8 value without control characters, or nothing.
fn profile_value(headers: &HeaderMap, name: &HeaderName) -> Option<String> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let Ok(text) = std::str::from_utf8(value.as_bytes()) else {
        return None;
    };
    let text = text.trim();
    let usable = !text.is_empty()
        && text.chars().count() <= PROXY_PROFILE_CHARS
        && !text.chars().any(char::is_control);
    usable.then(|| text.to_owned())
}

fn header_name(value: &str, key: &str) -> Result<HeaderName, ControlError> {
    let name = HeaderName::from_bytes(value.trim().as_bytes()).map_err(|_| {
        ControlError::Invalid(format!("[trusted_proxy] {key} is not a valid header name"))
    })?;
    if PROXY_RESERVED_HEADERS.contains(&name.as_str()) {
        return Err(ControlError::Invalid(format!(
            "[trusted_proxy] {key} cannot be {name}: browsers, proxies or the server set it"
        )));
    }
    Ok(name)
}

/// An absolute HTTP(S) URL without credentials, or a path on this origin.
fn logout_url(value: &str) -> Result<String, ControlError> {
    let error = || {
        ControlError::Invalid(
            "[trusted_proxy] proxy_logout_url must be an http:// or https:// URL without \
             credentials, or a path starting with a single /"
                .into(),
        )
    };
    if value.starts_with('/') {
        return if value.starts_with("//")
            || value.contains('\\')
            || value.chars().any(char::is_control)
        {
            Err(error())
        } else {
            Ok(value.to_owned())
        };
    }
    let url = reqwest::Url::parse(value).map_err(|_| error())?;
    let allowed = (url.scheme() == HTTPS_SCHEME || url.scheme() == HTTP_SCHEME)
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none();
    if allowed {
        Ok(url.into())
    } else {
        Err(error())
    }
}
