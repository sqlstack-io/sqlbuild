/// Recorded SQL and its fingerprint, baselined against the current SQL's fingerprint.
pub(super) struct BaselineFingerprintTestCase {
    pub(super) description: &'static str,
    pub(super) current_sql: &'static str,
    pub(super) recorded_definition: &'static str,
    /// The recorded hash, or `None` when it is the current scheme's own hash of the current SQL.
    pub(super) recorded_hash: Option<&'static str>,
    /// Why fingerprinting the recorded definition fails, when it does.
    pub(super) recorded_fingerprint_failure: Option<&'static str>,
    /// Whether the baseline is the recorded hash, and whether it is the current fingerprint.
    pub(super) expected_matches_recorded_and_current: (bool, bool),
}
