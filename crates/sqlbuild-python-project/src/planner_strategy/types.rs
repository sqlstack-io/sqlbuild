//! Public traits and type aliases of planner strategy.

/// `(resource type value, name)` of one fixture relation.
pub type FixtureKey = (String, String);

/// One relation's `((name, type, nullable), ...)` columns and whether they are authoritative.
pub(crate) type FixtureRelationInput = (Vec<(String, Option<String>, Option<bool>)>, bool);
