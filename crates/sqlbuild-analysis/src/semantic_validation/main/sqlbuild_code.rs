/// SQLBuild's code for an analysis finding; `None` for a code SQLBuild does not report.
#[must_use]
pub fn sqlbuild_code(code: &str) -> Option<String> {
    crate::semantic_validation::_helpers::diagnostics::sqlbuild_code(code)
}
