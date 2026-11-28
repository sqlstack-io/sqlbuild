//! The loading settings of each source a `@loader` fills, read from the loader's SQLLoad table options.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;

/// The source entry facts a loader's options decide, each read from the loader record.
const LOADER_FACTS: [&str; 3] = ["write_strategy", "cursor_column", "unique_key"];

/// Give each source a loader fills the write strategy, cursor and key its `@loader` sets.
pub(crate) fn with_loader_settings(
    declarations: &Bound<'_, PyDict>,
    loaders: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = declarations.py();
    let mut by_name: HashMap<String, Bound<'_, PyAny>> = HashMap::new();
    for loader in loaders.try_iter()? {
        let loader: Bound<'_, PyAny> = loader?;
        by_name.insert(native_field(&loader, "name")?.extract()?, loader);
    }
    let Some(files) = declarations.get_item("source_files")? else {
        return Ok(());
    };
    if by_name.is_empty() {
        return Ok(());
    }
    let mut rebuilt: Vec<Bound<'_, PyAny>> = Vec::new();
    for file in files.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        let mut entries: Vec<Bound<'_, PyAny>> = Vec::new();
        for entry in native_field(&file, "source_entries")?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            let loader: Option<String> = native_field(&entry, "loader")?.extract()?;
            match loader.as_ref().and_then(|name| by_name.get(name)) {
                Some(loader) => entries.push(filled_entry(&entry, loader)?),
                None => entries.push(entry),
            }
        }
        rebuilt.push(replaced(
            &file,
            &[("source_entries", PyTuple::new(py, entries)?.into_any())],
        )?);
    }
    declarations.set_item("source_files", PyTuple::new(py, rebuilt)?)
}

/// `entry` with the facts `loader` decides.
fn filled_entry<'py>(
    entry: &Bound<'py, PyAny>,
    loader: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut changes: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for fact in LOADER_FACTS {
        changes.push((fact, native_field(loader, fact)?));
    }
    replaced(entry, &changes)
}
