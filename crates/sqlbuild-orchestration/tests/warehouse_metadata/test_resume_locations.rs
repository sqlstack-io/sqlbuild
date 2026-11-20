//! A sync resumes expired holds only where a dynamic-model adapter keeps its pause table.

use crate::warehouse_metadata::main::dynamic_resume::resume_locations::resume_locations;
use crate::warehouse_metadata::models::{DynamicResumeLocation, WarehouseStateLocation};
use crate::warehouse_metadata::tests::test_types::ResumeLocationsTestCase;

#[test]
fn given_sync_locations_when_choosing_where_to_resume_then_the_pause_table_is_included_and_other_adapters_get_none()
 {
    let test_cases = [
        ResumeLocationsTestCase {
            description: "every model in custom schemas still resumes in the target's default schema",
            requested: &[("orders_mart", None), ("finance_mart", Some("token-1"))],
            dynamic: true,
            pause_schema: Some("analytics"),
            expected_locations: &[
                ("orders_mart", None),
                ("finance_mart", Some("token-1")),
                ("analytics", None),
            ],
        },
        ResumeLocationsTestCase {
            description: "a default schema the models already write to is asked once, with its token",
            requested: &[("analytics", Some("token-2"))],
            dynamic: true,
            pause_schema: Some("analytics"),
            expected_locations: &[("analytics", Some("token-2"))],
        },
        ResumeLocationsTestCase {
            description: "a preserved target schema adds no location",
            requested: &[("orders_mart", None)],
            dynamic: true,
            pause_schema: None,
            expected_locations: &[("orders_mart", None)],
        },
        ResumeLocationsTestCase {
            description: "an adapter that cannot build dynamic models resumes nowhere",
            requested: &[("analytics", None)],
            dynamic: false,
            pause_schema: Some("analytics"),
            expected_locations: &[],
        },
    ];
    let location = |schema: &str| WarehouseStateLocation {
        database: Some("warehouse".into()),
        schema: schema.into(),
    };
    let resume = |locations: &[(&str, Option<&str>)]| -> Vec<DynamicResumeLocation> {
        locations
            .iter()
            .map(|(schema, token)| DynamicResumeLocation {
                location: location(schema),
                quiet_token: token.map(str::to_owned),
            })
            .collect()
    };
    for test_case in test_cases {
        assert_eq!(
            resume_locations(
                resume(test_case.requested),
                (test_case.dynamic, test_case.pause_schema.map(location)),
            ),
            resume(test_case.expected_locations),
            "{}",
            test_case.description
        );
    }
}
