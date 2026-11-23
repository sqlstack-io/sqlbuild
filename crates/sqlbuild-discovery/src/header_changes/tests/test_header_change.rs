use crate::header_changes::models::HeaderChange;
use crate::header_changes::tests::helpers::{edited_change, keys};
use crate::header_changes::tests::test_types::HeaderChangeTestCase;

const STORED: &str =
    "MODEL (materialized view, description 'orders');\nSELECT id FROM __ref(\"customers\")";

#[test]
fn given_an_edited_model_header_when_compared_then_the_change_is_classified() {
    let test_cases = [
        HeaderChangeTestCase {
            description: "an unrecorded model is unsupported",
            stored_contents: STORED,
            edited_contents: Some(STORED),
            recorded: false,
            expected_change: HeaderChange::Unsupported(
                "sql/models/orders.sql has no recorded header".to_owned(),
            ),
        },
        HeaderChangeTestCase {
            description: "an unreadable file is unparsed",
            stored_contents: STORED,
            edited_contents: None,
            recorded: true,
            expected_change: HeaderChange::Unparsed,
        },
        HeaderChangeTestCase {
            description: "the same header is unchanged",
            stored_contents: STORED,
            edited_contents: Some(STORED),
            recorded: true,
            expected_change: HeaderChange::Unchanged,
        },
        HeaderChangeTestCase {
            description: "a changed description and a table materialization are patched",
            stored_contents: STORED,
            edited_contents: Some(
                "MODEL (materialized table, description 'all orders');\nSELECT id FROM __ref(\"customers\")",
            ),
            recorded: true,
            expected_change: HeaderChange::Patched(keys(&["description", "materialized"])),
        },
        HeaderChangeTestCase {
            description: "an incremental materialization is unsupported",
            stored_contents: STORED,
            edited_contents: Some(
                "MODEL (materialized incremental, description 'orders');\nSELECT id FROM __ref(\"customers\")",
            ),
            recorded: true,
            expected_change: HeaderChange::Unsupported(
                "sql/models/orders.sql changed its materialization beyond views and tables".to_owned(),
            ),
        },
        HeaderChangeTestCase {
            description: "another changed key is unsupported",
            stored_contents: STORED,
            edited_contents: Some(
                "MODEL (materialized view, description 'orders', unique_key id);\nSELECT id FROM __ref(\"customers\")",
            ),
            recorded: true,
            expected_change: HeaderChange::Unsupported(
                "sql/models/orders.sql changed its unique_key header key".to_owned(),
            ),
        },
        HeaderChangeTestCase {
            description: "a new reference is unsupported",
            stored_contents: STORED,
            edited_contents: Some(
                "MODEL (materialized view, description 'orders');\nSELECT id FROM __ref(\"products\")",
            ),
            recorded: true,
            expected_change: HeaderChange::Unsupported(
                "sql/models/orders.sql changed what it references".to_owned(),
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            edited_change(&test_case),
            test_case.expected_change,
            "{}",
            test_case.description
        );
    }
}
