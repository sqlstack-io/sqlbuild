//! Execution dependencies over gate edges already computed for this project.

use crate::compiled_views::main::project::compiled_models::compiled_models;
use crate::compiled_views::main::project::functions::functions;
use crate::compiled_views::main::project::seeds::seeds;
use crate::compiled_views::main::project::sources::sources;
use crate::compiled_views::main::project::sql_tests::sql_tests;
use crate::graph::main::nodes::native_key::native_key;
use crate::graph::main::project::load_rows::load_rows;
use crate::planner_identity::_helpers::graph::TABLE_FN_TEST_MODE;
use crate::references::types::ExecutionGates;
use crate::resource_views::main::resource_reads::graph_row::graph_row;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::main::execution_upstream::execution_upstream;
use sqlbuild_analysis::planner_identity::models::ExecutionTest;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Execution dependencies over gate edges already computed for this project.
pub fn execution_deps_with(
    project: &Bound<'_, PyAny>,
    (gates, hooks): &ExecutionGates,
) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
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
    for row in load_rows(&native_field(project, "loads")?)? {
        resources.push((row.key, row.deps));
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
    let injected: Vec<_> = gates
        .iter()
        .map(|gate| (gate.gated.clone(), gate.read.clone()))
        .chain(
            hooks
                .iter()
                .map(|hook| (hook.gated.clone(), hook.read.clone())),
        )
        .collect();
    Ok(execution_upstream(resources, &tests, &injected))
}
