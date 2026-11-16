//! Fixture planning key and input shapes.

/// One relation's `((name, type, nullable), ...)` columns and whether they are authoritative.
pub(crate) type FixtureRelationInput = (Vec<(String, Option<String>, Option<bool>)>, bool);
