//! The Polyglot parse options SQLBuild's wheel proxy applies to every parse.

use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use polyglot_sql::ParseOptions;

/// The proxy's complexity guard, decoded as the wheel decodes it.
pub(crate) fn guarded_parse_options() -> Result<ParseOptions, serde_json::Error> {
    Ok(polyglot_parse_options())
}
