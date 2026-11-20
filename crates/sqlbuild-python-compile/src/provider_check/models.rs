//! A provider whose missing secrets stop part of a selection.

/// One provider whose missing secrets stop part of a selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretBlock {
    pub provider: String,
    /// The provider file, relative to the project.
    pub path: String,
    /// The unset variables, sorted.
    pub variables: Vec<String>,
    /// Selected node keys that run the provider, such as `source:raw_payments`; sinks as `sink:<name>`.
    pub users: Vec<String>,
    /// Selected node keys downstream of the users, which can't run without them.
    pub readers: Vec<String>,
}
