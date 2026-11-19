use crate::assembly::dynamic_checks::models::DynamicModel;

pub(crate) struct NeverRefreshedTestCase {
    pub(crate) description: &'static str,
    pub(crate) models: Vec<DynamicModel>,
    pub(crate) expected_name: Option<&'static str>,
}
