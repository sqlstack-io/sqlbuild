//! Planner-ready project assembly: diagnostics collection, the macro bridge and explicit references.

use std::sync::{Arc, Mutex, PoisonError};

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyCFunction, PyDict, PyTuple};
use sqlbuild_host::entries::models::UserCodeHost;

use sqlbuild_python_base::compile_context::main::diagnostic_collector::collect_compile_diagnostics::collect_compile_diagnostics;
use crate::compile_driver::_helpers::assembly::assembly_project::assembled_project;
use crate::compile_driver::_helpers::setup::project::lineage_mode;
use sqlbuild_python_project::compiled_views::main::project::project_with::project_with;
use sqlbuild_python_project::compiled_views::main::project::record_model_analyses::record_model_analyses;
use sqlbuild_python_project::references::main::explicit_reference_report::explicit_reference_report;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const STAGE_REACHED: &str = "assembly stopped before the requested stage";
const EXPRESSION_SHAPES_STAGE: &str = "expression_shapes";
const MODEL_FACTS_STAGE: &str = "model_facts";
const SEMANTIC_COMPLETION_STAGE: &str = "semantic_completion";

const OPTIONAL: [&str; 5] = [
    "inference_profile",
    "skip_column_inference",
    "column_lineage_mode",
    "analysis_cache_dir",
    "analysis_model_names",
];

/// The keyword arguments of one project assembly.
pub(crate) struct AssemblyRequest {
    pub(crate) inputs: Py<PyAny>,
    pub(crate) inference_profile: Py<PyAny>,
    pub(crate) skip_column_inference: Py<PyAny>,
    pub(crate) column_lineage_mode: Py<PyAny>,
    pub(crate) analysis_cache_dir: Py<PyAny>,
    pub(crate) analysis_model_names: Py<PyAny>,
    /// The stage to stop before, keeping its inputs, when assembly runs for stage tests.
    pub(crate) stop: Option<StageStop>,
}

/// An assembly stage whose inputs tests can take instead of running it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssemblyStage {
    ExpressionShapes,
    ModelFacts,
    SemanticCompletion,
}

/// Where an assembly stopped for a stage test keeps that stage's inputs.
pub(crate) struct StageStop {
    stage: AssemblyStage,
    inputs: Arc<Mutex<Option<Py<PyDict>>>>,
}

impl AssemblyRequest {
    /// Stop with `inputs` before `stage` when the request stops there; else continue.
    pub(crate) fn before_stage<'py>(
        &self,
        stage: AssemblyStage,
        inputs: impl FnOnce() -> PyResult<Bound<'py, PyDict>>,
    ) -> PyResult<()> {
        let Some(stop) = self.stop.as_ref().filter(|stop| stop.stage == stage) else {
            return Ok(());
        };
        *stop.inputs.lock().unwrap_or_else(PoisonError::into_inner) = Some(inputs()?.unbind());
        Err(PyRuntimeError::new_err(STAGE_REACHED))
    }
}

/// The assembly request of `arguments`, stopping at `stop` when given.
fn assembly_request(
    arguments: &KeywordArguments<'_>,
    stop: Option<StageStop>,
) -> PyResult<AssemblyRequest> {
    Ok(AssemblyRequest {
        inputs: arguments.value("inputs")?.unbind(),
        inference_profile: arguments.value("inference_profile")?.unbind(),
        skip_column_inference: arguments.value_or_false("skip_column_inference")?.unbind(),
        column_lineage_mode: lineage_mode(arguments)?.unbind(),
        analysis_cache_dir: arguments.value("analysis_cache_dir")?.unbind(),
        analysis_model_names: arguments.value("analysis_model_names")?.unbind(),
        stop,
    })
}

/// Convert compile inputs into the planner-ready project view.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn assemble_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "assemble_project", options, (&["inputs"], &OPTIONAL))?;
    collected_project(py, assembly_request(&arguments, None)?)
}

/// Test support: assemble like `assemble_project` but stop before `stage`, returning its inputs or None.
#[pyfunction]
#[pyo3(signature = (stage, **options))]
pub(crate) fn assembly_stage_inputs<'py>(
    py: Python<'py>,
    stage: &str,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let stage: AssemblyStage = match stage {
        EXPRESSION_SHAPES_STAGE => AssemblyStage::ExpressionShapes,
        MODEL_FACTS_STAGE => AssemblyStage::ModelFacts,
        SEMANTIC_COMPLETION_STAGE => AssemblyStage::SemanticCompletion,
        _ => {
            return Err(PyValueError::new_err(format!(
                "unknown assembly stage '{stage}'"
            )));
        }
    };
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "assembly_stage_inputs",
        options,
        (&["inputs"], &OPTIONAL),
    )?;
    let inputs: Arc<Mutex<Option<Py<PyDict>>>> = Arc::new(Mutex::new(None));
    let stop: StageStop = StageStop {
        stage,
        inputs: Arc::clone(&inputs),
    };
    let assembled: PyResult<Bound<'py, PyAny>> =
        collected_project(py, assembly_request(&arguments, Some(stop))?);
    let captured: Option<Py<PyDict>> = inputs.lock().unwrap_or_else(PoisonError::into_inner).take();
    match (captured, assembled) {
        (Some(captured), _) => Ok(Some(captured.into_bound(py))),
        (None, Ok(_)) => Ok(None),
        (None, Err(error)) => Err(error),
    }
}

/// `request`'s project, with diagnostics collected while assembling appended.
fn collected_project<'py>(
    py: Python<'py>,
    request: AssemblyRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let collector: Bound<'py, PyAny> = collect_compile_diagnostics(py)?.into_any();
    let (built, violations): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        within(&collector, |violations| {
            Ok((referenced_project(py, request)?, violations.clone()))
        })?;
    let collected: Bound<'py, PyAny> = native_field(&violations, "diagnostics")?;
    if !collected.is_truthy()? {
        return Ok(built);
    }
    let diagnostics: Bound<'py, PyTuple> =
        chained(&native_field(&built, "diagnostics")?, &collected)?;
    project_with(
        &built,
        Some(keywords(py, &[("diagnostics", diagnostics.into_any())])?),
    )
}

/// `first` followed by `second`, as one tuple.
pub(crate) fn chained<'py>(
    first: &Bound<'py, PyAny>,
    second: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(
        first.py(),
        first
            .try_iter()?
            .chain(second.try_iter()?)
            .collect::<PyResult<Vec<_>>>()?,
    )
}

/// The undecorated `assemble_project`.
fn referenced_project(py: Python<'_>, request: AssemblyRequest) -> PyResult<Bound<'_, PyAny>> {
    let inputs: Bound<'_, PyAny> = request.inputs.bind(py).clone();
    let project: Bound<'_, PyAny> = if host_entry(py, UserCodeHost::ActiveMacroBridge)?
        .call0()?
        .is_none()
    {
        let stage: Bound<'_, PyCFunction> = PyCFunction::new_closure(
            py,
            None,
            None,
            move |arguments: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| {
                assembled_project(arguments.py(), &request).map(Bound::unbind)
            },
        )?;
        call_with(
            &host_entry(py, UserCodeHost::RunWithMacroBridge)?,
            &[("stage", stage.into_any())],
        )?
    } else {
        assembled_project(py, &request)?
    };
    let project: Bound<'_, PyAny> =
        with_native_project(&project, &native_field(&inputs, "native_project")?)?;
    let references: Bound<'_, PyAny> = explicit_reference_report(
        py,
        &project,
        &native_field(&inputs, "discovered_inputs")?,
        native_field(&inputs, "project_config")?
            .getattr("references")?
            .getattr("enforce_explicit")?
            .extract()?,
    )?;
    let reference_diagnostics: Bound<'_, PyAny> = native_field(&references, "diagnostics")?;
    let unmatched: Bound<'_, PyAny> = native_field(&references, "unmatched")?;
    if !reference_diagnostics.is_truthy()? && !unmatched.is_truthy()? {
        return Ok(project);
    }
    let diagnostics: Bound<'_, PyTuple> = chained(
        &native_field(&project, "diagnostics")?,
        &reference_diagnostics,
    )?;
    project_with(
        &project,
        Some(keywords(
            py,
            &[
                ("diagnostics", diagnostics.into_any()),
                ("unmatched_literal_sql_relations", unmatched),
            ],
        )?),
    )
}

/// Hand the compile's native project the assembled analyses and keep it on the project.
fn with_native_project<'py>(
    project: &Bound<'py, PyAny>,
    native_project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if native_project.is_none() {
        return Ok(project.clone());
    }
    record_model_analyses(native_project.cast()?, &native_field(project, "models")?)?;
    if native_field(project, "native_project")?.is(native_project) {
        return Ok(project.clone());
    }
    project_with(
        &project.clone(),
        Some(keywords(py, &[("native_project", native_project.clone())])?),
    )
}
