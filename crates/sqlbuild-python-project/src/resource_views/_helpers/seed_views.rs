//! Seeds paired with their declarations, held by the compiled project, and their views.

use crate::resource_views::models::SeedHandles;
use crate::resource_views::models::SeedNamespace;
use crate::resource_views::models::SeedParts;
use crate::resource_views::types::SeedRecord;
use crate::resource_views::types::SeedViews;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use sqlbuild_attachments::seeds::main::pair_seed_declarations::pair_seed_declarations;

use crate::compiled_project::models::NativeCompiledProject;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const SEED_RESOURCE: &str = "seed";
pub(crate) const TAGS_ATTRIBUTE: &str = "tags";
const NAME_ATTRIBUTE: &str = "name";
const FILE_PATH_ATTRIBUTE: &str = "file_path";
const STEM_ATTRIBUTE: &str = "stem";
const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";

/// Pair each declaration with the CSV file of its name, raising for the first without one.
pub(crate) fn paired_seeds(
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

impl SeedViews for SeedRecord {
    fn parts(&self) -> SeedParts<'_> {
        SeedParts {
            name: &self.name,
            seed_file: &self.host.seed_file,
            schema_entry: &self.host.schema_entry,
            schema_file: &self.host.schema_file,
        }
    }

    fn set_namespace(&mut self, namespace: SeedNamespace) {
        let SeedNamespace {
            database,
            schema,
            logical_database,
            logical_schema,
        } = namespace;
        self.destination = Some(DestinationFact {
            database,
            schema,
            name: self.name.clone(),
            qualified_name: None,
            logical_schema,
            logical_database,
        });
    }
}

pub(crate) fn read_seed<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&SeedRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    let seed: &SeedRecord = owner
        .seeds()
        .get(index)
        .ok_or_else(|| PyIndexError::new_err(index))?;
    read(seed, py)
}
