use crate::declaration_files::models::{CollectionKind, CollectionRequest};
use crate::declaration_files::tests::helpers::session_reads;
use crate::declaration_files::tests::test_types::SessionReadTestCase;

const FILES: &[(&str, &str)] = &[
    (
        "sql/enums/status.sql",
        "ENUM (name order_status, members [PLACED]);\n",
    ),
    (
        "sql/constants/limits.sql",
        "CONSTANT (name max_orders, value 10);\n",
    ),
    (
        "sql/macros/python/cents.py",
        "def cents(value):\n    return value\n",
    ),
    ("sql/schemas/orders.sql", "SCHEMA (name orders_shape);\n"),
    (
        "sql/audits/generic/positive.sql",
        "AUDIT (name positive);\nSELECT 1\n",
    ),
];

const SHARED_REQUESTS: &[CollectionRequest] = &[
    CollectionRequest {
        kind: CollectionKind::Enums,
        isolate_kind: false,
    },
    CollectionRequest {
        kind: CollectionKind::Constants,
        isolate_kind: false,
    },
    CollectionRequest {
        kind: CollectionKind::Macros,
        isolate_kind: false,
    },
    CollectionRequest {
        kind: CollectionKind::ModelSchemas,
        isolate_kind: false,
    },
    CollectionRequest {
        kind: CollectionKind::Audits,
        isolate_kind: false,
    },
];
const ISOLATED_REQUESTS: &[CollectionRequest] = &[
    CollectionRequest {
        kind: CollectionKind::Enums,
        isolate_kind: false,
    },
    CollectionRequest {
        kind: CollectionKind::Constants,
        isolate_kind: true,
    },
];

#[test]
fn given_session_when_reading_collections_then_layout_is_walked_once_and_reads_are_retained() {
    let test_cases = [
        SessionReadTestCase {
            description: "every kind shares one layout",
            files: FILES,
            requests: SHARED_REQUESTS,
            expected_counts: &[1, 1, 1, 1, 1],
            expected_layouts: 1,
        },
        SessionReadTestCase {
            description: "an isolated kind walks its own layout",
            files: FILES,
            requests: ISOLATED_REQUESTS,
            expected_counts: &[1, 1],
            expected_layouts: 2,
        },
    ];

    for test_case in test_cases {
        let (counts, layouts, reused) = session_reads(test_case.files, test_case.requests);

        assert_eq!(
            counts, test_case.expected_counts,
            "{}",
            test_case.description
        );
        assert_eq!(
            layouts, test_case.expected_layouts,
            "{}",
            test_case.description
        );
        assert!(reused, "{}", test_case.description);
    }
}
