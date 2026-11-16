//! Synthetic loader records for sources declaring an ingestr or dlt integration loader.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_discovery::yaml_entries::main::integration_loader_name::integration_loader_name;
use sqlbuild_host::entries::models::ThirdPartyHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// One integration: its kind constant and the runtime module building its loader functions.
struct Integration {
    kind_constant: ThirdPartyHost,
    loader_factory: ThirdPartyHost,
}

/// Integrations in loader order: every ingestr loader precedes every dlt loader.
const INTEGRATIONS: [Integration; 2] = [
    Integration {
        kind_constant: ThirdPartyHost::IngestrIntegrationKind,
        loader_factory: ThirdPartyHost::IngestrLoaderFunction,
    },
    Integration {
        kind_constant: ThirdPartyHost::DltIntegrationKind,
        loader_factory: ThirdPartyHost::DltLoaderFunction,
    },
];

/// The external-connection `DiscoveredLoaderFunction` of every integration source, in file order.
pub(crate) fn integration_loader_functions<'py>(
    py: Python<'py>,
    source_files: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut declared: Vec<(String, Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
    for source_file in source_files.try_iter()? {
        let source_file: Bound<'py, PyAny> = source_file?;
        for source_entry in native_field(&source_file, "source_entries")?.try_iter()? {
            let source_entry: Bound<'py, PyAny> = source_entry?;
            let loader: Bound<'py, PyAny> = native_field(&source_entry, "integration_loader")?;
            if loader.is_none() {
                continue;
            }
            let kind: String = native_field(&loader, "kind")?.extract()?;
            declared.push((kind, source_file.clone(), source_entry));
        }
    }
    if declared.is_empty() {
        return Ok(PyTuple::empty(py).into_any());
    }
    let external: Bound<'py, PyAny> = enum_member(py, RecordId::LoaderConnectionMode, "EXTERNAL")?;
    let mut loaders: Vec<Bound<'py, PyAny>> = Vec::new();
    for integration in &INTEGRATIONS {
        let kind: String = host_entry(py, integration.kind_constant)?.extract()?;
        let factory: Bound<'py, PyAny> = host_entry(py, integration.loader_factory)?;
        for (_, source_file, source_entry) in declared.iter().filter(|loader| loader.0 == kind) {
            let name: String = integration_loader_name(
                &kind,
                &native_field(source_entry, "name")?.extract::<String>()?,
            );
            let function: Bound<'py, PyAny> = call_with(
                &factory,
                &[
                    ("source_entry", source_entry.clone()),
                    ("name", name.clone().into_pyobject(py)?.into_any()),
                ],
            )?;
            loaders.push(new_record_with(
                py,
                RecordId::DiscoveredLoaderFunction,
                &[
                    ("file_path", native_field(source_file, "file_path")?),
                    ("relative_path", native_field(source_file, "relative_path")?),
                    ("name", name.into_pyobject(py)?.into_any()),
                    ("function", function),
                    ("connection_mode", external.clone()),
                ],
            )?);
        }
    }
    Ok(PyTuple::new(py, loaders)?.into_any())
}
