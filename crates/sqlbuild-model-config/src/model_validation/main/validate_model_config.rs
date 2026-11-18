//! Validate a model's effective config, raising the first error the Python validators raise.

use crate::model_validation::_helpers::config::ConfigView;
use crate::model_validation::_helpers::incremental::check_incremental;
use crate::model_validation::_helpers::materialization::{
    check_contract, check_custom_materialization, check_execution_limit, check_maintained,
    check_migration, check_non_incremental, check_physical_storage, check_placeholders,
    check_project_capability, check_review_marker, check_storage_policies,
};
use crate::model_validation::_helpers::references::check_references;
use crate::model_validation::_helpers::snapshot::check_snapshot;
use crate::model_validation::models::{
    ModelValidationFacts, ProjectValidationFacts, ValidationStop,
};
use crate::types::AuthoredNode;

/// Run the model validators in order, stopping at the first problem.
pub fn validate_model_config<N: AuthoredNode>(
    entries: Vec<(N, N)>,
    project: &ProjectValidationFacts,
    facts: &ModelValidationFacts<'_>,
) -> Result<(), ValidationStop> {
    let config = ConfigView::new(entries, facts.model_name);
    check_references(facts, project)?;
    check_incremental(&config, facts)?;
    check_project_capability(&config, project)?;
    check_contract(&config)?;
    check_execution_limit(&config)?;
    check_review_marker(&config)?;
    check_non_incremental(&config)?;
    check_snapshot(&config, facts)?;
    check_maintained(&config, project)?;
    check_custom_materialization(&config, project)?;
    check_storage_policies(&config, facts)?;
    check_physical_storage(&config)?;
    check_migration(&config, facts)?;
    check_placeholders(&config, project, facts)
}
