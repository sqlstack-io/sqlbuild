//! Structured failures of reading configuration files.

/// Why a configuration document could not be loaded natively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// The text is not a valid document; Python's parser rejects it too.
    Syntax,
    /// The document parses but a value cannot be built from it.
    Construct,
    /// The document uses a feature the native reader leaves to Python.
    Unsupported,
}

/// A configuration failure, at its one-based line and column where known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub kind: ConfigErrorKind,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl ConfigError {
    /// An error without a position.
    pub fn new(kind: ConfigErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// This error at a one-based line and column, unless it already has a position.
    pub fn at(self, line: usize, column: usize) -> Self {
        if self.line.is_some() {
            return self;
        }
        Self {
            line: Some(line),
            column: Some(column),
            ..self
        }
    }
}
