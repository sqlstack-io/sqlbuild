//! `report_invalid_reference_calls`: P012 for each rejected reference call, located when authored.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::compiled_project::main::located_scan_error::located_scan_error;
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::ScanFaultOrigin;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{
    InvalidReferenceCall, ReferenceExtraction, SqlReference,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::compile_error;

const REFERENCE_CALL_SYNTAX_CODE: &str = "P012";
const MODEL_RESOURCE: &str = "model";

/// The model name, or none for a function, and the relative path, file path and contents of a file.
#[derive(Clone, Copy)]
pub(crate) struct CallOrigin<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) relative_path: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) contents: &'a str,
}

/// Report each rejected call in order, at its authored location when the authored text has it.
pub(crate) fn report_invalid_calls(
    context: &LoopContext<'_, '_>,
    syntax: &LexicalSyntax,
    invalid_calls: &[InvalidReferenceCall],
    origin: CallOrigin<'_>,
) -> PyResult<()> {
    if invalid_calls.is_empty() {
        return Ok(());
    }
    let CallOrigin { contents, .. } = origin;
    let authored: HashMap<String, Vec<usize>> = match extract_sql_references(contents, syntax) {
        ReferenceExtraction::Extracted(scan) => {
            let mut starts: HashMap<String, Vec<usize>> = HashMap::new();
            for call in scan.invalid_calls {
                starts.entry(call.call).or_default().push(call.start);
            }
            starts
        }
        ReferenceExtraction::Failed(_) => HashMap::new(),
    };
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for invalid_call in invalid_calls {
        let occurrence: &mut usize = seen.entry(invalid_call.call.as_str()).or_default();
        let start: Option<usize> = authored
            .get(&invalid_call.call)
            .and_then(|starts| starts.get(*occurrence).copied());
        *occurrence += 1;
        report_invalid_call(context, invalid_call, start, origin)?;
    }
    Ok(())
}

/// Report one rejected call as P012, at `start` in the origin's contents when given.
pub(crate) fn report_invalid_call(
    context: &LoopContext<'_, '_>,
    invalid_call: &InvalidReferenceCall,
    start: Option<usize>,
    origin: CallOrigin<'_>,
) -> PyResult<()> {
    let CallOrigin {
        name,
        relative_path,
        file_path,
        contents,
    } = origin;
    let py: Python<'_> = context.host.py();
    let path: Bound<'_, PyAny> = context.classes.path_class.call1((relative_path,))?;
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("phase", "compile")?;
    fields.set_item("severity", "error")?;
    fields.set_item("code", REFERENCE_CALL_SYNTAX_CODE)?;
    fields.set_item("message", &invalid_call.message)?;
    fields.set_item("resource_type", name.map(|_| MODEL_RESOURCE))?;
    fields.set_item("resource_name", name)?;
    fields.set_item("help", &invalid_call.help)?;
    let key: Bound<'_, PyTuple> = match start {
        Some(start) => {
            let begin: TextPosition = text_position(contents, start);
            let end: TextPosition =
                text_position(contents, start + invalid_call.call.chars().count());
            let location: Bound<'_, PyDict> = PyDict::new(py);
            location.set_item("path", &path)?;
            location.set_item("line", begin.line)?;
            location.set_item("column", begin.column)?;
            location.set_item("end_line", end.line)?;
            location.set_item("end_column", end.column)?;
            fields.set_item(
                "location",
                context
                    .contracts
                    .source_location
                    .call((), Some(&location))?,
            )?;
            PyTuple::new(
                py,
                [
                    REFERENCE_CALL_SYNTAX_CODE.to_owned(),
                    file_path.to_owned(),
                    format!("{:09}:{:09}", begin.line, begin.column),
                    invalid_call.call.clone(),
                ],
            )?
        }
        None => {
            fields.set_item("path", &path)?;
            PyTuple::new(
                py,
                [
                    REFERENCE_CALL_SYNTAX_CODE.to_owned(),
                    file_path.to_owned(),
                    invalid_call.call.clone(),
                ],
            )?
        }
    };
    let diagnostic: Bound<'_, PyAny> = context
        .classes
        .compiler_diagnostic
        .call((), Some(&fields))?;
    let arguments: Bound<'_, PyDict> = PyDict::new(py);
    arguments.set_item("key", key)?;
    arguments.set_item("diagnostic", diagnostic)?;
    context
        .classes
        .report_diagnostic
        .call((), Some(&arguments))?;
    Ok(())
}

/// `extract_sql_references(sql, origin=file)`, reporting rejected calls and located scan errors.
pub(crate) fn origin_references(
    context: &LoopContext<'_, '_>,
    syntax: &LexicalSyntax,
    sql: &str,
    file: (&str, &str, &str),
) -> PyResult<Vec<SqlReference>> {
    let (file_path, relative_path, contents) = file;
    let scanned = match extract_sql_references(sql, syntax) {
        ReferenceExtraction::Extracted(scanned) => scanned,
        ReferenceExtraction::Failed(failure) => {
            let origin: ScanFaultOrigin<'_> = ScanFaultOrigin {
                relative_path,
                contents,
                query_sql: sql,
                passes: None,
            };
            let message: String = located_scan_error(&failure, sql, &origin, syntax);
            return Err(compile_error(context.classes, &message, None));
        }
    };
    report_invalid_calls(
        context,
        syntax,
        &scanned.invalid_calls,
        CallOrigin {
            name: None,
            relative_path,
            file_path,
            contents,
        },
    )?;
    Ok(scanned.references)
}
