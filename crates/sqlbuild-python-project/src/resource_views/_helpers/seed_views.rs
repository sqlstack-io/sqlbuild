//! Seeds paired with their declarations, held by the compiled project, and their views.

use crate::resource_views::models::SeedNamespace;
use crate::resource_views::models::SeedParts;
use crate::resource_views::types::SeedRecord;
use crate::resource_views::types::SeedViews;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Py, PyResult, Python};

use crate::compiled_project::models::NativeCompiledProject;
use sqlbuild_project::models::DestinationFact;

pub(crate) const SEED_RESOURCE: &str = "seed";
pub(crate) const TAGS_ATTRIBUTE: &str = "tags";
pub(crate) const NAME_ATTRIBUTE: &str = "name";
pub(crate) const FILE_PATH_ATTRIBUTE: &str = "file_path";
pub(crate) const STEM_ATTRIBUTE: &str = "stem";
pub(crate) const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";

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
