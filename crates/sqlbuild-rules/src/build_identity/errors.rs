//! Why a build identity cannot be installed.

use std::fmt;

/// The reason `install_build_identity` refused an identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildIdentityError {
    /// The offered identity is empty, so it would match builds of any sources.
    Empty,
    /// A different identity is already installed in this process.
    Conflicting { installed: String, offered: String },
}

impl fmt::Display for BuildIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("the native build identity is empty"),
            Self::Conflicting { installed, offered } => write!(
                formatter,
                "the native build identity {installed} is already installed; refusing {offered}"
            ),
        }
    }
}

impl std::error::Error for BuildIdentityError {}
