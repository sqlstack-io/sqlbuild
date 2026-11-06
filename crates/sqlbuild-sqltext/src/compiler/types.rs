use crate::sql_scan::models::LexicalSyntax;

/// The project variables, process environment and context values one interpolation reads.
pub trait InterpolationHost {
    /// The text of project variable `name`, `Err(message)` when it cannot be text, or `None`.
    fn variable(&self, name: &str) -> Option<Result<String, String>>;
    /// Every project variable name, sorted.
    fn variable_names(&self) -> Vec<String>;
    /// The value of environment variable `name`, or `Err(message)` when it cannot be read.
    fn environment(&self, name: &str) -> Result<Option<String>, String>;
    /// Whether this SQL may use `@@CTX:` tokens.
    fn context_allowed(&self) -> bool;
    /// `Some(value)` for a known context key, whose value may be unavailable (`None`).
    fn context(&self, name: &str) -> Option<Option<String>>;
    /// Every known context key.
    fn context_names(&self) -> Vec<String>;
    /// The lexical rules the SQL is authored in; `None` reads it under the generic compiler rules.
    fn lexical_syntax(&self) -> Option<&LexicalSyntax> {
        None
    }
}
