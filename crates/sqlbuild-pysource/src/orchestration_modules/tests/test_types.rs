use crate::orchestration_modules::models::ModuleDeclarations;

pub(super) struct ScanTestCase {
    pub(super) description: &'static str,
    pub(super) contents: &'static str,
    pub(super) expected_module: Option<ModuleDeclarations>,
}
