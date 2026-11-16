use crate::models::EvaluateRequest;

pub(crate) fn annotate(request: EvaluateRequest) -> EvaluateRequest {
    crate::rules::_helpers::sql_checks::sql_test_rules::annotate_empty_input_only_tests(request)
}
