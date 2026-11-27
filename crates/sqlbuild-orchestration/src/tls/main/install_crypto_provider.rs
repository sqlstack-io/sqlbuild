//! Give the process its rustls crypto provider, which reqwest needs before it builds a client.

/// Make ring the process's rustls provider unless one is installed already; safe to call before every client.
pub fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _lost_race = rustls::crypto::ring::default_provider().install_default();
    }
}
