use crate::tests::helpers::install_test_identity;
use serde_json::json;
use tempfile::TempDir;

use crate::engine::tests::{helpers, test_types};

#[test]
fn given_owner_and_declaration_layouts_when_evaluating_then_returns_expected_faults()
-> Result<(), String> {
    install_test_identity();
    let oversized_paths: Vec<String> = (0..11)
        .map(|index| format!("models/commerce/_macros/item_{index}.py"))
        .collect();
    let repeated_paths: Vec<String> = (0..11)
        .map(|_| "models/commerce/_macros/orders.py".to_owned())
        .collect();
    let python_and_sql: Vec<String> = (0..6)
        .map(|index| format!("models/commerce/_macros/python/normalise/item_{index}.py"))
        .chain(
            (0..7).map(|index| format!("models/commerce/_macros/sql/normalise/item_{index}.sql")),
        )
        .collect();
    let oversized_sql: Vec<String> = (0..11)
        .map(|index| format!("models/commerce/_macros/sql/normalise/item_{index}.sql"))
        .collect();
    let test_cases = [
        test_types::DomainLayoutTestCase {
            description: "python and sql macro kind folders are counted apart",
            code: "SQBRDECLARATION304",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&python_and_sql),
            expected_codes: &[],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "an oversized sql macro kind folder names its real folder",
            code: "SQBRDECLARATION304",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&oversized_sql),
            expected_codes: &["SQBRDECLARATION304"],
            expected_message_fragments: &[
                "declaration container \"models/commerce/_macros/sql/normalise\" has 11 files",
            ],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "a shared prefix across kind folders is no fault",
            code: "SQBRDECLARATION306",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/python/normalise_order.py".into(),
                "models/commerce/_macros/sql/normalise_person.sql".into(),
            ]),
            expected_codes: &[],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "sql macro files sharing a prefix move into a subfolder of sql",
            code: "SQBRDECLARATION306",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/sql/normalise_order.sql".into(),
                "models/commerce/_macros/sql/normalise_person.sql".into(),
            ]),
            expected_codes: &["SQBRDECLARATION306"],
            expected_message_fragments: &[
                "2 sibling SQL macro files",
                "Move these files into models/commerce/_macros/sql/normalise/",
                "each macro keeps its name",
            ],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "leaf and branch models fault",
            code: "SQBRPROJECT202",
            models: json!([
                helpers::model("models/commerce/mart/summary.sql"),
                helpers::model("models/commerce/mart/orders/model.sql")
            ]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT202"],
            expected_message_fragments: &["mixes direct model"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "default owner depth faults",
            code: "SQBRPROJECT203",
            models: json!([helpers::model(
                "models/commerce/intermediate/enriched/order/clusters_resolved/model.sql"
            )]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT203"],
            expected_message_fragments: &["configured maximum is 1"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "configured owner depth passes",
            code: "SQBRPROJECT203",
            models: json!([helpers::model(
                "models/commerce/intermediate/enriched/order/clusters_resolved/model.sql"
            )]),
            thresholds: json!({"max_subdomain_depth": 2}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &[],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "longest unary owner prefix faults",
            code: "SQBRPROJECT204",
            models: json!([
                helpers::model("models/commerce/mart/order_status/model.sql"),
                helpers::model("models/commerce/mart/order_status_history/model.sql")
            ]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT204"],
            expected_message_fragments: &["order_status"],
            expected_absent_fragments: &["bucket \"order\""],
        },
        test_types::DomainLayoutTestCase {
            description: "each branching owner prefix faults",
            code: "SQBRPROJECT204",
            models: json!([
                helpers::model("models/crm/mart/partner_annotation_export/model.sql"),
                helpers::model("models/crm/mart/partner_annotation_validation/model.sql"),
                helpers::model("models/crm/mart/partner_events/model.sql")
            ]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT204", "SQBRPROJECT204"],
            expected_message_fragments: &["partner\"", "partner_annotation"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "mixed declaration container faults",
            code: "SQBRDECLARATION302",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/direct.py".into(),
                "models/commerce/_macros/scoring/grouped.py".into(),
            ]),
            expected_codes: &["SQBRDECLARATION302"],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "nested declaration bucket faults",
            code: "SQBRDECLARATION303",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/scoring/candidates/item.py".into(),
            ]),
            expected_codes: &["SQBRDECLARATION303"],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "oversized flat declaration container faults",
            code: "SQBRDECLARATION304",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&oversized_paths),
            expected_codes: &["SQBRDECLARATION304"],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "multiple exports in one declaration file count once",
            code: "SQBRDECLARATION304",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&repeated_paths),
            expected_codes: &[],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "generic declaration bucket faults",
            code: "SQBRDECLARATION305",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/utils/item.py".into()
            ]),
            expected_codes: &["SQBRDECLARATION305"],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "shared declaration filename prefix faults",
            code: "SQBRDECLARATION306",
            models: json!([]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_with_macros(&[
                "models/commerce/_macros/normalise_order.py".into(),
                "models/commerce/_macros/normalise_person.py".into(),
            ]),
            expected_codes: &["SQBRDECLARATION306"],
            expected_message_fragments: &["normalise"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "model outside configured levels faults",
            code: "SQBRPROJECT201",
            models: json!([helpers::model("models/commerce/unknown/model.sql")]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT201"],
            expected_message_fragments: &["does not resolve"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "implicit domain grouping prefix faults",
            code: "SQBRPROJECT204",
            models: json!([
                helpers::model("models/partner_annotation_export/mart/model.sql"),
                helpers::model("models/partner_annotation_validation/mart/model.sql")
            ]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT204"],
            expected_message_fragments: &["partner_annotation"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "ambiguous inferred domain root faults",
            code: "SQBRPROJECT201",
            models: json!([helpers::model("models/commerce/staging/mart/model.sql")]),
            thresholds: json!({}),
            layout: json!({}),
            scope_index: helpers::scope_index(),
            expected_codes: &["SQBRPROJECT201"],
            expected_message_fragments: &["ambiguous"],
            expected_absent_fragments: &[],
        },
        test_types::DomainLayoutTestCase {
            description: "explicit nested domain root resolves",
            code: "SQBRPROJECT201",
            models: json!([helpers::model("models/commerce/orders/mart/core/model.sql")]),
            thresholds: json!({}),
            layout: json!({"levels": ["mart"], "domain_roots": ["commerce/orders"]}),
            scope_index: helpers::scope_index(),
            expected_codes: &[],
            expected_message_fragments: &[],
            expected_absent_fragments: &[],
        },
    ];

    for test_case in test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let result = helpers::domain_layout_evaluation(
            &project_dir,
            test_case.code,
            test_case.models,
            test_case.thresholds,
            test_case.layout,
            test_case.scope_index,
        )?;
        let faults = result["faults"]
            .as_array()
            .ok_or_else(|| "faults must be an array".to_owned())?;
        let codes: Vec<&str> = faults
            .iter()
            .filter_map(|fault| fault["code"].as_str())
            .collect();
        let messages = faults
            .iter()
            .map(|fault| {
                format!(
                    "{} {}",
                    fault["message"].as_str().unwrap_or_default(),
                    fault["remediation"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(codes, test_case.expected_codes, "{}", test_case.description);
        for fragment in test_case.expected_message_fragments {
            assert!(messages.contains(fragment), "{}", test_case.description);
        }
        for fragment in test_case.expected_absent_fragments {
            assert!(!messages.contains(fragment), "{}", test_case.description);
        }
    }
    Ok(())
}
