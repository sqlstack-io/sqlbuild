//! Python's `_build_schema_mapping` and `_build_star_lineage`.

use std::collections::{HashMap, HashSet};

use crate::lineage::_helpers::references::physical_resource_name;
use crate::lineage::models::PhysicalResource;
use crate::lineage::models::{
    LineageColumn, LineageConfidence, LineageSchemaResource, LineageSource, LineageTransformKind,
};

/// Known columns by physical name; a later resource with columns replaces an earlier one.
pub(crate) fn schema_mapping(resources: &[LineageSchemaResource]) -> HashMap<String, Vec<String>> {
    let mut schema = HashMap::with_capacity(resources.len());
    for resource in resources {
        let mut seen = HashSet::with_capacity(resource.columns.len());
        let columns: Vec<String> = resource
            .columns
            .iter()
            .filter(|column| seen.insert(column.as_str()))
            .cloned()
            .collect();
        if !columns.is_empty() {
            let _ = schema.insert(
                physical_resource_name(resource.resource_type, &resource.name),
                columns,
            );
        }
    }
    schema
}

/// Star columns for every referenced resource's known columns not already output.
pub(crate) fn star_lineage<'a>(
    schema: &HashMap<String, Vec<String>>,
    physical_resources: &[PhysicalResource],
    existing_columns: impl IntoIterator<Item = &'a str>,
) -> Vec<LineageColumn> {
    let mut seen: HashSet<String> = existing_columns.into_iter().map(str::to_owned).collect();
    let mut lineages: Vec<LineageColumn> = Vec::new();
    for resource in physical_resources {
        for column_name in schema.get(&resource.physical_name).into_iter().flatten() {
            if !seen.insert(column_name.clone()) {
                continue;
            }
            lineages.push(LineageColumn {
                output_column: column_name.clone(),
                transform_kind: LineageTransformKind::Star,
                confidence: LineageConfidence::Medium,
                upstream_columns: vec![LineageSource {
                    resource_type: resource.resource_type,
                    resource_name: resource.resource_name.clone(),
                    column_name: column_name.clone(),
                }],
            });
        }
    }
    lineages
}
