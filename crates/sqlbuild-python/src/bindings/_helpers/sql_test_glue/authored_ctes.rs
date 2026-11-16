//! Reject authored SQL tests and scenarios whose CTE names cannot be read, at the authored text.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_analysis::compiler::main::sql_test_classification::extract_sql_tests;
use sqlbuild_analysis::compiler::models::{SqlTestExtractionFailure, SqlTestRequest};
use sqlbuild_analysis::sql_test_attachment::main::authored_test_offset::authored_test_offset;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const MODEL_MODE: &str = "model";

/// Extract fixture bodies from authored test or scenario CTEs in authored order.
pub(crate) fn prefixed_ctes<'py>(
    ctes: &Bound<'py, PyAny>,
    prefix: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(ctes.py());
    for cte in ctes.try_iter()? {
        let cte = cte?;
        let name: String = native_field(&cte, "name")?.extract()?;
        if let Some(name) = name.strip_prefix(prefix) {
            result.set_item(name, native_field(&cte, "sql_body")?)?;
        }
    }
    Ok(result)
}

/// One authored SQL body to read: its text, file contents, relative path and body span.
struct AuthoredBody {
    sql: String,
    contents: String,
    relative_path: String,
    /// `str(relative_path)`, the label extraction errors name the file by.
    file_label: String,
    span: Option<(usize, usize)>,
}

impl AuthoredBody {
    fn read(
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
fn check_bodies(
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

/// Reject the first authored test block, then scenario, whose CTE names cannot be read.
#[pyfunction]
pub(crate) fn validate_authored_sql_test_ctes(
    py: Python<'_>,
    test_files: Bound<'_, PyAny>,
    scenario_files: Bound<'_, PyAny>,
    syntax: LexicalSyntaxInput,
) -> Result<(), PyErr> {
    let syntax: LexicalSyntax = syntax.into();
    let mut blocks: Vec<AuthoredBody> = Vec::new();
    for test_file in test_files.try_iter()? {
        let test_file: Bound<'_, PyAny> = test_file?;
        for block in native_field(&test_file, "blocks")?.try_iter()? {
            let block: Bound<'_, PyAny> = block?;
            blocks.push(AuthoredBody::read(
                &block,
                &native_field(&block, "sql_body")?,
                &test_file,
            )?);
        }
    }
    check_bodies(py, &blocks, false, &syntax)?;
    let mut scenarios: Vec<AuthoredBody> = Vec::new();
    for scenario_file in scenario_files.try_iter()? {
        let scenario_file: Bound<'_, PyAny> = scenario_file?;
        scenarios.push(AuthoredBody::read(
            &scenario_file,
            &native_field(&scenario_file, "sql_body")?,
            &scenario_file,
        )?);
    }
    check_bodies(py, &scenarios, true, &syntax)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_authored_sql_test_ctes, module)?)
}
