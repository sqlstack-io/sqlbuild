//! Whether two unquoted names written in one dialect resolve to the same warehouse object.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;

/// Whether unquoted `left` and `right` name one object in `dialect`; exact without its native module.
#[must_use]
pub fn same_unquoted_name(dialect: Option<&str>, left: &str, right: &str) -> bool {
    let AnalysisRoute::Native(native) = analysis_route(dialect) else {
        return left == right;
    };
    let profile = native.profile();
    profile.same_identifier(
        &profile.resolved_identifier(left, false),
        &profile.resolved_identifier(right, false),
    )
}
