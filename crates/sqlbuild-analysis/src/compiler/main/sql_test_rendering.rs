use crate::compiler::_helpers;

pub fn render_json(request_json: &str) -> Result<String, String> {
    _helpers::sql_tests::rendering::render_json(request_json)
}
