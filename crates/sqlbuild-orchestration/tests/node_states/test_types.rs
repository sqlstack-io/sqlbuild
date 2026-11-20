pub(crate) struct HeldChecksTestCase {
    pub(crate) description: &'static str,
    pub(crate) signals_during_read: usize,
    pub(crate) expected_held: Option<&'static str>,
}
