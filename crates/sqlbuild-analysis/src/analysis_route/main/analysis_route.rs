use crate::analysis_route::constants::NATIVE_DIALECTS;
use crate::analysis_route::models::AnalysisRoute;

/// An adapter's analysis `dialect`'s route: its native module, or unavailable when none reads it.
#[must_use]
pub fn analysis_route(dialect: Option<&str>) -> AnalysisRoute {
    dialect
        .and_then(|name| NATIVE_DIALECTS.iter().find(|(native, _)| *native == name))
        .map_or(AnalysisRoute::Unavailable, |(_, module)| {
            AnalysisRoute::Native(*module)
        })
}
