//! Whether a model's compiled SQL reads a relation that none of its declared inputs writes.

use std::collections::{BTreeSet, HashMap};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::auditing::main::replace_reference_calls::replace_reference_calls;
use sqlbuild_state::consumption::models::{InputFact, NodeKey, RelationKey};

use crate::bindings::_helpers::analysis::relation_names::{
    RelationRow, extract_sql_relation_names,
};
use crate::bindings::_helpers::native_host::consumption_state::optional_text;
use crate::bindings::_helpers::records::fields::{field as native_field, mapping_items};

/// Function calls, `__udf("name")(...)`, parsed as plain calls; their reads are declared inputs.
const FUNCTION_CALLS: [&str; 2] = ["__udf", "__table_fn"];

/// The relations each model, seed and source names, by node key or source name.
pub(crate) struct Locations {
    nodes: HashMap<NodeKey, RelationKey>,
    sources: HashMap<String, (Option<String>, Option<String>, String)>,
    /// Every project relation, distinct and lower-cased, by lower-cased relation name.
    by_name: HashMap<String, BTreeSet<RelationKey>>,
}

/// The relation a model or seed destination names; a destination without a schema has an empty one.
pub(crate) fn relation(destination: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    Ok(RelationKey {
        database: optional_text(&native_field(destination, "database")?)?,
        schema: optional_text(&native_field(destination, "schema")?)?.unwrap_or_default(),
        name: native_field(destination, "name")?.extract()?,
    })
}

impl Locations {
    /// `nodes` holds every model and seed relation of the project.
    pub(crate) fn read(
        plan: &Bound<'_, PyAny>,
        nodes: HashMap<NodeKey, RelationKey>,
    ) -> PyResult<Self> {
        let mut sources: HashMap<String, (Option<String>, Option<String>, String)> = HashMap::new();
        for map in ["source_map", "source_read_map"] {
            for item in mapping_items(&native_field(plan, map)?)?.try_iter()? {
                let (name, entry): (String, Bound<'_, PyAny>) = item?.extract()?;
                let table: String =
                    optional_text(&native_field(&entry, "table")?)?.unwrap_or_else(|| name.clone());
                sources.insert(
                    name,
                    (
                        optional_text(&native_field(&entry, "database")?)?,
                        optional_text(&native_field(&entry, "schema")?)?,
                        table,
                    ),
                );
            }
        }
        let mut by_name: HashMap<String, BTreeSet<RelationKey>> = HashMap::new();
        let source_keys = sources
            .values()
            .map(|(database, schema, table)| RelationKey {
                database: database.clone(),
                schema: schema.clone().unwrap_or_default(),
                name: table.clone(),
            });
        for key in nodes.values().cloned().chain(source_keys) {
            let key: RelationKey = RelationKey {
                database: key.database.map(|database| database.to_lowercase()),
                schema: key.schema.to_lowercase(),
                name: key.name.to_lowercase(),
            };
            by_name.entry(key.name.clone()).or_default().insert(key);
        }
        Ok(Self {
            nodes,
            sources,
            by_name,
        })
    }

    /// Whether the SQL relation, with the qualifiers it spells, can name several project relations.
    fn ambiguous(&self, relation: &RelationRow) -> bool {
        self.by_name
            .get(&relation.0.to_lowercase())
            .is_some_and(|keys| keys.iter().filter(|key| names(relation, key)).count() > 1)
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

/// True when `sql` cannot be parsed or names an ambiguous relation or one no input writes.
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
        if created {
            continue;
        }
        if locations.ambiguous(relation)
            || (!own.is_some_and(|own| names(relation, own)) && !declared)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
