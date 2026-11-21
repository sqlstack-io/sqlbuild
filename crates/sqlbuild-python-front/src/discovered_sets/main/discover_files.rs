//! The file each discover function is defined in, so a location reruns only what an edit touched.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each discover function's name with its file, as a POSIX path relative to the project.
pub fn discover_files(inputs: &Bound<'_, PyAny>) -> PyResult<BTreeMap<String, String>> {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for record in native_field(inputs, "discover_functions")?.try_iter()? {
        let record: Bound<'_, PyAny> = record?;
        let path: String = native_field(&record, "relative_path")?
            .str()?
            .to_string()
            .replace('\\', "/");
        files.insert(native_field(&record, "name")?.extract()?, path);
    }
    Ok(files)
}
