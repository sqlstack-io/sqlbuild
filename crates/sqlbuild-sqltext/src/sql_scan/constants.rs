//! Lexical vocabulary shared by SQL scanning.

/// Characters that may prefix a string literal, such as `E'...'` or `r'...'`.
pub(crate) const STRING_PREFIX_CHARACTERS: &[u8] = b"bBeErR";
/// Longest string-literal prefix, such as `rb`.
pub(crate) const STRING_PREFIX_MAX_LENGTH: usize = 2;
/// Length of a triple-quote string delimiter.
pub(crate) const TRIPLE_QUOTE_LENGTH: usize = 3;
/// Line comment prefixes SQLBuild's scanners understand.
pub const SUPPORTED_LINE_COMMENT_PREFIXES: [&str; 3] = ["#", "--", "//"];
/// The generic SQL line comment prefix.
pub const GENERIC_LINE_COMMENT_PREFIX: &str = "--";
/// Quote characters a triple-quoted string may repeat.
pub const TRIPLE_QUOTES: [&str; 2] = ["'''", "\"\"\""];
pub const ESCAPE_CHARACTER: char = '\\';
pub const BLOCK_COMMENT_OPEN: &str = "/*";
/// What a lexical key adds for bracket identifiers.
pub(crate) const BRACKET_IDENTIFIERS_KEY: &str = "brackets";
/// The wrapper Snowflake takes around a table function call in FROM.
pub const TABLE_WRAPPER: &str = "TABLE(";
