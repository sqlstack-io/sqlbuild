use crate::tls::main::client_builder::client_builder;
use crate::tls::tests::test_types::ClientBuilderTestCase;

#[test]
fn given_no_provider_when_building_clients_then_ring_is_installed_and_every_client_builds() {
    let test_cases = [
        ClientBuilderTestCase {
            description: "the first client installs ring and builds",
            https_only: true,
            expected_built: true,
        },
        ClientBuilderTestCase {
            description: "a later client finds ring installed and builds",
            https_only: false,
            expected_built: true,
        },
    ];
    for test_case in test_cases {
        let built = client_builder().https_only(test_case.https_only).build();

        assert_eq!(
            built.is_ok() && rustls::crypto::CryptoProvider::get_default().is_some(),
            test_case.expected_built,
            "{}",
            test_case.description
        );
    }
}
