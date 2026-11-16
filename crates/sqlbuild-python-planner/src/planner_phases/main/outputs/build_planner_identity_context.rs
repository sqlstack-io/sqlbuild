//! Build planner identity context.

use crate::planner_phases::_helpers::inputs::contexts::class_field;
use crate::planner_phases::_helpers::outputs::identities::version_identities;
use crate::planner_phases::models::PlannerIdentityContext;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;

/// Build planner identity context.
pub fn build_planner_identity_context<'py>(
    project: &Bound<'py, PyAny>,
    (scopes, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    include_stale_warning_identities: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let recorded_fingerprints: Bound<'py, PyAny> = native_field(snapshot, "fingerprints")?;
    let identities = version_identities(
        project,
        class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
        &recorded_fingerprints,
    )?;
    let stale = if include_stale_warning_identities {
        version_identities(
            project,
            class_field(scopes, |s: &PlannerScopeResolution| &s.stale_warning_scope)?,
            &recorded_fingerprints,
        )?
    } else {
        identities.clone()
    };
    Ok(Bound::new(
        project.py(),
        PlannerIdentityContext {
            version_identities: identities.unbind(),
            stale_warning_identities: stale.unbind(),
        },
    )?
    .into_any())
}
