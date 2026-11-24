//! Whether one build lets failing audits hold nothing back: the flag, the target, then `[settings]`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::effective::main::effective_ignore_audit_failures::effective_ignore_audit_failures;
use sqlbuild_config::effective::models::AuditFailureLayers;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::command_runtime::main::target_and_settings_value::target_and_settings_value;

const IGNORE_AUDIT_FAILURES: &str = "ignore_audit_failures";

/// `request`'s flag (a job run's settings arrive as it), else the selected target's, else `[settings]`.
pub(crate) fn resolve_ignore_audit_failures(
    request: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let (target, settings) =
        target_and_settings_value(discovered, selected_target, IGNORE_AUDIT_FAILURES)?;
    let target: Option<bool> = target.map(|value| value.extract()).transpose()?.flatten();
    let settings: bool = settings.extract()?;
    Ok(effective_ignore_audit_failures(&AuditFailureLayers {
        requested: native_field(request, IGNORE_AUDIT_FAILURES)?.extract()?,
        target,
        settings,
    }))
}
