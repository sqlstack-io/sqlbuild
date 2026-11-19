//! Migration relations indexed by schema and name, matched as `MigrationRelation.matches` matches.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use std::collections::HashMap;

/// A relation's `identity`: `(database.lower() or None, (schema or "").lower(), name.lower())`.
type Identity = (Option<String>, String, String);

/// Each indexed relation's lowered database and position, in input order.
type Candidates = Vec<(Option<String>, usize)>;

/// The positions of indexed relations by lowered `(schema, name)`, each list in input order.
#[derive(Debug)]
pub(crate) struct IdentityIndex {
    by_name: HashMap<(String, String), Candidates>,
}

fn identity(relation: &Bound<'_, PyAny>) -> PyResult<Identity> {
    relation.getattr("identity")?.extract()
}

impl IdentityIndex {
    /// Index `relations` by position.
    pub(crate) fn of<'py>(relations: &[Bound<'py, PyAny>]) -> PyResult<Self> {
        let mut by_name: HashMap<(String, String), Candidates> = HashMap::new();
        for (position, relation) in relations.iter().enumerate() {
            let (database, schema, name) = identity(relation)?;
            by_name
                .entry((schema, name))
                .or_default()
                .push((database, position));
        }
        Ok(Self { by_name })
    }

    /// Positions of the indexed relations `relation.matches(...)` accepts, in input order.
    pub(crate) fn matching(&self, relation: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
        let (database, schema, name) = identity(relation)?;
        let mut positions: Vec<usize> = Vec::new();
        let Some(candidates) = self.by_name.get(&(schema, name)) else {
            return Ok(positions);
        };
        for (candidate, position) in candidates {
            let same_database: bool = match (candidate, &database) {
                (Some(left), Some(right)) => left == right,
                _ => true,
            };
            if same_database {
                positions.push(*position);
            }
        }
        Ok(positions)
    }
}
