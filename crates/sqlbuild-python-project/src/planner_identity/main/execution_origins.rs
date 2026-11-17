//! Injected-edge descriptions remain native for cycle and buildability diagnostics.

use crate::compiled_views::main::project::sql_tests::sql_tests;
use crate::graph::main::nodes::native_key::native_key;
use crate::graph::models::NativeProjectGraph;
use crate::planner_identity::_helpers::graph::hook_label;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use crate::references::models::{GateRow, HookRow};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::edge_keys::edge_keys;
use sqlbuild_analysis::graph::models::EdgeDirection;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashSet;
use crate::planner_identity::_helpers::graph::execution_origins_with;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Injected-edge descriptions remain native for cycle and buildability diagnostics.
pub fn execution_origins(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<((ObjectKey, ObjectKey), String)>> {
    execution_origins_with(project, &native_execution_gate_edges(project)?)
}
