//! Star dependencies read from SQLQuack's tree, a superset of what each star reads.

use std::collections::{HashMap, HashSet};

use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::queries::models::Select;
use sqlquack_syntax::traversal::types::ExpressionWalk;

use crate::analysis_route::types::NativeDialect;
use crate::planner_fixtures::_helpers::stub_markers::stub_markers;
use crate::planner_fixtures::models::FixtureStarRelation;

/// Each qualifier in a query, casefolded, to every relation it names there.
type Qualifiers = HashMap<String, HashSet<String>>;

/// Relations the stars read: the qualifier's relations, or every relation given when unsettled.
pub(crate) fn native_star_keys(
    native: &dyn NativeDialect,
    sql: &str,
    relations: &[FixtureStarRelation],
) -> Vec<(String, String)> {
    let every = || {
        relations
            .iter()
            .map(|relation| relation.key.clone())
            .collect()
    };
    let Ok(sql) = stub_markers(sql) else {
        return every();
    };
    let Ok(root) = native.read_whole(&sql) else {
        return every();
    };
    let Some(names) = star_relations(&root) else {
        return every();
    };
    relations
        .iter()
        .filter(|relation| names.contains(&python_casefold(&relation.key.1)))
        .map(|relation| relation.key.clone())
        .collect()
}

/// The relations the query's stars read, or None when a star's qualifier names no relation.
fn star_relations(root: &Expression) -> Option<HashSet<String>> {
    let mut qualifiers: Qualifiers = HashMap::new();
    let mut stars: Vec<Option<String>> = Vec::new();
    for node in ExpressionWalk::dfs(root) {
        match node {
            Expression::Table(table) => {
                let name: String = python_casefold(&table.name.name);
                let written = std::iter::once(name.clone())
                    .chain(table.alias.iter().map(|alias| python_casefold(&alias.name)));
                for qualifier in written {
                    qualifiers
                        .entry(qualifier)
                        .or_default()
                        .insert(name.clone());
                }
            }
            Expression::Select(select) => stars.extend(projected_stars(select)),
            _ => {}
        }
    }
    let mut names: HashSet<String> = HashSet::new();
    for star in &stars {
        let read: &HashSet<String> = match star {
            Some(qualifier) => qualifiers.get(qualifier)?,
            None => {
                names.extend(qualifiers.values().flatten().cloned());
                continue;
            }
        };
        names.extend(read.iter().cloned());
    }
    Some(names)
}

/// The SELECT's projected stars: each one's casefolded qualifier, or None for a bare star.
fn projected_stars(select: &Select) -> Vec<Option<String>> {
    select
        .expressions
        .iter()
        .filter_map(star_qualifier)
        .collect()
}

/// A star projection's casefolded qualifier, None for a bare star; not a star, no entry.
fn star_qualifier(projection: &Expression) -> Option<Option<String>> {
    let Expression::Star(star) = projection else {
        return None;
    };
    Some(
        star.table
            .as_ref()
            .map(|table| python_casefold(&table.name)),
    )
}
