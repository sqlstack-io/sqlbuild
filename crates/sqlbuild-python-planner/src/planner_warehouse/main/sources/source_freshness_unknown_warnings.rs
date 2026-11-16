//! One warning per reason that left configured source freshness unknown.

use crate::freshness::constants::PREVIEW_MESSAGE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_warehouse::main::freshness_unknown_message::freshness_unknown_message;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_phases::main::native::native;
use sqlbuild_python_project::planner_phases::main::text::text;
use sqlbuild_python_project::planner_phases::main::tuple::tuple;

/// One warning per reason that left configured source freshness unknown.
pub fn source_freshness_unknown_warnings<'py>(
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = source_freshness.py();
    let mut reasons: Vec<(String, Vec<String>)> = Vec::new();
    for unknown in native_field(source_freshness, "unknown_sources")?
        .call_method0("values")?
        .try_iter()?
    {
        let unknown = unknown?;
        let reason: String = native_field(&unknown, "reason")?.str()?.extract()?;
        if freshness_unknown_message(&reason, &[]).is_none()
            || native_field(&unknown, "message")?.eq(PREVIEW_MESSAGE)?
        {
            continue;
        }
        let name: String = native_field(&unknown, "source_name")?.extract()?;
        match reasons.iter_mut().find(|(existing, _)| *existing == reason) {
            Some((_, names)) => names.push(name),
            None => reasons.push((reason, vec![name])),
        }
    }
    let mut warnings: Vec<_> = Vec::new();
    for (reason, names) in reasons {
        if let Some(message) = freshness_unknown_message(&reason, &names) {
            warnings.push(native(
                py,
                RecordId::PlanWarning,
                &[
                    ("model_name", py.None().into_bound(py)),
                    (
                        "severity",
                        enum_member(py, RecordId::WarningSeverity, "WARNING")?,
                    ),
                    ("message", text(py, &message)),
                ],
            )?);
        }
    }
    tuple(py, warnings)
}
