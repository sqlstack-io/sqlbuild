//! Fixture dependencies selected by star projections.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use polyglot_sql::{
    AnalyzeQueryOptions, DialectType, SchemaColumn, SchemaTable, ValidationSchema, analyze_query,
};
use sqlbuild_core::text::main::python_casefold::python_casefold;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::planner_fixtures::_helpers::native_star_keys::native_star_keys;
use crate::planner_fixtures::_helpers::stub_markers::stub_markers;
use crate::planner_fixtures::models::FixtureStarRelation;

/// Resolve star inputs using the same semantic analysis as the compiler.
pub fn star_keys(
    sql: &str,
    relations: &[FixtureStarRelation],
    dialect: Option<&str>,
    proven_inputs: Option<&[String]>,
) -> Vec<(String, String)> {
    if let Some(inputs) = proven_inputs {
        let names: HashSet<String> = inputs.iter().map(|name| python_casefold(name)).collect();
        return relations
            .iter()
            .filter(|relation| names.contains(&python_casefold(&relation.key.1)))
            .map(|relation| relation.key.clone())
            .collect();
    }
    if !relations
        .iter()
        .any(|relation| !relation.columns.is_empty())
    {
        return Vec::new();
    }
    if let AnalysisRoute::Native(native) = analysis_route(dialect) {
        return native_star_keys(native, sql, relations);
    }
    let Ok(dialect) =
        DialectType::from_str(dialect.filter(|name| !name.is_empty()).unwrap_or("generic"))
    else {
        return Vec::new();
    };
    let tables: Vec<SchemaTable> = relations
        .iter()
        .filter(|relation| !relation.columns.is_empty())
        .map(schema_table)
        .collect();
    let options = AnalyzeQueryOptions {
        dialect,
        schema: Some(ValidationSchema {
            tables,
            strict: None,
        }),
        complexity_guard: None,
    };
    let Ok(sql) = stub_markers(sql) else {
        return Vec::new();
    };
    let Ok(analysis) = analyze_query(&sql, options) else {
        return Vec::new();
    };
    let mut aliases: HashMap<String, String> = HashMap::new();
    for relation in analysis.relations {
        let name = python_casefold(&relation.name);
        aliases.insert(name.clone(), name.clone());
        if let Some(alias) = relation.alias {
            aliases.insert(python_casefold(&alias), name);
        }
    }
    let mut names: HashSet<String> = HashSet::new();
    for projection in analysis.star_projections {
        if let Some(table) = projection.table {
            if let Some(name) = aliases.get(&python_casefold(&table)) {
                names.insert(name.clone());
            }
        } else {
            let expanded: HashSet<String> = projection
                .expanded_columns
                .iter()
                .map(|name| python_casefold(name))
                .collect();
            for relation in relations {
                if relation
                    .columns
                    .iter()
                    .any(|column| expanded.contains(&column.folded_name))
                {
                    names.insert(python_casefold(&relation.key.1));
                }
            }
        }
    }
    relations
        .iter()
        .filter(|relation| names.contains(&python_casefold(&relation.key.1)))
        .map(|relation| relation.key.clone())
        .collect()
}

fn schema_table(relation: &FixtureStarRelation) -> SchemaTable {
    SchemaTable {
        name: relation.key.1.clone(),
        schema: None,
        columns: relation
            .columns
            .iter()
            .map(|column| SchemaColumn {
                name: column.name.clone(),
                data_type: column_type(column.sql_type.as_deref()),
                nullable: None,
                primary_key: false,
                unique: false,
                references: None,
            })
            .collect(),
        aliases: Vec::new(),
        primary_key: Vec::new(),
        unique_keys: Vec::new(),
        foreign_keys: Vec::new(),
    }
}

fn column_type(sql_type: Option<&str>) -> String {
    sql_type
        .filter(|name| !name.is_empty())
        .unwrap_or("UNKNOWN")
        .to_owned()
}
