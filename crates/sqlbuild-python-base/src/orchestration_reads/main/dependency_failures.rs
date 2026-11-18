//! Each published member whose `[dependencies] on_failure`, its override or else the group's, is not `partial`.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use sqlbuild_discovery::orchestration_files::models::GroupDependencies;
use sqlbuild_orchestration::declarations::models::DependencyFailure;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Each published member whose `[dependencies] on_failure`, its override or else the group's, is not `partial`.
#[inline]
pub fn dependency_failures(
    dependencies: &GroupDependencies,
    members: &BTreeSet<String>,
) -> BTreeMap<String, DependencyFailure> {
    orchestration_declarations::dependency_failures(dependencies, members)
}
