//! In-memory declaration file parsing, for the oracle comparing native and Python parsing.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_discovery::declaration_files::main::parse_declaration_text::parse_declaration_text;
use sqlbuild_discovery::declaration_files::models::{
    CollectionKind, DeclarationFileOptions, ParsedDeclarationText,
};
use sqlbuild_discovery::models::FileOutcome;

use crate::bindings::models::{DeclarationTextRequest, NativeDiscoverySession};
use sqlbuild_python_base::types::CompilerDetach;

const PARSED_TAG: &str = "ok";
const FAILED_TAG: &str = "error";
const UNREADABLE_TAG: &str = "read";

/// Whether a `kind` declaration file with these contents parses (`ok`), fails or is unreadable.
#[pyfunction]
fn parse_declaration_contents(
    py: Python<'_>,
    request: DeclarationTextRequest,
    names: (String, String),
    contents: String,
) -> PyResult<&'static str> {
    let (file_path, hook_name): (String, String) = names;
    let kind: CollectionKind = CollectionKind::from_name(&request.kind).ok_or_else(|| {
        PyValueError::new_err(format!("unknown declaration collection {:?}", request.kind))
    })?;
    let options = DeclarationFileOptions {
        function_keys: request.function_keys,
        audit_keys: request.audit_keys,
        hook_keys: request.hook_keys,
    };
    let parsed: Option<FileOutcome<ParsedDeclarationText>> = py
        .compiler_detach(|| {
            Ok(parse_declaration_text(
                kind,
                (&file_path, &hook_name),
                contents,
                &options,
            ))
        })
        .map_err(sqlbuild_python_base::boundary::main::compiler_error::compiler_error)?;
    match parsed {
        Some(FileOutcome::Parsed(_)) => Ok(PARSED_TAG),
        Some(FileOutcome::Failed(_)) => Ok(FAILED_TAG),
        Some(FileOutcome::Unreadable(_)) => Ok(UNREADABLE_TAG),
        None => Err(PyValueError::new_err(format!(
            "{} files have no authored text to parse",
            kind.as_str()
        ))),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDiscoverySession>()?;
    module.add_function(wrap_pyfunction!(parse_declaration_contents, module)?)?;
    Ok(())
}
