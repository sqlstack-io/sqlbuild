pub(super) struct NodeSelectorTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_names: Result<&'static [&'static str], SelectorFailure>,
}

pub(super) struct RunSelectionTestCase {
    pub(super) description: &'static str,
    pub(super) select: &'static [&'static str],
    pub(super) exclude: &'static [&'static str],
    pub(super) expected_selection: Result<ExpectedRun, SelectorFailure>,
}

pub(super) struct InventoryTestCase {
    pub(super) description: &'static str,
    pub(super) relative_path: &'static str,
    pub(super) expected_folder: &'static str,
}

/// `(sql keys, python node names)` a run selects.
pub(super) type ExpectedRun = (
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
);

/// `(code, message)` of the `PlannerInputError` Python raises.
pub(super) type SelectorFailure = (&'static str, &'static str);

pub(super) struct RoundTripTestCase {
    pub(super) description: &'static str,
    /// `(name, kind, upstream Python nodes, SQL reads)` of each Python node.
    pub(super) nodes: &'static [RoundTripNodeSpec],
    pub(super) allow_warehouse_round_trips: bool,
    pub(super) select: &'static [&'static str],
    pub(super) expected_selection: Result<ExpectedRun, (&'static str, &'static str, &'static str)>,
}

/// `(name, kind, upstream Python nodes, SQL reads)`.
pub(super) type RoundTripNodeSpec = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
);

pub(super) struct CycleTestCase {
    pub(super) description: &'static str,
    /// `(name, kind, upstream Python nodes, SQL reads)` of each Python node.
    pub(super) nodes: &'static [RoundTripNodeSpec],
    /// Whether the `partner_scores` source names `partner_scores` as its loader.
    pub(super) filled: bool,
    pub(super) allow_warehouse_round_trips: bool,
    pub(super) expected_error: Option<&'static str>,
}
