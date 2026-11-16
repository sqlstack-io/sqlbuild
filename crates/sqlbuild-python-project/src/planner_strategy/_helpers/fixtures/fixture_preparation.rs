//! Inference and schema preparation at the fixture Python boundary.

use crate::planner_strategy::main::complete_empty_fixture_sql::complete_empty_fixture_sql;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::planner_fixtures::main::empty_fixture_marker::empty_fixture_marker;

use crate::planner_strategy::_helpers::fixtures::fixture_values::{diagnostic, label};
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::planner_strategy::types::FixtureKey;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) struct PreparedFixtures<'py> {
    /// Each fixture's key with its Python key, in request order.
    pub(crate) keys: Vec<(FixtureKey, Bound<'py, PyAny>)>,
    pub(crate) sql: Bound<'py, PyDict>,
    pub(crate) inferred: Bound<'py, PyDict>,
    pub(crate) null_names: Bound<'py, PyDict>,
    pub(crate) quoted_names: Bound<'py, PyDict>,
    pub(crate) diagnostics: Vec<Bound<'py, PyAny>>,
}

pub(crate) fn prepare<'py>(
    groups: &Bound<'py, PyAny>,
    context: &RelationFixturePlanningContext,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<PreparedFixtures<'py>> {
    let py = groups.py();
    let inferences = context.fixture_inferences.bind(py);
    let mut prepared = PreparedFixtures {
        keys: Vec::new(),
        sql: PyDict::new(py),
        inferred: PyDict::new(py),
        null_names: PyDict::new(py),
        quoted_names: PyDict::new(py),
        diagnostics: Vec::new(),
    };
    for group in groups.try_iter()? {
        let group = group?;
        let kind = group.get_item(0)?;
        let fixtures = group.get_item(1)?.cast_into::<PyDict>()?;
        for (name, sql) in fixtures.iter() {
            let key = PyTuple::new(py, [&kind, &name])?.into_any();
            let fixture_key: FixtureKey = key.extract()?;
            prepared.keys.push((fixture_key.clone(), key.clone()));
            let mut sql: String = sql.extract()?;
            if empty_fixture_marker(&sql).map_err(value_error)? {
                let (empty_sql, error) =
                    complete_empty_fixture_sql(&sql, context.relations.get(&fixture_key), adapter)?;
                if let Some(error) = error {
                    prepared
                        .diagnostics
                        .push(diagnostic(&key, &format!("{} {error}", label(&key)?))?);
                    prepared.sql.set_item(&key, &sql)?;
                    continue;
                }
                sql = empty_sql;
            }
            prepared.sql.set_item(&key, &sql)?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("query_sql", &sql)?;
            kwargs.set_item(
                "inference_profile",
                adapter.call_method0("expression_inference_profile")?,
            )?;
            let inference = inferences.call_method("infer", (), Some(&kwargs))?;
            if !inference.is_none() {
                prepared
                    .inferred
                    .set_item(&key, native_field(&inference, "columns")?)?;
                prepared
                    .null_names
                    .set_item(&key, native_field(&inference, "null_literal_names")?)?;
                prepared
                    .quoted_names
                    .set_item(&key, native_field(&inference, "quoted_names")?)?;
            }
        }
    }
    Ok(prepared)
}
