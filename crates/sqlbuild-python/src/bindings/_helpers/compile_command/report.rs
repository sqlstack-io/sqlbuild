//! The `sqb compile` report: human text, the JSON report, and the JSON report of a stopped compile.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods,
};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_cache::project_reuse::constants::FAILED_COMPILED_DIRECTORY_NAME;
use sqlbuild_core::cli_text::main::format_compile_text::format_compile_text as compile_text;
use sqlbuild_core::cli_text::main::semantic_coverage_notice::semantic_coverage_notice as notice_text;
use sqlbuild_core::cli_text::main::strict_report::{reason_text, strict_report};
use sqlbuild_core::cli_text::models::{
    CliStyle, CompileTextReport, DiagnosticText, LineageNote, LocationText, ModelLine, RelatedText,
};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::compile_command::constants::{
    STRICT_CODE, STRICT_HELP, STRICT_ON, STRICT_REPORT,
};
use crate::bindings::_helpers::compile_command::surrogates::SurrogateText;
use crate::bindings::_helpers::compile_outputs::json_report::{
    emit_json_report, emit_orjson_report,
};
use crate::bindings::_helpers::discovery::constants::SQL_HOOK_OUTPUT_FIELDS;
use crate::bindings::_helpers::discovery::hook_entries::hook_entries;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::{new_record, new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::semantic_shapes::coverage::semantic_coverage;

const METADATA_MODULE: &str = "importlib.metadata";
const TARGET_DIRECTORY: &str = "target";
const COMPILED_DIRECTORY: &str = "compiled";
const MANIFEST_ARTIFACT: &str = "target/manifest.json";
const PACKAGE_NAME: &str = "sqlbuild";
const UNKNOWN_VERSION: &str = "unknown";
const MODEL_TYPE: &str = "model";
const SEED_TYPE: &str = "seed";
const FUNCTION_TYPES: [&str; 2] = ["udf", "table_fn"];
const SQL_TEST_TYPE: &str = "sql_test";
const ERROR_SEVERITY: &str = "error";
const WARNING_SEVERITY: &str = "warning";
const RICH_LINEAGE: &str = "rich";
const NO_LINEAGE: &str = "none";
const DECIMAL_KIND: &str = "decimal";
const REPORT_OPTIONS: (&[&str], &[&str]) = (
    &["graph", "written", "manifest", "lineage", "diagnostics"],
    &[
        "lineage_mode",
        "selected_keys",
        "sql_validation_enabled",
        "use_color",
        "timings_ms",
        "strict",
    ],
);
const HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];

/// What both compile reports are built from.
struct ReportInputs<'py> {
    graph: Bound<'py, PyAny>,
    project: Bound<'py, PyAny>,
    written: Bound<'py, PyAny>,
    /// A manifest was requested and written, which a compile whose Rules failed never does.
    manifest: bool,
    lineage: Bound<'py, PyAny>,
    diagnostics: Vec<Bound<'py, PyAny>>,
    lineage_mode: String,
    selected_keys: Bound<'py, PyAny>,
    sql_validation_enabled: bool,
    /// `--strict` was passed, which enforces strict whatever `settings.strict` says.
    strict_flag: bool,
}

impl<'py> ReportInputs<'py> {
    fn read(arguments: &KeywordArguments<'py>) -> PyResult<Self> {
        let graph: Bound<'py, PyAny> = arguments.value("graph")?;
        let lineage_mode: Bound<'py, PyAny> = arguments.value("lineage_mode")?;
        let written: Bound<'py, PyAny> = arguments.value("written")?;
        let failed_compile: bool = native_field(&written, "failed_compile")?.is_truthy()?;
        Ok(Self {
            project: native_field(&graph, "project")?,
            graph,
            manifest: arguments.flag("manifest")? && !failed_compile,
            written,
            lineage: arguments.value("lineage")?,
            diagnostics: arguments
                .value("diagnostics")?
                .try_iter()?
                .collect::<PyResult<_>>()?,
            lineage_mode: if lineage_mode.is_none() {
                "fast".to_owned()
            } else {
                lineage_mode.str()?.to_string()
            },
            selected_keys: arguments.value("selected_keys")?,
            sql_validation_enabled: !arguments.passed("sql_validation_enabled")?
                || arguments.flag("sql_validation_enabled")?,
            strict_flag: arguments.flag("strict")?,
        })
    }

    fn py(&self) -> Python<'py> {
        self.graph.py()
    }

    /// The selected keys' names of `resource_type`, or `None` when nothing was deselected.
    fn selected_names(&self, resource_type: &str) -> PyResult<Option<Bound<'py, PySet>>> {
        if self.selected_keys.is_none() {
            return Ok(None);
        }
        let names: Bound<'py, PySet> = PySet::empty(self.py())?;
        for key in self.selected_keys.try_iter()? {
            let key: Bound<'py, PyAny> = key?;
            if native_field(&key, "resource_type")?.eq(resource_type)? {
                names.add(native_field(&key, "name")?)?;
            }
        }
        Ok(Some(names))
    }

    /// The project's models, kept to the selected ones.
    fn selected_models(&self) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let models: Vec<Bound<'py, PyAny>> = native_field(&self.project, "models")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let Some(names) = self.selected_names(MODEL_TYPE)? else {
            return Ok(models);
        };
        let mut selected: Vec<Bound<'py, PyAny>> = Vec::new();
        for model in models {
            if names.contains(native_field(&model, "name")?)? {
                selected.push(model);
            }
        }
        Ok(selected)
    }

    /// How many graph or selected resources have one of `types`.
    fn selected_count(&self, types: &[&str]) -> PyResult<usize> {
        let mut count: usize = 0;
        if self.selected_keys.is_none() {
            for key in native_field(&self.graph, "native")?
                .call_method0("keys")?
                .try_iter()?
            {
                let (kind, _): (String, Bound<'py, PyAny>) = key?.extract()?;
                count += usize::from(types.contains(&kind.as_str()));
            }
            return Ok(count);
        }
        for key in self.selected_keys.try_iter()? {
            let kind: String = native_field(&key?, "resource_type")?.str()?.to_string();
            count += usize::from(types.contains(&kind.as_str()));
        }
        Ok(count)
    }

    fn severity_count(&self, severity: &str) -> PyResult<usize> {
        let mut count: usize = 0;
        for diagnostic in &self.diagnostics {
            count += usize::from(native_field(diagnostic, "severity")?.eq(severity)?);
        }
        Ok(count)
    }

    /// `selected_semantic_coverage`: each selected model's partial-check reasons.
    fn semantic_coverage(&self) -> PyResult<Bound<'py, PyDict>> {
        let py: Python<'py> = self.py();
        if !self.sql_validation_enabled {
            return Ok(PyDict::new(py));
        }
        let reasons: Bound<'py, PyDict> = semantic_coverage(&self.project)?;
        if crate::bindings::_helpers::compile_context::delta::injection().is_some() {
            return Ok(reasons);
        }
        let Some(names) = self.selected_names(MODEL_TYPE)? else {
            return Ok(reasons);
        };
        let selected: Bound<'py, PyDict> = PyDict::new(py);
        for (name, values) in reasons.iter() {
            if names.contains(&name)? {
                selected.set_item(name, values)?;
            }
        }
        Ok(selected)
    }

    fn semantic_notice(&self, reader: &SurrogateText) -> PyResult<Option<String>> {
        self.coverage_text(&self.semantic_coverage()?, reader)
    }

    /// The notice of these coverage reasons, or the strict report when `strict = "report"`.
    fn coverage_text(
        &self,
        coverage: &Bound<'py, PyDict>,
        reader: &SurrogateText,
    ) -> PyResult<Option<String>> {
        let reasons: Vec<(String, Vec<String>)> = coverage_reasons(coverage, reader)?;
        let mode: String = self.strict_mode()?;
        Ok(match mode.as_str() {
            STRICT_REPORT => strict_report(&reasons, false),
            STRICT_ON => strict_report(&reasons, true),
            _ => notice_text(&reasons),
        })
    }

    /// The strict mode in force: `on` with `--strict`, else `settings.strict` (`off`, `report`
    /// or `on`).
    fn strict_mode(&self) -> PyResult<String> {
        strict_mode(&self.project, self.strict_flag)
    }

    /// `written.target_dir / "compiled"`, or `"compiled-failed"` when Rules failed, relative to its `target` directory, POSIX style.
    fn compiled_dir(&self) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.py();
        let directory: &str = if native_field(&self.written, "failed_compile")?.is_truthy()? {
            FAILED_COMPILED_DIRECTORY_NAME
        } else {
            COMPILED_DIRECTORY
        };
        let path: Bound<'py, PyAny> = native_field(&self.written, "target_dir")?.div(directory)?;
        let parts: Bound<'py, PyTuple> = native_field(&path, "parts")?.downcast_into()?;
        for (index, part) in parts.iter().enumerate() {
            if part.eq(TARGET_DIRECTORY)? {
                let kept: Bound<'py, PyTuple> = parts.get_slice(index, parts.len());
                return imported(py, "pathlib", "Path")?
                    .call1(kept)?
                    .call_method0("as_posix");
            }
        }
        path.call_method0("as_posix")
    }

    /// Each failing model's SQL file under `compiled_dir`, relative like it, when Rules failed.
    fn failed_model_sql(&self) -> PyResult<Vec<String>> {
        let directory: String = self.compiled_dir()?.extract()?;
        Ok(native_field(&self.written, "failed_model_sql")?
            .extract::<Vec<String>>()?
            .into_iter()
            .map(|relative| format!("{directory}/{relative}"))
            .collect())
    }
}

/// `int(value)` of a location field, or `None`.
fn optional_number(value: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

/// Each authored file's text by path: models with SQL, then sources, then SQL tests.
fn source_texts<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let texts: Bound<'py, PyDict> = PyDict::new(project.py());
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let authored: Bound<'py, PyAny> = native_field(&model, "authored_sql")?;
        if authored.is_truthy()? {
            texts.set_item(native_field(&model, "relative_path")?, authored)?;
        }
    }
    for (collection, file) in [("sources", "source_file"), ("sql_tests", "test_file")] {
        for resource in native_field(project, collection)?.try_iter()? {
            let file: Bound<'py, PyAny> = native_field(&resource?, file)?;
            texts.set_item(
                native_field(&file, "relative_path")?,
                native_field(&file, "contents")?,
            )?;
        }
    }
    Ok(texts)
}

fn location_text(
    reader: &SurrogateText,
    location: &Bound<'_, PyAny>,
    texts: &Bound<'_, PyDict>,
) -> PyResult<LocationText> {
    let path: Bound<'_, PyAny> = native_field(location, "path")?;
    Ok(LocationText {
        path: reader.text(&path)?,
        line: native_field(location, "line")?.extract()?,
        column: native_field(location, "column")?.extract()?,
        end_line: optional_number(&native_field(location, "end_line")?)?,
        end_column: optional_number(&native_field(location, "end_column")?)?,
        source: texts
            .get_item(path)?
            .map(|text| reader.text(&text))
            .transpose()?,
    })
}

fn texts_of(reader: &SurrogateText, value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value.try_iter()?.map(|item| reader.text(&item?)).collect()
}

fn diagnostic_text(
    reader: &SurrogateText,
    diagnostic: &Bound<'_, PyAny>,
    texts: &Bound<'_, PyDict>,
    occurrences: usize,
) -> PyResult<DiagnosticText> {
    let location: Bound<'_, PyAny> = native_field(diagnostic, "location")?;
    let mut related: Vec<RelatedText> = Vec::new();
    for item in native_field(diagnostic, "related_locations")?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        related.push(RelatedText {
            label: reader.text(&native_field(&item, "label")?)?,
            location: location_text(reader, &native_field(&item, "location")?, texts)?,
            message: reader.optional(&native_field(&item, "message")?)?,
        });
    }
    Ok(DiagnosticText {
        severity: reader.text(&native_field(diagnostic, "severity")?)?,
        code: reader.text(&native_field(diagnostic, "code")?)?,
        message: reader.text(&native_field(diagnostic, "message")?)?,
        resource_type: reader.optional(&native_field(diagnostic, "resource_type")?)?,
        resource_name: reader.optional(&native_field(diagnostic, "resource_name")?)?,
        location: if location.is_none() {
            None
        } else {
            Some(location_text(reader, &location, texts)?)
        },
        path: reader.optional(&native_field(diagnostic, "path")?)?,
        related,
        notes: texts_of(reader, &native_field(diagnostic, "notes")?)?,
        affected_rules: texts_of(reader, &native_field(diagnostic, "affected_rules")?)?,
        help: reader.optional(&native_field(diagnostic, "help")?)?,
        occurrences,
    })
}

/// The diagnostics the human report shows, each repeated warning folded into its first.
fn shown_diagnostics(
    inputs: &ReportInputs<'_>,
    reader: &SurrogateText,
) -> PyResult<Vec<DiagnosticText>> {
    let py: Python<'_> = inputs.py();
    if inputs.diagnostics.is_empty() {
        return Ok(Vec::new());
    }
    let texts: Bound<'_, PyDict> = source_texts(&inputs.project)?;
    crate::bindings::_helpers::compile_delta::report_hooks::complete_source_texts(
        &inputs.project,
        &inputs.diagnostics,
        &texts,
    )?;
    let warning_counts: Bound<'_, PyDict> = PyDict::new(py);
    for diagnostic in &inputs.diagnostics {
        if native_field(diagnostic, "severity")?.eq(WARNING_SEVERITY)? {
            let count: usize = warning_counts
                .get_item(diagnostic)?
                .map_or(Ok(0), |count| count.extract())?;
            warning_counts.set_item(diagnostic, count + 1)?;
        }
    }
    let shown_warnings: Bound<'_, PyDict> = PyDict::new(py);
    let mut shown: Vec<DiagnosticText> = Vec::new();
    for diagnostic in &inputs.diagnostics {
        if shown_warnings.contains(diagnostic)? {
            continue;
        }
        if native_field(diagnostic, "severity")?.eq(WARNING_SEVERITY)? {
            shown_warnings.set_item(diagnostic, true)?;
        }
        let occurrences: usize = warning_counts
            .get_item(diagnostic)?
            .map_or(Ok(1), |count| count.extract())?;
        shown.push(diagnostic_text(reader, diagnostic, &texts, occurrences)?);
    }
    Ok(shown)
}

/// Each model an error diagnostic names, other than SQL tests.
fn error_models<'py>(inputs: &ReportInputs<'py>) -> PyResult<Bound<'py, PySet>> {
    let names: Bound<'py, PySet> = PySet::empty(inputs.py())?;
    for diagnostic in &inputs.diagnostics {
        let name: Bound<'_, PyAny> = native_field(diagnostic, "resource_name")?;
        if native_field(diagnostic, "is_error")?.is_truthy()?
            && !name.is_none()
            && !native_field(diagnostic, "resource_type")?.eq(SQL_TEST_TYPE)?
        {
            names.add(name)?;
        }
    }
    Ok(names)
}

/// The human-readable compile report.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_compile_text<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyString>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "format_compile_text", options, REPORT_OPTIONS)?;
    let inputs: ReportInputs<'py> = ReportInputs::read(&arguments)?;
    let reader: SurrogateText = SurrogateText::default();
    let failed: Bound<'py, PySet> = error_models(&inputs)?;
    let mut models: Vec<ModelLine> = Vec::new();
    for model in inputs.selected_models()? {
        let name: Bound<'py, PyAny> = native_field(&model, "name")?;
        models.push(ModelLine {
            failed: failed.contains(&name)?,
            name: reader.text(&name)?,
            column_count: native_field(&model, "output_column_count")?.extract()?,
        });
    }
    let models: Vec<ModelLine> =
        crate::bindings::_helpers::compile_delta::report_hooks::text_model_lines(models);
    let lineage: LineageNote = if !inputs.lineage.is_none() && inputs.lineage_mode == RICH_LINEAGE {
        LineageNote::Rich
    } else if inputs.lineage.is_none()
        && native_field(&inputs.project, "settings")?
            .getattr("sql_analysis")?
            .is_truthy()?
        && inputs.lineage_mode != NO_LINEAGE
    {
        LineageNote::Unavailable
    } else {
        LineageNote::Silent
    };
    let report: CompileTextReport = CompileTextReport {
        style: CliStyle {
            use_color: arguments.flag("use_color")?,
        },
        models,
        semantic_notice: inputs.semantic_notice(&reader)?,
        diagnostics: shown_diagnostics(&inputs, &reader)?,
        seed_count: inputs.selected_count(&[SEED_TYPE])?,
        function_count: inputs.selected_count(&FUNCTION_TYPES)?,
        error_count: inputs.severity_count(ERROR_SEVERITY)?,
        warning_count: inputs.severity_count(WARNING_SEVERITY)?,
        compiled_dir: reader.text(&inputs.compiled_dir()?)?,
        failed_model_sql: inputs.failed_model_sql()?,
        manifest: inputs.manifest,
        lineage,
    };
    reader.python(py, &compile_text(&report))
}

/// The strict mode in force for `project`: `on` with `--strict`, else `settings.strict`.
fn strict_mode(project: &Bound<'_, PyAny>, flag: bool) -> PyResult<String> {
    if flag {
        return Ok(STRICT_ON.to_owned());
    }
    native_field(project, "settings")?
        .getattr("strict")?
        .extract()
}

/// One error per selected model whose checks were partial, when strict is enforced.
pub(crate) fn strict_diagnostics<'py>(
    graph: &Bound<'py, PyAny>,
    (selected_keys, sql_validation_enabled): (&Bound<'py, PyAny>, bool),
    flag: bool,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = graph.py();
    let project: Bound<'py, PyAny> = native_field(graph, "project")?;
    if strict_mode(&project, flag)? != STRICT_ON {
        return Ok(Vec::new());
    }
    let inputs: ReportInputs<'py> = ReportInputs {
        graph: graph.clone(),
        project: project.clone(),
        written: py.None().into_bound(py),
        manifest: false,
        lineage: py.None().into_bound(py),
        diagnostics: Vec::new(),
        lineage_mode: String::new(),
        selected_keys: selected_keys.clone(),
        sql_validation_enabled,
        strict_flag: flag,
    };
    let reader: SurrogateText = SurrogateText::default();
    let mut paths: HashMap<String, Bound<'py, PyAny>> = HashMap::new();
    for model in native_field(&project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        paths.insert(
            native_field(&model, "name")?.extract()?,
            native_field(&model, "relative_path")?,
        );
    }
    let mut reasons: Vec<(String, Vec<String>)> =
        coverage_reasons(&inputs.semantic_coverage()?, &reader)?;
    reasons.sort();
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::with_capacity(reasons.len());
    for (model, values) in reasons {
        let details: Vec<String> = values.iter().map(|reason| reason_text(reason)).collect();
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("phase", "compile")?;
        fields.set_item("severity", ERROR_SEVERITY)?;
        fields.set_item("code", STRICT_CODE)?;
        fields.set_item(
            "message",
            format!(
                "model '{model}' has unproven checks under strict: {}",
                details.join("; ")
            ),
        )?;
        fields.set_item("resource_type", MODEL_TYPE)?;
        fields.set_item(
            "path",
            paths
                .get(&model)
                .map_or_else(|| py.None().into_bound(py), Clone::clone),
        )?;
        fields.set_item("resource_name", model)?;
        fields.set_item("help", STRICT_HELP)?;
        diagnostics.push(new_record(
            py,
            RecordId::CompilerDiagnostic,
            (),
            Some(&fields),
        )?);
    }
    Ok(diagnostics)
}

/// Each model's coverage reasons as text.
fn coverage_reasons(
    coverage: &Bound<'_, PyDict>,
    reader: &SurrogateText,
) -> PyResult<Vec<(String, Vec<String>)>> {
    let mut reasons: Vec<(String, Vec<String>)> = Vec::new();
    for (name, values) in coverage.iter() {
        let values: Vec<String> = values
            .try_iter()?
            .map(|value| reader.text(&value?))
            .collect::<PyResult<_>>()?;
        reasons.push((reader.text(&name)?, values));
    }
    Ok(reasons)
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

fn any<'py, T: IntoPyObjectExt<'py>>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

/// The installed SQLBuild version, or `unknown`.
fn sqlbuild_version(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    match imported(py, METADATA_MODULE, "version")?.call1((PACKAGE_NAME,)) {
        Ok(version) => Ok(version),
        Err(error)
            if error.is_instance(py, &imported(py, METADATA_MODULE, "PackageNotFoundError")?) =>
        {
            any(py, UNKNOWN_VERSION)
        }
        Err(error) => Err(error),
    }
}

fn location_json<'py>(location: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = location.py();
    let payload: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("path", native_field(location, "path")?.str()?.into_any()),
            ("line", native_field(location, "line")?),
            ("column", native_field(location, "column")?),
        ],
    )?;
    for key in ["end_line", "end_column"] {
        let value: Bound<'py, PyAny> = native_field(location, key)?;
        if !value.is_none() {
            payload.set_item(key, value)?;
        }
    }
    Ok(payload)
}

/// One diagnostic as the report's JSON item.
pub(crate) fn diagnostic_item<'py>(diagnostic: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    diagnostic_json(diagnostic)
}

/// `_diagnostic_to_json`: one diagnostic's JSON fields, omitting the unset ones.
fn diagnostic_json<'py>(diagnostic: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = diagnostic.py();
    let payload: Bound<'py, PyDict> = dict(
        py,
        vec![
            (
                "phase",
                native_field(diagnostic, "phase")?.str()?.into_any(),
            ),
            (
                "severity",
                native_field(diagnostic, "severity")?.str()?.into_any(),
            ),
            ("code", native_field(diagnostic, "code")?),
            ("message", native_field(diagnostic, "message")?),
        ],
    )?;
    let resource_type: Bound<'py, PyAny> = native_field(diagnostic, "resource_type")?;
    if !resource_type.is_none() {
        payload.set_item("resource_type", resource_type.str()?)?;
    }
    for key in ["resource_name", "column_name"] {
        let value: Bound<'py, PyAny> = native_field(diagnostic, key)?;
        if !value.is_none() {
            payload.set_item(key, value)?;
        }
    }
    let path: Bound<'py, PyAny> = native_field(diagnostic, "path")?;
    if !path.is_none() {
        payload.set_item("path", path.str()?)?;
    }
    for key in ["line", "column"] {
        let value: Bound<'py, PyAny> = native_field(diagnostic, key)?;
        if !value.is_none() {
            payload.set_item(key, value)?;
        }
    }
    let location: Bound<'py, PyAny> = native_field(diagnostic, "location")?;
    if !location.is_none() {
        payload.set_item("location", location_json(&location)?)?;
    }
    let related: Bound<'py, PyAny> = native_field(diagnostic, "related_locations")?;
    if related.is_truthy()? {
        let mut items: Vec<Bound<'py, PyDict>> = Vec::new();
        for item in related.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let entry: Bound<'py, PyDict> = dict(
                py,
                vec![
                    ("label", native_field(&item, "label")?),
                    (
                        "location",
                        location_json(&native_field(&item, "location")?)?.into_any(),
                    ),
                ],
            )?;
            let message: Bound<'py, PyAny> = native_field(&item, "message")?;
            if !message.is_none() {
                entry.set_item("message", message)?;
            }
            items.push(entry);
        }
        payload.set_item("related_locations", PyList::new(py, items)?)?;
    }
    let help: Bound<'py, PyAny> = native_field(diagnostic, "help")?;
    if !help.is_none() {
        payload.set_item("help", help)?;
    }
    for key in ["notes", "affected_rules"] {
        let value: Bound<'py, PyAny> = native_field(diagnostic, key)?;
        if value.is_truthy()? {
            payload.set_item(
                key,
                PyList::new(py, value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
            )?;
        }
    }
    Ok(payload)
}

/// `item[key] = value.qualified_name` when the destination names a relation.
fn qualified_name(item: &Bound<'_, PyDict>, resource: &Bound<'_, PyAny>) -> PyResult<()> {
    let name: Bound<'_, PyAny> =
        native_field(resource, "destination")?.getattr("qualified_name")?;
    if !name.is_none() {
        item.set_item("qualified_name", name)?;
    }
    Ok(())
}

fn lineage_summary<'py>(
    lineage: &Bound<'py, PyAny>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model.py();
    let name: Bound<'py, PyAny> = native_field(model, "name")?;
    if lineage.is_none() || !lineage.call_method1("has_model", (&name,))?.is_truthy()? {
        return dict(py, vec![("available", any(py, false)?)]);
    }
    dict(
        py,
        vec![
            ("available", any(py, true)?),
            ("column_count", native_field(model, "output_column_count")?),
            (
                "edge_count",
                lineage.call_method1("edge_count_targeting", (&name,))?,
            ),
            (
                "has_star",
                lineage.call_method1("model_has_star", (&name,))?,
            ),
        ],
    )
}

fn model_resource<'py>(
    inputs: &ReportInputs<'py>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model.py();
    let key: Bound<'py, PyAny> = native_field(model, "key")?;
    let mut depends_on: Vec<Bound<'py, PyDict>> = Vec::new();
    for upstream in native_field(&inputs.graph, "native")?
        .call_method1(
            "upstream",
            ((
                native_field(&key, "resource_type")?.str()?,
                native_field(&key, "name")?,
            ),),
        )?
        .try_iter()?
    {
        let (kind, name): (Bound<'py, PyAny>, Bound<'py, PyAny>) = upstream?.extract()?;
        depends_on.push(dict(py, vec![("resource_type", kind), ("name", name)])?);
    }
    let values: Bound<'py, PyAny> = native_field(model, "config")?.getattr("values")?;
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(model, "name")?),
            (
                "relative_path",
                native_field(model, "relative_path")?.str()?.into_any(),
            ),
            (
                "materialized",
                mapping_get_or(&values, "materialized", "view")?
                    .str()?
                    .into_any(),
            ),
            ("column_count", native_field(model, "output_column_count")?),
            ("depends_on", PyList::new(py, depends_on)?.into_any()),
            (
                "lineage",
                lineage_summary(&inputs.lineage, model)?.into_any(),
            ),
            ("query_sql", native_field(model, "query_sql")?),
        ],
    )?;
    qualified_name(&item, model)?;
    for hook_key in HOOK_KEYS {
        let hooks: Bound<'py, PyAny> = mapping_get(&values, hook_key)?;
        if !hooks.is_none() {
            item.set_item(
                hook_key,
                hook_entries(&hooks, &SQL_HOOK_OUTPUT_FIELDS.map(str::to_owned), None)?,
            )?;
        }
    }
    Ok(item)
}

fn hook_definitions<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = project.py();
    let mut definitions: Vec<Bound<'py, PyDict>> = Vec::new();
    for hook in native_field(project, "sql_hook_files")?.try_iter()? {
        let hook: Bound<'py, PyAny> = hook?;
        definitions.push(dict(
            py,
            vec![
                ("type", any(py, "sql")?),
                ("name", native_field(&hook, "name")?),
                (
                    "relative_path",
                    native_field(&hook, "relative_path")?.call_method0("as_posix")?,
                ),
                ("description", native_field(&hook, "description")?),
                ("sql", native_field(&hook, "sql_body")?),
            ],
        )?);
    }
    let functions: Bound<'py, PyAny> = native_field(project, "hook_functions")?;
    let identities: Bound<'py, PyAny> =
        host::entry(py, UserCodeHost::BuildHookIdentities)?.call1((&functions,))?;
    for hook in functions.try_iter()? {
        let hook: Bound<'py, PyAny> = hook?;
        let identity: Bound<'py, PyAny> = identities.get_item(native_field(&hook, "name")?)?;
        definitions.push(dict(
            py,
            vec![
                ("type", any(py, "python")?),
                ("name", native_field(&hook, "name")?),
                (
                    "relative_path",
                    native_field(&hook, "relative_path")?.call_method0("as_posix")?,
                ),
                ("description", native_field(&hook, "description")?),
                (
                    "definition_hash",
                    native_field(&identity, "definition_hash")?,
                ),
                ("version_hash", native_field(&identity, "version_hash")?),
            ],
        )?);
    }
    PyList::new(py, definitions)
}

fn function_resource<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = function.py();
    let return_columns: Bound<'py, PyAny> = native_field(function, "return_columns")?;
    let mut columns: Vec<Bound<'py, PyDict>> = Vec::new();
    for column in return_columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        columns.push(dict(
            py,
            vec![
                ("name", native_field(&column, "name")?),
                ("type", native_field(&column, "type")?),
            ],
        )?);
    }
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(function, "name")?),
            (
                "relative_path",
                native_field(function, "relative_path")?.str()?.into_any(),
            ),
            (
                "language",
                native_field(function, "language")?.getattr("value")?,
            ),
            ("description", native_field(function, "description")?),
            (
                "return_kind",
                any(
                    py,
                    if return_columns.is_truthy()? {
                        "table"
                    } else {
                        "scalar"
                    },
                )?,
            ),
            ("returns", native_field(function, "returns")?),
            ("return_columns", PyList::new(py, columns)?.into_any()),
        ],
    )?;
    qualified_name(&item, function)?;
    Ok(item)
}

fn audit_resource<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = audit.py();
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(audit, "name")?),
            (
                "relative_path",
                native_field(audit, "audit_file")?
                    .getattr("relative_path")?
                    .str()?
                    .into_any(),
            ),
        ],
    )?;
    let kind: Bound<'py, PyAny> = native_field(audit, "attached_target_kind")?;
    let target: Bound<'py, PyAny> = native_field(audit, "attached_target_name")?;
    if !kind.is_none() && !target.is_none() {
        let attached: Bound<'py, PyDict> = dict(
            py,
            vec![
                ("resource_type", native_field(&kind, "value")?),
                ("name", target),
            ],
        )?;
        let column: Bound<'py, PyAny> = native_field(audit, "attached_column_name")?;
        if !column.is_none() {
            attached.set_item("column_name", column)?;
        }
        item.set_item("attached_to", attached)?;
    }
    Ok(item)
}

fn test_resource<'py>(test: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = test.py();
    let payload: Bound<'py, PyAny> = native_field(test, "payload")?;
    let expected_models: Bound<'py, PyList> =
        if payload.is_instance(&record_class(py, RecordId::CompiledModelSqlTestPayload)?)? {
            PyList::new(
                py,
                native_field(&payload, "expected_model_names")?
                    .try_iter()?
                    .collect::<PyResult<Vec<_>>>()?,
            )?
        } else {
            PyList::empty(py)
        };
    let test_file_path: Bound<'py, PyAny> =
        native_field(test, "test_file")?.getattr("relative_path")?;
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(test, "name")?),
            ("relative_path", test_file_path.str()?.into_any()),
            ("expected_models", expected_models.into_any()),
        ],
    )?;
    let case_name: Bound<'py, PyAny> = native_field(test, "case_name")?;
    if case_name.is_none() {
        return Ok(item);
    }
    let schema: Vec<Bound<'py, PyAny>> = native_field(test, "parameter_schema")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let types: Bound<'py, PyDict> = PyDict::new(py);
    let mut parameter_schema: Vec<Bound<'py, PyDict>> = Vec::new();
    for parameter in &schema {
        let kind: Bound<'py, PyAny> = native_field(parameter, "value_type")?.getattr("value")?;
        types.set_item(native_field(parameter, "name")?, &kind)?;
    }
    for parameter in &schema {
        parameter_schema.push(dict(
            py,
            vec![
                ("name", native_field(parameter, "name")?),
                (
                    "type",
                    native_field(parameter, "value_type")?.getattr("value")?,
                ),
                ("nullable", native_field(parameter, "nullable")?),
            ],
        )?);
    }
    let mut parameters: Vec<Bound<'py, PyDict>> = Vec::new();
    for pair in native_field(test, "parameter_values")?.try_iter()? {
        let (name, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        let inner: Bound<'py, PyAny> = native_field(&value, "value")?;
        let shown: Bound<'py, PyAny> = if native_field(&value, "kind")?.eq(DECIMAL_KIND)? {
            inner.str()?.into_any()
        } else {
            inner
        };
        let kind: Bound<'py, PyAny> = types.as_any().get_item(&name)?;
        parameters.push(dict(
            py,
            vec![("name", name), ("type", kind), ("value", shown)],
        )?);
    }
    let source_path: Bound<'py, PyAny> = native_field(test, "source_path")?;
    let source_path: Bound<'py, PyAny> = if source_path.is_truthy()? {
        source_path
    } else {
        test_file_path
    };
    for (key, value) in [
        ("source_path", source_path.call_method0("as_posix")?),
        ("block_index", native_field(test, "block_index")?),
        ("parent_name", native_field(test, "parent_name")?),
        ("case_name", case_name),
        ("case_index", native_field(test, "case_index")?),
        ("case_fingerprint", native_field(test, "case_fingerprint")?),
        (
            "parameter_schema",
            PyList::new(py, parameter_schema)?.into_any(),
        ),
        ("parameters", PyList::new(py, parameters)?.into_any()),
    ] {
        item.set_item(key, value)?;
    }
    Ok(item)
}

/// Each item of `project.<collection>` through `resource`.
fn resources<'py>(
    project: &Bound<'py, PyAny>,
    collection: &str,
    resource: impl Fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<Bound<'py, PyDict>> = native_field(project, collection)?
        .try_iter()?
        .map(|item| resource(&item?))
        .collect::<PyResult<_>>()?;
    Ok(PyList::new(project.py(), items)?.into_any())
}

fn source_resource<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = source.py();
    dict(
        py,
        vec![
            ("name", native_field(source, "name")?),
            (
                "relative_path",
                native_field(source, "source_file")?
                    .getattr("relative_path")?
                    .str()?
                    .into_any(),
            ),
            (
                "column_count",
                any(
                    py,
                    native_field(source, "source_entry")?
                        .getattr("columns")?
                        .len()?,
                )?,
            ),
        ],
    )
}

fn seed_resource<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = seed.py();
    let item: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("name", native_field(seed, "name")?),
            (
                "relative_path",
                native_field(seed, "seed_file")?
                    .getattr("relative_path")?
                    .str()?
                    .into_any(),
            ),
            (
                "column_count",
                any(
                    py,
                    native_field(seed, "schema_entry")?
                        .getattr("columns")?
                        .len()?,
                )?,
            ),
        ],
    )?;
    qualified_name(&item, seed)?;
    Ok(item)
}

/// The machine-readable offline compile report.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_compile_json(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    compile_json(py, options, None::<fn(Bound<'_, PyString>) -> PyResult<()>>)
}

/// `format_compile_json`, first handing the selected models' partial-check notice to `notice`.
pub(crate) fn format_compile_json_with_notice<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
    notice: impl FnOnce(Bound<'py, PyString>) -> PyResult<()>,
) -> PyResult<String> {
    compile_json(py, options, Some(notice))
}

fn compile_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
    notice: Option<impl FnOnce(Bound<'py, PyString>) -> PyResult<()>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "format_compile_json", options, REPORT_OPTIONS)?;
    let inputs: ReportInputs<'py> = ReportInputs::read(&arguments)?;
    let coverage: Option<Bound<'py, PyDict>> = match notice {
        Some(notice) => {
            let coverage: Bound<'py, PyDict> = inputs.semantic_coverage()?;
            let reader: SurrogateText = SurrogateText::default();
            if let Some(text) = inputs.coverage_text(&coverage, &reader)? {
                notice(reader.python(py, &text)?)?;
            }
            Some(coverage)
        }
        None => None,
    };
    let project: &Bound<'_, PyAny> = &inputs.project;
    let mut has_errors: bool = false;
    let mut diagnostics: Vec<Bound<'_, PyDict>> = Vec::new();
    for diagnostic in &inputs.diagnostics {
        has_errors = has_errors || native_field(diagnostic, "is_error")?.is_truthy()?;
        diagnostics.push(diagnostic_json(diagnostic)?);
    }
    let length = |collection: &str| -> PyResult<usize> { native_field(project, collection)?.len() };
    let summary: Bound<'_, PyDict> = dict(
        py,
        vec![
            ("models", any(py, length("models")?)?),
            ("selected_models", any(py, inputs.selected_models()?.len())?),
            ("sources", any(py, length("sources")?)?),
            ("seeds", any(py, length("seeds")?)?),
            (
                "selected_seeds",
                any(py, inputs.selected_count(&[SEED_TYPE])?)?,
            ),
            ("functions", any(py, length("functions")?)?),
            (
                "selected_functions",
                any(py, inputs.selected_count(&FUNCTION_TYPES)?)?,
            ),
            ("audits", any(py, length("audits")?)?),
            ("tests", any(py, length("sql_tests")?)?),
            (
                "hooks",
                any(py, length("sql_hook_files")? + length("hook_functions")?)?,
            ),
            (
                "execution_layers",
                native_field(&inputs.graph, "native")?.call_method0("model_layer_count")?,
            ),
            ("errors", any(py, inputs.severity_count(ERROR_SEVERITY)?)?),
            (
                "warnings",
                any(py, inputs.severity_count(WARNING_SEVERITY)?)?,
            ),
        ],
    )?;
    let resource_lists: Bound<'_, PyDict> = dict(
        py,
        vec![
            (
                "models",
                resources(project, "models", |model| model_resource(&inputs, model))?,
            ),
            ("sources", resources(project, "sources", source_resource)?),
            ("seeds", resources(project, "seeds", seed_resource)?),
            (
                "functions",
                resources(project, "functions", function_resource)?,
            ),
            ("audits", resources(project, "audits", audit_resource)?),
            ("tests", resources(project, "sql_tests", test_resource)?),
            ("hooks", hook_definitions(project)?.into_any()),
        ],
    )?;
    let artifacts: Bound<'_, PyDict> = dict(
        py,
        vec![
            (
                "compiled_sql_dir",
                inputs.compiled_dir()?.call_method1("__add__", ("/",))?,
            ),
            (
                "manifest",
                if inputs.manifest {
                    any(py, MANIFEST_ARTIFACT)?
                } else {
                    py.None().into_bound(py)
                },
            ),
        ],
    )?;
    if native_field(&inputs.written, "failed_compile")?.is_truthy()? {
        artifacts.set_item(
            "failed_model_sql",
            PyList::new(py, inputs.failed_model_sql()?)?,
        )?;
    }
    let report: Bound<'_, PyDict> = dict(
        py,
        vec![
            ("version", sqlbuild_version(py)?),
            ("command", any(py, "compile")?),
            ("offline", any(py, true)?),
            ("has_errors", any(py, has_errors)?),
            ("summary", summary.into_any()),
            ("diagnostics", PyList::new(py, diagnostics)?.into_any()),
            (
                "semantic_checks_partial",
                match coverage {
                    Some(coverage) => coverage,
                    None => inputs.semantic_coverage()?,
                }
                .into_any(),
            ),
            ("strict", any(py, inputs.strict_mode()?)?),
            ("compile_timings", arguments.value("timings_ms")?),
            ("lineage_mode", any(py, inputs.lineage_mode.as_str())?),
            ("resources", resource_lists.into_any()),
            ("artifacts", artifacts.into_any()),
        ],
    )?;
    emit_json_report(report.as_any())
}

/// A compile that stopped on a raised error, as one JSON diagnostic report.
#[pyfunction]
#[pyo3(signature = (*, code, message, help_text, location=None))]
pub(crate) fn format_compile_error_json<'py>(
    code: Bound<'py, PyAny>,
    message: Bound<'py, PyAny>,
    help_text: Bound<'py, PyAny>,
    location: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    let py: Python<'py> = code.py();
    let location: Bound<'py, PyAny> = location.unwrap_or_else(|| py.None().into_bound(py));
    let (path, line, column) = if location.is_none() {
        (location.clone(), location.clone(), location.clone())
    } else {
        (
            native_field(&location, "path")?,
            native_field(&location, "line")?,
            native_field(&location, "column")?,
        )
    };
    let diagnostic: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            ("phase", any(py, "compile")?),
            ("severity", any(py, ERROR_SEVERITY)?),
            ("code", code),
            ("message", message),
            ("path", path),
            ("line", line),
            ("column", column),
            ("location", location),
            ("help", help_text),
        ],
    )?;
    let report: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("version", sqlbuild_version(py)?),
            ("command", any(py, "compile")?),
            ("offline", any(py, true)?),
            ("has_errors", any(py, true)?),
            ("stopped", any(py, true)?),
            (
                "diagnostics",
                PyList::new(py, [diagnostic_json(&diagnostic)?])?.into_any(),
            ),
        ],
    )?;
    emit_orjson_report(report.as_any())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_compile_text, module)?)?;
    module.add_function(wrap_pyfunction!(format_compile_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_compile_error_json, module)?)
}
