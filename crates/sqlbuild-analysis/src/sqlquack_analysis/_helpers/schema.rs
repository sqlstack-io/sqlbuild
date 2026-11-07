//! Relation shapes as SQLQuack's name checks read them.

use std::collections::HashMap;

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};
use sqlquack_semantics::lineage::types::LineageSchema;

use crate::assembly::analysis_session::types::Shapes;
use crate::sqlquack_analysis::constants::{QUOTE, RELATION_PART_SEPARATOR};

/// Known relations' columns by resolved name.
#[derive(Debug, Default)]
pub(crate) struct RelationColumns(pub(crate) HashMap<String, Vec<SchemaColumn>>);

impl LineageSchema for RelationColumns {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.0.get(relation).map(Vec::as_slice)
    }
}

impl RelationColumns {
    /// The relations of `shapes` whose columns are known, a relation with none left open.
    pub(crate) fn from_shapes(shapes: &Shapes, profile: &dyn DialectProfile) -> Self {
        Self(
            shapes
                .iter()
                .filter(|(_, columns)| !columns.is_empty())
                .map(|(relation, columns)| {
                    (
                        relation_key(relation, profile),
                        schema_columns(columns.iter().map(|(name, _)| name.as_str())),
                    )
                })
                .collect(),
        )
    }
}

/// A relation name, each dot-separated part resolved as written unquoted.
fn relation_key(name: &str, profile: &dyn DialectProfile) -> String {
    name.split(RELATION_PART_SEPARATOR)
        .map(|part| profile.resolved_identifier(part, false))
        .collect::<Vec<String>>()
        .join(RELATION_PART_SEPARATOR)
}

/// A relation's columns: a name in double quotes, or listed elsewhere in them, is quoted.
fn schema_columns<'a>(names: impl Iterator<Item = &'a str> + Clone) -> Vec<SchemaColumn> {
    let quoted: Vec<&str> = names.clone().filter_map(unquoted).collect();
    let mut columns: Vec<SchemaColumn> = Vec::new();
    for name in names {
        let column: SchemaColumn = match unquoted(name) {
            Some(inner) => SchemaColumn {
                name: inner.to_owned(),
                quoted: true,
                data_type: None,
                nullability: Nullability::Unknown,
            },
            None => SchemaColumn {
                name: name.to_owned(),
                quoted: quoted.contains(&name),
                data_type: None,
                nullability: Nullability::Unknown,
            },
        };
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn unquoted(name: &str) -> Option<&str> {
    name.strip_prefix(QUOTE)?.strip_suffix(QUOTE)
}
