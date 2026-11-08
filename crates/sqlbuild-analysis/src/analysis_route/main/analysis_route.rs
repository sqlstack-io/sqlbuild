use crate::analysis_route::constants::NATIVE_DIALECTS;
use crate::analysis_route::models::AnalysisRoute;

/// The route of `dialect`, an adapter's analysis dialect name; unlisted dialects stay on Polyglot.
#[must_use]
pub fn analysis_route(dialect: Option<&str>) -> AnalysisRoute {
    dialect
        .and_then(|name| NATIVE_DIALECTS.iter().find(|(native, _)| *native == name))
        .map_or(AnalysisRoute::Legacy, |(_, module)| {
            AnalysisRoute::Native(*module)
        })
}
