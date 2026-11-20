use crate::assembly::dynamic_checks::models::DynamicModel;

/// A dynamic model named `name` reading `reads`.
pub(crate) fn dynamic(name: &str, has_refresh: bool, reads: &[&str]) -> DynamicModel {
    DynamicModel {
        name: name.to_owned(),
        has_refresh,
        reads: reads.iter().map(|read| (*read).to_owned()).collect(),
    }
}
