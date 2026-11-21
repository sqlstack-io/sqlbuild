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
        loader: None,
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

/// SQL models, a source with its loaders, Python nodes and a job, all published, indexed by the server's planner.
pub(super) fn orders_publication()
-> std::sync::Arc<dyn sqlbuild_orchestration::runs::types::PreparedPublication> {
    let resources = vec![
        resource("model:stg_orders", &[], &["orders"], Some("models/staging")),
        resource(
            "model:orders",
            &["model:stg_orders"],
            &["orders"],
            Some("models/marts"),
        ),
        resource("model:customers", &[], &[], Some("models/marts")),
        resource(
            "asset:partner_orders_file",
            &["model:orders"],
            &["orders"],
            Some("python/orders/partner_export"),
        ),
        resource(
            "asset:request_carrier_collection",
            &["asset:partner_orders_file"],
            &["orders"],
            Some("python/fulfillment/carrier_sync"),
        ),
        PlannerResource {
            loader: Some("raw_shipments".to_owned()),
            ..resource("source:raw_shipments", &[], &[], None)
        },
        resource(
            "model:shipments",
            &["source:raw_shipments"],
            &[],
            Some("models/marts"),
        ),
        resource(
            "loader:raw_shipments",
            &["loader:stage_shipments"],
            &[],
            Some("python/loaders"),
        ),
        resource("loader:stage_shipments", &[], &[], Some("python/loaders")),
    ];
    let jobs = sqlbuild_orchestration::runs::models::PublishedJobs {
        selections: std::collections::BTreeMap::from([(
            "carrier_sync".to_owned(),
            std::collections::BTreeSet::from(["asset:request_carrier_collection".to_owned()]),
        )]),
        ..Default::default()
    };
    crate::adhoc::main::adhoc_planner::adhoc_planner().prepare(resources, jobs)
}

/// An expected evaluation in the owned form the planner returns.
pub(super) fn owned_evaluation(
    expected: Result<&[&str], super::test_types::ExpectedProblem>,
) -> Result<Vec<String>, Box<sqlbuild_orchestration::runs::models::SelectionProblem>> {
    use sqlbuild_orchestration::runs::models::{SelectionLocation, SelectionProblem};
    expected
        .map(|keys| keys.iter().map(|key| (*key).to_owned()).collect())
        .map_err(|(code, message, location, suggestions)| {
            Box::new(SelectionProblem {
                code: code.map(str::to_owned),
                message: message.to_owned(),
                help: None,
                location: location.map(|(field, token, start, end)| SelectionLocation {
                    field,
                    token: token.to_owned(),
                    start,
                    end,
                }),
                suggestions: suggestions
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                refused: None,
            })
        })
}

/// A pinned `nightly` job holding `selection`, with its settings.
pub(super) fn nightly_pin(selection: &[&str]) -> serde_json::Value {
    serde_json::json!({"selection": selection, "settings": {"concurrency": 1, "full_refresh": true, "vars": {"region": "north"}}})
}

/// A pinned `weekly` job holding `selection`, with its settings.
pub(super) fn weekly_pin(selection: &[&str]) -> serde_json::Value {
    serde_json::json!({"selection": selection, "settings": {"vars": {"region": "north"}, "warehouse": "REPORTS_XS"}})
}

/// A pinning case's expected keys, pinned jobs and build-wide settings, or refusal fragment, owned.
pub(super) fn expected_pins(
    expected: Result<(&[&str], serde_json::Value, serde_json::Value), &str>,
) -> Result<(Vec<String>, serde_json::Value, serde_json::Value), String> {
    expected
        .map(|(selection, jobs, build)| {
            let selection: Vec<String> = selection.iter().map(|key| (*key).to_owned()).collect();
            (selection, jobs, build)
        })
        .map_err(str::to_owned)
}
