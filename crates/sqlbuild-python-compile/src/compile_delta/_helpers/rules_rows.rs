//! A delta compile's built-in rules request: the previous compile's, with the rerun rows fresh.

use std::collections::HashMap;

use pyo3::PyResult;
use sqlbuild_rules::models::{EvaluateRequest, SqlTestFact};

use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::models::BaseRules;

/// The retained request with each row's payload digest restored.
pub(crate) fn restored(base: BaseRules) -> EvaluateRequest {
    let mut request: EvaluateRequest = base.request;
    for (model, digest) in request.models.iter_mut().zip(base.digests) {
        model.payload_digest = Some(digest);
        model.stripped = true;
    }
    request
}

/// `base` with the facts of every file `fresh` covers replaced, in place and in order.
pub(crate) fn replaced_tests(
    base: &[SqlTestFact],
    fresh: Vec<SqlTestFact>,
) -> PyResult<Vec<SqlTestFact>> {
    let mut by_file: HashMap<String, std::vec::IntoIter<SqlTestFact>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<SqlTestFact>> = HashMap::new();
    for fact in fresh {
        grouped
            .entry(fact.source_path.clone())
            .or_default()
            .push(fact);
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for fact in base {
        *counts.entry(fact.source_path.as_str()).or_default() += 1;
    }
    for (path, facts) in grouped {
        if counts.get(path.as_str()).copied().unwrap_or_default() != facts.len() {
            return Err(delta_unsupported(
                "a rerun SQL test file changed its test count",
            ));
        }
        by_file.insert(path, facts.into_iter());
    }
    Ok(base
        .iter()
        .map(|fact| {
            by_file
                .get_mut(&fact.source_path)
                .and_then(Iterator::next)
                .unwrap_or_else(|| fact.clone())
        })
        .collect())
}
