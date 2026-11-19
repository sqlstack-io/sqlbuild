use crate::assembly::maintained_checks::models::MaintainedModel;

pub(crate) struct NeverRefreshedTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: Vec<MaintainedModel>,
    pub(crate) expected_name: Option<&'static str>,
}
