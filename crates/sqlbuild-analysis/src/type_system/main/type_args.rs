//! The scalar arguments of a parsed type.

use serde_json::{Map, Value};

use crate::type_system::_helpers::parsed::{parse_type, type_args as tree_args};
use crate::type_system::_helpers::spelling::SpellingDialect;

/// The tag, name, length, precision and scale of `type_sql` in `dialect`, or the parse error.
pub fn type_args(type_sql: &str, dialect: &str) -> Result<Map<String, Value>, String> {
    let spelling: SpellingDialect =
        SpellingDialect::named(dialect).ok_or_else(|| format!("unknown dialect '{dialect}'"))?;
    parse_type(type_sql, spelling).map(|tree| tree_args(&tree))
}
