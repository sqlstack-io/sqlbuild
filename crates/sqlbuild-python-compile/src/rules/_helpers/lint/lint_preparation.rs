//! One authored body expanded and prepared for native SQL lint, as a `LintBody`.

use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyTuple};
use sqlbuild_rules::lint_text::models::{ExpansionSpan, LintSqlBody};

use crate::rules::_helpers::lint::lint_pipeline::BodyRole;
use crate::rules::_helpers::lint::lint_pipeline::SingleBody;
use crate::rules::_helpers::lint::lint_pipeline::SingleBodyRequest;
use crate::rules::_helpers::lint::lint_pipeline::prepare_single_body;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::render::models::SqlExpansionInputs;

/// `ExpansionSpan` objects of one pass.
fn span_objects<'py>(
    class: &Bound<'py, PyAny>,
    spans: &[ExpansionSpan],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut objects: Vec<Bound<'py, PyAny>> = Vec::with_capacity(spans.len());
    for span in spans {
        objects.push(class.call1((
            span.source_start,
            span.source_end,
            span.output_start,
            span.output_end,
        ))?);
    }
    PyTuple::new(class.py(), objects)
}

/// `ExpansionSpan` tuples for each pass.
fn pass_objects<'py>(
    py: Python<'py>,
    passes: &[Vec<ExpansionSpan>],
) -> PyResult<Bound<'py, PyTuple>> {
    let class: Bound<'py, PyAny> = record_class(py, RecordId::ExpansionSpan)?;
    let passes: Vec<Bound<'py, PyTuple>> = passes
        .iter()
        .map(|spans| span_objects(&class, spans))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, passes)
}

/// Expand a single body and finish its native SQL preparation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn prepare_lint_body<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "prepare_lint_body",
        options,
        (
            &[
                "role",
                "file_path",
                "contents",
                "body_range",
                "context",
                "dialect",
            ],
            &[],
        ),
    )?;
    let role: Bound<'py, PyAny> = arguments.value("role")?;
    let file_path: Bound<'py, PyAny> = arguments.value("file_path")?;
    let contents: String = arguments.value("contents")?.extract()?;
    let dialect: String = arguments.value("dialect")?.extract()?;
    let kind: Option<String> = native_field(&role, "declaration_kind")?.extract()?;
    let prepared: SingleBody = prepare_single_body(&SingleBodyRequest {
        context: &SqlExpansionInputs::from_record(&arguments.value("context")?)?,
        file_path: &file_path,
        contents: &contents,
        range: arguments.value("body_range")?.extract()?,
        role: BodyRole {
            kind: kind.as_deref(),
            in_project: native_field(&role, "in_project")?.is_truthy()?,
            in_hook_directory: native_field(&role, "in_hook_directory")?.is_truthy()?,
        },
        dialect: &dialect,
    })?;
    let body: &LintSqlBody = &prepared.body;
    let flag = |value: bool| PyBool::new(py, value).to_owned().into_any();
    new_record_with(
        py,
        RecordId::LintBody,
        &[
            ("file_path", file_path.clone()),
            ("body_start", body.body_start.into_pyobject(py)?.into_any()),
            ("body_end", prepared.body_end.into_pyobject(py)?.into_any()),
            (
                "lint_text",
                body.lint_text.clone().into_pyobject(py)?.into_any(),
            ),
            ("passes", pass_objects(py, &body.passes)?.into_any()),
            (
                "external_identifiers",
                PyTuple::new(py, &body.external_identifiers)?.into_any(),
            ),
            (
                "dependency_identifiers",
                PyTuple::new(py, &body.dependency_identifiers)?.into_any(),
            ),
            (
                "externally_referenced_ctes",
                PyTuple::new(py, &body.externally_referenced_ctes)?.into_any(),
            ),
            (
                "dependency_relations",
                PyTuple::new(py, &prepared.dependency_relations)?.into_any(),
            ),
            (
                "allows_ceremonial_select",
                flag(body.allows_ceremonial_select),
            ),
            (
                "allows_dynamic_output_star",
                flag(body.allows_dynamic_output_star),
            ),
            (
                "allows_empty_fixture_star",
                flag(body.allows_empty_fixture_star),
            ),
        ],
    )
}
