//! Operator warnings a trusted proxy setup prints on stderr; an untrusted peer's header values are never printed.

use std::net::IpAddr;

use crate::api::constants::PROXY_NOTED_SOURCES;

pub(crate) fn warn_plain_http_origin(origin: &str) {
    eprintln!(
        "warning: the trusted-proxy origin {origin} is plain HTTP, so sign-ins and data cross the \
         network unencrypted; serve it over HTTPS where you can"
    );
}

pub(crate) fn warn_untrusted_source(header: &str, peer: IpAddr) {
    eprintln!(
        "warning: ignored the trusted-proxy username header {header} from {peer}, which is not in \
         [trusted_proxy] trusted_peers; if {peer} is your proxy, add it there"
    );
}

pub(crate) fn warn_untrusted_overflow() {
    eprintln!(
        "warning: more than {PROXY_NOTED_SOURCES} untrusted peers sent the trusted-proxy username \
         header; further peers are not logged"
    );
}

pub(crate) fn warn_taken_username(username: &str) {
    eprintln!(
        "warning: refused trusted-proxy username '{username}', which already names another \
         principal; bind it under [trusted_proxy.identities] to sign that person in"
    );
}
