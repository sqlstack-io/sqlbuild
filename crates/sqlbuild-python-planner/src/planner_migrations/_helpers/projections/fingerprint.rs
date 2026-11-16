//! Migration identity binding with unchanged metadata and hash format.

use crate::planner_migrations::main::fingerprint::fingerprint;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use std::collections::BTreeMap;

/// Hash normalized SQL and non-storage metadata through rename identities.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_migration_fingerprint(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<Option<String>> {
    let arguments = KeywordArguments::read(
        py,
        "build_migration_fingerprint",
        options,
        (
            &["query_sql", "metadata_json", "ref_identities", "dialect"],
            &[],
        ),
    )?;
    let mut identities: BTreeMap<String, String> = BTreeMap::new();
    for item in arguments
        .value("ref_identities")?
        .call_method0("items")?
        .try_iter()?
    {
        let (key, value): (String, String) = item?.extract()?;
        identities.insert(key, value);
    }
    let dialect: Option<String> = arguments.value("dialect")?.extract()?;
    fingerprint(
        &arguments.value("query_sql")?.extract::<String>()?,
        &arguments.value("metadata_json")?.extract::<String>()?,
        &identities,
        dialect.as_deref(),
    )
}
