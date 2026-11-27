use crate::nested_progress::models::{NestedItem, NestedLayout};

pub(super) struct NestedItemTestCase {
    pub(super) description: &'static str,
    pub(super) item: NestedItem,
    pub(super) group: &'static str,
    pub(super) layout: NestedLayout,
    pub(super) expected_text: &'static str,
}

pub(super) struct GroupTestCase {
    pub(super) description: &'static str,
    pub(super) count: usize,
    pub(super) expected_text: &'static str,
}
