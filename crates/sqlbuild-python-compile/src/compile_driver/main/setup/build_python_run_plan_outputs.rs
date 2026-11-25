//! Attach python-aware selection warnings and plan entries to the plan output.

use crate::compile_driver::_helpers::setup::python_plan_entries::NodeKinds;
use crate::compile_driver::_helpers::setup::python_plan_entries::node_graph;
use crate::compile_driver::_helpers::setup::python_plan_entries::plan_entries;
use crate::compile_driver::_helpers::setup::python_plan_entries::skipped_ingress_warnings;
use crate::compile_driver::main::setup::publish_plan_summary::publish_plan_summary;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::graph::main::python_nodes::build_python_sql_run_lifecycle::build_python_sql_run_lifecycle;

/// Attach python-aware selection warnings and plan entries to the plan output.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_python_run_plan_outputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_python_run_plan_outputs",
        options,
        (
            &[
                "discovered_inputs",
                "plan_output",
                "run_selection",
                "selected_python_node_names",
            ],
            &[],
        ),
    )?;
    let plan_output: Bound<'py, PyAny> = arguments.value("plan_output")?;
    let run_selection: Bound<'py, PyAny> = arguments.value("run_selection")?;
    let selected: Bound<'py, PyAny> = arguments.value("selected_python_node_names")?;
    if run_selection.is_none() {
        publish_plan_summary(&plan_output, PyTuple::empty(py).as_any())?;
        return new_record_with(
            py,
            RecordId::PythonRunPlanOutputs,
            &[
                ("plan_output", plan_output),
                ("python_plan_entries", PyTuple::empty(py).into_any()),
                ("selected_python_node_names", selected),
            ],
        );
    }
    let graph: Bound<'py, PyAny> = node_graph(&arguments.value("discovered_inputs")?)?;
    let kinds: NodeKinds<'py> = NodeKinds::load(py)?;
    let mut warnings: Vec<Bound<'py, PyAny>> = native_field(&plan_output, "warnings")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    warnings.extend(skipped_ingress_warnings(
        &plan_output,
        &run_selection,
        (&graph, &kinds),
    )?);
    let plan_output: Bound<'py, PyAny> = replaced(
        &plan_output,
        &[("warnings", PyTuple::new(py, warnings)?.into_any())],
    )?;
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PythonSqlRunSelection,
        &[
            ("sql_keys", native_field(&plan_output, "selected_keys")?),
            ("python_node_names", selected.clone()),
        ],
    )?;
    let lifecycle: Bound<'py, PyAny> = build_python_sql_run_lifecycle(&selection, &graph)?;
    let entries: Bound<'py, PyTuple> = plan_entries(
        &lifecycle,
        (&graph, &kinds),
        &native_field(&plan_output, "python_identity_fingerprints")?,
    )?;
    publish_plan_summary(&plan_output, entries.as_any())?;
    new_record_with(
        py,
        RecordId::PythonRunPlanOutputs,
        &[
            ("plan_output", plan_output),
            ("python_plan_entries", entries.into_any()),
            ("selected_python_node_names", selected),
        ],
    )
}
