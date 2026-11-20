//! Function argument and return types parsed with the adapter's SQL analysis dialect.

use polyglot_sql::Dialect;

use crate::compiler::constants::FUNCTION_TYPE_DIALECTS;
use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;

/// The error for a type the adapter's dialect cannot parse; adapters without a dialect pass.
pub(crate) fn function_type_error(
    type_sql: &str,
    adapter_name: &str,
    context: &str,
) -> Result<Option<String>, String> {
    let Some((_, dialect_name, dialect)) = FUNCTION_TYPE_DIALECTS
        .iter()
        .find(|(adapter, _, _)| *adapter == adapter_name)
    else {
        return Ok(None);
    };
    Ok(Dialect::get(*dialect)
        .parse_data_type_with_options(type_sql, &polyglot_parse_options())
        .err()
        .map(|error| {
            format!(
                "{context} type '{type_sql}' is not valid for adapter '{adapter_name}' SQL \
                 analysis dialect '{dialect_name}': {error}"
            )
        }))
}
