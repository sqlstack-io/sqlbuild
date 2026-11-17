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
use crate::planner_identity::_helpers::graph::execution_deps_with;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Execution dependencies kept native until a public graph or plan asks for its dict view.
pub fn execution_deps(project: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    execution_deps_with(project, &native_execution_gate_edges(project)?)
}
