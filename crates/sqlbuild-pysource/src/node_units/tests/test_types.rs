use crate::node_units::models::{DeferredSpan, UnitFunction, UnitImport};

pub(super) struct StatementKindsTestCase {
    pub(super) description: &'static str,
    pub(super) contents: &'static str,
    pub(super) expected_kinds: Option<Vec<&'static str>>,
}

pub(super) struct ImportsTestCase {
    pub(super) description: &'static str,
    pub(super) contents: &'static str,
    pub(super) expected_imports: Vec<UnitImport>,
}

pub(super) struct FunctionTestCase {
    pub(super) description: &'static str,
    pub(super) contents: &'static str,
    pub(super) expected_function: UnitFunction,
}

pub(super) struct DeferredSpansTestCase {
    pub(super) description: &'static str,
    pub(super) contents: &'static str,
    pub(super) expected_spans: Option<Vec<DeferredSpan>>,
}
