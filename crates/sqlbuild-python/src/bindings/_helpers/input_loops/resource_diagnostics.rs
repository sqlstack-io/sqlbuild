//! Compile-input diagnostics of the project's resources, read from the native owner.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_analysis::contracts::main::join_helps::join_helps;
use sqlbuild_analysis::contracts::main::snippet_help::snippet_help;
use sqlbuild_analysis::resource_diagnostics::main::cursor_lacks_inputs::cursor_lacks_inputs;
use sqlbuild_analysis::resource_diagnostics::main::header_code_offset::header_code_offset;
use sqlbuild_analysis::resource_diagnostics::main::header_help::header_help;
use sqlbuild_analysis::resource_diagnostics::main::missing_description::missing_description;
use sqlbuild_analysis::resource_diagnostics::main::yaml_help::yaml_help;
use sqlbuild_analysis::resource_diagnostics::models::{CursorModel, HeaderHelp};
use sqlbuild_discovery::yaml_files::main::yaml_entry_lines::yaml_entry_lines;
use sqlbuild_pysource::definition_locations::main::decorator_location::decorator_location;
use sqlbuild_rules::dsl_headers::main::description_line_count::description_line_count;
use sqlbuild_sqltext::compiler::main::model_header_entry_span::model_header_entry_span;

use crate::bindings::_helpers::compile_context::delta::{
    DeltaInjection, capture, capturing, injection,
};
use crate::bindings::_helpers::compiled_project::attachment_inputs::{
    DiagnosticSettings, HookDescriptionRow, LoaderRow, PythonResourceRow, ResourceDiagnostics,
    ScenarioDescriptionRow,
};
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::seed_views::{SeedParts, SeedViews};
use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_project::models::ReferenceRow;

const MISSING_DESCRIPTION_CODE: &str = "P010";
const DESCRIPTION_LENGTH_CODE: &str = "P027";
const CURSOR_WITHOUT_INPUTS_CODE: &str = "P011";
const ORPHAN_FACTORY_CODE: &str = "C216";
const AUDIT_FACTORIES_KEY: &str = "audit_factories";
const CURSOR_KEY: &str = "cursor";
const CURSOR_INPUTS_KEY: &str = "cursor_inputs";
const PYTHON_UDF_DECORATOR: &str = "udf";

struct Diagnostics<'a, 'py> {
    py: Python<'py>,
    settings: &'a DiagnosticSettings,
    yaml_lines: RefCell<HashMap<String, HashMap<String, usize>>>,
}

impl<'py> Diagnostics<'_, 'py> {
    fn path(&self, relative_path: &str) -> PyResult<Bound<'py, PyAny>> {
        self.settings
            .path_class
            .bind(self.py)
            .call1((relative_path,))
    }

    fn location(
        &self,
        relative_path: &str,
        position: (usize, usize),
        end: Option<(usize, usize)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py);
        fields.set_item("path", self.path(relative_path)?)?;
        fields.set_item("line", position.0)?;
        fields.set_item("column", position.1)?;
        if let Some((end_line, end_column)) = end {
            fields.set_item("end_line", end_line)?;
            fields.set_item("end_column", end_column)?;
        }
        self.settings
            .source_location
            .bind(self.py)
            .call((), Some(&fields))
    }

    /// The first code after leading whitespace and comments, raising Python's comment error.
    fn header_location(&self, relative_path: &str, contents: &str) -> PyResult<Bound<'py, PyAny>> {
        let offset: usize = header_code_offset(contents).map_err(|message| {
            match self.settings.compile_error.bind(self.py).call1((message,)) {
                Ok(error) => pyo3::PyErr::from_value(error),
                Err(failure) => failure,
            }
        })?;
        let position: TextPosition = text_position(contents, offset);
        self.location(relative_path, (position.line, position.column), None)
    }

    fn yaml_location(
        &self,
        relative_path: &str,
        contents: &str,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let line: usize = {
            let mut files = self.yaml_lines.borrow_mut();
            let entries = files.entry(relative_path.to_owned()).or_insert_with(|| {
                let mut entries: HashMap<String, usize> = HashMap::new();
                for (entry, line) in yaml_entry_lines(contents) {
                    entries.entry(entry).or_insert(line);
                }
                entries
            });
            entries.get(name).copied().unwrap_or(1)
        };
        self.location(relative_path, (line, 1), None)
    }

    /// One diagnostic: `(code, severity)`, `(message, help)`, the resource and its location.
    fn diagnostic(
        &self,
        code: (&str, &str),
        text: (&str, Option<&str>),
        resource: (Option<&str>, &str),
        location: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (code, severity) = code;
        let (message, help) = text;
        let (resource_type, resource_name) = resource;
        let fields: Bound<'py, PyDict> = PyDict::new(self.py);
        fields.set_item("phase", "compile")?;
        fields.set_item("severity", severity)?;
        fields.set_item("code", code)?;
        fields.set_item("message", message)?;
        fields.set_item("resource_type", resource_type)?;
        fields.set_item("resource_name", resource_name)?;
        fields.set_item("location", location)?;
        fields.set_item("help", help)?;
        self.settings
            .compiler_diagnostic
            .bind(self.py)
            .call((), Some(&fields))
    }

    /// P010 for `kind` `name` when its description is missing or blank.
    fn described(
        &self,
        resource: (&str, &str, Option<&str>),
        description: Option<&str>,
        location: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
        help: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let (kind, name, resource_type) = resource;
        let Some(message) = missing_description(kind, name, description) else {
            return Ok(None);
        };
        Ok(Some(self.diagnostic(
            (MISSING_DESCRIPTION_CODE, "error"),
            (&message, Some(help)),
            (resource_type, name),
            location()?,
        )?))
    }

    /// P010 for a Python `kind` `name` without a description, or P027 when it wraps past the limit.
    fn described_python(
        &self,
        resource: (&str, &str, Option<&str>),
        description: Option<&str>,
        location: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
        help: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let (kind, name, resource_type) = resource;
        let (limit, line_width) = self.settings.description_limit;
        let Some(text) =
            description.filter(|_| missing_description(kind, name, description).is_none())
        else {
            return self.described(resource, description, location, help);
        };
        if description_line_count(text, line_width) <= limit {
            return Ok(None);
        }
        Ok(Some(self.diagnostic(
            (DESCRIPTION_LENGTH_CODE, "error"),
            (
                &format!("{kind} '{name}' description exceeds {limit} lines"),
                Some(&format!(
                    "Shorten the description to {limit} lines; move detail into documentation."
                )),
            ),
            (resource_type, name),
            location()?,
        )?))
    }
}

/// Orphaned factory warnings, missing descriptions and cursor errors of the owner's resources.
pub(crate) fn resource_diagnostics<'py>(
    project: &NativeCompiledProject,
    py: Python<'py>,
    settings: &DiagnosticSettings,
) -> PyResult<ResourceDiagnostics<'py>> {
    let diagnostics = Diagnostics {
        py,
        settings,
        yaml_lines: RefCell::new(HashMap::new()),
    };
    Ok((
        orphaned_factories(&diagnostics)?,
        missing_descriptions(project, &diagnostics)?,
        cursor_without_inputs(project, &diagnostics)?,
    ))
}

fn orphaned_factories<'py>(diagnostics: &Diagnostics<'_, 'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = diagnostics.py;
    if diagnostics.settings.audit_factories.is_empty() {
        return Ok(Vec::new());
    }
    let injected: Option<Arc<DeltaInjection>> = injection();
    let by_path: bool = injected.is_some() || capturing();
    let mut referenced: HashSet<String> = HashSet::new();
    let mut paths: HashSet<String> = HashSet::new();
    for model_file in &diagnostics.settings.model_files {
        let model_file: &Bound<'py, PyAny> = model_file.bind(py);
        let path: Option<String> = if by_path {
            Some(posix(
                &native_field(model_file, "relative_path")?.str()?.to_cow()?,
            ))
        } else {
            None
        };
        let header: Bound<'py, PyDict> =
            native_field(model_file, "header_values")?.downcast_into()?;
        let mut names: Vec<String> = Vec::new();
        if let Some(values) = header.get_item(AUDIT_FACTORIES_KEY)?
            && let Ok(values) = values.downcast::<PyList>()
        {
            for name in values.iter() {
                if name.is_instance_of::<PyString>() {
                    names.push(name.extract()?);
                }
            }
        }
        if let Some(path) = path {
            if !names.is_empty() {
                capture(|capture| {
                    capture.audit_references.insert(path.clone(), names.clone());
                });
            }
            paths.insert(path);
        }
        referenced.extend(names);
    }
    if let Some(injected) = &injected {
        for (path, names) in &injected.audit_references {
            if !paths.contains(path) {
                referenced.extend(names.iter().cloned());
            }
        }
    }
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for (name, relative_path, line) in &diagnostics.settings.audit_factories {
        if referenced.contains(name) {
            continue;
        }
        warnings.push(diagnostics.diagnostic(
            (ORPHAN_FACTORY_CODE, "warning"),
            (
                &format!(
                    "Audit factory '{name}' is not referenced by any model and will not produce \
                     audits"
                ),
                None,
            ),
            (Some("audit"), name),
            diagnostics.location(relative_path, (*line, 1), None)?,
        )?);
    }
    Ok(warnings)
}

fn posix(path: &str) -> String {
    path.replace('\\', "/")
}

fn missing_descriptions<'py>(
    project: &NativeCompiledProject,
    diagnostics: &Diagnostics<'_, 'py>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = diagnostics.py;
    let mut found: Vec<Option<Bound<'py, PyAny>>> = Vec::new();
    for record in project.inputs() {
        let model_file: &NativeModelFile = record.host.model_file.get();
        let name: String = record.name.clone();
        let path: String = model_file.relative_text(py)?;
        let contents: &str = model_file.contents_text();
        let entry: String = format!("description \"What one row of {name} represents\"");
        let help: String = header_help(&HeaderHelp {
            kind: "model",
            name: &name,
            statement: "MODEL",
            entry: &entry,
            path: &posix(&path),
            description_only: false,
        });
        let description: Option<&str> = record
            .schema
            .as_ref()
            .and_then(|schema| schema.description.as_deref());
        found.push(diagnostics.described(
            ("model", &name, Some("model")),
            description,
            || diagnostics.header_location(&path, contents),
            &help,
        )?);
    }
    for ScenarioDescriptionRow {
        name,
        path,
        contents,
        raw,
    } in &diagnostics.settings.scenarios
    {
        let raw: &Bound<'py, PyAny> = raw.bind(py);
        let description: Option<String> = if raw.is_instance_of::<PyString>() {
            Some(raw.extract()?)
        } else {
            None
        };
        let entry: String = format!("description \"The behaviour {name} proves\"");
        let help: String = header_help(&HeaderHelp {
            kind: "scenario",
            name,
            statement: "SCENARIO",
            entry: &entry,
            path: &posix(path),
            description_only: false,
        });
        found.push(diagnostics.described(
            ("scenario", name, Some("sql_scenario")),
            description.as_deref(),
            || diagnostics.header_location(path, contents),
            &help,
        )?);
    }
    for seed in project.seeds() {
        let SeedParts {
            schema_entry: entry,
            schema_file,
            ..
        } = seed.parts();
        let entry: &Bound<'py, PyAny> = entry.bind(py);
        let schema_file: &Bound<'py, PyAny> = schema_file.bind(py);
        let name: String = native_field(entry, "name")?.extract()?;
        let description: Option<String> = native_field(entry, "description")?.extract()?;
        let path: String = native_field(schema_file, "relative_path")?
            .str()?
            .extract()?;
        let contents_value = native_field(schema_file, "contents")?;
        let contents = contents_value.downcast::<PyString>()?.to_str()?;
        found.push(diagnostics.described(
            ("seed", &name, Some("seed")),
            description.as_deref(),
            || diagnostics.yaml_location(&path, contents, &name),
            &yaml_help("seed", &name, &posix(&path)),
        )?);
    }
    for source in project.sources() {
        found.push(source_description(diagnostics, source)?);
    }
    for function in project.functions() {
        found.push(function_description(diagnostics, function)?);
    }
    for HookDescriptionRow {
        name,
        path,
        contents,
        description,
    } in &diagnostics.settings.sql_hooks
    {
        let entry: String = format!("description \"What {name} does\"");
        let help: String = header_help(&HeaderHelp {
            kind: "hook",
            name,
            statement: "HOOK",
            entry: &entry,
            path: &posix(path),
            description_only: true,
        });
        found.push(diagnostics.described(
            ("hook", name, None),
            description.as_deref(),
            || diagnostics.header_location(path, contents),
            &help,
        )?);
    }
    for PythonResourceRow {
        kind,
        name,
        description,
        location,
        help,
    } in &diagnostics.settings.python_resources
    {
        let text = description
            .as_ref()
            .map(|description| docstring_text(description.bind(py)));
        found.push(diagnostics.described_python(
            (kind, name, None),
            text.as_deref(),
            || location.bind(py).call0(),
            help,
        )?);
    }
    Ok(found.into_iter().flatten().collect())
}

/// A docstring's text; a lone surrogate becomes U+FFFD, which `str.strip` keeps as it would.
fn docstring_text(description: &Bound<'_, PyString>) -> String {
    description.to_string_lossy().into_owned()
}

/// A source's P010, falling back to its loader's description and naming the loader in help.
fn source_description<'py>(
    diagnostics: &Diagnostics<'_, 'py>,
    source: &crate::bindings::_helpers::compiled_views::source_views::SourceRecord,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = diagnostics.py;
    let attached = source.attached();
    let name: &str = &attached.name;
    let source_file: &Bound<'py, PyAny> = attached.host.source_file.bind(py);
    let path: String = native_field(source_file, "relative_path")?
        .str()?
        .extract()?;
    let contents_value = native_field(source_file, "contents")?;
    let contents = contents_value.downcast::<PyString>()?.to_str()?;
    let loader_name: Option<String> =
        native_field(attached.host.entry.bind(py), "loader")?.extract()?;
    let loader: Option<(&String, &LoaderRow)> = loader_name
        .as_ref()
        .and_then(|loader| diagnostics.settings.loaders.get_key_value(loader));
    let mut description: Option<String> = attached.description.clone();
    if let Some((_, (loader_description, _, _))) = loader
        && missing_description("", "", description.as_deref()).is_some()
    {
        description = loader_description
            .clone()
            .filter(|text| !text.is_empty())
            .or(description);
    }
    let mut help: String = yaml_help("source", name, &posix(&path));
    if let Some((loader_name, (_, loader_path, false))) = loader {
        help = join_helps(&[
            help,
            format!(
                "or describe its loader '{loader_name}' in {} with a docstring or \
                 @loader(description=\"What one row of {name} represents\")",
                posix(loader_path)
            ),
        ]);
    }
    diagnostics.described(
        ("source", name, Some("source")),
        description.as_deref(),
        || diagnostics.yaml_location(&path, contents, name),
        &help,
    )
}

/// A SQL or Python function's P010, located at its header or `@udf` decorator.
fn function_description<'py>(
    diagnostics: &Diagnostics<'_, 'py>,
    function: &crate::bindings::_helpers::compiled_views::function_views::FunctionRecord,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = diagnostics.py;
    let attached = &function.attached;
    let name: &str = &attached.name;
    let function_file: &Bound<'py, PyAny> = attached.host.function_file.bind(py);
    let path: String = native_field(function_file, "relative_path")?
        .str()?
        .extract()?;
    let contents: String = native_field(function_file, "contents")?.extract()?;
    let resource_type: &str = if attached.return_columns.is_empty() {
        "udf"
    } else {
        "table_fn"
    };
    if attached.python.is_some() {
        let help: String = snippet_help(
            &format!("to describe function '{name}'"),
            &format!(
                "give its function in {} a docstring or add this to @udf",
                posix(&path)
            ),
            &[format!("@udf(description=\"What {name} returns\", ...)")],
        );
        return diagnostics.described_python(
            ("function", name, Some(resource_type)),
            attached.description.as_deref(),
            || {
                let version = py.version_info();
                let position: (usize, usize) = decorator_location(
                    &contents,
                    PYTHON_UDF_DECORATOR,
                    (version.major, version.minor),
                );
                diagnostics.location(&path, position, None)
            },
            &help,
        );
    }
    let entry: String = format!("description \"What {name} returns\"");
    let help: String = header_help(&HeaderHelp {
        kind: "function",
        name,
        statement: "FUNCTION",
        entry: &entry,
        path: &posix(&path),
        description_only: false,
    });
    diagnostics.described(
        ("function", name, Some(resource_type)),
        attached.description.as_deref(),
        || diagnostics.header_location(&path, &contents),
        &help,
    )
}

fn config_text(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) if value.is_instance_of::<PyString>() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

fn cursor_without_inputs<'py>(
    project: &NativeCompiledProject,
    diagnostics: &Diagnostics<'_, 'py>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = diagnostics.py;
    let mut errors: Vec<Bound<'py, PyAny>> = Vec::new();
    for record in project.inputs() {
        let values: Bound<'py, PyDict> =
            native_field(record.host.config.bind(py), "values")?.downcast_into()?;
        let kinds: Vec<&str> = record
            .references
            .iter()
            .map(|ReferenceRow { kind, .. }| kind.as_str())
            .collect();
        let materialized: Option<String> = config_text(&values, "materialized")?;
        let cursor: Option<String> = config_text(&values, CURSOR_KEY)?;
        let strategy: Option<String> = config_text(&values, "microbatch_strategy")?;
        let model_file: &NativeModelFile = record.host.model_file.get();
        let name: String = record.name.clone();
        let Some((message, help)) = cursor_lacks_inputs(
            &name,
            &CursorModel {
                materialized: materialized.as_deref(),
                cursor: cursor.as_deref(),
                microbatch_strategy: strategy.as_deref(),
                has_cursor_inputs: values
                    .get_item(CURSOR_INPUTS_KEY)?
                    .is_some_and(|value| !value.is_none()),
                reference_kinds: &kinds,
            },
        ) else {
            continue;
        };
        let path: String = model_file.relative_text(py)?;
        let contents: &str = model_file.contents_text();
        let span: Option<(usize, usize)> =
            model_header_entry_span(contents, CURSOR_KEY).map_err(PyValueError::new_err)?;
        let start: TextPosition = text_position(contents, span.map_or(0, |span| span.0));
        let location: Bound<'py, PyAny> = diagnostics.location(
            &path,
            (start.line, start.column),
            span.map(|(begin, end)| (start.line, start.column + end - begin)),
        )?;
        errors.push(diagnostics.diagnostic(
            (CURSOR_WITHOUT_INPUTS_CODE, "error"),
            (&message, Some(help)),
            (Some("model"), &name),
            location,
        )?);
    }
    Ok(errors)
}
