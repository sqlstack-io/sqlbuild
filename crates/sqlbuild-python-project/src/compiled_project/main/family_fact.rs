//! A parsed dynamic column family's fact, keyed by its casefolded name.

use sqlbuild_analysis::compiled_project::models::DynamicFamilyFact;
use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

/// A parsed dynamic column family's fact, keyed by its casefolded name.
pub fn family_fact(family: &DynamicFamily) -> DynamicFamilyFact {
    DynamicFamilyFact {
        key: python_casefold(&family.name),
        name: family.name.clone(),
        pivot_column: family.pivot_column.clone(),
        value_column: family.value_column.clone(),
        aggregate: family.aggregate.clone(),
        data_type: family.data_type.clone(),
        name_pattern: family.name_pattern.clone(),
    }
}
