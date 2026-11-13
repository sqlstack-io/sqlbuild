use crate::project::errors::ConfigFailure;
use crate::project::models::{ChangesConfig, ControlConfig, ReviewConfig};

pub(super) struct ProjectFailureTestCase {
    pub(super) description: &'static str,
    pub(super) project: &'static str,
    pub(super) local: &'static str,
    pub(super) expected_failure: Option<ConfigFailure>,
}

pub(super) struct GuidanceTestCase {
    pub(super) description: &'static str,
    pub(super) rendered: (String, String),
    pub(super) expected_note: &'static str,
    pub(super) expected_help: &'static str,
}

pub(crate) struct GitBaseRefTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: &'static str,
    pub(crate) expected_base_ref: Option<&'static str>,
}

pub(crate) struct ChangesConfigTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: &'static str,
    pub(crate) expected_config: ChangesConfig,
}

pub(crate) struct ControlConfigTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: &'static str,
    pub(crate) expected_config: ControlConfig,
}

pub(crate) struct ReviewConfigTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: &'static str,
    pub(crate) expected_config: ReviewConfig,
}

pub(crate) struct ReviewConfigErrorTestCase {
    pub(crate) description: &'static str,
    pub(crate) project: &'static str,
    pub(crate) expected_fragment: &'static str,
}
