//! The project's orchestration selections, read from disk only when an orchestration rule is selected.

use std::path::Path;

use sqlbuild_discovery::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::models::EvaluateRequest;
use crate::orchestration_rules::_helpers::imports::ImportReader;
use crate::orchestration_rules::_helpers::selections::facts;
use crate::orchestration_rules::constants::NAMED_SELECTOR_LIMIT_CODE;
use crate::orchestration_rules::models::OrchestrationFacts;
use crate::rules::main::request_selects::request_selects;

/// Every job and sensor-condition selection when the request selects an orchestration rule, otherwise none.
pub(crate) fn orchestration_facts(request: &EvaluateRequest) -> Result<OrchestrationFacts, String> {
    if !request_selects(request, |rule| rule.code == NAMED_SELECTOR_LIMIT_CODE)? {
        return Ok(OrchestrationFacts::default());
    }
    let tree = ProjectTree::new(Path::new(&request.project_dir));
    let inventory: OrchestrationInventory = orchestration_inventory(&tree, request.python_version)
        .map_err(|failure| format!("cannot read the orchestration declarations: {failure:?}"))?;
    Ok(facts(
        inventory,
        &ImportReader {
            tree: &tree,
            python_version: request.python_version,
        },
    ))
}
