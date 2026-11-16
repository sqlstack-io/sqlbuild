//! A `ScopeReport`, `ScopeBrowseResult` or, otherwise, `ScopeListResult` as its native result.

use crate::scopes::_helpers::index::scope_objects::field;
use crate::scopes::_helpers::reports::scope_report_objects::read_declaration_reports;
use crate::scopes::_helpers::reports::scope_report_objects::read_diagnostics;
use crate::scopes::_helpers::reports::scope_report_objects::read_folder;
use crate::scopes::_helpers::reports::scope_report_objects::read_report;
use crate::scopes::_helpers::reports::scope_report_objects::read_section;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_scopes::scope_report::models::{BrowseResult, ListResult, ScopeResult};

/// A `ScopeReport`, `ScopeBrowseResult` or, otherwise, `ScopeListResult` as its native result.
pub fn read_result(classes: &ScopeClasses<'_>, result: &Bound<'_, PyAny>) -> PyResult<ScopeResult> {
    if result.is_instance(&classes.model(RecordId::ScopeReport)?)? {
        return Ok(ScopeResult::Report(Box::new(read_report(result)?)));
    }
    if result.is_instance(&classes.model(RecordId::ScopeBrowseResult)?)? {
        return Ok(ScopeResult::Browse(BrowseResult {
            folder: field(result, "folder")?,
            folders: native_field(result, "folders")?
                .try_iter()?
                .map(|folder| read_folder(&folder?))
                .collect::<PyResult<_>>()?,
            diagnostics: read_diagnostics(result)?,
            complete: field(result, "complete")?,
        }));
    }
    Ok(ScopeResult::List(ListResult {
        folder: field(result, "folder")?,
        declarations: read_declaration_reports(result, "declarations")?,
        section: read_section(&native_field(result, "section")?)?,
        diagnostics: read_diagnostics(result)?,
    }))
}
