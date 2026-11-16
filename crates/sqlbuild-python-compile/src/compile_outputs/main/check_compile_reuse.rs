//! Check the stored compile for this invocation, replaying it on a hit.

use crate::compile_outputs::_helpers::reuse::native_digest;
use crate::compile_outputs::_helpers::reuse::store_slot;
use crate::compile_outputs::_helpers::reuse::texts;
use crate::compile_outputs::models::NativeReuseAttempt;
use crate::compile_outputs::models::ReuseCheckPy;
use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_cache::project_reuse::main::check_reuse::check_reuse;
use sqlbuild_cache::project_reuse::models::{HostIdentity, ReuseAttempt, ReuseCheck, ReuseRules};
use sqlbuild_cache::project_snapshot::main::path_text::path_text;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::sync::Mutex;

/// Check the stored compile for this invocation, replaying it on a hit.
#[pyfunction]
pub fn check_compile_reuse(py: Python<'_>, check: ReuseCheckPy) -> PyResult<NativeReuseAttempt> {
    compiler_guard(|| {
        let rules: ReuseRules = check.rules.into();
        let request: ReuseCheck<'_> = ReuseCheck {
            project_dir: check.project_dir,
            store_path: store_slot(&check.store_directory, check.selected_target.as_deref()),
            identity: HostIdentity {
                invocation_digest: native_digest(&check.invocation),
                runtime: check
                    .runtime
                    .iter()
                    .map(|(name, value)| (name.clone(), path_text(value)))
                    .collect(),
                search_path: texts(&check.search_path),
            },
            bypass_requested: check.bypass_requested,
            json_output: check.json_output,
            rules: &rules,
        };
        let attempt: ReuseAttempt = py.detach(|| check_reuse(request));
        Ok(NativeReuseAttempt {
            attempt: Mutex::new(attempt),
        })
    })
}
