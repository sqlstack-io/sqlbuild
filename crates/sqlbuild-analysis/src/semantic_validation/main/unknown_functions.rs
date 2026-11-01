use std::sync::LazyLock;

use polyglot_sql::DialectType;

use crate::semantic_validation::_helpers::function_names::unknown_calls;
use crate::semantic_validation::models::FunctionProbes;

static PROBES: LazyLock<FunctionProbes> = LazyLock::new(FunctionProbes::default);

/// The names of calls in `sql` that are not built-ins of `dialect` and not `declared`, in order.
pub fn unknown_functions(
    sql: &str,
    dialect: &str,
    declared: &[String],
) -> Result<Vec<String>, String> {
    let dialect: DialectType = dialect
        .parse::<DialectType>()
        .map_err(|error| error.to_string())?;
    Ok(unknown_calls(sql, dialect, (&PROBES, declared)))
}
