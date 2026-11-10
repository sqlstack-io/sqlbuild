//! DSL header kinds, rule codes and the text the header rules read and write.

pub const HEADER_KINDS: [&str; 10] = [
    "AUDIT", "CONSTANT", "ENUM", "FUNCTION", "HOOK", "MACRO", "MODEL", "SCENARIO", "SCHEMA", "TEST",
];
pub const MEASUREMENT_KINDS: [&str; 2] = ["EVIDENCE", "MEASURE"];
pub(crate) const DESCRIPTION_HEADER_KINDS: [&str; 2] = ["MODEL", "SCENARIO"];
/// Headers whose top-level description `sqb format` lays out; `MACRO` is laid out after its header.
pub(crate) const PROSE_LAYOUT_HEADER_KINDS: [&str; 5] =
    ["FUNCTION", "HOOK", "MODEL", "SCENARIO", "SCHEMA"];
pub(crate) const DESCRIPTION_REQUIRED_HEADER_KINDS: [&str; 4] =
    ["MODEL", "SCENARIO", "FUNCTION", "HOOK"];
pub(crate) const MODEL_KIND: &str = "MODEL";
/// The SQL macro header, whose typed arguments the compiler parses and checks, not these rules.
pub(crate) const SQL_MACRO_HEADER_KIND: &str = "MACRO";
pub const RULE_DESCRIPTION_PRESENT: &str = "description-present";
pub const RULE_DESCRIPTION_LENGTH: &str = "description-length";
pub const RULE_LEADING_COMMENT_DESCRIPTION: &str = "leading-comment-description";
pub const RULE_HEADER_WHITESPACE: &str = "header-whitespace";
pub const RULE_HEADER_PARSE: &str = "header-parse";
pub(crate) const DESCRIPTION_KEY: &str = "description";
pub(crate) const HEADER_INDENT: &str = "  ";
pub(crate) const HEADER_TERMINATOR: &str = ";";
pub(crate) const AUDITS_KEYWORD: &str = "audits";
pub(crate) const RETURNS_KEYWORD: &str = "returns";
pub(crate) const TABLE_RETURNS_KEYWORD: &str = "table";
pub(crate) const AUDIT_WRAP_MAX_PASSES: usize = 64;
pub(crate) const QUOTES: [char; 2] = ['\'', '"'];
