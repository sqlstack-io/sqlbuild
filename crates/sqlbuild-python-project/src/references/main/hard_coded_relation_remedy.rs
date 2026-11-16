//! How an owner of `owner_kind` should read `target` instead of naming its relation in SQL.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_analysis::explicit_references::main::relation_remedy::relation_remedy;
use sqlbuild_analysis::explicit_references::types::ResourceKey;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// How an owner of `owner_kind` should read `target` instead of naming its relation in SQL.
#[pyfunction]
#[pyo3(signature = (*, owner_kind, r#ref, upstream_loader_by_source = None))]
pub fn hard_coded_relation_remedy(
    owner_kind: &str,
    r#ref: &Bound<'_, PyAny>,
    upstream_loader_by_source: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let target: ResourceKey = (
        native_field(r#ref, "kind")?.getattr("value")?.extract()?,
        native_field(r#ref, "name")?.extract()?,
    );
    let mut upstream_loaders: Vec<(String, String)> = Vec::new();
    if let Some(loaders) = upstream_loader_by_source.filter(|loaders| !loaders.is_none()) {
        for item in mapping_items(loaders)?.try_iter()? {
            upstream_loaders.push(item?.extract()?);
        }
    }
    Ok(relation_remedy(owner_kind, &target, &upstream_loaders))
}
