//! Payload keys and parse limits Python's relation-name walk reads.

/// Keys every table payload carries; Python's `_TABLE_KEYS`.
pub(crate) const TABLE_KEYS: [&str; 5] = ["name", "schema", "catalog", "alias", "hints"];
pub(crate) const NAME_KEY: &str = "name";
pub(crate) const SCHEMA_KEY: &str = "schema";
pub(crate) const CATALOG_KEY: &str = "catalog";
pub(crate) const CTE_LIST_KEY: &str = "ctes";
pub(crate) const CTE_ALIAS_KEY: &str = "alias";
pub(crate) const CREATE_TABLE_KEY: &str = "create_table";
pub(crate) const TEMPORARY_KEY: &str = "temporary";
pub(crate) const QUALIFIER_SEPARATOR: char = '.';
pub(crate) const GENERIC_DIALECT: &str = "generic";
