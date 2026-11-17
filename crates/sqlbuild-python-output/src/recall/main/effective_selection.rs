//! The effective selection: positional selectors and `--select`, then what every `--select-file` lists, with the files as passed. A file that cannot be read adds no selectors here; the command reports it.

use crate::recall::_helpers::session::file_text;
use crate::recall::_helpers::session::list_attribute;
use crate::recall::main::escaped_text::escaped_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_cli::dispatch::main::selector_file_selectors::selector_file_selectors;
use std::path::Path;

/// The effective selection: positional selectors and `--select`, then what every `--select-file` lists, with the files as passed. A file that cannot be read adds no selectors here; the command reports it.
pub fn effective_selection(args: &Bound<'_, PyAny>) -> PyResult<(Vec<String>, Vec<String>)> {
    let mut selectors: Vec<String> = list_attribute(args, "select")?;
    let files: Vec<String> = match args.getattr_opt("select_file")? {
        Some(value) if !value.is_none() => value
            .try_iter()?
            .map(|item| escaped_text(&item?.str()?))
            .collect::<PyResult<_>>()?,
        _ => Vec::new(),
    };
    for file in &files {
        if let Some(text) = file_text(Path::new(file)) {
            selectors.extend(selector_file_selectors(&text));
        }
    }
    Ok((selectors, files))
}
