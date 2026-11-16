//! A parsed dynamic column family's fact, keyed by its casefolded name.

use crate::compiled_project::_helpers::schema_facts;
use sqlbuild_analysis::compiled_project::models::DynamicFamilyFact;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

/// A parsed dynamic column family's fact, keyed by its casefolded name.
#[inline]
pub fn family_fact(family: &DynamicFamily) -> DynamicFamilyFact {
    schema_facts::family_fact(family)
}
