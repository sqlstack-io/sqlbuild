//! Whether a model's compiled SQL reads a relation that none of its declared inputs writes.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::auditing::main::replace_reference_calls::replace_reference_calls;
use sqlbuild_state::consumption::models::{InputFact, NodeKey, RelationKey};

use crate::bindings::_helpers::analysis::relation_names::{
    RelationRow, extract_sql_relation_names,
};
use crate::bindings::_helpers::compiled_views::{model_reads, project_reads, seed_reads};
use crate::bindings::_helpers::records::fields::{field as native_field, mapping_items};

/// Function calls, `__udf("name")(...)`, parsed as plain calls; their reads are declared inputs.
const FUNCTION_CALLS: [&str; 2] = ["__udf", "__table_fn"];

/// The relations each model, seed and source names, by node key or source name.
pub(crate) struct Locations {
    nodes: HashMap<NodeKey, RelationKey>,
    sources: HashMap<String, (Option<String>, Option<String>, String)>,
}

fn optional(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

fn location(destination: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    Ok(RelationKey {
        database: optional(native_field(destination, "database")?)?,
        schema: optional(native_field(destination, "schema")?)?.unwrap_or_default(),
        name: native_field(destination, "name")?.extract()?,
    })
}

impl Locations {
    pub(crate) fn read(plan: &Bound<'_, PyAny>, project: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut nodes: HashMap<NodeKey, RelationKey> = HashMap::new();
        for model in project_reads::models(project)?.try_iter()? {
            let model: Bound<'_, PyAny> = model?;
            nodes.insert(
                NodeKey {
                    kind: "model".to_owned(),
                    name: model_reads::model_name(&model)?,
                },
                location(&model_reads::destination(&model)?)?,
            );
        }
        for seed in project_reads::seeds(project)?.try_iter()? {
            let seed: Bound<'_, PyAny> = seed?;
            nodes.insert(
                NodeKey {
                    kind: "seed".to_owned(),
                    name: seed_reads::name(&seed)?.extract()?,
                },
                location(&seed_reads::destination(&seed)?)?,
            );
        }
        let mut sources: HashMap<String, (Option<String>, Option<String>, String)> = HashMap::new();
        for map in ["source_map", "source_read_map"] {
            for item in mapping_items(&native_field(plan, map)?)?.try_iter()? {
                let (name, entry): (String, Bound<'_, PyAny>) = item?.extract()?;
                let table: String =
                    optional(native_field(&entry, "table")?)?.unwrap_or_else(|| name.clone());
                sources.insert(
                    name,
                    (
                        optional(native_field(&entry, "database")?)?,
                        optional(native_field(&entry, "schema")?)?,
                        table,
                    ),
                );
            }
        }
        Ok(Self { nodes, sources })
    }

    fn allows(&self, input: &InputFact, relation: &RelationRow) -> bool {
        match input {
            InputFact::Node(key) => self
                .nodes
                .get(key)
                .is_some_and(|location| names(relation, location)),
            InputFact::Source { name, .. } => {
                self.sources
                    .get(name)
                    .is_some_and(|(database, schema, table)| {
                        names(
                            relation,
                            &RelationKey {
                                database: database.clone(),
                                schema: schema.clone().unwrap_or_default(),
                                name: table.clone(),
                            },
                        )
                    })
            }
            InputFact::Opaque(_) => false,
        }
    }
}

/// Whether the SQL relation, with the qualifiers it spells, can name `location`.
fn names((name, schema, database): &RelationRow, location: &RelationKey) -> bool {
    let same = |left: &str, right: &str| left.eq_ignore_ascii_case(right);
    same(name, &location.name)
        && schema
            .as_deref()
            .is_none_or(|schema| same(schema, &location.schema))
        && database.as_deref().is_none_or(|database| {
            location
                .database
                .as_deref()
                .is_some_and(|known| same(database, known))
        })
}

/// True when `sql` cannot be parsed or names a relation no input or its own relation writes.
pub(crate) fn sql_reads_undeclared(
    py: Python<'_>,
    sql: String,
    (inputs, own): (&[InputFact], Option<&RelationKey>),
    (locations, dialect): (&Locations, Option<&str>),
) -> PyResult<bool> {
    let sql: String = FUNCTION_CALLS.iter().fold(sql, |sql, function| {
        replace_reference_calls(&sql, function, |_| {
            Ok::<_, std::convert::Infallible>(Some((*function).to_owned()))
        })
        .unwrap_or_else(|never| match never {})
    });
    let Some((relations, temporary)) = extract_sql_relation_names(py, &sql, dialect)? else {
        return Ok(true);
    };
    for relation in &relations {
        let created: bool = temporary
            .iter()
            .any(|created| created.eq_ignore_ascii_case(&relation.0));
        let declared: bool = inputs.iter().any(|input| locations.allows(input, relation));
        if !created && !own.is_some_and(|own| names(relation, own)) && !declared {
            return Ok(true);
        }
    }
    Ok(false)
}
