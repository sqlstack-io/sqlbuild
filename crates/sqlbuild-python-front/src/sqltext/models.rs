//! Public records of sqltext.

use crate::render::types::SpanRow;
use pyo3::FromPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_sqltext::code_scanner::main::code_scanner::code_scanner;
use sqlbuild_sqltext::code_scanner::main::scanner_matching_paren::scanner_matching_paren;
use sqlbuild_sqltext::code_scanner::main::scanner_non_code_end::scanner_non_code_end;
use sqlbuild_sqltext::code_scanner::models::CodeScanner;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use sqlbuild_sqltext::sql_scan::models::Unclosed;

/// One SQL text scanned under an adapter's lexical rules, or the generic compiler rules.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct SqlCodeScanner {
    pub(crate) scanner: CodeScanner,
    pub(crate) context: String,
}

#[pymethods]
impl SqlCodeScanner {
    #[new]
    #[pyo3(signature = (sql, syntax, context))]
    pub fn new(sql: &str, syntax: Option<LexicalSyntaxInput>, context: String) -> Self {
        Self {
            scanner: code_scanner(sql, syntax.map(Into::into)),
            context,
        }
    }

    /// The end of the comment or quoted text at `index`, or `None` when code starts there.
    pub fn non_code_end(&self, py: Python<'_>, index: usize) -> PyResult<Option<usize>> {
        scanner_non_code_end(&self.scanner, index).map_err(|unclosed| self.error(py, unclosed))
    }

    /// The index of the parenthesis closing the one at `open`.
    pub fn matching_paren(&self, py: Python<'_>, open: usize) -> PyResult<usize> {
        scanner_matching_paren(&self.scanner, open).map_err(|unclosed| self.error(py, unclosed))
    }
}

impl SqlCodeScanner {
    /// `CompileInputError("<context> contains an unclosed ...")`.
    pub(crate) fn error(&self, py: Python<'_>, unclosed: Unclosed) -> pyo3::prelude::PyErr {
        let construct: &str = match unclosed {
            Unclosed::BlockComment => "block comment",
            Unclosed::Quote => "quoted string",
            Unclosed::Parenthesis => "parenthesis",
        };
        let message: String = format!("{} contains an unclosed {construct}", self.context);
        let raised = || -> PyResult<pyo3::prelude::PyErr> {
            let error: Bound<'_, PyAny> =
                error_class(py, ErrorId::CompileInput)?.call1((message,))?;
            Ok(pyo3::prelude::PyErr::from_value(error))
        };
        raised().unwrap_or_else(|error| error)
    }
}

/// One natively scanned reference: kind 0 is an enum member, 1 a constant, 2 a SQL macro call.
#[derive(Debug, Clone)]
pub struct ScannedReference {
    pub kind: u8,
    pub name: String,
    pub member: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// One adapter's `SqlLexicalSyntax`, read from a Python mapping.
#[derive(Debug, Clone, FromPyObject)]
#[pyo3(from_item_all)]
pub struct LexicalSyntaxInput {
    pub(crate) backslash_escape_quotes: Vec<String>,
    pub(crate) escape_string_prefix: bool,
    pub(crate) raw_string_prefix: bool,
    pub(crate) triple_quoted_strings: bool,
    pub(crate) nested_block_comments: bool,
    pub(crate) line_comment_prefixes: Vec<String>,
}

/// A column name and its authored line, column, end line and end column.
#[derive(Debug)]
pub struct ColumnLocation {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl<'py> pyo3::IntoPyObject<'py> for ColumnLocation {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.name,
                self.line,
                self.column,
                self.end_line,
                self.end_column,
            ),
            py,
        )
    }
}

/// One SQL text interpolated: its SQL, substitution spans, reads and error message.
#[derive(Debug)]
pub struct InterpolationRow {
    /// The interpolated SQL, or `None` when unchanged.
    pub sql: Option<String>,
    pub spans: Vec<SpanRow>,
    pub reads: Vec<(&'static str, String)>,
    pub error: Option<String>,
}

/// Variables, environment, context values (or `None`), variable renderer and lexical rules.
#[derive(Debug)]
pub struct InterpolationSources<'py>(
    pub Bound<'py, PyDict>,
    pub Bound<'py, PyAny>,
    pub Option<Bound<'py, PyDict>>,
    pub Bound<'py, PyAny>,
    /// The lexical rules the SQL is authored in; `None` reads it under the generic rules.
    pub Option<LexicalSyntax>,
);
