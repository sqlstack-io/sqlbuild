//! The orchestration facts a rules request carries.

use serde::{Deserialize, Serialize};

/// One job `select` or sensor-condition `select`, where it is written and who owns it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OrchestrationSelection {
    pub path: String,
    pub owner: String,
    pub name: String,
    /// The selection text; `None` for a sensor condition whose `select` is not a string literal.
    pub selection: Option<String>,
    /// What the rule could not resolve to reach the selection, when it could not.
    #[serde(default)]
    pub unresolved: Option<String>,
    pub line: u64,
    pub column: u64,
}

/// The selections a project's jobs and sensors make, and the job and job group names a selector may use.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct OrchestrationFacts {
    pub selections: Vec<OrchestrationSelection>,
    pub run_names: Vec<String>,
}
