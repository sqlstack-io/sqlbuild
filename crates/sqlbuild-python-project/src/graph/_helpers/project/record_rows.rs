//! The graph rows of compiled resource records: their keys, dependencies, tags and folders.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::tag_texts::tag_texts;

use crate::graph::main::nodes::native_key::native_key;
use crate::graph::models::ResourceRow;

/// A resource's tags and model folder.
pub(crate) type ResourceTags = (Vec<String>, Option<String>);
/// Reads one resource's tags and model folder.
pub(crate) type DescribeResource = fn(&Bound<'_, PyAny>) -> PyResult<ResourceTags>;

/// The graph rows of each resource in `resources`, with tags and folder from `describe`.
pub(crate) fn resource_rows(
    resources: &Bound<'_, PyAny>,
    describe: DescribeResource,
) -> PyResult<Vec<ResourceRow>> {
    let mut rows: Vec<ResourceRow> = Vec::new();
    for resource in resources.try_iter()? {
        let resource: Bound<'_, PyAny> = resource?;
        let (tags, folder) = describe(&resource)?;
        rows.push(ResourceRow {
            key: native_key(&native_field(&resource, "key")?)?,
            deps: native_field(&resource, "deps")?
                .try_iter()?
                .map(|key| native_key(&key?))
                .collect::<PyResult<_>>()?,
            tags,
            folder,
        });
    }
    Ok(rows)
}

/// A model's configured tags and folder.
pub(crate) fn model_tags(model: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let values: Bound<'_, PyAny> = native_field(&native_field(model, "config")?, "values")?;
    let tags: Vec<String> = match values.cast::<PyDict>()?.get_item("tags")? {
        Some(tags) => tag_texts(&tags)?,
        None => Vec::new(),
    };
    let folder: String = native_field(&native_field(model, "relative_path")?, "parent")?
        .str()?
        .extract()?;
    Ok((tags, Some(folder)))
}

/// A source has neither tags nor folder.
pub(crate) fn source_tags(_source: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    Ok((Vec::new(), None))
}

/// A seed's schema entry tags.
pub(crate) fn seed_tags(seed: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let tags: Vec<String> = native_field(&native_field(seed, "schema_entry")?, "tags")?
        .try_iter()?
        .map(|tag| tag?.extract())
        .collect::<PyResult<_>>()?;
    Ok((tags, None))
}

/// A function's tags.
pub(crate) fn function_tags(function: &Bound<'_, PyAny>) -> PyResult<ResourceTags> {
    let tags: Vec<String> = native_field(function, "tags")?
        .try_iter()?
        .map(|tag| tag?.extract())
        .collect::<PyResult<_>>()?;
    Ok((tags, None))
}
