use serde_json::Value;
use sqlbuild_orchestration::runs::models::PlannerResource;

/// A published node as the planner receives it from the control plane.
pub(super) fn resource(
    key: &str,
    deps: &[&str],
    tags: &[&str],
    folder: Option<&str>,
) -> PlannerResource {
    PlannerResource {
        key: key.into(),
        deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        folder: folder.map(str::to_owned),
    }
}

/// An expected plan in the planner's owned form, so the case compares whole results.
pub(super) fn owned_plan(
    expected: &Result<(&[&str], Value), &str>,
) -> Result<(Vec<String>, Value), String> {
    expected
        .clone()
        .map(|(keys, settings)| (keys.iter().map(|key| (*key).to_owned()).collect(), settings))
        .map_err(str::to_owned)
}
