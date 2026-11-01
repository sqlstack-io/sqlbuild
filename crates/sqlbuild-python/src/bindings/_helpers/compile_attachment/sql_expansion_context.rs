//! The SQL expansion inputs lint uses to expand authored SQL outside a compile.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods};

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compile_attachment::declaration_scope::build_declaration_scope;
use crate::bindings::_helpers::compile_attachment::declaration_scope::rebind_declaration_scope;
use crate::bindings::_helpers::compile_attachment::effective_config::{
    build_effective_vars, target_selection,
};
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_collection_rendering;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::render::authored_sql::SqlExpansionInputs;
use crate::bindings::_helpers::render::declaration_indexes::public_declaration_indexes;
use pyo3::IntoPyObjectExt;

/// Build the declaration scope of `discovered`; a static scope lends only its index to macros.
fn declaration_scope<'py>(
    discovered: &Bound<'py, PyAny>,
    static_scope: Bound<'py, PyAny>,
    sql_lexical_syntax: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let loaded_macros: Bound<'py, PyAny> = host::entry(py, UserCodeHost::LoadProjectMacros)?
        .call1((native_field(discovered, "macro_files")?,))?;
    if !static_scope.is_none() {
        let rebound: Bound<'py, PyAny> =
            rebind_declaration_scope(&static_scope, &discovered.clone(), &loaded_macros.clone())
                .and_then(|value| value.into_bound_py_any(py))?;
        if !rebound.is_none() {
            return Ok(rebound);
        }
    }
    build_declaration_scope(
        &discovered.clone(),
        &loaded_macros,
        &sql_lexical_syntax,
        None,
    )
}

/// Each model file's own enum and constant declarations, by file path.
fn local_declarations<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = discovered.py();
    let by_name = |declarations: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
        let named: Bound<'py, PyDict> = PyDict::new(py);
        for declaration in declarations.try_iter()? {
            let declaration: Bound<'py, PyAny> = declaration?;
            named.set_item(native_field(&declaration, "name")?, declaration)?;
        }
        Ok(named.into_any())
    };
    let local: Bound<'py, PyDict> = PyDict::new(py);
    for model_file in native_field(discovered, "model_files")?.try_iter()? {
        let model_file: Bound<'py, PyAny> = model_file?;
        local.set_item(
            native_field(&model_file, "file_path")?,
            new_record_with(
                py,
                RecordId::DeclarationResolutionContext,
                &[
                    (
                        "enums",
                        by_name(native_field(&model_file, "enum_declarations")?)?,
                    ),
                    (
                        "constants",
                        by_name(native_field(&model_file, "constant_declarations")?)?,
                    ),
                ],
            )?,
        )?;
    }
    Ok(local)
}

/// Where SQL expansion inputs come from: the project, its discovery and the scope to reuse.
pub(crate) struct ExpansionSources<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) value_renderer: Bound<'py, PyAny>,
    pub(crate) vars: ExpansionVars<'py>,
    pub(crate) discovered_inputs: Option<Bound<'py, PyAny>>,
    pub(crate) declaration_scope: Option<Bound<'py, PyAny>>,
    pub(crate) static_declaration_scope: Option<Bound<'py, PyAny>>,
}

/// Where expansion takes its target and vars from.
pub(crate) enum ExpansionVars<'py> {
    /// The default target's effective vars, overridden by these CLI vars when given.
    DefaultTarget(Option<Bound<'py, PyAny>>),
    /// The target and vars of the compiling command's macro context.
    Compile(Bound<'py, PyAny>),
}

/// Assemble SQL expansion inputs; a static scope lends only its index to private macros.
pub(crate) fn sql_expansion_inputs(sources: ExpansionSources<'_>) -> PyResult<SqlExpansionInputs> {
    let py: Python<'_> = sources.project_dir.py();
    let value_renderer = sources.value_renderer;
    let discovered = match sources.discovered_inputs {
        Some(discovered) => discovered,
        None => discover_project_inputs(
            py,
            Some(keywords(
                py,
                &[
                    ("project_dir", sources.project_dir.clone()),
                    (
                        "sql_analysis_enabled_override",
                        PyBool::new(py, false).to_owned().into_any(),
                    ),
                ],
            )?),
        )?,
    };
    let project_config: Bound<'_, PyAny> = native_field(&discovered, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(&discovered, "local_config")?;
    let (target_name, effective_vars): (Bound<'_, PyAny>, Bound<'_, PyAny>) = match sources.vars {
        ExpansionVars::Compile(context) => (
            native_field(&context, "target_name")?,
            native_field(&context, "vars")?,
        ),
        ExpansionVars::DefaultTarget(cli_vars) => {
            let cli_vars: Bound<'_, PyAny> = cli_vars.unwrap_or_else(|| PyDict::new(py).into_any());
            let (target_name, target_config) = target_selection(&discovered, None)?;
            let vars: Bound<'_, PyAny> = build_effective_vars(
                &project_config.clone(),
                &local_config.clone(),
                &target_config,
                &cli_vars,
            )?;
            (target_name, vars)
        }
    };
    let scope: Bound<'_, PyAny> = match sources.declaration_scope {
        Some(scope) => scope,
        None => declaration_scope(
            &discovered,
            sources
                .static_declaration_scope
                .unwrap_or_else(|| py.None().into_bound(py)),
            native_field(&value_renderer, "sql_lexical_syntax")?,
        )?,
    };
    let (enums, constants): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
        public_declaration_indexes(discovered.clone())
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
            .extract()?;
    let collection_rendering: Bound<'_, PyAny> =
        resolve_effective_collection_rendering(&project_config.clone(), &py.None().into_bound(py))
            .and_then(|value| value.into_bound_py_any(py))?;
    let macro_context: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::MacroContext,
        &[
            (
                "adapter_name",
                resolve_effective_adapter_name(&project_config.clone(), &local_config)
                    .and_then(|value| value.into_bound_py_any(py))?,
            ),
            (
                "sql_analysis_enabled",
                PyBool::new(py, false).to_owned().into_any(),
            ),
            ("target_name", target_name),
            ("vars", effective_vars.clone()),
            ("_value_renderer", value_renderer.clone()),
            ("_collection_rendering", collection_rendering.clone()),
            (
                "_enforce_explicit_references",
                native_field(&project_config, "references")?.getattr("enforce_explicit")?,
            ),
            (
                "_lexical_syntax",
                native_field(&value_renderer, "sql_lexical_syntax")?,
            ),
        ],
    )?;
    Ok(SqlExpansionInputs {
        effective_vars: effective_vars.unbind(),
        loaded_macros: native_field(&scope, "loaded_macros")?.unbind(),
        macro_context: macro_context.unbind(),
        enums: enums.unbind(),
        constants: constants.unbind(),
        value_renderer: value_renderer.unbind(),
        collection_rendering: collection_rendering.unbind(),
        local_declarations: local_declarations(&discovered)?.unbind(),
        declaration_resolver: native_field(&scope, "resolver")?.unbind(),
    })
}

/// `value`, or `None` when it is Python `None`.
pub(crate) fn present(value: Bound<'_, PyAny>) -> Option<Bound<'_, PyAny>> {
    (!value.is_none()).then_some(value)
}

/// Assemble SQL expansion inputs; a static scope lends only its index to private macros.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_sql_expansion_context<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_sql_expansion_context",
        options,
        (
            &["project_dir", "value_renderer"],
            &[
                "cli_vars",
                "discovered_inputs",
                "declaration_scope",
                "static_declaration_scope",
            ],
        ),
    )?;
    sql_expansion_inputs(ExpansionSources {
        project_dir: arguments.value("project_dir")?,
        value_renderer: arguments.value("value_renderer")?,
        vars: ExpansionVars::DefaultTarget(present(arguments.value("cli_vars")?)),
        discovered_inputs: present(arguments.value("discovered_inputs")?),
        declaration_scope: present(arguments.value("declaration_scope")?),
        static_declaration_scope: present(arguments.value("static_declaration_scope")?),
    })?
    .record(py)
}
