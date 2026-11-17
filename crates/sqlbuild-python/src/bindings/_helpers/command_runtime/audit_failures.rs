//! Whether one build lets failing audits hold nothing back: the flag, the target, then `[settings]`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::effective::main::effective_ignore_audit_failures::effective_ignore_audit_failures;
use sqlbuild_config::effective::models::AuditFailureLayers;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings::build_effective_settings;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_front::contracts::main::resolve_target_name::resolve_target_name;

const IGNORE_AUDIT_FAILURES: &str = "ignore_audit_failures";

/// `request`'s flag (a job run's settings arrive as it), else the selected target's, else `[settings]`.
pub(crate) fn resolve_ignore_audit_failures(
    request: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let py = request.py();
    let project_config: Bound<'_, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(discovered, "local_config")?;
    let target: Option<bool> =
        match resolve_target_name(py, &project_config, &local_config, selected_target)? {
            Some(name) => native_field(
                &resolve_target_config(py, &project_config, &local_config, name.as_any())?,
                IGNORE_AUDIT_FAILURES,
            )?
            .extract()?,
            None => None,
        };
    let settings: bool = native_field(
        &build_effective_settings(&project_config, &local_config)?,
        IGNORE_AUDIT_FAILURES,
    )?
    .extract()?;
    Ok(effective_ignore_audit_failures(&AuditFailureLayers {
        requested: native_field(request, IGNORE_AUDIT_FAILURES)?.extract()?,
        target,
        settings,
    }))
}
