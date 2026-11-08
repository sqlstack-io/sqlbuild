//! Every discovered model file attached as a model input by the native model loop.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_analysis::assembly::project::main::sql_analysis_opt_out_help::sql_analysis_opt_out_help;

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::attachments::cursor_intrinsics::reserved_cursor_markers;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::compile_attachment::model_config::native_model_config_session;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::model_input_view::NativeModelInput;
use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::input_loops::loop_classes::model_loop_classes;
use crate::bindings::_helpers::input_loops::model_loop::{LoopSwitches, NativeModelLoop};
use crate::bindings::_helpers::model_config::contract_classes::contract_classes;
use crate::bindings::_helpers::model_config::model_validator::NativeModelValidator;
use crate::bindings::_helpers::model_config::unicode_environment::unicode_environment;
use crate::bindings::_helpers::records::construct::{
    enum_member, new_record, new_record_with, record_class,
};
use crate::bindings::_helpers::render::declaration_values::ConstantRendering;
use crate::bindings::_helpers::render::macro_expansion_result::macro_expansion_classes;

use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
/// The explicit reference call kinds and the `SqlResourceRefKind` member each resolves to.
const EXPLICIT_REFERENCE_KINDS: [(&str, &str); 3] =
    [("ref", "MODEL"), ("source", "SOURCE"), ("seed", "SEED")];

/// The explicit reference call kinds mapped to their `SqlResourceRefKind` members.
fn explicit_reference_kinds(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let kinds: Bound<'_, PyAny> = host::entry(py, UserCodeHost::SqlResourceRefKind)?;
    let built: Bound<'_, PyDict> = PyDict::new(py);
    for (call_kind, member) in EXPLICIT_REFERENCE_KINDS {
        built.set_item(call_kind, native_field(&kinds, member)?)?;
    }
    Ok(built)
}

fn dict<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    let built: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in entries {
        built.set_item(key, value)?;
    }
    Ok(built)
}

/// The parser opt-out help as a Python string.
fn opt_out_help(py: Python<'_>, model_owned: bool) -> PyResult<Bound<'_, PyAny>> {
    Ok(PyString::new(py, &sql_analysis_opt_out_help(model_owned)).into_any())
}

/// The adapter's constant rendering every model, source and hook expansion shares.
fn constant_rendering<'py>(context: &Bound<'py, PyAny>) -> PyResult<Bound<'py, ConstantRendering>> {
    Bound::new(
        context.py(),
        ConstantRendering::new(
            native_field(context, "value_renderer")?.unbind(),
            native_field(context, "collection_rendering")?.unbind(),
        ),
    )
}

fn macro_inputs<'py>(context: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = context.py();
    dict(
        py,
        vec![
            ("loaded_macros", native_field(context, "loaded_macros")?),
            ("macro_context", native_field(context, "macro_context")?),
            (
                "bridge",
                host::entry(py, UserCodeHost::ActiveMacroBridge)?.call0()?,
            ),
            (
                "scoped",
                pyo3::types::PyBool::new(
                    py,
                    !native_field(context, "declaration_resolver")?.is_none(),
                )
                .to_owned()
                .into_any(),
            ),
            (
                "declaration_context",
                record_class(py, RecordId::DeclarationResolutionContext)?,
            ),
            (
                "macro_argument_error",
                error_class(py, ErrorId::MacroArgument)?,
            ),
            ("expansion", macro_expansion_classes(py)?.into_any()),
            (
                "generic_syntax",
                native_field(
                    &new_record(py, RecordId::SqlLexicalSyntax, (), None)?,
                    "native_mapping",
                )?,
            ),
        ],
    )
}

fn sql_hook_inputs<'py>(
    discovered: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = context.py();
    dict(
        py,
        vec![
            (
                "named_sql_hook_entry",
                record_class(py, RecordId::NamedSqlHookEntry)?,
            ),
            (
                "python_hook_entry",
                record_class(py, RecordId::PythonHookEntry)?,
            ),
            ("hook_files", native_field(discovered, "sql_hook_files")?),
            ("effective_vars", native_field(context, "effective_vars")?),
            ("environment", unicode_environment(py)?),
            (
                "render_project_var",
                pyo3::wrap_pyfunction!(crate::bindings::_helpers::authored_values::project_vars::render_project_var_text, py)?.into_any(),
            ),
            (
                "sql_resource_ref",
                host::entry(py, UserCodeHost::SqlResourceRef)?,
            ),
            ("explicit_kinds", explicit_reference_kinds(py)?.into_any()),
            (
                "declaration_identity",
                record_class(py, RecordId::DeclarationIdentity)?,
            ),
            (
                "sql_hook_kind",
                enum_member(py, RecordId::DeclarationKind, "SQL_HOOK")?,
            ),
        ],
    )
}

/// Attach schema metadata to discovered model files.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_model_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_model_inputs",
        options,
        (
            &["discovered_inputs", "context", "native_project"],
            &[
                "no_sql_validation",
                "defer_model_sql_validation",
                "external_sql_reference_resolver",
            ],
        ),
    )?;
    let discovered_inputs: &Bound<'py, PyAny> = &arguments.value("discovered_inputs")?;
    let context: &Bound<'py, PyAny> = &arguments.value("context")?;
    let native_project: &Bound<'py, PyAny> = &arguments.value("native_project")?;
    let no_sql_validation: bool = arguments.flag("no_sql_validation")?;
    let defer_model_sql_validation: bool = arguments.flag("defer_model_sql_validation")?;
    let external: Bound<'py, PyAny> = arguments.value("external_sql_reference_resolver")?;
    let settings: Bound<'py, PyAny> = native_field(context, "effective_settings")?;
    let validator: Bound<'py, PyAny> = Bound::new(
        py,
        NativeModelValidator::from_inputs(
            discovered_inputs.clone(),
            native_field(&settings, "microbatch_concurrency")?.extract()?,
        )?,
    )?
    .into_any();
    if !external.is_none() {
        call_with(
            &native_field(&external, "validate_model_names")?,
            &[("known_model_names", validator.call_method0("model_names")?)],
        )?;
    }
    let project_dir: Bound<'py, PyAny> = native_field(discovered_inputs, "project_dir")?;
    let root: Bound<'py, PyAny> = if project_dir.is_truthy()? {
        project_dir
    } else {
        imported(py, "pathlib", "Path")?.call0()?
    };
    let project_config_path: Bound<'py, PyAny> = root.div(PROJECT_CONFIG_FILENAME)?;
    let config_inputs: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::NativeModelConfigInputs,
        &[
            (
                "project_config",
                native_field(discovered_inputs, "project_config")?,
            ),
            ("target_config", native_field(context, "target_config")?),
            ("effective_vars", native_field(context, "effective_vars")?),
            (
                "effective_target_name",
                native_field(context, "effective_target_name")?,
            ),
            ("run_id", native_field(context, "run_id")?),
            (
                "microbatch_concurrency",
                native_field(&settings, "microbatch_concurrency")?,
            ),
        ],
    )?;
    let session: Bound<'py, PyAny> = native_model_config_session(&config_inputs, &validator)?;
    let provider_names: Vec<Bound<'py, PyAny>> = native_field(discovered_inputs, "providers")?
        .try_iter()?
        .map(|provider| native_field(&provider?, "name"))
        .collect::<PyResult<_>>()?;
    let host_inputs: Bound<'py, PyDict> = dict(
        py,
        vec![
            (
                "report_diagnostic",
                pyo3::wrap_pyfunction!(crate::bindings::_helpers::compile_context::diagnostic_collector::report_compile_diagnostic, py)?.into_any(),
            ),
            (
                "record_template_reads",
                pyo3::wrap_pyfunction!(crate::bindings::_helpers::model_config::template_data::record_template_reads, py)?.into_any(),
            ),
            (
                "visible_declarations",
                record_class(py, RecordId::VisibleModelDeclarations)?,
            ),
            ("config_session", session),
            ("syntax_help", opt_out_help(py, true)?),
            ("source_syntax_help", opt_out_help(py, false)?),
            ("macros", macro_inputs(context)?.into_any()),
            (
                "sql_hooks",
                sql_hook_inputs(discovered_inputs, context)?.into_any(),
            ),
            (
                "hook_functions",
                native_field(discovered_inputs, "hook_functions")?,
            ),
            (
                "provider_names",
                PyFrozenSet::new(py, &provider_names)?.into_any(),
            ),
        ],
    )?;
    let classes: Bound<'py, PyDict> = model_loop_classes(
        &native_field(discovered_inputs, "audit_factories")?,
        context,
        &external,
        &host_inputs,
    )?;
    let syntax: Bound<'py, PyAny> = native_field(context, "sql_lexical_syntax")?;
    let native_loop: Bound<'py, PyAny> =
        crate::bindings::_helpers::input_loops::model_loop::NativeModelLoop::new(
            (
                validator.unbind(),
                contract_classes(py)?.unbind(),
                classes.unbind(),
            ),
            native_field(&syntax, "native_mapping")?.extract()?,
            reserved_cursor_markers(),
            (
                LoopSwitches {
                    sql_analysis: native_field(&settings, "sql_analysis")?.extract()?,
                    no_sql_validation,
                    defer_sql_validation: defer_model_sql_validation,
                    require_sql_analysis: native_field(&settings, "require_sql_analysis")?
                        .extract()?,
                },
                project_config_path.str()?.extract()?,
                native_field(context, "sql_analysis_dialect")?.extract()?,
            ),
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?;
    let files: Vec<Py<NativeModelFile>> = native_field(discovered_inputs, "model_files")?
        .try_iter()?
        .map(|file| file?.extract())
        .collect::<PyResult<_>>()?;
    let inputs: Vec<Bound<'py, NativeModelInput>> = NativeModelLoop::run(
        native_loop.cast::<NativeModelLoop>()?,
        native_project.cast::<NativeCompiledProject>()?.clone(),
        constant_rendering(context)?,
        files,
    )?;
    PyTuple::new(py, inputs)
}
