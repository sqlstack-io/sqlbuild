use std::collections::BTreeSet;

use crate::cli_text::models::CliStyle;
use crate::format_command::_helpers::selectors::lint_path_prefixes;
use crate::format_command::main::exact_selector_names::exact_selector_names;
use crate::format_command::main::partition_lint_selectors::partition_lint_selectors;
use crate::format_command::main::render_lint_json::render_lint_json;
use crate::format_command::main::render_lint_text::render_lint_text;
use crate::format_command::tests::helpers::{sample_report, texts};
use crate::format_command::tests::test_types::{ReportTestCase, SelectorTestCase};

#[test]
fn given_format_report_when_rendering_then_text_and_json_match_python() {
    let test_cases = [ReportTestCase {
        description: "excerpt, wrapped help, location-only warning and a Rule fix",
        report: sample_report(),
        expected_text: "Formatted files:\n  /p/models/orders.sql\n\nerror[SQLCLEAN004]: Avoid LIMIT\n --> models/orders.sql:2:8\n  |\n2 | SELECT\t1 LIMIT 5\n  |       \t^^^^^^^\n  |\n  = help: Remove the LIMIT clause from the model body because the model must return every row it is\n          expected to produce downstream.\n\nwarning[description-present]: Missing\n --> /other/x.sql:1:1\n\nAPPLIED  SQLCLEAN002  /p/models/orders.sql:4: ok\nCompleted.  FAULT=1  WARN=1  FILES=3\n",
        expected_json: "{\"faults\":1,\"files_checked\":3,\"formatted_files\":[\"/p/models/orders.sql\"],\"rule_fixes\":[{\"code\":\"SQLCLEAN002\",\"file\":\"/p/models/orders.sql\",\"line\":4,\"reason\":\"ok\",\"status\":\"applied\"}],\"violations\":[{\"code\":\"SQLCLEAN004\",\"column\":8,\"end_column\":15,\"end_line\":2,\"engine\":\"native\",\"file\":\"/p/models/orders.sql\",\"line\":2,\"message\":\"Avoid LIMIT\",\"remediation\":\"Remove the LIMIT clause from the model body because the model must return every row it is expected to produce downstream.\",\"severity\":\"fault\"},{\"code\":\"description-present\",\"column\":1,\"end_column\":null,\"end_line\":null,\"engine\":\"sqlbuild\",\"file\":\"/other/x.sql\",\"line\":1,\"message\":\"Missing\",\"remediation\":null,\"severity\":\"warning\"}],\"warnings\":1}\n",
    }];

    for test_case in test_cases {
        let style: CliStyle = CliStyle { use_color: false };
        assert_eq!(
            render_lint_text(&test_case.report, style, "Formatted files:"),
            test_case.expected_text,
            "{}",
            test_case.description
        );
        assert_eq!(
            render_lint_json(&test_case.report).ok().as_deref(),
            Some(test_case.expected_json),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_selectors_when_classifying_then_paths_and_models_match_python() {
    let test_cases = [
        SelectorTestCase {
            description: "path selectors and backslashes become prefixes",
            selectors: texts(&["path:models/staging/ tests\\unit"]),
            expected_prefixes: Some(texts(&["models/staging", "tests/unit"])),
            expected_partition: (texts(&["models/staging", "tests/unit"]), Vec::new()),
            expected_names: None,
        },
        SelectorTestCase {
            description: "model names are exact names, not paths",
            selectors: texts(&["orders customers"]),
            expected_prefixes: None,
            expected_partition: (Vec::new(), texts(&["orders", "customers"])),
            expected_names: Some(texts(&["customers", "orders"])),
        },
        SelectorTestCase {
            description: "graph operators keep a token a model selector",
            selectors: texts(&["path:models/staging +orders", "models/a.sql"]),
            expected_prefixes: None,
            expected_partition: (
                texts(&["models/staging", "models/a.sql"]),
                texts(&["+orders"]),
            ),
            expected_names: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            lint_path_prefixes(&test_case.selectors),
            test_case.expected_prefixes,
            "{}",
            test_case.description
        );
        assert_eq!(
            partition_lint_selectors(&test_case.selectors).ok(),
            Some(test_case.expected_partition),
            "{}",
            test_case.description
        );
        assert_eq!(
            exact_selector_names(&test_case.selectors),
            test_case
                .expected_names
                .map(|names| names.into_iter().collect::<BTreeSet<String>>()),
            "{}",
            test_case.description
        );
    }
}
