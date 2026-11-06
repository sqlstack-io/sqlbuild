//! The first attached compile input snapshot: target, vars, macros, scopes, models and resources.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, keywords, within,
};
use crate::bindings::_helpers::compile_attachment::declaration_scope::build_declaration_scope;
use crate::bindings::_helpers::compile_attachment::effective_config::{
    build_effective_connection, build_effective_settings, build_effective_vars,
};
use crate::bindings::_helpers::compile_attachment::model_inputs::build_model_inputs;
use crate::bindings::_helpers::compile_context::diagnostic_collector::collect_compile_diagnostics;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::records::{compile_inputs_view, inputs_with};
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::input_loops::attachment::{attach_checks, attach_resources};
use crate::bindings::_helpers::model_config::template_data::run_id;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::render::declaration_indexes::{
    public_declaration_indexes, public_model_schema_index,
};

const REQUIRED: [&str; 2] = ["discovered_inputs", "adapter_context"];
const OPTIONAL: [&str; 8] = [
    "selected_target",
    "cli_vars",
    "run_id",
    "no_sql_validation",
    "defer_model_sql_validation",
    "external_sql_reference_resolver",
    "resolved_connection",
    "no_cache",
];

/// The arguments of one compile input attachment; unset flags are `False`, other options `None`.
pub(crate) struct InputsRequest {
    pub(crate) discovered_inputs: Py<PyAny>,
    pub(crate) adapter_context: Py<PyAny>,
    pub(crate) selected_target: Py<PyAny>,
    pub(crate) cli_vars: Py<PyAny>,
    pub(crate) run_id: Py<PyAny>,
    pub(crate) no_sql_validation: Py<PyAny>,
    pub(crate) defer_model_sql_validation: Py<PyAny>,
    pub(crate) external_sql_reference_resolver: Py<PyAny>,
    pub(crate) resolved_connection: Py<PyAny>,
    pub(crate) no_cache: Py<PyAny>,
}

impl InputsRequest {
    /// The request `build_compile_inputs` was called with.
    fn read(arguments: &KeywordArguments<'_>) -> PyResult<Self> {
        Ok(Self {
            discovered_inputs: arguments.value("discovered_inputs")?.unbind(),
            adapter_context: arguments.value("adapter_context")?.unbind(),
            selected_target: arguments.value("selected_target")?.unbind(),
            cli_vars: arguments.value("cli_vars")?.unbind(),
            run_id: arguments.value("run_id")?.unbind(),
            no_sql_validation: arguments.value_or_false("no_sql_validation")?.unbind(),
            defer_model_sql_validation: arguments
                .value_or_false("defer_model_sql_validation")?
                .unbind(),
            external_sql_reference_resolver: arguments
                .value("external_sql_reference_resolver")?
                .unbind(),
            resolved_connection: arguments.value("resolved_connection")?.unbind(),
            no_cache: arguments.value_or_false("no_cache")?.unbind(),
        })
    }
}

/// The effective target, vars, settings and macro context every later stage shares.
struct TargetContext<'py> {
    target_name: Bound<'py, PyAny>,
    target: Bound<'py, PyAny>,
    compile_cache_dir: Bound<'py, PyAny>,
    vars: Bound<'py, PyAny>,
    settings: Bound<'py, PyAny>,
    macro_context: Bound<'py, PyAny>,
}

/// The attached models with the native project that owns them and their declarations.
struct ModelBuild<'py> {
    inputs: Bound<'py, PyAny>,
    native_project: Bound<'py, PyAny>,
    enums: Bound<'py, PyAny>,
    constants: Bound<'py, PyAny>,
    context: Bound<'py, PyAny>,
}

/// Seeds, functions and sources, and `(tests, scenarios, audits, diagnostics)` as attached.
struct AttachedResources<'py> {
    seeds: Bound<'py, PyAny>,
    functions: Bound<'py, PyAny>,
    sources: Bound<'py, PyAny>,
    checks: Bound<'py, PyAny>,
}

/// Attach discovered metadata into the first compile input snapshot.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_compile_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "build_compile_inputs", options, (&REQUIRED, &OPTIONAL))?;
    attached_inputs(py, &InputsRequest::read(&arguments)?)
}

/// The first compile input snapshot, with diagnostics collected while attaching appended.
pub(crate) fn attached_inputs<'py>(
    py: Python<'py>,
    request: &InputsRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let collector: Bound<'py, PyAny> = collect_compile_diagnostics(py)?.into_any();
    let (built, violations): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        within(&collector, |violations| {
            Ok((compile_inputs(py, request)?, violations.clone()))
        })?;
    let collected: Bound<'py, PyAny> = native_field(&violations, "diagnostics")?;
    if !collected.is_truthy()? {
        return Ok(built);
    }
    let diagnostics: Bound<'py, PyTuple> = PyTuple::new(
        py,
        native_field(&built, "diagnostics")?
            .try_iter()?
            .chain(collected.try_iter()?)
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    inputs_with(
        &built,
        Some(keywords(py, &[("diagnostics", diagnostics.into_any())])?),
    )
}

/// The undecorated `build_compile_inputs`.
fn compile_inputs<'py>(py: Python<'py>, request: &InputsRequest) -> PyResult<Bound<'py, PyAny>> {
    let discovered: Bound<'py, PyAny> = request.discovered_inputs.bind(py).clone();
    let adapter_context: Bound<'py, PyAny> = request.adapter_context.bind(py).clone();
    let target: TargetContext<'py> = target_context(request, &discovered, &adapter_context)?;
    let run_id: Bound<'py, PyAny> = run_id(&request.run_id.bind(py).clone())?;
    let loaded_macros: Bound<'py, PyAny> = host::entry(py, UserCodeHost::LoadProjectMacros)?
        .call1((native_field(&discovered, "macro_files")?,))?;
    let syntax: Bound<'py, PyAny> = native_field(&adapter_context, "sql_lexical_syntax")?;
    let declaration_scope: Bound<'py, PyAny> = build_declaration_scope(
        &discovered,
        &loaded_macros,
        &syntax,
        Some(target.compile_cache_dir.clone()),
    )?;
    let value_renderer: Bound<'py, PyAny> = native_field(&adapter_context, "value_renderer")?;
    let model_context: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::ModelInputBuildContext,
        &[
            ("effective_vars", target.vars.clone()),
            ("effective_settings", target.settings.clone()),
            ("target_config", target.target.clone()),
            ("effective_target_name", target.target_name.clone()),
            ("run_id", run_id.clone()),
            ("macro_context", target.macro_context.clone()),
            (
                "loaded_macros",
                native_field(&declaration_scope, "loaded_macros")?,
            ),
            ("value_renderer", value_renderer.clone()),
            (
                "collection_rendering",
                native_field(&adapter_context, "collection_rendering")?,
            ),
            ("sql_lexical_syntax", syntax.clone()),
            (
                "declaration_resolver",
                native_field(&declaration_scope, "resolver")?,
            ),
        ],
    )?;
    let models: ModelBuild<'py> = model_build(request, &discovered, &model_context)?;
    let attached: AttachedResources<'py> = attached_resources(
        request,
        &adapter_context,
        (&discovered, &declaration_scope),
        &models,
    )?;
    let fields: Bound<'py, PyDict> = keywords(
        py,
        &[
            (
                "project_config",
                native_field(&discovered, "project_config")?,
            ),
            ("local_config", native_field(&discovered, "local_config")?),
            ("discovered_inputs", discovered.clone()),
            ("sql_lexical_syntax", syntax),
            ("run_id", run_id),
            ("effective_target_name", target.target_name.clone()),
            ("effective_target", target.target.clone()),
            ("compile_cache_dir", target.compile_cache_dir.clone()),
            (
                "effective_connection",
                effective_connection(request, &discovered, &target)?,
            ),
            ("effective_settings", target.settings.clone()),
            (
                "no_sql_validation",
                request.no_sql_validation.bind(py).clone(),
            ),
            ("effective_vars", target.vars.clone()),
            ("macro_context", target.macro_context.clone()),
            (
                "loaded_macros",
                native_field(&declaration_scope, "loaded_macros")?,
            ),
            ("public_enums", models.enums.clone()),
            ("public_constants", models.constants.clone()),
            ("model_inputs", models.inputs.clone()),
            ("seed_inputs", attached.seeds),
            ("source_inputs", attached.sources),
            ("sql_function_inputs", attached.functions),
            ("test_inputs", attached.checks.get_item(0)?),
            ("scenario_inputs", attached.checks.get_item(1)?),
            ("audit_inputs", attached.checks.get_item(2)?),
            ("diagnostics", attached.checks.get_item(3)?),
            (
                "external_sql_reference_resolver",
                request.external_sql_reference_resolver.bind(py).clone(),
            ),
            ("scope_index", native_field(&declaration_scope, "index")?),
            ("declaration_scope", declaration_scope),
        ],
    )?;
    compile_inputs_view(&fields, Some(models.native_project.cast_into()?))
}

/// The selected target with the macro call store, and the effective vars, settings and macro context.
fn target_context<'py>(
    request: &InputsRequest,
    discovered: &Bound<'py, PyAny>,
    adapter_context: &Bound<'py, PyAny>,
) -> PyResult<TargetContext<'py>> {
    let py: Python<'py> = discovered.py();
    let selected: Bound<'py, PyAny> = call_with(
        &host::entry(py, UserCodeHost::TargetContextWithMacroCallStore)?,
        &[
            ("discovered_inputs", discovered.clone()),
            ("selected_target", request.selected_target.bind(py).clone()),
            ("no_cache", request.no_cache.bind(py).clone()),
        ],
    )?;
    let target_name: Bound<'py, PyAny> = selected.get_item(0)?;
    let target: Bound<'py, PyAny> = selected.get_item(1)?;
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let cli_vars: Bound<'py, PyAny> = request.cli_vars.bind(py).clone();
    let cli_vars: Bound<'py, PyAny> = if cli_vars.is_none() {
        PyDict::new(py).into_any()
    } else {
        cli_vars
    };
    let vars: Bound<'py, PyAny> = build_effective_vars(
        &project_config.clone(),
        &local_config.clone(),
        &target.clone(),
        &cli_vars,
    )?;
    let settings: Bound<'py, PyAny> =
        build_effective_settings(&project_config.clone(), &local_config.clone())?;
    let adapter_name: Bound<'py, PyAny> =
        resolve_effective_adapter_name(&project_config.clone(), &local_config)?;
    let macro_context: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::MacroContext,
        &[
            ("adapter_name", adapter_name),
            (
                "sql_analysis_enabled",
                native_field(&settings, "sql_analysis")?,
            ),
            ("target_name", target_name.clone()),
            ("vars", vars.clone()),
            (
                "_value_renderer",
                native_field(adapter_context, "value_renderer")?,
            ),
            (
                "_collection_rendering",
                native_field(adapter_context, "collection_rendering")?,
            ),
            (
                "_lexical_syntax",
                native_field(adapter_context, "sql_lexical_syntax")?,
            ),
            (
                "_enforce_explicit_references",
                native_field(&project_config, "references")?.getattr("enforce_explicit")?,
            ),
        ],
    )?;
    Ok(TargetContext {
        target_name,
        target,
        compile_cache_dir: selected.get_item(2)?,
        vars,
        settings,
        macro_context,
    })
}

/// `_build_models_with_declarations`: attach every model with the public declaration indexes.
fn model_build<'py>(
    request: &InputsRequest,
    discovered: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<ModelBuild<'py>> {
    let py: Python<'py> = discovered.py();
    let (enums, constants) = public_declaration_indexes(discovered.clone())?;
    let (enums, constants): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        (enums.into_any(), constants.into_any());
    let schemas: Bound<'py, PyAny> = public_model_schema_index(discovered.clone())?.into_any();
    let native_project: Bound<'py, PyAny> =
        Bound::new(py, NativeCompiledProject::default())?.into_any();
    let model_context: Bound<'py, PyAny> = replaced(
        context,
        &[
            ("public_enums", enums.clone()),
            ("public_constants", constants.clone()),
            ("public_model_schemas", schemas),
        ],
    )?;
    let inputs: Bound<'py, PyAny> = build_model_inputs(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("context", model_context),
                (
                    "no_sql_validation",
                    request.no_sql_validation.bind(py).clone(),
                ),
                (
                    "defer_model_sql_validation",
                    request.defer_model_sql_validation.bind(py).clone(),
                ),
                (
                    "external_sql_reference_resolver",
                    request.external_sql_reference_resolver.bind(py).clone(),
                ),
                ("native_project", native_project.clone()),
            ],
        )?),
    )?
    .into_any();
    let context: Bound<'py, PyAny> = replaced(
        context,
        &[
            ("public_enums", enums.clone()),
            ("public_constants", constants.clone()),
        ],
    )?;
    Ok(ModelBuild {
        inputs,
        native_project,
        enums,
        constants,
        context,
    })
}

/// The seeds, functions, sources and checks attached to the models.
fn attached_resources<'py>(
    request: &InputsRequest,
    adapter_context: &Bound<'py, PyAny>,
    (discovered, declaration_scope): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    models: &ModelBuild<'py>,
) -> PyResult<AttachedResources<'py>> {
    let py: Python<'py> = discovered.py();
    let context: &Bound<'py, PyAny> = &models.context;
    let settings: Bound<'py, PyAny> = native_field(context, "effective_settings")?;
    let resources: Bound<'py, PyAny> = attach_resources(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("native_project", models.native_project.clone()),
                (
                    "settings",
                    PyTuple::new(
                        py,
                        [
                            settings.clone(),
                            native_field(context, "target_config")?,
                            native_field(context, "macro_context")?.getattr("adapter_name")?,
                        ],
                    )?
                    .into_any(),
                ),
                (
                    "no_sql_validation",
                    request.no_sql_validation.bind(py).clone(),
                ),
                (
                    "python_functions_inherit_default_namespace",
                    native_field(
                        adapter_context,
                        "python_functions_inherit_default_namespace",
                    )?,
                ),
            ],
        )?),
    )?
    .into_any();
    let sources: Bound<'py, PyAny> = resources.get_item(2)?;
    let checks: Bound<'py, PyAny> = attach_checks(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("native_project", models.native_project.clone()),
                (
                    "settings",
                    PyTuple::new(
                        py,
                        [
                            settings,
                            native_field(adapter_context, "value_renderer")?,
                            native_field(context, "sql_lexical_syntax")?,
                        ],
                    )?
                    .into_any(),
                ),
                (
                    "scan_cache",
                    native_field(declaration_scope, "sql_test_scans")?,
                ),
                ("source_inputs", sources.clone()),
            ],
        )?),
    )?
    .into_any();
    Ok(AttachedResources {
        seeds: resources.get_item(0)?,
        functions: resources.get_item(1)?,
        sources,
        checks,
    })
}

/// The resolved connection when given, otherwise the configured one for the target.
fn effective_connection<'py>(
    request: &InputsRequest,
    discovered: &Bound<'py, PyAny>,
    target: &TargetContext<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let resolved: Bound<'py, PyAny> = request.resolved_connection.bind(discovered.py()).clone();
    if !resolved.is_none() {
        return Ok(resolved);
    }
    build_effective_connection(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &target.target.clone(),
        &target.vars.clone(),
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_compile_inputs, module)?)
}
