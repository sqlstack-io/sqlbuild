//! The compile output phases: manifest, DAG artifact, compiled artifacts and withheld test errors.

use crate::compile_driver::main::stages::elapsed_ms::elapsed_ms;
use crate::compile_driver::main::stages::monotonic::monotonic;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;

use crate::compile_command::main::artifacts::planning_diagnostics::planning_diagnostics;
use crate::compile_command::main::artifacts::write_static_target::write_static_target;
use crate::compile_command::models::ArtifactBatch;
use crate::compile_command::models::Prepared;
use crate::compile_command::types::ArtifactRecorder;
use crate::dag_artifact::main::build_dag_json::build_dag_json;
use crate::manifest::main::build_manifest::build_manifest;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::Status;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;

const EMPTY_DAG_PATH: &str = "";
const TARGET_DIRECTORY: &str = "target";
const DEFAULT_DAG_FILE: &str = "sqlbuild_dag.json";

/// Seconds since a `time.monotonic()` reading, formatted `{:.2f}`.
fn seconds_since(py: Python<'_>, started: f64) -> PyResult<String> {
    let now: f64 = imported(py, "time", "monotonic")?.call0()?.extract()?;
    Ok(format!("{:.2}", now - started))
}

/// `project_dir / "target"`.
pub(crate) fn target_dir<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_dir.call_method1("__truediv__", (TARGET_DIRECTORY,))
}

/// The manifest payload, when requested.
pub(crate) fn manifest_payload<'py>(
    requested: bool,
    analysis: &Bound<'py, PyAny>,
    status: &Status<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = analysis.py();
    if !requested {
        return Ok(py.None().into_bound(py));
    }
    let started: f64 = monotonic(py)?;
    status.start("Building manifest...")?;
    let discovered: Bound<'py, PyAny> = native_field(analysis, "discovered_inputs")?;
    let project_config: Bound<'py, PyAny> = native_field(&discovered, "project_config")?;
    let graph: Bound<'py, PyAny> = native_field(analysis, "graph")?;
    let adapter_type: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &project_config,
        &native_field(&discovered, "local_config")?,
    )?;
    let payload: Bound<'py, PyAny> = build_manifest(Some(&keywords(
        py,
        &[
            ("project", native_field(&graph, "project")?),
            ("plan_output", py.None().into_bound(py)),
            ("project_name", native_field(&project_config, "name")?),
            ("adapter_type", adapter_type),
            ("graph", native_field(&graph, "native")?),
        ],
    )?))?
    .into_bound(py)
    .into_any();
    status.complete(&format!(
        "Built manifest. ({}s)",
        seconds_since(py, started)?
    ))?;
    Ok(payload)
}

/// The DAG artifact path a `--dag` value names, resolved against the project.
pub(crate) fn resolved_dag_path<'py>(
    project_dir: &Bound<'py, PyAny>,
    dag_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    if dag_path.eq(EMPTY_DAG_PATH)? {
        return target_dir(project_dir)?.call_method1("__truediv__", (DEFAULT_DAG_FILE,));
    }
    let path: Bound<'py, PyAny> = imported(py, "pathlib", "Path")?.call1((dag_path,))?;
    if path.call_method0("is_absolute")?.is_truthy()? {
        return Ok(path);
    }
    project_dir.call_method1("__truediv__", (path,))
}

/// Write the DAG JSON artifact when a DAG path is requested.
pub(crate) fn write_dag_artifact<'py>(
    (dag_path, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (analysis, recorder): (&Bound<'py, PyAny>, ArtifactRecorder<'_>),
    status: &Status<'py>,
) -> PyResult<()> {
    if dag_path.is_none() {
        return Ok(());
    }
    let py: Python<'py> = analysis.py();
    let started: f64 = monotonic(py)?;
    status.start("Writing DAG artifact...")?;
    let discovered: Bound<'py, PyAny> = native_field(analysis, "discovered_inputs")?;
    let python_graph: Bound<'py, PyAny> = python_node_graph(&discovered)?;
    let path: Bound<'py, PyAny> = resolved_dag_path(project_dir, dag_path)?;
    let project_name: String = native_field(&discovered, "project_config")?
        .getattr("name")?
        .extract()?;
    let dag_json: String = build_dag_json(
        &native_field(analysis, "graph")?,
        &project_name,
        Some(&python_graph),
    )?;
    let mut batch: ArtifactBatch = ArtifactBatch::new(false, recorder);
    batch.queue(&path, dag_json.into_bytes())?;
    batch.flush(py)?;
    status.complete(&format!(
        "Wrote DAG artifact. ({}s)",
        seconds_since(py, started)?
    ))
}

/// SQL test planning errors of a compile whose artifacts are withheld, reusing staged planning.
pub(crate) fn sql_test_planning_diagnostics<'py>(
    project_dir: &Bound<'py, PyAny>,
    analysis: &Bound<'py, PyAny>,
    (prepared_artifacts, recorder): (&Bound<'py, PyAny>, ArtifactRecorder<'_>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = analysis.py();
    let planned: PyResult<Bound<'py, PyAny>> = (|| {
        if !prepared_artifacts.is_none() {
            let staged: Bound<'py, PyAny> =
                prepared_artifacts.call_method0("planning_diagnostics")?;
            if !staged.is_none() {
                return Ok(staged);
            }
        }
        Ok(planning_diagnostics(
            &target_dir(project_dir)?,
            (
                &native_field(analysis, "adapter")?,
                &native_field(analysis, "graph")?.getattr("project")?,
            ),
            Prepared {
                planned_tests: None,
                recorder,
            },
        )?
        .into_any())
    })();
    let error = match planned {
        Ok(diagnostics) => return Ok(diagnostics),
        Err(error) => error,
    };
    let compile_input_error: Bound<'py, PyAny> = error_class(py, ErrorId::CompileInput)?;
    let planner_input_error: Bound<'py, PyAny> = error_class(py, ErrorId::PlannerInput)?;
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if !(value.is_instance(&compile_input_error)? || value.is_instance(&planner_input_error)?) {
        return Err(error);
    }
    let reference_error: Bound<'py, PyAny> = error_class(py, ErrorId::SqlTestReference)?;
    let location: Bound<'py, PyAny> = if value.is_instance(&reference_error)? {
        native_field(&value, "location")?
    } else {
        py.None().into_bound(py)
    };
    let diagnostic: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            ("phase", enum_member(py, RecordId::DiagnosticPhase, "TEST")?),
            (
                "severity",
                enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?,
            ),
            ("code", native_field(&value, "code")?),
            ("message", value.str()?.into_any()),
            ("help", native_field(&value, "help")?),
            ("location", location),
        ],
    )?;
    Ok(PyTuple::new(py, [diagnostic])?.into_any())
}

/// Write compiled artifacts to `target/` and return `(written target, write milliseconds)`.
pub(crate) fn write_artifacts<'py>(
    (skip_write, project_dir): (bool, &Bound<'py, PyAny>),
    (analysis, manifest): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (prepared_artifacts, recorder): (&Bound<'py, PyAny>, ArtifactRecorder<'_>),
    status: &Status<'py>,
) -> PyResult<(Bound<'py, PyAny>, i64)> {
    let py: Python<'py> = analysis.py();
    let started: f64 = monotonic(py)?;
    let target: Bound<'py, PyAny> = target_dir(project_dir)?;
    let written: Bound<'py, PyAny> = if skip_write {
        let zero: Bound<'py, PyAny> = 0_i64.into_pyobject(py)?.into_any();
        new_record_with(
            py,
            RecordId::WrittenTarget,
            &[
                ("model_count", zero.clone()),
                ("seed_count", zero.clone()),
                ("function_count", zero.clone()),
                ("audit_count", zero.clone()),
                ("test_count", zero),
                ("target_dir", target),
            ],
        )?
    } else {
        status.start("Writing compiled artifacts...")?;
        let staged: Bound<'py, PyAny> = if prepared_artifacts.is_none() {
            py.None().into_bound(py)
        } else {
            call_with(
                &native_field(prepared_artifacts, "publish")?,
                &[
                    ("target_dir", target.clone()),
                    ("manifest", manifest.clone()),
                ],
            )?
        };
        if staged.is_none() {
            write_static_target(
                &target,
                (
                    &native_field(analysis, "adapter")?,
                    &native_field(analysis, "graph")?.getattr("project")?,
                ),
                manifest,
                Prepared {
                    planned_tests: None,
                    recorder,
                },
            )?
        } else {
            staged
        }
    };
    let write_ms: i64 = elapsed_ms(py, started)?;
    if !skip_write {
        status.complete(&format!(
            "Wrote compiled artifacts. ({:.2}s)",
            write_ms as f64 / 1000.0
        ))?;
    }
    Ok((written, write_ms))
}
