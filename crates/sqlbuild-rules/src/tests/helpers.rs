use crate::build_identity::main::install_build_identity::install_build_identity;

/// The build identity every test of this crate installs; the binary installs the real one.
pub(crate) const TEST_BUILD_IDENTITY: &str = "0123456789abcdef";

/// Install the test build identity, for tests that read cache keys or rule fingerprints.
pub(crate) fn install_test_identity() {
    install_build_identity(TEST_BUILD_IDENTITY).expect("tests install only the test identity");
}
