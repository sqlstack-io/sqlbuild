//! Pair each declaration with the CSV file of its name, raising for the first without one.

use crate::resource_views::_helpers::seed_views::FILE_PATH_ATTRIBUTE;
use crate::resource_views::_helpers::seed_views::NAME_ATTRIBUTE;
use crate::resource_views::_helpers::seed_views::RELATIVE_PATH_ATTRIBUTE;
use crate::resource_views::_helpers::seed_views::STEM_ATTRIBUTE;
use crate::resource_views::models::SeedHandles;
use crate::resource_views::types::SeedRecord;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use sqlbuild_attachments::seeds::main::pair_seed_declarations::pair_seed_declarations;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Pair each declaration with the CSV file of its name, raising for the first without one.
pub fn paired_seeds(
    declarations: Vec<(Py<PyAny>, Py<PyAny>)>,
    seed_files: &[Py<PyAny>],
    compile_error: &Bound<'_, PyAny>,
) -> PyResult<Vec<SeedRecord>> {
    let py: Python<'_> = compile_error.py();
    let mut names: Vec<String> = Vec::with_capacity(declarations.len());
    for (entry, _) in &declarations {
        names.push(native_field(entry.bind(py), NAME_ATTRIBUTE)?.extract()?);
    }
    let mut stems: Vec<String> = Vec::with_capacity(seed_files.len());
    for seed_file in seed_files {
        stems.push(
            native_field(
                &native_field(seed_file.bind(py), FILE_PATH_ATTRIBUTE)?,
                STEM_ATTRIBUTE,
            )?
            .extract()?,
        );
    }
    let pairs: Vec<usize> = match pair_seed_declarations(&names, &stems) {
        Ok(pairs) => pairs,
        Err(missing) => {
            let schema_file: &Py<PyAny> = &declarations[missing].1;
            let relative_path: String =
                native_field(schema_file.bind(py), RELATIVE_PATH_ATTRIBUTE)?
                    .str()?
                    .extract()?;
            let message: String = format!(
                "Seed declaration '{}' in {relative_path} has no matching CSV file under seeds/",
                names[missing]
            );
            return Err(PyErr::from_value(compile_error.call1((message,))?));
        }
    };
    let mut seeds: Vec<SeedRecord> = Vec::with_capacity(pairs.len());
    for (((schema_entry, schema_file), name), file_index) in
        declarations.into_iter().zip(names).zip(pairs)
    {
        seeds.push(SeedRecord {
            name,
            destination: None,
            host: SeedHandles {
                seed_file: seed_files[file_index].clone_ref(py),
                schema_entry,
                schema_file,
                key: PyOnceLock::new(),
            },
        });
    }
    Ok(seeds)
}
