use crate::tls::main::client_builder::client_builder;

#[test]
fn given_no_provider_when_building_clients_then_ring_is_installed_once_and_https_clients_build() {
    let first = client_builder().https_only(true).build();
    let second = client_builder().build();

    assert!(first.is_ok(), "{first:?}");
    assert!(second.is_ok(), "{second:?}");
    assert!(rustls::crypto::CryptoProvider::get_default().is_some());
}
