use crate::compiler::_helpers;

pub fn render_json(request_json: &str) -> Result<String, String> {
    _helpers::sql_tests::rendering::render_json(request_json)
}

/// One planned test's comparison SQL with each CTE body's character range and origin, as JSON.
pub fn render_placed_json(request_json: &str) -> Result<String, String> {
    _helpers::sql_tests::rendering::render_placed_json(request_json)
}
