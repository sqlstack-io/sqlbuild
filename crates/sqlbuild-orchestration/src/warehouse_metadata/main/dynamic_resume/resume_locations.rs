//! The state locations a sync resumes expired dynamic-model holds in.

use crate::warehouse_metadata::models::{DynamicResumeLocation, WarehouseStateLocation};

/// None unless the adapter builds dynamic models; else the requested ones and the pause table's.
pub fn resume_locations(
    requested: Vec<DynamicResumeLocation>,
    (dynamic, pause_location): (bool, Option<WarehouseStateLocation>),
) -> Vec<DynamicResumeLocation> {
    if !dynamic {
        return Vec::new();
    }
    let mut locations: Vec<DynamicResumeLocation> = requested;
    if let Some(location) = pause_location
        && !locations
            .iter()
            .any(|requested| requested.location == location)
    {
        locations.push(DynamicResumeLocation {
            location,
            quiet_token: None,
        });
    }
    locations
}
