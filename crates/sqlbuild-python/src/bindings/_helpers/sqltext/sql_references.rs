//! Logical SQL references extracted natively under one adapter's lexical rules.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, IntoPyObjectExt, pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_python_base::positional_record;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{
    InvalidReferenceCall, ReferenceExtraction, SqlReference,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::compile_context::main::diagnostic_collector::report_compile_diagnostic::report_compile_diagnostic;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const REFERENCE_CALL_SYNTAX_CODE: &str = "P012";

positional_record! {
    into;
    /// `(kind, name, package, call_argument_count)` for one reference.
    struct ReferenceRow {
        kind: &'static str,
        name: String,
        package: Option<String>,
        call_argument_count: Option<usize>,
    }
}

positional_record! {
    into;
    /// `(kind, call, start, message, help, corrected_call)` for one rejected call.
    struct InvalidCallRow {
        kind: &'static str,
        call: String,
        start: usize,
        message: String,
        help: String,
        corrected_call: String,
    }
}
/// Python references and rejected calls of one scan.
type ScannedObjects<'py> = (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>);
/// The references and rejected calls, or the scan error and its start.
type ExtractionRow = (
    Option<(Vec<ReferenceRow>, Vec<InvalidCallRow>)>,
    Option<(String, usize)>,
);

/// Extracts references under one adapter's lexical rules, read once per syntax.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlReferenceScanner {
    syntax: LexicalSyntax,
}

#[pymethods]
impl SqlReferenceScanner {
    #[new]
    fn new(syntax: LexicalSyntaxInput) -> Self {
        Self {
            syntax: syntax.into(),
        }
    }

    /// Return `((references, rejected calls), None)` or `(None, (message, start))`.
    fn extract(&self, sql: &str) -> PyResult<ExtractionRow> {
        compiler_guard(|| {
            Ok(match extract_sql_references(sql, &self.syntax) {
                ReferenceExtraction::Extracted(scan) => (
                    Some((
                        scan.references.into_iter().map(reference_row).collect(),
                        scan.invalid_calls
                            .into_iter()
                            .map(invalid_call_row)
                            .collect(),
                    )),
                    None,
                ),
                ReferenceExtraction::Failed(failure) => {
                    (None, Some((failure.message, failure.start)))
                }
            })
        })
    }
}

fn reference_row(reference: SqlReference) -> ReferenceRow {
    ReferenceRow {
        kind: reference.kind,
        name: reference.name,
        package: reference.package,
        call_argument_count: reference.call_argument_count,
    }
}

fn invalid_call_row(call: InvalidReferenceCall) -> InvalidCallRow {
    InvalidCallRow {
        kind: call.kind,
        call: call.call,
        start: call.start,
        message: call.message,
        help: call.help,
        corrected_call: call.corrected_call,
    }
}

fn text<'py>(py: Python<'py>, value: impl IntoPyObject<'py>) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

/// The references and rejected calls `sql` holds as Python objects; a scan error raises.
fn scanned<'py>(
    py: Python<'py>,
    sql: &str,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<ScannedObjects<'py>> {
    let input: LexicalSyntaxInput = native_field(syntax, "native_mapping")?.extract()?;
    let syntax: LexicalSyntax = input.into();
    let scan = match compiler_guard(|| Ok(extract_sql_references(sql, &syntax)))? {
        ReferenceExtraction::Extracted(scan) => scan,
        ReferenceExtraction::Failed(failure) => {
            let error: Bound<'py, PyAny> =
                error_class(py, ErrorId::CompileInput)?.call1((failure.message,))?;
            return Err(PyErr::from_value(error));
        }
    };
    let references: Vec<Bound<'py, PyAny>> = scan
        .references
        .into_iter()
        .map(|reference| {
            new_record_with(
                py,
                RecordId::CompileSqlReference,
                &[
                    (
                        "ref_kind",
                        enum_from_value(py, RecordId::SqlReferenceKind, reference.kind)?,
                    ),
                    ("ref_name", text(py, reference.name)?),
                    ("ref_package", text(py, reference.package)?),
                    (
                        "call_argument_count",
                        text(py, reference.call_argument_count)?,
                    ),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    let invalid_calls: Vec<Bound<'py, PyAny>> = scan
        .invalid_calls
        .into_iter()
        .map(|call| {
            new_record_with(
                py,
                RecordId::InvalidSqlReferenceCall,
                &[
                    (
                        "ref_kind",
                        enum_from_value(py, RecordId::SqlReferenceKind, call.kind)?,
                    ),
                    ("call", text(py, call.call)?),
                    ("start", text(py, call.start)?),
                    ("message", text(py, call.message)?),
                    ("help", text(py, call.help)?),
                    ("corrected_call", text(py, call.corrected_call)?),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    Ok((references, invalid_calls))
}

/// Valid references and rejected calls; a scan error raises unlocated.
#[pyfunction]
#[pyo3(signature = (*, sql, syntax))]
fn scan_sql_reference_calls<'py>(
    sql: &str,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = syntax.py();
    let (references, invalid_calls) = scanned(py, sql, syntax)?;
    new_record_with(
        py,
        RecordId::SqlReferenceScan,
        &[
            ("references", PyTuple::new(py, references)?.into_any()),
            ("invalid_calls", PyTuple::new(py, invalid_calls)?.into_any()),
        ],
    )
}

/// Logical SQL refs outside comments and quotes, reporting rejected calls as P012.
#[pyfunction(name = "extract_sql_references")]
#[pyo3(signature = (*, sql, syntax))]
pub(crate) fn extract_references<'py>(
    sql: &str,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = syntax.py();
    let (references, invalid_calls) = scanned(py, sql, syntax)?;
    for call in invalid_calls {
        let diagnostic: Bound<'py, PyAny> = new_record_with(
            py,
            RecordId::CompilerDiagnostic,
            &[
                (
                    "phase",
                    enum_member(py, RecordId::DiagnosticPhase, "COMPILE")?,
                ),
                (
                    "severity",
                    enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?,
                ),
                ("code", text(py, REFERENCE_CALL_SYNTAX_CODE)?),
                ("message", native_field(&call, "message")?),
                ("help", native_field(&call, "help")?),
            ],
        )?;
        report_compile_diagnostic(
            (REFERENCE_CALL_SYNTAX_CODE, "", native_field(&call, "call")?)
                .into_pyobject(py)?
                .as_any(),
            &diagnostic,
        )?;
    }
    PyTuple::new(py, references)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlReferenceScanner>()?;
    module.add_function(wrap_pyfunction!(scan_sql_reference_calls, module)?)?;
    module.add_function(wrap_pyfunction!(extract_references, module)?)?;
    Ok(())
}
