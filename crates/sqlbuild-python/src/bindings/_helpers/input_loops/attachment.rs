//! Seeds, functions, sources, SQL tests, scenarios and audits attached from discovered inputs.

use std::collections::HashMap;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_attachments::audits::constants::BUILTIN_AUDITS;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_host::entries::models::{ExecutorHost, FacadeHost, UserCodeHost};
use sqlbuild_rules::lint_text::constants::{DEFAULT_LINE_WIDTH, DEFAULT_MAX_DESCRIPTION_LINES};
use sqlbuild_rules::lint_text::main::resolve_format_settings::resolve_format_settings;
use sqlbuild_rules::lint_text::models::FormatSettings;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditBlockRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditFileRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::DiagnosticSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::LoaderRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::ScenarioRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SourceRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::TestBlockRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::TestCaseRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::TestFileRow;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::input_loops::resource_attachment::{
    attach_audits, attach_functions, attach_sources, attach_sql_tests,
};
use crate::bindings::_helpers::input_loops::resource_diagnostics::resource_diagnostics;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, NativeAttr, RecordId};
use crate::bindings::_helpers::records::registry;
use crate::bindings::_helpers::sql_test_glue::constants::SQL_TEST_CTE_SCAN_ALGORITHM;

const YAML_FILE_SUFFIXES: [&str; 2] = [".yml", ".yaml"];
const PATHLIB_MODULE: &str = "pathlib";
const TEST_TIMING_KEY: &str = "test_input_compile_ms";
const DESCRIPTION_KEY: &str = "description";
const MEASUREMENT_MODE: &str = "MEASUREMENT";

/// `getattr(value, name)` extracted.
fn attr<'py, T: pyo3::FromPyObject<'py>>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<T> {
    native_field(value, name)?.extract()
}

/// `str(value.<name>)`.
fn text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(value, name)?.str()?.to_string())
}

/// `value.<path>.stem`.
fn stem(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    attr(&native_field(value, name)?, "stem")
}

/// Each item of the iterable `value.<name>`.
fn items<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(value, name)?.try_iter()?.collect()
}

/// `{member.value: member for member in enum_class}` of a native string enum.
fn native_by_value(py: Python<'_>, class: RecordId) -> PyResult<Bound<'_, PyDict>> {
    members_by_value(&record_class(py, class)?)
}

fn members_by_value<'py>(enum_class: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let members: Bound<'py, PyDict> = PyDict::new(enum_class.py());
    for member in enum_class.try_iter()? {
        let member: Bound<'py, PyAny> = member?;
        members.set_item(native_field(&member, "value")?, member)?;
    }
    Ok(members)
}

fn seed_declarations<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<Vec<(Py<PyAny>, Py<PyAny>)>> {
    let mut declarations: Vec<(Py<PyAny>, Py<PyAny>)> = Vec::new();
    for schema_file in items(discovered, "schema_files")? {
        for entry in items(&schema_file, "seed_entries")? {
            declarations.push((entry.unbind(), schema_file.clone().unbind()));
        }
    }
    Ok(declarations)
}

/// The function rows of `field`; Python function files have no authored SQL contents.
fn function_rows(
    discovered: &Bound<'_, PyAny>,
    (field, body): (&str, &str),
    python: bool,
) -> PyResult<Vec<FunctionRow>> {
    items(discovered, field)?
        .into_iter()
        .map(|file| {
            Ok(FunctionRow {
                file_path: text(&file, "file_path")?,
                relative_path: text(&file, "relative_path")?,
                name: stem(&file, "file_path")?,
                function_file: file.clone().unbind(),
                header_values: attr(&file, "header_values")?,
                body: attr(&file, body)?,
                contents: if python {
                    None
                } else {
                    Some(attr(&file, "contents")?)
                },
            })
        })
        .collect()
}

/// `override if override is not None else fallback`.
fn either<'py>(
    defaults: &Bound<'py, PyAny>,
    (preferred, fallback): (&str, &str),
) -> PyResult<Option<String>> {
    let value: Bound<'py, PyAny> = native_field(defaults, preferred)?;
    if value.is_none() {
        return attr(defaults, fallback);
    }
    value.extract()
}

/// `(database, schema)` of a target config, or `None`.
fn target_namespace(
    target: &Bound<'_, PyAny>,
) -> PyResult<Option<(Option<String>, Option<String>)>> {
    if target.is_none() {
        return Ok(None);
    }
    Ok(Some((attr(target, "database")?, attr(target, "schema")?)))
}

/// Pair every seed, then attach every function and source, in that order.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn attach_resources<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "attach_resources",
        options,
        (
            &[
                "discovered_inputs",
                "native_project",
                "settings",
                "no_sql_validation",
                "python_functions_inherit_default_namespace",
            ],
            &[],
        ),
    )?;
    let discovered_inputs: &Bound<'py, PyAny> = &arguments.value("discovered_inputs")?;
    let native_project: &Bound<'py, NativeCompiledProject> =
        &arguments.value("native_project")?.downcast_into()?;
    let (effective_settings, target_config, adapter_name): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        String,
    ) = arguments.value("settings")?.extract()?;
    let no_sql_validation: bool = arguments.flag("no_sql_validation")?;
    let python_functions_inherit_default_namespace: bool =
        arguments.flag("python_functions_inherit_default_namespace")?;
    let validate: bool = attr::<bool>(&effective_settings, "sql_analysis")? && !no_sql_validation;
    let seeds = NativeCompiledProject::attach_seeds(
        native_project,
        seed_declarations(discovered_inputs)?,
        items(discovered_inputs, "seed_files")?
            .into_iter()
            .map(Bound::unbind)
            .collect(),
        &error_class(py, ErrorId::CompileInput)?,
    )?;
    let defaults: Bound<'py, PyAny> =
        native_field(discovered_inputs, "project_config")?.getattr("defaults")?;
    let function_kind: Bound<'py, PyAny> = enum_member(py, RecordId::ResourceKind, "FUNCTION")?;
    let functions = attach_functions(
        native_project,
        (
            function_rows(discovered_inputs, ("sql_function_files", "body_sql"), false)?,
            function_rows(
                discovered_inputs,
                ("python_function_files", "body_python"),
                true,
            )?,
        ),
        FunctionSettings {
            adapter_name,
            validate,
            defaults: (
                either(&defaults, ("function_database", "database"))?,
                either(&defaults, ("function_schema", "schema"))?,
            ),
            target: target_namespace(&target_config)?,
            inherit: python_functions_inherit_default_namespace,
            kind: function_kind.unbind(),
        },
    )?;
    let mut source_rows: Vec<SourceRow> = Vec::new();
    for source_file in items(discovered_inputs, "source_files")? {
        let file_path: String = text(&source_file, "file_path")?;
        for entry in items(&source_file, "source_entries")? {
            source_rows.push((
                file_path.clone(),
                source_file.clone().unbind(),
                entry.unbind(),
            ));
        }
    }
    let sources = attach_sources(
        native_project,
        source_rows,
        (
            attr(&defaults, "contract")?,
            validate,
            enum_member(py, RecordId::ResourceKind, "SOURCE")?.unbind(),
        ),
    )?;
    PyTuple::new(
        py,
        [
            PyTuple::new(py, seeds)?.into_any(),
            PyTuple::new(py, functions)?.into_any(),
            PyTuple::new(py, sources)?.into_any(),
        ],
    )
}

fn test_case_row(case: Bound<'_, PyAny>) -> PyResult<TestCaseRow> {
    Ok((
        case.clone().unbind(),
        attr(&case, "name")?,
        items(&case, "values")?
            .into_iter()
            .map(|pair| pair.extract())
            .collect::<PyResult<_>>()?,
    ))
}

fn test_block_row(block: Bound<'_, PyAny>) -> PyResult<TestBlockRow> {
    Ok(TestBlockRow {
        block: block.clone().unbind(),
        name: attr(&block, "name")?,
        sql_body: attr(&block, "sql_body")?,
        mode: attr(&native_field(&block, "mode")?, "value")?,
        parameters: items(&block, "parameters")?
            .iter()
            .map(|parameter| attr(parameter, "name"))
            .collect::<PyResult<_>>()?,
        cases: items(&block, "cases")?
            .into_iter()
            .map(test_case_row)
            .collect::<PyResult<_>>()?,
        sql_body_span: attr(&block, "sql_body_span")?,
    })
}

fn test_file_row(file: Bound<'_, PyAny>) -> PyResult<TestFileRow> {
    Ok(TestFileRow {
        file_path: text(&file, "file_path")?,
        relative_path: text(&file, "relative_path")?,
        stem: stem(&file, "relative_path")?,
        file: file.clone().unbind(),
        contents: attr(&file, "contents")?,
        blocks: items(&file, "blocks")?
            .into_iter()
            .map(test_block_row)
            .collect::<PyResult<_>>()?,
    })
}

fn scenario_row(scenario: Bound<'_, PyAny>) -> PyResult<ScenarioRow> {
    Ok(ScenarioRow {
        file_path: text(&scenario, "file_path")?,
        relative_path: text(&scenario, "relative_path")?,
        scenario_file: scenario.clone().unbind(),
        contents: attr(&scenario, "contents")?,
        sql_body: attr(&scenario, "sql_body")?,
        name: attr(&scenario, "name")?,
    })
}

/// The adapter's typed scalar renderer as `(text, None)`, or `(None, error)` for a bad value.
fn parameter_renderer<'py>(value_renderer: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value_renderer.py();
    let renderer: Py<PyAny> = value_renderer.clone().unbind();
    let errors: Py<PyAny> = PyTuple::new(
        py,
        [
            host::entry(py, ExecutorHost::SqlValueRenderingError)?,
            registry::native(py, NativeAttr::SqlValueValidationError)?,
        ],
    )?
    .into_any()
    .unbind();
    let render = move |arguments: &Bound<'_, PyTuple>,
                       _: Option<&Bound<'_, PyDict>>|
          -> PyResult<(Option<String>, Option<String>)> {
        let py: Python<'_> = arguments.py();
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("value", arguments.get_item(0)?)?;
        match renderer
            .bind(py)
            .call_method("render_typed_scalar", (), Some(&keywords))
        {
            Ok(rendered) => Ok((Some(rendered.extract()?), None)),
            Err(error) if error.matches(py, errors.bind(py))? => {
                Ok((None, Some(error.value(py).str()?.to_string())))
            }
            Err(error) => Err(error),
        }
    };
    Ok(PyCFunction::new_closure(py, None, None, render)?.into_any())
}

fn sql_test_settings<'py>(
    py: Python<'py>,
    (value_renderer, syntax, scan_cache): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<Bound<'py, PyDict>> {
    let settings: Bound<'py, PyDict> = PyDict::new(py);
    for (key, class) in [
        ("test_input", RecordId::CompileSqlTestInput),
        ("model_payload", RecordId::CompileModelSqlTestInputPayload),
        (
            "direct_payload",
            RecordId::CompileDirectLogicSqlTestInputPayload,
        ),
        ("test_cte", RecordId::CompileSqlTestCte),
        ("scenario_input", RecordId::CompileSqlScenarioInput),
        ("scenario_cte", RecordId::CompileSqlScenarioCte),
    ] {
        settings.set_item(key, record_class(py, class)?)?;
    }
    settings.set_item(
        "modes",
        members_by_value(&host::entry(py, FacadeHost::SqlTestMode)?)?,
    )?;
    settings.set_item(
        "test_kind",
        enum_member(py, RecordId::ResourceKind, "TEST")?,
    )?;
    settings.set_item(
        "scenario_kind",
        enum_member(py, RecordId::ResourceKind, "SCENARIO")?,
    )?;
    settings.set_item(
        "runtime_usage",
        enum_member(py, RecordId::UsageKind, "RUNTIME")?,
    )?;
    settings.set_item("render_parameter", parameter_renderer(value_renderer)?)?;
    settings.set_item(
        "adapter_name",
        native_field(value_renderer, "adapter_name")?,
    )?;
    settings.set_item(
        "source_location",
        record_class(py, RecordId::SourceLocation)?,
    )?;
    settings.set_item("scan_store", scan_cache.call_method0("opened")?)?;
    settings.set_item(
        "scan_key",
        (
            SQL_TEST_CTE_SCAN_ALGORITHM,
            native_field(syntax, "cache_key")?,
        ),
    )?;
    Ok(settings)
}

fn audit_block_row(block: Bound<'_, PyAny>) -> PyResult<AuditBlockRow> {
    let py: Python<'_> = block.py();
    let measurement: Bound<'_, PyAny> =
        host::entry(py, FacadeHost::AuditEvaluationMode)?.getattr(MEASUREMENT_MODE)?;
    Ok(AuditBlockRow {
        block: block.clone().unbind(),
        name: attr(&block, "name")?,
        sql_body: attr(&block, "sql_body")?,
        evidence_sql: attr(&block, "evidence_sql")?,
        measurement: native_field(&block, "evaluation_mode")?.eq(measurement)?,
        header: attr(&block, "header_values")?,
    })
}

fn audit_file_row(file: Bound<'_, PyAny>) -> PyResult<AuditFileRow> {
    let py: Python<'_> = file.py();
    let generic: Bound<'_, PyAny> = enum_member(py, RecordId::DeclarationKind, "AUDIT")?;
    Ok(AuditFileRow {
        file_path: text(&file, "file_path")?,
        relative_path: text(&file, "relative_path")?,
        stem: stem(&file, "file_path")?,
        file: file.clone().unbind(),
        generic: native_field(&file, "declaration_kind")?.is(&generic),
        contents: attr(&file, "contents")?,
        blocks: items(&file, "blocks")?
            .into_iter()
            .map(audit_block_row)
            .collect::<PyResult<_>>()?,
    })
}

/// The discovered file of one built-in generic audit, as project discovery would describe it.
fn builtin_audit_file<'py>(
    py: Python<'py>,
    (name, sql_body): (&str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let path_class: Bound<'py, PyAny> = imported(py, PATHLIB_MODULE, "Path")?;
    let relative_path: Bound<'py, PyAny> =
        path_class.call1((format!("audits/generic/{name}.sql"),))?;
    let block: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::DiscoveredAuditBlock,
        &[
            ("audit_index", 0.into_bound_py_any(py)?),
            ("header_values", PyDict::new(py).into_any()),
            ("sql_body", sql_body.into_bound_py_any(py)?),
        ],
    )?;
    new_record_with(
        py,
        RecordId::DiscoveredAuditFile,
        &[
            (
                "file_path",
                path_class.call1(("<built-in>",))?.div(&relative_path)?,
            ),
            ("relative_path", relative_path),
            (
                "contents",
                format!("AUDIT ();\n\n{sql_body}\n").into_bound_py_any(py)?,
            ),
            ("blocks", PyTuple::new(py, [block])?.into_any()),
        ],
    )
}

fn audit_settings<'py>(
    py: Python<'py>,
    effective_settings: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let settings: Bound<'py, PyDict> = PyDict::new(py);
    let severity: Bound<'py, PyAny> = native_field(effective_settings, "default_audit_severity")?;
    settings.set_item(
        "default_severity",
        if severity.is_none() {
            None
        } else {
            Some(severity.str()?)
        },
    )?;
    settings.set_item(
        "default_run_scope",
        native_field(effective_settings, "default_audit_run_scope")?,
    )?;
    let builtins: Vec<AuditFileRow> = BUILTIN_AUDITS
        .iter()
        .map(|definition| audit_file_row(builtin_audit_file(py, *definition)?))
        .collect::<PyResult<_>>()?;
    settings.set_item("builtins", builtins)?;
    settings.set_item(
        "declaration_identity",
        record_class(py, RecordId::DeclarationIdentity)?,
    )?;
    settings.set_item(
        "singular_kind",
        enum_member(py, RecordId::DeclarationKind, "SINGULAR_AUDIT")?,
    )?;
    settings.set_item(
        "audit_kind",
        enum_member(py, RecordId::DeclarationKind, "AUDIT")?,
    )?;
    settings.set_item(
        "resource_kinds",
        native_by_value(py, RecordId::ResourceKind)?,
    )?;
    Ok(settings)
}

/// Each loader's description, relative path and whether that path is a YAML file.
fn loader_rows<'py>(discovered: &Bound<'py, PyAny>) -> PyResult<HashMap<String, LoaderRow>> {
    let mut rows: HashMap<String, LoaderRow> = HashMap::new();
    for loader in items(discovered, "loader_functions")? {
        let relative_path: Bound<'py, PyAny> = native_field(&loader, "relative_path")?;
        rows.insert(
            attr(&loader, "name")?,
            (
                attr(&loader, DESCRIPTION_KEY)?,
                relative_path.str()?.to_string(),
                YAML_FILE_SUFFIXES.contains(
                    &native_field(&relative_path, "suffix")?
                        .extract::<String>()?
                        .as_str(),
                ),
            ),
        );
    }
    Ok(rows)
}

fn diagnostic_settings<'py>(
    discovered: &Bound<'py, PyAny>,
    source_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = discovered.py();
    let settings: Bound<'py, PyDict> = PyDict::new(py);
    settings.set_item(
        "compiler_diagnostic",
        record_class(py, RecordId::CompilerDiagnostic)?,
    )?;
    settings.set_item("compile_error", error_class(py, ErrorId::CompileInput)?)?;
    settings.set_item(
        "source_location",
        record_class(py, RecordId::SourceLocation)?,
    )?;
    settings.set_item("path_class", imported(py, PATHLIB_MODULE, "Path")?)?;
    settings.set_item("model_files", items(discovered, "model_files")?)?;
    let factories: Vec<(String, String, usize)> = items(discovered, "audit_factories")?
        .iter()
        .map(|factory| {
            Ok((
                attr(factory, "name")?,
                text(factory, "relative_path")?,
                attr(factory, "line")?,
            ))
        })
        .collect::<PyResult<_>>()?;
    settings.set_item("audit_factories", factories)?;
    let scenarios: Vec<Bound<'py, PyTuple>> = items(discovered, "scenario_files")?
        .iter()
        .map(|scenario| {
            PyTuple::new(
                py,
                [
                    native_field(scenario, "name")?,
                    text(scenario, "relative_path")?.into_bound_py_any(py)?,
                    native_field(scenario, "contents")?,
                    native_field(scenario, "header_values")?
                        .call_method1("get", (DESCRIPTION_KEY,))?,
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    settings.set_item("scenarios", scenarios)?;
    let hooks: Vec<Bound<'py, PyTuple>> = items(discovered, "sql_hook_files")?
        .iter()
        .map(|hook| {
            PyTuple::new(
                py,
                [
                    native_field(hook, "name")?,
                    text(hook, "relative_path")?.into_bound_py_any(py)?,
                    native_field(hook, "contents")?,
                    native_field(hook, DESCRIPTION_KEY)?,
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    settings.set_item("sql_hooks", hooks)?;
    settings.set_item("loaders", loader_rows(discovered)?)?;
    settings.set_item("description_limit", description_limit(discovered)?)?;
    settings.set_item(
        "python_resources",
        call_with(
            &host::entry(py, UserCodeHost::PythonResourceRows)?,
            &[
                ("discovered_inputs", discovered.clone()),
                ("source_inputs", source_inputs.clone()),
            ],
        )?,
    )?;
    Ok(settings)
}

/// The project's `max_description_lines` and `line_width`, as `sqb format` resolves them.
fn description_limit(discovered: &Bound<'_, PyAny>) -> PyResult<(usize, i64)> {
    let directory: Option<PathBuf> = native_field(discovered, "project_dir")?.extract()?;
    // Without a project directory there is no configuration to read, not the current directory's.
    let Some(directory) = directory else {
        return Ok((
            usize::try_from(DEFAULT_MAX_DESCRIPTION_LINES).unwrap_or(0),
            DEFAULT_LINE_WIDTH,
        ));
    };
    let settings: FormatSettings =
        resolve_format_settings(&directory).map_err(PyValueError::new_err)?;
    Ok((
        usize::try_from(settings.max_description_lines).unwrap_or(0),
        settings.line_width,
    ))
}

/// Attach every SQL test case, scenario and audit, then report every resource diagnostic.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn attach_checks<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "attach_checks",
        options,
        (
            &[
                "discovered_inputs",
                "native_project",
                "settings",
                "scan_cache",
                "source_inputs",
            ],
            &[],
        ),
    )?;
    let discovered_inputs: &Bound<'py, PyAny> = &arguments.value("discovered_inputs")?;
    let native_project: &Bound<'py, NativeCompiledProject> =
        &arguments.value("native_project")?.downcast_into()?;
    let (effective_settings, value_renderer, syntax): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = arguments.value("settings")?.extract()?;
    let scan_cache: &Bound<'py, PyAny> = &arguments.value("scan_cache")?;
    let source_inputs: &Bound<'py, PyAny> = &arguments.value("source_inputs")?;
    let (tests, scenarios) = record_compile_timing(TEST_TIMING_KEY, || -> PyResult<_> {
        let attached = attach_sql_tests(
            native_project,
            items(discovered_inputs, "test_files")?
                .into_iter()
                .map(test_file_row)
                .collect::<PyResult<_>>()?,
            items(discovered_inputs, "scenario_files")?
                .into_iter()
                .map(scenario_row)
                .collect::<PyResult<_>>()?,
            sql_test_settings(py, (&value_renderer, &syntax, scan_cache))?.extract()?,
        )?;
        scan_cache.call_method0("save")?;
        Ok(attached)
    })?;
    let (audits, audit_diagnostics) = attach_audits(
        native_project,
        items(discovered_inputs, "audit_files")?
            .into_iter()
            .map(audit_file_row)
            .collect::<PyResult<_>>()?,
        audit_settings(py, &effective_settings)?.extract()?,
    )?;
    let settings: DiagnosticSettings =
        diagnostic_settings(discovered_inputs, source_inputs)?.extract()?;
    let (orphans, descriptions, cursors) =
        resource_diagnostics(&native_project.borrow(), py, &settings)?;
    let diagnostics: Vec<Bound<'py, PyAny>> = audit_diagnostics
        .into_iter()
        .chain(orphans)
        .chain(descriptions)
        .chain(cursors)
        .collect();
    PyTuple::new(
        py,
        [
            PyTuple::new(py, tests)?.into_any(),
            PyTuple::new(py, scenarios)?.into_any(),
            PyTuple::new(py, audits)?.into_any(),
            PyTuple::new(py, diagnostics)?.into_any(),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(attach_resources, module)?)?;
    module.add_function(wrap_pyfunction!(attach_checks, module)?)
}
