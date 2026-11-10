use crate::compiled_project::models::CompiledProjectFacts;

/// Mark a recorded model as skipped by its native parser; `false` when it was never recorded.
pub fn record_skipped_analysis(project: &mut CompiledProjectFacts, name: &str) -> bool {
    project.record_skipped(name)
}
