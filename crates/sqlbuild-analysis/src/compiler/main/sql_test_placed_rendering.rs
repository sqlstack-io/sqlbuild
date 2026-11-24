use crate::compiler::_helpers;

/// One planned test's comparison SQL and each CTE body's start, end and origin in it.
pub fn render_placed_comparison(
    request_json: &str,
) -> Result<(String, Vec<(usize, usize, String)>), String> {
    _helpers::sql_tests::rendering::render_placed_request(request_json)
}
