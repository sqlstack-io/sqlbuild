//! Discover model schema.yml and seed declaration files natively.

use crate::discovery::_helpers::failures::faults::record_tuple;
use crate::discovery::_helpers::failures::raised::{discovery_error, stage_error};
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::records::yaml_records::ListingTree;
use crate::discovery::_helpers::records::yaml_records::SCHEMA_KIND;
use crate::discovery::_helpers::records::yaml_records::YamlKind;
use crate::discovery::_helpers::records::yaml_records::listing_tree;
use crate::discovery::_helpers::records::yaml_records::load_files;
use crate::discovery::_helpers::records::yaml_records::path_class;
use crate::discovery::_helpers::snapshots::snapshot::in_snapshot;
use crate::discovery::_helpers::sources::yaml_entries::parse_schema_document;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyStringMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_discovery::yaml_files::main::schema_yaml_paths::schema_yaml_paths;
use sqlbuild_discovery::yaml_files::models::YamlPathFailure;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Discover model schema.yml and seed declaration files natively.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn discover_schema_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    in_snapshot(project_dir, || {
        let listing: ListingTree<'py> = listing_tree(project_dir)?;
        let paths: Vec<String> = match schema_yaml_paths(listing.tree()) {
            Ok(paths) => paths,
            Err(YamlPathFailure::Stage(failure)) => {
                return Err(stage_error(listing.tree(), project_dir, failure));
            }
            Err(YamlPathFailure::YamlSeedDeclaration(path)) => {
                let relative: Bound<'py, PyAny> =
                    path_class(py)?.call1((path_object(py, listing.tree(), path)?,))?;
                return Err(discovery_error(
                    py,
                    SCHEMA_KIND,
                    format!(
                        "Seed declaration file {} must use .yml; .yaml is not supported",
                        relative.str()?
                    ),
                    None,
                ));
            }
        };
        if paths.is_empty() {
            return record_tuple(py, Vec::new());
        }
        let mut records: Vec<Bound<'py, PyAny>> = Vec::new();
        load_files(
            project_dir,
            (paths, YamlKind::Schema),
            |(relative_path, file_path), built| {
                let (contents, loaded) = built?;
                let shown: String = file_path.str()?.to_str()?.to_owned();
                records.push(new_record_with(
                    py,
                    RecordId::DiscoveredSchemaFile,
                    &[
                        ("file_path", file_path.clone()),
                        ("relative_path", relative_path),
                        ("contents", contents.into_bound_py_any(py)?),
                        ("model_entries", PyTuple::empty(py).into_any()),
                        (
                            "seed_entries",
                            parse_schema_document(py, loaded, &shown)?.into_any(),
                        ),
                    ],
                )?);
                Ok(())
            },
        )?;
        record_tuple(py, records)
    })
}
