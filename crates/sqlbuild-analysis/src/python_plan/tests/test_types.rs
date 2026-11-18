pub(crate) struct OrderTestCase {
    pub(crate) description: &'static str,
    pub(crate) selected: &'static [&'static str],
    pub(crate) upstream: &'static [(&'static str, &'static [&'static str])],
    pub(crate) expected_order: &'static [&'static str],
}

pub(crate) struct ClosureTestCase {
    pub(crate) description: &'static str,
    pub(crate) start: &'static str,
    pub(crate) upstream: &'static [(&'static str, &'static [&'static str])],
    pub(crate) expected_closure: &'static [&'static str],
}

pub(crate) struct LifecycleTestCase {
    pub(crate) description: &'static str,
    pub(crate) selected: &'static [&'static str],
    pub(crate) nodes: &'static [LifecycleNodeSpec],
    pub(crate) upstream: &'static [(&'static str, &'static [&'static str])],
    pub(crate) expected_ingress: &'static [&'static str],
    pub(crate) expected_loaders: &'static [&'static str],
    pub(crate) expected_read_side: &'static [&'static str],
    /// `(loader, SQL it waits for, tasks and assets that gate it, the gates it reads directly)`.
    pub(crate) expected_round_trips: &'static [ExpectedRoundTrip],
}

/// `(name, kind, SQL reads)`.
pub(crate) type LifecycleNodeSpec = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// `(loader, SQL it waits for, tasks and assets that gate it)`.
pub(crate) type ExpectedRoundTrip = (
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
    &'static [&'static str],
);
