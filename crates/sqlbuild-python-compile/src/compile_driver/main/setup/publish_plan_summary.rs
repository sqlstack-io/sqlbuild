//! Publish the planned phase with what the plan will run, in the parts of its `Plan ready` line.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::plan_display::main::plan_summary_parts::plan_summary_parts;
use sqlbuild_core::plan_display::models::PlanSummaryCounts;
use sqlbuild_python_base::invocation_phases::main::publish_planned_phase::publish_planned_phase;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Publish the held planned phase for `plan_output` and its `python_entries`, such as `3 selected · 1 Python node`.
pub fn publish_plan_summary(
    plan_output: &Bound<'_, PyAny>,
    python_entries: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let count = |name: &str| -> PyResult<usize> { native_field(plan_output, name)?.len() };
    let mut reloads_sources: bool = false;
    for load in native_field(plan_output, "source_load_entries")?.try_iter()? {
        reloads_sources |= native_field(&load?, "is_reload")?.is_truthy()?;
    }
    let counts: PlanSummaryCounts = PlanSummaryCounts {
        full_refresh: false,
        selected: count("model_entries")? + count("seed_entries")? + count("function_entries")?,
        sources: count("source_load_entries")?,
        reloads_sources,
        python: python_entries.len()?,
        audits: count("selected_audit_names")?,
        audits_not_run: count("audits_not_run")?,
    };
    publish_planned_phase(
        plan_output.py(),
        (
            plan_summary_parts(&counts).join(" · "),
            counts.selected + counts.sources + counts.python,
        ),
    )
}
