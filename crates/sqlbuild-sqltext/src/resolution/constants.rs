//! Alias boundaries retained from the planner resolution contract.

pub(super) const EXTERNAL_REFERENCE: &str = "dbt_ref";
/// The keyword of a partition clause that follows a table function call.
pub(super) const OVER_KEYWORD: &str = "OVER";

pub(super) const REF_CLAUSES: &[&str] = &[
    "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "CROSS", "ON", "GROUP", "ORDER", "HAVING",
    "LIMIT", "UNION", "QUALIFY",
];
pub(super) const SOURCE_CLAUSES: &[&str] = &[
    "APPLY",
    "CROSS",
    "EXCEPT",
    "FULL",
    "GROUP",
    "HAVING",
    "INNER",
    "INTERSECT",
    "JOIN",
    "LEFT",
    "LIMIT",
    "OFFSET",
    "ON",
    "ORDER",
    "OUTER",
    "RIGHT",
    "UNION",
    "WHERE",
];
