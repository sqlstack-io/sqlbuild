/// An overlay that sets one variable, removes one inherited variable and carries an env file marker.
pub(super) struct OverlayTestCase {
    pub(super) description: &'static str,
    pub(super) set: (&'static str, &'static str),
    /// An inherited variable the overlay unsets.
    pub(super) removed: &'static str,
    pub(super) expected_value: &'static str,
}
