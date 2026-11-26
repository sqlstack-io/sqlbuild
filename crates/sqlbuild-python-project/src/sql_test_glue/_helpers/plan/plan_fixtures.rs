//! Complete SQL-test fixtures through the native fixture validation boundary.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::planner_strategy::main::prewarm_fixture_reads::prewarm_fixture_reads;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::sql_test_glue::_helpers::inputs::fixture_validation::validated_fixtures;
use crate::sql_test_glue::_helpers::plan::plan_driver::resolve_sql_test_model_chains;
use crate::sql_test_glue::_helpers::plan::plan_values::model_payload;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_front::sql_test_glue::main::prefixed_ctes::prefixed_ctes;

pub(crate) fn validated_test<'py>(
    test: &Bound<'py, PyAny>,
    chain: &Bound<'py, PyAny>,
    inputs: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    context: &RelationFixturePlanningContext,
) -> PyResult<Result<Bound<'py, PyAny>, Vec<String>>> {
    let py = test.py();
    let payload = native_field(test, "payload")?;
    let ctes = native_field(&payload, "authored_ctes")?;
    let expected = native_field(&payload, "expected_ctes")?;
    let completed = match validated_fixtures(
        test,
        (
            chain,
            [
                prefixed_ctes(&ctes, "__ref__")?,
                prefixed_ctes(&ctes, "__source__")?,
                prefixed_ctes(&ctes, "__seed__")?,
                prefixed_ctes(&expected, "__expected__")?,
            ],
        ),
        inputs,
        context,
    )? {
        Ok(completed) => completed,
        Err(diagnostics) => return Ok(Err(diagnostics)),
    };
    let mut authored: Vec<Bound<'py, PyAny>> = Vec::new();
    for cte in ctes.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        let mut replacement = cte.clone();
        for (index, prefix) in ["__ref__", "__source__", "__seed__"].iter().enumerate() {
            if let Some(name) = name.strip_prefix(prefix) {
                let sql = mapping_get_or(&completed[index], name, native_field(&cte, "sql_body")?)?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("sql_body", sql)?;
                replacement = replaced_keywords(&cte, &kwargs)?;
                break;
            }
        }
        authored.push(replacement);
    }
    let mut expected_ctes: Vec<Bound<'py, PyAny>> = Vec::new();
    for cte in expected.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        let name = name.strip_prefix("__expected__").unwrap_or(&name);
        let kwargs = PyDict::new(py);
        kwargs.set_item(
            "sql_body",
            mapping_get_or(&completed[3], name, native_field(&cte, "sql_body")?)?,
        )?;
        expected_ctes.push(replaced_keywords(&cte, &kwargs)?);
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("authored_ctes", PyTuple::new(py, authored)?)?;
    kwargs.set_item("expected_ctes", PyTuple::new(py, expected_ctes)?)?;
    let payload = replaced_keywords(&payload, &kwargs)?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("payload", payload)?;
    replaced_keywords(test, &kwargs).map(Ok)
}

pub(crate) fn validate<'py>(
    tests: &[Bound<'py, PyAny>],
    inputs: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    context: &RelationFixturePlanningContext,
) -> PyResult<crate::sql_test_glue::types::ValidatedTests<'py>> {
    let py = inputs.0.py();
    let mut model_tests: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut indexes: Vec<usize> = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        if model_payload(test)? {
            indexes.push(index);
            model_tests.push(test.clone());
        }
    }
    if indexes.is_empty() {
        return Ok((tests.to_vec(), Default::default()));
    }
    let chains =
        resolve_sql_test_model_chains(inputs.0, &PyTuple::new(py, model_tests)?.into_any())?;
    let chains: Vec<Bound<'py, PyAny>> = chains.extract()?;
    if chains.len() != indexes.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "zip() argument lengths differ",
        ));
    }
    let mut names: Vec<String> = Vec::new();
    for chain in &chains {
        names.extend(chain.extract::<Vec<String>>()?);
    }
    names.sort_unstable();
    names.dedup();
    prewarm_fixture_reads(context, &names, &Dialect::of(inputs.1)?)?;
    let mut validated = tests.to_vec();
    let mut diagnostics: std::collections::BTreeMap<usize, Bound<'py, PyAny>> =
        std::collections::BTreeMap::new();
    for (index, chain) in indexes.into_iter().zip(chains) {
        match validated_test(&tests[index], &chain, inputs, context)? {
            Ok(test) => validated[index] = test,
            Err(errors) => {
                diagnostics.insert(index, PyTuple::new(py, errors)?.into_any());
            }
        }
    }
    Ok((validated, diagnostics))
}
