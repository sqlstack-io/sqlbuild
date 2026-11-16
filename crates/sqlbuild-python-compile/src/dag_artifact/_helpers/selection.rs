//! DAG artifact loading and SQLBuild selectors resolved over a DAG artifact mapping.

use sqlbuild_python_base::native_host::main::host_error::host_error;
use std::collections::BTreeSet;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};
use sqlbuild_analysis::dag_artifact::errors::DagSelectionError;
use sqlbuild_analysis::dag_artifact::main::resolve_dag_selection::resolve_dag_selection as resolve;
use sqlbuild_analysis::dag_artifact::models::{
    DagSelectionCheck, DagSelectionGraph, DagSelectionNode,
};
use sqlbuild_analysis::planner_selection::main::unit_test_selector_rejection::unit_test_selector_rejection;
use sqlbuild_host::entries::models::ThirdPartyHost;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_project::planner_selection::models::Carrier;

const PATHLIB_MODULE: &str = "pathlib";
const JSON_MODULE: &str = "json";
const ABC_MODULE: &str = "collections.abc";
const DAG_VERSION: i64 = 1;
const DAG_LIST_KEYS: [&str; 3] = ["nodes", "edges", "checks"];

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

fn field<'py>(mapping: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    mapping_get(mapping, key)
}

fn items<'py>(mapping: &Bound<'py, PyAny>, key: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let empty: Bound<'py, PyTuple> = PyTuple::empty(mapping.py());
    mapping_get_or(mapping, key, empty)?.try_iter()?.collect()
}

fn texts(mapping: &Bound<'_, PyAny>, key: &str) -> PyResult<Vec<String>> {
    items(mapping, key)?.iter().map(text).collect()
}

fn selection_graph(dag: &Bound<'_, PyAny>) -> PyResult<DagSelectionGraph> {
    let nodes: Vec<DagSelectionNode> = items(dag, "nodes")?
        .iter()
        .map(|node| {
            let path: Bound<'_, PyAny> = field(node, "path")?;
            Ok(DagSelectionNode {
                id: text(&node.get_item("id")?)?,
                name: text(&field(node, "name")?)?,
                kind: text(&field(node, "kind")?)?,
                path: if path.is_instance_of::<PyString>() {
                    Some(path.extract()?)
                } else {
                    None
                },
                tags: texts(node, "tags")?,
            })
        })
        .collect::<PyResult<_>>()?;
    let edges: Vec<(String, String)> = items(dag, "edges")?
        .iter()
        .map(|edge| {
            Ok((
                text(&field(edge, "from_id")?)?,
                text(&field(edge, "to_id")?)?,
            ))
        })
        .collect::<PyResult<_>>()?;
    let checks: Vec<DagSelectionCheck> = items(dag, "checks")?
        .iter()
        .map(|check| {
            Ok(DagSelectionCheck {
                name: text(&field(check, "name")?)?,
                id: text(&field(check, "id")?)?,
                checked_asset_ids: texts(check, "checked_asset_ids")?,
            })
        })
        .collect::<PyResult<_>>()?;
    Ok(DagSelectionGraph {
        nodes,
        edges,
        checks,
    })
}

fn selection_error(py: Python<'_>, error: DagSelectionError, raw: &str) -> PyErr {
    let carrier = match Carrier::single(&PyString::new(py, raw)) {
        Ok(carrier) => carrier,
        Err(failure) => return failure,
    };
    match error {
        DagSelectionError::Selector(error) => carrier.error(error),
        DagSelectionError::UnitTest(selector) => {
            carrier.error(unit_test_selector_rejection(&selector))
        }
        DagSelectionError::Input(message) => {
            host_error(py, ThirdPartyHost::DagsterDagInputError, &message)
        }
    }
}

/// Ids of DAG artifact nodes `select` matches minus those `exclude` matches.
pub(crate) fn selected_node_ids(
    dag: &Bound<'_, PyAny>,
    (select, exclude): (&str, Option<&str>),
) -> PyResult<BTreeSet<String>> {
    let graph: DagSelectionGraph = selection_graph(dag)?;
    resolve(&graph, (select, exclude)).map_err(|error| selection_error(dag.py(), error, select))
}

fn raised(error_class: &Bound<'_, PyAny>, message: String, cause: Option<PyErr>) -> PyErr {
    let py: Python<'_> = error_class.py();
    match error_class.call1((message,)) {
        Ok(instance) => {
            let error: PyErr = PyErr::from_value(instance);
            error.set_cause(py, cause);
            error
        }
        Err(failure) => failure,
    }
}

/// A SQLBuild DAG artifact from a mapping or JSON file path, validated, raising `error_class`.
#[pyfunction]
#[pyo3(signature = (dag, *, error_class))]
pub(crate) fn load_sqlbuild_dag<'py>(
    dag: Bound<'py, PyAny>,
    error_class: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = dag.py();
    let path_class: Bound<'py, PyAny> = imported(py, PATHLIB_MODULE, "Path")?;
    let payload: Bound<'py, PyAny> = if dag.is_instance_of::<PyString>()
        || dag.is_instance(&path_class)?
    {
        let path: Bound<'py, PyAny> = path_class.call1((&dag,))?;
        let encoding: Bound<'py, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        let content: Bound<'py, PyAny> = match path.call_method("read_text", (), Some(&encoding)) {
            Ok(content) => content,
            Err(error) if error.is_instance_of::<PyOSError>(py) => {
                return Err(raised(
                    error_class,
                    format!("could not read SQLBuild DAG artifact: {}", path.str()?),
                    Some(error),
                ));
            }
            Err(error) => return Err(error),
        };
        let json: Bound<'py, PyModule> = py.import(JSON_MODULE)?;
        let loaded: Bound<'py, PyAny> = match json.call_method1("loads", (content,)) {
            Ok(loaded) => loaded,
            Err(error) if error.is_instance(py, &native_field(&json, "JSONDecodeError")?) => {
                let message: String = native_field(error.value(py), "msg")?.extract()?;
                return Err(raised(
                    error_class,
                    format!(
                        "SQLBuild DAG artifact is not valid JSON: {}: {message}",
                        path.str()?
                    ),
                    Some(error),
                ));
            }
            Err(error) => return Err(error),
        };
        if !loaded.is_instance(&imported(py, ABC_MODULE, "Mapping")?)? {
            return Err(raised(
                error_class,
                "SQLBuild DAG artifact must be a JSON object".to_owned(),
                None,
            ));
        }
        loaded
    } else {
        dag
    };
    let version: Bound<'py, PyAny> = mapping_get(&payload, "version")?;
    if version.ne(DAG_VERSION)? {
        return Err(raised(
            error_class,
            format!(
                "unsupported SQLBuild DAG artifact version: {}",
                version.repr()?
            ),
            None,
        ));
    }
    for key in DAG_LIST_KEYS {
        if !mapping_get(&payload, key)?.is_instance_of::<PyList>() {
            return Err(raised(
                error_class,
                format!("SQLBuild DAG artifact field '{key}' must be a list"),
                None,
            ));
        }
    }
    Ok(payload)
}
