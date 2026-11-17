//! Compiled project and project graph construction: frontier stages, target defaults and checks.

use crate::compile_driver::main::setup::attached_inputs::attached_inputs;
use crate::compile_driver::main::setup::compile_adapter_context::compile_adapter_context;
use crate::compile_driver::main::setup::preserved_loader_schemas::preserved_loader_schemas;
use crate::compile_driver::main::setup::project_graph::project_graph;
use crate::compile_driver::main::stages::monotonic::monotonic;
use std::collections::HashSet;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyCFunction, PyDict, PyTuple};

use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;

use crate::compile_driver::_helpers::setup::target_defaults::TargetDefaults;
use crate::compile_driver::_helpers::setup::target_defaults::apply_target_defaults;
use crate::compile_driver::_helpers::setup::targets::managed_loader_target_isolation;
use crate::compile_driver::_helpers::setup::targets::managed_source_loaders;
use crate::compile_driver::_helpers::setup::targets::managed_write_schemas;
use crate::compile_driver::_helpers::setup::targets::named_target_schema_strategy;
use crate::compile_driver::_helpers::setup::targets::project_targets;
use crate::compile_driver::_helpers::setup::targets::requires_explicit_targets;
use crate::compile_driver::models::InputsRequest;
use crate::frontier::main::bridged::bridged;
use crate::frontier::main::produced::produced;
use crate::frontier::models::FrontierStage;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_front::sqltext::main::optional_record_syntax::optional_record_syntax;
use sqlbuild_python_front::sqltext::main::syntax_record::syntax_record;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::compile_driver::_helpers::stages::assembly::assemble_project;
use crate::compile_driver::_helpers::stages::selection_phases::resolve_analysis_model_names;
use pyo3::IntoPyObjectExt;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::constants::TABLE_PROMOTION_MODE_SETTING_KEY;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::compile_driver::constants::PROMOTION_CONFLICTS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_project::compiled_views::main::project::project_with::project_with;
use sqlbuild_python_project::contracts::main::promotion_conflict_diagnostics::promotion_conflict_diagnostics;
const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
/// Optional keywords of the project graph builders.
pub(crate) const GRAPH_OPTIONAL: [&str; 7] = [
    "selected_target",
    "no_sql_validation",
    "skip_column_inference",
    "column_lineage_mode",
    "cli_vars",
    "external_sql_reference_resolver",
    "on_progress",
];

/// The keyword arguments of one compiled-project build.
pub(crate) struct ProjectRequest {
    discovered_inputs: Py<PyAny>,
    adapter: Py<PyAny>,
    selected_target: Py<PyAny>,
    no_sql_validation: bool,
    skip_column_inference: bool,
    column_lineage_mode: Py<PyAny>,
    cli_vars: Py<PyAny>,
    external_sql_reference_resolver: Py<PyAny>,
    resolved_connection: Py<PyAny>,
    analysis_selection: Py<PyAny>,
    /// Rules the project's SQL is authored in where they differ from the adapter's, as in replay.
    source_lexical_syntax: Option<LexicalSyntax>,
}

/// `column_lineage_mode`, defaulting to `ColumnLineageMode.FAST` when not passed.
pub(crate) fn lineage_mode<'py>(arguments: &KeywordArguments<'py>) -> PyResult<Bound<'py, PyAny>> {
    let mode: Bound<'py, PyAny> = arguments.value("column_lineage_mode")?;
    if arguments.passed("column_lineage_mode")? {
        return Ok(mode);
    }
    enum_member(mode.py(), RecordId::ColumnLineageMode, "FAST")
}

/// `value` as a Python `bool`.
pub(crate) fn py_bool(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

impl ProjectRequest {
    /// The request named by keyword `arguments`.
    pub(crate) fn read(arguments: &KeywordArguments<'_>) -> PyResult<Self> {
        Ok(Self {
            discovered_inputs: arguments.value("discovered_inputs")?.unbind(),
            adapter: arguments.value("adapter")?.unbind(),
            selected_target: arguments.value("selected_target")?.unbind(),
            no_sql_validation: arguments.flag("no_sql_validation")?,
            skip_column_inference: arguments.flag("skip_column_inference")?,
            column_lineage_mode: lineage_mode(arguments)?.unbind(),
            cli_vars: arguments.value("cli_vars")?.unbind(),
            external_sql_reference_resolver: arguments
                .value("external_sql_reference_resolver")?
                .unbind(),
            resolved_connection: arguments.value("resolved_connection")?.unbind(),
            analysis_selection: arguments.value("analysis_selection")?.unbind(),
            source_lexical_syntax: optional_record_syntax(
                &arguments.value("source_lexical_syntax")?,
            )?,
        })
    }
}

impl ProjectRequest {
    /// The lexical rules the project's SQL is read with: the source syntax, else the adapter's.
    fn lexical_syntax<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.source_lexical_syntax {
            Some(syntax) => syntax_record(py, syntax),
            None => native_field(self.adapter.bind(py), "sql_lexical_syntax"),
        }
    }
}

/// The compiled project produced through the `COMPILED_PROJECT` frontier.
pub(crate) fn compiled_project(
    py: Python<'_>,
    request: ProjectRequest,
) -> PyResult<Bound<'_, PyAny>> {
    produced(
        FrontierStage::CompiledProject,
        python_compiled_project(py, &request)?,
    )
}

/// `_build_python_compiled_project`.
fn python_compiled_project<'py>(
    py: Python<'py>,
    request: &ProjectRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let discovered: &Bound<'py, PyAny> = request.discovered_inputs.bind(py);
    let adapter: &Bound<'py, PyAny> = request.adapter.bind(py);
    named_target_schema_strategy(
        discovered,
        adapter,
        (request.selected_target.bind(py), request.cli_vars.bind(py)),
    )?;
    managed_loader_target_isolation(discovered, adapter, request.resolved_connection.bind(py))?;
    let selection: &Bound<'py, PyAny> = request.analysis_selection.bind(py);
    let no_cache: bool =
        !selection.is_none() && native_field(selection, "no_cache")?.is_truthy()?;
    let compile_inputs: Bound<'py, PyAny> = record_compile_timing("attachment_ms", || {
        frontier_compile_inputs(py, request, no_cache)
    })?;
    if !selection.is_none() {
        let on_inputs_ready: Bound<'py, PyAny> = native_field(selection, "on_inputs_ready")?;
        if !on_inputs_ready.is_none() {
            call_with(&on_inputs_ready, &[("inputs", compile_inputs.clone())])?;
        }
    }
    let inference_profile: Bound<'py, PyAny> =
        adapter.call_method0("expression_inference_profile")?;
    let analysis_model_names: Bound<'py, PyAny> =
        resolve_analysis_model_names(&compile_inputs, &inference_profile, selection)?;
    let project: Bound<'py, PyAny> = assemble_project(
        py,
        Some(keywords(
            py,
            &[
                ("inputs", compile_inputs.clone()),
                ("inference_profile", inference_profile),
                (
                    "skip_column_inference",
                    py_bool(py, request.skip_column_inference),
                ),
                (
                    "column_lineage_mode",
                    request.column_lineage_mode.bind(py).clone(),
                ),
                (
                    "analysis_cache_dir",
                    native_field(&compile_inputs, "compile_cache_dir")?,
                ),
                ("analysis_model_names", analysis_model_names),
            ],
        )?),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    targeted_project(py, request, &project)
}

/// The assembled project with target defaults, target checks and promotion conflicts applied.
fn targeted_project<'py>(
    py: Python<'py>,
    request: &ProjectRequest,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let discovered: &Bound<'py, PyAny> = request.discovered_inputs.bind(py);
    let adapter: &Bound<'py, PyAny> = request.adapter.bind(py);
    let source_loaders: HashSet<String> = managed_source_loaders(project)?;
    let mut intermediate_loaders: HashSet<String> = HashSet::new();
    for loader in native_field(discovered, "loader_functions")?.try_iter()? {
        let name: String = native_field(&loader?, "name")?.extract()?;
        if !source_loaders.contains(&name) {
            intermediate_loaders.insert(name);
        }
    }
    preserved_loader_schemas(
        discovered,
        (
            request.selected_target.bind(py),
            &native_field(project, "effective_vars")?,
        ),
        &intermediate_loaders,
    )?;
    managed_write_schemas(adapter, project)?;
    let effective_schema: Bound<'py, PyAny> = native_field(project, "effective_target_schema")?;
    let default_schema: Bound<'py, PyAny> = if effective_schema.is_truthy()? {
        effective_schema
    } else {
        adapter.call_method0("default_schema")?
    };
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let adapter_name: Bound<'py, PyAny> =
        resolve_effective_adapter_name(&project_config, &local_config.clone())
            .and_then(|value| value.into_bound_py_any(py))?;
    let effective_database: Bound<'py, PyAny> = native_field(project, "effective_target_database")?;
    let default_database: Bound<'py, PyAny> =
        if requires_explicit_targets(&adapter_name)? && effective_database.is_truthy()? {
            effective_database
        } else {
            adapter.call_method0("default_database")?
        };
    let defaults: TargetDefaults<'py> = TargetDefaults {
        default_schema,
        default_database,
        render_qualified_name: native_field(adapter, "render_qualified_name")?,
        python_functions_inherit_default_namespace: adapter
            .call_method0("python_functions_inherit_default_namespace")?,
    };
    let project: Bound<'py, PyAny> = apply_target_defaults(project, &defaults)?;
    project_targets(&adapter_name, &project)?;
    let promotion_key: Bound<'py, PyAny> =
        pyo3::types::PyString::new(py, TABLE_PROMOTION_MODE_SETTING_KEY).into_any();
    let settings_file: Bound<'py, PyAny> =
        if native_field(&local_config, "setting_overrides")?.contains(promotion_key)? {
            pyo3::types::PyString::new(py, LOCAL_CONFIG_FILENAME).into_any()
        } else {
            pyo3::types::PyString::new(py, PROJECT_CONFIG_FILENAME).into_any()
        };
    count_stage(py, PROMOTION_CONFLICTS, 1)?;
    let conflicts: Bound<'py, PyAny> = promotion_conflict_diagnostics(
        py,
        &project,
        &adapter.call_method0("default_table_promotion_mode")?,
        settings_file.extract()?,
    )?
    .into_any();
    let diagnostics: Bound<'py, PyTuple> = PyTuple::new(
        py,
        native_field(&project, "diagnostics")?
            .try_iter()?
            .chain(conflicts.try_iter()?)
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    project_with(
        &project,
        Some(keywords(py, &[("diagnostics", diagnostics.into_any())])?),
    )
}

/// The compile inputs produced through the `COMPILE_PROJECT_INPUTS` frontier.
fn frontier_compile_inputs<'py>(
    py: Python<'py>,
    request: &ProjectRequest,
    no_cache: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let discovered: &Bound<'py, PyAny> = request.discovered_inputs.bind(py);
    let adapter: &Bound<'py, PyAny> = request.adapter.bind(py);
    let adapter_context: Bound<'py, PyAny> =
        compile_adapter_context(discovered, adapter, &request.lexical_syntax(py)?)?;
    let inputs: InputsRequest = InputsRequest {
        discovered_inputs: discovered.clone().unbind(),
        adapter_context: adapter_context.unbind(),
        selected_target: request.selected_target.clone_ref(py),
        cli_vars: request.cli_vars.clone_ref(py),
        run_id: py.None(),
        no_sql_validation: py_bool(py, request.no_sql_validation).unbind(),
        defer_model_sql_validation: py_bool(py, true).unbind(),
        external_sql_reference_resolver: request.external_sql_reference_resolver.clone_ref(py),
        resolved_connection: request.resolved_connection.clone_ref(py),
        no_cache: py_bool(py, no_cache).unbind(),
    };
    let stage: Bound<'py, PyCFunction> = PyCFunction::new_closure(
        py,
        None,
        None,
        move |arguments: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| {
            attached_inputs(arguments.py(), &inputs).map(Bound::unbind)
        },
    )?;
    produced(
        FrontierStage::CompileProjectInputs,
        bridged(stage.as_any())?,
    )
}

/// Build a static graph with an explicit deep-analysis selection.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_project_graph_with_analysis_selection<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_project_graph_with_analysis_selection",
        options,
        (
            &["discovered_inputs", "adapter", "analysis_selection"],
            &GRAPH_OPTIONAL,
        ),
    )?;
    selected_graph(&arguments, arguments.value("analysis_selection")?)
}

/// `build_project_graph_with_analysis_selection` with its `analysis_selection`.
pub(crate) fn selected_graph<'py>(
    arguments: &KeywordArguments<'py>,
    analysis_selection: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let on_progress: Bound<'py, PyAny> = arguments.value("on_progress")?;
    let py: Python<'py> = on_progress.py();
    if !on_progress.is_none() {
        on_progress.call1(("Compiling project...",))?;
    }
    let started: f64 = monotonic(py)?;
    let mut request: ProjectRequest = ProjectRequest::read(arguments)?;
    request.analysis_selection = analysis_selection.unbind();
    let project: Bound<'py, PyAny> = compiled_project(py, request)?;
    if !on_progress.is_none() {
        let elapsed: f64 = monotonic(py)? - started;
        on_progress.call1((format!("Compiled project. ({elapsed:.2}s)"),))?;
    }
    project_graph(&project)
}
