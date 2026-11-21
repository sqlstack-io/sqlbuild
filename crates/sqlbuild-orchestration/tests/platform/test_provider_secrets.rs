use chrono::{TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::platform::main::health::missing_provider_secrets::missing_provider_secrets;
use crate::platform::main::health::unallowed_secret_count::unallowed_secret_count;
use crate::platform::models::{
    LocationHealth, LocationHealthObservation, LocationHealthReport, LocationState,
};
use crate::platform::tests::test_types::ProviderSecretsHealthTestCase;

#[test]
fn given_providers_missing_secrets_when_a_location_reports_them_then_only_unallowed_ones_are_a_problem()
 {
    // A snapshot's providers document: settings carry `missing` and never a value.
    let document = json!({"providers": [
        {"name": "orders_api", "path": "providers/orders_api.py", "nodes": ["source:raw_orders"],
         "settings": [
            {"name": "base_url", "env": "ORDERS_API_BASE_URL", "missing": false, "line": 9},
            {"name": "api_key", "env": "ORDERS_API_KEY", "missing": true, "line": 12}]},
        {"name": "export_bucket", "path": "providers/storage.py", "nodes": ["task:export"],
         "settings": [
            {"name": "region", "env": "BUCKET_REGION", "missing": true},
            {"name": "secret", "env": "BUCKET_SECRET", "missing": true, "line": 14}]},
        {"name": "ready", "path": "providers/ready.py", "nodes": [],
         "settings": [{"name": "token", "env": "READY_TOKEN", "missing": false}]}
    ]});
    let test_cases = [
        ProviderSecretsHealthTestCase {
            description: "nothing allowed",
            allowed: &[],
            expected_providers: &[("export_bucket", false), ("orders_api", false)],
            expected_missing: 3,
            expected_state: LocationState::MissingSecrets,
        },
        ProviderSecretsHealthTestCase {
            description: "one provider allowed",
            allowed: &["export_bucket"],
            expected_providers: &[("export_bucket", true), ("orders_api", false)],
            expected_missing: 1,
            expected_state: LocationState::MissingSecrets,
        },
        ProviderSecretsHealthTestCase {
            description: "every provider allowed",
            allowed: &["export_bucket", "orders_api"],
            expected_providers: &[("export_bucket", true), ("orders_api", true)],
            expected_missing: 0,
            expected_state: LocationState::Healthy,
        },
    ];
    for test_case in test_cases {
        let allowed: Vec<String> = test_case
            .allowed
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let providers = missing_provider_secrets(&document, &allowed);
        let listed: Vec<(&str, bool)> = providers
            .iter()
            .map(|provider| (provider.provider.as_str(), provider.allowed))
            .collect();
        assert_eq!(
            listed, test_case.expected_providers,
            "{}",
            test_case.description
        );
        let orders = providers
            .iter()
            .find(|provider| provider.provider == "orders_api");
        let orders = orders.expect("orders_api is missing a secret");
        assert_eq!(
            orders.nodes,
            ["source:raw_orders"],
            "{}",
            test_case.description
        );
        assert_eq!(
            orders
                .settings
                .iter()
                .map(|setting| (
                    setting.field.as_str(),
                    setting.variable.as_str(),
                    setting.line
                ))
                .collect::<Vec<_>>(),
            [("api_key", "ORDERS_API_KEY", Some(12))],
            "{}",
            test_case.description
        );
        let missing = unallowed_secret_count(&providers);
        assert_eq!(
            missing, test_case.expected_missing,
            "{}",
            test_case.description
        );
        let now = Utc::now();
        let health = LocationHealth::ProviderSecrets {
            generation: 4,
            missing,
            providers,
        };
        // The report crosses the wire and back unchanged.
        let wire = serde_json::to_value(&health).expect("health serializes");
        assert_eq!(
            serde_json::from_value::<LocationHealth>(wire).expect("health deserializes"),
            health,
            "{}",
            test_case.description
        );
        let observations = [LocationHealthObservation {
            received_at: now - TimeDelta::seconds(5),
            report: LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                target: "prod".into(),
                health,
            },
        }];
        assert_eq!(
            LocationState::of(&observations, ("prod", now)),
            test_case.expected_state,
            "{}",
            test_case.description
        );
    }
}
