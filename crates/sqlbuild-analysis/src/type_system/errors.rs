//! Why type normalization fails.

/// Why `normalize_type` raises instead of returning a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeNormalizationError {
    /// SQLBuild has no type rules for the dialect name: Python's `ValueError`.
    UnknownDialect(String),
}

impl TypeNormalizationError {
    /// The message Python's exception carries.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownDialect(name) => format!("Unknown dialect: {name}"),
        }
    }
}
