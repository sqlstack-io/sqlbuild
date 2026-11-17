//! Injected-edge descriptions over gate edges already computed for this project.

use crate::compiled_views::main::project::sql_tests::sql_tests;
use crate::graph::main::nodes::native_key::native_key;
use crate::graph::models::NativeProjectGraph;
use crate::planner_identity::_helpers::graph::hook_label;
use crate::references::models::{GateRow, HookRow};
use crate::references::types::ExecutionGates;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::edge_keys::edge_keys;
use sqlbuild_analysis::graph::models::EdgeDirection;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashSet;

/// Injected-edge descriptions over gate edges already computed for this project.
pub fn execution_origins_with(
    project: &Bound<'_, PyAny>,
    (gates, hooks): &ExecutionGates,
) -> PyResult<Vec<((ObjectKey, ObjectKey), String)>> {
    let mut origins: Vec<((ObjectKey, ObjectKey), String)> = Vec::new();
    for test in sql_tests(project)?.try_iter()? {
        let test = test?;
        let test_name: String = native_field(&test, "name")?.extract()?;
        for target_key in native_field(&test, "scope_deps")?.try_iter()? {
            let target_key = native_key(&target_key?)?;
            let target_name = &target_key.1;
            let description = format!("SQL test '{test_name}' runs before '{target_name}'");
            origins.push((
                (target_key, native_key(&native_field(&test, "key")?)?),
                description,
            ));
        }
    }
    let lineage = native_field(project, "lineage_graph")?;
    let lineage = lineage.cast::<NativeProjectGraph>()?.borrow();
    let is_lineage = |gated: &ObjectKey, read: &ObjectKey| {
        edge_keys(lineage.graph(), gated, EdgeDirection::Upstream).contains(read)
    };
    let mut seen: HashSet<_> = origins.iter().map(|(edge, _)| edge.clone()).collect();
    for GateRow {
        audit_name,
        target,
        gated,
        read,
    } in gates
    {
        if is_lineage(gated, read) || !seen.insert((gated.clone(), read.clone())) {
            continue;
        }
        origins.push((
            (gated.clone(), read.clone()),
            format!("audit '{audit_name}' on '{}' reads '{}'", target.1, read.1),
        ));
    }
    for HookRow {
        hook_name,
        gated,
        read,
        hook_type: kind,
        ..
    } in hooks
    {
        if is_lineage(gated, read) || !seen.insert((gated.clone(), read.clone())) {
            continue;
        }
        origins.push((
            (gated.clone(), read.clone()),
            format!(
                "{} on '{}' reads '{}'",
                hook_label(hook_name, kind),
                gated.1,
                read.1
            ),
        ));
    }
    Ok(origins)
}
