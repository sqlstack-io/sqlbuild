use crate::scope_report::_helpers::paths::ancestry;
use crate::scope_report::main::browse_scope_folders::browse_scope_folders;
use crate::scope_report::main::build_scope_report::build_scope_report;
use crate::scope_report::main::serialize_scope_result::serialize_scope_result;
use crate::scope_report::models::{
    FolderRequest, ReportRequest, ReportSource, ScopeReport, ScopeResult,
};
use crate::scope_report::tests::helpers::{open_filters, orders_facts, text_target};
use crate::scope_report::tests::test_types::{
    DescendantTestCase, ReportTestCase, SerializeTestCase,
};

#[test]
fn given_target_or_prospective_path_when_reporting_then_sections_follow_scope_rules() {
    let test_cases = [
        ReportTestCase {
            description: "an existing model sees globals and inherited declarations",
            target: Some("model:orders"),
            at: None,
            expected_available: &["constant:c", "macro:g"],
            expected_used: &["constant:c"],
            expected_nearby: &["enum:e"],
            expected_chain: &[
                "local sql/models/sales 0",
                "inherited models 1",
                "global global 1",
            ],
            expected_complete: true,
        },
        ReportTestCase {
            description: "a prospective path in the enum's folder sees it but stays incomplete",
            target: None,
            at: Some("sql/models/finance/new.sql"),
            expected_available: &["constant:c", "enum:e", "macro:g"],
            expected_used: &[],
            expected_nearby: &[],
            expected_chain: &[
                "local sql/models/finance 1",
                "inherited models 1",
                "global global 1",
            ],
            expected_complete: false,
        },
    ];
    for test_case in test_cases {
        let (records, lookup, values) = orders_facts();
        let source: ReportSource<'_> = ReportSource {
            records: &records,
            lookup: &lookup,
            values: &values,
        };
        let request: ReportRequest = ReportRequest {
            target: text_target(test_case.target),
            at: test_case.at.map(str::to_owned),
            directory: false,
            filters: open_filters(),
        };

        let report: ScopeReport = build_scope_report(source, &request).expect("valid paths");

        let identities = |reports: &[crate::scope_report::models::DeclarationReport]| {
            reports
                .iter()
                .map(|report| report.identity.clone())
                .collect::<Vec<String>>()
        };
        assert_eq!(
            (
                identities(&report.available),
                identities(&report.used),
                identities(&report.nearby_unavailable),
                report
                    .scope_chain
                    .iter()
                    .map(|entry| format!(
                        "{} {} {}",
                        entry.kind, entry.path, entry.declaration_count
                    ))
                    .collect::<Vec<String>>(),
                report.complete,
            ),
            (
                test_case
                    .expected_available
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect(),
                test_case
                    .expected_used
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect(),
                test_case
                    .expected_nearby
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect(),
                test_case
                    .expected_chain
                    .iter()
                    .map(|item| (*item).to_owned())
                    .collect(),
                test_case.expected_complete,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_browse_result_when_serializing_then_json_is_sorted_compact_ascii() {
    let test_cases = [SerializeTestCase {
        description: "the project root lists the global and model declaration folders",
        folder: ".",
        expected_json: concat!(
            "{\"complete\":true,\"diagnostics\":[],\"folder\":\".\",\"folders\":[",
            "{\"child_count\":1,\"descendant_count\":1,\"kind_counts\":{\"macro\":1},",
            "\"name\":\"global\",\"path\":\"global\",\"used_count\":0},",
            "{\"child_count\":2,\"descendant_count\":2,\"kind_counts\":{\"constant\":1,\"enum\":1},",
            "\"name\":\"models\",\"path\":\"models\",\"used_count\":1}],",
            "\"schema_version\":2}\n"
        ),
    }];
    for test_case in test_cases {
        let (records, lookup, values) = orders_facts();
        let source: ReportSource<'_> = ReportSource {
            records: &records,
            lookup: &lookup,
            values: &values,
        };
        let request: FolderRequest = FolderRequest {
            folder: test_case.folder.to_owned(),
            target: None,
            prospective: false,
            filters: None,
        };

        let result = ScopeResult::Browse(browse_scope_folders(source, &request));

        assert_eq!(
            serialize_scope_result(&result).expect("serializes"),
            test_case.expected_json,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_paths_when_comparing_ancestry_then_whole_components_decide() {
    let test_cases = [
        DescendantTestCase {
            description: "a shared name prefix is not an ancestor",
            path: "sql/models/orders",
            ancestor: "sql/models/order",
            expected_descendant: false,
        },
        DescendantTestCase {
            description: "the project root contains every path",
            path: "sql/models/orders",
            ancestor: ".",
            expected_descendant: true,
        },
        DescendantTestCase {
            description: "paths compare after normalization",
            path: "models\\orders\\..\\sales",
            ancestor: "sql/models/./sales",
            expected_descendant: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            ancestry(test_case.path, test_case.ancestor).expect("valid paths"),
            test_case.expected_descendant,
            "{}",
            test_case.description
        );
    }
}
