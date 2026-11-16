//! One Python rule pass: the selected Fensu-backed rules over project Python, or over its tests.

use std::collections::BTreeMap;

use fensu_native::NativeProjectPlane;

use crate::models::{EvaluateRequest, PythonSource};
use crate::python_rules::_helpers::engine::contexts::context_settings;
use crate::python_rules::_helpers::engine::execution::{evaluated_files, file_plans};
use crate::python_rules::_helpers::engine::file_cache::project_digest;
use crate::python_rules::_helpers::engine::placement::{python_targets, test_targets};
use crate::python_rules::_helpers::engine::programs::Programs;
use crate::python_rules::_helpers::engine::project_plane::loaded_project;
use crate::python_rules::models::{
    CallInputs, PythonCacheEntry, PythonEvaluation, PythonFiles, PythonRuleSpec, PythonTarget,
};

/// Cache-aware findings of the `selected` rules over project Python tests, or the rest of project Python.
pub(crate) fn evaluated_pass(
    request: &EvaluateRequest,
    (selected, tests): (&[&'static PythonRuleSpec], bool),
    cached: &BTreeMap<String, PythonCacheEntry>,
    fingerprint: &str,
) -> Result<PythonEvaluation, String> {
    let rules: Vec<&PythonRuleSpec> = selected
        .iter()
        .copied()
        .filter(|rule| (rule.applies == PythonFiles::Tests) == tests)
        .collect();
    let sources: &[PythonSource] = if tests {
        &request.python_test_sources
    } else {
        &request.python_sources
    };
    if rules.is_empty() || sources.is_empty() {
        return Ok(PythonEvaluation::default());
    }
    let programs = Programs::new(sources, request.python_version);
    let targets: Vec<PythonTarget> = if tests {
        test_targets(sources)
    } else {
        python_targets(sources)
    };
    let (plane, entrypoints) = if tests {
        (NativeProjectPlane::default(), Vec::new())
    } else {
        loaded_project(&rules, &targets, &programs, request)?
    };
    let inputs = CallInputs {
        request,
        settings: context_settings(request, &rules, entrypoints),
        programs: &programs,
        cached,
        fingerprint,
        project_digest: project_digest(sources),
    };
    evaluated_files(file_plans(&targets, &rules, &inputs)?, &inputs, &plane)
}
