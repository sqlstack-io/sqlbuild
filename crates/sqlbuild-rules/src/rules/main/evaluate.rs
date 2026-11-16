use crate::models::Fault;
use crate::rules::models::ModelEvaluationRequest;

pub(crate) fn evaluate_model(request: ModelEvaluationRequest<'_>) -> Result<Vec<Fault>, String> {
    crate::rules::_helpers::dispatch::evaluation::evaluate_model(request)
}
