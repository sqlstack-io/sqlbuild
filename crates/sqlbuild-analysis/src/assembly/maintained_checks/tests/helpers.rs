use crate::assembly::maintained_checks::models::MaintainedModel;

/// A maintained model named `name` reading `reads`.
pub(crate) fn maintained(name: &str, has_refresh: bool, reads: &[&str]) -> MaintainedModel {
    MaintainedModel {
        name: name.to_owned(),
        has_refresh,
        reads: reads.iter().map(|read| (*read).to_owned()).collect(),
    }
}
