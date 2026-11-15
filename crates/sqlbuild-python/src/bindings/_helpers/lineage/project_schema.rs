//! Project resources and their columns, read from a compiled project view for lineage.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::lineage::models::{
    LineageResourceType, LineageSchemaResource, RichSchemaResource,
};

use crate::bindings::_helpers::compiled_views::model_reads::{declared_rows, inferred_rows};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

type TypedColumns = Vec<(String, Option<String>)>;

/// The project's models, then sources, then seeds, each with its typed and schema columns.
struct ProjectColumns {
    resource_type: LineageResourceType,
    name: String,
    typed: TypedColumns,
    schema: TypedColumns,
}

/// Fast lineage's schema: model inferred then schema column names; source and seed columns.
pub(crate) fn fast_schema(project: &Bound<'_, PyAny>) -> PyResult<Vec<LineageSchemaResource>> {
    let mut schema: Vec<LineageSchemaResource> = Vec::new();
    for resource in project_columns(project)? {
        let mut columns: Vec<String> = Vec::with_capacity(resource.typed.len());
        for (name, _) in resource.typed.into_iter().chain(resource.schema) {
            columns.push(name);
        }
        schema.push(LineageSchemaResource {
            resource_type: resource.resource_type,
            name: resource.name,
            columns,
        });
    }
    Ok(schema)
}

/// Rich lineage's typed schema: inferred or declared types, then model schema columns.
pub(crate) fn rich_schema(project: &Bound<'_, PyAny>) -> PyResult<Vec<RichSchemaResource>> {
    Ok(project_columns(project)?
        .into_iter()
        .map(|resource| RichSchemaResource {
            resource_type: resource.resource_type,
            name: resource.name,
            assigned: resource.typed,
            defaulted: resource.schema,
        })
        .collect())
}

/// `[name for name, _, _ in model.inferred_rows or ()]`.
pub(crate) fn inferred_names(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    Ok(inferred_columns(model)?
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

fn project_columns(project: &Bound<'_, PyAny>) -> PyResult<Vec<ProjectColumns>> {
    let mut resources: Vec<ProjectColumns> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        resources.push(ProjectColumns {
            resource_type: LineageResourceType::Model,
            name: native_field(&model, "name")?.extract()?,
            typed: inferred_columns(&model)?,
            schema: schema_columns(&model)?,
        });
    }
    for (attribute, entry, resource_type) in [
        ("sources", "source_entry", LineageResourceType::Source),
        ("seeds", "schema_entry", LineageResourceType::Seed),
    ] {
        for resource in native_field(project, attribute)?.try_iter()? {
            let resource: Bound<'_, PyAny> = resource?;
            let mut typed: TypedColumns = Vec::new();
            for column in native_field(&native_field(&resource, entry)?, "columns")?.try_iter()? {
                let column: Bound<'_, PyAny> = column?;
                typed.push((
                    native_field(&column, "name")?.extract()?,
                    native_field(&column, "type")?.extract()?,
                ));
            }
            resources.push(ProjectColumns {
                resource_type,
                name: native_field(&resource, "name")?.extract()?,
                typed,
                schema: Vec::new(),
            });
        }
    }
    Ok(resources)
}

fn inferred_columns(model: &Bound<'_, PyAny>) -> PyResult<TypedColumns> {
    Ok(inferred_rows(model)?
        .unwrap_or_default()
        .into_iter()
        .map(|(name, column_type, _)| (name, column_type))
        .collect())
}

fn schema_columns(model: &Bound<'_, PyAny>) -> PyResult<TypedColumns> {
    Ok(declared_rows(model)?.unwrap_or_default())
}
