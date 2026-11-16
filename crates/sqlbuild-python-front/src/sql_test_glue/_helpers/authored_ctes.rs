//! Reject authored SQL tests and scenarios whose CTE names cannot be read, at the authored text.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_analysis::compiler::main::sql_test_classification::extract_sql_tests;
use sqlbuild_analysis::compiler::models::{SqlTestExtractionFailure, SqlTestRequest};
use sqlbuild_analysis::sql_test_attachment::main::authored_test_offset::authored_test_offset;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const MODEL_MODE: &str = "model";

/// One authored SQL body to read: its text, file contents, relative path and body span.
pub(crate) struct AuthoredBody {
    sql: String,
    contents: String,
    relative_path: String,
    /// `str(relative_path)`, the label extraction errors name the file by.
    file_label: String,
    span: Option<(usize, usize)>,
}

impl AuthoredBody {
    pub(crate) fn read(
        owner: &Bound<'_, PyAny>,
        sql: &Bound<'_, PyAny>,
        file: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            sql: sql.extract()?,
            contents: native_field(file, "contents")?.extract()?,
            relative_path: native_field(file, "relative_path")?
                .call_method0("as_posix")?
                .extract()?,
            file_label: native_field(file, "relative_path")?.str()?.extract()?,
            span: native_field(owner, "sql_body_span")?.extract()?,
        })
    }
}

/// `CompileInputError(message, help=help)` at the authored offset when it is exact.
fn located_error(py: Python<'_>, failure: &SqlTestExtractionFailure, body: &AuthoredBody) -> PyErr {
    let offset: Option<usize> = failure.token.as_ref().and_then(|(_, offset)| {
        authored_test_offset(&body.contents, &body.sql, body.span, *offset)
    });
    let message: String = match offset {
        Some(offset) => {
            let position: TextPosition = text_position(&body.contents, offset);
            format!(
                "{}\n  --> {}:{}:{}",
                failure.message, body.relative_path, position.line, position.column
            )
        }
        None => failure.message.clone(),
    };
    let raised = || -> PyResult<PyErr> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", failure.help.as_deref())?;
        let error: Bound<'_, PyAny> =
            error_class(py, ErrorId::CompileInput)?.call((message,), Some(&keywords))?;
        Ok(PyErr::from_value(error))
    };
    raised().unwrap_or_else(|error| error)
}

/// Raise the located error of the first body whose authored CTEs cannot be read.
pub(crate) fn check_bodies(
    py: Python<'_>,
    bodies: &[AuthoredBody],
    scenario: bool,
    syntax: &LexicalSyntax,
) -> PyResult<()> {
    if bodies.is_empty() {
        return Ok(());
    }
    let requests: Vec<SqlTestRequest> = bodies
        .iter()
        .map(|body| SqlTestRequest {
            sql: body.sql.clone(),
            file_label: body.file_label.clone(),
            mode: MODEL_MODE.to_owned(),
            raw: false,
            authored: true,
            scenario,
        })
        .collect();
    match extract_sql_tests(&requests, syntax) {
        Ok(_) => Ok(()),
        Err(failure) => Err(located_error(py, &failure, &bodies[failure.index])),
    }
}
