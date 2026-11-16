use sqlbuild_config::models::ConfigValue;

/// A loaded YAML document kept by the delta compile's document store.
pub(crate) struct DocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) document: ConfigValue,
    /// How many truncations of the stored document still read back as it.
    pub(crate) expected_truncations_read: usize,
}
