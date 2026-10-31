//! `physical_name`: the physical SQL identifier analysis reads a resource by.

use crate::lineage::_helpers::references::physical_resource_name;
use crate::lineage::models::LineageResourceType;

/// The physical name of the resource `name` of `resource_type`, as Python's `_physical_resource_name`.
pub fn physical_name(resource_type: LineageResourceType, name: &str) -> String {
    physical_resource_name(resource_type, name)
}
