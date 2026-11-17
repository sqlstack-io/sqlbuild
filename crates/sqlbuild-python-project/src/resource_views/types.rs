//! Public traits and type aliases of resource views.

use crate::compiled_views::models::PyProjectHost;
use crate::resource_views::models::SeedNamespace;
use crate::resource_views::models::SeedParts;
use crate::resource_views::models::SourceKeyFacts;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_project::models as project_models;

/// What the audit loop attached for one singular audit block or attached generic audit.
pub type AttachedAudit = project_models::AttachedAudit<PyProjectHost>;

/// One attached audit held by the compiled project.
pub type AuditRecord = project_models::AuditRecord<PyProjectHost>;

/// What the function loop attached for one SQL or Python function.
pub type AttachedFunction = project_models::AttachedFunction<PyProjectHost>;

/// One attached function held by the compiled project.
pub type FunctionRecord = project_models::FunctionRecord<PyProjectHost>;

/// One seed CSV paired with its declaration; the discovered objects are discovery's.
pub type SeedRecord = project_models::SeedRecord<PyProjectHost>;

/// What a seed record reads and keeps through its Python values.
pub trait SeedViews {
    /// The seed's name, CSV file, declaration and declaring schema file.
    fn parts(&self) -> SeedParts<'_>;

    /// Keep the destination native project assembly resolved for the seed.
    fn set_namespace(&mut self, namespace: SeedNamespace);
}

/// What the source loop attached for one source: its expanded metadata and usages.
pub type AttachedSource = project_models::AttachedSource<PyProjectHost>;

/// One attached source held by the compiled project.
pub type SourceRecord = project_models::SourceRecord<PyProjectHost>;

/// What a source record projects from its Python values, each built once.
pub trait SourceViews {
    /// The source's declared column facts and audit key rows, read once from its expanded specs.
    fn key_facts(&self, py: Python<'_>) -> PyResult<&SourceKeyFacts>;

    /// The attached `SourceEntry`: the discovered entry with its expanded metadata.
    fn attached_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>>;

    /// The assembled `SourceEntry`: the attached entry in its resolved namespace.
    fn assembled_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>>;
}
