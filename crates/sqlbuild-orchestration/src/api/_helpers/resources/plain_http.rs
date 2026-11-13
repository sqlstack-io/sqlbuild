//! Warn once per process before a bearer token crosses the network over plain HTTP.

use std::net::IpAddr;
use std::sync::Once;

use crate::api::constants::{HTTP_SCHEME, LOOPBACK_HOST};

/// Loopback origins stay silent; any other `http://` origin warns on the first connection only.
pub(crate) fn warn_plain_http(url: &reqwest::Url) {
    static WARNED: Once = Once::new();
    let loopback = url.host_str().is_some_and(|host| {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        host.eq_ignore_ascii_case(LOOPBACK_HOST)
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() == HTTP_SCHEME && !loopback {
        WARNED.call_once(|| {
            eprintln!(
                "warning: the bearer token for {} travels unencrypted over HTTP; put TLS or an HTTPS reverse proxy in front of the server",
                url.origin().ascii_serialization()
            );
        });
    }
}
