/// A model query, and whether `orders` gaining a `region` column could change its result.
pub(super) struct AddedColumnsTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_affected: bool,
}
