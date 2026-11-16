//! Comment, quoted-text and parenthesis boundaries at Python string offsets of one SQL text.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::{pyclass, pymethods};
use sqlbuild_sqltext::code_scanner::main::code_scanner::code_scanner;
use sqlbuild_sqltext::code_scanner::main::scanner_matching_paren::scanner_matching_paren;
use sqlbuild_sqltext::code_scanner::main::scanner_non_code_end::scanner_non_code_end;
use sqlbuild_sqltext::code_scanner::models::CodeScanner;
use sqlbuild_sqltext::sql_scan::models::Unclosed;

use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// One SQL text scanned under an adapter's lexical rules, or the generic compiler rules.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct SqlCodeScanner {
    scanner: CodeScanner,
    context: String,
}

#[pymethods]
impl SqlCodeScanner {
    #[new]
    #[pyo3(signature = (sql, syntax, context))]
    pub(crate) fn new(sql: &str, syntax: Option<LexicalSyntaxInput>, context: String) -> Self {
        Self {
            scanner: code_scanner(sql, syntax.map(Into::into)),
            context,
        }
    }

    /// The end of the comment or quoted text at `index`, or `None` when code starts there.
    pub(crate) fn non_code_end(&self, py: Python<'_>, index: usize) -> PyResult<Option<usize>> {
        scanner_non_code_end(&self.scanner, index).map_err(|unclosed| self.error(py, unclosed))
    }

    /// The index of the parenthesis closing the one at `open`.
    pub(crate) fn matching_paren(&self, py: Python<'_>, open: usize) -> PyResult<usize> {
        scanner_matching_paren(&self.scanner, open).map_err(|unclosed| self.error(py, unclosed))
    }
}

impl SqlCodeScanner {
    /// `CompileInputError("<context> contains an unclosed ...")`.
    fn error(&self, py: Python<'_>, unclosed: Unclosed) -> PyErr {
        let construct: &str = match unclosed {
            Unclosed::BlockComment => "block comment",
            Unclosed::Quote => "quoted string",
            Unclosed::Parenthesis => "parenthesis",
        };
        let message: String = format!("{} contains an unclosed {construct}", self.context);
        let raised = || -> PyResult<PyErr> {
            let error: Bound<'_, PyAny> =
                error_class(py, ErrorId::CompileInput)?.call1((message,))?;
            Ok(PyErr::from_value(error))
        };
        raised().unwrap_or_else(|error| error)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlCodeScanner>()?;
    Ok(())
}
