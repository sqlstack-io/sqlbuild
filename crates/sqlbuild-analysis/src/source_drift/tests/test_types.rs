/// A model query, and whether `orders` gaining a `region` column could change its result.
pub(super) struct AddedColumnsTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_affected: bool,
}

pub(super) struct DialectAddedColumnsTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_affected: bool,
}
