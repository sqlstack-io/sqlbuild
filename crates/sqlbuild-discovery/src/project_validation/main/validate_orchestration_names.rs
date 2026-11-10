//! Orchestration declarations share the project's one resource namespace.

use crate::project_validation::_helpers::resource_names::validate_unique_project_resource_names;
use crate::project_validation::models::{ProjectFacts, ValidationFailure};

/// Reject the first name a job, group, sensor or calendar schedule shares with anything else.
pub fn validate_orchestration_names(facts: &ProjectFacts) -> Result<(), ValidationFailure> {
    validate_unique_project_resource_names(facts)
}
