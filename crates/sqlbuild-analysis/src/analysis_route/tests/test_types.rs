use crate::analysis_route::models::LocatedGap;

pub(super) struct NativeGapTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) placeholders: &'static [(&'static str, &'static str)],
    pub(super) expected_gap: Option<LocatedGap>,
}
