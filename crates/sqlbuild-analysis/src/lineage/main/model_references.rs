//! `model_references`: a model's SQL with references written as physical names, and those references.

use crate::lineage::_helpers::references::{normalized_sql, physical_resources};
use crate::lineage::models::PhysicalResource;

/// `sql` with each reference and UDF call written as its physical name, and the references it makes.
pub fn model_references(sql: &str) -> (String, Vec<PhysicalResource>) {
    (normalized_sql(sql), physical_resources(sql))
}
