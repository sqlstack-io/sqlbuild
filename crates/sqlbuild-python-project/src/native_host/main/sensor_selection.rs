//! A sensor condition's selectors, resolved as publication resolves them.

use pyo3::PyResult;
use serde_json::Value;
use sqlbuild_analysis::graph::models::ProjectGraph;

use crate::graph::models::ProjectSelection;
use crate::native_host::_helpers::orchestration_triggers::sensor_nodes;

/// The published node keys `selectors` name for the condition `field`, or why they name none.
pub(crate) fn sensor_selection(
    project: (&ProjectGraph, &ProjectSelection),
    selectors: &[String],
    field: &str,
) -> PyResult<Value> {
    sensor_nodes(project, selectors, field)
}
