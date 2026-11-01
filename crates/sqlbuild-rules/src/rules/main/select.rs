use crate::models::RuleMetadata;

pub(crate) fn select<'a>(
    catalogue: &'a [RuleMetadata],
    selected: Option<&[String]>,
    ignored: &[String],
) -> Result<Vec<&'a RuleMetadata>, String> {
    crate::rules::_helpers::catalogue::select(catalogue, selected, ignored)
}
