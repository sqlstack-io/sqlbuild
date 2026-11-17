//! The first attached compile input snapshot: target, vars, macros, scopes, models and resources.

use crate::compile_driver::main::setup::attached_inputs::attached_inputs;
use crate::compile_driver::models::InputsRequest;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::compile_attachment::main::build_declaration_scope::build_declaration_scope;
use crate::compile_attachment::main::build_model_inputs::build_model_inputs;
use crate::input_loops::main::attach_checks::attach_checks;
use crate::input_loops::main::attach_resources::attach_resources;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection::build_effective_connection;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings::build_effective_settings;
use sqlbuild_python_front::compile_attachment::main::build_effective_vars::build_effective_vars;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::model_config::main::config::run_id::run_id;
use sqlbuild_python_front::render::main::declarations::public_declaration_indexes::public_declaration_indexes;
use sqlbuild_python_front::render::main::declarations::public_model_schema_index::public_model_schema_index;
use sqlbuild_python_project::compiled_project::models::NativeCompiledProject;
use sqlbuild_python_project::compiled_views::main::project::compile_inputs_view::compile_inputs_view;

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
pub(crate) fn build_compile_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "build_compile_inputs", options, (&REQUIRED, &OPTIONAL))?;
    attached_inputs(py, &InputsRequest::read(&arguments)?)
}

/// The undecorated `build_compile_inputs`.
pub(crate) fn compile_inputs<'py>(
    py: Python<'py>,
    request: &InputsRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let discovered: Bound<'py, PyAny> = request.discovered_inputs.bind(py).clone();
    let adapter_context: Bound<'py, PyAny> = request.adapter_context.bind(py).clone();
    let target: TargetContext<'py> = target_context(request, &discovered, &adapter_context)?;
    let run_id: Bound<'py, PyAny> = run_id(&request.run_id.bind(py).clone())?;
    let loaded_macros: Bound<'py, PyAny> = host_entry(py, UserCodeHost::LoadProjectMacros)?
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
            (
                "sql_analysis_dialect",
                native_field(&adapter_context, "sql_analysis_dialect")?,
            ),
        ],
    )?;
    let models: ModelBuild<'py> = model_build(
        request,
        &discovered,
        (&model_context, &target.compile_cache_dir),
    )?;
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
        &host_entry(py, UserCodeHost::TargetContextWithMacroCallStore)?,
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
    (context, compile_cache_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
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
                ("compile_cache_dir", compile_cache_dir.clone()),
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
