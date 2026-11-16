//! In a delta, add the text of files that stored diagnostics point into but were not rerun.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyStringMethods};
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// In a delta, add the text of files that stored diagnostics point into but were not rerun.
pub fn complete_source_texts(
    project: &Bound<'_, PyAny>,
    diagnostics: &[Bound<'_, PyAny>],
    texts: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let Some(injection) = injection() else {
        return Ok(());
    };
    let _ = project;
    let project_dir: &std::path::Path = &injection.project_dir;
    let mut locations: Vec<Bound<'_, PyAny>> = Vec::new();
    for diagnostic in diagnostics {
        locations.push(native_field(diagnostic, "location")?);
        for item in native_field(diagnostic, "related_locations")?.try_iter()? {
            locations.push(native_field(&item?, "location")?);
        }
    }
    for location in locations.into_iter().filter(|location| !location.is_none()) {
        let path: Bound<'_, PyAny> = native_field(&location, "path")?;
        if texts.contains(&path)? {
            continue;
        }
        let relative: String = path.str()?.to_str()?.replace('\\', "/");
        if !injection.source_paths.contains(&relative) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(project_dir.join(&relative)) {
            texts.set_item(&path, text)?;
        }
    }
    Ok(())
}
