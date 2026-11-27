pub(super) struct ClientBuilderTestCase {
    pub(super) description: &'static str,
    pub(super) https_only: bool,
    pub(super) expected_built: bool,
}
