//! What `plan --changed` previews: what a build would run now, read without recording anything.

use crate::consumption::_helpers::facts::{BuildFacts, build_facts};
use crate::consumption::_helpers::recorder::changed_decision;
use crate::consumption::_helpers::recorder::read_history;
use crate::consumption::models::ChangedDecision;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_front::native_host::models::ConsumptionConnection;
use sqlbuild_state::consumption::models::{ChangedPolicy, ConsumptionHistory};

/// What `plan --changed` previews: what a build would run now, read without recording anything.
pub fn preview_changed(
    (plan, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (policy, full_refresh): (&ChangedPolicy, bool),
) -> PyResult<(ChangedDecision, Vec<String>)> {
    let py: Python<'_> = plan.py();
    let facts: BuildFacts = build_facts((plan, project), (true, full_refresh))?;
    let connection = ConsumptionConnection::new(adapter, connection_config);
    let (history, warnings): (ConsumptionHistory, Vec<String>) =
        read_history(py, &connection, (&facts, Some(policy)));
    connection.close(py)?;
    Ok((changed_decision(&facts, &history, policy), warnings))
}
