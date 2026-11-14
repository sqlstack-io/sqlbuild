pub(super) struct InventoryErrorCase {
    pub(super) description: &'static str,
    pub(super) files: Vec<(&'static str, &'static str)>,
    pub(super) expected_message: &'static str,
    pub(super) expected_help: Option<&'static str>,
}
