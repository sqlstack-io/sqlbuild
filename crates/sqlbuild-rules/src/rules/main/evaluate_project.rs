use crate::models::Fault;
use crate::orchestration_rules::constants::NAMED_SELECTOR_LIMIT_CODE;
use crate::orchestration_rules::main::evaluate_orchestration_rules::evaluate_orchestration_rules;
use crate::rules::models::ProjectEvaluationRequest;

pub(crate) fn evaluate_project(
    request: ProjectEvaluationRequest<'_>,
) -> Result<Vec<Fault>, String> {
    let mut faults = crate::rules::_helpers::layers::domain_layout::evaluate_project(&request);
    faults.extend(crate::rules::_helpers::dispatch::evaluation::evaluate_project_rules(&request));
    if let Some(rule) = request.selected.get(NAMED_SELECTOR_LIMIT_CODE) {
        faults.extend(evaluate_orchestration_rules(request.request, rule));
    }
    faults.extend(crate::rules::_helpers::sql_checks::sql_test_rules::evaluate_project(request)?);
    Ok(faults)
}
