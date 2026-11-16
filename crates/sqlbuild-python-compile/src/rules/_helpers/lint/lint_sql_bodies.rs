//! Native SQL lint of prepared lint bodies, as Python lint violations.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_rules::lint_text::main::lint_sql_bodies::lint_sql_bodies;
use sqlbuild_rules::lint_text::models::{
    AuthoredDiagnostic, ExpansionSpan, LintSqlBody, LintSqlOptions,
};

use crate::rules::_helpers::lint::lint_pipeline::relation_names as sentinel_relation_names;
use crate::rules::models::LintViolationRow;
use crate::rules::models::ViolationClasses;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::types::CompilerDetach;

fn spans(passes: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<ExpansionSpan>>> {
    passes.try_iter()?.map(|pass| pass_spans(&pass?)).collect()
}

fn pass_spans(pass: &Bound<'_, PyAny>) -> PyResult<Vec<ExpansionSpan>> {
    pass.try_iter()?
        .map(|span| {
            let span: Bound<'_, PyAny> = span?;
            Ok(ExpansionSpan {
                source_start: native_field(&span, "source_start")?.extract()?,
                source_end: native_field(&span, "source_end")?.extract()?,
                output_start: native_field(&span, "output_start")?.extract()?,
                output_end: native_field(&span, "output_end")?.extract()?,
            })
        })
        .collect()
}

/// Declared keys of the relations one body reads, by their neutralized sentinel.
fn relation_keys(
    body: &Bound<'_, PyAny>,
    declared: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, Vec<Vec<String>>)>> {
    let mut keys: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for relation in native_field(body, "dependency_relations")?.try_iter()? {
        let (sentinel, name): (String, Bound<'_, PyAny>) = relation?.extract()?;
        let found: Bound<'_, PyAny> = mapping_get(declared, name)?;
        if found.is_truthy()? {
            keys.push((sentinel, found.extract()?));
        }
    }
    Ok(keys)
}

/// The relation name each dependency sentinel of one body stands for.
fn relation_names(body: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut relations: Vec<(String, String)> = Vec::new();
    for relation in native_field(body, "dependency_relations")?.try_iter()? {
        relations.push(relation?.extract()?);
    }
    Ok(sentinel_relation_names(&relations))
}

fn lint_body(
    body: &Bound<'_, PyAny>,
    index_by_path: &Bound<'_, PyDict>,
    declared: &Bound<'_, PyAny>,
) -> PyResult<LintSqlBody> {
    let file: usize = index_by_path
        .as_any()
        .get_item(native_field(body, "file_path")?)?
        .extract()?;
    Ok(LintSqlBody {
        file,
        body_start: native_field(body, "body_start")?.extract()?,
        lint_text: native_field(body, "lint_text")?.extract()?,
        passes: spans(&native_field(body, "passes")?)?,
        external_identifiers: native_field(body, "external_identifiers")?.extract()?,
        dependency_identifiers: native_field(body, "dependency_identifiers")?.extract()?,
        externally_referenced_ctes: native_field(body, "externally_referenced_ctes")?.extract()?,
        allows_ceremonial_select: native_field(body, "allows_ceremonial_select")?.extract()?,
        allows_dynamic_output_star: native_field(body, "allows_dynamic_output_star")?.extract()?,
        allows_empty_fixture_star: native_field(body, "allows_empty_fixture_star")?.extract()?,
        relation_keys: relation_keys(body, declared)?,
        relation_names: relation_names(body)?,
        expression_frame: None,
    })
}

fn options(config: &Bound<'_, PyAny>) -> PyResult<LintSqlOptions> {
    Ok(LintSqlOptions {
        dialect: native_field(config, "dialect")?.extract()?,
        enabled_rules: native_field(config, "enabled_native_rules")?.extract()?,
        ignored_rules: native_field(config, "ignored_native_rules")?.extract()?,
        max_literal_length: native_field(config, "max_literal_length")?.extract()?,
        max_ranking_order_by: native_field(config, "max_ranking_order_by")?.extract()?,
    })
}

/// Lint expanded bodies natively and map every diagnostic onto authored source.
#[pyfunction]
pub(crate) fn lint_native_sql_bodies<'py>(
    py: Python<'py>,
    bodies: Vec<Bound<'py, PyAny>>,
    contents_by_path: Bound<'py, PyDict>,
    config: Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let index_by_path: Bound<'py, PyDict> = PyDict::new(py);
    let mut paths: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    for (path, text) in contents_by_path.iter() {
        index_by_path.set_item(&path, paths.len())?;
        paths.push(path);
        texts.push(text.extract()?);
    }
    let declared: Bound<'py, PyAny> = native_field(&config, "relation_keys")?;
    let inputs: Vec<LintSqlBody> = bodies
        .iter()
        .map(|body| lint_body(body, &index_by_path, &declared))
        .collect::<PyResult<_>>()?;
    let options: LintSqlOptions = options(&config)?;
    let diagnostics: Vec<AuthoredDiagnostic> = py
        .compiler_detach(|| {
            let files: Vec<&str> = texts.iter().map(String::as_str).collect();
            Ok(lint_sql_bodies(&inputs, &files, &options))
        })
        .map_err(value_error)?;
    let classes: ViolationClasses<'py> = ViolationClasses::new(py)?;
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let row: LintViolationRow = LintViolationRow::diagnostic(diagnostic);
            classes.violation(&paths[row.file], &row)
        })
        .collect()
}
