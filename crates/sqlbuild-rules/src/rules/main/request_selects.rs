use crate::models::{EvaluateRequest, RuleMetadata};
use crate::rules::main::assemble_catalogue::assemble_catalogue;
use crate::rules::main::select::select;

/// Whether the request's configuration selects a rule that `matches`.
pub(crate) fn request_selects(
    request: &EvaluateRequest,
    matches: impl Fn(&RuleMetadata) -> bool,
) -> Result<bool, String> {
    let catalogue = assemble_catalogue(&request.custom_rules)?;
    let selected = select(
        &catalogue,
        request.config.select.as_deref(),
        &request.config.ignore,
    )?;
    Ok(selected.into_iter().any(matches))
}
