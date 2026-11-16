//! The compile-and-plan pipeline of execution commands: compile, rules gate, then planning.

use crate::compile_driver::main::setup::progress::progress;
use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PySet, PySetMethods};

use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// Raise one `CompileInputError` listing every compile error the command must stop on.
pub(crate) fn reject_compile_errors(
    project: &Bound<'_, PyAny>,
    options: &Bound<'_, PyAny>,
    on_progress: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = project.py();
    let plan_sql_tests: bool = native_field(options, "plan_sql_tests")?.is_truthy()?;
    let sql_test: Bound<'_, PyAny> = enum_member(py, RecordId::CompiledResourceType, "SQL_TEST")?;
    let mut errors: Vec<Bound<'_, PyAny>> = Vec::new();
    for diagnostic in native_field(project, "diagnostics")?.try_iter()? {
        let diagnostic: Bound<'_, PyAny> = diagnostic?;
        if native_field(&diagnostic, "is_error")?.is_truthy()?
            && (plan_sql_tests || native_field(&diagnostic, "resource_type")?.ne(&sql_test)?)
        {
            errors.push(diagnostic);
        }
    }
    let Some(first) = errors.first() else {
        return Ok(());
    };
    progress(on_progress, "Project compilation failed.")?;
    let mut lines: Vec<String> = Vec::with_capacity(errors.len());
    let helps: Bound<'_, PySet> = PySet::empty(py)?;
    for diagnostic in &errors {
        let or_one = |name: &str| -> PyResult<String> {
            let value: Bound<'_, PyAny> = native_field(diagnostic, name)?;
            Ok(if value.is_truthy()? {
                value.str()?.to_string()
            } else {
                "1".to_owned()
            })
        };
        lines.push(format!(
            "[{}] {}:{}:{}: {}",
            native_field(diagnostic, "code")?.str()?,
            native_field(diagnostic, "path")?.str()?,
            or_one("line")?,
            or_one("column")?,
            native_field(diagnostic, "message")?.str()?,
        ));
        helps.add(native_field(diagnostic, "help")?)?;
    }
    let help: Bound<'_, PyAny> = if helps.len() == 1 {
        helps.pop().unwrap_or_else(|| py.None().into_bound(py))
    } else {
        py.None().into_bound(py)
    };
    let error: Bound<'_, PyAny> = error_class(py, ErrorId::CompileInput)?.call(
        (lines.join("\n"),),
        Some(&keywords(
            py,
            &[("code", native_field(first, "code")?), ("help", help)],
        )?),
    )?;
    Err(PyErr::from_value(error))
}
