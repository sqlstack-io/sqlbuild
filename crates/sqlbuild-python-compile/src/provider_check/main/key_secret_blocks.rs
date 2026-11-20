//! The providers missing secrets that a run of exact node keys uses.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_project::graph::models::ProjectSelection;

use crate::provider_check::_helpers::blocks::blocks_for;
use crate::provider_check::_helpers::users::missing_providers;
use crate::provider_check::models::SecretBlock;

/// The providers missing secrets that the run of exactly `selected` node keys uses, with the nodes they stop.
pub fn key_secret_blocks(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    selected: &BTreeSet<String>,
) -> PyResult<Vec<SecretBlock>> {
    let missing = missing_providers(discovered)?;
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    let selection = ProjectSelection::new(project, discovered)?;
    blocks_for((discovered, project), (&selection, selected), missing)
}
