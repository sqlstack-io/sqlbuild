use crate::load_command::models::LoadKinds;

pub(super) struct LoadSelectorsTestCase {
    pub(super) description: &'static str,
    pub(super) selectors: &'static [&'static str],
    pub(super) kinds: LoadKinds,
    pub(super) expected_selectors: &'static [&'static str],
}
