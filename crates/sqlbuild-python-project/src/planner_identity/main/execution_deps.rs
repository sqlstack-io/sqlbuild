//! Execution dependencies kept native until a public graph or plan asks for its dict view.

use crate::compiled_views::main::project::compiled_models::compiled_models;
use crate::compiled_views::main::project::functions::functions;
use crate::compiled_views::main::project::seeds::seeds;
use crate::compiled_views::main::project::sources::sources;
use crate::compiled_views::main::project::sql_tests::sql_tests;
use crate::graph::main::nodes::native_key::native_key;
use crate::planner_identity::_helpers::graph::TABLE_FN_TEST_MODE;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use crate::references::models::{GateRow, HookRow};
use crate::resource_views::main::resource_reads::graph_row::graph_row;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::main::execution_upstream::execution_upstream;
use sqlbuild_analysis::planner_identity::models::ExecutionTest;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Execution dependencies kept native until a public graph or plan asks for its dict view.
pub fn execution_deps(project: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    let mut resources: Vec<(ObjectKey, Vec<ObjectKey>)> = Vec::new();
    let collections: [Bound<'_, PyAny>; 4] = [
        compiled_models(project)?,
        sources(project)?,
        seeds(project)?,
        functions(project)?,
    ];
    for collection in collections {
        for resource in collection.try_iter()? {
            let row = graph_row(&resource?)?;
            resources.push((row.key, row.deps));
        }
    }
    let mut tests: Vec<ExecutionTest> = Vec::new();
    for test in sql_tests(project)?.try_iter()? {
        let test = test?;
        tests.push(ExecutionTest {
            key: native_key(&native_field(&test, "key")?)?,
            scope_deps: native_field(&test, "scope_deps")?
                .try_iter()?
                .map(|key| native_key(&key?))
                .collect::<PyResult<_>>()?,
            table_function: native_field(&test, "mode")?.str()?.extract::<String>()?
                == TABLE_FN_TEST_MODE,
        });
    }
    let (gates, hooks): (Vec<GateRow>, Vec<HookRow>) = native_execution_gate_edges(project)?;
    let injected: Vec<_> = gates
        .into_iter()
        .map(|gate| (gate.gated, gate.read))
        .chain(hooks.into_iter().map(|hook| (hook.gated, hook.read)))
        .collect();
    Ok(execution_upstream(resources, &tests, &injected))
}
