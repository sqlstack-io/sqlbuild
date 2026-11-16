//! Discover source YAML files natively, reporting failing files to `on_fault`.

use crate::discovery::_helpers::failures::faults::{FileFaults, record_tuple};
use crate::discovery::_helpers::failures::raised::stage_error;
use crate::discovery::_helpers::records::yaml_records::ListingTree;
use crate::discovery::_helpers::records::yaml_records::YamlKind;
use crate::discovery::_helpers::records::yaml_records::listing_tree;
use crate::discovery::_helpers::records::yaml_records::load_files;
use crate::discovery::_helpers::snapshots::snapshot::in_snapshot;
use crate::discovery::_helpers::sources::yaml_entries::parse_sources_document;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyStringMethods, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_discovery::yaml_files::main::source_yaml_paths::source_yaml_paths;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Discover source YAML files natively, reporting failing files to `on_fault`.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub fn discover_source_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    in_snapshot(project_dir, || {
        let listing: ListingTree<'py> = listing_tree(project_dir)?;
        let paths: Vec<String> = source_yaml_paths(listing.tree())
            .map_err(|failure| stage_error(listing.tree(), project_dir, failure))?;
        if paths.is_empty() {
            return record_tuple(py, Vec::new());
        }
        let faults = FileFaults {
            project_dir: project_dir.clone(),
            on_fault,
        };
        let mut records: Vec<Bound<'py, PyAny>> = Vec::new();
        load_files(
            project_dir,
            (paths, YamlKind::Source),
            |(relative_path, file_path), built| {
                let record = built.and_then(|(contents, loaded)| {
                    let shown: String = file_path.str()?.to_str()?.to_owned();
                    new_record_with(
                        py,
                        RecordId::DiscoveredSourceFile,
                        &[
                            ("file_path", file_path.clone()),
                            ("relative_path", relative_path.clone()),
                            ("contents", contents.into_bound_py_any(py)?),
                            (
                                "source_entries",
                                parse_sources_document(py, loaded, &shown)?.into_any(),
                            ),
                        ],
                    )
                });
                records.extend(faults.kept(&relative_path, record)?);
                Ok(())
            },
        )?;
        record_tuple(py, records)
    })
}
