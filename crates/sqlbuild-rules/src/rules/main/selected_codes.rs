pub fn selected_codes_json(request_json: &str) -> Result<String, String> {
    crate::rules::_helpers::dispatch::catalogue::selected_codes_json(request_json)
}
