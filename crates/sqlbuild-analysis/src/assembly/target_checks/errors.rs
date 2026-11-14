//! The target namespace check failures.

use crate::assembly::target_checks::models::{LoaderCollision, MissingParts};

/// One target namespace check failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetError {
    /// A resource of an adapter that needs both parts resolved without some of them.
    MissingParts(MissingParts),
    /// Two targets on one connection write managed loader data to one namespace.
    LoaderCollision(LoaderCollision),
    /// A managed write, named by its label, without an explicit physical schema.
    ManagedWriteSchema(String),
    /// A named target without an explicit schema.
    NamedTargetSchema(String),
    /// An intermediate loader, by name, whose schema resolves to the target's `preserve`.
    PreservedLoaderSchema(String),
}
