//! The builder of every outbound reqwest client, with ring as the process's rustls provider.

use crate::tls::main::install_crypto_provider::install_crypto_provider;

/// `reqwest::Client::builder()`, once the process has its rustls provider.
pub fn client_builder() -> reqwest::ClientBuilder {
    install_crypto_provider();
    reqwest::Client::builder()
}
