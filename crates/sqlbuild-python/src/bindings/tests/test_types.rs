use sqlbuild_cache::project_reuse::types::ImageSections;
use sqlbuild_config::models::ConfigValue;

use crate::bindings::_helpers::compile_context::delta::ModelCoverage;
use crate::bindings::_helpers::compile_delta::image::ProjectImage;

/// A stored report, a delta report and what splicing the delta into the stored one gives.
pub(crate) struct SpliceTestCase {
    pub(crate) description: &'static str,
    pub(crate) base: &'static str,
    pub(crate) delta: &'static str,
    pub(crate) models: &'static [&'static str],
    pub(crate) tests: &'static [&'static str],
    pub(crate) sources: Option<&'static [&'static str]>,
    /// The spliced report, or `None` when the reports must not line up.
    pub(crate) expected_report: Option<&'static str>,
}

/// A loaded YAML document kept by the delta compile's document store.
pub(crate) struct DocumentTestCase {
    pub(crate) description: &'static str,
    pub(crate) document: ConfigValue,
    /// How many truncations of the stored document still read back as it.
    pub(crate) expected_truncations_read: usize,
}

/// Stored image bytes damaged before they are read back.
pub(crate) struct DamagedImageTestCase {
    pub(crate) description: &'static str,
    pub(crate) damage: fn(&mut ImageSections),
    pub(crate) expected_gate: bool,
    pub(crate) expected_image: bool,
}

/// An image encoded and decoded again.
pub(crate) struct ImageRoundTripTestCase {
    pub(crate) description: &'static str,
    pub(crate) image: fn() -> ProjectImage,
    pub(crate) expected_model_subjects: bool,
    /// The retained rules request's model and digest count, or `None` without one.
    pub(crate) expected_rule_models: Option<usize>,
}

/// Stored and fresh semantic coverage facts, and the whole project's partial-check reasons.
pub(crate) struct CoverageTestCase {
    pub(crate) description: &'static str,
    pub(crate) stored: Vec<ModelCoverage>,
    pub(crate) fresh: Vec<ModelCoverage>,
    pub(crate) stored_order: Vec<(String, Vec<String>)>,
    pub(crate) fresh_order: Vec<(String, Vec<String>)>,
    pub(crate) expected_reasons: Vec<(String, Vec<String>)>,
}

/// An error message a delta round reported.
pub(crate) struct UncompiledModelTestCase {
    pub(crate) description: &'static str,
    pub(crate) message: &'static str,
    pub(crate) expected_uncompiled: bool,
}

/// A custom-rule fact key a delta compile holding some models may or may not read in full.
pub(crate) struct DeltaFactTestCase {
    pub(crate) description: &'static str,
    pub(crate) key: &'static [&'static str],
    pub(crate) all_sources: bool,
    pub(crate) expected_safe: bool,
}

pub(crate) struct FailureCauseTestCase {
    pub(crate) description: &'static str,
    pub(crate) report: fn(),
    pub(crate) expected_code: Option<&'static str>,
}

pub(crate) struct CacheKeyTestCase {
    pub(crate) description: &'static str,
    /// The environment variable values of the second lookup, by name.
    pub(crate) changed: &'static [(&'static str, &'static str)],
    pub(crate) expected_same_key: bool,
}
