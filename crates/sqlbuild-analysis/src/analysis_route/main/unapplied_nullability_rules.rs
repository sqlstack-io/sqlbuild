use crate::analysis_route::models::AnalysisRoute;

/// The warning for an adapter's own nullability rules on a native route, which does not call them.
#[must_use]
pub fn unapplied_nullability_rules(
    route: &AnalysisRoute,
    adapter: &str,
    functions: &[String],
) -> Option<String> {
    match route {
        AnalysisRoute::Native(_) if !functions.is_empty() => Some(format!(
            "Adapter '{adapter}' defines its own nullability rule for {}; native SQL analysis \
             does not apply it, so these calls take the dialect's own nullability",
            functions.join(", ")
        )),
        _ => None,
    }
}
